pub mod embedding;
pub mod knowledge;
pub mod manage;
pub mod persona;
pub mod prompt;
pub mod session;

use crate::db;
use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde_json::Value;

pub struct ChatFeature {
    llm_base_url: String,
    llm_api_key: String,
    llm_model: String,
    embed: embedding::EmbeddingClient,
}

impl ChatFeature {
    pub fn new(
        llm_base_url: String,
        llm_api_key: String,
        llm_model: String,
        embedding_url: String,
        embedding_model: String,
    ) -> Self {
        Self {
            embed: embedding::EmbeddingClient::new(embedding_url, embedding_model),
            llm_base_url,
            llm_api_key,
            llm_model,
        }
    }

    pub fn feature_id() -> &'static str {
        "chat"
    }

    pub fn feature_name() -> &'static str {
        "智能聊天: 人格+知识库对话"
    }

    async fn call_llm(&self, messages: &[serde_json::Value]) -> Result<String, String> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "model": self.llm_model,
            "max_tokens": 4096,
            "messages": messages,
        });

        let resp = client
            .post(format!("{}/chat/completions", self.llm_base_url))
            .header("Authorization", format!("Bearer {}", self.llm_api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("LLM request failed: {}", e))?;

        let status = resp.status();
        let body_text = resp.text().await.map_err(|e| format!("read response: {}", e))?;

        if !status.is_success() {
            let preview: String = body_text.chars().take(300).collect();
            return Err(format!("LLM HTTP {}: {}", status.as_u16(), preview));
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|e| format!("parse JSON: {}", e))?;
        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        if content.is_empty() {
            Err("LLM returned empty response".to_string())
        } else {
            Ok(content)
        }
    }
}

#[async_trait]
impl Feature for ChatFeature {
    fn feature_id(&self) -> &str {
        ChatFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        ChatFeature::feature_name()
    }

    fn is_passive(&self) -> bool {
        true
    }

    fn check_command(&self, msg: &Value) -> bool {
        msg.get("type").and_then(|t| t.as_str()) == Some("text")
    }

    async fn deal_with_message(
        &self,
        ctx: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg.get("data")?.get("text")?.as_str()?;
        let pool = db::pg().await;

        let vec = self.embed.embed(text).await.ok()?;

        let chunks = knowledge::KnowledgeBase::search(pool, &vec, 5)
            .await
            .unwrap_or_default();

        let gid = ctx.group_id.map(|g| g.to_string());
        let history = session::recent(pool, &ctx.user_id.to_string(), gid.as_deref(), 20)
            .await
            .unwrap_or_default();

        let persona = if let Some(ref gid_str) = gid {
            let gp = persona::get_for_group(pool, gid_str).await.ok().flatten();
            if gp.is_some() {
                gp
            } else {
                persona::get_default(pool).await.ok().flatten()
            }
        } else {
            persona::get_default(pool).await.ok().flatten()
        };

        let messages = prompt::build_messages(persona.as_ref(), &history, &chunks, text);

        let reply = match self.call_llm(&messages).await {
            Ok(r) => r,
            Err(e) => {
                log::error!("[chat] LLM call failed: {}", e);
                return Some(msg_segment_from_string(format!(
                    "AI 回复失败：{}",
                    e
                )));
            }
        };

        if let Some(ref p) = persona {
            let _ = session::save(
                pool,
                &ctx.user_id.to_string(),
                gid.as_deref(),
                p.id,
                "user",
                text,
            )
            .await;
            let _ = session::save(
                pool,
                &ctx.user_id.to_string(),
                gid.as_deref(),
                p.id,
                "assistant",
                &reply,
            )
            .await;
        }

        Some(msg_segment_from_string(reply))
    }
}
