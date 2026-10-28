//! Device management (`GET`/`DELETE /auth/devices`, mounted straight from
//! `payserver-commons`'s `auth` crate) and this server's own device
//! enrollment endpoint (`POST /users/me/devices`; see
//! `server/src/api/users/devices.rs` upstream), plus the session-authenticated
//! passkey ceremony that goes with it (`/auth/passkey/register/start` and
//! `/complete`, also from the `auth` crate). `ui-kit`'s own API client has no
//! wrapper for either of those last two - only new-user registration and
//! recovery - so they are added here instead of reused.
//!
//! `/auth/devices` and `/auth/devices/{id}` take the session id as a JSON
//! body field rather than reading the `Authorization` header the way
//! `/auth/me` does - both `list_devices` and `revoke_device` are defined
//! that way server-side (`auth::api::management`), so this mirrors that
//! shape instead of guessing at a header contract those two routes do not
//! implement.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ui_kit::auth::types::{DeviceType, EncryptedBlob, KdfParams};

use super::{ApiClient, ApiError};

/// Hand-mirrors `auth::DeviceInfo`: that type lives in a server-side crate
/// this client does not depend on (see `WalletCredentialResponse` for the
/// same cross-repository reasoning), so this defines a matching shape for
/// deserializing the response. Deliberately sanitized upstream - never
/// carries the wrapped key.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceInfo {
    pub id: uuid::Uuid,
    pub name: String,
    pub device_type: DeviceType,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Serialize)]
struct AuthenticatedBody<'a> {
    session_id: &'a str,
}

#[derive(Serialize)]
struct StartAddPasskeyRequest<'a> {
    session_id: &'a str,
    passkey_name: &'a str,
}

#[derive(Serialize)]
struct CompleteAddPasskeyRequest<'a> {
    session_id: &'a str,
    passkey_name: &'a str,
    credential: serde_json::Value,
}

#[derive(Serialize)]
struct EnrollDeviceRequest {
    device_name: String,
    device_type: DeviceType,
    encrypted_symmetric_key: EncryptedBlob,
    kdf_params: KdfParams,
}

impl ApiClient {
    /// `GET /auth/devices` - every active device on the account.
    pub async fn list_devices(&self, session_id: &str) -> Result<Vec<DeviceInfo>, ApiError> {
        self.get_with_body("/api/auth/devices", &AuthenticatedBody { session_id })
            .await
    }

    /// `DELETE /auth/devices/{id}` - revoke a device other than the one
    /// making this request (the server refuses that case specifically).
    pub async fn revoke_device(
        &self,
        session_id: &str,
        device_id: uuid::Uuid,
    ) -> Result<(), ApiError> {
        self.delete_with_body(
            &format!("/api/auth/devices/{device_id}"),
            &AuthenticatedBody { session_id },
        )
        .await
    }

    /// `POST /auth/passkey/register/start` - begin adding a second passkey
    /// to the already-authenticated account. Returns the WebAuthn creation
    /// options to hand to `ui_kit::auth::webauthn::create_credential`.
    pub async fn start_add_passkey(
        &self,
        session_id: &str,
        passkey_name: &str,
    ) -> Result<serde_json::Value, ApiError> {
        #[derive(Deserialize)]
        struct Resp {
            options: serde_json::Value,
        }
        let resp: Resp = self
            .post(
                "/api/auth/passkey/register/start",
                &StartAddPasskeyRequest {
                    session_id,
                    passkey_name,
                },
            )
            .await?;
        Ok(resp.options)
    }

    /// `POST /auth/passkey/register/complete` - finish adding the passkey
    /// `start_add_passkey` began.
    pub async fn complete_add_passkey(
        &self,
        session_id: &str,
        passkey_name: &str,
        credential: serde_json::Value,
    ) -> Result<(), ApiError> {
        self.post::<serde_json::Value, _>(
            "/api/auth/passkey/register/complete",
            &CompleteAddPasskeyRequest {
                session_id,
                passkey_name,
                credential,
            },
        )
        .await?;
        Ok(())
    }

