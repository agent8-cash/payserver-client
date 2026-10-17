//! Admin settings tab - server settings and user management (admin only).

use crate::api::{
    AdminUserInfo, ApiClient, ApiError, SafeModeStatus, Store, UpdateServerSettingsRequest,
    UpdateUserRoleRequest, UserListResponse,
};
use crate::components::{PAGE_SIZE, Pagination};
use leptos::prelude::*;
use types::ChainId;

use super::IconShield;

/// Which safe-mode banner, if any, the admin tab should show.
///
/// Active always wins over CheckFailed: once a check has confirmed safe mode
/// is on, a later transient fetch error must not downgrade that to "unknown".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeModeBanner {
    Active,
    CheckFailed,
    None,
}

fn safe_mode_banner(safe_mode: bool, check_failed: bool) -> SafeModeBanner {
    if safe_mode {
        SafeModeBanner::Active
    } else if check_failed {
        SafeModeBanner::CheckFailed
    } else {
        SafeModeBanner::None
    }
}

/// What the two safe-mode signals should become after a fresh check result.
///
/// A success always clears a prior "could not confirm" warning - a stale
/// failure must not survive the check that just worked. A failure never
/// reports a `safe_mode` value: a transient error (network blip, session
/// hiccup) must read as "unknown", not be mistaken for "plugins are fine".
fn safe_mode_after_check(result: &Result<SafeModeStatus, ApiError>) -> (Option<bool>, bool) {
    match result {
        Ok(status) => (Some(status.safe_mode), false),
        Err(_) => (None, true),
    }
}

/// Apply a safe-mode check result to the tab's two signals.
///
/// Pulled out of `AdminTab`'s load-on-mount task so the `if let Some` guard -
/// the thing that actually leaves `safe_mode` untouched on a transient error,
/// as opposed to `safe_mode_after_check` merely saying it should - runs
/// against real signals in a test, not just the pure tuple it's fed.
fn apply_safe_mode_check(
    result: &Result<SafeModeStatus, ApiError>,
    set_safe_mode: WriteSignal<bool>,
    set_safe_mode_check_failed: WriteSignal<bool>,
) {
    let (mode, check_failed) = safe_mode_after_check(result);
    if let Some(mode) = mode {
        set_safe_mode.set(mode);
    }
    set_safe_mode_check_failed.set(check_failed);
}

/// How a user is labelled in the table: email, else a shortened wallet
/// address, else the start of the id.
fn user_display_name(user: &AdminUserInfo) -> String {
    user.email
        .clone()
        .or(user.primary_wallet_address.as_ref().map(|w| {
            let prefix = w.get(..6).unwrap_or(w.as_str());
            let suffix = w.get(w.len().saturating_sub(4)..).unwrap_or("");
            format!("{prefix}...{suffix}")
        }))
        .unwrap_or_else(|| user.id.get(..8).unwrap_or(user.id.as_str()).to_string())
}

/// The offset to reload after the server reports `total` users.
///
/// A page that no longer exists (users removed since it was loaded) falls back
/// to the last page that does, so the table never sits empty under a badge
/// that says there are users.
fn clamp_user_offset(offset: i64, total: i64) -> i64 {
    if total <= 0 {
        return 0;
    }
    if offset < total {
        return offset.max(0);
    }
    ((total - 1) / PAGE_SIZE) * PAGE_SIZE
}

/// Fetch the page at `offset`, refetching once at the last page that exists if
/// the first response shows `offset` is past the end. Returns the offset the
/// rows actually belong to, so offset and rows cannot disagree. A failure on
/// either request is returned, never swallowed.
async fn fetch_user_page<F, Fut>(
    offset: i64,
    mut fetch: F,
) -> Result<(i64, UserListResponse), ApiError>
where
    F: FnMut(i64) -> Fut,
    Fut: std::future::Future<Output = Result<UserListResponse, ApiError>>,
{
    let resp = fetch(offset).await?;
    let clamped = clamp_user_offset(offset, resp.total);
    if clamped == offset {
        return Ok((offset, resp));
    }
    // The requested page is past the end now.
    Ok((clamped, fetch(clamped).await?))
}

