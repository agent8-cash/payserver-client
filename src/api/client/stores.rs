//! Store API methods: stores, payment methods, webhooks, settings, token
//! policy, and wallets.

use super::{ApiClient, ApiError};
use crate::api::{
    CreatePaymentMethodRequest, CreateStoreRequest, CreateWalletRequest, SetTokenPolicyRequest,
    Store, StorePaymentMethod, StoreSettings, StoreWebhook, TokenPolicy,
    UpdatePaymentMethodRequest, UpdateStoreRequest, UpdateStoreSettingsRequest,
    UpdateWalletRequest, UpdateWebhookRequest, Wallet, WalletXpubResponse,
};

/// Path for a single wallet, shared by `update_wallet` and `delete_wallet` so
/// neither drifts onto another domain's `/api/{collection}/{id}` shape.
fn wallet_path(id: &str) -> String {
    format!("/api/wallets/{}", id)
}

impl ApiClient {
    // =========================================================================
    // Stores
    // =========================================================================

    /// List stores.
    pub async fn list_stores(&self) -> Result<Vec<Store>, ApiError> {
        self.get("/api/stores").await
    }

    /// Get a store by ID.
    pub async fn get_store(&self, id: &str) -> Result<Store, ApiError> {
        self.get(&format!("/api/stores/{}", id)).await
    }

    /// Create a new store.
    pub async fn create_store(&self, request: &CreateStoreRequest) -> Result<Store, ApiError> {
        self.post("/api/stores", request).await
    }

    /// Update a store.
    pub async fn update_store(
        &self,
        id: &str,
        request: &UpdateStoreRequest,
    ) -> Result<Store, ApiError> {
        self.put(&format!("/api/stores/{}", id), request).await
    }

    /// Delete a store.
    pub async fn delete_store(&self, id: &str) -> Result<(), ApiError> {
        self.delete(&format!("/api/stores/{}", id)).await
    }

    // =========================================================================
    // Payment Methods
    // =========================================================================

    /// List payment methods for a store.
    pub async fn list_payment_methods(
        &self,
        store_id: &str,
    ) -> Result<Vec<StorePaymentMethod>, ApiError> {
        self.get(&format!("/api/stores/{}/payment-methods", store_id))
            .await
    }

    /// Create a payment method for a store.
    pub async fn create_payment_method(
        &self,
        store_id: &str,
        request: &CreatePaymentMethodRequest,
    ) -> Result<StorePaymentMethod, ApiError> {
        self.post(
            &format!("/api/stores/{}/payment-methods", store_id),
            request,
        )
        .await
    }

    /// Update a payment method.
    pub async fn update_payment_method(
        &self,
        store_id: &str,
        method_id: &str,
        request: &UpdatePaymentMethodRequest,
    ) -> Result<StorePaymentMethod, ApiError> {
        self.put(
            &format!("/api/stores/{}/payment-methods/{}", store_id, method_id),
            request,
        )
        .await
    }

    /// Delete a payment method.
    pub async fn delete_payment_method(
        &self,
        store_id: &str,
        method_id: &str,
    ) -> Result<(), ApiError> {
        self.delete(&format!(
            "/api/stores/{}/payment-methods/{}",
            store_id, method_id
        ))
        .await
    }

    // =========================================================================
    // Webhooks
    // =========================================================================

    /// Get webhook configuration for a store.
    pub async fn get_store_webhook(&self, store_id: &str) -> Result<StoreWebhook, ApiError> {
        self.get(&format!("/api/stores/{}/webhook", store_id)).await
    }

    /// Configure (create or update) webhook for a store.
    /// Returns the webhook with the secret visible (only time it's shown).
    pub async fn configure_store_webhook(
        &self,
        store_id: &str,
        request: &UpdateWebhookRequest,
    ) -> Result<StoreWebhook, ApiError> {
        self.put(&format!("/api/stores/{}/webhook", store_id), request)
            .await
    }

    /// Delete webhook configuration for a store.
    pub async fn delete_store_webhook(&self, store_id: &str) -> Result<(), ApiError> {
        self.delete(&format!("/api/stores/{}/webhook", store_id))
            .await
    }

    // =========================================================================
    // Store Settings
    // =========================================================================

    /// Get store settings.
    pub async fn get_store_settings(&self, store_id: &str) -> Result<StoreSettings, ApiError> {
        self.get(&format!("/api/stores/{}/settings", store_id))
            .await
    }

