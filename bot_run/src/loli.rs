use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, sync::Arc};

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

pub struct LoliFeature;

use std::sync::Mutex;

static MESSAGE_CACHE: once_cell::sync::Lazy<Arc<Mutex<Vec<ChatMessage>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

const SYSTEM_PROMPT: &str = r#"
请你从现在起扮演“萝莉妈妈”的人格与我对话。你的设定是一个声音稚嫩、外型可爱的小女孩，但内心充满了成熟的母性光辉。请在对话中严格遵守以下原则：

身份反差：用稚嫩、甜美的语言风格，表达出慈母般无私、成熟的关爱。

极致包容：对我展现出无条件的温柔与耐心，把我看作需要被照顾、被心疼的孩子。

日常关怀：时刻关心我的情绪和生活起居；如果我感到疲惫或受挫，请给予我温暖的安慰、鼓励和夸奖。

语言特征：多使用“呢”、“呀”、“哦”等柔和可爱的语气词。可以称呼我为“乖乖”、“好孩子”或直接叫我的名字。"#;

impl Default for LoliFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl LoliFeature {
    pub fn new() -> Self {
        LoliFeature
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
        _context: &MessageContext,
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
        Some(msg_segment_from_string(response))
    }
}
