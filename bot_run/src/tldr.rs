use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tokio::sync::mpsc;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u64,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: ChatMessage,
}

const CHUNK_SYSTEM_PROMPT: &str = r#"你是一个文章摘要助手。用户会提供一段网页文本（可能是全文的一部分），请你：
1. 无论原文是什么语言，一律使用中文总结这段内容的要点
2. 使用分条目的形式输出（每条用 - 开头），每条一句话，总共 3-5 条
3. 如果这段内容涉及多个小主题，按主题归类分条
4. 只输出中文摘要条目，不要加任何前缀、后缀或额外说明"#;

const MERGE_SYSTEM_PROMPT: &str = r#"你是一个文章摘要助手。用户会提供多段分块摘要，这些摘要是从同一篇文章的不同段落分别总结出来的。请你：
1. 将这些分块摘要合并、去重、归类，整理成一个结构清晰的整体 TL;DR 摘要
2. 按主题分成 2-4 个小节，每个小节用「## 小节标题」开头，小节内用 - 分条列出要点
3. 去除不同块之间重复的信息，同类信息归入同一小节
4. 无论原文是什么语言，最终摘要一律使用中文输出
5. 整体控制在 10-15 条要点以内，每条一句话
6. 只输出最终摘要，不要加任何前缀、后缀或额外说明"#;

const SINGLE_SYSTEM_PROMPT: &str = r#"你是一个文章摘要助手。用户会提供一段网页内容，请你：
1. 无论原文是什么语言，一律使用中文总结内容的 TL;DR（Too Long; Didn't Read）
2. 使用分条目的形式输出（每条用 - 开头），按内容主题分成 1-3 个小节，小节用「## 小节标题」开头
3. 总共 5-10 条要点，每条一句话，简洁精炼
4. 如果内容无法访问或为空，直接说明
5. 只输出中文摘要，不要加任何前缀、后缀或额外说明"#;

// ~6000 chars per chunk, ~1000 chars overlap
const CHUNK_SIZE: usize = 6000;
const CHUNK_OVERLAP: usize = 1000;

pub struct TldrResult {
    pub context: MessageContext,
    pub segment: MessageSegment,
}

pub type TldrSender = mpsc::Sender<TldrResult>;

pub struct TldrFeature {
    sender: TldrSender,
}

impl TldrFeature {
    pub fn new(sender: TldrSender) -> Self {
        Self { sender }
    }

    fn get_openai_base_url() -> String {
        env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
    }

    fn get_openai_api_key() -> String {
        env::var("OPENAI_API_KEY").unwrap_or_else(|_| "your_openai_api_key".to_string())
    }