/// Admin tab - server settings and user management (admin only).
#[component]
pub fn AdminTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    // Safe mode state - true when the server booted with every plugin disabled.
    let (safe_mode, set_safe_mode) = signal(false);
    // Whether the safe-mode check itself failed - kept distinct from `safe_mode`
    // so a transient fetch error can't be mistaken for "plugins are fine".
    let (safe_mode_check_failed, set_safe_mode_check_failed) = signal(false);

    // Settings form state
    let (default_confirmations, set_default_confirmations) = signal("3".to_string());
    let (invoice_expiry, set_invoice_expiry) = signal("60".to_string());
    let (rate_limit, set_rate_limit) = signal("100".to_string());
    let (enabled_chain_ids, set_enabled_chain_ids) = signal(Vec::<ChainId>::new());
    // Whether the operator actually touched the chain list on this visit.
    //
    // The server treats a stored list as authoritative - every chain not in
    // it is refused - and answers `GET` with compiled-in *mainnet* defaults
    // when nothing is stored. Sending back whatever was loaded would
    // therefore write a list nobody chose, and on a testnet deployment that
    // list has no Sepolia in it, so the instance would stop accepting the
    // only chain it watches. Saving the operator store must not do that.
    let (chains_edited, set_chains_edited) = signal(false);
    let (settings_status, set_settings_status) = signal(String::new());

    // The operator's own store: where this instance issues its own invoices.
    // Empty string means none - an instance that issues no invoices to
    // itself.
    let (operator_store, set_operator_store) = signal(String::new());
    // Whether the saved value is the one the server is actually running with.
    // It is read at boot, so a change sits pending until a restart, and an
    // admin looking at this page needs to be able to tell the difference.
    let (operator_store_active, set_operator_store_active) = signal(true);
    let (stores, set_stores) = signal(Vec::<Store>::new());

    // User list state
    let (users, set_users) = signal(Vec::<AdminUserInfo>::new());
    let (user_total, set_user_total) = signal(0i64);
    let (user_offset, set_user_offset) = signal(0i64);
    let (user_status, set_user_status) = signal(String::new());

    // All available networks
    // EVM chains this server can be told to enable. The name is carried
    // alongside because a CAIP-2 identifier does not imply one.
    let all_networks: Vec<(ChainId, &'static str)> = vec![
        (ChainId::evm(1), "Ethereum"),
        (ChainId::evm(10), "Optimism"),
        (ChainId::evm(137), "Polygon"),
        (ChainId::evm(42161), "Arbitrum"),
        (ChainId::evm(8453), "Base"),
        (ChainId::evm(56), "BSC"),
        (ChainId::evm(43114), "Avalanche"),
        (ChainId::evm(250), "Fantom"),
        (ChainId::evm(100), "Gnosis"),
        (ChainId::evm(324), "zkSync Era"),
        (ChainId::evm(59144), "Linea"),
        (ChainId::evm(534352), "Scroll"),
    ];

    // Load settings and users on mount
    leptos::task::spawn_local({
        let api = api.get_untracked();
        async move {
            if let Ok(settings) = api.get_server_settings().await {
                set_default_confirmations.set(settings.default_confirmations.to_string());
                set_invoice_expiry.set(settings.invoice_expiry_minutes.to_string());
                set_rate_limit.set(settings.rate_limit_rpm.to_string());
                set_enabled_chain_ids.set(settings.enabled_chain_ids);
                set_operator_store.set(
                    settings
                        .operator_store_id
                        .map(|id| id.0.to_string())
                        .unwrap_or_default(),
                );
                set_operator_store_active.set(settings.operator_store_id_active);
            }
            // Offered as a list rather than a UUID field. An operator should
            // not have to copy an identifier out of a URL to configure where
            // their own revenue lands, and a typo there is silent until a
            // merchant cannot pay.
            if let Ok(list) = api.list_stores().await {
                set_stores.set(list.into_iter().filter(|s| !s.archived).collect());
            }
            let result = api.get_safe_mode().await;
            if let Err(ref e) = result {
                web_sys::console::error_1(&format!("safe-mode check failed: {e}").into());
            }
            apply_safe_mode_check(&result, set_safe_mode, set_safe_mode_check_failed);
        }
    });

    // Reload the page of users at `offset`. One path for the initial load,
    // paging, and the refresh after a role or lock change, so they cannot
    // disagree about which page is showing.
    //
    // Each call takes a ticket and only the newest ticket may write: rapid
    // paging, or a click racing the reload after a role change, would
    // otherwise let a slow stale response overwrite the page just chosen.
    let load_seq = StoredValue::new(0u64);
    let load_users = move |offset: i64| {
        let api = api.get_untracked();
        load_seq.update_value(|n| *n += 1);
        let ticket = load_seq.get_value();
        leptos::task::spawn_local(async move {
            let result = fetch_user_page(offset, |o| api.list_users(o, PAGE_SIZE)).await;
            if load_seq.get_value() != ticket {
                return;
            }
            match result {
                Ok((target, resp)) => {
                    // Offset, total and rows land together, from one response.
                    set_user_offset.set(target);
                    set_user_total.set(resp.total);
                    set_users.set(resp.users);
                }
                Err(e) => set_user_status.set(format!("Error: {}", e)),
            }
        });
    };
    load_users(0);

    // Save settings handler
    let save_settings = move |_| {
        let api = api.get_untracked();
        let confirmations = default_confirmations
            .get_untracked()
            .parse::<i32>()
            .unwrap_or(3);
        let expiry = invoice_expiry.get_untracked().parse::<i32>().unwrap_or(60);
        let rpm = rate_limit.get_untracked().parse::<i32>().unwrap_or(100);
        // `None` means "not talking about chains", which is not the same as
        // an empty list. Only a deliberate edit sends one.
        let chains = chains_edited
            .get_untracked()
            .then(|| enabled_chain_ids.get_untracked());
        let operator_store_value = operator_store.get_untracked();
        leptos::task::spawn_local(async move {
            // Always `Some(..)`: this form knows about the field, so it is
            // always talking about it. The inner option is the value - `None`
            // clears the setting. An absent field would mean "leave it
            // alone", which is what an older client sends and is not what a
            // save from this page means.
            let operator_store_id = Some(if operator_store_value.is_empty() {
                None
            } else {
                operator_store_value.parse().ok().map(types::StoreId)
            });

            let request = UpdateServerSettingsRequest {
                default_confirmations: confirmations,
                invoice_expiry_minutes: expiry,
                rate_limit_rpm: rpm,
                enabled_chain_ids: chains,
                operator_store_id,
            };
            match api.update_server_settings(&request).await {
                Ok(()) => {
                    set_chains_edited.set(false);
                    // Saved is not applied. Saying only "saved" would leave an
                    // admin believing the server is using the store they just
                    // picked, which it is not until it restarts.
                    set_settings_status.set(
                        "Settings saved. The operator store takes effect when the server restarts."
                            .to_string(),
                    );
                    set_operator_store_active.set(false);
                }
                Err(e) => set_settings_status.set(format!("Error: {}", e)),
            }
        });
    };

    // Toggle network handler
    let toggle_network = move |chain_id: ChainId| {
        set_chains_edited.set(true);
        set_enabled_chain_ids.update(|ids| {
            if ids.contains(&chain_id) {
                ids.retain(|id| id != &chain_id);
            } else {
                ids.push(chain_id);
            }
        });
    };

    // Role change handler
    let change_role = move |user_id: String, new_role: String| {
        let api = api.get_untracked();
        leptos::task::spawn_local(async move {
            let request = UpdateUserRoleRequest { role: new_role };
            match api.update_user_role(&user_id, &request).await {
                Ok(()) => {
                    set_user_status.set("Role updated".to_string());
                    load_users(user_offset.get_untracked());
                }
                Err(e) => set_user_status.set(format!("Error: {}", e)),
            }
        });
    };

    // Lock/unlock handler
    let toggle_lock = move |user_id: String, is_locked: bool| {
        let api = api.get_untracked();
        leptos::task::spawn_local(async move {
            let result = if is_locked {
                api.unlock_user(&user_id).await
            } else {
                api.lock_user(&user_id).await
            };
            match result {
                Ok(()) => {
                    load_users(user_offset.get_untracked());
                }
                Err(e) => set_user_status.set(format!("Error: {}", e)),
            }
        });
    };

    view! {
        <div class="settings-tab-admin">
            {move || {
                match safe_mode_banner(safe_mode.get(), safe_mode_check_failed.get()) {
                    SafeModeBanner::Active => view! {
                        <div class="alert alert-error">
                            <strong>"⚠ SAFE MODE: every plugin is disabled"</strong>
                            <p>
                                "The server booted with ETHPAY_DISABLE_PLUGINS set (or the "
                                "--disable-plugins flag). No plugin is running - including "
                                "billing, if it is installed as one. Plugins are not "
                                "uninstalled and their data is untouched: clear the flag and "
                                "restart to bring them back."
                            </p>
                        </div>
                    }.into_any(),
                    SafeModeBanner::CheckFailed => view! {
                        <div class="admin-warning">
                            <strong>"⚠ Could not confirm plugin status"</strong>
                            <p>
                                "The safe-mode check itself failed, so whether every plugin is "
                                "disabled for this boot is unknown - this is not the same as "
                                "confirming plugins are running normally."
                            </p>
                        </div>
                    }.into_any(),
                    SafeModeBanner::None => view! { <span></span> }.into_any(),
                }
            }}

            <div class="admin-warning">
                <IconShield />
                <div>
                    <strong>"Server Administration"</strong>
                    <p>"These settings affect the entire server. Changes may require a restart to take effect."</p>
                </div>
            </div>

            // User Management
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"User Management"</h3>
                    <span class="badge">{move || format!("{} users", user_total.get())}</span>
                </div>
                <div class="ps-card-body">
                    {move || {
                        let status = user_status.get();
                        if status.is_empty() {
                            view! { <span></span> }.into_any()
                        } else {
                            view! { <p class="form-help">{status}</p> }.into_any()
                        }
                    }}
                    <div class="admin-users-table">
                        <table class="data-table table">
                            <thead>
                                <tr>
                                    <th>"User"</th>
                                    <th>"Role"</th>
                                    <th>"Created"</th>
                                    <th>"Actions"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {move || users.get().into_iter().map(|user| {
                                    let user_id_role = user.id.clone();
                                    let user_id_lock = user.id.clone();
                                    let is_locked = user.locked_until.is_some();
                                    let display_name = user_display_name(&user);
                                    let current_role = user.role.clone();
                                    let created_date = user.created_at.format("%Y-%m-%d").to_string();
                                    view! {
                                        <tr class=if is_locked { "user-row locked" } else { "user-row" }>
                                            <td class="user-cell">
                                                <span class="user-name">{display_name}</span>
                                            </td>
                                            <td>
                                                <select
                                                    class="form-input form-input-sm"
                                                    on:change=move |ev| {
                                                        let new_role = event_target_value(&ev);
                                                        change_role(user_id_role.clone(), new_role);
                                                    }
                                                >
                                                    <option value="server_admin" selected=current_role == "server_admin">"Server Admin"</option>
                                                    <option value="user" selected=current_role == "user">"User"</option>
                                                </select>
                                            </td>
                                            <td class="date-cell">{created_date}</td>
                                            <td>
                                                <button
                                                    class=if is_locked { "ps-btn ps-btn-xs ps-btn-success" } else { "ps-btn ps-btn-xs ps-btn-warning" }
                                                    on:click=move |_| toggle_lock(user_id_lock.clone(), is_locked)
                                                >
                                                    {if is_locked { "Unlock" } else { "Lock" }}
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                    {move || {
                        let total = user_total.get();
                        (total > PAGE_SIZE).then(|| view! {
                            <Pagination
                                total=total
                                page_size=PAGE_SIZE
                                current_offset=user_offset.get()
                                on_page_change=move |new_offset| load_users(new_offset)
                                item_label="users"
                            />
                        })
                    }}
                </div>
            </div>

            // Operator store
            //
            // Its own card rather than a field among the payment defaults:
            // this is the only setting on the page that decides where money
            // is invoiced, and it is the only one that does not take effect
            // until a restart.
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Operator Store"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="form-group">
                        <label class="form-label">"Own store"</label>
                        <select
                            class="form-input"
                            prop:value=move || operator_store.get()
                            on:change=move |ev| set_operator_store.set(event_target_value(&ev))
                        >
                            <option value="">"None - this server issues no invoices to itself"</option>
                            <For
                                each=move || stores.get()
                                key=|store| store.id
                                let:store
                            >
                                <option value=store.id.to_string()>{store.name.clone()}</option>
                            </For>
                        </select>
                        <p class="form-help">
                            "The server's own invoices are issued on this store, and payments to \
                             it are what a plugin watching it is told about. It needs an enabled \
                             payment method whose wallet resolves, or it will be refused."
                        </p>
                        <Show when=move || !operator_store_active.get()>
                            <p class="form-help" style="color: var(--color-warning);">
                                "Pending restart - the server is not using this store yet."
                            </p>
                        </Show>
                    </div>
                </div>
            </div>

            // Payment Defaults
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Payment Defaults"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="settings-grid">
                        <div class="form-group">
                            <label class="form-label">"Required Confirmations"</label>
                            <input
                                type="number"
                                class="form-input"
                                min="1"
                                max="100"
                                prop:value=move || default_confirmations.get()
                                on:input=move |ev| set_default_confirmations.set(event_target_value(&ev))
                            />
                            <p class="form-help">"Block confirmations before payment is final"</p>
                        </div>

                        <div class="form-group">
                            <label class="form-label">"Invoice Expiry (minutes)"</label>
                            <input
                                type="number"
                                class="form-input"
                                min="5"
                                max="1440"
                                prop:value=move || invoice_expiry.get()
                                on:input=move |ev| set_invoice_expiry.set(event_target_value(&ev))
                            />
                            <p class="form-help">"Default expiration time for new invoices"</p>
                        </div>

                        <div class="form-group">
                            <label class="form-label">"Rate Limit (req/min)"</label>
                            <input
                                type="number"
                                class="form-input"
                                min="10"
                                max="1000"
                                prop:value=move || rate_limit.get()
                                on:input=move |ev| set_rate_limit.set(event_target_value(&ev))
                            />
                            <p class="form-help">"API rate limit per IP address"</p>
                        </div>
                    </div>
                </div>
            </div>

            // Enabled Networks
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Enabled Networks"</h3>
                </div>
                <div class="ps-card-body">
                    <p class="form-help" style="margin-bottom: 16px;">
                        "Networks available for payment processing. Disabled networks cannot be used by any store."
                    </p>
                    <div class="admin-networks-grid">
                        {all_networks.into_iter().map(|(chain_id, name)| {
                            let for_class = chain_id.clone();
                            let for_checked = chain_id.clone();
                            let for_toggle = chain_id.clone();
                            view! {
                                <div class=move || if enabled_chain_ids.get().contains(&for_class) { "admin-network-item enabled" } else { "admin-network-item" }>
                                    <div class="admin-network-info">
                                        <span class="admin-network-name">{name}</span>
                                        <span class="admin-network-chain-id">"Chain: "{chain_id.to_string()}</span>
                                    </div>
                                    <label class="toggle">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || enabled_chain_ids.get().contains(&for_checked)
                                            on:change=move |_| toggle_network(for_toggle.clone())
                                        />
                                        <span class="toggle-slider"></span>
                                    </label>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </div>
            </div>

            // Maintenance
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Maintenance"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="admin-actions">
                        <div class="admin-action">
                            <div class="admin-action-info">
                                <span class="admin-action-title">"Clear cache"</span>
                                <span class="admin-action-desc">"Clear all cached data including exchange rates"</span>
                            </div>
                            // None of the three maintenance actions call the server. Left
                            // enabled, an admin has no way to tell a no-op from a restart
                            // that happened.
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                disabled=true
                                title="Clear cache is not implemented yet"
                            >
                                "Clear cache"
                            </button>
                        </div>

                        <div class="admin-action">
                            <div class="admin-action-info">
                                <span class="admin-action-title">"Restart monitor"</span>
                                <span class="admin-action-desc">"Restart the EVM chain monitor service"</span>
                            </div>
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                disabled=true
                                title="Restarting the monitor is not implemented yet"
                            >
                                "Restart"
                            </button>
                        </div>

                        <div class="admin-action">
                            <div class="admin-action-info">
                                <span class="admin-action-title">"Export data"</span>
                                <span class="admin-action-desc">"Export all server data as JSON backup"</span>
                            </div>
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                disabled=true
                                title="Export is not implemented yet"
                            >
                                "Export"
                            </button>
                        </div>
                    </div>
                </div>
            </div>

            // Save + status
            <div class="form-actions">
                {move || {
                    let status = settings_status.get();
                    if status.is_empty() {
                        view! { <span></span> }.into_any()
                    } else {
                        view! { <span class="form-status">{status}</span> }.into_any()
                    }
                }}
                <button class="ps-btn ps-btn-primary" on:click=save_settings>"Save server settings"</button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(email: Option<&str>, wallet: Option<&str>, id: &str) -> AdminUserInfo {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "email": email,
            "primary_wallet_address": wallet,
            "role": "user",
            "created_at": "2026-01-01T00:00:00Z",
            "last_login_at": null,
            "locked_until": null,
        }))
        .unwrap()
    }

    fn page(total: i64) -> UserListResponse {
        UserListResponse {
            users: vec![],
            total,
            offset: 0,
            limit: PAGE_SIZE,
        }
    }

    fn block_on<T>(fut: impl std::future::Future<Output = T>) -> T {
        let mut fut = std::pin::pin!(fut);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("future was not immediately ready"),
        }
    }

    #[test]
    fn page_inside_the_result_set_is_fetched_once() {
        let mut calls = vec![];
        let (offset, _) = block_on(fetch_user_page(PAGE_SIZE, |o| {
            calls.push(o);
            std::future::ready(Ok(page(PAGE_SIZE * 3)))
        }))
        .unwrap();
        assert_eq!(offset, PAGE_SIZE);
        assert_eq!(calls, vec![PAGE_SIZE]);
    }

    #[test]
    fn page_past_the_end_refetches_the_last_page_and_reports_its_offset() {
        // Deleting the only user on the last page leaves the old offset empty.
        let mut calls = vec![];
        let (offset, _) = block_on(fetch_user_page(PAGE_SIZE * 2, |o| {
            calls.push(o);
            std::future::ready(Ok(page(PAGE_SIZE * 2)))
        }))
        .unwrap();
        assert_eq!(offset, PAGE_SIZE);
        assert_eq!(calls, vec![PAGE_SIZE * 2, PAGE_SIZE]);
    }

    #[test]
    fn failed_refetch_is_an_error_not_a_stale_offset() {
        let mut n = 0;
        let result = block_on(fetch_user_page(PAGE_SIZE * 2, |_| {
            n += 1;
            std::future::ready(if n == 1 {
                Ok(page(PAGE_SIZE * 2))
            } else {
                Err(ApiError::Network("down".into()))
            })
        }));
        assert!(result.is_err());
    }

    #[test]
    fn user_is_named_by_email_first() {
        let u = user(
            Some("a@b.test"),
            Some("0xabcdef0123456789"),
            "12345678-aaaa",
        );
        assert_eq!(user_display_name(&u), "a@b.test");
    }

    #[test]
    fn walletonly_user_is_named_by_shortened_address() {
        let u = user(None, Some("0xabcdef0123456789"), "12345678-aaaa");
        assert_eq!(user_display_name(&u), "0xabcd...6789");
    }

    #[test]
    fn user_with_neither_is_named_by_id_prefix() {
        let u = user(None, None, "12345678-aaaa");
        assert_eq!(user_display_name(&u), "12345678");
    }

    #[test]
    fn offset_inside_the_result_set_is_kept() {
        let total = PAGE_SIZE * 2 + 5;
        assert_eq!(clamp_user_offset(0, total), 0);
        assert_eq!(clamp_user_offset(PAGE_SIZE * 2, total), PAGE_SIZE * 2);
    }

    #[test]
    fn offset_past_the_end_falls_back_to_the_last_page() {
        // Exactly full pages: the last page starts one page before the end.
        assert_eq!(clamp_user_offset(PAGE_SIZE * 2, PAGE_SIZE * 2), PAGE_SIZE);
        // A partial last page starts where the final partial run begins.
        assert_eq!(
            clamp_user_offset(PAGE_SIZE * 5, PAGE_SIZE * 2 + 5),
            PAGE_SIZE * 2
        );
    }

    #[test]
    fn empty_result_set_stays_on_the_first_page() {
        assert_eq!(clamp_user_offset(0, 0), 0);
        assert_eq!(clamp_user_offset(PAGE_SIZE, 0), 0);
    }

    #[test]
    fn no_check_yet_shows_no_banner() {
        assert_eq!(safe_mode_banner(false, false), SafeModeBanner::None);
    }

    #[test]
    fn safe_mode_confirmed_shows_the_active_banner() {
        assert_eq!(safe_mode_banner(true, false), SafeModeBanner::Active);
    }

    #[test]
    fn a_failed_check_shows_the_check_failed_banner() {
        assert_eq!(safe_mode_banner(false, true), SafeModeBanner::CheckFailed);
    }

    #[test]
    fn a_confirmed_active_mode_outranks_a_later_failed_check() {
        // Once safe mode has been confirmed on, a later transient fetch
        // error must not read as "we no longer know" - it stays Active.
        assert_eq!(safe_mode_banner(true, true), SafeModeBanner::Active);
    }

    #[test]
    fn a_successful_check_reports_the_mode_and_clears_any_prior_failure() {
        let result = Ok(SafeModeStatus { safe_mode: true });
        assert_eq!(safe_mode_after_check(&result), (Some(true), false));

        let result = Ok(SafeModeStatus { safe_mode: false });
        assert_eq!(safe_mode_after_check(&result), (Some(false), false));
    }

    #[test]
    fn a_failed_check_reports_no_mode_and_flags_the_failure() {
        let result = Err(ApiError::Network("offline".to_string()));
        assert_eq!(safe_mode_after_check(&result), (None, true));
    }

    #[test]
    fn an_error_after_a_success_does_not_silently_read_as_plugins_fine() {
        // The signal-update contract: a failure never carries `Some(false)` -
        // that would be indistinguishable from a check that actually ran and
        // found plugins enabled. It always reports `None` and lets the
        // caller leave the last-known `safe_mode` value alone.
        let ok = safe_mode_after_check(&Ok(SafeModeStatus { safe_mode: false }));
        let err = safe_mode_after_check(&Err(ApiError::Unauthorized));
        assert_eq!(ok, (Some(false), false));
        assert_eq!(err, (None, true));
    }

    #[test]
    fn an_error_then_a_success_clears_the_stale_warning() {
        // This is the round-trip the sticky-banner bug lived in: an initial
        // failed check must not leave `safe_mode_check_failed` stuck true
        // forever once a later check succeeds.
        let (mode, check_failed) = safe_mode_after_check(&Err(ApiError::Network("x".into())));
        assert_eq!((mode, check_failed), (None, true));

        let (mode, check_failed) = safe_mode_after_check(&Ok(SafeModeStatus { safe_mode: false }));
        assert_eq!((mode, check_failed), (Some(false), false));
    }

    #[test]
    fn a_confirmed_signal_survives_a_later_failed_check() {
        // safe_mode_after_check proves the *tuple* it returns on a failure
        // carries no mode. This proves the `if let Some` guard that consumes
        // that tuple actually leaves a real signal alone: a confirmed `true`
        // must still read `true` after a subsequent transient error, on the
        // live signal the component renders from, not just on paper.
        let (safe_mode, set_safe_mode) = signal(false);
        let (check_failed, set_check_failed) = signal(false);

        apply_safe_mode_check(
            &Ok(SafeModeStatus { safe_mode: true }),
            set_safe_mode,
            set_check_failed,
        );
        assert!(safe_mode.get_untracked());
        assert!(!check_failed.get_untracked());

        apply_safe_mode_check(
            &Err(ApiError::Network("x".into())),
            set_safe_mode,
            set_check_failed,
        );
        assert!(
            safe_mode.get_untracked(),
            "a transient error must not clear a confirmed safe mode"
        );
        assert!(check_failed.get_untracked());
    }
}
