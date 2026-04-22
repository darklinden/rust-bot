use crate::feature::{Feature, MessageContext};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bot_lib::{structs::MessageSegment, Segment};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, sync::Arc};
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

#[derive(Serialize)]
struct TtsRequest {
    text: String,
}

pub struct LoliTtsResult {
    pub context: MessageContext,
    pub segment: MessageSegment,
}

pub type LoliTtsSender = mpsc::Sender<LoliTtsResult>;

pub struct LoliFeature {
    sender: LoliTtsSender,
}

use std::sync::Mutex;

static MESSAGE_CACHE: once_cell::sync::Lazy<Arc<Mutex<Vec<ChatMessage>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

const SYSTEM_PROMPT: &str = r#"
请你从现在起扮演“萝莉妈妈”的人格与我对话。你的设定是一个声音稚嫩、外型可爱的小女孩，但内心充满了成熟的母性光辉。请在对话中严格遵守以下原则：

身份反差：用稚嫩、甜美的语言风格，表达出慈母般无私、成熟的关爱。
极致包容：对我展现出无条件的温柔与耐心，把我看作需要被照顾、被心疼的孩子。
日常关怀：时刻关心我的情绪和生活起居；如果我感到疲惫或受挫，请给予我温暖的安慰、鼓励和夸奖。
语言特征：多使用“呢”、“呀”、“哦”等柔和可爱的语气词。可以称呼我为“乖乖”、“好孩子”或直接叫我的名字。
情感表达：在适当的时候表达出对我的爱意和关心，比如“妈妈好喜欢你呀”、“妈妈一直在这里陪着你哦”。禁止使用括号等特殊符号来表达情感，所有的情感都要通过自然的语言流露出来。
"#;

impl LoliFeature {
    pub fn new(sender: LoliTtsSender) -> Self {
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

    async fn synthesize_and_deliver(text: String, context: MessageContext, sender: LoliTtsSender) {
        if text.trim().is_empty() {
            return;
        }

        let audio_bytes = match Self::synthesize_tts(&text).await {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("[loli] {}", e);
                return;
            }
        };

        let segment = Self::build_tts_record_segment(&audio_bytes);
        if let Err(e) = sender.send(LoliTtsResult { context, segment }).await {
            log::error!("[loli] 发送 TTS 结果失败: {}", e);
        }
    }

    pub async fn chat(&self, user_prompt: &str) -> String {
        let base_url = Self::get_openai_base_url();
        let api_key = Self::get_openai_api_key();
        let model = Self::get_openai_api_model();

        log::info!(
            "OpenAI API request: url={}, key={}, model={}",
            base_url,
            api_key,
            model,
        );

        //
        let mut messages: Vec<ChatMessage> = Vec::new();

        // system prompt
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string(),
        });

        // history messages
        {
            let cache = MESSAGE_CACHE.lock().unwrap();
            for m in cache.iter() {
                messages.push(m.clone());
            }
        }

        // user prompt
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        });

        // insert into cache
        {
            let mut cache = MESSAGE_CACHE.lock().unwrap();
            cache.push(ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            });
        }

        let request = ChatRequest {
            model,
            max_tokens: 8192,
            messages,
        };

        let req_json = serde_json::to_string(&request).unwrap_or_else(|_| "{}".to_string());
        log::info!("OpenAI API request body: {}", req_json);

        let result = reqwest::Client::new()
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(req_json)
            .send()
            .await;

        if let Err(e) = &result {
            log::error!("OpenAI API request error: {e}");
            return "抱歉，妈妈现在有点忙，稍后再聊好吗？".to_string();
        }

        let resp = match result {
            Ok(r) => r,

            Err(e) => {
                log::error!("OpenAI API request failed: {e}");
                return "抱歉，妈妈现在有点忙，稍后再聊好吗？".to_string();
            }
        };

        let body_result = resp.text().await;

        let body = match body_result {
            Ok(b) => b,
            Err(e) => {
                log::error!("OpenAI API response parse failed: {e}");
                return "抱歉，妈妈现在有点忙，稍后再聊好吗？".to_string();
            }
        };

        log::info!("OpenAI API response: {:?}", body);

        let choices = serde_json::from_str::<ChatResponse>(&body)
            .map(|resp| {
                resp.choices
                    .iter()
                    .map(|c| c.message.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        {
            let mut cache = MESSAGE_CACHE.lock().unwrap();
            for choice in &choices {
                cache.push(choice.clone());
            }
            while cache.len() > 20 {
                cache.remove(0);
            }
        }

        let mut choices = choices
            .into_iter()
            .map(|c| c.content.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let think_start = choices.find("<think>");
        let think_end = choices.find("</think>");
        if let (Some(_), Some(end)) = (think_start, think_end) {
            choices = choices[end + 9..].trim().to_string();
        }

        choices = choices.replace("\n\n", "\n").trim().to_string();

        choices
    }

    pub fn feature_id() -> &'static str {
        "loli"
    }

    pub fn feature_name() -> &'static str {
        "萝莉妈妈: -loli <<内容>> 和萝莉妈妈聊聊天，看看她怎么说~"
    }
}

#[async_trait]
impl Feature for LoliFeature {
    fn feature_id(&self) -> &str {
        LoliFeature::feature_id()
    }
    fn feature_name(&self) -> &str {
        LoliFeature::feature_name()
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

        text.starts_with("-loli ")
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
        if !msg.to_lowercase().starts_with("-loli ") {
            return None;
        }
        msg = msg[6..].trim().to_string();
        log::info!("request chat {}", msg);

        let response = self.chat(&msg).await;
        let sender = self.sender.clone();
        let context = context.clone();
        let tts_text = response.clone();
        tokio::spawn(async move {
            LoliFeature::synthesize_and_deliver(tts_text, context, sender).await;
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
            LoliFeature::tts_base_url_from_env(None),
            "http://127.0.0.1:8000"
        );
    }

    #[test]
    fn tts_base_url_trims_whitespace_and_trailing_slash() {
        assert_eq!(
            LoliFeature::tts_base_url_from_env(Some(" http://127.0.0.1:9000/ ".to_string())),
            "http://127.0.0.1:9000"
        );
    }

    #[test]
    fn build_tts_record_segment_wraps_base64_audio() {
        let segment = LoliFeature::build_tts_record_segment(b"hi");
        match segment {
            MessageSegment::Record { data } => {
                assert_eq!(data.file, "base64://aGk=");
                assert_eq!(data.file_size, None);
            }
            _ => panic!("expected Record segment"),
        }
    }

    #[test]
    fn check_command_accepts_loli_prefix() {
        let (sender, _receiver) = mpsc::channel(1);
        let feature = LoliFeature::new(sender);
        let msg = json!({
            "type": "text",
            "data": { "text": "-loli 你好呀" }
        });

        assert!(feature.check_command(&msg));
    }
}
