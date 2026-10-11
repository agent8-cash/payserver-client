//! Where to send a merchant after they sign back in.
//!
//! The page they were on when the server ended their session is saved here and
//! read once by the sign-in route.

use ui_kit::hooks::use_storage::{get_local, remove_local, set_local};

const RETURN_TO_KEY: &str = "return_to";
const LOGIN_PATH: &str = "/login";

/// A path is only a valid destination if it stays on this origin and is not
/// the sign-in or registration screen itself, which would loop.
fn sanitize(path: &str) -> Option<&str> {
    let same_origin = path.starts_with('/') && !path.starts_with("//") && !path.contains('\\');
    let auth_screen = [LOGIN_PATH, "/register"].iter().any(|p| {
        path.strip_prefix(p)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['?', '#', '/']))
    });
    (same_origin && !auth_screen).then_some(path)
}

/// Remember `path` as the place to return to after sign-in.
pub(super) fn save(path: &str) {
    if let Some(path) = sanitize(path) {
        let _ = set_local(RETURN_TO_KEY, &path);
    }
}

/// Take the saved destination, defaulting to the dashboard.
pub(super) fn take() -> String {
    let saved = get_local::<String>(RETURN_TO_KEY);
    remove_local(RETURN_TO_KEY);
    saved
        .as_deref()
        .and_then(sanitize)
        .unwrap_or("/")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn keeps_same_origin_app_paths() {
        assert_eq!(
            sanitize("/evm/invoices?page=2"),
            Some("/evm/invoices?page=2")
        );
        assert_eq!(sanitize("/"), Some("/"));
    }

    #[test]
    fn refuses_other_origins() {
        assert_eq!(sanitize("//evil.example/x"), None);
        assert_eq!(sanitize("https://evil.example"), None);
        assert_eq!(sanitize("/\\evil.example"), None);
        assert_eq!(sanitize(""), None);
    }

    #[test]
    fn refuses_the_auth_screens() {
        assert_eq!(sanitize("/login"), None);
        assert_eq!(sanitize("/login?x=1"), None);
        assert_eq!(sanitize("/register"), None);
        assert_eq!(sanitize("/loginfoo"), Some("/loginfoo"));
    }
}
