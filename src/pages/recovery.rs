//! Account recovery page: redeem a lost account with its BIP39 recovery
//! phrase. Unauthenticated by design - this is exactly what a merchant
//! reaches for when they no longer have a working passkey or wallet.
//!
//! SENSITIVE: the whole point of this page is that the phrase typed into it
//! never leaves the browser. Every crypto step is `auth_crypto`, built on
//! the same primitives registration uses; nothing here invents its own.
//!
//! Flow: identify the account -> enter the phrase -> the server verifies a
//! hash of it and returns the account's current wrapped key -> unwrap it
//! locally -> register a new passkey (WebAuthn `create()`) -> re-wrap under
//! a freshly generated phrase -> submit. The server then revokes every
//! other passkey, device and session on the account (see
//! `auth::service::recovery::complete_account_recovery` upstream) - this is
//! account recovery, not "add a device", which is the separate, narrower
//! flow in `pages/settings/security.rs`.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use ui_kit::auth::components::TurnstileWidget;
use ui_kit::auth::session::get_device_name;
use ui_kit::auth::types::{CompleteRecoveryRequest, DeviceType, StartRecoveryRequest};
use ui_kit::auth::webauthn::create_credential;
use ui_kit::hooks::use_api::ApiClient as UiApiClient;
use ui_kit::use_auth;

use crate::auth_crypto;
use crate::components::LoadingInline;

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Identify,
    Working,
    ShowNewPhrase,
}