    /// `POST /users/me/devices` - record the `devices` row for a passkey
    /// `start_add_passkey`/`complete_add_passkey` just registered. Requires
    /// a *fresh* session (`FreshlyAuthenticatedUser` server-side): a 401 here
    /// specifically means re-authenticate and retry, not that the session
    /// is gone.
    pub async fn enroll_device(
        &self,
        device_name: &str,
        device_type: DeviceType,
        encrypted_symmetric_key: EncryptedBlob,
        kdf_params: KdfParams,
    ) -> Result<DeviceInfo, ApiError> {
        self.post(
            "/api/users/me/devices",
            &EnrollDeviceRequest {
                device_name: device_name.to_string(),
                device_type,
                encrypted_symmetric_key,
                kdf_params,
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::super::{RequestSpec, TestTransport};
    use super::*;
    use std::sync::{Arc, Mutex};

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
    fn list_devices_sends_the_session_id_as_a_get_body_not_a_header_claim() {
        let (transport, recorded) = recording_transport(serde_json::json!([{
            "id": "11111111-1111-1111-1111-111111111111",
            "name": "Laptop",
            "device_type": "browser",
            "created_at": "2026-01-01T00:00:00Z",
            "last_used_at": null,
            "is_active": true,
        }]));
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.list_devices("session-abc"));
        let devices = result.expect("list_devices");

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Laptop");
        assert!(matches!(devices[0].device_type, DeviceType::Browser));
        assert!(devices[0].is_active);

        let spec = recorded.lock().unwrap().clone().unwrap();
        assert_eq!(spec.method, "GET");
        assert_eq!(spec.path, "/api/auth/devices");
        assert_eq!(
            spec.body,
            Some(serde_json::json!({"session_id": "session-abc"}))
        );
    }

    #[test]
    fn revoke_device_deletes_with_the_session_id_in_the_body() {
        let (transport, recorded) = recording_transport(serde_json::json!(null));
        let client = ApiClient::with_test_transport("", transport);

        let id = uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        block_on(client.revoke_device("session-abc", id)).expect("revoke_device");

        let spec = recorded.lock().unwrap().clone().unwrap();
        assert_eq!(spec.method, "DELETE");
        assert_eq!(
            spec.path,
            "/api/auth/devices/22222222-2222-2222-2222-222222222222"
        );
        assert_eq!(
            spec.body,
            Some(serde_json::json!({"session_id": "session-abc"}))
        );
    }

    #[test]
    fn enroll_device_posts_the_wrapped_blob_verbatim_and_never_a_plaintext_key() {
        let (transport, recorded) = recording_transport(serde_json::json!({
            "id": "33333333-3333-3333-3333-333333333333",
            "name": "New laptop",
            "device_type": "browser",
            "created_at": "2026-01-01T00:00:00Z",
            "last_used_at": null,
            "is_active": true,
        }));
        let client = ApiClient::with_test_transport("", transport);

        let blob = EncryptedBlob {
            ciphertext: "Y2lwaGVy".to_string(),
            iv: "aXY=".to_string(),
            mac: "bWFj".to_string(),
        };
        let params = KdfParams {
            algorithm: "argon2id".to_string(),
            memory_kb: 65536,
            iterations: 3,
            parallelism: 4,
            salt: "c2FsdA==".to_string(),
        };

        let result = block_on(client.enroll_device(
            "New laptop",
            DeviceType::Browser,
            blob.clone(),
            params.clone(),
        ));
        assert_eq!(result.unwrap().name, "New laptop");

        let spec = recorded.lock().unwrap().clone().unwrap();
        assert_eq!(spec.method, "POST");
        assert_eq!(spec.path, "/api/users/me/devices");
        let body = spec.body.unwrap();
        // Opaque fields only, exactly as supplied - this request is the one
        // place a client-held key ever crosses the wire, and it must never
        // be anything but the pre-wrapped blob.
        assert_eq!(body["encrypted_symmetric_key"]["ciphertext"], "Y2lwaGVy");
        assert_eq!(body["kdf_params"]["algorithm"], "argon2id");
        assert_eq!(body["device_name"], "New laptop");
    }
}
