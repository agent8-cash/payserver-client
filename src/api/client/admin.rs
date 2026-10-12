//! Auth/user, dashboard, API key, and admin API methods.

use serde::Deserialize;

use super::{ApiClient, ApiError};
use crate::api::{
    ApiKeyListResponse, CreateApiKeyRequest, CreateApiKeyResponsePayload, DashboardAnalytics,
    DashboardStats, RotateApiKeyResponse, ServerSettingsResponse, UpdateServerSettingsRequest,
    UpdateUserRoleRequest, UserInfo, UserListResponse,
};

/// Whether the server booted with every plugin disabled.
///
/// `get_safe_mode` calls `/api/admin/safe-mode`, the same `/api/admin/*`
/// prefix as every other method in this file - `docker/nginx.conf`'s `/api/`
/// location strips that prefix before proxying to the server, which mounts
/// the route at `/admin/safe-mode` in its own `server/src/api/admin` module,
/// admin-gated alongside the rest of `/admin/*`. The server sets `safe_mode`
/// from `ETHPAY_DISABLE_PLUGINS` (or `--disable-plugins`) at boot, skips
/// loading every plugin when it is set, and logs the condition loudly at
/// startup; that logic lives in the server's own repository, so it never
/// appears in a diff against this crate. This client only displays the flag
/// the route returns.
///
/// Not in `api-types` yet: the response is a local, non-shared type today,
/// pending a fuller plugin admin contract - listing plugins and disabling
/// them individually - that doesn't exist yet.
#[derive(Debug, Clone, Deserialize)]
pub struct SafeModeStatus {
    pub safe_mode: bool,
}

/// Whether the server can send outgoing email at all.
///
/// Local rather than in `api-types`: it is one flag from one route.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailStatus {
    pub configured: bool,
}

impl ApiClient {
    // =========================================================================
    // Auth / User
    // =========================================================================

    /// Whether the server can send email, so the UI does not offer email
    /// channels that would never deliver.
    pub async fn get_email_status(&self) -> Result<EmailStatus, ApiError> {
        self.get("/api/users/me/email/status").await
    }

    /// Get the current authenticated user's info.
    pub async fn get_me(&self) -> Result<UserInfo, ApiError> {
        self.get("/api/auth/me").await
    }

    /// Log out the current session (server-side invalidation).
    /// Delete the authenticated account.
    ///
    /// `confirm` must be the account's email, or its id where it has none. The
    /// server refuses while the account's stores hold any payment, payout or
    /// refund - deleting would cascade through invoices into payments and erase
    /// a merchant's financial history - and answers 409 naming what blocked it.
    ///
    /// The confirmation is a query parameter rather than a body so that no DTO
    /// has to be defined twice, once here and once server-side.
    pub async fn delete_account(&self, confirm: &str) -> Result<(), ApiError> {
        self.delete(&format!(
            "/api/users/me?confirm={}",
            js_sys::encode_uri_component(confirm)
        ))
        .await
    }

    pub async fn logout(&self) -> Result<(), ApiError> {
        let request = self
            .build_request("POST", "/api/auth/logout")
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(ApiError::Unauthorized);
        }

