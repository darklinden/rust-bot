use crate::feature::{msg_segment_from_string, Feature, MessageContext};

fn reply(text: &str) -> bot_lib::structs::MessageSegment {
    msg_segment_from_string(text.to_string())
}
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bot_lib::structs::{ImageData, MessageSegment};
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ─── Constants ───────────────────────────────────────────────────────────────

const SESSION_TTL_SECS: u64 = 15 * 60;
const MAX_POLL_ATTEMPTS: u32 = 200;
const POLL_INTERVAL_SECS: u64 = 3;

const SIZE_OPTIONS: &[(&str, &str)] = &[
    // A 比例
    ("A1", "auto"),
    ("A2", "1:1"),
    ("A3", "3:2"),
    ("A4", "2:3"),
    ("A5", "16:9"),
    ("A6", "9:16"),
    ("A7", "4:3"),
    ("A8", "3:4"),
    ("A9", "21:9"),
    ("A10", "9:21"),
    ("A11", "1:3"),
    ("A12", "3:1"),
    ("A13", "2:1"),
    ("A14", "1:2"),
    // B 固定像素
    ("B1", "1024x1024"),
    ("B2", "1536x1024"),
    ("B3", "1024x1536"),
    ("B4", "2048x2048"),
    ("B5", "2048x1152"),
    ("B6", "3840x2160"),
    ("B7", "2160x3840"),
];

const RATIO_COUNT: usize = 14;

// ─── API types ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct GrsaiGenerateRequest {
    #[serde(rename = "webHook")]
    web_hook: String,
    model: String,
    prompt: String,
    #[serde(rename = "aspectRatio")]
    size: String,
    n: i32,
    #[serde(skip_serializing_if = "String::is_empty", rename = "quality")]
    quality: String,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "urls")]
    urls: Vec<String>,
}

// ─── Session types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum GptImageStage {
    SelectingSize,
    SelectingQuality,
    CollectingRefImages,
    Confirming,
    Polling(String),
}

#[derive(Clone)]
struct GptImageSession {
    prompt: String,
    size: Option<String>,
    quality: Option<String>,     // "auto" or "high"
    model: Option<String>,       // resolved from quality
    ref_image_urls: Vec<String>, // QQ CDN URLs
    stage: GptImageStage,
    user_id: i64,
    created_at: Instant,
    waiting_abandon: bool, // true after asking abandon question
}

impl GptImageSession {
    fn new(prompt: String, user_id: i64, ref_urls: Vec<String>) -> Self {
        Self {
            prompt,
            size: None,
            quality: None,
            model: None,
            ref_image_urls: ref_urls,
            stage: GptImageStage::SelectingSize,
            user_id,
            created_at: Instant::now(),
            waiting_abandon: false,
        }
    }
}

static SESSIONS: once_cell::sync::Lazy<Arc<Mutex<HashMap<i64, GptImageSession>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

// ─── Session helpers ─────────────────────────────────────────────────────────

fn session_get(user_id: i64) -> Option<GptImageSession> {
    let mut map = SESSIONS.lock().unwrap();
    let entry = map.get(&user_id)?;
    if entry.created_at.elapsed().as_secs() > SESSION_TTL_SECS {
        map.remove(&user_id);
        return None;
    }
    Some(GptImageSession {
        prompt: entry.prompt.clone(),
        size: entry.size.clone(),
        quality: entry.quality.clone(),
        model: entry.model.clone(),
        ref_image_urls: entry.ref_image_urls.clone(),
        stage: entry.stage.clone(),
        user_id: entry.user_id,
        created_at: entry.created_at,
        waiting_abandon: entry.waiting_abandon,
    })
}

fn session_upsert(s: GptImageSession) {
    let mut map = SESSIONS.lock().unwrap();
    map.insert(s.user_id, s);
}

fn session_remove(user_id: i64) {
    let mut map = SESSIONS.lock().unwrap();
    map.remove(&user_id);
}

// ─── Image extraction ────────────────────────────────────────────────────────

