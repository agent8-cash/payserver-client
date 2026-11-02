use crate::api::DashboardAnalytics;
use crate::components::EmptyState;
use leptos::prelude::*;

/// Charts section — one `/dashboard/analytics` fetch feeding both panels.
///
/// The volume chart and the methods breakdown are the same aggregation read
/// two ways, so they share a resource rather than each hitting the API. That
/// resource is owned by `DashboardPage` and passed in, because the header's
/// Export button reads it too.
#[component]
pub(super) fn DashboardCharts(
    analytics: LocalResource<Option<DashboardAnalytics>>,
    days: ReadSignal<u32>,
    set_days: WriteSignal<u32>,
) -> impl IntoView {
    // None = follow the busiest asset the account actually uses; a click pins
    // one. Cleared whenever the window changes, since the busiest asset in a
    // different window may not be the pinned one.
    let (pinned_asset, set_pinned_asset) = signal(None::<String>);

    view! {
        <div class="charts-section">
            <div class="chart-card chart-card-main">
                <div class="chart-header">
                    <div>
                        <h3 class="chart-title">"Payment volume"</h3>
                        <p class="chart-subtitle">
                            {move || format!("Daily volume over the last {} days", days.get())}
                        </p>
                    </div>
                    <div class="chart-controls">
                        {[7_u32, 30, 90].into_iter().map(|window| {
                            view! {
                                <button
                                    class=move || if days.get() == window {
                                        "ps-btn ps-btn-ghost ps-btn-xs active"
                                    } else {
                                        "ps-btn ps-btn-ghost ps-btn-xs"
                                    }
                                    on:click=move |_| {
                                        set_days.set(window);
                                        set_pinned_asset.set(None);
                                    }
                                >
                                    {format!("{window}D")}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </div>
                <div class="chart-body">
                    <Suspense fallback=move || view! {
                        <p class="chart-subtitle">"Loading volume…"</p>
                    }>
                        {move || Suspend::new(async move {
                            match analytics.await {
                                Some(data) => view! {
                                    <VolumeChart
                                        data=data
                                        pinned_asset=pinned_asset
                                        set_pinned_asset=set_pinned_asset
                                    />
                                }.into_any(),
                                None => view! {
                                    <p class="chart-subtitle">
                                        "Volume is unavailable right now."
                                    </p>
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>
            </div>

            <div class="chart-card">
                <div class="chart-header">
                    <div>
                        <h3 class="chart-title">"Payment methods"</h3>
                        <p class="chart-subtitle">"Share of payments received"</p>
                    </div>
                </div>
                <div class="chart-body">
                    <Suspense fallback=move || view! {
                        <p class="chart-subtitle">"Loading breakdown…"</p>
                    }>
                        {move || Suspend::new(async move {
                            match analytics.await {
                                Some(data) => view! {
                                    <PaymentMethodsBreakdown data=data />
                                }.into_any(),
                                None => view! {
                                    <p class="chart-subtitle">
                                        "Breakdown is unavailable right now."
                                    </p>
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>
            </div>
        </div>
    }
}

/// Brand colour for the assets we ship payment methods for.
///
/// Anything else gets a neutral accent rather than a colour invented for it.
fn asset_color(symbol: &str) -> &'static str {
    match symbol {
        "ETH" | "WETH" => "#627eea",
        "USDC" => "#2775ca",
        "USDT" => "#26a17b",
        "DAI" | "xDAI" => "#f5ac37",
        "POL" | "MATIC" => "#8247e5",
        "WBTC" => "#f7931a",
        _ => "#6b7280",
    }
}

/// Parse a decimal amount string for bar scaling only.
///
/// The displayed figures always come from the server's exact strings; this is
/// used solely to work out how tall a bar is, where f64 is plenty.
fn amount_for_scale(amount: &str) -> f64 {
    amount.parse::<f64>().unwrap_or(0.0)
}

/// Daily volume for one asset.
///
/// Bars are per asset because volume is not summable across assets — 1 ETH
/// plus 1 USDC is not 2 of anything. The selected asset's symbol is
/// on the axis label so the numbers mean something.
#[component]
fn VolumeChart(
    data: DashboardAnalytics,
    pinned_asset: ReadSignal<Option<String>>,
    set_pinned_asset: WriteSignal<Option<String>>,
) -> impl IntoView {
    if data.assets.is_empty() {
        // An account with no payments gets an honest blank, not a flat line
        // and not the invented upward trend this panel used to draw.
        return view! {
            <EmptyState
                title="No payments yet"
                description="Volume will appear here once your first payment is received."
            />
        }
        .into_any();
    }

    let assets = StoredValue::new(data.assets);
    let selector = (assets.with_value(Vec::len) > 1).then(|| {
        let symbols: Vec<String> = assets.with_value(|a| {
            a.iter().map(|asset| asset.asset_symbol.clone()).collect()
        });
        view! {
            <div class="chart-controls">
                {symbols.into_iter().map(|symbol| {
                    let selected = symbol.clone();
                    let label = symbol.clone();
                    view! {
                        <button
                            class=move || {
                                let active = pinned_asset.get().as_deref() == Some(symbol.as_str());
                                if active { "ps-btn ps-btn-ghost ps-btn-xs active" } else { "ps-btn ps-btn-ghost ps-btn-xs" }
                            }
                            on:click=move |_| set_pinned_asset.set(Some(selected.clone()))
                        >
                            {label}
                        </button>
                    }
                }).collect_view()}
            </div>
        }
    });

    view! {
        <div class="volume-chart">
            {selector}
            {move || {
                let pinned = pinned_asset.get();
                let asset = assets.with_value(|list| {
                    pinned
                        .as_deref()
                        .and_then(|want| list.iter().find(|a| a.asset_symbol == want))
                        .or_else(|| list.first())
                        .cloned()
                });
                asset.map(|asset| {
                    // Scale to the busiest day so a quiet window still reads;
                    // a zero max would divide by zero, so it floors at 1.
                    let max = asset
                        .daily
                        .iter()
                        .map(|d| amount_for_scale(&d.amount))
                        .fold(0.0_f64, f64::max)
                        .max(f64::MIN_POSITIVE);
                    let color = asset_color(&asset.asset_symbol);
                    let last = asset.daily.len().saturating_sub(1);
                    let first_label = asset.daily.first().map(|d| d.date.to_string()).unwrap_or_default();
                    let last_label = asset.daily.last().map(|d| d.date.to_string()).unwrap_or_default();
                    let footer = format!(
                        "{} {} from {} payments",
                        asset.total_amount, asset.asset_symbol, asset.payment_count,
                    );
                    view! {
                        <div class="volume-chart-bars">
                            {asset.daily.iter().enumerate().map(|(i, point)| {
                                let height = (amount_for_scale(&point.amount) / max * 100.0)
                                    .clamp(0.0, 100.0);
                                let title = format!(
                                    "{}: {} {} ({} payments)",
                                    point.date, point.amount, asset.asset_symbol,
                                    point.payment_count,
                                );
                                let style = if i == last {
                                    format!("height: {height}%; background: {color}")
                                } else {
                                    format!("height: {height}%")
                                };
                                view! {
                                    <div
                                        class=if i == last { "volume-bar volume-bar-today" } else { "volume-bar" }
                                        style=style
                                        title=title
                                    ></div>
                                }
                            }).collect_view()}
                        </div>
                        <div class="volume-chart-labels">
                            <span>{first_label}</span>
                            <span>{footer}</span>
                            <span>{last_label}</span>
                        </div>
                    }
                })
            }}
        </div>
    }
    .into_any()
}

/// Payment methods breakdown.
///
/// Percentages are each asset's share of the *payment count* over the window.
/// Sharing by value would mean adding ETH to USDC, which is not a number.
#[component]
fn PaymentMethodsBreakdown(data: DashboardAnalytics) -> impl IntoView {
    if data.assets.is_empty() {
        return view! {
            <EmptyState
                title="No payments yet"
                description="The assets your customers pay with will appear here."
            />
        }
        .into_any();
    }

    view! {
        <div class="payment-methods">
            {data.assets.into_iter().map(|asset| {
                let color = asset_color(&asset.asset_symbol);
                let width = asset.share_percent.clamp(0.0, 100.0);
                let title = format!(
                    "{} payments totalling {} {}",
                    asset.payment_count, asset.total_amount, asset.asset_symbol,
                );
                view! {
                    <div class="payment-method-row" title=title>
                        <div class="payment-method-info">
                            <span class="payment-method-dot" style=format!("background: {color}")></span>
                            <span class="payment-method-name">{asset.asset_symbol}</span>
                        </div>
                        <div class="payment-method-bar-container">
                            <div
                                class="payment-method-bar"
                                style=format!("width: {width}%; background: {color}")
                            ></div>
                        </div>
                        <span class="payment-method-pct">{format!("{:.0}", asset.share_percent)}"%"</span>
                    </div>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}