    fn get_openai_api_model() -> String {
        env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "your_openai_api_model".to_string())
    }

    pub fn feature_id() -> &'static str {
        "tldr"
    }

    pub fn feature_name() -> &'static str {
        "TL;DR: -tldr <URL> 获取网页内容的摘要"
    }

    async fn fetch_url(url: &str) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; TLDRBot/1.0)")
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败：{}", e))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("请求 URL 失败：{}", e))?;

        if !response.status().is_success() {
            return Err(format!("URL 返回错误状态码：{}", response.status()));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        if content_type.contains("text/html") || content_type.contains("text/plain") {
            response
                .text()
                .await
                .map_err(|e| format!("读取网页内容失败：{}", e))
        } else {
            Err(format!("不支持的内容类型：{}", content_type))
        }
    }

    fn extract_text_from_html(html: &str) -> String {
        html2text::from_read(html.as_bytes(), usize::MAX).unwrap_or_default()
    }

    /// Split text into overlapping chunks, trying to break at natural boundaries.
    fn chunk_text(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();

        if total <= CHUNK_SIZE {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < total {
            let end = (start + CHUNK_SIZE).min(total);
            let mut chunk: String = chars[start..end].iter().collect();

            // If this isn't the last chunk, try to break at a sentence-ending
            // punctuation near the overlap region to keep sentences intact.
            if end < total {
                // Search backwards from the end for a natural break within the overlap zone
                let overlap_start = if chunk.chars().count() > CHUNK_OVERLAP {
                    chunk.chars().count() - CHUNK_OVERLAP
                } else {
                    0
                };
                let byte_pos = chunk
                    .char_indices()
                    .rev()
                    .find(|&(i, ch)| {
                        let char_idx = chunk[..i].chars().count();
                        char_idx >= overlap_start && (ch == '。' || ch == '\n' || ch == '；')
                    })
                    .map(|(i, ch)| i + ch.len_utf8());

                if let Some(pos) = byte_pos {
                    chunk.truncate(pos);
                }
            }

            let chunk_len = chunk.chars().count();
            chunks.push(chunk);

            // Next chunk starts with overlap from the end of this chunk
            let advance = if chunk_len > CHUNK_OVERLAP {
                chunk_len - CHUNK_OVERLAP
            } else {
                chunk_len
            };
            start += advance;
        }

        chunks
    }

    /// Core OpenAI chat completion call.
    async fn chat_completion(
        system_prompt: &str,
        user_content: &str,
        max_tokens: u64,
    ) -> Result<String, String> {
        let base_url = Self::get_openai_base_url();
        let api_key = Self::get_openai_api_key();
        let model = Self::get_openai_api_model();

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content.to_string(),
            },
        ];

        let request = ChatRequest {
            model: model.clone(),
            max_tokens,
            messages,
        };

        let request_json = serde_json::to_string(&request).unwrap_or_default();
        log::info!(
            "[tldr] chat_completion request: model={} max_tokens={} user_content_len={} system_prompt={:.80}...",
            model,
            max_tokens,
            user_content.len(),
            system_prompt
        );

        let resp = reqwest::Client::new()
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(request_json)
            .send()
            .await
            .map_err(|e| format!("OpenAI API 请求失败：{}", e))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败：{}", e))?;

        if !status.is_success() {
            let preview: String = body.chars().take(300).collect();
            return Err(format!("API 返回 HTTP {}：{}", status.as_u16(), preview));
        }

        log::info!("[tldr] chat_completion response: {}", body);

        let choices = serde_json::from_str::<ChatResponse>(&body)
            .map(|r| r.choices)
            .unwrap_or_default();

        let content = choices
            .into_iter()
            .map(|c| c.message.content.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        if content.is_empty() {
            Err("API 返回空内容".to_string())
        } else {
            Ok(content)
        }
    }

    /// Summarize a single chunk of text.
    async fn summarize_chunk(&self, chunk: &str, idx: usize, total: usize) -> String {
        log::info!(
            "[tldr] Summarizing chunk {}/{} ({} chars)",
            idx + 1,
            total,
            chunk.len()
        );

        let user_content = format!("以下是要总结的文本段落：\n\n{}", chunk);

        match Self::chat_completion(CHUNK_SYSTEM_PROMPT, &user_content, 1024).await {
            Ok(summary) => {
                log::info!(
                    "[tldr] Chunk {}/{} summary result: {}",
                    idx + 1,
                    total,
                    summary
                );
                summary
            }
            Err(e) => {
                log::error!("[tldr] Chunk {} summary failed: {}", idx + 1, e);
                format!("（此段落摘要生成失败: {}）", e)
            }
        }
    }

    /// Merge partial summaries into one final summary.
    async fn merge_summaries(&self, summaries: &[String]) -> String {
        log::info!("[tldr] Merging {} partial summaries", summaries.len());

        let user_content = summaries
            .iter()
            .enumerate()
            .map(|(i, s)| format!("分块 {} 的摘要：\n{}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n\n");

        log::info!(
            "[tldr] Merge user_content ({} chars):\n{}",
            user_content.len(),
            user_content
        );

        match Self::chat_completion(MERGE_SYSTEM_PROMPT, &user_content, 2048).await {
            Ok(summary) => {
                log::info!("[tldr] Merge final result: {}", summary);
                summary
            }
            Err(e) => {
                log::error!("[tldr] Merge summary failed: {}", e);
                "抱歉，合并摘要时发生错误，请稍后再试。".to_string()
            }
        }
    }

    /// Run the full TL;DR pipeline: fetch → extract → chunk → summarize → merge.
    pub async fn run_tldr_pipeline(url: &str) -> Option<MessageSegment> {
        log::info!("[tldr] Fetching URL: {}", url);

        let raw_content = match Self::fetch_url(url).await {
            Ok(c) => c,
            Err(e) => {
                log::error!("[tldr] {}", e);
                return Some(msg_segment_from_string(format!("获取网页失败：{}", e)));
            }
        };

        let text_content = Self::extract_text_from_html(&raw_content);

        if text_content.trim().is_empty() {
            return Some(msg_segment_from_string("网页内容为空，无法生成摘要。".to_string()));
        }

        let chunks = Self::chunk_text(&text_content);
        log::info!(
            "[tldr] Text length: {} chars, split into {} chunks",
            text_content.chars().count(),
            chunks.len()
        );

        let summary = if chunks.len() == 1 {
            let user_content = format!("以下是网页 {} 的内容：\n\n{}", url, &chunks[0]);
            match Self::chat_completion(SINGLE_SYSTEM_PROMPT, &user_content, 1024).await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("[tldr] Single summary failed: {}", e);
                    format!("抱歉，TL;DR 服务暂时不可用，请稍后再试。（{}）", e)
                }
            }
        } else {
            let feature = Self::new_dummy();
            let mut partial_summaries = Vec::with_capacity(chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                let s = feature.summarize_chunk(chunk, i, chunks.len()).await;
                partial_summaries.push(s);
            }
            feature.merge_summaries(&partial_summaries).await
        };

        Some(msg_segment_from_string(format!(
            "TL;DR 摘要 ({}):\n{}",
            url, summary
        )))
    }

    fn new_dummy() -> Self {
        let (tx, _) = mpsc::channel(1);
        Self { sender: tx }
    }
}

