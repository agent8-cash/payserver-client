//! Notifications settings tab: which events reach which channel, per store.
//!
//! This tab used to offer four account-wide toggles — "Payment notifications",
//! "Invoice updates", "Security alerts", "Product updates" — bound to nothing
//! and saved by nothing. The vocabulary was the deeper problem: the server has
//! no account-wide preference store and no notion of a "security alert", so
//! there was nothing to wire those toggles to. What it has is per-store
//! `store_settings.notification_prefs`, keyed by event, with a flag per
//! channel. This tab is drawn against that instead.
//!
//! "Product updates" has no successor here on purpose. It is not an event the
//! server emits, it is a marketing mailing list — which needs a list, consent
//! records and an unsubscribe path before a toggle over it means anything.
//! Carrying it across as a checkbox that saves nothing, or one that saves a
//! key nothing reads, is the same defect this tab is being rebuilt to fix.

use leptos::prelude::*;
use serde_json::{Value, json};

use crate::api::{ApiClient, ApiError, EmailStatus, UpdateStoreSettingsRequest};
use crate::app::StoreContext;
use crate::components::NoStoreSelected;

/// What the email channel does for an event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EmailChannel {
    /// The server sends no mail for this event, so there is no cell to fill.
    Unused,
    /// The customer payment receipt, gated by [`CUSTOMER_RECEIPTS_KEY`].
    CustomerReceipt,
}

/// Shown when the server reports it cannot send email. Email preferences would
/// save but never deliver, so the tab says so instead of offering them.
const EMAIL_UNAVAILABLE_NOTICE: &str = "This server has no outgoing email configured, so no \
     email notifications or receipts will be sent. Webhooks are unaffected.";

/// Shown when the status question itself failed, so the tab cannot say either
/// way. A failed answer is not a "configured" answer.
const EMAIL_UNKNOWN_NOTICE: &str = "Could not check whether this server can send email. \
     Email notifications and receipts may not be delivered.";

/// What the tab knows about the server's outgoing email.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmailAvailability {
    /// The question is still in flight.
    Checking,
    Configured,
    NotConfigured,
    /// The question failed (network, auth, a server without the route).
    Unknown,
}

impl EmailAvailability {
    /// Map the server's answer to the status question. `None` is the request
    /// still in flight; an error is never read as an answer.
    fn from_status(answer: Option<&Result<EmailStatus, ApiError>>) -> Self {
        match answer {
            None => Self::Checking,
            Some(Ok(s)) if s.configured => Self::Configured,
            Some(Ok(_)) => Self::NotConfigured,
            Some(Err(_)) => Self::Unknown,
        }
    }

    /// Whether the email channel is drawn as a working control. Only a server
    /// that has said it cannot send mail removes it; while the answer is
    /// pending or failed the control stays, since hiding it on a guess would
    /// break a working server.
    fn channel_offered(self) -> bool {
        self != Self::NotConfigured
    }

    /// The banner for this state, if any.
    fn notice(self) -> Option<&'static str> {
        match self {
            Self::NotConfigured => Some(EMAIL_UNAVAILABLE_NOTICE),
            Self::Unknown => Some(EMAIL_UNKNOWN_NOTICE),
            Self::Checking | Self::Configured => None,
        }
    }
}

/// One row of the matrix.
struct NotificationEvent {
    /// The `notification_prefs` key, and what the webhook dispatcher looks up.
    key: &'static str,
    label: &'static str,
    description: &'static str,
    email: EmailChannel,
}

/// The events, in the order an invoice moves through them.
///
/// This list must stay equal to `VALID_NOTIFICATION_EVENTS` in
/// `server/src/api/stores/settings.rs`: a key invented here is a 400 on save,
/// and one left out is a webhook the merchant cannot switch off.
const NOTIFICATION_EVENTS: [NotificationEvent; 5] = [
    NotificationEvent {
        key: "payment_detected",
        label: "Payment detected",
        description: "A payment has arrived on-chain but is not yet confirmed",
        email: EmailChannel::Unused,
    },
    NotificationEvent {
        key: "payment_confirmed",
        label: "Payment confirmed",
        description: "A payment reached the required confirmations",
        email: EmailChannel::CustomerReceipt,
    },
    NotificationEvent {
        key: "invoice_expired",
        label: "Invoice expired",
        description: "An invoice passed its expiry without being paid",
        email: EmailChannel::Unused,
    },
    NotificationEvent {
        key: "invoice_cancelled",
        label: "Invoice cancelled",
        description: "An invoice was cancelled before it was paid",
        email: EmailChannel::Unused,
    },
    NotificationEvent {
        key: "late_paid",
        label: "Paid late",
        description: "A payment arrived after the invoice had expired",
        email: EmailChannel::Unused,
    },
];

