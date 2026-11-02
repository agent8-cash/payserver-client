//! Dashboard page: an overview of EVM payment activity.

mod activity;
mod charts;
mod header;
mod icons;
mod metrics;
mod network_status;
mod onboarding;

use crate::api::ApiClient;
use crate::services::StatusUpdate;
use activity::DashboardActivity;
use charts::DashboardCharts;
use header::DashboardHeader;
use leptos::prelude::*;
use metrics::DashboardMetrics;
use onboarding::OnboardingChecklist;

/// Dashboard page component.
#[component]
pub fn DashboardPage() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let ws_update = use_context::<ReadSignal<Option<StatusUpdate>>>();

    let (ws_version, set_ws_version) = signal(0u32);
    if let Some(ws_update) = ws_update {
        Effect::new(move || {
            if let Some(StatusUpdate::InvoiceStatus { .. } | StatusUpdate::PaymentUpdate { .. }) =
                ws_update.get()
            {
                set_ws_version.update(|n| *n = n.wrapping_add(1));
            }
        });
    }

    // The 7D/30D/90D buttons used to be decorative. They now drive the window,
    // and the server rejects anything outside 1..=90.
    let (days, set_days) = signal(30u32);

    // Owned here rather than inside `DashboardCharts`, so the header's Export
    // button can read the same fetch the chart is rendering rather than
    // issuing its own and risking a window mismatch between the two.
    let analytics = LocalResource::new(move || {
        let client = api.get();
        let days = days.get();
        let _ = ws_version.get();
        async move { client.get_dashboard_analytics(days).await.ok() }
    });

    view! {
        <div class="dashboard">
            <DashboardHeader analytics=analytics />
            <OnboardingChecklist />
            <DashboardMetrics />
            <DashboardCharts analytics=analytics days=days set_days=set_days />
            <DashboardActivity />
        </div>
    }
}
