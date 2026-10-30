//! API client for a payserver.

mod client;
mod types;

pub use self::types::*;
pub use client::{
    ApiClient, ApiError, DeviceInfo, EmailStatus, SafeModeStatus, set_session_ended_handler,
};
