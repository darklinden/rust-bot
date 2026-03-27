use crate::api_types::{
    HeartBeat, LifeCycle, WSCloseRes,
    WSConnecting, WSErrorRes, WSOpenRes,
};
use crate::websocket_base::NapcatWebSocketBase;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct HandlerEntry {
    pub id: u64,
    pub handler: Arc<dyn Fn(Value) + Send + Sync>,
}

pub struct EventBus {
    handlers: Arc<RwLock<HashMap<String, Vec<HandlerEntry>>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    ws: Option<Arc<NapcatWebSocketBase>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            ws: None,
        }
    }

    pub fn with_ws(ws: Arc<NapcatWebSocketBase>) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            ws: Some(ws),
        }
    }

    pub async fn on<F>(&self, event: &str, handler: F) -> u64
    where
        F: Fn(Value) + Send + Sync + 'static,
    {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let entry = HandlerEntry {
            id,
            handler: Arc::new(handler),
        };
        let mut handlers = self.handlers.write().await;
        handlers
            .entry(event.to_string())
            .or_insert_with(Vec::new)
            .push(entry);
        id
    }

    pub async fn off_by_id(&self, event: &str, id: u64) {
        let mut handlers = self.handlers.write().await;
        if let Some(list) = handlers.get_mut(event) {
            list.retain(|e| e.id != id);
            if list.is_empty() {
                handlers.remove(event);
            }
        }
    }

    pub async fn once<F>(&self, event: &str, handler: F) -> u64
    where
        F: Fn(Value) + Send + Sync + 'static,
    {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let bus = self.clone();
        let event_name = event.to_string();
        
        let wrapped_handler = Arc::new(move |data: Value| {
            let bus_clone = bus.clone();
            let event_name_clone = event_name.clone();
            handler(data);
            tokio::spawn(async move {
                bus_clone.off_by_id(&event_name_clone, id).await;
            });
        });
        
        let entry = HandlerEntry {
            id,
            handler: wrapped_handler,
        };
        let mut handlers = self.handlers.write().await;
        handlers
            .entry(event.to_string())
            .or_insert_with(Vec::new)
            .push(entry);
        id
    }

    pub async fn off(&self, event: &str) {
        let mut handlers = self.handlers.write().await;
        handlers.remove(event);
    }

    pub async fn emit(&self, event: &str, data: Value) {
        self.emit_internal(event, data).await;
    }

    fn emit_internal<'a>(&'a self, event: &'a str, data: Value) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let handlers_to_run = {
                let handlers = self.handlers.read().await;
                handlers.get(event).cloned()
            };

            if let Some(handler_list) = handlers_to_run {
                for entry in handler_list {
                    (entry.handler)(data.clone());
                }
            }

            if let Some(last_dot) = event.rfind('.') {
                let parent_event = &event[..last_dot];
                self.emit_internal(parent_event, data).await;
            }
        })
    }

    pub async fn emit_socket_connecting(&self, res: WSConnecting) {
        self.emit("socket.connecting", serde_json::to_value(res).unwrap()).await;
    }

    pub async fn emit_socket_open(&self, res: WSOpenRes) {
        self.emit("socket.open", serde_json::to_value(res).unwrap()).await;
    }

    pub async fn emit_socket_close(&self, res: WSCloseRes) {
        self.emit("socket.close", serde_json::to_value(res).unwrap()).await;
    }

    pub async fn emit_socket_error(&self, res: WSErrorRes) {
        self.emit("socket.error", serde_json::to_value(res).unwrap()).await;
    }

    pub async fn emit_api_presend(&self, params: Value) {
        self.emit("api.preSend", params).await;
    }

    pub async fn emit_api_success(&self, res: Value) {
        self.emit("api.response.success", res).await;
    }

    pub async fn emit_api_failure(&self, res: Value) {
        self.emit("api.response.failure", res).await;
    }

    pub async fn parse_message(&self, json: &Value) {
        let post_type = json.get("post_type").and_then(|v| v.as_str());

        match post_type {
            Some("meta_event") => {
                self.meta_event(json).await;
            }
            Some("message") => {
                self.message(json).await;
            }
            Some("message_sent") => {
                self.message_sent(json).await;
            }
            Some("request") => {
                self.request(json).await;
            }
            Some("notice") => {
                self.notice(json).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown post_type: {:?}", post_type);
            }
        }
    }

    async fn meta_event(&self, json: &Value) {
        let meta_event_type = json.get("meta_event_type").and_then(|v| v.as_str());

        match meta_event_type {
            Some("lifecycle") => {
                self.life_cycle(json).await;
            }
            Some("heartbeat") => {
                if let Ok(heartbeat) = serde_json::from_value::<HeartBeat>(json.clone()) {
                    self.emit("meta_event.heartbeat", serde_json::to_value(heartbeat).unwrap()).await;
                }
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown meta_event_type: {:?}", meta_event_type);
            }
        }
    }

    async fn life_cycle(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("connect") => {
                if let Ok(lifecycle) = serde_json::from_value::<LifeCycle>(json.clone()) {
                    self.emit("meta_event.lifecycle.connect", serde_json::to_value(lifecycle).unwrap()).await;
                }
            }
            Some("enable") => {
                if let Ok(lifecycle) = serde_json::from_value::<LifeCycle>(json.clone()) {
                    self.emit("meta_event.lifecycle.enable", serde_json::to_value(lifecycle).unwrap()).await;
                }
            }
            Some("disable") => {
                if let Ok(lifecycle) = serde_json::from_value::<LifeCycle>(json.clone()) {
                    self.emit("meta_event.lifecycle.disable", serde_json::to_value(lifecycle).unwrap()).await;
                }
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown meta_event.lifecycle_type: {:?}", sub_type);
            }
        }
    }

    async fn message(&self, json: &Value) {
        let message_type = json.get("message_type").and_then(|v| v.as_str());

        match message_type {
            Some("private") => {
                self.message_private(json).await;
            }
            Some("group") => {
                self.message_group(json).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown message_type: {:?}", message_type);
            }
        }
    }

    async fn message_private(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("group") => {
                self.emit("message.private.group", json.clone()).await;
            }
            Some("friend") => {
                self.emit("message.private.friend", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown message_private_type: {:?}", sub_type);
            }
        }
    }

    async fn message_group(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("normal") => {
                self.emit("message.group.normal", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown message_group_type: {:?}", sub_type);
            }
        }
    }

    async fn message_sent(&self, json: &Value) {
        let message_type = json.get("message_type").and_then(|v| v.as_str());

        match message_type {
            Some("private") => {
                self.message_sent_private(json).await;
            }
            Some("group") => {
                self.message_sent_group(json).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown message_sent_type: {:?}", message_type);
            }
        }
    }

    async fn message_sent_private(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("group") => {
                self.emit("message_sent.private.group", json.clone()).await;
            }
            Some("friend") => {
                self.emit("message_sent.private.friend", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown message_sent_private_type: {:?}", sub_type);
            }
        }
    }

    async fn message_sent_group(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("normal") => {
                self.emit("message_sent.group.normal", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown message_sent_group_type: {:?}", sub_type);
            }
        }
    }

    async fn request(&self, json: &Value) {
        let request_type = json.get("request_type").and_then(|v| v.as_str());

        match request_type {
            Some("friend") => {
                self.emit("request.friend", json.clone()).await;
            }
            Some("group") => {
                self.request_group(json).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown request_type: {:?}", request_type);
            }
        }
    }

    async fn request_group(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("add") => {
                self.emit("request.group.add", json.clone()).await;
            }
            Some("invite") => {
                self.emit("request.group.invite", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown request_group_type: {:?}", sub_type);
            }
        }
    }

    async fn notice(&self, json: &Value) {
        let notice_type = json.get("notice_type").and_then(|v| v.as_str());

        match notice_type {
            Some("bot_offline") => {
                self.emit("notice.bot_offline", json.clone()).await;
            }
            Some("friend_add") => {
                self.emit("notice.friend_add", json.clone()).await;
            }
            Some("friend_recall") => {
                self.emit("notice.friend_recall", json.clone()).await;
            }
            Some("group_admin") => {
                self.notice_group_admin(json).await;
            }
            Some("group_ban") => {
                self.notice_group_ban(json).await;
            }
            Some("group_card") => {
                self.emit("notice.group_card", json.clone()).await;
            }
            Some("group_decrease") => {
                self.notice_group_decrease(json).await;
            }
            Some("essence") => {
                self.notice_essence(json).await;
            }
            Some("group_increase") => {
                self.notice_group_increase(json).await;
            }
            Some("notify") => {
                self.notice_notify(json).await;
            }
            Some("group_recall") => {
                self.emit("notice.group_recall", json.clone()).await;
            }
            Some("group_upload") => {
                self.emit("notice.group_upload", json.clone()).await;
            }
            Some("group_msg_emoji_like") => {
                self.emit("notice.group_msg_emoji_like", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_type: {:?}", notice_type);
            }
        }
    }

    async fn notice_group_admin(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("set") => {
                self.emit("notice.group_admin.set", json.clone()).await;
            }
            Some("unset") => {
                self.emit("notice.group_admin.unset", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_group_admin_type: {:?}", sub_type);
            }
        }
    }

    async fn notice_group_ban(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("ban") => {
                self.emit("notice.group_ban.ban", json.clone()).await;
            }
            Some("lift_ban") => {
                self.emit("notice.group_ban.lift_ban", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_group_ban_type: {:?}", sub_type);
            }
        }
    }

    async fn notice_group_decrease(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("leave") => {
                self.emit("notice.group_decrease.leave", json.clone()).await;
            }
            Some("kick") => {
                self.emit("notice.group_decrease.kick", json.clone()).await;
            }
            Some("kick_me") => {
                self.emit("notice.group_decrease.kick_me", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_group_decrease_type: {:?}", sub_type);
            }
        }
    }

    async fn notice_group_increase(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("approve") => {
                self.emit("notice.group_increase.approve", json.clone()).await;
            }
            Some("invite") => {
                self.emit("notice.group_increase.invite", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_group_increase_type: {:?}", sub_type);
            }
        }
    }

    async fn notice_essence(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("add") => {
                self.emit("notice.essence.add", json.clone()).await;
            }
            Some("delete") => {
                self.emit("notice.essence.delete", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_essence_type: {:?}", sub_type);
            }
        }
    }

    async fn notice_notify(&self, json: &Value) {
        let sub_type = json.get("sub_type").and_then(|v| v.as_str());

        match sub_type {
            Some("group_name") => {
                self.emit("notice.notify.group_name", json.clone()).await;
            }
            Some("title") => {
                self.emit("notice.notify.title", json.clone()).await;
            }
            Some("input_status") => {
                self.notice_notify_input_status(json).await;
            }
            Some("poke") => {
                self.notice_notify_poke(json).await;
            }
            Some("profile_like") => {
                self.emit("notice.notify.profile_like", json.clone()).await;
            }
            _ => {
                log::warn!("[bot_lib] [eventBus] unknown notice_notify_type: {:?}", sub_type);
            }
        }
    }

    async fn notice_notify_input_status(&self, json: &Value) {
        let group_id = json.get("group_id").and_then(|v| v.as_i64());

        if group_id.is_some_and(|id| id != 0) {
            self.emit("notice.notify.input_status.group", json.clone()).await;
        } else {
            self.emit("notice.notify.input_status.friend", json.clone()).await;
        }
    }

    async fn notice_notify_poke(&self, json: &Value) {
        if json.get("group_id").is_some() {
            self.emit("notice.notify.poke.group", json.clone()).await;
        } else {
            self.emit("notice.notify.poke.friend", json.clone()).await;
        }
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            handlers: Arc::clone(&self.handlers),
            next_id: Arc::clone(&self.next_id),
            ws: self.ws.clone(),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
