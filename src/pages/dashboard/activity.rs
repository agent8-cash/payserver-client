use super::network_status::{NetworkStatus, RELATIVE_TIME_TICK_MS, use_tick};
use crate::api::{ApiClient, ApiError};
use crate::app::{StoreContext, StoresStatus};
use crate::pages::payments::format::{
    column_scales, payment_status, payment_status_class, truncate_hash,
};
use crate::services::StatusUpdate;
use crate::util::{chain_name, relative_time_at};
use leptos::prelude::*;
use leptos_router::components::A;
use ui_kit::{trim_amount, units_to_decimal};

#[cfg(test)]
mod test_recent_payments {
    use super::*;
    use crate::api::Payment;
    use chrono::Utc;
    use types::ChainId;

    fn payment(amount: &str, decimals: u8, asset_symbol: &str) -> Payment {
        Payment {
            id: "pay-1".to_string(),
            store_id: Some("store-1".to_string()),
            store_name: Some("Test Store".to_string()),
            chain_id: ChainId::evm(1),
            invoice_id: "inv-1".to_string(),
            tx_hash: "0xabc".to_string(),
            amount: amount.to_string(),
            asset_symbol: asset_symbol.to_string(),
            token_address: None,
            block_number: None,
            from_address: None,
            detected_at: Utc::now(),
            confirmed_at: Some(Utc::now()),
            reorged: false,
            decimals,
        }
    }

    #[test]
    fn recent_payments_share_one_scale_per_asset() {
        // Two amounts of the same asset with different scales:
        // 0.000139292 (9 fractional) and 0.0000085129 (10 fractional)
        let rows = [
            payment("139292000000000", 18, "ETH"),
            payment("8512900000000", 18, "ETH"),
        ];

        let scales = column_scales(&rows);
        assert_eq!(scales["ETH"], 10);

        // Both should render at the widest scale
        let shown: Vec<_> = rows
            .iter()
            .map(|p| {
                let scale = scales.get(&p.asset_symbol).copied().unwrap_or(0);
                trim_amount(&units_to_decimal(&p.amount, p.decimals), scale)
            })
            .collect();

        assert_eq!(shown, ["0.0001392920", "0.0000085129"]);
    }

    #[test]
    fn recent_payments_keep_assets_independent() {
        // Two different assets should have independent scales
        let eth = payment("1000000000000000", 18, "ETH");
        let usdc = payment("1500000", 6, "USDC");

        let scales = column_scales(&[eth, usdc]);
        assert_eq!(scales["ETH"], 3);
        assert_eq!(scales["USDC"], 1);
    }

    #[test]
    fn recent_payments_keep_the_exact_amount_in_the_label() {
        let rows = [
            payment("139292000000000", 18, "ETH"),
            payment("8512900000000", 18, "ETH"),
        ];

        let scales = column_scales(&rows);

        for payment in rows {
            let scale = scales.get(&payment.asset_symbol).copied().unwrap_or(0);
            let exact = format!(
                "{} {}",
                trim_amount(&units_to_decimal(&payment.amount, payment.decimals), 0),
                payment.asset_symbol
            );
            let padded = format!(
                "{} {}",
                trim_amount(&units_to_decimal(&payment.amount, payment.decimals), scale),
                payment.asset_symbol
            );

            // Check that we have different representations
            if payment.amount == "8512900000000" {
                assert_eq!(exact, "0.0000085129 ETH");
                assert_eq!(padded, "0.0000085129 ETH");
            } else {
                assert_eq!(exact, "0.000139292 ETH");
                assert_eq!(padded, "0.0001392920 ETH");
            }

            // Exact should always be the minimal representation
            assert_eq!(
                trim_amount(&units_to_decimal(&payment.amount, payment.decimals), 0),
                exact.split_whitespace().next().unwrap()
            );
        }
    }

    #[test]
    fn recent_payments_no_subscripts_in_list() {
        let rows = [
            payment("139292000000000", 18, "ETH"),
            payment("8512900000000", 18, "ETH"),
        ];

        let scales = column_scales(&rows);

        for payment in rows {
            let scale = scales.get(&payment.asset_symbol).copied().unwrap_or(0);
            let padded = trim_amount(&units_to_decimal(&payment.amount, payment.decimals), scale);

            // Verify no subscript characters are present (U+2080-U+2089)
            for c in padded.chars() {
                assert!(!('\u{2080}'..='\u{2089}').contains(&c));
            }
        }
    }
}

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
                    // Compute scales once for the entire list, so all amounts
                    // of the same asset display at the same width.
                    let scales = column_scales(&rows);

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
                                let scale = scales.get(&asset_symbol).copied().unwrap_or(0);
                                let exact = format!("{} {asset_symbol}", trim_amount(&amount_decimal, 0));
                                let padded = format!("{} {asset_symbol}", trim_amount(&amount_decimal, scale));

                                view! {
                                    <A href=href attr:class="payment-row">
                                        <div class="payment-info">
                                            <span class="payment-tx">{tx}</span>
                                            <span class="payment-time">{when}</span>
                                        </div>
                                        <div class="payment-amount">
                                            <span class="payment-crypto" title=exact.clone() aria-label=exact>
                                                {padded}
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