fn extract_image_urls(context: &MessageContext) -> Vec<String> {
    context
        .message
        .iter()
        .filter_map(|seg| {
            if seg.get("type").and_then(|v| v.as_str()) == Some("image") {
                seg.pointer("/data/url")
                    .and_then(|v| v.as_str())
                    .filter(|u| !u.is_empty())
                    .map(|u| u.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn extract_image_url_from_segment(msg: &Value) -> Option<String> {
    if msg.get("type").and_then(|v| v.as_str()) == Some("image") {
        msg.pointer("/data/url")
            .and_then(|v| v.as_str())
            .filter(|u| !u.is_empty())
            .map(|u| u.to_string())
    } else {
        None
    }
}

// ─── Size parsing ────────────────────────────────────────────────────────────

fn parse_size(input: &str) -> Option<String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Some("auto".to_string());
    }

    // Parse numeric index
    if let Ok(idx) = trimmed.parse::<usize>() {
        if idx >= 1 && idx <= SIZE_OPTIONS.len() {
            return Some(SIZE_OPTIONS[idx - 1].1.to_string());
        }
        return None;
    }

    // Parse prefixed index like "A1" or "B3"
    let upper = trimmed.to_uppercase();
    for (label, value) in SIZE_OPTIONS {
        if upper == *label {
            return Some(value.to_string());
        }
    }

    // Accept free-text like "1024x1024" or "1024*1024" (normalize * to x)
    let normalized = trimmed.replace('*', "x").to_lowercase();
    if normalized.contains('x') {
        let parts: Vec<&str> = normalized.split('x').collect();
        if parts.len() == 2 {
            let w = parts[0].trim().parse::<u32>().ok()?;
            let h = parts[1].trim().parse::<u32>().ok()?;
            if w > 0 && h > 0 && w <= 10000 && h <= 10000 {
                return Some(format!("{}x{}", w, h));
            }
        }
    }

    None
}

// ─── SVG rendering ───────────────────────────────────────────────────────────

static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/SourceHanSans-Regular.otf");
const FONT_FAMILY: &str = "Source Han Sans CN";

const CARD_WIDTH: u32 = 800;
const PADDING_X: f64 = 28.0;
const HEADER_H: f64 = 52.0;
const ROW_H: f64 = 34.0;
const SECTION_H: f64 = 32.0;

fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_size_card_svg() -> (String, u32) {
    let w = CARD_WIDTH as f64;
    let font = FONT_FAMILY;
    let mut y = 0.0f64;
    let mut svg_lines: Vec<String> = Vec::new();

    // Header
    svg_lines.push(format!(
        r##"<rect x="0" y="{y}" width="{w}" height="{HEADER_H}" rx="10" fill="#1A237E"/>"##
    ));
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="20" font-weight="bold" fill="#FFF">GPT Image - Choose Size</text>"##,
        y + 35.0
    ));
    y += HEADER_H + 12.0;

    // ── Section A ──
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="15" font-weight="bold" fill="#555">Ratio</text>"##,
        y + 22.0
    ));
    y += SECTION_H;

    let col1_x = PADDING_X;
    let label_w = 48.0;

    for i in 0..RATIO_COUNT {
        let col = (i % 3) as f64;
        let row = (i / 3) as f64;
        let cx = col1_x + col * 250.0;
        let cy = y + row * ROW_H;

        let (label, value) = SIZE_OPTIONS[i];
        svg_lines.push(format!(
            r##"<text x="{}" y="{}" font-family="{}" font-size="14" font-weight="bold" fill="#1A237E">{}</text>"##,
            cx, cy + 24.0, font, label
        ));
        svg_lines.push(format!(
            r##"<text x="{}" y="{}" font-family="{}" font-size="14" fill="#333">{}</text>"##,
            cx + label_w,
            cy + 24.0,
            font,
            value
        ));
    }

    let ratio_rows = (RATIO_COUNT + 2) / 3;
    y += ratio_rows as f64 * ROW_H + 12.0;

    // ── Section B ──
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="15" font-weight="bold" fill="#555">Fixed Size</text>"##,
        y + 22.0
    ));
    y += SECTION_H;

    let pixel_count = SIZE_OPTIONS.len() - RATIO_COUNT;
    for i in 0..pixel_count {
        let col = (i % 3) as f64;
        let row = (i / 3) as f64;
        let cx = col1_x + col * 250.0;
        let cy = y + row * ROW_H;

        let (label, value) = SIZE_OPTIONS[RATIO_COUNT + i];
        svg_lines.push(format!(
            r##"<text x="{}" y="{}" font-family="{}" font-size="14" font-weight="bold" fill="#1A237E">{}</text>"##,
            cx, cy + 24.0, font, label
        ));
        svg_lines.push(format!(
            r##"<text x="{}" y="{}" font-family="{}" font-size="14" fill="#333">{}</text>"##,
            cx + label_w,
            cy + 24.0,
            font,
            value
        ));
    }

    let pixel_rows = (pixel_count + 2) / 3;
    y += pixel_rows as f64 * ROW_H + 14.0;

    // Hint
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="12" fill="#999">Enter index number or size (e.g. 1024x1024), default A1 (auto)</text>"##,
        y + 18.0
    ));
    y += 30.0;

    let total_h = y;
    let mut svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{total_h}">"#);
    svg.push_str(&format!(
        r##"<rect width="{w}" height="{total_h}" rx="12" fill="#FFFFFF"/>"##
    ));
    for line in &svg_lines {
        svg.push_str(line);
    }
    svg.push_str("</svg>");

    (svg, total_h.ceil() as u32)
}

