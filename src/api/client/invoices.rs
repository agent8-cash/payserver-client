//! Invoice API methods.

use super::{ApiClient, ApiError};
use crate::api::{
    CheckoutResponse, CreateInvoiceRequest, Invoice, InvoiceListResponse, InvoiceStatusResponse,
    Payment, TxHashLookupResponse,
};

impl ApiClient {
    /// List invoices with filters and pagination.
    ///
    /// `store_id` of `None` means "All Stores": the server then returns
    /// invoices across every store, which it only allows for server admins —
    /// any other caller gets `400 Bad Request`.
    ///
    /// `search` is a free-text term over id, currency, amount and metadata,
    /// applied in SQL so `total` counts the same rows the page shows.
    pub async fn list_invoices(
        &self,
        store_id: Option<&str>,
        status: Option<&str>,
        currency: Option<&str>,
        search: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<InvoiceListResponse, ApiError> {
        let mut params = Vec::new();
        if let Some(sid) = store_id {
            params.push(format!("store_id={}", js_sys::encode_uri_component(sid)));
        }
        if let Some(s) = status {
            params.push(format!("status={}", js_sys::encode_uri_component(s)));
        }
        if let Some(c) = currency {
            params.push(format!("currency={}", js_sys::encode_uri_component(c)));
        }
        if let Some(q) = search {
            params.push(format!("search={}", js_sys::encode_uri_component(q)));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        self.get(&format!("/api/invoices?{}", params.join("&")))
            .await
    }

    /// Get an invoice by ID.
    pub async fn get_invoice(&self, id: &str) -> Result<Invoice, ApiError> {
        self.get(&format!("/api/invoices/{}", id)).await
    }

    /// Get public checkout data for an invoice (no auth required).
    pub async fn get_checkout(&self, invoice_id: &str) -> Result<CheckoutResponse, ApiError> {
        self.get(&format!(
            "/api/checkout/{}",
            js_sys::encode_uri_component(invoice_id)
        ))
        .await
    }

    /// Create a new invoice.
    pub async fn create_invoice(
        &self,
        request: &CreateInvoiceRequest,
    ) -> Result<Invoice, ApiError> {
        self.post("/api/invoices", request).await
    }

    /// Get payments for an invoice.
    pub async fn get_invoice_payments(&self, invoice_id: &str) -> Result<Vec<Payment>, ApiError> {
        self.get(&format!("/api/invoices/{}/payments", invoice_id))
            .await
    }

    /// Get invoice status (includes payment options and payments).
    pub async fn get_invoice_status(
        &self,
        invoice_id: &str,
    ) -> Result<InvoiceStatusResponse, ApiError> {
        self.get(&format!("/api/invoices/{}/status", invoice_id))
            .await
    }

    /// Export invoices as CSV text.
    ///
    /// `store_id` of `None` exports across all stores (admins only) — see
    /// [`Self::list_invoices`].
    ///
    /// `search` is passed for the same reason `status` is: the export shares
    /// the list's filter builders server-side, and an export that ignores the
    /// search box hands the merchant rows they cannot see.
    pub async fn export_invoices_csv(
        &self,
        store_id: Option<&str>,
        status: Option<&str>,
        search: Option<&str>,
    ) -> Result<String, ApiError> {
        let mut params = Vec::new();
        if let Some(sid) = store_id {
            params.push(format!("store_id={}", js_sys::encode_uri_component(sid)));
        }
        if let Some(s) = status {
            params.push(format!("status={}", js_sys::encode_uri_component(s)));
        }
        if let Some(q) = search {
            params.push(format!("search={}", js_sys::encode_uri_component(q)));
        }
        self.get_text(&format!("/api/invoices/export.csv?{}", params.join("&")))
            .await
    }

    /// Look up an invoice by transaction hash.
    pub async fn lookup_invoice_by_tx(
        &self,
        chain_id: u64,
        tx_hash: &str,
    ) -> Result<TxHashLookupResponse, ApiError> {
        self.get(&format!("/api/invoices/by-tx/{}/{}", chain_id, tx_hash))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::super::{RequestSpec, TestTransport};
    use super::*;
    use std::sync::{Arc, Mutex};

    // `get_checkout` calls `js_sys::encode_uri_component` unconditionally, and
    // `list_invoices`/`export_invoices_csv` call it on every `Some` string
    // filter. That binding panics ("cannot call wasm-bindgen imported
    // functions on non-wasm targets") the instant it runs outside an actual
    // wasm host, regardless of what transport answers the request underneath
    // it - so `get_checkout` cannot be driven through `cargo test` on this
    // workspace's host target at all, and the two list/export functions below
    // are only exercised with the `Some` filters that don't touch encoding.
    // Covering the encoding branches needs a `wasm-bindgen-test` run in a real
    // (or headless) browser, which this crate's test suite does not do today.

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

    fn sample_invoice_json(id: &str, store_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "store_id": store_id,
            "store_name": null,
            "currency": "USD",
            "status": "pending",
            "amount": "10.00",
            "amount_received": "0.00",
            "created_at": "2026-01-01T00:00:00Z",
            "expires_at": "2026-01-01T00:15:00Z",
            "metadata": null,
            "customer_email": null,
            "payment_options": [],
        })
    }

    fn sample_payment_json(id: &str, invoice_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "store_id": null,
            "store_name": null,
            "chain_id": "eip155:1",
            "invoice_id": invoice_id,
            "tx_hash": "0xabc",
            "amount": "1.0",
            "asset_symbol": "ETH",
            "token_address": null,
            "block_number": null,
            "from_address": null,
            "detected_at": "2026-01-01T00:00:00Z",
            "confirmed_at": null,
            "reorged": false,
            "decimals": 18,
        })
    }

    #[test]
    fn list_invoices_sends_only_the_filters_that_were_set() {
        // `store_id`/`status`/`currency`/`search` are left `None` here - see
        // the module-level note above on why.
        let (transport, recorded) = recording_transport(serde_json::json!({
            "total": 0,
            "invoices": [],
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.list_invoices(None, None, None, None, Some(20), Some(40)));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices?limit=20&offset=40".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn list_invoices_with_no_filters_sends_an_empty_query_string() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "total": 0,
            "invoices": [],
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.list_invoices(None, None, None, None, None, None));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices?".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn get_invoice_reads_the_given_invoice_from_the_invoices_collection() {
        let (transport, recorded) = recording_transport(sample_invoice_json("inv-1", "store-1"));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.get_invoice("inv-1"));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices/inv-1".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn get_invoice_does_not_carry_a_trailing_segment() {
        let (transport, recorded) = recording_transport(sample_invoice_json("inv-1", "store-1"));
        let client = ApiClient::with_test_transport("", transport);

        let _ = block_on(client.get_invoice("inv-1"));

        let path = recorded.lock().unwrap().clone().unwrap().path;
        assert!(!path.ends_with("/payments"));
        assert!(!path.ends_with("/status"));
    }

    #[test]
    fn create_invoice_posts_the_request_body_to_the_invoices_collection() {
        let (transport, recorded) = recording_transport(sample_invoice_json("inv-1", "store-1"));
        let client = ApiClient::with_test_transport("", transport);
        let req = CreateInvoiceRequest {
            store_id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            currency: "USD".to_string(),
            amount: "10.00".to_string(),
            expiration_seconds: None,
            metadata: None,
            customer_email: None,
            webhook_url: None,
            redirect_url: None,
        };

        let result = block_on(client.create_invoice(&req));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "POST",
                path: "/api/invoices".to_string(),
                // `expiration_seconds`/`metadata`/`customer_email` all carry
                // `skip_serializing_if = "Option::is_none"` server-side, so a
                // request that leaves them unset must omit them rather than
                // send explicit nulls.
                body: Some(serde_json::json!({
                    "store_id": "00000000-0000-0000-0000-000000000001",
                    "currency": "USD",
                    "amount": "10.00",
                })),
            }
        );
    }

    #[test]
    fn get_invoice_payments_reads_the_given_invoice_s_payments_subcollection() {
        let (transport, recorded) =
            recording_transport(serde_json::json!([sample_payment_json("pay-1", "inv-1")]));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.get_invoice_payments("inv-1"));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices/inv-1/payments".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn get_invoice_status_reads_the_given_invoice_s_status_subcollection() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "id": "inv-1",
            "status": "pending",
            "amount": "10.00",
            "amount_received": "0.00",
            "currency": "USD",
            "expires_at": "2026-01-01T00:15:00Z",
            "payment_count": 0,
            "confirmed_count": 0,
            "is_paid": false,
            "is_expired": false,
            "payment_options": [],
            "payments": [],
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.get_invoice_status("inv-1"));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices/inv-1/status".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn export_invoices_csv_with_no_filters_sends_an_empty_query_string() {
        // `store_id`/`status`/`search` are all string filters, so every
        // populated one routes through `js_sys::encode_uri_component` - see
        // the module-level note above on why only the all-`None` case runs
        // here.
        let (transport, recorded) =
            recording_transport(serde_json::Value::String("id,status\n".to_string()));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.export_invoices_csv(None, None, None));

        assert_eq!(result.unwrap(), "id,status\n");
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices/export.csv?".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn lookup_invoice_by_tx_places_chain_id_and_tx_hash_in_their_own_segments() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "invoice": sample_invoice_json("inv-1", "store-1"),
            "payment": sample_payment_json("pay-1", "inv-1"),
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.lookup_invoice_by_tx(1, "0xabc"));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices/by-tx/1/0xabc".to_string(),
                body: None,
            }
        );
    }
}
