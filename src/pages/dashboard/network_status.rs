use crate::api::{ApiClient, ApiError, ChainHealthInfo};
use crate::util::chain_name;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::rc::Rc;

/// The connection state a monitor reports for one chain.
///
/// `/health/chains` sends this as a free-form string, and the failure case
/// carries its reason inline ("failed: no RPC endpoint configured"), so this
/// classifies rather than deserialises. Anything unrecognised is [`Self::Unknown`]
/// and shows as such: guessing "connected" for a string we cannot read is how
/// the panel would go green over a monitor that is not running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChainState {
    Connected,
    Connecting,
    Disconnected,
    Failed,
    Unknown,
}

impl ChainState {
    fn from_status(status: &str) -> Self {
        let status = status.trim();
        if status.eq_ignore_ascii_case("connected") {
            Self::Connected
        } else if status.eq_ignore_ascii_case("connecting") {
            Self::Connecting
        } else if status.eq_ignore_ascii_case("disconnected") {
            Self::Disconnected
        } else if status
            .as_bytes()
            // Byte slice, not `&status[..6]`: a status whose first six bytes
            // land mid-character would panic on a str slice and take the whole
            // dashboard down with it.
            .get(..6)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"failed"))
        {
            Self::Failed
        } else {
            Self::Unknown
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::Connecting => "Connecting",
            Self::Disconnected => "Disconnected",
            Self::Failed => "Failed",
            Self::Unknown => "Unknown",
        }
    }

    /// Dot class for this state.
    ///
    /// `is_healthy` is the monitor's own verdict and only matters while
    /// connected: a monitor can hold an RPC connection and still not be
    /// processing, which is precisely the state that once rendered as green.
    fn dot_class(self, is_healthy: bool) -> &'static str {
        match self {
            Self::Connected if is_healthy => "network-dot network-dot-online",
            Self::Connected | Self::Connecting => "network-dot network-dot-degraded",
            Self::Disconnected => "network-dot network-dot-offline",
            Self::Failed | Self::Unknown => "network-dot network-dot-error",
        }
    }
}

/// How far behind the chain head the monitor is, when both numbers are known.
fn monitor_lag(chain: &ChainHealthInfo) -> Option<u64> {
    let head = chain.current_block?;
    let processed = chain.last_processed_block?;
    Some(head.saturating_sub(processed))
}

/// Right-hand text for a chain row.
///
/// Replaces the invented "N conf" number: monitor lag is derived from two
/// values the monitor actually publishes, and reads as "we do not know" when
/// either is missing.
fn chain_detail(chain: &ChainHealthInfo, state: ChainState) -> String {
    if state != ChainState::Connected {
        return state.label().to_string();
    }
    match monitor_lag(chain) {
        None => "no block data".to_string(),
        Some(0) => "in sync".to_string(),
        Some(1) => "1 block behind".to_string(),
        Some(n) => format!("{n} blocks behind"),
    }
}

/// Display name for a chain row, preferring what the monitor reported.
fn chain_label(chain: &ChainHealthInfo) -> String {
    if chain.chain_name.trim().is_empty() {
        chain_name(&chain.chain_id).to_string()
    } else {
        chain.chain_name.clone()
    }
}