fn build_confirmation_svg(
    prompt: &str,
    size: &str,
    quality_label: &str,
    model: &str,
    ref_count: usize,
    thumbnails_svg: &str,
    thumbs_h: f64,
) -> String {
    let w = CARD_WIDTH as f64;
    let font = FONT_FAMILY;
    let mut y = 0.0f64;
    let mut svg_lines: Vec<String> = Vec::new();

    // Header
    svg_lines.push(format!(
        r##"<rect x="0" y="{y}" width="{w}" height="{HEADER_H}" rx="10" fill="#2E7D32"/>"##
    ));
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="20" font-weight="bold" fill="#FFF">确认生成</text>"##,
        y + 35.0
    ));
    y += HEADER_H + 14.0;

    // Prompt
    let prompt_escaped = svg_escape(prompt);
    // Wrap prompt text
    let max_chars = 55;
    let prompt_lines: Vec<String> = prompt_escaped
        .chars()
        .collect::<Vec<char>>()
        .chunks(max_chars)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect();
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="13" font-weight="bold" fill="#888">Prompt</text>"##,
        y + 18.0
    ));
    y += 24.0;
    for line in &prompt_lines {
        svg_lines.push(format!(
            r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="14" fill="#222">{}</text>"##,
            y + 20.0,
            svg_escape(line)
        ));
        y += 22.0;
    }
    y += 8.0;

    // Divider
    svg_lines.push(format!(
        r##"<line x1="{PADDING_X}" y1="{y}" x2="{}" y2="{y}" stroke="#E0E0E0" stroke-width="1"/>"##,
        w - PADDING_X
    ));
    y += 14.0;

    // Info rows
    let info_x = PADDING_X + 80.0;
    let ref_count_str = format!("{} 张", ref_count);
    let info_items: Vec<(&str, &str)> = vec![
        ("尺寸", size),
        ("画质", quality_label),
        ("模型", model),
        ("参考图", &ref_count_str),
    ];
    for (label, value) in &info_items {
        svg_lines.push(format!(
            r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="14" font-weight="bold" fill="#555">{label}</text>"##,
            y + 20.0
        ));
        svg_lines.push(format!(
            r##"<text x="{info_x}" y="{}" font-family="{font}" font-size="14" fill="#222">{}</text>"##,
            y + 20.0,
            svg_escape(value)
        ));
        y += 26.0;
    }
    y += 6.0;

    // Thumbnails
    if !thumbnails_svg.is_empty() {
        svg_lines.push(format!(
            r##"<line x1="{PADDING_X}" y1="{y}" x2="{}" y2="{y}" stroke="#E0E0E0" stroke-width="1"/>"##,
            w - PADDING_X
        ));
        y += 10.0;
        svg_lines.push(format!(
            r##"<g transform="translate(0, {y})">"##
        ));
        svg_lines.push(thumbnails_svg.to_string());
        svg_lines.push("</g>".to_string());
        y += thumbs_h + 10.0;
    }

    // Footer hint
    svg_lines.push(format!(
        r##"<line x1="{PADDING_X}" y1="{y}" x2="{}" y2="{y}" stroke="#E0E0E0" stroke-width="1"/>"##,
        w - PADDING_X
    ));
    y += 12.0;
    svg_lines.push(format!(
        r##"<text x="{PADDING_X}" y="{}" font-family="{font}" font-size="13" fill="#888">回复 "确认/是/y" 提交生成，回复 "否/no/n" 取消</text>"##,
        y + 18.0
    ));
    y += 30.0;

    let total_h = y;
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{total_h}">"#
    );
    svg.push_str(&format!(
        r##"<rect width="{w}" height="{total_h}" rx="12" fill="#FFFFFF"/>"##
    ));
    for line in &svg_lines {
        svg.push_str(line);
    }
    svg.push_str("</svg>");

    svg
}

fn render_svg_to_png(svg: &str) -> Vec<u8> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(FONT_DATA.to_vec());

    let mut opt = usvg::Options::default();
    *opt.fontdb_mut() = fontdb;

    let tree = match usvg::Tree::from_str(svg, &opt) {
        Ok(t) => t,
        Err(e) => {
            log::error!("[grsai-gpt-image] SVG parse failed: {}", e);
            return vec![];
        }
    };

    let size = tree.size().to_int_size();
    let w = size.width().max(1);
    let h = size.height().max(1);
    let mut pixmap = match tiny_skia::Pixmap::new(w, h) {
        Some(p) => p,
        None => {
            log::error!("[grsai-gpt-image] Pixmap creation failed");
            return vec![];
        }
    };

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    match pixmap.encode_png() {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("[grsai-gpt-image] PNG encode failed: {}", e);
            vec![]
        }
    }
}

fn detect_mime_from_bytes(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 4 && &bytes[..4] == b"\x89PNG" {
        "image/png"
    } else if bytes.len() >= 3 && &bytes[..3] == b"\xff\xd8\xff" {
        "image/jpeg"
    } else if bytes.len() >= 4
        && &bytes[..4] == b"RIFF"
        && bytes.len() >= 12
        && &bytes[8..12] == b"WEBP"
    {
        "image/webp"
    } else if bytes.len() >= 4 && &bytes[..4] == b"GIF8" {
        "image/gif"
    } else {
        "image/png"
    }
}

async fn download_image_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{}", e))?;

    let resp = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36",
            ),
        )
        .header(
            reqwest::header::REFERER,
            HeaderValue::from_static("https://multimedia.nt.qq.com.cn/"),
        )
        .header(reqwest::header::ACCEPT, HeaderValue::from_static("*/*"))
        .send()
        .await
        .map_err(|e| format!("下载图片失败：{}", e))?;

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("读取图片数据失败：{}", e))
}

