//! Security tab: device list, revocation, and adding a passkey/device to
//! this account.
//!
//! SENSITIVE: "add a passkey" asks for the account's recovery phrase even
//! though the caller already holds a valid session - there is no session
//! key material standing in for it (see `auth_crypto`'s module doc): the
//! phrase is the only thing that has ever unwrapped this account's data
//! key, authenticated or not. The session still does real work here: it is
//! what lets the server attach the new passkey/device to *this* account and
//! enforce the device cap, which the phrase alone cannot do (unlike
//! `/auth/recovery/*`, this never revokes the account's existing passkeys,
//! devices or sessions - that is the whole difference between "add a
//! device" and "recover a lost one", see `pages/recovery.rs`).

use leptos::prelude::*;
use ui_kit::auth::session::{get_device_id, get_device_name};
use ui_kit::auth::types::{
    CompletePasskeyLoginRequest, DeviceType, LoginResponse, StartRecoveryRequest,
};
use ui_kit::auth::webauthn::{create_credential, get_credential};
use ui_kit::hooks::use_api::ApiClient as UiApiClient;
use ui_kit::use_auth;

use crate::api::ApiClient;
use crate::auth_crypto;
use crate::components::{ErrorState, LoadingState};

use super::format_date;

/// The step-up ceremony `on_add_submit` runs immediately before
/// `enroll_device`: a plain login (asserting with any of the account's
/// passkeys - discoverable credentials, so the one just registered counts
/// too) that mints a session with a fresh `created_at`. That is the one
/// thing `FreshlyAuthenticatedUser` actually checks (see this module's doc
/// and `ethpayserver/server/src/api/extractors.rs`), and nothing before this
/// point in the flow refreshes it - the recovery-phrase check proves
/// possession of the phrase, not session age.
///
/// A cancelled or failed assertion is caught here, before the login is
/// ever completed, so the caller gets a plain `Err` to show inline - never
/// a 401 that could be mistaken for the session itself having ended.
async fn perform_step_up(login_api: &UiApiClient) -> Result<LoginResponse, String> {
    let challenge = login_api
        .start_passkey_login()
        .await
        .map_err(|e| format!("Could not start step-up verification: {e}"))?;

    let credential = get_credential(&challenge.options)
        .await
        .map_err(|e| format!("Step-up verification was cancelled or failed: {e}"))?;

    login_api
        .complete_passkey_login(CompletePasskeyLoginRequest {
            challenge_id: challenge.challenge_id,
            credential,
            // Reuse this browser's existing device id/name so step-up logs
            // in on the same device record rather than minting a second one.
            device_id: get_device_id(),
            device_name: get_device_name(),
            device_type: DeviceType::Browser,
        })
        .await
        .map_err(|e| format!("Step-up verification failed: {e}"))
}

#[derive(Clone, Copy, PartialEq)]
enum AddStep {
    Closed,
    EnterPhrase,
    Working,
    Done,
}