/// Account recovery page, mounted at `/recovery`.
#[component]
pub fn RecoveryPage() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();

    let (step, set_step) = signal(Step::Identify);
    let (identifier_input, set_identifier_input) = signal(String::new());
    let (phrase_input, set_phrase_input) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (new_phrase, set_new_phrase) = signal(Vec::<String>::new());
    let (captcha_token, set_captcha_token) = signal(Option::<String>::None);
    let (captcha_site_key, set_captcha_site_key) = signal(Option::<String>::None);

    // Fetch CAPTCHA configuration up front, same as the register page: this
    // endpoint is unauthenticated and reachable with any public identifier,
    // so a human-proof widget is what makes scripted guessing costly rather
    // than free.
    leptos::task::spawn_local(async move {
        let api = UiApiClient::new("/api");
        if let Ok(config) = api.get_captcha_config().await
            && config.enabled
        {
            set_captcha_site_key.set(config.site_key);
        }
    });

    let on_submit = move |_| {
        set_error.set(None);
        let identifier_raw = identifier_input.get().trim().to_string();
        let phrase = phrase_input.get().trim().to_string();
        if identifier_raw.is_empty() || phrase.is_empty() {
            set_error.set(Some(
                "Enter the account identifier and the full recovery phrase.".to_string(),
            ));
            return;
        }
        let captcha = captcha_token.get();
        set_step.set(Step::Working);

        leptos::task::spawn_local(async move {
            let salt_identifier = auth_crypto::salt_identifier_for_recovery_input(&identifier_raw);
            let hash = match auth_crypto::verification_hash(&phrase, &salt_identifier) {
                Ok(h) => h,
                Err(e) => {
                    set_error.set(Some(e));
                    set_step.set(Step::Identify);
                    return;
                }
            };

            let api = UiApiClient::new("/api");
            let start = match api
                .start_recovery(StartRecoveryRequest {
                    identifier: identifier_raw.clone(),
                    recovery_verification_hash: hash,
                    captcha_token: captcha,
                })
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    set_error.set(Some(format!("Recovery could not start: {e}")));
                    set_step.set(Step::Identify);
                    return;
                }
            };

            let account_key = match auth_crypto::unwrap_account_key(
                &phrase,
                &salt_identifier,
                &start.encrypted_symmetric_key,
            ) {
                Ok(k) => k,
                Err(e) => {
                    set_error.set(Some(e));
                    set_step.set(Step::Identify);
                    return;
                }
            };

            let credential = match create_credential(&start.options).await {
                Ok(c) => c,
                Err(e) => {
                    set_error.set(Some(format!("Could not create a new passkey: {e}")));
                    set_step.set(Step::Identify);
                    return;
                }
            };

            let (words, new_kdf_params, new_blob, new_hash) =
                match auth_crypto::rewrap_under_new_phrase(&account_key, &salt_identifier) {
                    Ok(v) => v,
                    Err(e) => {
                        set_error.set(Some(e));
                        set_step.set(Step::Identify);
                        return;
                    }
                };

            let complete = CompleteRecoveryRequest {
                identifier: identifier_raw,
                credential,
                passkey_name: "Recovered passkey".to_string(),
                new_kdf_params,
                new_encrypted_symmetric_key: new_blob,
                new_recovery_verification_hash: new_hash,
                device_name: get_device_name(),
                device_type: DeviceType::Browser,
            };

            match api.complete_recovery(complete).await {
                Ok(response) => {
                    set_new_phrase.set(words);
                    set_step.set(Step::ShowNewPhrase);
                    // Logged in as the account recovery just rebuilt. Every
                    // other passkey, device and session is already revoked
                    // server-side by this point.
                    auth.save_login(&response);
                }
                Err(e) => {
                    set_error.set(Some(format!("Recovery could not complete: {e}")));
                    set_step.set(Step::Identify);
                }
            }
        });
    };

    view! {
        <div class="ps-auth-page">
            <div class="ps-auth-card">
                {move || match step.get() {
                    Step::Identify => view! {
                        <div class="ps-auth-header">
                            <h1 class="ps-auth-title">"Recover your account"</h1>
                            <p class="ps-auth-subtitle">
                                "Enter your account identifier and recovery phrase. \
                                 This replaces every passkey, device and session on \
                                 the account - including this one, if it had any."
                            </p>
                        </div>
                        {move || error.get().map(|e| view! {
                            <div class="ps-auth-error"><p>{e}</p></div>
                        })}
                        <div class="form-group">
                            <label class="form-label">"Email, wallet address, or account id"</label>
                            <input
                                type="text"
                                class="form-input"
                                prop:value=move || identifier_input.get()
                                on:input=move |ev| set_identifier_input.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="form-group">
                            <label class="form-label">"Recovery phrase"</label>
                            <textarea
                                class="form-input"
                                rows="3"
                                prop:value=move || phrase_input.get()
                                on:input=move |ev| set_phrase_input.set(event_target_value(&ev))
                            ></textarea>
                            <p class="form-help">"The 24 words shown when this account was created."</p>
                        </div>
                        {move || captcha_site_key.get().map(|key| view! {
                            <TurnstileWidget
                                site_key=key
                                on_token=Callback::new(move |token: String| {
                                    set_captcha_token.set(Some(token));
                                })
                            />
                        })}
                        <div class="form-actions">
                            <button class="ps-btn ps-btn-primary" on:click=on_submit>
                                "Recover account"
                            </button>
                        </div>
                    }.into_any(),
                    Step::Working => view! {
                        <div class="ps-auth-header">
                            <h1 class="ps-auth-title">"Recovering your account..."</h1>
                        </div>
                        <LoadingInline message="Verifying phrase and registering a new passkey..." />
                    }.into_any(),
                    Step::ShowNewPhrase => view! {
                        <div class="ps-auth-header">
                            <h1 class="ps-auth-title">"Account recovered"</h1>
                            <p class="ps-auth-subtitle">
                                "Write down this NEW recovery phrase now - it replaces \
                                 the old one, which no longer works. This is the only \
                                 time it is shown."
                            </p>
                        </div>
                        <div class="ps-mnemonic-grid">
                            {new_phrase.get().into_iter().enumerate().map(|(i, word)| view! {
                                <div class="ps-mnemonic-word">
                                    <span class="ps-mnemonic-index">{i + 1}</span>
                                    <span class="ps-mnemonic-text">{word}</span>
                                </div>
                            }).collect_view()}
                        </div>
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary"
                                on:click={
                                    let navigate = navigate.clone();
                                    move |_| navigate("/", Default::default())
                                }
                            >
                                "I've saved it - continue"
                            </button>
                        </div>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