/// Build thumbnails SVG from pre-downloaded image data.
fn build_thumbnails_from_data(images: &[(Vec<u8>, &str)]) -> (String, f64) {
    if images.is_empty() {
        return (String::new(), 0.0);
    }

    let thumb_w = 110.0;
    let thumb_h = 110.0;
    let gap = 12.0;
    let label_h = 18.0;
    let max_per_row = 5;

    let mut svg_lines: Vec<String> = Vec::new();
    let font = FONT_FAMILY;

    for (i, (img_bytes, mime)) in images.iter().enumerate() {
        let row = (i / max_per_row) as f64;
        let col = (i % max_per_row) as f64;
        let x = PADDING_X + col * (thumb_w + gap);
        let img_y = row * (thumb_h + label_h + 8.0);

        let b64 = BASE64.encode(img_bytes);
        let data_uri = format!("data:{};base64,{}", mime, b64);

        svg_lines.push(format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="#F5F5F5" stroke="#DDD" stroke-width="1"/>"##,
            x, img_y, thumb_w, thumb_h
        ));
        svg_lines.push(format!(
            r##"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="xMidYMid slice" xlink:href="{}"/>"##,
            x, img_y, thumb_w, thumb_h, data_uri
        ));
        svg_lines.push(format!(
            r##"<text x="{}" y="{}" font-family="{}" font-size="11" fill="#888" text-anchor="middle">{}</text>"##,
            x + thumb_w / 2.0, img_y + thumb_h + 16.0, font, i + 1
        ));
    }

    let rows = ((images.len() + max_per_row - 1) / max_per_row) as f64;
    let total_h = rows * (thumb_h + label_h + 8.0);

    (svg_lines.join(""), total_h)
}

// ─── API client ──────────────────────────────────────────────────────────────

fn grsai_host() -> String {
    env::var("GRSAI_HOST")
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string()
}

fn grsai_api_key() -> String {
    env::var("GRSAI_API_KEY").unwrap_or_default()
}

async fn submit_and_poll(
    prompt: String,
    model: String,
    size: String,
    quality: String,
    ref_urls: Vec<String>,
    user_id: i64,
    resolve_quality_label: fn(&str, &str) -> String,
    resolve_model_label: fn(&str) -> String,
) {
    let host = grsai_host();
    let api_key = grsai_api_key();

    if host.is_empty() || api_key.is_empty() {
        send_error_to_user(
            user_id,
            "GRSAI API 未配置，请联系管理员设置 GRSAI_HOST 和 GRSAI_API_KEY。",
        );
        return;
    }

    let quality_label = resolve_quality_label(&quality, &model);
    let model_label = resolve_model_label(&model);

    // ── Submit ──
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let submit_url = format!("{}/v1/draw/completions", host);
    let body = GrsaiGenerateRequest {
        web_hook: "-1".to_string(),
        model,
        prompt,
        size,
        n: 1,
        quality,
        stream: false,
        urls: ref_urls,
    };

    let resp = match client
        .post(&submit_url)
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("提交生成请求失败：{}", e);
            log::error!("[grsai-gpt-image] {msg}");
            send_error_to_user(user_id, &msg);
            return;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let msg = format!("grsai 提交返回 {}: {}", status, text);
        log::error!("[grsai-gpt-image] {msg}");
        send_error_to_user(user_id, &msg);
        return;
    }

    let submit_text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            let msg = format!("读取提交响应失败：{}", e);
            log::error!("[grsai-gpt-image] {msg}");
            send_error_to_user(user_id, &msg);
            return;
        }
    };

    #[derive(serde::Deserialize)]
    struct SubmitResponse {
        data: Option<SubmitData>,
    }
    #[derive(serde::Deserialize)]
    struct SubmitData {
        id: Option<String>,
    }

    let provider_task_id = match serde_json::from_str::<SubmitResponse>(&submit_text)
        .ok()
        .and_then(|r| r.data)
        .and_then(|d| d.id)
    {
        Some(id) => id,
        None => {
            let msg = format!(
                "grsai 未返回任务 ID，响应：{}",
                &submit_text[..submit_text.len().min(300)]
            );
            log::error!("[grsai-gpt-image] {msg}");
            send_error_to_user(user_id, &msg);
            return;
        }
    };

    // ── Poll ──
    let poll_url = format!("{}/v1/draw/result", host);
    let mut attempts = 0u32;

    let image_url = loop {
        if attempts >= MAX_POLL_ATTEMPTS {
            let msg = format!("图片生成超时（任务ID: {}），请稍后重试。", provider_task_id);
            send_error_to_user(user_id, &msg);
            return;
        }

        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        attempts += 1;

        let poll_body = serde_json::json!({ "id": &provider_task_id });

        let poll_resp = match client
            .post(&poll_url)
            .bearer_auth(&api_key)
            .json(&poll_body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("[grsai-gpt-image] Poll attempt {} error: {}", attempts, e);
                continue;
            }
        };

        if !poll_resp.status().is_success() {
            log::warn!(
                "[grsai-gpt-image] Poll attempt {} returned {}",
                attempts,
                poll_resp.status()
            );
            continue;
        }

        let poll_text = match poll_resp.text().await {
            Ok(t) => t,
            Err(_) => continue,
        };

        #[derive(serde::Deserialize)]
        struct TaskResponse {
            data: Option<TaskData>,
            msg: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct TaskData {
            status: Option<String>,
            results: Option<Vec<TaskResult>>,
            failure_reason: Option<String>,
            error: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct TaskResult {
            url: Option<String>,
        }

        let poll: TaskResponse = match serde_json::from_str(&poll_text) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let status = poll
            .data
            .as_ref()
            .and_then(|d| d.status.as_deref())
            .unwrap_or("pending");

        if status == "failed" || status == "error" {
            let reason = poll
                .data
                .as_ref()
                .and_then(|d| d.failure_reason.as_deref().or(d.error.as_deref()))
                .or(poll.msg.as_deref())
                .unwrap_or("未知错误");
            let msg = format!("图片生成失败：{}", reason);
            send_error_to_user(user_id, &msg);
            return;
        }

        if status == "succeeded" {
            if let Some(url) = poll
                .data
                .as_ref()
                .and_then(|d| d.results.as_ref())
                .and_then(|r| r.first())
                .and_then(|r| r.url.as_ref())
            {
                break url.clone();
            }
            // succeeded but no URL — unusual, keep polling
        }
    };

    // ── Download result ──
    let img_bytes = match download_image_bytes(&image_url).await {
        Ok(b) => b,
        Err(e) => {
            send_error_to_user(user_id, &format!("下载生成图片失败：{}", e));
            return;
        }
    };

    let file_uri = match crate::media_file::write_media(&img_bytes, "png") {
        Ok(uri) => uri,
        Err(e) => {
            send_error_to_user(user_id, &format!("保存图片失败：{}", e));
            return;
        }
    };

    deliver_image(user_id, file_uri, &quality_label, &model_label);
}