/// The prefs key gating the customer receipt email.
const CUSTOMER_RECEIPTS_KEY: &str = "customer_receipts_enabled";

/// Whether the webhook for `event` is on, per the store's prefs blob.
///
/// Absent means on, matching the dispatcher: `webhook_dispatch.rs` suppresses
/// only on an explicit `false`, so a store that has never saved this tab still
/// gets every webhook. Reading a missing key as "off" here would draw the
/// matrix as the exact inverse of what the server does.
fn webhook_enabled(prefs: &Value, event: &str) -> bool {
    prefs.get(event).and_then(|e| e.get("webhook")) != Some(&Value::Bool(false))
}

/// Whether customer receipt emails are on for the store.
///
/// Same "absent means on" rule, from `confirmation_handler.rs`.
fn customer_receipts_enabled(prefs: &Value) -> bool {
    prefs.get(CUSTOMER_RECEIPTS_KEY) != Some(&Value::Bool(false))
}

/// The whole matrix read out of a prefs blob, in [`NOTIFICATION_EVENTS`] order.
fn read_matrix(prefs: &Value) -> [bool; NOTIFICATION_EVENTS.len()] {
    let mut cells = [true; NOTIFICATION_EVENTS.len()];
    for (cell, event) in cells.iter_mut().zip(NOTIFICATION_EVENTS.iter()) {
        *cell = webhook_enabled(prefs, event.key);
    }
    cells
}

/// The `notification_prefs` blob to PATCH for a given matrix.
///
/// Carries `customer_receipts_enabled` alongside the five event keys. It used
/// to be omitted, because the validator rejected any top-level key outside
/// `VALID_NOTIFICATION_EVENTS` and including it made every save a 400 — and
/// since the server replaced the blob rather than merging, omitting it *removed*
/// it, which reads as enabled. Saving one preference therefore switched customer
/// receipt emails back on. The server now accepts the key and merges, so it is
/// sent explicitly rather than left to survive by luck.
fn write_matrix(cells: &[bool; NOTIFICATION_EVENTS.len()], receipts: bool) -> Value {
    let mut obj = serde_json::Map::new();
    for (cell, event) in cells.iter().zip(NOTIFICATION_EVENTS.iter()) {
        obj.insert(event.key.to_string(), json!({ "webhook": *cell }));
    }
    obj.insert(CUSTOMER_RECEIPTS_KEY.to_string(), Value::Bool(receipts));
    Value::Object(obj)
}

