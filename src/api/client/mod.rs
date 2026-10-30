//! HTTP client for the payserver API.
//!
//! Every method in this module's six files reaches the network only through
//! `get`/`get_text`/`post`/`put`/`patch`/`delete` (plus the `_empty` variants)
//! below, which call into `gloo-net` and panic the instant a request is built
//! outside an actual wasm host - so `cargo test` cannot observe the method,
//! path, or body any of them send. That is true of all 52 `pub async fn`s
//! here (15 `admin`, 1 `health`, 8 `invoices`, 3 `payments`, 2 `plugins`, 23
//! `stores`), not just the wallet calls below: it is this module's
//! structural default, not drift in a few recent paths. `get`/`post`/`patch`/
//! `delete` now carry a `#[cfg(test)]` seam (`TestTransport`) so calls built
//! entirely on those - `create_wallet` (POST), `update_wallet` (PATCH),
//! `delete_wallet` (DELETE), `export_wallet_xpub` (GET) (see `stores`
//! tests) - none of the four is a `put` call, so all four are already inside
//! the seam's coverage, not outside it - and the `invoices`/`payments` reads
//! and writes, run for real under `cargo test`. `put`, the `_empty`
//! variants, and anything that calls `build_request` directly (`logout`)
//! still cannot be exercised past their own pure helpers without the same
//! seam extended further, or a `wasm-bindgen-test` harness (already a
//! dev-dependency, unused in this crate's test runs). Separately, any call
//! that reaches `js_sys::*` (URL component encoding) panics the same way on
//! a non-wasm host regardless of transport - the seam here does not help
//! those branches; see the `invoices`/`payments` tests for which branches
//! that leaves untested.

use std::{cell::RefCell, rc::Rc};

use gloo_net::http::{Request, RequestBuilder};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

mod admin;

type SessionEndedHandler = Rc<dyn Fn()>;

thread_local! {
    static ON_SESSION_ENDED: RefCell<Option<SessionEndedHandler>> = const { RefCell::new(None) };
}

/// Register the one reaction to the server rejecting the session; `None` removes it.
///
/// A plain callback rather than a signal so that firing it from a 401 reads
/// and writes nothing reactive on the caller's side.
pub fn set_session_ended_handler(handler: Option<SessionEndedHandler>) {
    ON_SESSION_ENDED.with(|h| *h.borrow_mut() = handler);
}

/// Every 401 from this client funnels through here: it tells the registered
/// handler the session is over, and still returns the typed error so the
/// calling page's own failure handling is unchanged.
fn session_ended() -> ApiError {
    // Clone out first so a handler that re-enters cannot hit a borrowed cell.
    let handler = ON_SESSION_ENDED.with(|h| h.borrow().clone());
    if let Some(handler) = handler {
        handler();
    }
    ApiError::Unauthorized
}
mod devices;
mod health;
mod invoices;
mod payments;
mod plugins;
mod standing;
mod stores;

pub use admin::{EmailStatus, SafeModeStatus};
pub use devices::DeviceInfo;

/// API client errors.
#[derive(Error, Debug, Clone)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("HTTP error {status}: {message}")]
    Http { status: u16, message: String },
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Unauthorized")]
    Unauthorized,
}

/// A request as `post`/`patch`/`delete` are about to hand it off - method,
/// path, and (if any) body - captured so a test can assert on the object
/// actually produced by a call like `create_wallet`, not just on a pure
/// helper around it. Test-only: nothing in the real request path outside
/// `#[cfg(test)]` ever constructs one.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RequestSpec {
    pub method: &'static str,
    pub path: String,
    pub body: Option<serde_json::Value>,
}

/// A test double for the transport, standing in for `gloo-net`. `gloo-net`
/// calls into `wasm-bindgen`-imported bindings the instant a request is
/// built, which panics ("cannot call wasm-bindgen imported functions on
/// non-wasm targets") outside an actual wasm host - so this is the only way
/// to run `create_wallet`/`update_wallet`/`delete_wallet` themselves, rather
/// than a helper extracted from them, under `cargo test`.
#[cfg(test)]
pub(crate) type TestTransport =
    std::sync::Arc<dyn Fn(RequestSpec) -> Result<serde_json::Value, ApiError> + Send + Sync>;

/// API client for a payserver.
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    token: Option<String>,
    #[cfg(test)]
    test_transport: Option<TestTransport>,
}

