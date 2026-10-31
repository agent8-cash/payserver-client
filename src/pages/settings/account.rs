//! Account settings tab.

use crate::api::ApiClient;
use leptos::prelude::*;
use ui_kit::use_auth;

use super::format_date;

/// The line under the address once the code has been mailed. The lifetime is
/// the server's to report: without it, the copy makes no promise about a time
/// it cannot back.
#[component]
fn CodeSentNotice(addr: String, ttl_minutes: Option<i64>) -> impl IntoView {
    let expiry = match ttl_minutes {
        Some(n) => format!(". It expires in {n} minutes."),
        None => ". It will expire soon.".to_string(),
    };
    view! {
        <p class="form-help" style="margin-bottom: 16px;">
            "We sent a confirmation code to " <code>{addr}</code> {expiry}
        </p>
    }
}

/// Set or change the contact email. The address is only attached once the
/// code mailed to it is entered here, so a typo cannot lock a merchant out of
/// notices. Optional, and offered only where the server can send mail.
#[component]
fn ContactEmailCard(has_email: bool, on_changed: Callback<()>) -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    let email_status = LocalResource::new(move || {
        let api = api.get();
        async move { api.get_email_status().await }
    });
    // None while loading; Some(false) when the server cannot send mail or the
    // check failed, so no flow is offered that could never complete.
    let configured = move || {
        email_status
            .get()
            .map(|w| (*w).as_ref().map(|s| s.configured).unwrap_or(false))
    };

    let ttl_minutes = move || {
        email_status
            .get()
            .and_then(|w| (*w).as_ref().ok().and_then(|s| s.change_ttl_minutes))
    };

    let (address, set_address) = signal(String::new());
    // Set once the code has been mailed; the confirm step only shows after it.
    let (pending, set_pending) = signal(Option::<String>::None);
    let (code, set_code) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (saved, set_saved) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let on_request = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let new_email = address.get().trim().to_string();
        if new_email.is_empty() || busy.get() {
            return;
        }
        let api = api.get();
        set_busy.set(true);
        set_saved.set(false);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match api.request_email_change(&new_email).await {
                Ok(()) => {
                    let _ = set_pending.try_set(Some(new_email));
                }
                Err(e) => {
                    let _ = set_error.try_set(Some(e.to_string()));
                }
            }
            let _ = set_busy.try_set(false);
        });
    };

    let on_confirm = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let token = code.get().trim().to_string();
        if token.is_empty() || busy.get() {
            return;
        }
        let api = api.get();
        set_busy.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match api.confirm_email_change(&token).await {
                Ok(()) => {
                    let _ = set_pending.try_set(None);
                    let _ = set_address.try_set(String::new());
                    let _ = set_code.try_set(String::new());
                    let _ = set_saved.try_set(true);
                    on_changed.run(());
                }
                Err(e) => {
                    let _ = set_error.try_set(Some(e.to_string()));
                }
            }
            let _ = set_busy.try_set(false);
        });
    };

    let action_label = if has_email {
        "Change email"
    } else {
        "Add email"
    };

    view! {
        <div class="ps-card">
            <div class="ps-card-header">
                <h3>"Contact Email"</h3>
                <span class="badge badge-neutral">"Optional"</span>
            </div>
            <div class="ps-card-body">
                <p class="form-help" style="margin-bottom: 16px;">
                    "Used for receipts and billing notices. Account recovery does not use it: recovery is seed-based, so an account without an email can still be recovered."
                </p>
                {move || match configured() {
                    None => ().into_any(),
                    Some(false) => view! {
                        <p class="text-muted">"This server cannot send email, so a contact address cannot be verified here."</p>
                    }.into_any(),
                    Some(true) => match pending.get() {
                        None => view! {
                            <form on:submit=on_request>
                                <div class="form-group">
                                    <label class="form-label" for="contact-email">{action_label}</label>
                                    <input
                                        id="contact-email"
                                        type="email"
                                        class="form-input"
                                        prop:value=move || address.get()
                                        on:input=move |ev| set_address.set(event_target_value(&ev))
                                    />
                                </div>
                                <div class="form-actions">
                                    <button class="ps-btn ps-btn-primary ps-btn-sm" type="submit" disabled=move || busy.get()>
                                        "Send confirmation code"
                                    </button>
                                </div>
                            </form>
                        }.into_any(),
                        Some(addr) => view! {
                            <CodeSentNotice addr=addr ttl_minutes=ttl_minutes() />
                            <form on:submit=on_confirm>
                                <div class="form-group">
                                    <label class="form-label" for="contact-email-code">"Confirmation code"</label>
                                    <input
                                        id="contact-email-code"
                                        type="text"
                                        class="form-input"
                                        prop:value=move || code.get()
                                        on:input=move |ev| set_code.set(event_target_value(&ev))
                                    />
                                </div>
                                <div class="form-actions">
                                    <button class="ps-btn ps-btn-primary ps-btn-sm" type="submit" disabled=move || busy.get()>
                                        "Confirm"
                                    </button>
                                    <button
                                        class="ps-btn ps-btn-secondary ps-btn-sm"
                                        type="button"
                                        disabled=move || busy.get()
                                        on:click=move |_| {
                                            set_pending.set(None);
                                            set_code.set(String::new());
                                            set_error.set(None);
                                        }
                                    >
                                        "Use a different address"
                                    </button>
                                </div>
                            </form>
                        }.into_any(),
                    },
                }}
                {move || saved.get().then(|| view! {
                    <p class="form-help">"Contact email saved."</p>
                })}
                {move || error.get().map(|msg| view! {
                    <p class="text-error">{msg}</p>
                })}
            </div>
        </div>
    }
}

