//! Account standing, shown on every page.
//!
//! Email is optional, so a merchant may have no address on file and this is
//! the only place they can be told their plan is about to lapse, or has. It
//! renders nothing while things are fine, and nothing when the standing could
//! not be read: an error here must not look like a warning.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::api::{AccountStandingInfo, ApiClient, StandingState};

/// How often the standing is read again, in milliseconds. The shell outlives
/// every route, so a single read would show a stale remedy link for as long as
/// the tab stays open, and would go on saying "lapsed" after the merchant has
/// paid. A tab that was hidden is read again the moment it is shown, so the
/// interval is not what keeps a returning merchant's link fresh.
const REFRESH_MS: u32 = 60_000;

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
            "Your subscription lapsed (paid through {d} UTC). New invoices are refused until it is renewed."
        ),
        (true, None) => {
            "Your subscription has lapsed. New invoices are refused until it is renewed."
                .to_string()
        }
        (false, Some(d)) => format!(
            "Your subscription is paid through {d} UTC. Renew before then to keep creating invoices."
        ),
        (false, None) => {
            "Your subscription is close to lapsing. Renew to keep creating invoices.".to_string()
        }
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

/// What stays on screen after a read failed: the warning, but not the link. A
/// remedy link is only as good as the read that produced it, and a link kept
/// across a long outage may point at something that has since expired.
fn after_failed_read(previous: Option<AccountStandingInfo>) -> Option<AccountStandingInfo> {
    previous.map(|mut p| {
        p.checkout_url = None;
        p
    })
}

/// A banner for the merchant's own standing, mounted in the app shell.
#[component]
pub fn StandingBanner() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let (tick, set_tick) = signal(0u32);
    let handle: Rc<RefCell<Option<gloo_timers::callback::Interval>>> = Rc::new(RefCell::new(None));
    let handle_for_effect = handle.clone();
    Effect::new(move |_| {
        let interval = gloo_timers::callback::Interval::new(REFRESH_MS, move || {
            set_tick.update(|n| *n = n.wrapping_add(1));
        });
        *handle_for_effect.borrow_mut() = Some(interval);
    });
    // Dropped with the component: a timer that outlives its owner and writes a
    // disposed signal panics the whole app.
    let handle_for_cleanup = SendWrapper::new(handle);
    on_cleanup(move || {
        handle_for_cleanup.borrow_mut().take();
    });

    // Read again as soon as the tab is shown: a hidden tab skips its reads, so
    // a merchant returning after a long absence would otherwise see the old
    // answer, and its link, until the next tick.
    let listener: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        let doc = document.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            if !doc.hidden() {
                set_tick.update(|n| *n = n.wrapping_add(1));
            }
        });
        let _ = document
            .add_event_listener_with_callback("visibilitychange", closure.as_ref().unchecked_ref());
        *listener.borrow_mut() = Some(closure);
    }
    let listener_for_cleanup = SendWrapper::new(listener);
    on_cleanup(move || {
        if let Some(closure) = listener_for_cleanup.borrow_mut().take()
            && let Some(document) = web_sys::window().and_then(|w| w.document())
        {
            let _ = document.remove_event_listener_with_callback(
                "visibilitychange",
                closure.as_ref().unchecked_ref(),
            );
        }
    });

    // The last answer that was read. A failed read keeps the warning, minus
    // its link: a lapse warning must not vanish because one poll hit a network
    // blip, but a link must not outlive the read that vouched for it.
    let (current, set_current) = signal(None::<AccountStandingInfo>);
    Effect::new(move |_| {
        tick.track();
        // A hidden tab is nobody's presence; do not poll on its behalf.
        if web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| d.hidden())
        {
            return;
        }
        let client = api.get_untracked();
        leptos::task::spawn_local(async move {
            match client.get_account_standing().await {
                Ok(info) => set_current.set(Some(info)),
                Err(e) => {
                    set_current.update(|c| *c = after_failed_read(c.take()));
                    // Still silent in the UI, but not in the console.
                    web_sys::console::warn_1(
                        &format!("Could not read account standing: {e}").into(),
                    );
                }
            }
        });
    });

    view! {
        {move || {
            current
                .get()
                .as_ref()
                .and_then(notice)
                .map(|n| {
                    let class = if n.lapsed { "alert alert-error" } else { "alert alert-warning" };
                    view! {
                        <div role="status" class=class>
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
    fn a_failed_read_keeps_the_warning_but_drops_the_link() {
        let kept = after_failed_read(Some(info(
            StandingState::Lapsed,
            Some("https://pay.example/c"),
        )))
        .unwrap();
        assert_eq!(kept.state, StandingState::Lapsed);
        assert_eq!(kept.checkout_url, None);
        assert!(notice(&kept).unwrap().lapsed);
        assert_eq!(after_failed_read(None), None);
    }

    #[test]
    fn lapsed_without_a_date_still_says_so() {
        let mut i = info(StandingState::Lapsed, Some("https://pay.example/c"));
        i.paid_through = None;
        let n = notice(&i).unwrap();
        assert!(n.lapsed && n.message.contains("renewed"), "{}", n.message);
        assert!(n.link.is_some());
    }

    #[test]
    fn a_link_that_is_not_https_is_never_rendered() {
        let n = notice(&info(StandingState::Lapsed, Some("javascript:alert(1)"))).unwrap();
        assert_eq!(n.link, None);
    }
}