impl ApiClient {
    /// Create a new API client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            token: None,
            #[cfg(test)]
            test_transport: None,
        }
    }

    /// Create a client whose `post`/`patch`/`delete` calls are answered by
    /// `transport` instead of `gloo-net`, so the async function itself -
    /// including the request it builds - runs under `cargo test`.
    #[cfg(test)]
    pub(crate) fn with_test_transport(
        base_url: impl Into<String>,
        transport: TestTransport,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            token: None,
            test_transport: Some(transport),
        }
    }

    /// Create a client for public endpoints (no auth header sent).
    ///
    /// Uses a same-origin relative base URL. Call `with_token` to authenticate later.
    pub fn unauthenticated() -> Self {
        Self::new("")
    }

    /// Set the authorization token.
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token;
        self
    }

    /// Build a request with authentication.
    fn build_request(&self, method: &str, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let builder = match method {
            "GET" => Request::get(&url),
            "POST" => Request::post(&url),
            "PUT" => Request::put(&url),
            "DELETE" => Request::delete(&url),
            "PATCH" => Request::patch(&url),
            _ => Request::get(&url),
        };

        let builder = if let Some(ref token) = self.token {
            builder.header("Authorization", &format!("Bearer {}", token))
        } else {
            builder
        };

        builder.header("Content-Type", "application/json")
    }

    /// Make a GET request.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            let response = transport(RequestSpec {
                method: "GET",
                path: path.to_string(),
                body: None,
            })?;
            return serde_json::from_value(response).map_err(|e| ApiError::Parse(e.to_string()));
        }

        let request = self
            .build_request("GET", path)
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a GET request with a JSON body.
    ///
    /// Not RFC-idiomatic, but required by the two `auth` crate endpoints
    /// (`/auth/devices`, `/auth/passkeys`) that take the session id as a body
    /// field instead of the `Authorization` header - see
    /// `api::client::devices` for why.
    async fn get_with_body<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            let body = serde_json::to_value(body).map_err(|e| ApiError::Parse(e.to_string()))?;
            let response = transport(RequestSpec {
                method: "GET",
                path: path.to_string(),
                body: Some(body),
            })?;
            return serde_json::from_value(response).map_err(|e| ApiError::Parse(e.to_string()));
        }

        let request = self
            .build_request("GET", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a DELETE request with a JSON body - same reasoning as
    /// [`Self::get_with_body`].
    async fn delete_with_body<B: Serialize>(&self, path: &str, body: &B) -> Result<(), ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            let body = serde_json::to_value(body).map_err(|e| ApiError::Parse(e.to_string()))?;
            transport(RequestSpec {
                method: "DELETE",
                path: path.to_string(),
                body: Some(body),
            })?;
            return Ok(());
        }

        let request = self
            .build_request("DELETE", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(session_ended());
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

    /// Make a GET request returning raw text (for CSV downloads).
    async fn get_text(&self, path: &str) -> Result<String, ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            let response = transport(RequestSpec {
                method: "GET",
                path: path.to_string(),
                body: None,
            })?;
            return match response {
                serde_json::Value::String(s) => Ok(s),
                other => Err(ApiError::Parse(format!(
                    "test transport: expected a string response for get_text, got {other}"
                ))),
            };
        }

        let request = self
            .build_request("GET", path)
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(session_ended());
        }

        if !response.ok() {
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::Http {
                status: response.status(),
                message,
            });
        }

        response
            .text()
            .await
            .map_err(|e| ApiError::Parse(e.to_string()))
    }

    /// Make a POST request.
    async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            let body = serde_json::to_value(body).map_err(|e| ApiError::Parse(e.to_string()))?;
            let response = transport(RequestSpec {
                method: "POST",
                path: path.to_string(),
                body: Some(body),
            })?;
            return serde_json::from_value(response).map_err(|e| ApiError::Parse(e.to_string()));
        }

        let request = self
            .build_request("POST", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a PUT request.
    async fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let request = self
            .build_request("PUT", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a PATCH request.
    async fn patch<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            let body = serde_json::to_value(body).map_err(|e| ApiError::Parse(e.to_string()))?;
            let response = transport(RequestSpec {
                method: "PATCH",
                path: path.to_string(),
                body: Some(body),
            })?;
            return serde_json::from_value(response).map_err(|e| ApiError::Parse(e.to_string()));
        }

        let request = self
            .build_request("PATCH", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a DELETE request.
    async fn delete(&self, path: &str) -> Result<(), ApiError> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            transport(RequestSpec {
                method: "DELETE",
                path: path.to_string(),
                body: None,
            })?;
            return Ok(());
        }

        let request = self
            .build_request("DELETE", path)
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(session_ended());
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

    /// Make a POST request without a body, returning parsed JSON.
    async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let request = self
            .build_request("POST", path)
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Make a POST request without a body, ignoring response body.
    async fn post_empty_body(&self, path: &str) -> Result<(), ApiError> {
        let request = self
            .build_request("POST", path)
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(session_ended());
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

    /// Make a PATCH request with body, ignoring response body.
    async fn patch_empty<B: Serialize>(&self, path: &str, body: &B) -> Result<(), ApiError> {
        let request = self
            .build_request("PATCH", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(session_ended());
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

    /// Make a PUT request with body, ignoring response body.
    async fn put_empty<B: Serialize>(&self, path: &str, body: &B) -> Result<(), ApiError> {
        let request = self
            .build_request("PUT", path)
            .json(body)
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(session_ended());
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

    /// Handle response and parse JSON.
    async fn handle_response<T: DeserializeOwned>(
        &self,
        response: gloo_net::http::Response,
    ) -> Result<T, ApiError> {
        if response.status() == 401 {
            return Err(session_ended());
        }

        if !response.ok() {
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::Http {
                status: response.status(),
                message,
            });
        }

        response
            .json()
            .await
            .map_err(|e| ApiError::Parse(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ended_notifies_handler_and_returns_typed_error() {
        let fired = Rc::new(std::cell::Cell::new(0));
        let seen = fired.clone();
        set_session_ended_handler(Some(Rc::new(move || seen.set(seen.get() + 1))));
        assert!(matches!(session_ended(), ApiError::Unauthorized));
        assert_eq!(fired.get(), 1);

        set_session_ended_handler(None);
        assert!(matches!(session_ended(), ApiError::Unauthorized));
        assert_eq!(fired.get(), 1, "a removed handler must not fire");
    }

    #[test]
    fn test_api_client_new() {
        let client = ApiClient::new("http://localhost:5000");
        assert_eq!(client.base_url, "http://localhost:5000");
        assert_eq!(client.token, None);
    }

    #[test]
    fn test_api_client_with_token() {
        let client =
            ApiClient::new("http://localhost:5000").with_token(Some("test-token".to_string()));

        assert_eq!(client.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::Http {
            status: 404,
            message: "Not Found".to_string(),
        };
        assert_eq!(err.to_string(), "HTTP error 404: Not Found");

        let err = ApiError::Unauthorized;
        assert_eq!(err.to_string(), "Unauthorized");
    }
}
