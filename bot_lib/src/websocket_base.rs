use crate::api_types::{ReconnectionConfig as ApiReconnectionConfig, WSCloseRes, WSConnecting, WSErrorRes, WSOpenRes};
use crate::event_bus::EventBus;
use crate::utils::{cq_decode, cq_to_json};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest};

#[derive(Error, Debug)]
pub enum WebSocketError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Timeout")]
    Timeout,
}

#[derive(Debug, Clone)]
pub struct WebSocketOptions {
    pub base_url: String,
    pub access_token: String,
    pub reconnection: ApiReconnectionConfig,
    pub throw_on_error: bool,
    pub debug: bool,
}

impl WebSocketOptions {
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            base_url: url.into(),
            access_token: String::new(),
            reconnection: ApiReconnectionConfig::default(),
            throw_on_error: false,
            debug: false,
        }
    }

    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = token.into();
        self
    }

    pub fn with_reconnection(mut self, enable: bool, attempts: u32, delay_ms: u64) -> Self {
        self.reconnection = ApiReconnectionConfig {
            enable,
            attempts,
            delay: delay_ms,
            now_attempts: 1,
        };
        self
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }
}

use std::collections::HashMap;

pub type PendingRequests = Arc<std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<Result<serde_json::Value, WebSocketError>>>>>;

pub struct NapcatWebSocketBase {
    options: WebSocketOptions,
    event_bus: EventBus,
    event_tx: broadcast::Sender<serde_json::Value>,
    write_tx: mpsc::UnboundedSender<String>,
    write_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<String>>,
    pending: PendingRequests,
    request_timeout: std::time::Duration,
}