    /// Update store settings (partial update).
    pub async fn update_store_settings(
        &self,
        store_id: &str,
        request: &UpdateStoreSettingsRequest,
    ) -> Result<StoreSettings, ApiError> {
        self.patch(&format!("/api/stores/{}/settings", store_id), request)
            .await
    }

    // =========================================================================
    // Token Policy
    // =========================================================================

    /// Get the token policy for a store.
    pub async fn get_token_policy(&self, store_id: &str) -> Result<Option<TokenPolicy>, ApiError> {
        self.get(&format!("/api/stores/{}/token-policy", store_id))
            .await
    }

    /// Set (upsert) the token policy for a store.
    pub async fn set_token_policy(
        &self,
        store_id: &str,
        request: &SetTokenPolicyRequest,
    ) -> Result<TokenPolicy, ApiError> {
        self.put(&format!("/api/stores/{}/token-policy", store_id), request)
            .await
    }

    /// Delete the token policy for a store.
    pub async fn delete_token_policy(&self, store_id: &str) -> Result<(), ApiError> {
        self.delete(&format!("/api/stores/{}/token-policy", store_id))
            .await
    }

    // =========================================================================
    // Wallets
    // =========================================================================

    /// List wallets.
    pub async fn list_wallets(&self) -> Result<Vec<Wallet>, ApiError> {
        self.get("/api/wallets").await
    }

    /// Get a wallet by ID.
    pub async fn get_wallet(&self, id: &str) -> Result<Wallet, ApiError> {
        self.get(&format!("/api/wallets/{}", id)).await
    }

    /// Add a wallet to the account.
    ///
    /// The server refuses an xpub already registered to another account with a
    /// 409 - one key, one account, so two merchants cannot derive the same
    /// addresses for different customers. Called from the add-wallet form on
    /// the wallets settings page (`src/pages/wallets.rs`).
    pub async fn create_wallet(&self, req: &CreateWalletRequest) -> Result<Wallet, ApiError> {
        self.post("/api/wallets", req).await
    }

    /// Rename a wallet, or make it the account primary.
    ///
    /// `is_primary: Some(true)` promotes; `Some(false)` is ignored server-side,
    /// because "no primary" is not a state a merchant can usefully ask for.
    /// Called from the wallets settings page (`src/pages/wallets.rs`).
    pub async fn update_wallet(
        &self,
        id: &str,
        req: &UpdateWalletRequest,
    ) -> Result<Wallet, ApiError> {
        self.patch(&wallet_path(id), req).await
    }

    /// Delete a wallet.
    ///
    /// Refused while any store still derives from it, so this cannot silently
    /// strand a store's payment methods. Called from the wallets settings page
    /// (`src/pages/wallets.rs`).
    pub async fn delete_wallet(&self, id: &str) -> Result<(), ApiError> {
        self.delete(&wallet_path(id)).await
    }