/// Notifications tab.
#[component]
pub fn NotificationsTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let store_ctx = use_context::<StoreContext>().expect("StoreContext must be provided");

    let selected_store_id = store_ctx.selected_store_id;
    let store_status = store_ctx.stores_status;
    let has_stores = Signal::derive({
        let ctx = store_ctx.clone();
        move || !ctx.stores.get().is_empty()
    });
    let retry_stores = Callback::new({
        let ctx = store_ctx.clone();
        move |()| ctx.refetch_stores()
    });

    let email_status = LocalResource::new(move || {
        let api = api.get();
        async move { api.get_email_status().await }
    });
    let email_configured = Signal::derive(move || {
        let answer = email_status.get();
        if let Some(Err(e)) = answer.as_deref() {
            web_sys::console::warn_1(&format!("Could not check email status: {e}").into());
        }
        EmailAvailability::from_status(answer.as_deref())
    });

    let (refresh, set_refresh) = signal(0u32);
    let settings = LocalResource::new(move || {
        let api = api.get();
        let store_id = selected_store_id.get();
        refresh.get();
        async move {
            match store_id {
                Some(id) => Some(api.get_store_settings(&id).await),
                None => None,
            }
        }
    });

    // The matrix: one webhook flag per event, parallel to NOTIFICATION_EVENTS.
    let cells = RwSignal::new([true; NOTIFICATION_EVENTS.len()]);
    // Receipts: a real control now that the endpoint accepts the key.
    let receipts = RwSignal::new(true);
    // Which store the matrix currently holds, not merely "something loaded":
    // the sidebar can switch stores under this tab, and a plain `loaded` flag
    // would leave the previous store's answers on screen — and then save them
    // onto the new store.
    let (loaded_store, set_loaded_store) = signal(Option::<String>::None);
    let (saving, set_saving) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (saved, set_saved) = signal(false);

    // Saveable only while the matrix on screen belongs to the store the save
    // would go to. Between picking another store and its settings landing,
    // those are two different stores.
    let ready = Signal::derive(move || {
        let loaded = loaded_store.get();
        loaded.is_some() && loaded == selected_store_id.get()
    });

    let on_save = move |_| {
        let Some(store_id) = selected_store_id.get() else {
            return;
        };
        set_saving.set(true);
        set_error_msg.set(None);
        set_saved.set(false);

        let api = api.get();
        let request = UpdateStoreSettingsRequest {
            default_chain_id: None,
            default_display_currency: None,
            logo_url: None,
            accent_color: None,
            notification_prefs: Some(write_matrix(&cells.get(), receipts.get())),
        };

        leptos::task::spawn_local(async move {
            let result = api.update_store_settings(&store_id, &request).await;
            // `try_*` throughout: switching tabs disposes this component while
            // the request is still in flight, and writing a disposed signal
            // panics the whole client.
            match result {
                Ok(_) => {
                    let _ = set_saved.try_set(true);
                    let _ = set_refresh.try_update(|n| *n += 1);
                }
                Err(e) => {
                    let _ = set_error_msg.try_set(Some(format!("{e}")));
                }
            }
            let _ = set_saving.try_set(false);
        });
    };

    view! {
        <div class="settings-tab-notifications">
            {move || {
                if selected_store_id.get().is_none() {
                    return view! {
                        <NoStoreSelected
                            entity="Notification settings"
                            status=store_status
                            has_stores
                            on_retry=retry_stores
                        />
                    }
                    .into_any();
                }

                view! {
                    <div class="ps-card">
                        <div class="ps-card-header">
                            <h3>"Event notifications"</h3>
                        </div>
                        <div class="ps-card-body">
                            <p class="form-help">
                                "These settings belong to the store selected in the sidebar. \
                                 Switching an event off stops the notification for that channel; \
                                 it does not stop the payment being processed."
                            </p>

                            {move || email_configured.get().notice().map(|n| view! {
                                <div class="form-alert form-alert-error">{n}</div>
                            })}

                            <Suspense fallback=move || view! {
                                <p class="text-muted">"Loading notification settings..."</p>
                            }>
                                {move || settings.get().map(|result| match result.as_ref() {
                                    Some(Ok(s)) => {
                                        // Populate on the first load for a store
                                        // and on every switch to another one, but
                                        // not on the refresh bump a save triggers,
                                        // which would otherwise re-render the
                                        // matrix from under the merchant.
                                        if loaded_store.get_untracked().as_deref()
                                            != Some(s.store_id.to_string().as_str())
                                        {
                                            cells.set(read_matrix(&s.notification_prefs));
                                            receipts.set(
                                                customer_receipts_enabled(&s.notification_prefs),
                                            );
                                            set_loaded_store.set(Some(s.store_id.to_string()));
                                        }
                                        view! { <NotificationMatrix cells receipts email_configured /> }.into_any()
                                    }
                                    Some(Err(e)) => {
                                        let msg = format!("Could not load notification settings: {e}");
                                        view! {
                                            <div class="form-alert form-alert-error">{msg}</div>
                                        }
                                        .into_any()
                                    }
                                    // The `None` arm is unreachable under the
                                    // store guard above, but the resource is
                                    // typed for it.
                                    None => view! { <div></div> }.into_any(),
                                })}
                            </Suspense>

                            {move || error_msg.get().map(|msg| view! {
                                <div class="form-alert form-alert-error">{msg}</div>
                            })}
                            <Show when=move || saved.get()>
                                <div class="form-alert form-alert-success">
                                    "Notification settings saved."
                                </div>
                            </Show>

                            <div class="form-actions">
                                <button
                                    class="ps-btn ps-btn-primary ps-btn-sm"
                                    disabled=move || saving.get() || !ready.get()
                                    on:click=on_save
                                >
                                    {move || if saving.get() { "Saving..." } else { "Save notification settings" }}
                                </button>
                            </div>
                        </div>
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}

/// The event x channel grid.
#[component]
fn NotificationMatrix(
    cells: RwSignal<[bool; NOTIFICATION_EVENTS.len()]>,
    receipts: RwSignal<bool>,
    email_configured: Signal<EmailAvailability>,
) -> impl IntoView {
    view! {
        <table class="notification-matrix">
            <thead>
                <tr>
                    <th scope="col">"Event"</th>
                    <th scope="col">"Webhook"</th>
                    <th scope="col">"Email"</th>
                </tr>
            </thead>
            <tbody>
                {NOTIFICATION_EVENTS.iter().enumerate().map(|(i, event)| {
                    let email_cell = match event.email {
                        EmailChannel::Unused => view! {
                            <span
                                class="text-muted"
                                title="No email is sent for this event"
                            >
                                "\u{2014}"
                            </span>
                        }
                        .into_any(),
                        // Reactive: the status can land after the matrix does.
                        EmailChannel::CustomerReceipt => (move || {
                            if !email_configured.get().channel_offered() {
                                view! {
                                    <span class="text-muted" title=EMAIL_UNAVAILABLE_NOTICE>
                                        "Unavailable"
                                    </span>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <label class="toggle">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || receipts.get()
                                            on:change=move |ev| {
                                                receipts.set(event_target_checked(&ev));
                                            }
                                            title="Email a receipt to the customer when their payment confirms"
                                        />
                                        <span class="toggle-slider"></span>
                                    </label>
                                }
                                .into_any()
                            }
                        })
                        .into_any(),
                    };

                    view! {
                        <tr>
                            <th scope="row">
                                <span class="notification-option-title">{event.label}</span>
                                <span class="notification-option-desc">{event.description}</span>
                                <code class="notification-event-key">{event.key}</code>
                            </th>
                            <td>
                                <label class="toggle">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || cells.get()[i]
                                        on:change=move |ev| {
                                            let on = event_target_checked(&ev);
                                            cells.update(|c| c[i] = on);
                                        }
                                    />
                                    <span class="toggle-slider"></span>
                                </label>
                            </td>
                            <td>{email_cell}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>

        <p class="form-help">
            "Webhooks are delivered to the endpoint configured on the store. \
             The only email the server sends is the customer payment receipt, \
             which is why the other email cells are empty rather than off."
        </p>
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CUSTOMER_RECEIPTS_KEY, NOTIFICATION_EVENTS, customer_receipts_enabled, read_matrix,
        webhook_enabled, write_matrix,
    };
    use crate::api::{ApiError, EmailStatus};
    use serde_json::json;

    #[test]
    fn email_is_removed_only_when_the_server_says_it_cannot_send() {
        use super::EmailAvailability::*;
        assert!(Configured.channel_offered());
        assert!(!NotConfigured.channel_offered());
        assert!(Checking.channel_offered());
        assert!(Unknown.channel_offered());
    }

    #[test]
    fn the_servers_answer_maps_to_the_right_state() {
        use super::EmailAvailability as A;
        let ok = |configured| Ok(EmailStatus { configured });
        assert_eq!(A::from_status(Some(&ok(true))), A::Configured);
        assert_eq!(A::from_status(Some(&ok(false))), A::NotConfigured);
        assert_eq!(A::from_status(None), A::Checking);
        // A failure (a server without the route, a 401) is not "configured".
        for e in [
            ApiError::Unauthorized,
            ApiError::Network("down".into()),
            ApiError::Http {
                status: 404,
                message: "no route".into(),
            },
            ApiError::Parse("bad".into()),
        ] {
            assert_eq!(A::from_status(Some(&Err(e))), A::Unknown);
        }
    }

    #[test]
    fn each_state_says_what_it_knows() {
        use super::EmailAvailability::*;
        assert_eq!(Configured.notice(), None);
        assert_eq!(Checking.notice(), None);
        assert_eq!(
            NotConfigured.notice(),
            Some(super::EMAIL_UNAVAILABLE_NOTICE)
        );
        // A failed check is never silent: it must not read as "configured".
        assert_eq!(Unknown.notice(), Some(super::EMAIL_UNKNOWN_NOTICE));
    }

    #[test]
    fn the_status_body_parses_in_both_states() {
        for want in [true, false] {
            let body = format!(r#"{{"configured":{want}}}"#);
            let s: crate::api::EmailStatus = serde_json::from_str(&body).unwrap();
            assert_eq!(s.configured, want);
        }
    }

    #[test]
    fn an_unset_blob_reads_as_everything_on() {
        // A store that has never saved this tab gets every webhook, so the
        // matrix must not draw it as everything off.
        let prefs = json!({});
        assert_eq!(read_matrix(&prefs), [true; 5]);
        assert!(customer_receipts_enabled(&prefs));
    }

    #[test]
    fn only_an_explicit_false_reads_as_off() {
        assert!(!webhook_enabled(
            &json!({"late_paid": {"webhook": false}}),
            "late_paid"
        ));
        assert!(webhook_enabled(
            &json!({"late_paid": {"webhook": true}}),
            "late_paid"
        ));
        // Anything the dispatcher would not treat as "off" is on here too.
        assert!(webhook_enabled(&json!({"late_paid": {}}), "late_paid"));
        assert!(webhook_enabled(
            &json!({"late_paid": {"webhook": "false"}}),
            "late_paid"
        ));
        assert!(webhook_enabled(
            &json!({"late_paid": {"email": false}}),
            "late_paid"
        ));
        assert!(webhook_enabled(&json!({}), "late_paid"));
    }

    #[test]
    fn receipts_follow_the_same_rule() {
        assert!(!customer_receipts_enabled(
            &json!({CUSTOMER_RECEIPTS_KEY: false})
        ));
        assert!(customer_receipts_enabled(
            &json!({CUSTOMER_RECEIPTS_KEY: true})
        ));
        assert!(customer_receipts_enabled(
            &json!({CUSTOMER_RECEIPTS_KEY: "no"})
        ));
    }

    #[test]
    fn a_matrix_survives_a_round_trip() {
        let cells = [true, false, false, true, false];
        assert_eq!(read_matrix(&write_matrix(&cells, true)), cells);
    }

    #[test]
    fn the_payload_carries_every_event_plus_the_receipts_switch() {
        let payload = write_matrix(&[true, false, true, false, true], true);
        let obj = payload.as_object().expect("payload is an object");
        assert_eq!(obj.len(), NOTIFICATION_EVENTS.len() + 1);
        for event in &NOTIFICATION_EVENTS {
            assert!(obj.contains_key(event.key), "missing {}", event.key);
        }
        assert!(obj.contains_key(CUSTOMER_RECEIPTS_KEY));
        assert_eq!(payload["payment_detected"], json!({"webhook": true}));
        assert_eq!(payload["payment_confirmed"], json!({"webhook": false}));
    }

    #[test]
    fn receipts_off_is_sent_as_an_explicit_false() {
        // Not omission. Absent reads as ENABLED on the server
        // (`confirmation_handler.rs` matches `Bool(false)` exactly), so leaving
        // the key out is how a merchant's "off" used to be lost.
        let payload = write_matrix(&[true; NOTIFICATION_EVENTS.len()], false);
        assert_eq!(payload[CUSTOMER_RECEIPTS_KEY], json!(false));
    }

    #[test]
    fn receipts_survive_a_round_trip_in_both_positions() {
        for want in [true, false] {
            let payload = write_matrix(&[true; NOTIFICATION_EVENTS.len()], want);
            assert_eq!(
                customer_receipts_enabled(&payload),
                want,
                "receipts={want} did not survive"
            );
        }
    }

    #[test]
    fn the_receipts_switch_does_not_disturb_the_event_cells() {
        let cells = [true, false, true, false, true];
        for receipts in [true, false] {
            assert_eq!(read_matrix(&write_matrix(&cells, receipts)), cells);
        }
    }

    #[test]
    fn rows_are_positional_and_stay_aligned_with_the_payload() {
        // Each cell must land on its own event; an off-by-one here would save
        // the merchant's choices onto the wrong events.
        for i in 0..NOTIFICATION_EVENTS.len() {
            let mut cells = [true; NOTIFICATION_EVENTS.len()];
            cells[i] = false;
            let payload = write_matrix(&cells, true);
            for (j, event) in NOTIFICATION_EVENTS.iter().enumerate() {
                assert_eq!(
                    payload[event.key]["webhook"],
                    json!(i != j),
                    "event {} at row {i}",
                    event.key
                );
            }
        }
    }
}
