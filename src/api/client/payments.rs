//! Payment API methods (store-scoped).

use super::{ApiClient, ApiError};
use crate::api::{Payment, PaymentListResponse};

impl ApiClient {
    /// List payments with filters and pagination.
    ///
    /// `store_id` of `None` means "All Stores": the server then returns
    /// payments across every store, which it only allows for server admins —
    /// any other caller gets `400 Bad Request`.
    ///
    /// `search` is a free-text term over tx hash, invoice id, asset symbol and
    /// sender, applied in SQL so `total` counts the same rows the page shows.
    pub async fn list_payments(
        &self,
        store_id: Option<&str>,
        status: Option<&str>,
        search: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<PaymentListResponse, ApiError> {
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
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        self.get(&format!("/api/payments?{}", params.join("&")))
            .await
    }

    /// Export payments as CSV text.
    ///
    /// `store_id` of `None` exports across all stores (admins only) — see
    /// [`Self::list_payments`].
    ///
    /// `search` is passed for the same reason `status` is: the export shares
    /// the list's filter builders server-side, and an export that ignores the
    /// search box hands the merchant rows they cannot see.
    pub async fn export_payments_csv(
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
        self.get_text(&format!("/api/payments/export.csv?{}", params.join("&")))
            .await
    }

    /// Get a single payment by ID.
    pub async fn get_payment(&self, id: &str) -> Result<Payment, ApiError> {
        self.get(&format!("/api/payments/{}", id)).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::{RequestSpec, TestTransport};
    use super::*;
    use std::sync::{Arc, Mutex};

    // `list_payments`/`export_payments_csv` call `js_sys::encode_uri_component`
    // on every `Some` string filter, and that binding panics ("cannot call
    // wasm-bindgen imported functions on non-wasm targets") the instant it
    // runs outside an actual wasm host, regardless of what transport answers
    // the request underneath it. So the two are only exercised here with the
    // filters that don't touch encoding; see the equivalent note in
    // `invoices.rs`'s tests.

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
    fn list_payments_sends_only_the_filters_that_were_set() {
        // `store_id`/`status`/`search` are left `None` here - see the
        // module-level note above on why.
        let (transport, recorded) = recording_transport(serde_json::json!({
            "total": 0,
            "payments": [],
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.list_payments(None, None, None, Some(20), Some(40)));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/payments?limit=20&offset=40".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn list_payments_with_no_filters_sends_an_empty_query_string() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "total": 0,
            "payments": [],
        }));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.list_payments(None, None, None, None, None));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/payments?".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn export_payments_csv_with_no_filters_sends_an_empty_query_string() {
        // `store_id`/`status`/`search` are all string filters, so every
        // populated one routes through `js_sys::encode_uri_component` - see
        // the module-level note above on why only the all-`None` case runs
        // here.
        let (transport, recorded) =
            recording_transport(serde_json::Value::String("id,status\n".to_string()));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.export_payments_csv(None, None, None));

        assert_eq!(result.unwrap(), "id,status\n");
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/payments/export.csv?".to_string(),
                body: None,
            }
        );
    }

    #[test]
    fn get_payment_reads_the_given_payment_from_the_payments_collection() {
        let (transport, recorded) = recording_transport(sample_payment_json("pay-1", "inv-1"));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.get_payment("pay-1"));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/payments/pay-1".to_string(),
                body: None,
            }
        );
    }
}
