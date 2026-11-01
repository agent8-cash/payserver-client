use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::{ApiClient, CreateWalletRequest, Wallet};
use crate::util::receiving_key::evm_account_xpub;

use super::format_date;
use super::icons::*;
use super::truncate_address;

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