    /// Read a wallet's FULL xpub.
    ///
    /// Everything else returns it masked. This is the one call that does not,
    /// which is why it is a separate endpoint and should stay a deliberate act
    /// by the merchant rather than something a page fetches to render. Called
    /// from the wallets settings page's export action (`src/pages/wallets.rs`).
    pub async fn export_wallet_xpub(&self, id: &str) -> Result<WalletXpubResponse, ApiError> {
        self.get(&format!("/api/wallets/{}/xpub", id)).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::{RequestSpec, TestTransport};
    use super::*;
    use std::sync::{Arc, Mutex};

    // `wallet_path` is what `update_wallet` and `delete_wallet` actually
    // compute before handing off to the transport. A copy-pasted template
    // from another domain (`/api/stores/{}`, `/api/wallets/{}/xpub`) would
    // send an id-scoped request to the wrong collection, or to the wrong
    // wallet endpoint.
    #[test]
    fn wallet_path_targets_the_given_wallet_in_the_wallets_collection() {
        assert_eq!(wallet_path("abc-123"), "/api/wallets/abc-123");
    }

    #[test]
    fn wallet_path_does_not_carry_a_trailing_segment() {
        // export_wallet_xpub deliberately does - see its own doc comment -
        // but update_wallet/delete_wallet must not drift onto that path.
        assert!(!wallet_path("abc-123").ends_with("/xpub"));
    }

    /// Polls `fut` to completion. Every test below drives its call through
    /// a `TestTransport` that answers synchronously - `gloo-net` never runs,
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

    fn sample_wallet_json(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "user_id": "00000000-0000-0000-0000-000000000001",
            "namespace": "eip155",
            "xpub_masked": "xpub6D4B...eacc",
            "derivation_index": 0,
            "name": null,
            "is_primary": false,
            "created_at": "2026-01-01T00:00:00Z",
        })
    }

    // These three are the functions the ticket names: the ones that build a
    // request and hand it to the transport, as opposed to `wallet_path`,
    // which is a pure helper extracted *from* them. A test that only calls
    // the helper, or only checks a request struct's `Serialize` output,
    // cannot catch `update_wallet` calling `self.get` instead of
    // `self.patch`, or `delete_wallet` targeting the wrong path - only
    // driving the async function itself, through a transport that records
    // method+path+body together, can.

    #[test]
    fn create_wallet_posts_the_request_body_to_the_wallets_collection() {
        let (transport, recorded) =
            recording_transport(sample_wallet_json("11111111-1111-1111-1111-111111111111"));
        let client = ApiClient::with_test_transport("", transport);
        let req = CreateWalletRequest {
            xpub: "xpub6D4BDPcP2GT...".to_string(),
            name: None,
            namespace: "eip155".to_string(),
        };

        let result = block_on(client.create_wallet(&req));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "POST",
                path: "/api/wallets".to_string(),
                body: Some(serde_json::json!({
                    "xpub": "xpub6D4BDPcP2GT...",
                    "name": null,
                    "namespace": "eip155",
                })),
            }
        );
    }

    #[test]
    fn update_wallet_patches_the_given_wallet_with_an_explicit_promotion() {
        let (transport, recorded) =
            recording_transport(sample_wallet_json("11111111-1111-1111-1111-111111111111"));
        let client = ApiClient::with_test_transport("", transport);
        let req = UpdateWalletRequest {
            name: Some("Payouts".to_string()),
            is_primary: Some(true),
        };

        let result = block_on(client.update_wallet("wallet-1", &req));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "PATCH",
                path: "/api/wallets/wallet-1".to_string(),
                body: Some(serde_json::json!({
                    "name": "Payouts",
                    "is_primary": true,
                })),
            }
        );
    }

    #[test]
    fn update_wallet_sends_an_explicit_false_rather_than_omitting_it() {
        // `is_primary: Some(false)` is documented as ignored server-side,
        // but only because the server sees it - if this ever serialized to
        // nothing, the field would stop existing on the wire and that doc
        // comment would describe behavior no request can trigger.
        let (transport, recorded) =
            recording_transport(sample_wallet_json("11111111-1111-1111-1111-111111111111"));
        let client = ApiClient::with_test_transport("", transport);
        let req = UpdateWalletRequest {
            name: None,
            is_primary: Some(false),
        };

        let result = block_on(client.update_wallet("wallet-1", &req));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "PATCH",
                path: "/api/wallets/wallet-1".to_string(),
                body: Some(serde_json::json!({
                    "name": null,
                    "is_primary": false,
                })),
            }
        );
    }

    #[test]
    fn delete_wallet_deletes_the_given_wallet_with_no_body() {
        let (transport, recorded) = recording_transport(serde_json::Value::Null);
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.delete_wallet("wallet-1"));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "DELETE",
                path: "/api/wallets/wallet-1".to_string(),
                body: None,
            }
        );
    }

    // The full, unmasked xpub is the one thing this API returns nowhere else
    // - a request built against the wrong id, or against the masked wallet
    // endpoint instead of `/xpub`, would leak or withhold key material with
    // no type error to catch it. Only driving `export_wallet_xpub` itself
    // through a transport that records the request can.
    #[test]
    fn export_wallet_xpub_gets_the_xpub_subpath_of_the_given_wallet() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "user_id": "00000000-0000-0000-0000-000000000001",
            "namespace": "eip155",
            "xpub": "xpub6D4BDPcP2GT...",
            "derivation_index": 0,
            "name": null,
            "created_at": "2026-01-01T00:00:00Z",
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.export_wallet_xpub("wallet-1")).unwrap();

        assert_eq!(result.xpub, "xpub6D4BDPcP2GT...");
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/wallets/wallet-1/xpub".to_string(),
                body: None,
            }
        );
    }
}