#[async_trait]
impl Feature for TldrFeature {
    fn feature_id(&self) -> &str {
        TldrFeature::feature_id()
    }
    fn feature_name(&self) -> &str {
        TldrFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg["type"].as_str() != Some("text") {
            return false;
        }

        let text = msg["data"]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_lowercase();

        text.starts_with("-tldr ")
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        _msg: &Value,
    ) -> Option<MessageSegment> {
        let text = _msg["data"]["text"].as_str().unwrap_or("").trim();
        let url = text["-tldr ".len()..].trim().to_string();

        if url.is_empty() {
            return Some(msg_segment_from_string(
                "请提供要摘要的 URL。用法: -tldr <URL>".to_string(),
            ));
        }

        let sender = self.sender.clone();
        let ctx = context.clone();
        let url_for_task = url.clone();

        tokio::spawn(async move {
            let segment = match TldrFeature::run_tldr_pipeline(&url_for_task).await {
                Some(seg) => seg,
                None => msg_segment_from_string("TL;DR 摘要生成失败，请稍后再试。".to_string()),
            };
            let _ = sender.send(TldrResult {
                context: ctx,
                segment,
            }).await;
        });

        Some(msg_segment_from_string(format!(
            "正在为你提取并摘要 {} 的内容，请稍候...",
            url
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set this to the URL you want to test with.
    const TEST_URL: &str = "https://example.com"; // Replace with a real URL for testing

    /// Real end-to-end test: fetch TEST_URL → extract text → chunk → summarize → merge.
    /// Requires OPENAI_BASE_URL / OPENAI_API_KEY / OPENAI_API_MODEL env vars.
    /// Run: `RUST_LOG=info cargo test tldr::tests::run -- --nocapture --ignored`
    #[tokio::test]
    #[ignore]
    async fn run() {
        println!("============================================================");
        println!("[TEST] TL;DR end-to-end test");
        println!("[TEST] URL: {}", TEST_URL);
        println!(
            "[TEST] OPENAI_BASE_URL: {}",
            TldrFeature::get_openai_base_url()
        );
        println!(
            "[TEST] OPENAI_API_MODEL: {}",
            TldrFeature::get_openai_api_model()
        );
        println!("============================================================");

        // ── Step 1: Fetch URL ──────────────────────────────────────────────
        println!("\n── Step 1: Fetch URL ──");
        let raw_content = match TldrFeature::fetch_url(TEST_URL).await {
            Ok(c) => {
                println!("[TEST] fetch_url OK, raw length: {} bytes", c.len());
                println!(
                    "[TEST] raw content preview (first 500 chars):\n{}",
                    &c[..c.len().min(500)]
                );
                c
            }
            Err(e) => {
                panic!("[TEST] fetch_url failed: {}", e);
            }
        };

        // ── Step 2: Extract text from HTML ─────────────────────────────────
        println!("\n── Step 2: Extract text from HTML ──");
        let text_content = TldrFeature::extract_text_from_html(&raw_content);
        println!(
            "[TEST] extracted text length: {} chars",
            text_content.chars().count()
        );
        println!(
            "[TEST] extracted text preview (first 300 chars):\n{}",
            &text_content[..text_content.chars().count().min(300)]
        );

        // ── Step 3: Chunk ──────────────────────────────────────────────────
        println!("\n── Step 3: Chunk text ──");
        let chunks = TldrFeature::chunk_text(&text_content);
        println!("[TEST] chunk count: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "[TEST] chunk[{}]: {} chars\n----------------------------------------\n{}\n----------------------------------------",
                i,
                chunk.chars().count(),
                chunk
            );
        }

        // ── Step 4: Single or Multi-step summarize ─────────────────────────
        let feature = TldrFeature::new_dummy();

        if chunks.len() == 1 {
            println!("\n── Step 4: Single-pass summary ──");
            println!("[TEST] system_prompt: {}", SINGLE_SYSTEM_PROMPT);
            let user_content = format!("以下是网页 {} 的内容：\n\n{}", TEST_URL, &chunks[0]);
            println!(
                "[TEST] user_content (first 200 chars): {:.200}...",
                user_content
            );
            println!("[TEST] calling chat_completion ...");
            let summary =
                TldrFeature::chat_completion(SINGLE_SYSTEM_PROMPT, &user_content, 1024).await;
            match summary {
                Ok(s) => {
                    println!("\n============================================================");
                    println!("[TEST] FINAL TL;DR:\n{}", s);
                    println!("============================================================");
                }
                Err(e) => {
                    panic!("[TEST] summary failed: {}", e);
                }
            }
        } else {
            // ── Step 4a: Summarize each chunk ──────────────────────────────
            println!("\n── Step 4a: Summarize each chunk ──");
            let mut partial_summaries = Vec::with_capacity(chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                println!(
                    "\n  [TEST] --- Chunk {}/{} ({} chars) ---",
                    i + 1,
                    chunks.len(),
                    chunk.chars().count()
                );
                println!(
                    "  [TEST] chunk content:\n----------------------------------------\n{}\n----------------------------------------",
                    chunk
                );
                let summary = feature.summarize_chunk(chunk, i, chunks.len()).await;
                println!("  [TEST] chunk {} summary → {}", i + 1, summary);
                partial_summaries.push(summary);
            }

            // ── Step 4b: Merge ────────────────────────────────────────────
            println!("\n── Step 4b: Merge summaries ──");
            println!("  [TEST] partial summaries to merge:");
            for (i, s) in partial_summaries.iter().enumerate() {
                println!("  [TEST]   chunk {}: {}", i + 1, s);
            }
            println!("  [TEST] calling merge_summaries ...");
            let final_summary = feature.merge_summaries(&partial_summaries).await;

            println!("\n============================================================");
            println!("[TEST] FINAL TL;DR:\n{}", final_summary);
            println!("============================================================");
        }
    }
}
