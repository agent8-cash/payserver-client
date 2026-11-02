use super::network_status::{NetworkStatus, RELATIVE_TIME_TICK_MS, use_tick};
use crate::api::{ApiClient, ApiError};
use crate::app::{StoreContext, StoresStatus};
use crate::pages::payments::format::{payment_status, payment_status_class, truncate_hash};
use crate::services::StatusUpdate;
use crate::util::{chain_name, relative_time_at};
use leptos::prelude::*;
use leptos_router::components::A;
use ui_kit::{CompactAmount, units_to_decimal};

/// Recent activity section.
#[component]
pub(super) fn DashboardActivity() -> impl IntoView {
    view! {
        <div class="activity-section">
            <div class="activity-card">
                <div class="activity-header">
                    <h3 class="activity-title">"Recent payments"</h3>
                    <A href="/evm/payments" attr:class="activity-link">"View all"</A>
                </div>
                <RecentPayments />
            </div>

            <div class="activity-card">
                <div class="activity-header">
                    <h3 class="activity-title">"Network status"</h3>
                </div>
                <NetworkStatus />
            </div>
        </div>
    }
}

/// How many payments the panel shows. The "View all" link covers the rest.
const RECENT_PAYMENTS_LIMIT: i64 = 5;

/// Recent payments list — the newest rows for the store the dashboard is
/// scoped to.
///
/// This panel used to be a literal list of invented tx hashes, amounts, dollar
/// values and timestamps, shown identically to every account including ones
/// with no payments at all. Everything here now comes off the row.
///
/// There is deliberately no fiat column. Rendering one needs a rate for the
/// asset *at the time the payment landed*; `/rates` only serves the current
/// rate, and today's price against a month-old payment is another invented
/// number next to a real one. The crypto amount and its chain are both true,
/// so that is what the row shows.
#[component]
fn RecentPayments() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let store_ctx = use_context::<StoreContext>().expect("StoreContext must be provided");
    // Signals off StoreContext are Copy; take them once so the resource closure
    // does not need to own the (non-Copy) context.
    let selected_store_id = store_ctx.selected_store_id;
    let store_status = store_ctx.stores_status;

    // Same WebSocket trigger DashboardMetrics uses, so the list does not go
    // stale next to counters that just moved.
    let (ws_version, set_ws_version) = signal(0u32);
    if let Some(ws_update) = use_context::<ReadSignal<Option<StatusUpdate>>>() {
        Effect::new(move || {
            if let Some(StatusUpdate::PaymentUpdate { .. }) = ws_update.get() {
                set_ws_version.update(|n| *n = n.wrapping_add(1));
            }
        });
    }

    // Relative timestamps are computed at render, so a dashboard left open
    // would otherwise say "Just now" indefinitely.
    let tick = use_tick(RELATIVE_TIME_TICK_MS);

    let payments_resource = LocalResource::new(move || {
        let api = api.get();
        let store_id = selected_store_id.get();
        let stores_loaded = matches!(store_status.get(), StoresStatus::Loaded);
        let _ = ws_version.get();

        async move {
            // Mirrors `pages/payments/list.rs`: "All Stores" is a
            // real query, but only once the store list has landed, and a
            // non-admin's 400 is a "pick a store" state rather than an error.
            // Widening the server side of that will make this branch stop
            // being reached.
            if store_id.is_none() && !stores_loaded {
                return Ok(None);
            }
            match api
                .list_payments(
                    store_id.as_deref(),
                    None,
                    None,
                    Some(RECENT_PAYMENTS_LIMIT),
                    Some(0),
                )
                .await
            {
                Ok(response) => Ok(Some(response)),
                Err(ApiError::Http { status: 400, .. }) if store_id.is_none() => Ok(None),
                Err(e) => Err(e),
            }
        }
    });

    view! {
        <Suspense fallback=move || view! {
            <div class="activity-note">"Loading payments…"</div>
        }>
            {move || payments_resource.get().map(|result| match &*result {
                Err(e) => view! {
                    <div class="activity-note activity-note-error">
                        {format!("Could not load payments: {e}")}
                    </div>
                }.into_any(),
                // Store list still settling, or an "All Stores" read this
                // account is not allowed to make. Either way we have no rows
                // for a specific store, and inventing some is the bug.
                Ok(None) => view! {
                    <div class="activity-note">
                        "Select a store in the sidebar to see its payments."
                    </div>
                }.into_any(),
                Ok(Some(response)) if response.payments.is_empty() => view! {
                    <div class="activity-note">
                        "No payments yet. They appear here once an invoice receives a transaction."
                    </div>
                }.into_any(),
                Ok(Some(response)) => {
                    // Server orders by detected_at DESC, so these are already
                    // newest first.
                    let rows = response.payments.clone();
                    // One clock read for the whole list, re-read on each tick.
                    let _ = tick.get();
                    let now_ms = js_sys::Date::now();

                    view! {
                        <div class="payments-list">
                            {rows.into_iter().map(|payment| {
                                let tx = truncate_hash(&payment.tx_hash, 8, 6);
                                let when = relative_time_at(payment.detected_at, now_ms)
                                    .unwrap_or_else(|| payment.detected_at.to_rfc3339());
                                let amount_decimal = units_to_decimal(&payment.amount, payment.decimals);
                                let asset_symbol = payment.asset_symbol.clone();
                                let network = chain_name(&payment.chain_id).to_string();
                                let status = payment_status(&payment);
                                let status_class = payment_status_class(&payment);
                                let href = format!("/evm/payments/{}", payment.id);

                                view! {
                                    <A href=href attr:class="payment-row">
                                        <div class="payment-info">
                                            <span class="payment-tx">{tx}</span>
                                            <span class="payment-time">{when}</span>
                                        </div>
                                        <div class="payment-amount">
                                            <span class="payment-crypto">
                                                <CompactAmount value=amount_decimal symbol=asset_symbol />
                                            </span>
                                            <span class="payment-chain">{network}</span>
                                        </div>
                                        <span class=status_class>{status}</span>
                                    </A>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}