/// Network status panel — the chains the monitor is actually reporting on.
///
/// This was a hardcoded five-chain list that named chains which are not
/// enabled and showed them all connected with invented confirmation counts.
/// It would have stayed green through the outage where every monitor RPC
/// endpoint was empty. Nothing here has a default: no data means
/// the panel says so.
#[component]
pub(super) fn NetworkStatus() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    // The monitor republishes every 10s under a 60s TTL, so a panel rendered
    // once and never refreshed is a stale claim about live infrastructure.
    // `/health` is exempt from the IP rate limit tiers, so polling is safe.
    let tick = use_tick(CHAIN_HEALTH_POLL_MS);

    let health_resource = LocalResource::new(move || {
        let api = api.get();
        let _ = tick.get();
        async move { api.get_chains_health().await }
    });

    view! {
        <Suspense fallback=move || view! {
            <div class="activity-note">"Loading chain status…"</div>
        }>
            {move || health_resource.get().map(|result| match &*result {
                // 503 means the monitor has published no health at all. That is
                // "we cannot tell you", which is not the same as "all good" and
                // must not render as rows - a green panel over dead monitors
                // has shipped before.
                //
                // There is no 403 case: the endpoint answers everyone, and
                // simply says less to a non-admin (block heights, the watched
                // address count and the failure reason are withheld). Whether a
                // chain is up is exactly what a merchant needs.
                Err(ApiError::Http { status: 503, .. }) => view! {
                    <div class="activity-note activity-note-error">
                        "No chain health reported — the monitor has not published any."
                    </div>
                }.into_any(),
                Err(e) => view! {
                    <div class="activity-note activity-note-error">
                        {format!("Could not load chain status: {e}")}
                    </div>
                }.into_any(),
                Ok(response) if response.chains.is_empty() => view! {
                    <div class="activity-note activity-note-error">
                        "No chain health reported — the monitor has not published any."
                    </div>
                }.into_any(),
                Ok(response) => {
                    let stale = !response.data_fresh;
                    let chains = response.chains.clone();

                    view! {
                        {stale.then(|| view! {
                            <div class="activity-note activity-note-error">
                                "Health data is stale — the monitor has stopped reporting."
                            </div>
                        })}
                        <div class="network-list">
                            {chains.into_iter().map(|chain| {
                                let state = ChainState::from_status(&chain.status);
                                let dot_class = state.dot_class(chain.is_healthy);
                                let name = chain_label(&chain);
                                let detail = chain_detail(&chain, state);
                                // The raw status carries the failure reason;
                                // keep it reachable without widening the row.
                                let title = format!(
                                    "chain {} — {}",
                                    chain.chain_id,
                                    chain.status
                                );

                                view! {
                                    <div class="network-row" title=title>
                                        <div class="network-info">
                                            <span class=dot_class></span>
                                            <span class="network-name">{name}</span>
                                        </div>
                                        <span class="network-detail">{detail}</span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}

/// How often the network panel re-reads `/health/chains`, in milliseconds.
/// Well inside the 60s TTL on the monitor's Redis keys.
const CHAIN_HEALTH_POLL_MS: u32 = 20_000;

/// How often rendered relative timestamps are recomputed, in milliseconds.
pub(super) const RELATIVE_TIME_TICK_MS: u32 = 30_000;

/// A counter that increments every `interval_ms` for as long as the calling
/// component is alive.
///
/// The interval handle is dropped in `on_cleanup`: a timer that outlives its
/// owner and writes a disposed signal panics the whole app.
pub(super) fn use_tick(interval_ms: u32) -> ReadSignal<u32> {
    let (tick, set_tick) = signal(0u32);

    let handle: Rc<RefCell<Option<gloo_timers::callback::Interval>>> = Rc::new(RefCell::new(None));
    let handle_for_effect = handle.clone();
    Effect::new(move |_| {
        let interval = gloo_timers::callback::Interval::new(interval_ms, move || {
            set_tick.update(|n| *n = n.wrapping_add(1));
        });
        *handle_for_effect.borrow_mut() = Some(interval);
    });

    let handle_for_cleanup = SendWrapper::new(handle);
    on_cleanup(move || {
        handle_for_cleanup.borrow_mut().take();
    });

    tick
}

#[cfg(test)]
mod tests {
    use super::{ChainState, chain_detail, chain_label, monitor_lag};
    use crate::api::ChainHealthInfo;
    use types::ChainId;

    fn chain(status: &str, current: Option<u64>, processed: Option<u64>) -> ChainHealthInfo {
        ChainHealthInfo {
            chain_id: ChainId::evm(11_155_111),
            chain_name: "Sepolia".to_string(),
            status: status.to_string(),
            current_block: current,
            last_processed_block: processed,
            watched_addresses: None,
            is_healthy: true,
        }
    }

    #[test]
    fn classifies_the_statuses_the_server_sends() {
        assert_eq!(ChainState::from_status("connected"), ChainState::Connected);
        assert_eq!(
            ChainState::from_status("connecting"),
            ChainState::Connecting
        );
        assert_eq!(
            ChainState::from_status("disconnected"),
            ChainState::Disconnected
        );
    }

    #[test]
    fn a_failure_keeps_its_reason_and_still_classifies() {
        // The server formats this variant as "failed: {msg}", so an equality
        // check against "failed" would fall through to Unknown.
        assert_eq!(
            ChainState::from_status("failed: no RPC endpoint configured"),
            ChainState::Failed
        );
        assert_eq!(ChainState::from_status("failed"), ChainState::Failed);
    }

    #[test]
    fn an_unreadable_status_is_never_treated_as_connected() {
        assert_eq!(ChainState::from_status(""), ChainState::Unknown);
        assert_eq!(ChainState::from_status("weird"), ChainState::Unknown);
        assert_eq!(ChainState::from_status("fail"), ChainState::Unknown);
        // Not a status the server sends, but a str slice at byte 6 here would
        // panic rather than classify.
        assert_eq!(ChainState::from_status("ééééé"), ChainState::Unknown);
    }

    #[test]
    fn only_a_healthy_connection_gets_the_green_dot() {
        let online = "network-dot network-dot-online";
        assert_eq!(ChainState::Connected.dot_class(true), online);
        // Connected but the monitor itself says unhealthy.
        assert_ne!(ChainState::Connected.dot_class(false), online);
        assert_ne!(ChainState::Connecting.dot_class(true), online);
        assert_ne!(ChainState::Disconnected.dot_class(true), online);
        assert_ne!(ChainState::Failed.dot_class(true), online);
        assert_ne!(ChainState::Unknown.dot_class(true), online);
    }

    #[test]
    fn lag_is_the_gap_between_head_and_processed() {
        assert_eq!(
            monitor_lag(&chain("connected", Some(100), Some(97))),
            Some(3)
        );
        assert_eq!(
            monitor_lag(&chain("connected", Some(100), Some(100))),
            Some(0)
        );
        // A processed block ahead of the head is a race, not a negative lag.
        assert_eq!(
            monitor_lag(&chain("connected", Some(100), Some(101))),
            Some(0)
        );
    }

    #[test]
    fn missing_block_numbers_have_no_lag_rather_than_zero() {
        assert_eq!(monitor_lag(&chain("connected", None, Some(97))), None);
        assert_eq!(monitor_lag(&chain("connected", Some(100), None)), None);
        assert_eq!(
            chain_detail(&chain("connected", None, None), ChainState::Connected),
            "no block data"
        );
    }

    #[test]
    fn detail_reads_the_state_when_not_connected() {
        assert_eq!(
            chain_detail(
                &chain("failed: rpc timeout", Some(100), Some(97)),
                ChainState::Failed
            ),
            "Failed"
        );
        assert_eq!(
            chain_detail(&chain("connecting", None, None), ChainState::Connecting),
            "Connecting"
        );
    }

    #[test]
    fn detail_pluralises_blocks() {
        assert_eq!(
            chain_detail(
                &chain("connected", Some(100), Some(100)),
                ChainState::Connected
            ),
            "in sync"
        );
        assert_eq!(
            chain_detail(
                &chain("connected", Some(100), Some(99)),
                ChainState::Connected
            ),
            "1 block behind"
        );
        assert_eq!(
            chain_detail(
                &chain("connected", Some(100), Some(90)),
                ChainState::Connected
            ),
            "10 blocks behind"
        );
    }

    #[test]
    fn falls_back_to_the_chain_id_when_the_monitor_sends_no_name() {
        let mut c = chain("connected", Some(1), Some(1));
        assert_eq!(chain_label(&c), "Sepolia");
        c.chain_name = "  ".to_string();
        c.chain_id = ChainId::evm(1);
        assert_eq!(chain_label(&c), "Ethereum");
    }
}