// These are called from the polling task and need to send messages back to the user.
// They use a global sender that gets set when the feature is created.

type ReplySender = tokio::sync::mpsc::Sender<GptImageResult>;

pub struct GptImageResult {
    pub user_id: i64,
    pub segment: MessageSegment,
}

static REPLY_SENDER: once_cell::sync::Lazy<Arc<Mutex<Option<ReplySender>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

fn send_error_to_user(user_id: i64, msg: &str) {
    let sender = REPLY_SENDER.lock().unwrap();
    if let Some(ref tx) = *sender {
        let _ = tx.try_send(GptImageResult {
            user_id,
            segment: msg_segment_from_string(msg.to_string()),
        });
    }
}

fn deliver_image(user_id: i64, file_uri: String, quality_label: &str, model_label: &str) {
    let sender = REPLY_SENDER.lock().unwrap();
    if let Some(ref tx) = *sender {
        let caption = format!("生成完成 | 画质: {} | 模型: {}", quality_label, model_label);
        // Send caption first, then image. Since we can only send one segment per result,
        // we send a text segment and the image. We use two separate sends.
        let _ = tx.try_send(GptImageResult {
            user_id,
            segment: msg_segment_from_string(caption),
        });
        let _ = tx.try_send(GptImageResult {
            user_id,
            segment: MessageSegment::Image {
                data: ImageData {
                    file: file_uri,
                    summary: Some("generated.png".to_string()),
                    sub_type: None,
                    url: None,
                    file_size: None,
                },
            },
        });
    }
}

// ─── Whitelist ───────────────────────────────────────────────────────────────

fn is_whitelisted(user_id: i64) -> bool {
    let raw = env::var("GPT_IMAGE_ACCEPT_SENDERS").unwrap_or_default();
    let allowed: Vec<i64> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    let ok = allowed.iter().any(|id| *id == user_id);
    if !ok {
        log::info!(
            "[grsai-gpt-image] Whitelist check: user_id={}, allowed={:?}, raw_env={:?}",
            user_id,
            allowed,
            raw
        );
    }
    ok
}

fn is_continuation_text(text: &str) -> bool {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();

    // Size index: 1-21
    if let Ok(n) = trimmed.parse::<usize>() {
        if n >= 1 && n <= 21 {
            return true;
        }
    }

    // Quality options
    if matches!(lower.as_str(), "auto" | "high") {
        return true;
    }

    // Skip ref images
    if matches!(lower.as_str(), "skip" | "跳过") {
        return true;
    }

    // Confirmation yes
    if matches!(lower.as_str(), "确认" | "是" | "y" | "yes" | "ok") {
        return true;
    }

    // Confirmation no
    if matches!(lower.as_str(), "否" | "no" | "n") {
        return true;
    }

    // Abandon question response
    if trimmed == "1" || trimmed == "2" {
        return true;
    }

    // Size label (A1-A14, B1-B7) or custom size (WxH / W*H)
    if parse_size(trimmed).is_some() {
        return true;
    }

    false
}

