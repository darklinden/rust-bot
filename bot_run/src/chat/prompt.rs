use super::knowledge::Chunk;
use super::persona::Persona;
use super::session::Message;

pub fn build_messages(
    persona: Option<&Persona>,
    history: &[Message],
    knowledge: &[Chunk],
    user_text: &str,
) -> Vec<serde_json::Value> {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    let system_prompt = build_system_prompt(persona, knowledge);
    if !system_prompt.is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt,
        }));
    }

    for msg in history {
        messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    messages.push(serde_json::json!({
        "role": "user",
        "content": user_text,
    }));

    messages
}

fn build_system_prompt(persona: Option<&Persona>, knowledge: &[Chunk]) -> String {
    let mut parts = Vec::new();

    if let Some(p) = persona {
        parts.push(p.system_prompt.clone());
    }

    if !knowledge.is_empty() {
        parts.push(String::new());
        parts.push("【参考知识】".to_string());
        for chunk in knowledge {
            parts.push(format!(
                "---\n来源: {}\n{}",
                chunk.source, chunk.text
            ));
        }
        parts.push("---".to_string());
    }

    parts.join("\n")
}