impl NapcatWebSocketBase {
    pub fn new(options: WebSocketOptions) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(100);
        let (write_tx, write_rx) = mpsc::unbounded_channel();
        let base = Self {
            options,
            event_bus: EventBus::new(),
            event_tx,
            write_tx,
            write_rx: tokio::sync::Mutex::new(write_rx),
            pending: Arc::new(std::sync::Mutex::new(HashMap::new())),
            request_timeout: std::time::Duration::from_secs(30),
        };
        Arc::new(base)
    }

    pub fn url(&self) -> String {
        if self.options.access_token.is_empty() {
            self.options.base_url.clone()
        } else {
            let base = if self.options.base_url.ends_with('/') {
                self.options.base_url.clone()
            } else {
                format!("{}/", self.options.base_url)
            };
            format!("{}?access_token={}", base, self.options.access_token)
        }
    }

    fn build_request(&self) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, WebSocketError> {
        let url = self.url();
        let mut request = url.into_client_request().map_err(|e| WebSocketError::ConnectionFailed(e.to_string()))?;
        if !self.options.access_token.is_empty() {
            request.headers_mut().insert(
                "Authorization",
                format!("Bearer {}", self.options.access_token)
                    .parse()
                    .map_err(|e: tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue| {
                        WebSocketError::ConnectionFailed(e.to_string())
                    })?,
            );
        }
        Ok(request)
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn event_receiver(&self) -> broadcast::Receiver<serde_json::Value> {
        self.event_tx.subscribe()
    }

    pub async fn on<F>(&self, event: &str, handler: F)
    where
        F: Fn(serde_json::Value) + Send + Sync + 'static,
    {
        self.event_bus.on(event, handler).await;
    }

    pub async fn once<F>(&self, event: &str, handler: F)
    where
        F: Fn(serde_json::Value) + Send + Sync + 'static,
    {
        self.event_bus.once(event, handler).await;
    }

    pub async fn off(&self, event: &str) {
        self.event_bus.off(event).await;
    }

    pub async fn emit(&self, event: &str, data: serde_json::Value) {
        self.event_bus.emit(event, data).await;
    }

    fn fail_all_pending(&self) {
        let mut pending = self.pending.lock().unwrap();
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(WebSocketError::ConnectionFailed("connection closed".into())));
        }
    }

    pub async fn run(&self) -> Result<(), WebSocketError> {
        let mut reconnect_attempts = 0;
        let mut write_rx = self.write_rx.lock().await;
        
        loop {
            let reconnection = self.options.reconnection.clone();
            
            let wsconnecting = WSConnecting { reconnection: reconnection.clone() };
            self.event_bus.emit_socket_connecting(wsconnecting).await;

            let request = self.build_request()?;

            match connect_async(request).await {
                Ok((ws_stream, _)) => {
                    info!("WebSocket connected");
                    reconnect_attempts = 0;
                    
                    let wsopen = WSOpenRes { reconnection: reconnection.clone() };
                    self.event_bus.emit_socket_open(wsopen).await;

                    let (mut write, mut read) = ws_stream.split();
                    let event_bus = self.event_bus.clone();
                    let event_tx = self.event_tx.clone();
                    let debug = self.options.debug;
                    let throw_on_error = self.options.throw_on_error;

                    loop {
                        tokio::select! {
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        if debug {
                                            debug!("Received: {}", text);
                                        }
                                        if let Some(json) = self.parse_message(&text) {
                                            self.handle_message(&json).await;
                                            if event_tx.receiver_count() > 0 {
                                                let _ = event_tx.send(json.clone());
                                            }
                                            event_bus.parse_message(&json).await;
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        info!("WebSocket closed");
                                        self.fail_all_pending();
                                        let wsclose = WSCloseRes {
                                            code: 1000,
                                            reason: "Normal closure".to_string(),
                                            reconnection: reconnection.clone(),
                                        };
                                        self.event_bus.emit_socket_close(wsclose).await;
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        error!("WebSocket error: {}", e);
                                        self.fail_all_pending();
                                        let wserror = WSErrorRes::ConnectError {
                                            reconnection: reconnection.clone(),
                                            error_type: "connect_error".to_string(),
                                            errors: vec![None],
                                        };
                                        self.event_bus.emit_socket_error(wserror).await;
                                        if throw_on_error {
                                            return Err(WebSocketError::ConnectionFailed(e.to_string()));
                                        }
                                        break;
                                    }
                                    None => {
                                        warn!("WebSocket stream ended");
                                        self.fail_all_pending();
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            outgoing = write_rx.recv() => {
                                match outgoing {
                                    Some(text) => {
                                        if let Err(e) = write.send(Message::Text(text.into())).await {
                                            error!("WebSocket write error: {}", e);
                                            break;
                                        }
                                    }
                                    None => {
                                        warn!("Write channel closed");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Connection failed: {}", e);
                    self.fail_all_pending();
                    let wserror = WSErrorRes::ConnectError {
                        reconnection: reconnection.clone(),
                        error_type: "connect_error".to_string(),
                        errors: vec![None],
                    };
                    self.event_bus.emit_socket_error(wserror).await;
                }
            }

            if self.options.reconnection.enable {
                reconnect_attempts += 1;
                if reconnect_attempts >= self.options.reconnection.attempts {
                    error!("Max reconnection attempts reached");
                    return Err(WebSocketError::ConnectionFailed("Max reconnection attempts reached".into()));
                }
                
                info!(
                    "Reconnecting in {}ms (attempt {}/{})",
                    self.options.reconnection.delay,
                    reconnect_attempts,
                    self.options.reconnection.attempts
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(self.options.reconnection.delay)).await;
            } else {
                break;
            }
        }

        Ok(())
    }

    fn parse_message(&self, text: &str) -> Option<serde_json::Value> {
        let trimmed = text.trim();
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            warn!("Received non-JSON data: {}", trimmed);
            return None;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(mut json) => {
                if let Some(post_type) = json.get("post_type").and_then(|v| v.as_str()) {
                    if post_type == "message" || post_type == "message_sent" {
                        if let Some(message_format) = json.get("message_format").and_then(|v| v.as_str()) {
                            if message_format == "string" {
                                if let Some(message) = json.get("message").and_then(|v| v.as_str()) {
                                    let decoded = cq_decode(message);
                                    json["message"] = serde_json::Value::Array(cq_to_json(&decoded));
                                    json["message_format"] = serde_json::Value::String("array".to_string());
                                }
                            }
                        }
                        if let Some(raw_message) = json.get("raw_message").and_then(|v| v.as_str()) {
                            json["raw_message"] = serde_json::Value::String(cq_decode(raw_message));
                        }
                    }
                }
                Some(json)
            }
            Err(e) => {
                warn!("Failed to parse JSON: {}", e);
                None
            }
        }
    }

    async fn handle_message(&self, json: &serde_json::Value) {
        if let Some(echo) = json.get("echo").and_then(|v| v.as_str()) {
            let tx_opt = {
                let mut pending = self.pending.lock().unwrap();
                pending.remove(echo)
            };

            if let Some(tx) = tx_opt {
                let retcode = json.get("retcode").and_then(|v| v.as_i64()).unwrap_or(0);
                if retcode == 0 {
                    self.event_bus.emit_api_success(json.clone()).await;
                    let _ = tx.send(Ok(json.get("data").cloned().unwrap_or_else(|| serde_json::json!({}))));
                } else {
                    self.event_bus.emit_api_failure(json.clone()).await;
                    let msg = json.get("msg").or_else(|| json.get("message")).and_then(|v| v.as_str()).unwrap_or("Unknown error");
                    let _ = tx.send(Err(WebSocketError::ApiError(msg.to_string())));
                }
            } else {
                debug!("Received late or unknown echo: {}", echo);
            }

            if self.event_tx.receiver_count() > 0 {
                let _ = self.event_tx.send(json.clone());
            }
        } else if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
            if status == "failed" {
                let mut reconnection = self.options.reconnection.clone();
                reconnection.enable = false;
                let wserror = WSErrorRes::ResponseError {
                    reconnection: reconnection.clone(),
                    error_type: "response_error".to_string(),
                    info: crate::api_types::ResponseErrorInfo {
                        errno: json.get("retcode").and_then(|v| v.as_i64()).unwrap_or(0),
                        message: json.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        url: self.url(),
                    },
                };
                self.event_bus.emit_socket_error(wserror).await;
            }
        }
    }

    pub async fn send_raw(&self, action: &str, params: serde_json::Value) -> Result<serde_json::Value, WebSocketError> {
        let echo = nanoid::nanoid!();
        let request = serde_json::json!({
            "action": action,
            "params": params,
            "echo": echo
        });

        if self.options.debug {
            debug!("Sending: {}", request);
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(echo.clone(), tx);
        }

        self.event_bus.emit_api_presend(request.clone()).await;

        let text = serde_json::to_string(&request)
            .map_err(|e| WebSocketError::SendFailed(e.to_string()))?;
        self.write_tx
            .send(text)
            .map_err(|e| WebSocketError::SendFailed(e.to_string()))?;

        struct Cleanup {
            pending: PendingRequests,
            echo: String,
        }
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Ok(mut p) = self.pending.lock() {
                    p.remove(&self.echo);
                }
            }
        }
        let _cleanup = Cleanup {
            pending: Arc::clone(&self.pending),
            echo: echo.clone(),
        };

        match tokio::time::timeout(self.request_timeout, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err(WebSocketError::ConnectionFailed("connection closed".into())),
            Err(_) => Err(WebSocketError::Timeout),
        }
    }
}
