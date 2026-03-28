use std::env;

use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde_json::Value;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MessageContext {
    pub self_id: i64,
    pub user_id: i64,
    pub group_id: Option<i64>,
    pub message_id: i64,
    pub message: Vec<Value>,
    pub raw_message: String,
    pub nickname: String,
    pub card: String,
}

impl MessageContext {
    pub fn from_json(json: &Value) -> Self {
        let user_id = json.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let group_id = json.get("group_id").and_then(|v| v.as_i64());
        let sender = json.get("sender");
        let nickname = sender
            .and_then(|s| s.get("nickname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let card = sender
            .and_then(|s| s.get("card"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Self {
            self_id: json.get("self_id").and_then(|v| v.as_i64()).unwrap_or(0),
            user_id,
            group_id,
            message_id: json.get("message_id").and_then(|v| v.as_i64()).unwrap_or(0),
            message: json
                .get("message")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            raw_message: json
                .get("raw_message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            nickname,
            card,
        }
    }

    pub fn display_name(&self) -> String {
        if !self.card.is_empty() {
            self.card.clone()
        } else {
            self.nickname.clone()
        }
    }
}

#[async_trait]
pub trait Feature: Send + Sync {
    fn feature_name(&self) -> &str;
    fn check_command(&self, msg: &Value) -> bool;
    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment>;
}

static MSG_PREFIX: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| env::var("BOT_MESSAGE_PREFIX").unwrap_or_default() + " ");
pub fn msg_segment_from_string(text: String) -> MessageSegment {
    MessageSegment::Text {
        data: bot_lib::structs::TextData {
            text: format!("{}{}", MSG_PREFIX.clone(), text),
        },
    }
}
