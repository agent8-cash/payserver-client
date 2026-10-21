//! Wallet management pages.
//!
//! Wallets contain HD wallet (xpub) configurations that can be used
//! across stores for payment method setup.

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params_map};

use crate::api::{ApiClient, CreateWalletRequest, UpdateWalletRequest, Wallet};
use crate::util::receiving_key::evm_account_xpub;

/// Format ISO date string for display.
fn format_date(iso: &str) -> String {
    if iso.len() >= 10 {
        let date_part = &iso[..10];
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            let month = match parts[1] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                _ => parts[1],
            };
            return format!("{} {}, {}", month, parts[2], parts[0]);
        }
    }
    iso.to_string()
}

/// Truncate address for display.
fn truncate_address(address: &str) -> String {
    if address.len() > 16 {
        format!("{}...{}", &address[..8], &address[address.len() - 6..])
    } else {
        address.to_string()
    }
}

/// Wallets list page.
#[component]
pub fn WalletsPage() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    let (refresh, set_refresh) = signal(0u32);
    let wallets_resource = LocalResource::new(move || {
        refresh.track();
        let api = api.get();
        async move { api.list_wallets().await }
    });

    let (show_form, set_show_form) = signal(false);
    let (xpub, set_xpub) = signal(String::new());
    let (wallet_name, set_wallet_name) = signal(String::new());
    let (creating, set_creating) = signal(false);
    let (create_error, set_create_error) = signal(Option::<String>::None);

    let on_create = move |_| {
        let xpub_value = xpub.get_untracked().trim().to_string();
        if xpub_value.is_empty() {
            return;
        }
        let name = wallet_name.get_untracked().trim().to_string();
        let api = api.get();
        set_creating.set(true);
        set_create_error.set(None);
        leptos::task::spawn_local(async move {
            let req = CreateWalletRequest {
                xpub: xpub_value,
                name: (!name.is_empty()).then_some(name),
                // The same value the server applies when the field is
                // absent, so this changes nothing about what this form has
                // always created. It is spelled out because the field is now
                // required by the type and a silent `..Default::default()`
                // would hide that this form does not ask.
                //
                // It should ask. An account-level xpub has its BIP-44 coin
                // type baked in and cannot be told apart from another
                // family's by looking at it, so a merchant pasting a Tron
                // key here gets valid-looking addresses their wallet never
                // watches. Adding that control is its own change.
                namespace: "eip155".to_string(),
            };
            match api.create_wallet(&req).await {
                Ok(_) => {
                    let _ = set_xpub.try_set(String::new());
                    let _ = set_wallet_name.try_set(String::new());
                    let _ = set_show_form.try_set(false);
                    let _ = set_refresh.try_update(|c| *c += 1);
                }
                Err(e) => {
                    // Includes the 409 for an xpub already registered to another
                    // account. Surfaced verbatim rather than flattened: "that key
                    // belongs to someone else" is the one thing a merchant needs
                    // to read here.
                    let _ = set_create_error.try_set(Some(e.to_string()));
                }
            }
            let _ = set_creating.try_set(false);
        });
    };

    // Opt-in: the receiving key is derived from a recovery phrase the merchant
    // types in again, here, and only the resulting xpub is sent. Re-entry is
    // deliberate: it makes the choice an explicit act, and it works long after
    // signup rather than only while the phrase is still on screen.
    let (show_phrase_form, set_show_phrase_form) = signal(false);
    let (phrase, set_phrase) = signal(String::new());
    let (phrase_name, set_phrase_name) = signal(String::new());
    let (phrase_ack, set_phrase_ack) = signal(false);
    let (phrase_busy, set_phrase_busy) = signal(false);
    let (phrase_error, set_phrase_error) = signal(Option::<String>::None);

    let on_create_from_phrase = move |_| {
        let derived = evm_account_xpub(&phrase.get_untracked());
        // The phrase is a secret: drop it from the field whatever the outcome.
        set_phrase.set(String::new());
        let xpub_value = match derived {
            Ok(x) => x,
            Err(e) => {
                set_phrase_error.set(Some(e.to_string()));
                return;
            }
        };
        let name = phrase_name.get_untracked().trim().to_string();
        let api = api.get();
        set_phrase_busy.set(true);
        set_phrase_error.set(None);
        leptos::task::spawn_local(async move {
            let req = CreateWalletRequest {
                xpub: xpub_value,
                name: (!name.is_empty()).then_some(name),
                namespace: "eip155".to_string(),
            };
            match api.create_wallet(&req).await {
                Ok(_) => {
                    let _ = set_phrase_name.try_set(String::new());
                    let _ = set_phrase_ack.try_set(false);
                    let _ = set_show_phrase_form.try_set(false);
                    let _ = set_refresh.try_update(|c| *c += 1);
                }
                Err(e) => {
                    let _ = set_phrase_error.try_set(Some(e.to_string()));
                }
            }
            let _ = set_phrase_busy.try_set(false);
        });
    };

    view! {
        <div class="wallets-page">
            // Page Header
            <div class="page-header-row">
                <div>
                    <h1 class="page-title">"Wallets"</h1>
                    <p class="page-description">"Manage HD wallets for receiving payments"</p>
                </div>
                <div class="page-actions">
                    <button
                        class="ps-btn ps-btn-primary ps-btn-sm"
                        on:click=move |_| set_show_form.update(|v| *v = !*v)
                    >
                        <IconPlus />
                        {move || if show_form.get() { "Cancel" } else { "Add wallet" }}
                    </button>
                    <button
                        class="ps-btn ps-btn-secondary ps-btn-sm"
                        on:click=move |_| {
                            set_phrase.set(String::new());
                            set_show_phrase_form.update(|v| *v = !*v)
                        }
                    >
                        {move || if show_phrase_form.get() { "Cancel" } else { "Use my recovery phrase" }}
                    </button>
                </div>
            </div>

            {move || show_form.get().then(|| view! {
                <div class="ps-card">
                    <div class="ps-card-header"><h3>"Add a wallet"</h3></div>
                    <div class="ps-card-body">
                        <div class="form-group">
                            <label class="form-label">"Extended public key (xpub)"</label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="xpub..."
                                prop:value=move || xpub.get()
                                on:input=move |ev| set_xpub.set(event_target_value(&ev))
                            />
                            <p class="form-help">
                                "An account-level xpub (m/44\u{2019}/60\u{2019}/0\u{2019}). Addresses are derived \
                                 from it here; the private key never leaves your wallet. One \
                                 xpub belongs to one account."
                            </p>
                        </div>
                        <div class="form-group">
                            <label class="form-label">"Name (optional)"</label>
                            <input
                                type="text"
                                class="form-input"
                                prop:value=move || wallet_name.get()
                                on:input=move |ev| set_wallet_name.set(event_target_value(&ev))
                            />
                        </div>
                        {move || create_error.get().map(|msg| view! {
                            <p style="color: var(--color-error)">{msg}</p>
                        })}
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                on:click=on_create
                                disabled=move || creating.get() || xpub.get().trim().is_empty()
                            >
                                {move || if creating.get() { "Adding..." } else { "Add wallet" }}
                            </button>
                        </div>
                    </div>
                </div>
            })}

            {move || show_phrase_form.get().then(|| view! {
                <div class="ps-card">
                    <div class="ps-card-header"><h3>"Use my recovery phrase as a receiving wallet"</h3></div>
                    <div class="ps-card-body">
                        <p class="form-help">
                            "This makes one secret do two jobs. The phrase already recovers your \
                             account; if you use it here too, its extended public key will also \
                             disclose every receiving address you will ever use and your whole \
                             payment history to anyone who obtains it. It cannot spend funds, and \
                             the phrase itself never leaves this browser, but you cannot undo \
                             this once addresses are published. If you want the account and your \
                             payments kept separate, cancel and add an xpub from a different \
                             wallet instead."
                        </p>
                        <div class="form-group">
                            <label class="form-label">"Recovery phrase"</label>
                            <textarea
                                class="form-input"
                                rows="3"
                                autocomplete="off"
                                spellcheck="false"
                                placeholder="Enter your 24 words"
                                prop:value=move || phrase.get()
                                on:input=move |ev| set_phrase.set(event_target_value(&ev))
                            ></textarea>
                        </div>
                        <div class="form-group">
                            <label class="form-label">"Name (optional)"</label>
                            <input
                                type="text"
                                class="form-input"
                                prop:value=move || phrase_name.get()
                                on:input=move |ev| set_phrase_name.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="form-group">
                            <label>
                                <input
                                    type="checkbox"
                                    prop:checked=move || phrase_ack.get()
                                    on:change=move |ev| set_phrase_ack.set(event_target_checked(&ev))
                                />
                                " I understand this phrase will also determine my receiving addresses"
                            </label>
                        </div>
                        {move || phrase_error.get().map(|msg| view! {
                            <p style="color: var(--color-error)">{msg}</p>
                        })}
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                on:click=on_create_from_phrase
                                disabled=move || {
                                    phrase_busy.get()
                                        || !phrase_ack.get()
                                        || phrase.get().trim().is_empty()
                                }
                            >
                                {move || if phrase_busy.get() { "Adding..." } else { "Add wallet from phrase" }}
                            </button>
                        </div>
                    </div>
                </div>
            })}

            // Wallets Grid
            <Suspense fallback=move || view! { <div class="loading-state">"Loading wallets..."</div> }>
                {move || {
                    wallets_resource.get().map(|result| match &*result {
                        Ok(wallets) if wallets.is_empty() => {
                            view! { <WalletsEmpty on_add=Callback::new(move |()| set_show_form.set(true)) /> }.into_any()
                        }
                        Ok(wallets) => {
                            let wallets = wallets.clone();
                            view! {
                                <div class="wallets-grid">
                                    {wallets.into_iter().map(|wallet| {
                                        view! { <WalletCard wallet=wallet /> }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            view! { <div class="error-state">"Error loading wallets: "{msg}</div> }.into_any()
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

/// Empty state for wallets.
#[component]
fn WalletsEmpty(on_add: Callback<()>) -> impl IntoView {
    view! {
        <div class="wallets-empty">
            <IconWalletLarge />
            <h3>"No wallets configured"</h3>
            <p>"Add an HD wallet to start receiving cryptocurrency payments"</p>
            <button
                class="ps-btn ps-btn-primary ps-btn-sm"
                on:click=move |_| on_add.run(())
            >
                "Add your first wallet"
            </button>
        </div>
    }
}

/// Wallet card component.
#[component]
fn WalletCard(wallet: Wallet) -> impl IntoView {
    let wallet_link = wallet.id;
    let wallet_name = wallet
        .name
        .clone()
        .unwrap_or_else(|| "Unnamed Wallet".to_string());
    let xpub_display = truncate_address(&wallet.xpub_masked);
    let created_display = format_date(&wallet.created_at.to_rfc3339());

    view! {
        <A href=format!("/evm/wallets/{}", wallet_link) attr:class="wallet-card">
            <div class="wallet-card-header">
                <div class="wallet-card-icon">
                    <IconWallet />
                </div>
                <div class="wallet-card-title">
                    <h3 class="wallet-card-name">{wallet_name}</h3>
                </div>
            </div>

            <div class="wallet-card-body">
                <div class="wallet-card-address">
                    <code>{xpub_display}</code>
                </div>

                <div class="wallet-card-meta">
                    <div class="wallet-card-row">
                        <span class="wallet-card-label">"Derivation Index"</span>
                        <code class="wallet-card-path">{wallet.derivation_index}</code>
                    </div>
                </div>
            </div>

            <div class="wallet-card-footer">
                <span class="wallet-card-date">"Created "{created_display}</span>
                <IconChevronRight />
            </div>
        </A>
    }
}

/// Wallet detail page.
#[component]
pub fn WalletDetailPage() -> impl IntoView {
    let params = use_params_map();
    let wallet_id = move || params.get().get("id").unwrap_or_default();

    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    let wallet_resource = LocalResource::new(move || {
        let api = api.get();
        let id = wallet_id();
        async move { api.get_wallet(&id).await }
    });

    let (exporting, set_exporting) = signal(false);

    // Export fetches the FULL xpub and copies it. Every other view of the key
    // is masked, so this has to be an explicit act by the merchant - which is
    // why it is its own endpoint and its own button rather than something the
    // page loads to render.
    let on_export = move |_| {
        let api = api.get();
        let id = wallet_id();
        set_exporting.set(true);
        leptos::task::spawn_local(async move {
            match api.export_wallet_xpub(&id).await {
                Ok(res) => {
                    // The alert stays: it is a security disclosure about what the
                    // full key exposes, not a copy confirmation, and it should be
                    // read before the key is pasted anywhere.
                    //
                    // What changes is that the claim is now true. The write was
                    // never awaited, so a refused clipboard still produced "The
                    // full xpub is on your clipboard" - a false statement about a
                    // key the merchant then believes they hold.
                    let copied = ui_kit::copy_to_clipboard(&res.xpub).await;
                    if let Some(window) = web_sys::window() {
                        let _ = window.alert_with_message(if copied {
                            "The full xpub is on your clipboard. It derives every address this \
                             wallet issues, so anyone holding it can see your incoming payments. \
                             It cannot move funds."
                        } else {
                            "Your browser refused the clipboard, so the key was NOT copied. \
                             Nothing was exposed. Try again from a normal browser tab."
                        });
                    }
                }
                Err(e) => {
                    web_sys::window()
                        .and_then(|w| w.alert_with_message(&format!("Export failed: {e}")).ok());
                }
            }
            let _ = set_exporting.try_set(false);
        });
    };

    // Active tab state
    let (active_tab, set_active_tab) = signal("general".to_string());

    let tabs = vec![("general", "General"), ("addresses", "Addresses")];

    view! {
        <div class="wallet-detail-page">
            <Suspense fallback=move || view! { <div class="loading-state">"Loading wallet..."</div> }>
                {move || {
                    let active_tab = active_tab;
                    let set_active_tab = set_active_tab;
                    let tabs = tabs.clone();
                    wallet_resource.get().map(move |result| {
                        match &*result {
                            Ok(wallet) => {
                                let wallet = wallet.clone();
                                let wallet_name = wallet.name.clone().unwrap_or_else(|| "Unnamed Wallet".to_string());
                                let created_display = format_date(&wallet.created_at.to_rfc3339());

                                view! {
                                    <div>
                                        // Header
                                        <div class="wallet-detail-header">
                                            <div class="wallet-detail-header-left">
                                                <A href="/evm/wallets" attr:class="back-link">
                                                    <IconArrowLeft />
                                                    "Wallets"
                                                </A>
                                                <div class="wallet-detail-title-row">
                                                    <h1 class="wallet-detail-title">{wallet_name}</h1>
                                                </div>
                                                <p class="wallet-detail-subtitle">
                                                    <code class="wallet-detail-id">{wallet.id.to_string()}</code>
                                                    " · Created "{created_display}
                                                </p>
                                            </div>
                                            <div class="wallet-detail-actions">
                                                <button
                                                    class="ps-btn ps-btn-secondary ps-btn-sm"
                                                    on:click=on_export
                                                    disabled=move || exporting.get()
                                                    title="Copy the full xpub to your clipboard"
                                                >
                                                    <IconDownload />
                                                    {move || if exporting.get() { "Exporting..." } else { "Export" }}
                                                </button>
                                            </div>
                                        </div>

                                        // Tabs
                                        <div class="wallet-tabs">
                                            {tabs.into_iter().map(|(key, label)| {
                                                let key_owned = key.to_string();
                                                let key_for_click = key.to_string();
                                                view! {
                                                    <button
                                                        class=move || if active_tab.get() == key_owned { "wallet-tab active" } else { "wallet-tab" }
                                                        on:click=move |_| set_active_tab.set(key_for_click.clone())
                                                    >
                                                        {label}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>

                                        // Tab Content
                                        <div class="wallet-tab-content">
                                            {move || match active_tab.get().as_str() {
                                                "general" => view! { <GeneralTab wallet=wallet.clone() /> }.into_any(),
                                                "addresses" => view! { <AddressesTab wallet=wallet.clone() /> }.into_any(),
                                                _ => view! { <GeneralTab wallet=wallet.clone() /> }.into_any(),
                                            }}
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            Err(e) => {
                                let msg = e.to_string();
                                view! { <div class="error-state">"Error loading wallet: "{msg}</div> }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

/// General settings tab.
#[component]
fn GeneralTab(wallet: Wallet) -> impl IntoView {
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

/// Addresses tab - shows derived addresses.
#[component]
fn AddressesTab(wallet: Wallet) -> impl IntoView {
    // In production, addresses are derived server-side from the wallet's xpub.
    // For now, show the derivation index range that has been used.
    let used_count = wallet.derivation_index as usize;

    view! {
        <div class="wallet-tab-addresses">
            <div class="section-header">
                <div>
                    <h3 class="section-title">"Derived Addresses"</h3>
                    <p class="section-desc">"Addresses generated from your wallet's extended public key"</p>
                </div>
            </div>

            <div class="ps-card">
                <div class="ps-card-body">
                    <div class="form-group">
                        <label class="form-label">"Addresses Derived"</label>
                        <div class="form-static">
                            <code>{used_count}" addresses derived so far"</code>
                        </div>
                        <p class="form-help">"New addresses are derived automatically when invoices are created"</p>
                    </div>
                </div>
            </div>

            <div class="addresses-info">
                <IconInfo />
                <p>"Addresses are derived sequentially from your wallet's xpub using the BIP-44 standard. Each invoice uses a unique address for payment tracking."</p>
            </div>
        </div>
    }
}

// ============================================
// Icons
// ============================================

#[component]
fn IconPlus() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
        </svg>
    }
}

#[component]
fn IconWallet() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12V7H5a2 2 0 0 1 0-4h14v4"></path>
            <path d="M3 5v14a2 2 0 0 0 2 2h16v-5"></path>
            <path d="M18 12a2 2 0 0 0 0 4h4v-4Z"></path>
        </svg>
    }
}

#[component]
fn IconWalletLarge() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12V7H5a2 2 0 0 1 0-4h14v4"></path>
            <path d="M3 5v14a2 2 0 0 0 2 2h16v-5"></path>
            <path d="M18 12a2 2 0 0 0 0 4h4v-4Z"></path>
        </svg>
    }
}

#[component]
fn IconChevronRight() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="9 18 15 12 9 6"></polyline>
        </svg>
    }
}

#[component]
fn IconArrowLeft() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="19" y1="12" x2="5" y2="12"></line>
            <polyline points="12 19 5 12 12 5"></polyline>
        </svg>
    }
}

#[component]
fn IconDownload() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
        </svg>
    }
}

#[component]
fn IconCopy() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
        </svg>
    }
}

#[component]
fn IconInfo() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="16" x2="12" y2="12"></line>
            <line x1="12" y1="8" x2="12.01" y2="8"></line>
        </svg>
    }
}