        if !response.ok() {
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::Http {
                status: response.status(),
                message,
            });
        }

        Ok(())
    }

    // =========================================================================
    // Dashboard
    // =========================================================================

    /// Get dashboard statistics.
    pub async fn get_dashboard_stats(&self) -> Result<DashboardStats, ApiError> {
        self.get("/api/dashboard/stats").await
    }

    /// Get per-day, per-asset payment volume for the dashboard charts.
    ///
    /// `days` is the window size; the server rejects anything outside 1..=90
    /// rather than aggregating unbounded history.
    pub async fn get_dashboard_analytics(&self, days: u32) -> Result<DashboardAnalytics, ApiError> {
        self.get(&format!("/api/dashboard/analytics?days={days}"))
            .await
    }

    // =========================================================================
    // API Keys
    // =========================================================================

    /// List API keys for the authenticated user.
    pub async fn list_api_keys(&self) -> Result<ApiKeyListResponse, ApiError> {
        self.get("/api/users/api-keys").await
    }

    /// Create a new API key.
    pub async fn create_api_key(
        &self,
        request: &CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponsePayload, ApiError> {
        self.post("/api/users/api-keys", request).await
    }

    /// Revoke an API key.
    pub async fn revoke_api_key(&self, id: &str) -> Result<(), ApiError> {
        self.delete(&format!("/api/users/api-keys/{}", id)).await
    }

    /// Rotate an API key (deprecates old, creates new).
    pub async fn rotate_api_key(&self, id: &str) -> Result<RotateApiKeyResponse, ApiError> {
        self.post_empty(&format!("/api/users/api-keys/{}/rotate", id))
            .await
    }

    // =========================================================================
    // Admin
    // =========================================================================

    /// List all users (admin only).
    pub async fn list_users(&self, offset: i64, limit: i64) -> Result<UserListResponse, ApiError> {
        self.get(&format!(
            "/api/admin/users?offset={}&limit={}",
            offset, limit
        ))
        .await
    }

    /// Update a user's role (admin only).
    pub async fn update_user_role(
        &self,
        user_id: &str,
        request: &UpdateUserRoleRequest,
    ) -> Result<(), ApiError> {
        self.patch_empty(&format!("/api/admin/users/{}/role", user_id), request)
            .await
    }

    /// Lock a user account (admin only).
    pub async fn lock_user(&self, user_id: &str) -> Result<(), ApiError> {
        self.post_empty_body(&format!("/api/admin/users/{}/lock", user_id))
            .await
    }

    /// Unlock a user account (admin only).
    pub async fn unlock_user(&self, user_id: &str) -> Result<(), ApiError> {
        self.post_empty_body(&format!("/api/admin/users/{}/unlock", user_id))
            .await
    }

    /// Get server settings (admin only).
    pub async fn get_server_settings(&self) -> Result<ServerSettingsResponse, ApiError> {
        self.get("/api/admin/settings").await
    }

    /// Update server settings (admin only).
    pub async fn update_server_settings(
        &self,
        request: &UpdateServerSettingsRequest,
    ) -> Result<(), ApiError> {
        self.put_empty("/api/admin/settings", request).await
    }

    /// Get safe mode status - whether every plugin is disabled for this boot
    /// (admin only).
    pub async fn get_safe_mode(&self) -> Result<SafeModeStatus, ApiError> {
        self.get("/api/admin/safe-mode").await
    }
}

#[cfg(test)]
mod tests {
    use super::super::{RequestSpec, TestTransport};
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Polls `fut` to completion. `get_safe_mode` is driven through a
    /// `TestTransport` that answers synchronously - `gloo-net` never runs,
    /// so nothing here ever returns `Poll::Pending` and a full executor
    /// would be dead weight.
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        let mut fut = std::pin::pin!(fut);
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        loop {
            if let std::task::Poll::Ready(value) = fut.as_mut().poll(&mut cx) {
                return value;
            }
        }
    }

    /// A transport that records the single request it receives and answers
    /// it with `response`.
    fn recording_transport(
        response: serde_json::Value,
    ) -> (TestTransport, Arc<Mutex<Option<RequestSpec>>>) {
        let recorded = Arc::new(Mutex::new(None));
        let recorded_clone = recorded.clone();
        let transport: TestTransport = Arc::new(move |spec| {
            *recorded_clone.lock().unwrap() = Some(spec);
            Ok(response.clone())
        });
        (transport, recorded)
    }

    #[test]
    fn get_safe_mode_reads_and_deserializes_the_safe_mode_field() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "safe_mode": true,
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.get_safe_mode());

        assert!(result.unwrap().safe_mode);
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/admin/safe-mode".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn get_email_status_reads_the_configured_flag() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "configured": false,
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.get_email_status());

        assert!(!result.unwrap().configured);
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/users/me/email/status".to_string(),
                body: None,
            }
        );
    }
}
