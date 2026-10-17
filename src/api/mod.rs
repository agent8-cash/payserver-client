//! API client for a payserver.

mod client;
mod types;

pub use self::types::*;
pub use client::{ApiClient, ApiError, EmailStatus, SafeModeStatus, set_session_ended_handler};
