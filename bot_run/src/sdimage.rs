use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bot_lib::structs::{ImageData, MessageSegment};

use serde_json::Value;
use std::env;
use tokio::sync::mpsc;

pub struct SdImageResult {
    pub context: MessageContext,
    pub segment: MessageSegment,
}

pub type SdImageSender = mpsc::Sender<SdImageResult>;

pub fn resolve_model(alias: &str) -> &'static str {
    match alias.to_lowercase().as_str() {
        "hana" => "hana4CHROME_huge.safetensors",
        "hunyuan" => "hunyuan3d-dit-v2.safetensors",
        "nova" => "novaAnimeXL_ilV170.safetensors",
        _ => "sd_xl_base_1.0.safetensors",
    }
}

#[derive(Debug)]
pub struct SdParams {
    pub prompt: String,
    pub model: String,
    pub negative: String,
    pub sampler: String,
    pub cfg: f64,
    pub steps: u32,
}

fn ts_trim(s: &str) -> &str {
    let s = s.trim();
    let mut start = 0;
    let mut end = s.len();
    let bytes = s.as_bytes();
    while end > start + 1 && bytes[start] == b'"' && bytes[end - 1] == b'"' {
        start += 1;
        end -= 1;
    }
    while end > start + 1 && bytes[start] == b'\'' && bytes[end - 1] == b'\'' {
        start += 1;
        end -= 1;
    }
    &s[start..end]
}

impl SdParams {
    pub fn parse(raw: &str) -> Option<Self> {
        let body = if let Some(s) = raw.strip_prefix("-sd ") {
            ts_trim(s)
        } else if let Some(s) = raw.strip_prefix("sd ") {
            ts_trim(s)
        } else {
            return None;
        };

        if body.is_empty() {
            return None;
        }

        let mut prompt_parts: Vec<&str> = Vec::new();
        let mut model = "xl".to_string();
        let mut negative = String::new();
        let mut sampler = "euler_ancestral".to_string();
        let mut cfg: f64 = 7.0;
        let mut steps: u32 = 20;

        for part in body.split('|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim().to_lowercase();
                let value = ts_trim(value);
                match key.as_str() {
                    "model" => model = value.to_string(),
                    "negative" => negative = value.to_string(),
                    "sampler" => sampler = value.to_string(),
                    "cfg" => {
                        if let Ok(v) = value.parse::<f64>() {
                            cfg = v;
                        }
                    }
                    "steps" => {
                        if let Ok(v) = value.parse::<u32>() {
                            steps = v;
                        }
                    }
                    _ => {}
                }
            } else {
                let trimmed = ts_trim(part);
                if !trimmed.is_empty() {
                    prompt_parts.push(trimmed);
                }
            }
        }

        let prompt = prompt_parts.join(" ");
        if prompt.is_empty() {
            return None;
        }

        Some(SdParams {
            prompt,
            model,
            negative,
            sampler,
            cfg,
            steps,
        })
    }
}

pub fn build_workflow(params: &SdParams) -> Value {
    let seed: u64 = rand::random();
    let ckpt_name = resolve_model(&params.model);
    let negative_text = if params.negative.is_empty() {
        "worst quality, bad quality, low quality, lowres, anatomical nonsense, artistic error, bad anatomy, blood, censored, monochrome".to_string()
    } else {
        params.negative.clone()
    };

    serde_json::json!({
        "3": {
            "class_type": "KSampler",
            "inputs": {
                "seed": seed,
                "steps": params.steps,
                "cfg": params.cfg,
                "sampler_name": params.sampler,
                "scheduler": "normal",
                "denoise": 1.0,
                "model": ["4", 0],
                "positive": ["6", 0],
                "negative": ["7", 0],
                "latent_image": ["5", 0]
            }
        },
        "4": {
            "class_type": "CheckpointLoaderSimple",
            "inputs": { "ckpt_name": ckpt_name }
        },
        "5": {
            "class_type": "EmptyLatentImage",
            "inputs": { "width": 1024, "height": 1024, "batch_size": 1 }
        },
        "6": {
            "class_type": "CLIPTextEncode",
            "inputs": { "text": params.prompt, "clip": ["4", 1] }
        },
        "7": {
            "class_type": "CLIPTextEncode",
            "inputs": { "text": negative_text, "clip": ["4", 1] }
        },
        "8": {
            "class_type": "VAEDecode",
            "inputs": { "samples": ["3", 0], "vae": ["4", 2] }
        },
        "9": {
            "class_type": "SaveImage",
            "inputs": { "filename_prefix": "sd_bot", "images": ["8", 0] }
        }
    })
}