// ─── Feature impl ────────────────────────────────────────────────────────────

pub struct GptImageFeature {
    #[allow(dead_code)]
    sender: ReplySender,
}

impl GptImageFeature {
    pub fn new(sender: ReplySender) -> Self {
        // Store sender in global for use by async polling tasks
        let mut global = REPLY_SENDER.lock().unwrap();
        *global = Some(sender.clone());
        Self { sender }
    }

    pub fn feature_id() -> &'static str {
        "grsai_gpt_image"
    }

    pub fn feature_name() -> &'static str {
        "GPT 图片生成: -img <prompt> 通过 GPT 生成图片"
    }
}

#[async_trait]
impl Feature for GptImageFeature {
    fn feature_id(&self) -> &str {
        GptImageFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        GptImageFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Image messages: only match when a CollectingRefImages session exists
        if msg_type == "image" {
            if extract_image_url_from_segment(msg).is_some() {
                let map = SESSIONS.lock().unwrap();
                return map.values().any(|s| {
                    s.stage == GptImageStage::CollectingRefImages
                        && s.created_at.elapsed().as_secs() <= SESSION_TTL_SECS
                });
            }
            return false;
        }

        if msg_type != "text" {
            return false;
        }

        let text = msg["data"]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        // New session trigger — always match
        if text.to_lowercase().starts_with("-img ") {
            return true;
        }

        // Continuation messages: only match when an active session exists
        // AND the text looks like a plausible continuation reply
        let map = SESSIONS.lock().unwrap();
        let has_active = map
            .values()
            .any(|s| s.created_at.elapsed().as_secs() <= SESSION_TTL_SECS);
        drop(map);

        if !has_active {
            return false;
        }

        is_continuation_text(&text)
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let user_id = context.user_id;

        // Silently ignore group messages — this feature is private-chat only
        if context.group_id.is_some_and(|gid| gid != 0) {
            return None;
        }

        // Never respond to own messages (prevents infinite loop from message_sent events)
        if user_id == context.self_id {
            return None;
        }

        // Check whitelist
        if !is_whitelisted(user_id) {
            return Some(msg_segment_from_string(
                "您没有权限使用 GPT 图片生成功能。".to_string(),
            ));
        }

        let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let text = if msg_type == "text" {
            msg["data"]["text"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string()
        } else {
            String::new()
        };

        // ── New session ──
        if text.to_lowercase().starts_with("-img ") {
            let prompt = text[5..].trim().to_string();
            if prompt.is_empty() {
                return Some(msg_segment_from_string(
                    "用法: -img <提示词>\n例如: -img a cute cat".to_string(),
                ));
            }

            // Check existing session
            if let Some(mut existing) = session_get(user_id) {
                match existing.stage {
                    GptImageStage::Polling(_) => {
                        // Old session still polling — start new one anyway
                        session_remove(user_id);
                    }
                    _ => {
                        // Old session in parameter collection — ask whether to abandon
                        existing.waiting_abandon = true;
                        session_upsert(existing);
                        return Some(msg_segment_from_string(
                            "当前有未完成的生成流程，是否放弃？\n1. 否（继续当前流程）\n2. 是（开始新流程）"
                                .to_string(),
                        ));
                    }
                }
            }

            let ref_urls = extract_image_urls(context);

            let session = GptImageSession::new(prompt, user_id, ref_urls);
            session_upsert(session);

            // Render and send size selection card
            let (svg, _) = build_size_card_svg();
            let png = render_svg_to_png(&svg);
            if png.is_empty() {
                return Some(msg_segment_from_string(
                    "渲染尺寸选择卡片失败，请重试。".to_string(),
                ));
            }

            match crate::media_file::write_media(&png, "png") {
                Ok(file_uri) => {
                    return Some(MessageSegment::Image {
                        data: ImageData {
                            file: file_uri,
                            summary: Some("size_select.png".to_string()),
                            sub_type: None,
                            url: None,
                            file_size: None,
                        },
                    });
                }
                Err(e) => {
                    log::error!("[grsai-gpt-image] 写入尺寸卡片失败: {}", e);
                    return Some(msg_segment_from_string(
                        "渲染尺寸选择卡片失败，请重试。".to_string(),
                    ));
                }
            }
        }

        // ── Handle existing session continuation ──
        let mut session = match session_get(user_id) {
            Some(s) => s,
            None => return None,
        };

        // Handle "abandon?" response — only when flag is set
        if session.waiting_abandon {
            session.waiting_abandon = false;
            let trimmed = text.trim().to_lowercase();
            if trimmed == "2" || trimmed == "是" || trimmed == "yes" {
                session_remove(user_id);
                return Some(reply("已取消当前流程。发送 -img <提示词> 开始新流程。"));
            }
            // "1", "否", "no", or anything else — continue current session
            session_upsert(session.clone());
            match session.stage {
                GptImageStage::SelectingSize => {
                    let (svg, _) = build_size_card_svg();
                    let png = render_svg_to_png(&svg);
                    if let Ok(file_uri) = crate::media_file::write_media(&png, "png") {
                        return Some(MessageSegment::Image {
                            data: ImageData {
                                file: file_uri,
                                summary: Some("size_select.png".to_string()),
                                sub_type: None,
                                url: None,
                                file_size: None,
                            },
                        });
                    }
                }
                GptImageStage::SelectingQuality => {
                    return Some(msg_segment_from_string(
                        "选择画质：\n1. Auto（标准）\n2. High（高质量）".to_string(),
                    ));
                }
                _ => {
                    // Re-send appropriate prompt based on current stage
                }
            }
            return None;
        }

        match session.stage.clone() {
            GptImageStage::SelectingSize => {
                // Parse size selection
                let size = match parse_size(&text) {
                    Some(s) => s,
                    None => {
                        return Some(reply(
                            "无效的尺寸选择。请直接输入序号（1-21）或尺寸值（如 1024x1024）。",
                        ));
                    }
                };

                session.size = Some(size);
                session.stage = GptImageStage::SelectingQuality;
                session.created_at = Instant::now();
                session_upsert(session);

                Some(msg_segment_from_string(
                    "选择画质：\n1. Auto（标准）\n2. High（高质量）".to_string(),
                ))
            }

            GptImageStage::SelectingQuality => {
                let trimmed = text.trim().to_lowercase();
                let (quality, model) = match trimmed.as_str() {
                    "1" | "auto" => ("auto".to_string(), "gpt-image-2".to_string()),
                    "2" | "high" => ("high".to_string(), "gpt-image-2-vip".to_string()),
                    _ => {
                        return Some(reply("无效选择。请回复 1 (Auto) 或 2 (High)。"));
                    }
                };

                session.quality = Some(quality);
                session.model = Some(model);
                session.stage = if session.ref_image_urls.is_empty() {
                    GptImageStage::CollectingRefImages
                } else {
                    GptImageStage::Confirming
                };
                session.created_at = Instant::now();

                if session.ref_image_urls.is_empty() {
                    session_upsert(session);
                    Some(reply("请发送参考图片（可多张），无需参考图请回复 skip。"))
                } else {
                    // Already have ref images, go to confirmation
                    session_upsert(session.clone());
                    return build_confirmation_response(&session).await;
                }
            }

            GptImageStage::CollectingRefImages => {
                let trimmed = text.trim().to_lowercase();

                if trimmed == "skip" || trimmed == "跳过" {
                    session.stage = GptImageStage::Confirming;
                    session.created_at = Instant::now();
                    session_upsert(session.clone());
                    return build_confirmation_response(&session).await;
                }

                // Check for images in this message segment or in the full context
                let new_urls = extract_image_urls(context);
                if new_urls.is_empty() {
                    // Check if this is a text message without images — might still be "skip"
                    return Some(msg_segment_from_string(
                        "请发送参考图片，或回复 skip 跳过。".to_string(),
                    ));
                }

                session.ref_image_urls.extend(new_urls);
                session.created_at = Instant::now();
                session.stage = GptImageStage::Confirming;
                session_upsert(session.clone());

                // Go directly to confirmation
                build_confirmation_response(&session).await
            }

            GptImageStage::Confirming => {
                let trimmed = text.trim().to_lowercase();

                if trimmed == "确认"
                    || trimmed == "是"
                    || trimmed == "y"
                    || trimmed == "yes"
                    || trimmed == "ok"
                {
                    let prompt = session.prompt.clone();
                    let model = session
                        .model
                        .clone()
                        .unwrap_or_else(|| "gpt-image-2".to_string());
                    let size = session.size.clone().unwrap_or_else(|| "auto".to_string());
                    let quality = session
                        .quality
                        .clone()
                        .unwrap_or_else(|| "auto".to_string());
                    let ref_urls = session.ref_image_urls.clone();

                    session.stage = GptImageStage::Polling(String::new());
                    session.created_at = Instant::now();
                    session_upsert(session);

                    tokio::spawn(submit_and_poll(
                        prompt,
                        model,
                        size,
                        quality,
                        ref_urls,
                        user_id,
                        resolve_quality_label,
                        resolve_model_label,
                    ));

                    Some(msg_segment_from_string(
                        "已提交，正在生成图片...".to_string(),
                    ))
                } else if trimmed == "否" || trimmed == "no" || trimmed == "n" {
                    session_remove(user_id);
                    Some(msg_segment_from_string(
                        "已取消。发送 -img <提示词> 开始新流程。".to_string(),
                    ))
                } else {
                    Some(reply("请回复 [确认] (提交) 或 [否] (取消)。"))
                }
            }

            GptImageStage::Polling(ref _task_id) => {
                // Already polling — ignore further input
                Some(msg_segment_from_string(
                    "图片正在生成中，请耐心等待...".to_string(),
                ))
            }
        }
    }
}

fn resolve_quality_label(quality: &str, _model: &str) -> String {
    match quality {
        "high" => "High (VIP)".to_string(),
        _ => "Auto (标准)".to_string(),
    }
}

fn resolve_model_label(model: &str) -> String {
    match model {
        "gpt-image-2-vip" => "GPT Image 2 Vip".to_string(),
        _ => "GPT Image 2".to_string(),
    }
}

async fn build_confirmation_response(session: &GptImageSession) -> Option<MessageSegment> {
    let size = session.size.as_deref().unwrap_or("auto");
    let quality = session.quality.as_deref().unwrap_or("auto");
    let model = session.model.as_deref().unwrap_or("gpt-image-2");
    let quality_label = resolve_quality_label(quality, model);
    let model_label = resolve_model_label(model);
    let ref_count = session.ref_image_urls.len();

    // Download reference image thumbnails
    let mut thumb_images: Vec<(Vec<u8>, &str)> = Vec::new();
    for url in &session.ref_image_urls {
        match download_image_bytes(url).await {
            Ok(bytes) => {
                let mime = detect_mime_from_bytes(&bytes);
                thumb_images.push((bytes, mime));
            }
            Err(e) => {
                log::warn!("[grsai-gpt-image] 下载参考图缩略图失败: {}", e);
            }
        }
    }

    let (thumbs_svg, thumbs_h) = build_thumbnails_from_data(&thumb_images);

    let svg = build_confirmation_svg(
        &session.prompt,
        size,
        &quality_label,
        &model_label,
        ref_count,
        &thumbs_svg,
        thumbs_h,
    );

    let png = render_svg_to_png(&svg);
    if png.is_empty() {
        return Some(msg_segment_from_string(
            "渲染确认卡片失败，请重试。".to_string(),
        ));
    }

    match crate::media_file::write_media(&png, "png") {
        Ok(file_uri) => Some(MessageSegment::Image {
            data: ImageData {
                file: file_uri,
                summary: Some("confirm.png".to_string()),
                sub_type: None,
                url: None,
                file_size: None,
            },
        }),
        Err(e) => {
            log::error!("[grsai-gpt-image] 写入确认卡片失败: {}", e);
            Some(msg_segment_from_string(
                "渲染确认卡片失败，请重试。".to_string(),
            ))
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_by_index() {
        assert_eq!(parse_size("1"), Some("auto".to_string()));
        assert_eq!(parse_size("2"), Some("1:1".to_string()));
        assert_eq!(parse_size("15"), Some("1024x1024".to_string()));
        assert_eq!(parse_size("21"), Some("2160x3840".to_string()));
    }

    #[test]
    fn parse_size_empty_defaults_auto() {
        assert_eq!(parse_size(""), Some("auto".to_string()));
    }

    #[test]
    fn parse_size_out_of_range() {
        assert_eq!(parse_size("0"), None);
        assert_eq!(parse_size("22"), None);
        assert_eq!(parse_size("999"), None);
    }

    #[test]
    fn parse_size_by_label() {
        assert_eq!(parse_size("A1"), Some("auto".to_string()));
        assert_eq!(parse_size("a1"), Some("auto".to_string()));
        assert_eq!(parse_size("B4"), Some("2048x2048".to_string()));
    }

    #[test]
    fn parse_size_free_text() {
        assert_eq!(parse_size("1024x1024"), Some("1024x1024".to_string()));
        assert_eq!(parse_size("1024*1024"), Some("1024x1024".to_string()));
        assert_eq!(parse_size("1920x1080"), Some("1920x1080".to_string()));
    }

    #[test]
    fn parse_size_invalid_free_text() {
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("0x0"), None);
        assert_eq!(parse_size("100000x100000"), None);
    }

    #[test]
    fn detect_mime_types() {
        let png = b"\x89PNG\r\n\x1a\n";
        assert_eq!(detect_mime_from_bytes(png), "image/png");

        let jpg = b"\xff\xd8\xff\xe0";
        assert_eq!(detect_mime_from_bytes(jpg), "image/jpeg");

        let gif = b"GIF89a";
        assert_eq!(detect_mime_from_bytes(gif), "image/gif");
    }

    #[test]
    fn size_card_svg_builds() {
        let (svg, _) = build_size_card_svg();
        assert!(svg.contains("GPT"));
        assert!(svg.contains("auto"));
        assert!(svg.contains("1:1"));
        assert!(svg.contains("1024x1024"));
        assert!(svg.contains("2160x3840"));
    }

    #[test]
    fn confirmation_svg_builds() {
        let svg = build_confirmation_svg(
            "a cute cat",
            "16:9",
            "Auto (标准)",
            "gpt-image-2",
            2,
            "",
            0.0,
        );
        assert!(svg.contains("a cute cat"));
        assert!(svg.contains("16:9"));
        assert!(svg.contains("Auto"));
        assert!(svg.contains("gpt-image-2"));
        assert!(svg.contains("2 张"));
    }

    #[test]
    fn render_size_card_to_png() {
        let (svg, _) = build_size_card_svg();
        let png = render_svg_to_png(&svg);
        assert!(!png.is_empty());
        // Should start with PNG magic bytes
        assert_eq!(&png[..4], b"\x89PNG");
    }
}
