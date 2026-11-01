//! Pure helpers for the create-invoice form: metadata assembly, URL checks and
//! the no-wallet error classifier.

use crate::api::ApiError;

/// Build metadata JSON from optional order_id and buyer_email fields.
pub(super) fn build_metadata(order_id: &str, buyer_email: &str) -> Option<serde_json::Value> {
    if order_id.is_empty() && buyer_email.is_empty() {
        return None;
    }
    let mut map = serde_json::Map::new();
    if !order_id.is_empty() {
        map.insert(
            "order_id".to_string(),
            serde_json::Value::String(order_id.to_string()),
        );
    }
    if !buyer_email.is_empty() {
        map.insert(
            "buyer_email".to_string(),
            serde_json::Value::String(buyer_email.to_string()),
        );
    }
    Some(serde_json::Value::Object(map))
}

/// Basic URL validation: must start with http:// or https:// followed by content.
pub(super) fn is_valid_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("http://") && url.len() > "http://".len())
        || (lower.starts_with("https://") && url.len() > "https://".len())
}

/// Whether a failed create-invoice call is the no-wallet gate, not some
/// other 400.
///
/// `ApiError::Http` carries the response body verbatim rather than a parsed
/// field, so this parses just enough JSON to read the `error` code the server
/// sends (`server/src/api/invoices/crud.rs`). A previous version of this modal
/// treated any 400 the same way - this reads the code specifically so a
/// different 400 (bad amount, no payment methods, ...) is never mistaken for
/// the wallet gate and shown a link that would not fix it.
pub(super) fn is_no_wallet_error(err: &ApiError) -> bool {
    let ApiError::Http {
        status: 400,
        message,
    } = err
    else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(message)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .is_some_and(|code| code == "no_wallet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_metadata_both_fields() {
        let meta = build_metadata("ORD-123", "buyer@example.com");
        let meta = meta.expect("should be Some");
        assert_eq!(meta["order_id"], "ORD-123");
        assert_eq!(meta["buyer_email"], "buyer@example.com");
    }

    #[test]
    fn test_build_metadata_order_id_only() {
        let meta = build_metadata("ORD-456", "");
        let meta = meta.expect("should be Some");
        assert_eq!(meta["order_id"], "ORD-456");
        assert!(meta.get("buyer_email").is_none());
    }

    #[test]
    fn test_build_metadata_email_only() {
        let meta = build_metadata("", "test@example.com");
        let meta = meta.expect("should be Some");
        assert!(meta.get("order_id").is_none());
        assert_eq!(meta["buyer_email"], "test@example.com");
    }

    #[test]
    fn test_build_metadata_empty() {
        assert!(build_metadata("", "").is_none());
    }

    #[test]
    fn test_is_valid_url_accepts_http_and_https() {
        assert!(is_valid_url("https://example.com/webhook"));
        assert!(is_valid_url("http://localhost:3000/callback"));
        assert!(is_valid_url("https://api.example.com/v1/notify?token=abc"));
        assert!(is_valid_url("http://192.168.1.1:8080/hook"));
        assert!(is_valid_url(
            "https://example.com/path/to/resource#fragment"
        ));
    }

    #[test]
    fn test_is_valid_url_rejects_non_http_schemes() {
        assert!(!is_valid_url("ftp://files.example.com"));
        assert!(!is_valid_url("ws://example.com/ws"));
        assert!(!is_valid_url("wss://example.com/ws"));
        assert!(!is_valid_url("javascript:alert(1)"));
        assert!(!is_valid_url("data:text/html,<h1>hi</h1>"));
        assert!(!is_valid_url("file:///etc/passwd"));
    }

    #[test]
    fn test_is_valid_url_rejects_invalid_input() {
        assert!(!is_valid_url(""));
        assert!(!is_valid_url("not-a-url"));
        assert!(!is_valid_url("example.com"));
        assert!(!is_valid_url(" https://example.com"));
        assert!(!is_valid_url("http://"));
        assert!(!is_valid_url("https://"));
    }

    #[test]
    fn test_is_valid_url_accepts_case_insensitive_scheme() {
        assert!(is_valid_url("HTTP://EXAMPLE.COM"));
        assert!(is_valid_url("Https://Example.Com/path"));
    }

    #[test]
    fn recognizes_the_wallet_gate_by_its_error_code() {
        let err = ApiError::Http {
            status: 400,
            message: r#"{"error":"no_wallet","message":"This store has no wallet"}"#.to_string(),
        };
        assert!(is_no_wallet_error(&err));
    }

    #[test]
    fn a_different_400_is_not_mistaken_for_the_wallet_gate() {
        let err = ApiError::Http {
            status: 400,
            message:
                r#"{"error":"no_payment_methods","message":"Store has no enabled payment methods"}"#
                    .to_string(),
        };
        assert!(!is_no_wallet_error(&err));
    }

    #[test]
    fn a_non_400_is_never_the_wallet_gate_even_with_the_same_code() {
        let err = ApiError::Http {
            status: 409,
            message: r#"{"error":"no_wallet","message":"..."}"#.to_string(),
        };
        assert!(!is_no_wallet_error(&err));
    }

    #[test]
    fn a_non_json_body_is_not_the_wallet_gate() {
        let err = ApiError::Http {
            status: 400,
            message: "invalid status filter".to_string(),
        };
        assert!(!is_no_wallet_error(&err));
    }
}
