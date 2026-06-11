use crate::db;
use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde_json::Value;

use super::persona;

pub struct PersonaManageFeature;

impl PersonaManageFeature {
    pub fn feature_id() -> &'static str {
        "persona_manage"
    }

    pub fn feature_name() -> &'static str {
        "人格管理: -p list/create/set/show"
    }
}

#[async_trait]
impl Feature for PersonaManageFeature {
    fn feature_id(&self) -> &str {
        PersonaManageFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        PersonaManageFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        let text = msg
            .get("data")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        text.starts_with("-p ")
    }

    async fn deal_with_message(
        &self,
        ctx: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg
            .get("data")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())?;
        let rest = text.strip_prefix("-p ")?.trim();
        let pool = db::pg().await;

        if rest == "list" {
            let personas = persona::list_all(pool).await.ok()?;
            if personas.is_empty() {
                return Some(msg_segment_from_string("暂无已创建的人格。".to_string()));
            }
            let mut lines: Vec<String> = Vec::new();
            for p in &personas {
                let marker = if p.is_default { " [默认]" } else { "" };
                lines.push(format!("- {}{}", p.name, marker));
            }
            return Some(msg_segment_from_string(lines.join("\n")));
        }

        if rest == "show" {
            let gid = ctx.group_id.map(|g| g.to_string());
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

            return match persona {
                Some(p) => Some(msg_segment_from_string(format!(
                    "当前使用的人格: {}",
                    p.name
                ))),
                None => Some(msg_segment_from_string("当前未设置人格。".to_string())),
            };
        }

        if let Some(args) = rest.strip_prefix("create ") {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() < 2 {
                return Some(msg_segment_from_string(
                    "用法: -p create <名称> <系统提示词>".to_string(),
                ));
            }
            let name = parts[0].trim();
            let prompt = parts[1].trim();
            if name.is_empty() || prompt.is_empty() {
                return Some(msg_segment_from_string(
                    "请提供人格名称和系统提示词。".to_string(),
                ));
            }
            match persona::create(pool, name, prompt).await {
                Ok(p) => Some(msg_segment_from_string(format!(
                    "人格 '{}' 创建成功。",
                    p.name
                ))),
                Err(e) => Some(msg_segment_from_string(format!("创建人格失败: {}", e))),
            }
        } else if let Some(name) = rest.strip_prefix("set ") {
            let name = name.trim();
            if name.is_empty() {
                return Some(msg_segment_from_string(
                    "用法: -p set <名称> 将当前群切换到指定人格".to_string(),
                ));
            }
            let personas = persona::list_all(pool).await.ok()?;
            let target = personas.iter().find(|p| p.name == name);
            let target = match target {
                Some(p) => p,
                None => {
                    return Some(msg_segment_from_string(format!(
                        "未找到人格 '{}'。使用 -p list 查看可用人格。",
                        name
                    )));
                }
            };

            if let Some(gid) = ctx.group_id {
                match persona::set_group_persona(pool, &gid.to_string(), target.id).await {
                    Ok(_) => Some(msg_segment_from_string(format!(
                        "本群人格已切换为: {}",
                        target.name
                    ))),
                    Err(e) => Some(msg_segment_from_string(format!("设置失败: {}", e))),
                }
            } else {
                match persona::set_default(pool, target.id).await {
                    Ok(_) => Some(msg_segment_from_string(format!(
                        "默认人格已切换为: {}",
                        target.name
                    ))),
                    Err(e) => Some(msg_segment_from_string(format!("设置失败: {}", e))),
                }
            }
        } else {
            Some(msg_segment_from_string(
                "可用命令: -p list | -p create <名称> <提示词> | -p set <名称> | -p show".to_string(),
            ))
        }
    }
}