/// Security tab: lists `GET /auth/devices`, revokes via `DELETE
/// /auth/devices/{id}`, and adds a device via the phrase-gated flow
/// described above.
#[component]
pub fn SecurityTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let auth = use_auth();

    let (reload, set_reload) = signal(0u32);
    let user_resource = LocalResource::new(move || {
        let api = api.get();
        async move { api.get_me().await }
    });
    let devices_resource = LocalResource::new(move || {
        reload.get();
        let api = api.get();
        let token = auth.token.get();
        async move {
            match token {
                Some(t) => api.list_devices(&t).await,
                None => Err(crate::api::ApiError::Unauthorized),
            }
        }
    });

    let current_device_id = get_device_id().map(|d| d.0);

    let (revoking, set_revoking) = signal(Option::<uuid::Uuid>::None);
    let (revoke_error, set_revoke_error) = signal(Option::<String>::None);
    let on_revoke = move |id: uuid::Uuid| {
        let api = api.get();
        let Some(token) = auth.token.get() else {
            return;
        };
        set_revoking.set(Some(id));
        set_revoke_error.set(None);
        leptos::task::spawn_local(async move {
            let result = api.revoke_device(&token, id).await;
            set_revoking.set(None);
            match result {
                Ok(()) => set_reload.update(|n| *n += 1),
                Err(e) => set_revoke_error.set(Some(e.to_string())),
            }
        });
    };

    let (add_step, set_add_step) = signal(AddStep::Closed);
    let (phrase_input, set_phrase_input) = signal(String::new());
    let (add_error, set_add_error) = signal(Option::<String>::None);

    let on_add_submit = move |_| {
        set_add_error.set(None);
        let phrase = phrase_input.get().trim().to_string();
        if phrase.is_empty() {
            set_add_error.set(Some("Enter the account's recovery phrase.".to_string()));
            return;
        }
        let snapshot = user_resource.get();
        let user = match snapshot.as_deref() {
            Some(Ok(user)) => user.clone(),
            _ => {
                set_add_error.set(Some(
                    "Could not load account info - try reopening this tab.".to_string(),
                ));
                return;
            }
        };
        let Some(token) = auth.token.get() else {
            set_add_error.set(Some("Not signed in.".to_string()));
            return;
        };
        let api = api.get();
        set_add_step.set(AddStep::Working);

        leptos::task::spawn_local(async move {
            let identifier = auth_crypto::identifier(
                user.email.as_deref(),
                user.primary_wallet_address.as_deref(),
                &user.id,
            );
            let hash = match auth_crypto::verification_hash(&phrase, &identifier) {
                Ok(h) => h,
                Err(e) => {
                    set_add_error.set(Some(e));
                    set_add_step.set(AddStep::EnterPhrase);
                    return;
                }
            };

            // Borrows `/auth/recovery/start` purely to fetch the account's
            // already-wrapped key under proof of the phrase - never calls
            // `/auth/recovery/complete`, so nothing is revoked. See the
            // module doc for why this is the only way to get it: there is
            // no session key material that could stand in for the phrase.
            let recovery_api = UiApiClient::new("/api");
            let start = match recovery_api
                .start_recovery(StartRecoveryRequest {
                    identifier,
                    recovery_verification_hash: hash,
                    captcha_token: None,
                })
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    set_add_error.set(Some(format!("Could not verify the phrase: {e}")));
                    set_add_step.set(AddStep::EnterPhrase);
                    return;
                }
            };

            let passkey_name = format!(
                "{} ({})",
                get_device_name(),
                chrono::Utc::now().format("%Y-%m-%d")
            );

            let options = match api.start_add_passkey(&token, &passkey_name).await {
                Ok(o) => o,
                Err(e) => {
                    set_add_error.set(Some(format!("Could not start passkey registration: {e}")));
                    set_add_step.set(AddStep::EnterPhrase);
                    return;
                }
            };
            let credential = match create_credential(&options).await {
                Ok(c) => c,
                Err(e) => {
                    set_add_error.set(Some(format!("Could not create a new passkey: {e}")));
                    set_add_step.set(AddStep::EnterPhrase);
                    return;
                }
            };
            if let Err(e) = api
                .complete_add_passkey(&token, &passkey_name, credential)
                .await
            {
                set_add_error.set(Some(format!("Could not register the passkey: {e}")));
                set_add_step.set(AddStep::EnterPhrase);
                return;
            }

            // `POST /users/me/devices` requires a session newer than
            // `FreshlyAuthenticatedUser`'s 5-minute window - nothing above
            // this point refreshes the session, so step up with a real
            // login before retrying it (see `perform_step_up` and this
            // module's doc). Reuses `recovery_api`: both it and the step-up
            // login hit unauthenticated endpoints, so the same client works
            // for either.
            let step_up = perform_step_up(&recovery_api).await;
            if let Ok(ref response) = step_up {
                // The step-up login is real, so treat it like one: update
                // the session the rest of the app uses, not just the one
                // `enroll_device_after_step_up` retries with below.
                auth.save_login(response);
            }

            match api
                .enroll_device_after_step_up(
                    step_up,
                    &get_device_name(),
                    DeviceType::Browser,
                    start.encrypted_symmetric_key,
                    start.kdf_params,
                )
                .await
            {
                Ok(_) => {
                    set_phrase_input.set(String::new());
                    set_add_step.set(AddStep::Done);
                    set_reload.update(|n| *n += 1);
                }
                Err(e) => {
                    // The passkey is already registered at this point - only
                    // the `devices` row failed. Not a dead end: the next
                    // login with this passkey still mints one lazily
                    // (`device_for_new_login` upstream), just without the
                    // cap having been checked up front.
                    set_add_error.set(Some(format!(
                        "The passkey was registered, but the device record failed: {e}. \
                         It will still work the next time you log in with it."
                    )));
                    set_add_step.set(AddStep::Done);
                }
            }
        });
    };

    view! {
        <div class="settings-tab-security">
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Devices"</h3>
                    <button
                        class="ps-btn ps-btn-secondary ps-btn-sm"
                        on:click=move |_| set_add_step.set(AddStep::EnterPhrase)
                        disabled=move || !matches!(add_step.get(), AddStep::Closed | AddStep::Done)
                    >
                        "Add a passkey"
                    </button>
                </div>
                <div class="ps-card-body">
                    <Suspense fallback=move || view! { <LoadingState message="Loading devices..." /> }>
                        {move || devices_resource.get().map(|result| match &*result {
                            Ok(devices) if devices.is_empty() => view! {
                                <p class="form-help">"No devices found."</p>
                            }.into_any(),
                            Ok(devices) => {
                                let devices = devices.clone();
                                // Reuses the API-key list's row layout
                                // (`.api-keys-list`/`.api-key-item`/...): a
                                // name, metadata and action buttons, same
                                // shape as a device row.
                                view! {
                                    <div class="api-keys-list">
                                        {devices.into_iter().map(|d| {
                                            let is_current = current_device_id == Some(d.id);
                                            let id = d.id;
                                            let created = format_date(&d.created_at.to_rfc3339());
                                            view! {
                                                <div class="api-key-item">
                                                    <div class="api-key-info">
                                                        <div class="api-key-header">
                                                            <span class="api-key-name">{d.name.clone()}</span>
                                                            {is_current.then(|| view! {
                                                                <span class="badge badge-neutral">"This device"</span>
                                                            })}
                                                        </div>
                                                        <span class="api-key-created">
                                                            {format!("Added {created}")}
                                                        </span>
                                                    </div>
                                                    <div class="api-key-actions">
                                                        <button
                                                            class="ps-btn ps-btn-danger ps-btn-sm"
                                                            disabled=move || is_current || revoking.get() == Some(id)
                                                            on:click=move |_| on_revoke(id)
                                                        >
                                                            {move || if revoking.get() == Some(id) { "Revoking..." } else { "Revoke" }}
                                                        </button>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                            Err(e) => view! { <ErrorState message=e.to_string() /> }.into_any(),
                        })}
                    </Suspense>
                    {move || revoke_error.get().map(|e| view! { <p class="text-error">{e}</p> })}
                </div>
            </div>

            {move || matches!(add_step.get(), AddStep::EnterPhrase | AddStep::Working).then(|| view! {
                <div class="ps-card">
                    <div class="ps-card-header"><h3>"Add a passkey"</h3></div>
                    <div class="ps-card-body">
                        <p class="form-help">
                            "Enter this account's recovery phrase to prove you can still \
                             decrypt its data, then approve a new passkey for this browser. \
                             This does not affect any other device."
                        </p>
                        {move || add_error.get().map(|e| view! { <p class="text-error">{e}</p> })}
                        <div class="form-group">
                            <label class="form-label">"Recovery phrase"</label>
                            <textarea
                                class="form-input"
                                rows="3"
                                prop:value=move || phrase_input.get()
                                on:input=move |ev| set_phrase_input.set(event_target_value(&ev))
                                disabled=move || matches!(add_step.get(), AddStep::Working)
                            ></textarea>
                        </div>
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                on:click=on_add_submit
                                disabled=move || matches!(add_step.get(), AddStep::Working)
                            >
                                {move || if matches!(add_step.get(), AddStep::Working) { "Working..." } else { "Continue" }}
                            </button>
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                on:click=move |_| {
                                    set_add_step.set(AddStep::Closed);
                                    set_add_error.set(None);
                                    set_phrase_input.set(String::new());
                                }
                                disabled=move || matches!(add_step.get(), AddStep::Working)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </div>
            })}
        </div>
    }
}
