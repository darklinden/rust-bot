use crate::feature::{Feature, MessageContext};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bot_lib::{structs::MessageSegment, Segment};
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use serde_json::Value;
use std::env;
use tokio::sync::mpsc;

#[derive(Serialize)]
struct TtsRequest {
    text: String,
}

pub struct TtsResult {
    pub context: MessageContext,
    pub segment: MessageSegment,
}

pub type TtsSender = mpsc::Sender<TtsResult>;

pub struct TtsFeature {
    sender: TtsSender,
}

impl TtsFeature {
    pub fn new(sender: TtsSender) -> Self {
        Self { sender }
    }

    pub fn tts_base_url_from_env(value: Option<String>) -> String {
        value
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:8000".to_string())
    }

    fn get_tts_base_url() -> String {
        Self::tts_base_url_from_env(env::var("TTS_URL").ok())
    }

    pub fn build_tts_record_segment(audio_bytes: &[u8]) -> MessageSegment {
        Segment::record(format!("base64://{}", BASE64.encode(audio_bytes)))
    }

    async fn synthesize_tts(text: &str) -> Result<Vec<u8>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("创建 TTS 客户端失败：{}", e))?;

        let url = format!("{}/synthesize", Self::get_tts_base_url());
        let response = client
            .post(&url)
            .json(&TtsRequest {
                text: text.to_string(),
            })
            .send()
            .await
            .map_err(|e| format!("调用 TTS 服务失败：{}", e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("TTS 服务返回错误状态码：{} {}", status, body));
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !content_type.starts_with("audio/wav") {
            return Err(format!(
                "TTS 服务未返回 audio/wav，实际为：{}",
                content_type
            ));
        }

        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|e| format!("读取 TTS 音频失败：{}", e))
    }

    async fn synthesize_and_deliver(text: String, context: MessageContext, sender: TtsSender) {
        if text.trim().is_empty() {
            return;
        }

        let audio_bytes = match Self::synthesize_tts(&text).await {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("[tts] {}", e);
                return;
            }
        };

        let segment = Self::build_tts_record_segment(&audio_bytes);
        if let Err(e) = sender.send(TtsResult { context, segment }).await {
            log::error!("[tts] 发送 TTS 结果失败: {}", e);
        }
    }

    pub fn feature_id() -> &'static str {
        "tts"
    }

    pub fn feature_name() -> &'static str {
        "语音合成: -tts <<内容>> 将文字转换为语音消息"
    }
}

#[async_trait]
impl Feature for TtsFeature {
    fn feature_id(&self) -> &str {
        TtsFeature::feature_id()
    }
    fn feature_name(&self) -> &str {
        TtsFeature::feature_name()
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

        text.starts_with("-tts ")
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        _msg: &Value,
    ) -> Option<MessageSegment> {
        let mut msg = _msg["data"]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        if !msg.to_lowercase().starts_with("-tts ") {
            return None;
        }
        msg = msg[5..].trim().to_string();
        log::info!("[tts] synthesizing: {}", msg);

        let sender = self.sender.clone();
        let context = context.clone();
        tokio::spawn(async move {
            TtsFeature::synthesize_and_deliver(msg, context, sender).await;
        });

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::Feature;
    use serde_json::json;

    #[test]
    fn tts_base_url_defaults_when_missing() {
        assert_eq!(
            TtsFeature::tts_base_url_from_env(None),
            "http://127.0.0.1:8000"
        );
    }

    #[test]
    fn tts_base_url_trims_whitespace_and_trailing_slash() {
        assert_eq!(
            TtsFeature::tts_base_url_from_env(Some(" http://127.0.0.1:9000/ ".to_string())),
            "http://127.0.0.1:9000"
        );
    }

    #[test]
    fn build_tts_record_segment_wraps_base64_audio() {
        let segment = TtsFeature::build_tts_record_segment(b"hi");
        match segment {
            MessageSegment::Record { data } => {
                assert_eq!(data.file, "base64://aGk=");
                assert_eq!(data.file_size, None);
            }
            _ => panic!("expected Record segment"),
        }
    }

    #[test]
    fn check_command_accepts_tts_prefix() {
        let (sender, _receiver) = mpsc::channel(1);
        let feature = TtsFeature::new(sender);

        let tts_msg = json!({
            "type": "text",
            "data": { "text": "-tts hello" }
        });
        assert!(feature.check_command(&tts_msg));

        let loli_msg = json!({
            "type": "text",
            "data": { "text": "-loli hello" }
        });
        assert!(!feature.check_command(&loli_msg));
    }
}
