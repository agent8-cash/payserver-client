//! Settings tab component (defaults, branding, notifications, token policy).

use leptos::prelude::*;
use types::ChainId;

use crate::api::{
    ApiClient, SetTokenPolicyRequest, StoreSettings, TokenPolicyEntry, UpdateStoreSettingsRequest,
};
use crate::util::chain_name;

use super::event_target_checked;

/// Store settings tab: defaults, branding, notifications.
#[component]
pub fn SettingsTab(store_id: String) -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let store_id_for_load = store_id.clone();
    let store_id_for_save = store_id.clone();

    let (refresh, set_refresh) = signal(0u32);
    let settings_resource = LocalResource::new(move || {
        let api = api.get();
        let id = store_id_for_load.clone();
        refresh.get();
        async move { api.get_store_settings(&id).await }
    });

    // Form state
    let (chain_id, set_chain_id) = signal(String::new());
    let (display_currency, set_display_currency) = signal(String::new());
    let (logo_url, set_logo_url) = signal(String::new());
    let (accent_color, set_accent_color) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (success_msg, set_success_msg) = signal(false);

    // Notification prefs state (one bool per event)
    let (wh_payment_detected, set_wh_payment_detected) = signal(true);
    let (wh_payment_confirmed, set_wh_payment_confirmed) = signal(true);
    let (wh_invoice_expired, set_wh_invoice_expired) = signal(true);
    let (wh_invoice_cancelled, set_wh_invoice_cancelled) = signal(true);
    let (wh_late_paid, set_wh_late_paid) = signal(true);

    // Populate form when settings load
    let populate = move |s: &StoreSettings| {
        set_chain_id.set(
            s.default_chain_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
        );
        set_display_currency.set(s.default_display_currency.clone().unwrap_or_default());
        set_logo_url.set(s.logo_url.clone().unwrap_or_default());
        set_accent_color.set(s.accent_color.clone().unwrap_or_default());
        // Parse notification prefs
        let prefs = &s.notification_prefs;
        let get_wh = |key: &str| -> bool {
            prefs
                .get(key)
                .and_then(|v| v.get("webhook"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
        };
        set_wh_payment_detected.set(get_wh("payment_detected"));
        set_wh_payment_confirmed.set(get_wh("payment_confirmed"));
        set_wh_invoice_expired.set(get_wh("invoice_expired"));
        set_wh_invoice_cancelled.set(get_wh("invoice_cancelled"));
        set_wh_late_paid.set(get_wh("late_paid"));
    };

    let on_save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_saving.set(true);
        set_error_msg.set(None);
        set_success_msg.set(false);

        let api = api.get();
        let id = store_id_for_save.clone();

        let chain = chain_id.get();
        let currency = display_currency.get();
        let logo = logo_url.get();
        let color = accent_color.get();

        // Build notification prefs JSON
        let prefs = serde_json::json!({
            "payment_detected": {"webhook": wh_payment_detected.get()},
            "payment_confirmed": {"webhook": wh_payment_confirmed.get()},
            "invoice_expired": {"webhook": wh_invoice_expired.get()},
            "invoice_cancelled": {"webhook": wh_invoice_cancelled.get()},
            "late_paid": {"webhook": wh_late_paid.get()},
        });

        let req = UpdateStoreSettingsRequest {
            default_chain_id: if chain.is_empty() {
                None
            } else {
                chain.parse().ok()
            },
            default_display_currency: if currency.is_empty() {
                None
            } else {
                Some(currency)
            },
            logo_url: if logo.is_empty() { None } else { Some(logo) },
            accent_color: if color.is_empty() { None } else { Some(color) },
            notification_prefs: Some(prefs),
        };

        leptos::task::spawn_local(async move {
            match api.update_store_settings(&id, &req).await {
                Ok(_) => {
                    set_success_msg.set(true);
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_error_msg.set(Some(format!("{}", e))),
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="settings-tab">
            <Suspense fallback=move || view! { <p style="color: var(--text-muted);">"Loading settings..."</p> }>
                {move || settings_resource.get().map(|result| match &*result {
                    Ok(settings) => {
                        populate(settings);
                        view! { <div></div> }.into_any()
                    }
                    Err(_) => view! { <div></div> }.into_any(),
                })}
            </Suspense>

            <form on:submit=on_save>
                // General section
                <div class="card card-body">
                    <h3 class="card-title">"General"</h3>
                    <div class="form-group">
                        <label class="form-label" for="chain_id">"Default Chain"</label>
                        <select id="chain_id" class="form-input form-select"
                            prop:value=move || chain_id.get()
                            on:change=move |ev| set_chain_id.set(event_target_value(&ev))
                        >
                            <option value="">"No default"</option>
                            <option value="1">"Ethereum"</option>
                            <option value="137">"Polygon"</option>
                            <option value="42161">"Arbitrum"</option>
                            <option value="10">"Optimism"</option>
                            <option value="8453">"Base"</option>
                            <option value="56">"BSC"</option>
                            <option value="43114">"Avalanche"</option>
                            <option value="100">"Gnosis"</option>
                            <option value="250">"Fantom"</option>
                        </select>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="display_currency">"Default Display Currency"</label>
                        <select id="display_currency" class="form-input form-select"
                            prop:value=move || display_currency.get()
                            on:change=move |ev| set_display_currency.set(event_target_value(&ev))
                        >
                            <option value="">"No default"</option>
                            <option value="USD">"USD"</option>
                            <option value="EUR">"EUR"</option>
                            <option value="GBP">"GBP"</option>
                            <option value="BRL">"BRL"</option>
                            <option value="JPY">"JPY"</option>
                        </select>
                    </div>
                </div>

                // Branding section
                <div class="card card-body">
                    <h3 class="card-title">"Branding"</h3>
                    <div class="form-group">
                        <label class="form-label" for="logo_url">"Logo URL"</label>
                        <input type="url" id="logo_url" class="form-input" placeholder="https://example.com/logo.png"
                            prop:value=move || logo_url.get()
                            on:input=move |ev| set_logo_url.set(event_target_value(&ev))
                        />
                        <small class="form-help">"Must be HTTPS. Displayed on checkout page."</small>
                    </div>
                    <div class="form-group">
                        <label class="form-label" for="accent_color">"Accent Color"</label>
                        <div class="color-input-row">
                            <input type="color" class="color-swatch"
                                prop:value=move || {
                                    let c = accent_color.get();
                                    if c.is_empty() { "#6366f1".to_string() } else { c }
                                }
                                on:input=move |ev| set_accent_color.set(event_target_value(&ev))
                            />
                            <input type="text" id="accent_color" class="form-input" placeholder="#6366f1" maxlength="7"
                                prop:value=move || accent_color.get()
                                on:input=move |ev| set_accent_color.set(event_target_value(&ev))
                            />
                        </div>
                        <small class="form-help">"Hex color for checkout button and header accent."</small>
                    </div>
                    {move || {
                        let logo = logo_url.get();
                        let color = accent_color.get();
                        let color_val = if color.is_empty() { "#6366f1".to_string() } else { color };
                        if !logo.is_empty() || !accent_color.get().is_empty() {
                            view! {
                                <div class="branding-preview" style=format!("border-top: 3px solid {};", color_val)>
                                    {(!logo.is_empty()).then(|| view! {
                                        <img src=logo alt="Logo preview" style="max-height: 40px; margin-bottom: 0.5rem;" />
                                    })}
                                    <span style="font-size: 0.875rem; color: var(--text-muted);">"Checkout header preview"</span>
                                </div>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    }}
                </div>

                // Notifications section
                <div class="card card-body">
                    <h3 class="card-title">"Notifications"</h3>
                    <p class="card-subtitle">"Control which events trigger webhook delivery."</p>
                    <div class="notification-list">
                        <label class="checkbox-label">
                            <input type="checkbox"
                                prop:checked=move || wh_payment_detected.get()
                                on:change=move |ev| set_wh_payment_detected.set(event_target_checked(&ev))
                            />
                            <span>"Payment Detected"</span>
                        </label>
                        <label class="checkbox-label">
                            <input type="checkbox"
                                prop:checked=move || wh_payment_confirmed.get()
                                on:change=move |ev| set_wh_payment_confirmed.set(event_target_checked(&ev))
                            />
                            <span>"Payment Confirmed"</span>
                        </label>
                        <label class="checkbox-label">
                            <input type="checkbox"
                                prop:checked=move || wh_invoice_expired.get()
                                on:change=move |ev| set_wh_invoice_expired.set(event_target_checked(&ev))
                            />
                            <span>"Invoice Expired"</span>
                        </label>
                        <label class="checkbox-label">
                            <input type="checkbox"
                                prop:checked=move || wh_invoice_cancelled.get()
                                on:change=move |ev| set_wh_invoice_cancelled.set(event_target_checked(&ev))
                            />
                            <span>"Invoice Cancelled"</span>
                        </label>
                        <label class="checkbox-label">
                            <input type="checkbox"
                                prop:checked=move || wh_late_paid.get()
                                on:change=move |ev| set_wh_late_paid.set(event_target_checked(&ev))
                            />
                            <span>"Late Payment"</span>
                        </label>
                    </div>
                </div>

                // Error / success messages
                {move || error_msg.get().map(|msg| view! {
                    <div class="alert alert-error">{msg}</div>
                })}
                {move || success_msg.get().then(|| view! {
                    <div class="alert alert-success">"Settings saved."</div>
                })}

                <div class="form-actions">
                    <button type="submit" class="ps-btn ps-btn-primary" disabled=move || saving.get()>
                        {move || if saving.get() { "Saving..." } else { "Save Settings" }}
                    </button>
                </div>
            </form>

            <TokenPolicyPanel store_id=store_id.clone() />
        </div>
    }
}

/// Token policy panel: allowlist/blocklist management.
#[component]
fn TokenPolicyPanel(store_id: String) -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let store_id_load = store_id.clone();
    let store_id_save = store_id.clone();
    let store_id_del = store_id.clone();

    let (refresh, set_refresh) = signal(0u32);

    let policy_resource = LocalResource::new(move || {
        let api = api.get();
        let id = store_id_load.clone();
        refresh.get();
        async move { api.get_token_policy(&id).await }
    });

    // Form state
    let (mode, set_mode) = signal("allowlist".to_string());
    let (entries, set_entries) = signal(Vec::<TokenPolicyEntry>::new());
    let (has_policy, set_has_policy) = signal(false);
    let (saving, set_saving) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (success_msg, set_success_msg) = signal(false);

    // New entry form
    let (new_chain_id, set_new_chain_id) = signal(String::new());
    let (new_token_address, set_new_token_address) = signal(String::new());
    let (new_asset_symbol, set_new_asset_symbol) = signal(String::new());

    let populate = move |loaded: &Option<crate::api::TokenPolicy>| match loaded {
        Some(p) => {
            set_mode.set(p.mode.clone());
            set_entries.set(p.entries.clone());
            set_has_policy.set(true);
        }
        None => {
            set_mode.set("allowlist".to_string());
            set_entries.set(Vec::new());
            set_has_policy.set(false);
        }
    };

    let add_entry = move |_| {
        // Same forgiving parse as the payment-method form: a CAIP-2 identifier
        // or a bare EIP-155 number.
        let raw = new_chain_id.get();
        let Some(chain) = ChainId::parse(raw.trim())
            .ok()
            .or_else(|| raw.trim().parse::<u64>().ok().map(ChainId::evm))
        else {
            return;
        };
        let symbol = new_asset_symbol.get();
        if symbol.is_empty() {
            return;
        }
        let addr = {
            let a = new_token_address.get();
            if a.is_empty() { None } else { Some(a) }
        };
        let is_dup = entries
            .get()
            .iter()
            .any(|e| e.chain_id == chain && e.token_address == addr && e.asset_symbol == symbol);
        if is_dup {
            return;
        }
        set_entries.update(|v| {
            v.push(TokenPolicyEntry {
                chain_id: chain,
                token_address: addr,
                asset_symbol: symbol,
            });
        });
        set_new_chain_id.set(String::new());
        set_new_token_address.set(String::new());
        set_new_asset_symbol.set(String::new());
    };

    let on_save = move |_: leptos::ev::MouseEvent| {
        set_saving.set(true);
        set_error_msg.set(None);
        set_success_msg.set(false);

        let api = api.get();
        let id = store_id_save.clone();
        let req = SetTokenPolicyRequest {
            mode: mode.get(),
            entries: entries.get(),
        };

        leptos::task::spawn_local(async move {
            match api.set_token_policy(&id, &req).await {
                Ok(_) => {
                    set_success_msg.set(true);
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_error_msg.set(Some(format!("{}", e))),
            }
            set_saving.set(false);
        });
    };

    let on_delete = move |_: leptos::ev::MouseEvent| {
        let confirmed = web_sys::window()
            .and_then(|w| {
                w.confirm_with_message(
                    "Remove token policy? This will revert the store to accepting all tokens.",
                )
                .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }

        set_saving.set(true);
        set_error_msg.set(None);
        set_success_msg.set(false);

        let api = api.get();
        let id = store_id_del.clone();

        leptos::task::spawn_local(async move {
            match api.delete_token_policy(&id).await {
                Ok(_) => {
                    set_has_policy.set(false);
                    set_entries.set(Vec::new());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_error_msg.set(Some(format!("{}", e))),
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="card card-body">
            <h3 class="card-title">"Token Policy"</h3>
            <p class="card-subtitle">
                "Control which chain+token pairs this store accepts for payments."
            </p>

            <Suspense fallback=move || view! { <p style="color: var(--text-muted);">"Loading policy..."</p> }>
                {move || policy_resource.get().map(|result| match &*result {
                    Ok(policy) => {
                        populate(policy);
                        view! { <div></div> }.into_any()
                    }
                    Err(_) => view! { <div></div> }.into_any(),
                })}
            </Suspense>

            <div class="form-group">
                <label class="form-label" for="policy_mode">"Mode"</label>
                <select id="policy_mode" class="form-input form-select"
                    prop:value=move || mode.get()
                    on:change=move |ev| set_mode.set(event_target_value(&ev))
                >
                    <option value="allowlist">"Allowlist (only listed tokens accepted)"</option>
                    <option value="blocklist">"Blocklist (all except listed tokens)"</option>
                </select>
            </div>

            // Entries table
            <div class="policy-table">
                <div class="policy-row policy-row-head">
                    <span style="flex: 1;">"Chain ID"</span>
                    <span style="flex: 2;">"Token Address"</span>
                    <span style="flex: 1;">"Symbol"</span>
                    <span style="width: 40px;"></span>
                </div>
                <For
                    each=move || entries.get()
                    key=|e| format!("{}:{:?}:{}", e.chain_id, e.token_address, e.asset_symbol)
                    children=move |entry| {
                        let entry_key = format!("{}:{:?}:{}", entry.chain_id, entry.token_address, entry.asset_symbol);
                        let remove = move |_: leptos::ev::MouseEvent| {
                            let key = entry_key.clone();
                            set_entries.update(|v| {
                                v.retain(|e| format!("{}:{:?}:{}", e.chain_id, e.token_address, e.asset_symbol) != key);
                            });
                        };
                        view! {
                            <div class="policy-row">
                                <span style="flex: 1;">{entry.chain_id.to_string()}</span>
                                <span style="flex: 2; font-family: monospace; font-size: 0.8rem;">
                                    {entry.token_address.clone().unwrap_or_else(|| "(native)".to_string())}
                                </span>
                                <span style="flex: 1;">{entry.asset_symbol.clone()}</span>
                                <button type="button" class="ps-btn ps-btn-sm ps-btn-danger" style="width: 40px;" on:click=remove>"X"</button>
                            </div>
                        }
                    }
                />
            </div>

            // Add entry form
            <div class="policy-add">
                <div style="flex: 1;">
                    <label class="form-label">"Chain ID"</label>
                    <input type="number" class="form-input" placeholder="1"
                        prop:value=move || new_chain_id.get()
                        on:input=move |ev| set_new_chain_id.set(event_target_value(&ev))
                    />
                    <span class="form-help">
                        {move || {
                            let raw = new_chain_id.get();
                            ChainId::parse(raw.trim())
                                .ok()
                                .or_else(|| raw.trim().parse::<u64>().ok().map(ChainId::evm))
                                .map(|c| chain_name(&c).to_string())
                                .unwrap_or_default()
                        }}
                    </span>
                </div>
                <div style="flex: 2;">
                    <label class="form-label">"Token Address"</label>
                    <input type="text" class="form-input" placeholder="0x... (empty for native)"
                        prop:value=move || new_token_address.get()
                        on:input=move |ev| set_new_token_address.set(event_target_value(&ev))
                    />
                </div>
                <div style="flex: 1;">
                    <label class="form-label">"Symbol"</label>
                    <input type="text" class="form-input" placeholder="USDC"
                        prop:value=move || new_asset_symbol.get()
                        on:input=move |ev| set_new_asset_symbol.set(event_target_value(&ev))
                    />
                </div>
                <button type="button" class="ps-btn ps-btn-secondary" on:click=add_entry>"Add"</button>
            </div>

            {move || error_msg.get().map(|msg| view! {
                <div class="alert alert-error" style="margin-top: 0.5rem;">{msg}</div>
            })}
            {move || success_msg.get().then(|| view! {
                <div class="alert alert-success" style="margin-top: 0.5rem;">"Token policy saved."</div>
            })}

            <div class="form-actions">
                <button type="button" class="ps-btn ps-btn-primary"
                    disabled=move || saving.get()
                    on:click=on_save
                >
                    {move || if saving.get() { "Saving..." } else { "Save Policy" }}
                </button>
                <button type="button" class="ps-btn ps-btn-danger"
                    style=move || if has_policy.get() { "margin-left: 0.5rem;" } else { "display: none;" }
                    disabled=move || saving.get()
                    on:click=on_delete
                >
                    "Remove Policy"
                </button>
            </div>
        </div>
    }
}
