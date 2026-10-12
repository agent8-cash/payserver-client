//! Account standing API method.

use super::{ApiClient, ApiError};
use crate::api::AccountStandingInfo;

impl ApiClient {
    /// Fetch the signed-in merchant's own account standing.
    pub async fn get_account_standing(&self) -> Result<AccountStandingInfo, ApiError> {
        self.get("/api/users/me/standing").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::StandingState;

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        let mut fut = std::pin::pin!(fut);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        loop {
            if let std::task::Poll::Ready(value) = fut.as_mut().poll(&mut cx) {
                return value;
            }
        }
    }

    #[test]
    fn reads_the_standing_from_the_merchant_endpoint() {
        let client = ApiClient::with_test_transport(
            "",
            std::sync::Arc::new(|spec| {
                assert_eq!(spec.path, "/api/users/me/standing");
                Ok(serde_json::json!({
                    "state": "expiring",
                    "plan_name": "p",
                    "paid_through": "2026-10-12T00:00:00Z",
                    "checkout_url": "https://pay.example/c"
                }))
            }),
        );
        let got = block_on(client.get_account_standing()).unwrap();
        assert_eq!(got.state, StandingState::Expiring);
        assert_eq!(got.checkout_url.as_deref(), Some("https://pay.example/c"));
    }
}