async fn poll_and_deliver(
    comfy_url: String,
    prompt_id: String,
    context: MessageContext,
    sender: SdImageSender,
) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("[sdimage] Failed to build HTTP client: {}", e);
            return;
        }
    };

    let history_url = format!("{}/history/{}", comfy_url, prompt_id);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(600_000);

    loop {
        if std::time::Instant::now() >= deadline {
            log::warn!("[sdimage] Timeout waiting for prompt_id={}", prompt_id);
            let _ = sender
                .send(SdImageResult {
                    context,
                    segment: msg_segment_from_string(format!(
                        "图片生成超时 (提示ID: {})，请稍后重试。",
                        prompt_id
                    )),
                })
                .await;
            return;
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let body: Value = match client.get(&history_url).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("[sdimage] History JSON parse error: {}", e);
                    continue;
                }
            },
            Err(e) => {
                log::warn!("[sdimage] History poll error: {}", e);
                continue;
            }
        };

        let job = match body.get(&prompt_id) {
            Some(j) => j.clone(),
            None => continue,
        };

        let filename = job
            .pointer("/outputs/9/images/0/filename")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let subfolder = job
            .pointer("/outputs/9/images/0/subfolder")
            .and_then(|v| v.as_str())
            .unwrap_or("output")
            .to_string();

        let filename = match filename {
            None => {
                log::warn!(
                    "[sdimage] Job done but no filename for prompt_id={}",
                    prompt_id
                );
                let _ = sender
                    .send(SdImageResult {
                        context,
                        segment: msg_segment_from_string(format!(
                            "图片生成完成但未找到输出文件 (提示ID: {})。",
                            prompt_id
                        )),
                    })
                    .await;
                return;
            }
            Some(f) => f,
        };

        let view_url = format!(
            "{}/view?filename={}&subfolder={}&type=output",
            comfy_url,
            percent_encode(&filename),
            percent_encode(&subfolder)
        );

        let img_bytes = match client.get(&view_url).send().await {
            Ok(r) => match r.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    log::error!("[sdimage] Failed to read image bytes: {}", e);
                    let _ = sender
                        .send(SdImageResult {
                            context,
                            segment: msg_segment_from_string(format!(
                                "图片下载失败 (提示ID: {})：{}",
                                prompt_id, e
                            )),
                        })
                        .await;
                    return;
                }
            },
            Err(e) => {
                log::error!("[sdimage] Failed to fetch image: {}", e);
                let _ = sender
                    .send(SdImageResult {
                        context,
                        segment: msg_segment_from_string(format!(
                            "图片下载失败 (提示ID: {})：{}",
                            prompt_id, e
                        )),
                    })
                    .await;
                return;
            }
        };

        let b64 = BASE64.encode(&img_bytes);
        let _ = sender
            .send(SdImageResult {
                context,
                segment: MessageSegment::Image {
                    data: ImageData {
                        file: format!("base64://{}", b64),
                        summary: Some(filename),
                        sub_type: None,
                        url: None,
                        file_size: None,
                    },
                },
            })
            .await;
        return;
    }
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => {
                out.push('%');
                out.push(
                    char::from_digit((other >> 4) as u32, 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit((other & 0xF) as u32, 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
}

pub struct SdImageFeature {
    sender: SdImageSender,
}

impl SdImageFeature {
    pub fn new(sender: SdImageSender) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl Feature for SdImageFeature {
    fn feature_name(&self) -> &str {
        "sd 图片生成: sd <prompt> 或 -sd <prompt> 生成图片"
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg["type"].as_str() != Some("text") {
            return false;
        }
        let text = msg["data"]["text"].as_str().unwrap_or("").trim();
        text.starts_with("sd ") || text.starts_with("-sd ")
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let ctx = context.clone();
        let sender = self.sender.clone();
        let text = msg["data"]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        let params = match SdParams::parse(&text) {
            Some(p) => p,
            None => {
                return Some(msg_segment_from_string(
                    "用法: sd <prompt> [|model=xl] [|negative=...] [|sampler=euler_ancestral] [|cfg=7] [|steps=20]".to_string(),
                ));
            }
        };

        let comfy_url =
            env::var("COMFY_UI_URL").unwrap_or_else(|_| "http://127.0.0.1:8188".to_string());

        let resp = match reqwest::Client::new()
            .post(format!("{}/prompt", comfy_url))
            .json(&build_workflow(&params))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return Some(msg_segment_from_string(format!(
                    "无法连接到 ComfyUI：{}",
                    e
                )));
            }
        };

        let resp_json: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                return Some(msg_segment_from_string(format!(
                    "ComfyUI 返回了无效响应：{}",
                    e
                )));
            }
        };

        let prompt_id = match resp_json.get("prompt_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                return Some(msg_segment_from_string(format!(
                    "ComfyUI 未返回 prompt_id，响应：{}",
                    resp_json
                )));
            }
        };

        tokio::spawn(poll_and_deliver(comfy_url, prompt_id.clone(), ctx, sender));

        Some(msg_segment_from_string(format!(
            "已收到请求，正在生成图片... (提示ID: {})",
            prompt_id
        )))
    }
}
