//! bot_lib - A Rust library for connecting to napcat QQ via WebSocket
//!
//! This library provides a WebSocket client for the OneBot 11 protocol,
//! specifically designed for napcat QQ.

pub mod api_types;
pub mod event_bus;
pub mod structs;
pub mod utils;
pub mod websocket_api;
pub mod websocket_base;

// Re-export commonly used types
pub use api_types::*;
pub use event_bus::EventBus;
pub use structs::{MessageSegment, Segment};
pub use utils::{cq_decode, cq_encode, cq_to_json, json_to_cq, logger};
pub use websocket_api::NapcatWebSocket;
pub use websocket_base::{NapcatWebSocketBase, WebSocketError, WebSocketOptions};
