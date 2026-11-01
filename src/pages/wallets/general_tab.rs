use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::{ApiClient, UpdateWalletRequest, Wallet};

use super::icons::*;

/// General settings tab.
#[component]
pub(super) fn GeneralTab(wallet: Wallet) -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let navigate = use_navigate();

    let (name, set_name) = signal(wallet.name.clone().unwrap_or_default());
    // `is_primary` is a promotion, never a demotion: the server ignores
    // `Some(false)`, because "this account has no primary wallet" is not a state
    // a merchant can usefully ask for - a store with no override would then
    // resolve to nothing. So the control offers promotion and goes away once
    // this wallet holds the role.
    let already_primary = wallet.is_primary;
    let (promote, set_promote) = signal(false);
    let (saving, set_saving) = signal(false);
    let (deleting, set_deleting) = signal(false);
    let (message, set_message) = signal(Option::<(bool, String)>::None);
    // `Option<bool>`: None at rest, Some(true) copied, Some(false) refused. The
    // clipboard write can be rejected - no secure context, no user gesture, some
    // embedded browsers - and reporting "Copied" regardless leaves the merchant
    // pasting a stale buffer.
    let (copied, set_copied) = signal(None::<bool>);

    let wallet_id = wallet.id;
    let xpub_for_copy = wallet.xpub_masked.clone();

    let on_copy = move |_| {
        // The masked xpub is what is on screen, and what gets copied. The full
        // key is a separate, deliberate export (`GET /wallets/{id}/xpub`);
        // silently copying it here would hand out the whole key from a button
        // whose label says nothing about that.
        let xpub = xpub_for_copy.clone();
        leptos::task::spawn_local(async move {
            // This control is an icon button, so it uses ui-kit's shared helper
            // rather than <CopyButton/> - same awaited write and same honesty
            // about failure, without turning the icon into a text label.
            let ok = ui_kit::copy_to_clipboard(&xpub).await;
            let _ = set_copied.try_set(Some(ok));
            gloo_timers::future::TimeoutFuture::new(2000).await;
            let _ = set_copied.try_set(None);
        });
    };

    let on_save = move |_| {
        let api = api.get();
        let trimmed = name.get().trim().to_string();
        let req = UpdateWalletRequest {
            // Empty means "no name", not "leave it alone" - the field is a text
            // input a merchant can clear on purpose.
            name: Some(trimmed),
            is_primary: promote.get().then_some(true),
        };
        set_saving.set(true);
        set_message.set(None);
        leptos::task::spawn_local(async move {
            // `try_*` throughout: navigating away disposes this component while
            // the request is in flight, and writing a disposed signal panics the
            // whole client.
            match api.update_wallet(&wallet_id.to_string(), &req).await {
                Ok(_) => {
                    let _ = set_message.try_set(Some((true, "Saved.".to_string())));
                    let _ = set_promote.try_set(false);
                }
                Err(e) => {
                    let _ = set_message.try_set(Some((false, format!("Save failed: {e}"))));
                }
            }
            let _ = set_saving.try_set(false);
        });
    };

    let on_delete = move |_| {
        let confirmed = web_sys::window()
            .and_then(|w| {
                w.confirm_with_message(
                    "Delete this wallet? Addresses it has already issued stay valid and any \
                     funds sent to them remain yours - the key is derived from your xpub, not \
                     held here. Stores still pointing at this wallet will refuse to derive new \
                     addresses until you give them another.",
                )
                .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        let api = api.get();
        let navigate = navigate.clone();
        set_deleting.set(true);
        set_message.set(None);
        leptos::task::spawn_local(async move {
            match api.delete_wallet(&wallet_id.to_string()).await {
                Ok(()) => navigate("/evm/wallets", Default::default()),
                Err(e) => {
                    // The server refuses while a store still derives from this
                    // wallet, so this is a real answer, not a failure to report.
                    let _ = set_message.try_set(Some((false, format!("Delete failed: {e}"))));
                    let _ = set_deleting.try_set(false);
                }
            }
        });
    };

    view! {
        <div class="wallet-tab-general">
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Wallet Information"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="form-group">
                        <label class="form-label">"Wallet Name"</label>
                        <input
                            type="text"
                            class="form-input"
                            prop:value=move || name.get()
                            on:input=move |ev| set_name.set(event_target_value(&ev))
                        />
                        <p class="form-help">"A friendly name to identify this wallet"</p>
                    </div>

                    <div class="form-group">
                        <label class="form-label">"Extended Public Key"</label>
                        <div class="form-static">
                            <code class="wallet-address-full">{wallet.xpub_masked.clone()}</code>
                            <button
                                class="ps-btn ps-btn-ghost ps-btn-sm ps-btn-icon"
                                on:click=on_copy
                                title="Copy the masked key shown here"
                            >
                                <IconCopy />
                            </button>
                            {move || copied.get().map(|ok| if ok {
                                view! { <span class="text-muted">"Copied"</span> }
                            } else {
                                view! { <span class="text-error">"Clipboard refused - press Ctrl+C"</span> }
                            })}
                        </div>
                        <p class="form-help">"Masked xpub used for HD address derivation"</p>
                    </div>

                    <div class="form-group">
                        <label class="form-label">"Derivation Index"</label>
                        <div class="form-static">
                            <code>{wallet.derivation_index}</code>
                        </div>
                        <p class="form-help">"Next BIP-44 index for address generation"</p>
                    </div>

                    {move || (!already_primary).then(|| view! {
                        <div class="form-group">
                            <label class="form-label">"Account primary"</label>
                            <label class="toggle">
                                <input
                                    type="checkbox"
                                    prop:checked=move || promote.get()
                                    on:change=move |ev| set_promote.set(event_target_checked(&ev))
                                />
                                <span class="toggle-slider"></span>
                            </label>
                            <p class="form-help">
                                "Stores with no wallet of their own derive from the account \
                                 primary. Promoting this wallet demotes the current one."
                            </p>
                        </div>
                    })}

                    {move || message.get().map(|(ok, msg)| {
                        let style = if ok { "color: var(--color-success)" } else { "color: var(--color-error)" };
                        view! { <p style=style>{msg}</p> }
                    })}

                    <div class="form-actions">
                        <button
                            class="ps-btn ps-btn-primary ps-btn-sm"
                            on:click=on_save
                            disabled=move || saving.get() || deleting.get()
                        >
                            {move || if saving.get() { "Saving..." } else { "Save changes" }}
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
                            <span class="danger-action-title">"Delete this wallet"</span>
                            <span class="danger-action-desc">"Remove this wallet configuration. This will not affect any funds."</span>
                        </div>
                        <button
                            class="ps-btn ps-btn-danger ps-btn-sm"
                            on:click=on_delete
                            disabled=move || deleting.get() || saving.get()
                        >
                            {move || if deleting.get() { "Deleting..." } else { "Delete wallet" }}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
