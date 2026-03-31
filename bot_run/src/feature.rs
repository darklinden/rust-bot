use std::collections::BTreeMap;
use std::env;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct FeatureInfo {
    pub id: String,
    pub name: String,
    pub loaded: bool,
}

impl FeatureInfo {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            loaded: false,
        }
    }
}

#[derive(Clone)]
pub struct FeatureEntry {
    pub info: FeatureInfo,
    pub factory: Arc<dyn Fn() -> Arc<dyn Feature + Send + Sync> + Send + Sync>,
}

pub struct FeatureManager {
    pub entries: BTreeMap<String, FeatureEntry>,
    pub loaded: Vec<Arc<dyn Feature + Send + Sync>>,
}

impl FeatureManager {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            loaded: Vec::new(),
        }
    }

    pub fn register<F>(&mut self, id: &str, name: &str, factory: F)
    where
        F: Fn() -> Arc<dyn Feature + Send + Sync> + Send + Sync + 'static,
    {
        if self.entries.contains_key(id) {
            return;
        }
        self.entries.insert(
            id.to_string(),
            FeatureEntry {
                info: FeatureInfo::new(id, name),
                factory: Arc::new(factory),
            },
        );
    }

    pub fn load_feature(&mut self, id: &str) -> Result<(), String> {
        let entry = self
            .entries
            .get(id)
            .ok_or_else(|| format!("未知功能: {}", id))?;
        let factory = entry.factory.clone();
        if !self.loaded.iter().any(|f| f.feature_id() == entry.info.id) {
            self.loaded.push(factory());
        }
        if let Some(e) = self.entries.get_mut(id) {
            e.info.loaded = true;
        }
        Ok(())
    }

    pub fn unload_feature(&mut self, id: &str) -> Result<(), String> {
        let feat_id = if let Some(entry) = self.entries.get(id) {
            entry.info.id.clone()
        } else {
            return Err(format!("未知功能: {}", id));
        };
        let initial_len = self.loaded.len();
        self.loaded.retain(|f| f.feature_id() != feat_id);
        if self.loaded.len() == initial_len {
            return Err(format!("功能 '{}' 未加载", id));
        }
        if let Some(e) = self.entries.get_mut(id) {
            e.info.loaded = false;
        }
        Ok(())
    }

    pub fn list_loaded(&self) -> Vec<String> {
        self.loaded
            .iter()
            .map(|f| f.feature_name().to_string())
            .collect()
    }

    pub fn list_all(&self) -> Vec<FeatureInfo> {
        self.entries.values().map(|e| e.info.clone()).collect()
    }
}

impl Default for FeatureManager {
    fn default() -> Self {
        Self::new()
    }
}

pub static FEATURE_MANAGER: once_cell::sync::Lazy<Arc<Mutex<FeatureManager>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(FeatureManager::new())));

pub struct FeatureConfig;

impl FeatureConfig {
    pub fn feature_id() -> &'static str {
        "feature_config"
    }
    pub fn feature_name() -> &'static str {
        "Feature管理: -features list/load/unload"
    }
}

#[async_trait]
impl Feature for FeatureConfig {
    fn feature_id(&self) -> &str {
        FeatureConfig::feature_id()
    }

    fn feature_name(&self) -> &str {
        FeatureConfig::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        let text = msg
            .get("data")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        text.starts_with("-features list")
            || text.starts_with("-features load ")
            || text.starts_with("-features unload ")
    }

    async fn deal_with_message(
        &self,
        _context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg.get("data")?.get("text")?.as_str()?;
        let response = if text.trim() == "-features list" {
            let manager = FEATURE_MANAGER.lock().unwrap();
            let all = manager.list_all();
            if all.is_empty() {
                "没有任何已注册的功能模块".to_string()
            } else {
                let loaded: Vec<_> = all.iter().filter(|f| f.loaded).collect();
                let available: Vec<_> = all.iter().filter(|f| !f.loaded).collect();
                let mut lines = Vec::new();
                if !loaded.is_empty() {
                    lines.push("=== 已加载的功能 ===".to_string());
                    for f in &loaded {
                        lines.push(format!("  [已加载] {} - {}", f.id, f.name));
                    }
                }
                if !available.is_empty() {
                    if !lines.is_empty() {
                        lines.push(String::new());
                    }
                    lines.push("=== 可加载的功能 ===".to_string());
                    for f in &available {
                        lines.push(format!("  [可加载] {} - {}", f.id, f.name));
                    }
                }
                lines.join("\n")
            }
        } else if let Some(name) = text.strip_prefix("-features load ") {
            let name = name.trim();
            if name.is_empty() {
                "请指定要加载的功能名称".to_string()
            } else {
                let mut manager = FEATURE_MANAGER.lock().unwrap();
                match manager.load_feature(name) {
                    Ok(()) => {
                        let entry = manager.entries.get(name);
                        let desc = entry.map(|e| e.info.name.as_str()).unwrap_or(name);
                        format!(
                            "功能 '{}' ({}: ...) 已加载",
                            name,
                            desc.split(':').next().unwrap_or(name)
                        )
                    }
                    Err(e) => e,
                }
            }
        } else if let Some(name) = text.strip_prefix("-features unload ") {
            let name = name.trim();
            if name.is_empty() {
                "请指定要卸载的功能名称".to_string()
            } else {
                let mut manager = FEATURE_MANAGER.lock().unwrap();
                match manager.unload_feature(name) {
                    Ok(()) => format!("功能 '{}' 已卸载", name),
                    Err(e) => e,
                }
            }
        } else {
            return None;
        };
        Some(msg_segment_from_string(response))
    }
}

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
    fn feature_id(&self) -> &str;
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