/// Account settings tab — loads profile from `/auth/me`.
#[component]
pub fn AccountTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let auth = use_auth();

    // Deleting is irreversible and cascades, so it takes two deliberate steps:
    // reveal the confirmation, then type the account's own handle back. A single
    // click - or a browser `confirm()`, which people dismiss by reflex - is not
    // enough for an action that destroys stores, wallets and API keys.
    let (confirming, set_confirming) = signal(false);
    let (typed, set_typed) = signal(String::new());
    let (deleting, set_deleting) = signal(false);
    let (delete_error, set_delete_error) = signal(Option::<String>::None);

    let user_resource = LocalResource::new(move || {
        let api = api.get();
        async move { api.get_me().await }
    });

    view! {
        <div class="settings-tab-account">
            <Suspense fallback=move || view! {
                <div style="text-align: center; padding: 3rem; color: var(--text-muted);">
                    "Loading profile..."
                </div>
            }>
                {move || user_resource.get().map(|result| match &*result {
                    Ok(user) => {
                        let email_display = user.email.clone().unwrap_or_else(|| "Not set".to_string());
                        let wallet_display = user.primary_wallet_address.clone().unwrap_or_else(|| "Not set".to_string());
                        let role_label = user.role.label().to_string();
                        let role_class = if user.role.is_admin() { "badge badge-warning" } else { "badge badge-neutral" };
                        let created = format_date(&user.created_at);
                        let last_login = user.last_login_at.as_deref().map(format_date).unwrap_or_else(|| "Never".to_string());
                        let user_id = user.id.clone();
                        // What the server will compare against: the email where
                        // there is one, else the account id. A passkey-only
                        // account has neither email nor wallet, so its id is the
                        // only handle it has.
                        let expected = user.email.clone().unwrap_or_else(|| user.id.clone());
                        // `StoredValue` so the handlers below stay `Copy`. A
                        // captured `String` would make `on_delete` `FnOnce`, and
                        // a reactive block has to be callable more than once.
                        let expected_stored = StoredValue::new(expected.clone());
                        let matches = move || {
                            expected_stored.with_value(|e| {
                                typed.get().trim().eq_ignore_ascii_case(e.trim())
                            })
                        };
                        let on_delete = move |_| {
                            if !matches() {
                                return;
                            }
                            let api = api.get();
                            let confirm = expected_stored.get_value();
                            set_deleting.set(true);
                            set_delete_error.set(None);
                            leptos::task::spawn_local(async move {
                                match api.delete_account(&confirm).await {
                                    Ok(()) => {
                                        // The session died with the account; drop
                                        // the local token so the app does not keep
                                        // retrying with it.
                                        auth.logout();
                                    }
                                    Err(e) => {
                                        // Includes the 409 naming the payments,
                                        // payouts or refunds holding the account.
                                        // Shown verbatim: "you have 3 payments" is
                                        // the answer, not a failure to report one.
                                        let _ = set_delete_error.try_set(Some(e.to_string()));
                                        let _ = set_deleting.try_set(false);
                                    }
                                }
                            });
                        };
                        let has_email = user.email.is_some();
                        let has_wallet = user.primary_wallet_address.is_some();

                        view! {
                            <div class="ps-card">
                                <div class="ps-card-header">
                                    <h3>"Profile Information"</h3>
                                    <span class=role_class>{role_label}</span>
                                </div>
                                <div class="ps-card-body">
                                    <div class="form-group">
                                        <label class="form-label">"User ID"</label>
                                        <div class="form-static">
                                            <code>{user_id}</code>
                                        </div>
                                    </div>

                                    <div class="form-group">
                                        <label class="form-label">"Email Address"</label>
                                        <div class="form-static">
                                            {if has_email {
                                                view! { <span>{email_display}</span> }.into_any()
                                            } else {
                                                view! { <span class="text-muted">"Not set (wallet-only account)"</span> }.into_any()
                                            }}
                                        </div>
                                    </div>

                                    <div class="form-group">
                                        <label class="form-label">"Wallet Address"</label>
                                        <div class="form-static">
                                            {if has_wallet {
                                                view! { <code>{wallet_display}</code> }.into_any()
                                            } else {
                                                view! { <span class="text-muted">"Not set (email-only account)"</span> }.into_any()
                                            }}
                                        </div>
                                    </div>

                                    <div class="settings-grid">
                                        <div class="form-group">
                                            <label class="form-label">"Account Created"</label>
                                            <div class="form-static">{created}</div>
                                        </div>
                                        <div class="form-group">
                                            <label class="form-label">"Last Login"</label>
                                            <div class="form-static">{last_login}</div>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <ContactEmailCard
                                has_email=has_email
                                on_changed=Callback::new(move |_| user_resource.refetch())
                            />

                            <div class="ps-card">
                                <div class="ps-card-header">
                                    <h3>"Security"</h3>
                                </div>
                                <div class="ps-card-body">
                                    <p class="form-help" style="margin-bottom: 16px;">
                                        "This server uses passwordless authentication. Manage your passkeys and connected wallets to control access to your account."
                                    </p>
                                    <div class="form-actions">
                                        <button class="ps-btn ps-btn-secondary ps-btn-sm" disabled=true>
                                            "Manage passkeys"
                                        </button>
                                        <button class="ps-btn ps-btn-secondary ps-btn-sm" disabled=true>
                                            "Manage wallets"
                                        </button>
                                    </div>
                                </div>
                            </div>

                            <div class="ps-card ps-card-danger">
                                <div class="ps-card-header">
                                    <h3>"Danger Zone"</h3>
                                </div>
                                <div class="ps-card-body">
                                    <div class="danger-action">
                                        <div class="danger-action-info">
                                            <span class="danger-action-title">"Delete account"</span>
                                            <span class="danger-action-desc">
                                                "Deletes this account, its stores, wallets and API keys. \
                                                 Refused if any store has taken a payment - that history \
                                                 cannot be destroyed. Addresses already issued stay valid \
                                                 and funds sent to them remain yours."
                                            </span>
                                        </div>
                                        <button
                                            class="ps-btn ps-btn-danger ps-btn-sm"
                                            on:click=move |_| set_confirming.set(true)
                                            disabled=move || confirming.get()
                                        >
                                            "Delete account"
                                        </button>
                                    </div>

                                    {
                                        // Owned by the closure and cloned per
                                        // call: cloning the captured value
                                        // directly would move it, and a reactive
                                        // block has to be callable more than once.
                                        let expected_label = expected.clone();
                                        move || confirming.get().then(|| {
                                        let expected = expected_label.clone();
                                        view! {
                                            <div class="form-group">
                                                <label class="form-label">
                                                    "Type " <code>{expected.clone()}</code> " to confirm"
                                                </label>
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    prop:value=move || typed.get()
                                                    on:input=move |ev| set_typed.set(event_target_value(&ev))
                                                />
                                                {move || delete_error.get().map(|msg| view! {
                                                    <p style="color: var(--color-error)">{msg}</p>
                                                })}
                                                <div class="form-actions">
                                                    <button
                                                        class="ps-btn ps-btn-danger ps-btn-sm"
                                                        on:click=on_delete
                                                        disabled=move || !matches() || deleting.get()
                                                    >
                                                        {move || if deleting.get() {
                                                            "Deleting..."
                                                        } else {
                                                            "Permanently delete this account"
                                                        }}
                                                    </button>
                                                    <button
                                                        class="ps-btn ps-btn-secondary ps-btn-sm"
                                                        on:click=move |_| {
                                                            set_confirming.set(false);
                                                            set_typed.set(String::new());
                                                            set_delete_error.set(None);
                                                        }
                                                        disabled=move || deleting.get()
                                                    >
                                                        "Cancel"
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    })
                                    }
                                </div>
                            </div>
                        }.into_any()
                    }
                    Err(e) => view! {
                        <div class="ps-card">
                            <div class="ps-card-body">
                                <p class="text-error">"Failed to load profile: "{e.to_string()}</p>
                            </div>
                        </div>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::CodeSentNotice;
    use leptos::prelude::*;

    fn render(ttl_minutes: Option<i64>) -> String {
        view! {
            <CodeSentNotice addr="merchant@example.com".to_string() ttl_minutes=ttl_minutes />
        }
        .to_html()
    }

    #[test]
    fn the_pending_copy_states_the_servers_ttl() {
        let html = render(Some(15));
        assert!(html.contains("merchant@example.com"));
        assert!(html.contains("It expires in 15 minutes."));
        assert!(!html.contains("30 minutes"));
    }

    #[test]
    fn the_pending_copy_makes_no_time_promise_without_a_ttl() {
        let html = render(None);
        assert!(html.contains("merchant@example.com"));
        assert!(html.contains("It will expire soon."));
        assert!(!html.contains("minutes"));
    }
}
