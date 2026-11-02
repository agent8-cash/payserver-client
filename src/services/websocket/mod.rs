//! WebSocket service for real-time invoice and payment status updates.

mod reconnect;
mod service;
mod types;

pub use service::WebSocketService;
pub use types::{ConnectionState, StatusUpdate};
