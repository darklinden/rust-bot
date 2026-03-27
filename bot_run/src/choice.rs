use async_trait::async_trait;
use crate::feature::{Feature, MessageContext};
use bot_lib::structs::MessageSegment;
use serde_json::Value;

pub struct ChoiceFeature;

#[async_trait]
impl Feature for ChoiceFeature {
    fn feature_name(&self) -> &str {
        "帮我选: 帮我选 + 选项1 + 选项2 + ... 来帮你做选择"
    }

    fn check_command(&self, msg: &Value) -> bool {
        if let Some(msg_type) = msg.get("type").and_then(|v| v.as_str()) {
            if msg_type != "text" {
                return false;
            }
        }

        if let Some(text) = msg.get("data").and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
            text.starts_with("choice ")
                || text.starts_with("-choice ")
                || text.starts_with("帮我选 ")
        } else {
            false
        }
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg.get("data")?.get("text")?.as_str()?;

        let options = if text.starts_with("choice ") {
            text.strip_prefix("choice ")?.split_whitespace().collect::<Vec<_>>()
        } else if text.starts_with("-choice ") {
            text.strip_prefix("-choice ")?.split_whitespace().collect::<Vec<_>>()
        } else if text.starts_with("帮我选 ") {
            text.strip_prefix("帮我选 ")?.split_whitespace().collect::<Vec<_>>()
        } else {
            return None;
        };

        if options.len() < 2 {
            return Some(MessageSegment::Text {
                data: bot_lib::structs::TextData { text: "请至少提供两个选项哦！".to_string() },
            });
        }

        let index = (rand::random::<f64>() * options.len() as f64).floor() as usize;
        let choice = options.get(index)?;

        let display_name = context.display_name();
        let user_id = context.user_id;
        let response = format!("帮 {}({}) 选择了：{}", display_name, user_id, choice);

        Some(MessageSegment::Text {
            data: bot_lib::structs::TextData { text: response },
        })
    }
}
