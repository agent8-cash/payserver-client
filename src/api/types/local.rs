//! Shapes the server does not define, and presentation helpers over shared ones.
//!
//! Everything here is genuinely client-side. Anything the API actually speaks
//! belongs in `api-types` so both ends compile against one definition.

use serde::{Deserialize, Serialize};
use types::InvoiceStatus;

use super::Store;

/// How an invoice status is shown.
///
/// Labels and CSS classes are a rendering decision, so they stay here rather
/// than in the contract - the server has no opinion on what colour "expired" is.
pub trait InvoiceStatusExt {
    fn label(&self) -> &'static str;
    fn css_class(&self) -> &'static str;
}

impl InvoiceStatusExt for InvoiceStatus {
    fn label(&self) -> &'static str {
        match self {
            InvoiceStatus::Pending => "Pending",
            InvoiceStatus::Processing => "Processing",
            InvoiceStatus::PartiallyPaid => "Partially Paid",
            InvoiceStatus::Paid => "Paid",
            InvoiceStatus::Expired => "Expired",
            InvoiceStatus::Cancelled => "Cancelled",
            InvoiceStatus::Refunded => "Refunded",
            InvoiceStatus::LatePaid => "Late Paid",
        }
    }

    fn css_class(&self) -> &'static str {
        match self {
            InvoiceStatus::Pending => "badge badge-warning",
            InvoiceStatus::Processing => "badge badge-info",
            InvoiceStatus::PartiallyPaid => "badge badge-warning",
            InvoiceStatus::Paid => "badge badge-success",
            InvoiceStatus::Expired => "badge badge-error",
            InvoiceStatus::Cancelled => "badge badge-neutral",
            InvoiceStatus::Refunded => "badge badge-neutral",
            InvoiceStatus::LatePaid => "badge badge-info",
        }
    }
}

/// A role a user holds on a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRole {
    pub id: String,
    pub store_id: Option<String>,
    pub role: String,
    pub permissions: Vec<String>,
}

/// A store together with the caller's role on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStoreInfo {
    pub store: Store,
    pub role: StoreRole,
}

/// One page a plugin serves, flattened for navigation.
///
/// The server groups pages by plugin; navigation wants a flat list, and the
/// plugin id has to travel with each entry because it is half the URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPage {
    pub plugin_id: String,
    pub path: String,
    pub label: String,
    pub icon: payserver_plugin_api::PageIcon,
}

/// `GET /api/plugins`, as the server sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginPagesResponse {
    pub plugins: Vec<PluginPagesInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginPagesInfo {
    pub id: String,
    pub pages: Vec<PluginPageInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginPageInfo {
    pub path: String,
    pub label: String,
    pub icon: payserver_plugin_api::PageIcon,
}

/// The signed-in merchant's own account standing, from `/users/me/standing`.
///
/// Defined here because the server defines it in its own crate rather than in
/// `api-types`; the shape is four plain fields, read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountStandingInfo {
    pub state: StandingState,
    pub plan_name: Option<String>,
    pub paid_through: Option<String>,
    pub checkout_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandingState {
    Good,
    Expiring,
    Lapsed,
    /// Also what a state this client does not know deserializes to, so a new
    /// server-side state degrades to "show nothing" rather than failing the read.
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod standing_tests {
    use super::*;

    #[test]
    fn a_state_the_client_does_not_know_reads_as_unknown() {
        let info: AccountStandingInfo = serde_json::from_str(
            r#"{"state":"suspended","plan_name":null,"paid_through":null,"checkout_url":null}"#,
        )
        .unwrap();
        assert_eq!(info.state, StandingState::Unknown);
    }
}
