//! Account standing, shown on every page.
//!
//! Email is optional, so a merchant may have no address on file and this is
//! the only place they can be told their plan is about to lapse, or has. It
//! renders nothing while things are fine, and nothing when the standing could
//! not be read: an error here must not look like a warning.

use leptos::prelude::*;

use crate::api::{AccountStandingInfo, ApiClient, StandingState};

/// What to say, separated from rendering so it can be tested.
#[derive(Debug, PartialEq, Eq)]
struct Notice {
    lapsed: bool,
    message: String,
    /// Only ever an `https` URL.
    link: Option<String>,
}

fn notice(info: &AccountStandingInfo) -> Option<Notice> {
    let lapsed = match info.state {
        StandingState::Unknown | StandingState::Good => return None,
        StandingState::Expiring => false,
        StandingState::Lapsed => true,
    };
    // The server sends an RFC 3339 timestamp; the date is what a merchant needs.
    let date = info
        .paid_through
        .as_deref()
        .and_then(|t| t.get(..10))
        .map(str::to_string);
    let message = match (lapsed, date) {
        (true, Some(d)) => format!(
            "Your subscription lapsed (paid through {d} UTC). New invoices are refused until it is renewed; payments to existing invoices still arrive."
        ),
        (true, None) => "Your subscription has lapsed. New invoices are refused until it is renewed; payments to existing invoices still arrive.".to_string(),
        (false, Some(d)) => format!(
            "Your subscription is paid through {d} UTC. Renew before then to keep creating invoices."
        ),
        (false, None) => "Your subscription is close to lapsing. Renew to keep creating invoices.".to_string(),
    };
    let link = info
        .checkout_url
        .as_deref()
        .filter(|u| u.starts_with("https://"))
        .map(str::to_string);
    Some(Notice {
        lapsed,
        message,
        link,
    })
}

/// A banner for the merchant's own standing, mounted in the app shell.
#[component]
pub fn StandingBanner() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let standing = LocalResource::new(move || {
        let client = api.get();
        async move {
            match client.get_account_standing().await {
                Ok(info) => Some(info),
                Err(e) => {
                    // Still silent in the UI, but not in the console: a failed
                    // read is not the same answer as "all good".
                    web_sys::console::warn_1(
                        &format!("Could not read account standing: {e}").into(),
                    );
                    None
                }
            }
        }
    });

    view! {
        {move || {
            standing
                .get()
                .and_then(|s| s.take())
                .as_ref()
                .and_then(notice)
                .map(|n| {
                    let class = if n.lapsed { "alert alert-error" } else { "alert alert-warning" };
                    view! {
                        <div class=class role="status">
                            <strong>{if n.lapsed { "Subscription lapsed" } else { "Subscription ending soon" }}</strong>
                            <p>{n.message}</p>
                            {n.link.map(|href| view! {
                                <p><a class="form-alert-action" href=href rel="noopener noreferrer">"Renew now"</a></p>
                            })}
                        </div>
                    }
                })
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(state: StandingState, url: Option<&str>) -> AccountStandingInfo {
        AccountStandingInfo {
            state,
            plan_name: Some("p".into()),
            paid_through: Some("2026-10-12T00:00:00Z".into()),
            checkout_url: url.map(str::to_string),
        }
    }

    #[test]
    fn healthy_and_unknown_standings_show_nothing() {
        assert_eq!(notice(&info(StandingState::Good, None)), None);
        assert_eq!(notice(&info(StandingState::Unknown, None)), None);
    }

    #[test]
    fn expiring_warns_with_the_date_before_any_refusal() {
        let n = notice(&info(StandingState::Expiring, None)).unwrap();
        assert!(!n.lapsed);
        assert!(n.message.contains("2026-10-12"), "{}", n.message);
    }

    #[test]
    fn lapsed_names_the_remedy_link() {
        let n = notice(&info(StandingState::Lapsed, Some("https://pay.example/c"))).unwrap();
        assert!(n.lapsed);
        assert_eq!(n.link.as_deref(), Some("https://pay.example/c"));
    }

    #[test]
    fn a_link_that_is_not_https_is_never_rendered() {
        let n = notice(&info(StandingState::Lapsed, Some("javascript:alert(1)"))).unwrap();
        assert_eq!(n.link, None);
    }
}
