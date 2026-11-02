use super::icons::{IconDownload, IconPlus};
use crate::api::DashboardAnalytics;
use leptos::prelude::*;
use leptos_router::components::A;

/// Dashboard header with title and actions.
///
/// The Export button downloads the payment-volume analytics behind the two
/// chart panels below - the same fetch, read a third way, rather than a new
/// endpoint. It stays disabled until that fetch has actually landed, same as
/// every other not-yet-available control on this page.
#[component]
pub(super) fn DashboardHeader(
    analytics: LocalResource<Option<DashboardAnalytics>>,
) -> impl IntoView {
    let export_available = move || matches!(analytics.get().as_deref(), Some(Some(_)));

    view! {
        <div class="dashboard-header">
            <div>
                <h1 class="dashboard-title">"Dashboard"</h1>
                <p class="dashboard-subtitle">"Overview of your payment activity"</p>
            </div>
            <div class="dashboard-actions">
                <button
                    class="ps-btn ps-btn-secondary ps-btn-sm"
                    disabled=move || !export_available()
                    title="Export the payment volume behind the charts below as CSV"
                    on:click=move |_| {
                        wasm_bindgen_futures::spawn_local(async move {
                            match analytics.await {
                                Some(data) => {
                                    let csv = dashboard_volume_csv(&data);
                                    let filename = format!(
                                        "dashboard-volume_{}_{}.csv",
                                        data.start_date, data.end_date,
                                    );
                                    crate::pages::trigger_csv_download(&csv, &filename);
                                }
                                // The button is only enabled once this same
                                // resource has already resolved to `Some`, so
                                // getting `None` here means a re-fetch (e.g. a
                                // WS-triggered refresh) landed between render
                                // and click and failed. Same as the payments
                                // and invoices export buttons: log it rather
                                // than fail silently, without inventing new
                                // toast plumbing for an edge this narrow.
                                None => {
                                    web_sys::console::error_1(
                                        &"dashboard export: analytics unavailable".into(),
                                    );
                                }
                            }
                        });
                    }
                >
                    <IconDownload />
                    "Export"
                </button>
                <A href="/evm/invoices" attr:class="ps-btn ps-btn-primary ps-btn-sm">
                    <IconPlus />
                    "Create Invoice"
                </A>
            </div>
        </div>
    }
}

/// Escape a field for CSV output: quote it if it holds a comma, quote or
/// newline, doubling any quotes inside.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Build one CRLF-terminated CSV row.
fn csv_row(fields: &[&str]) -> String {
    let mut row = fields
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",");
    row.push_str("\r\n");
    row
}

/// Render the dashboard's payment-volume analytics as CSV: one summary row
/// per asset, then the daily series each chart bar is drawn from.
///
/// Built client-side from data the page already fetched for the charts -
/// `/dashboard/analytics` has no CSV form of its own, and this needs no new
/// endpoint to be a useful export of what the merchant is already looking at.
fn dashboard_volume_csv(data: &DashboardAnalytics) -> String {
    // `share_percent` is the asset's share of the window's payment *count*,
    // not of `total_amount` - the two can disagree (a high-value, low-frequency
    // asset vs. a low-value, high-frequency one). Naming the column for what it
    // actually measures avoids a merchant reading it next to `total_amount` as
    // a volume split.
    let mut content = csv_row(&[
        "asset",
        "total_amount",
        "payment_count",
        "payment_count_share_percent",
    ]);
    for asset in &data.assets {
        content.push_str(&csv_row(&[
            &asset.asset_symbol,
            &asset.total_amount,
            &asset.payment_count.to_string(),
            &format!("{:.1}", asset.share_percent),
        ]));
    }

    content.push_str("\r\n");
    content.push_str(&csv_row(&["date", "asset", "amount", "payment_count"]));
    for asset in &data.assets {
        for day in &asset.daily {
            content.push_str(&csv_row(&[
                &day.date.to_string(),
                &asset.asset_symbol,
                &day.amount,
                &day.payment_count.to_string(),
            ]));
        }
    }

    content
}

#[cfg(test)]
mod tests {
    use super::{csv_escape, dashboard_volume_csv};
    use crate::api::{AssetVolume, DailyVolume, DashboardAnalytics};
    use chrono::NaiveDate;

    fn sample_analytics() -> DashboardAnalytics {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        DashboardAnalytics {
            days: 2,
            start_date: start,
            end_date: end,
            total_payments: 3,
            assets: vec![AssetVolume {
                asset_symbol: "ETH".to_string(),
                total_amount: "1.5".to_string(),
                payment_count: 3,
                share_percent: 100.0,
                daily: vec![
                    DailyVolume {
                        date: start,
                        amount: "1".to_string(),
                        payment_count: 2,
                    },
                    DailyVolume {
                        date: end,
                        amount: "0.5".to_string(),
                        payment_count: 1,
                    },
                ],
            }],
        }
    }

    #[test]
    fn volume_csv_has_a_summary_section_and_a_daily_section() {
        let csv = dashboard_volume_csv(&sample_analytics());
        let mut lines = csv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "asset,total_amount,payment_count,payment_count_share_percent"
        );
        assert_eq!(lines.next().unwrap(), "ETH,1.5,3,100.0");
        assert_eq!(lines.next().unwrap(), "");
        assert_eq!(lines.next().unwrap(), "date,asset,amount,payment_count");
        assert_eq!(lines.next().unwrap(), "2026-01-01,ETH,1,2");
        assert_eq!(lines.next().unwrap(), "2026-01-02,ETH,0.5,1");
        assert!(lines.next().is_none());
    }

    #[test]
    fn csv_escape_quotes_only_a_field_that_needs_it() {
        assert_eq!(csv_escape("ETH"), "ETH");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
        assert_eq!(csv_escape("a\rb"), "\"a\rb\"");
    }

    #[test]
    fn volume_csv_escapes_an_asset_symbol_that_needs_it() {
        let mut data = sample_analytics();
        data.assets[0].asset_symbol = "WEIRD,\"TOKEN\"".to_string();
        let csv = dashboard_volume_csv(&data);
        let mut lines = csv.lines();
        lines.next(); // header
        assert_eq!(lines.next().unwrap(), "\"WEIRD,\"\"TOKEN\"\"\",1.5,3,100.0");
    }

    #[test]
    fn volume_csv_on_an_empty_account_is_headers_only() {
        let mut data = sample_analytics();
        data.assets.clear();
        let csv = dashboard_volume_csv(&data);
        assert_eq!(
            csv,
            "asset,total_amount,payment_count,payment_count_share_percent\r\n\r\ndate,asset,amount,payment_count\r\n"
        );
    }
}
