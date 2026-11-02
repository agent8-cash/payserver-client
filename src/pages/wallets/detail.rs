use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::api::ApiClient;

use super::addresses_tab::AddressesTab;
use super::format_date;
use super::general_tab::GeneralTab;
use super::icons::*;

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
