use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bot_lib::structs::{ImageData, MessageSegment};
use reqwest::header::HeaderValue;
use serde_json::Value;
use std::collections::VecDeque;
use std::env;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct ImageMattingResult {
    pub context: MessageContext,
    pub segment: MessageSegment,
}

pub type ImageMattingSender = mpsc::Sender<ImageMattingResult>;
pub type MsgQueue = Arc<Mutex<VecDeque<Value>>>;

pub struct ImageMattingFeature {
    sender: ImageMattingSender,
    msg_queue: MsgQueue,
}

impl ImageMattingFeature {
    pub fn new(sender: ImageMattingSender, msg_queue: MsgQueue) -> Self {
        Self { sender, msg_queue }
    }

    pub fn feature_id() -> &'static str {
        "image_matting"
    }

    pub fn feature_name() -> &'static str {
        "图像抠图: -image-matting <图片> 去除图片背景"
    }
}

fn extract_image_url(context: &MessageContext) -> Option<String> {
    for seg in &context.message {
        if seg.get("type").and_then(|v| v.as_str()) == Some("image") {
            if let Some(url) = seg.pointer("/data/url").and_then(|v| v.as_str()) {
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

fn extract_reply_id(context: &MessageContext) -> Option<i64> {
    for seg in &context.message {
        if seg.get("type").and_then(|v| v.as_str()) == Some("reply") {
            return seg.pointer("/data/id").and_then(|v| {
                v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            });
        }
    }
    None
}

fn extract_image_url_from_msg(json: &Value) -> Option<String> {
    let messages = json.get("message")?.as_array()?;
    for seg in messages {
        if seg.get("type").and_then(|v| v.as_str()) == Some("image") {
            if let Some(url) = seg.pointer("/data/url").and_then(|v| v.as_str()) {
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

async fn download_image(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::new();
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
        .header(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("*/*"),
        )
        .send()
        .await
        .map_err(|e| format!("下载图片失败：{}", e))?;

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("读取图片数据失败：{}", e))
}

async fn call_matting_service(image_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let matting_url =
        env::var("IMAGE_MATTING_URL").unwrap_or_else(|_| "http://127.0.0.1:18213".to_string());
    let matting_auth = env::var("IMAGE_MATTING_AUTH_KEY").unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{}", e))?;

    let resp = client
        .post(&matting_url)
        .header("Authorization", format!("Bearer {}", matting_auth))
        .header("Content-Type", "application/octet-stream")
        .body(image_bytes.to_vec())
        .send()
        .await
        .map_err(|e| format!("无法连接抠图服务：{}", e))?;

    if !resp.status().is_success() {
        return Err(format!("抠图服务返回错误状态码：{}", resp.status()));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("读取抠图结果失败：{}", e))
}

async fn process_and_deliver(
    image_url: String,
    context: MessageContext,
    sender: ImageMattingSender,
) {
    let image_bytes = match download_image(&image_url).await {
        Ok(b) => b,
        Err(e) => {
            log::error!("[image_matting] {}", e);
            let _ = sender
                .send(ImageMattingResult {
                    context,
                    segment: msg_segment_from_string(e),
                })
                .await;
            return;
        }
    };

    let result_bytes = match call_matting_service(&image_bytes).await {
        Ok(b) => b,
        Err(e) => {
            log::error!("[image_matting] {}", e);
            let _ = sender
                .send(ImageMattingResult {
                    context,
                    segment: msg_segment_from_string(e),
                })
                .await;
            return;
        }
    };

    let b64 = BASE64.encode(&result_bytes);
    let _ = sender
        .send(ImageMattingResult {
            context,
            segment: MessageSegment::Image {
                data: ImageData {
                    file: format!("base64://{}", b64),
                    summary: Some("matted.png".to_string()),
                    sub_type: None,
                    url: None,
                    file_size: None,
                },
            },
        })
        .await;
}

#[async_trait]
impl Feature for ImageMattingFeature {
    fn feature_id(&self) -> &str {
        ImageMattingFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        ImageMattingFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg["type"].as_str() != Some("text") {
            return false;
        }
        let text = msg["data"]["text"].as_str().unwrap_or("").trim();
        text == "-image-matting" || text.starts_with("-image-matting ")
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let _ = msg;

        let image_url = match extract_image_url(context) {
            Some(url) => url,
            None => {
                if let Some(reply_id) = extract_reply_id(context) {
                    let queue = self.msg_queue.lock().await;
                    let found = queue.iter().find_map(|stored| {
                        let mid = stored.get("message_id").and_then(|v| v.as_i64())?;
                        if mid == reply_id {
                            extract_image_url_from_msg(stored)
                        } else {
                            None
                        }
                    });
                    match found {
                        Some(url) => url,
                        None => {
                            return Some(msg_segment_from_string(
                                "未能在历史消息中找到引用消息的图片。".to_string(),
                            ));
                        }
                    }
                } else {
                    return Some(msg_segment_from_string(
                        "用法: -image-matting <图片>\n请在消息中附带一张图片，或引用一条包含图片的消息。"
                            .to_string(),
                    ));
                }
            }
        };

        let ctx = context.clone();
        let sender = self.sender.clone();

        tokio::spawn(process_and_deliver(image_url, ctx, sender));

        Some(msg_segment_from_string(
            "正在处理图片抠图，请稍候...".to_string(),
        ))
    }
}
