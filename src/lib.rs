//! PayServer Web Client
//!
//! Leptos-based frontend for the random.cash payservers, providing:
//! - Invoice management
//! - Payment monitoring
//! - Store configuration
//! - Wallet management
//!
//! Can run standalone or be loaded as a module in the dashboard aggregator.

#![allow(clippy::items_after_test_module)]
// The `plugin_page` unit tests pull in leptos's `ssr` feature (dev-dependency
// only, see `Cargo.toml`) to render real markup instead of asserting on
// `view!` string literals by eye. `ssr`'s HTML-string codegen path is far
// more type-heavy per nesting level than the DOM-mutation path `csr` uses,
// and this crate's deeply nested `app`/`layout` views blow the default limit
// under it during test builds. Release/wasm builds never see this: `ssr` is
// a dev-dependency, so `trunk build` never activates it.
// Measured: 128 (the default) overflows, 192 compiles. Set to 256 (rustc's
// own suggested bump from the overflow message) for headroom over that floor
// without inheriting a much larger number than the crate needs.
#![recursion_limit = "256"]

pub mod api;
pub mod app;
pub mod components;
pub mod pages;
pub mod services;
pub mod telemetry;
pub mod util;

pub use app::App;

use types::ChainId;
use ui_kit::module::{CheckoutPluginConfig, ModuleInfo};
use ui_kit::types::RouteInfo;
use wasm_bindgen::prelude::*;

/// EVM module identifier.
pub const MODULE_ID: &str = "evm";

/// EVM module display name.
pub const MODULE_NAME: &str = "Ethereum";

/// Get module info for the dashboard aggregator.
pub fn module_info(api_url: String) -> ModuleInfo {
    ModuleInfo {
        id: MODULE_ID,
        name: MODULE_NAME,
        icon: "ethereum",
        api_url,
        routes: vec![
            RouteInfo {
                path: "/evm".to_string(),
                label: "Dashboard".to_string(),
                icon: Some("dashboard".to_string()),
            },
            RouteInfo {
                path: "/evm/invoices".to_string(),
                label: "Invoices".to_string(),
                icon: Some("receipt".to_string()),
            },
            RouteInfo {
                path: "/evm/payments".to_string(),
                label: "Payments".to_string(),
                icon: Some("payments".to_string()),
            },
            RouteInfo {
                path: "/evm/stores".to_string(),
                label: "Stores".to_string(),
                icon: Some("store".to_string()),
            },
            RouteInfo {
                path: "/evm/wallets".to_string(),
                label: "Wallets".to_string(),
                icon: Some("wallet".to_string()),
            },
            RouteInfo {
                path: "/evm/settings".to_string(),
                label: "Settings".to_string(),
                icon: Some("settings".to_string()),
            },
        ],
    }
}

/// Get checkout plugin configuration for the checkout page.
pub fn checkout_plugin() -> CheckoutPluginConfig {
    CheckoutPluginConfig {
        module_id: MODULE_ID,
        network_badge: render_network_badge,
        amount_details: Some(render_amount_details),
        qr_code: render_qr_code,
        wallet_actions: Some(render_wallet_actions),
    }
}

/// The invoice details `wallet_actions` needs but cannot receive as an
/// argument.
///
/// `ui_kit::module::checkout_slots::WalletActionsFn` is `fn(payment_address,
/// chain_id) -> Option<AnyView>` — frozen in `payserver-commons`, pinned by
/// `rev`, so widening it for one caller means the three-step cross-repo dance
/// for a single-ticket change. The amount and token travel through context
/// instead, provided by `checkout.rs` right before it calls the slot.
#[derive(Clone)]
pub(crate) struct WalletActionsContext {
    pub amount: String,
    pub token_address: Option<String>,
    /// Set once a send succeeds, so a later render of this slot starts up
    /// already locked instead of re-enabling the wallet buttons.
    ///
    /// `checkout.rs` rebuilds the whole payment-details subtree — including
    /// a fresh `Idle` status signal inside `render_wallet_actions` — on
    /// every resource refetch (the ten-second poll, and every websocket
    /// update, which fires exactly when a just-broadcast payment is
    /// detected). A status signal local to that subtree cannot survive
    /// that rebuild, so this one is owned by `CheckoutPage`, above the
    /// `Suspense` boundary, and threaded through context like the rest of
    /// this struct.
    pub sent_lock: leptos::prelude::RwSignal<bool>,
}

/// Outcome of a connect-and-send attempt, shown under the wallet list.
#[derive(Clone)]
enum WalletActionStatus {
    Idle,
    Connecting,
    WrongNetwork(String),
    Sent,
    Error(String),
}

// Checkout slot implementations
fn render_network_badge(chain_id: &ChainId, network_name: &str) -> leptos::prelude::AnyView {
    use leptos::prelude::*;
    use ui_kit::components::crypto::NetworkBadge;

    // The name is passed through rather than derived: a CAIP-2 reference is
    // mostly an opaque genesis hash, so nothing can turn one into a name.
    view! {
        <NetworkBadge chain_id=chain_id.clone() name=network_name.to_string() />
    }
    .into_any()
}

fn render_amount_details(chain_id: &ChainId, _amount: &str) -> Option<leptos::prelude::AnyView> {
    use leptos::prelude::*;

    // Show gas info for EVM chains
    Some(
        view! {
            <div class="evm-amount-details">
                <span class="evm-chain-info">"Chain: " {chain_id.to_string()}</span>
            </div>
        }
        .into_any(),
    )
}

fn render_qr_code(payment_request: &str) -> leptos::prelude::AnyView {
    use leptos::prelude::*;
    use ui_kit::components::crypto::QrCodeCard;

    let data = payment_request.to_string();

    view! {
        <QrCodeCard data=data label="Scan to pay" size=250 />
    }
    .into_any()
}

/// EIP-6963 wallet connect. Renders nothing until a wallet actually answers
/// discovery — an empty list is the normal "no wallet installed" case, not an
/// error, and the copy-address/QR path above this is the primary route
/// regardless of what happens here. This slot only reads `payment_address`
/// and the invoice amount/token from [`WalletActionsContext`]; it never
/// touches the signals that drive the displayed address or amount.
fn render_wallet_actions(
    payment_address: &str,
    chain_id: &ChainId,
) -> Option<leptos::prelude::AnyView> {
    use leptos::prelude::*;

    use crate::services::eip6963::discover_wallets;

    // Not provided means whoever called this slot isn't checkout's payment
    // flow (or forgot to set it up) — degrade to nothing rather than a
    // wallet-connect UI with no idea what to transfer.
    let transfer = use_context::<WalletActionsContext>();

    // `evm_chain_id()` is `None` both for non-EVM chains and for a malformed
    // `eip155:` reference that isn't a number — either way there's no id to
    // check the wallet against, so degrade to no button rather than reach
    // `connect_and_send` with nothing to compare and skip the network guard
    // it exists to enforce.
    let (expected_chain_hex, transfer) = wallet_actions_gate(chain_id, transfer)?;

    let payment_address = payment_address.to_string();
    let chain_id = chain_id.clone();
    let chain_label = crate::util::chain_name(&chain_id).to_string();

    let wallets = discover_wallets();
    let sent_lock = transfer.sent_lock;
    let (status, set_status) = signal(initial_wallet_status(sent_lock.get()));

    Some(
        view! {
            <Show when=move || should_show_wallet_list(&wallets.get())>
                <div class="checkout-wallet-actions">
                    <span class="checkout-wallet-label">"Or pay with a connected wallet"</span>
                    <div class="checkout-wallet-list">
                        {
                            // Cloned here, outside the reactive closure: `Show`
                            // and this dynamic child both need `Fn`, callable
                            // more than once, and a closure that *moves* its
                            // own captured field into a nested closure can
                            // only be `FnOnce`. Cloning first means each call
                            // borrows the captured original and moves a fresh
                            // copy onward instead.
                            let payment_address = payment_address.clone();
                            let expected_chain_hex = expected_chain_hex.clone();
                            let chain_label = chain_label.clone();
                            let transfer = transfer.clone();
                            move || {
                                let payment_address = payment_address.clone();
                                let expected_chain_hex = expected_chain_hex.clone();
                                let chain_label = chain_label.clone();
                                let transfer = transfer.clone();
                                wallets.get().into_iter().map(move |wallet| {
                                    let payment_address = payment_address.clone();
                                    let expected_chain_hex = expected_chain_hex.clone();
                                    let chain_label = chain_label.clone();
                                    let transfer = transfer.clone();
                                    let provider = wallet.provider.clone();
                                    view! {
                                        <button
                                            class="ps-btn ps-btn-secondary"
                                            // Disabled while connecting, and permanently once any
                                            // wallet here has sent — see wallet_button_disabled.
                                            disabled=move || {
                                                wallet_button_disabled(&status.get())
                                            }
                                            on:click=move |_| {
                                                let provider = provider.clone();
                                                let payment_address = payment_address.clone();
                                                let expected_chain_hex = expected_chain_hex.clone();
                                                let chain_label = chain_label.clone();
                                                let transfer = transfer.clone();
                                                set_status.set(WalletActionStatus::Connecting);
                                                leptos::task::spawn_local(async move {
                                                    let outcome = connect_and_send(
                                                        &provider,
                                                        &payment_address,
                                                        &expected_chain_hex,
                                                        &chain_label,
                                                        &transfer,
                                                    )
                                                    .await;
                                                    if matches!(outcome, WalletActionStatus::Sent) {
                                                        sent_lock.set(true);
                                                    }
                                                    set_status.set(outcome);
                                                });
                                            }
                                        >
                                            {wallet.name.clone()}
                                        </button>
                                    }
                                }).collect_view()
                            }
                        }
                    </div>
                    {move || match status.get() {
                        WalletActionStatus::Idle => None,
                        WalletActionStatus::Connecting => Some(view! {
                            <p class="checkout-wallet-status">"Connecting..."</p>
                        }.into_any()),
                        WalletActionStatus::WrongNetwork(chain) => Some(view! {
                            <p class="checkout-wallet-status checkout-wallet-error">
                                {format!("Wrong network — switch to {chain} in your wallet")}
                            </p>
                        }.into_any()),
                        WalletActionStatus::Sent => Some(view! {
                            <p class="checkout-wallet-status checkout-wallet-success">
                                "Transaction sent — waiting for confirmation."
                            </p>
                        }.into_any()),
                        WalletActionStatus::Error(message) => Some(view! {
                            <p class="checkout-wallet-status checkout-wallet-error">{message}</p>
                        }.into_any()),
                    }}
                </div>
            </Show>
        }
        .into_any(),
    )
}

/// Connect to one announced provider and send a transfer prefilled from the
/// invoice: chain checked, recipient and base-unit amount taken from the
/// checkout data, never from anything the wallet reports.
async fn connect_and_send(
    provider: &wasm_bindgen::JsValue,
    payment_address: &str,
    expected_chain_hex: &str,
    chain_label: &str,
    transfer: &WalletActionsContext,
) -> WalletActionStatus {
    use crate::services::eip6963::provider_request;

    let accounts = match provider_request(
        provider,
        "eth_requestAccounts",
        &js_sys::Array::new().into(),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return WalletActionStatus::Error(e),
    };
    // `eth_sendTransaction`'s `from` is required by every wallet that
    // implements it — without it the send below reaches the wallet and is
    // rejected there, so the connected account has to survive past the
    // "is anyone connected" check rather than being discarded after it.
    let first_element = js_sys::Array::from(&accounts).get(0);
    let first_account = if first_element.is_undefined() {
        FirstAccountElement::Missing
    } else {
        match first_element.as_string() {
            Some(s) => FirstAccountElement::String(s),
            None => FirstAccountElement::NotAString,
        }
    };
    let Some(from_address) = choose_from_address(&first_account) else {
        return WalletActionStatus::Error("wallet returned no account".to_string());
    };

    let reported =
        match provider_request(provider, "eth_chainId", &js_sys::Array::new().into()).await {
            Ok(v) => v,
            Err(e) => return WalletActionStatus::Error(e),
        };
    if !network_matches(reported.as_string().as_deref(), expected_chain_hex) {
        return WalletActionStatus::WrongNetwork(chain_label.to_string());
    }

    let fields = match build_transfer_tx_fields(payment_address, transfer) {
        Ok(fields) => fields,
        Err(e) => return WalletActionStatus::Error(e),
    };

    let tx = js_sys::Object::new();
    let set_ok = js_sys::Reflect::set(&tx, &"from".into(), &from_address.into()).is_ok()
        && match fields {
            TransferTxFields::Erc20 { to, data } => {
                js_sys::Reflect::set(&tx, &"to".into(), &to.into()).is_ok()
                    && js_sys::Reflect::set(&tx, &"data".into(), &data.into()).is_ok()
            }
            TransferTxFields::Native { to, value } => {
                js_sys::Reflect::set(&tx, &"to".into(), &to.into()).is_ok()
                    && js_sys::Reflect::set(&tx, &"value".into(), &value.into()).is_ok()
            }
        };
    if !set_ok {
        return WalletActionStatus::Error("failed to build transaction".to_string());
    }

    let params = js_sys::Array::new();
    params.push(&tx);

    match provider_request(provider, "eth_sendTransaction", &params.into()).await {
        Ok(_) => WalletActionStatus::Sent,
        Err(e) => WalletActionStatus::Error(e),
    }
}

/// Whether a wallet's `eth_chainId` reply matches the invoice's chain.
///
/// Pulled out of `connect_and_send` as a plain string comparison so it can be
/// unit-tested without a mocked EIP-1193 provider — the JS-interop call
/// around it (`reported.as_string()`) is the only part that actually needs
/// wasm.
fn network_matches(reported: Option<&str>, expected_chain_hex: &str) -> bool {
    reported.is_some_and(|r| r.eq_ignore_ascii_case(expected_chain_hex))
}

/// What `eth_requestAccounts` put at index zero of its reply, before any
/// interpretation of it as an address.
///
/// A plain enum rather than `Option<&str>` so "present but not a string" has
/// its own case: `JsValue::as_string()` already collapses that case and
/// "array empty" to the same `None`, and doing the same collapse here would
/// make the two indistinguishable to a unit test, same as the value it
/// replaces did. Built at the call site, the only part that needs a live
/// provider reply and a JS engine to run at all.
enum FirstAccountElement {
    /// The wallet returned no accounts.
    Missing,
    /// Present, but not the string type every wallet reports an address as.
    NotAString,
    String(String),
}

/// The account `eth_sendTransaction`'s `from` is filled in with: element zero
/// of `eth_requestAccounts`'s reply, required to be a non-empty string.
///
/// Pulled out of `connect_and_send` so the decision can be unit-tested
/// without a mocked EIP-1193 provider, same reasoning as `network_matches`.
/// A wallet that answers with a missing account, a non-string element zero,
/// or an empty string must not produce a `from` at all: forwarding it here
/// would build a transaction the wallet rejects on its own, surfacing to the
/// customer as the wallet misbehaving rather than as this bug.
fn choose_from_address(first_account: &FirstAccountElement) -> Option<&str> {
    match first_account {
        FirstAccountElement::String(s) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    }
}

/// The `to`/`data`/`value` fields `connect_and_send` puts on the
/// transaction, decided by whether the invoice is a token transfer or a
/// native-asset one.
enum TransferTxFields {
    Erc20 { to: String, data: String },
    Native { to: String, value: String },
}

/// Which of the two transfer shapes to send, and the fields for it. Pulled
/// out of `connect_and_send` so the branch that decides *where a customer's
/// funds go* — a token payment must land `to` the token contract with ABI
/// calldata, a native one `to` the payment address with a `value` — can be
/// unit-tested without a mocked EIP-1193 provider, same reasoning as
/// `network_matches` and `wallet_actions_gate`.
fn build_transfer_tx_fields(
    payment_address: &str,
    transfer: &WalletActionsContext,
) -> Result<TransferTxFields, String> {
    match &transfer.token_address {
        Some(token) => erc20_transfer_calldata(payment_address, &transfer.amount).map(|data| {
            TransferTxFields::Erc20 {
                to: token.clone(),
                data,
            }
        }),
        None => native_transfer_value_hex(&transfer.amount).map(|value| TransferTxFields::Native {
            to: payment_address.to_string(),
            value,
        }),
    }
}

/// The two bail-outs that decide whether `render_wallet_actions` has enough
/// to work with at all: a numeric chain id to check the wallet against, and
/// invoice data to prefill the transfer from. Pulled out as a plain function,
/// same reasoning as `network_matches` — a mounted view can't be unit-tested
/// without a browser, but the decision behind it can.
fn wallet_actions_gate(
    chain_id: &ChainId,
    transfer: Option<WalletActionsContext>,
) -> Option<(String, WalletActionsContext)> {
    let evm_chain_id = chain_id.evm_chain_id()?;
    Some((format!("0x{evm_chain_id:x}"), transfer?))
}

/// Whether the wallet list (and everything under it) should render at all.
///
/// An empty list is the normal "no wallet installed" state, not an error —
/// this stays `false` so copy-address/QR remain the only thing shown.
fn should_show_wallet_list(wallets: &[crate::services::eip6963::DiscoveredWallet]) -> bool {
    !wallets.is_empty()
}

/// The wallet-status signal's starting value: `Sent` if a previous render of
/// this slot already sent, `Idle` otherwise. Pulled out as a plain function,
/// same reasoning as `network_matches` — a signal created fresh inside a
/// view can't be unit-tested, but the decision behind its initial value can.
/// This is the piece that closes the double-send gap across a resource
/// refetch: without it, a rebuilt slot always starts `Idle` regardless of
/// what `sent_lock` says, re-enabling every wallet button.
fn initial_wallet_status(previously_sent: bool) -> WalletActionStatus {
    if previously_sent {
        WalletActionStatus::Sent
    } else {
        WalletActionStatus::Idle
    }
}

/// Whether a wallet button should be disabled: while a send is in flight,
/// and permanently once any wallet in the list has sent. `status` is one
/// signal shared by the whole list, and nothing ever moves it back to
/// `Idle` — so without also covering `Sent`, the instant
/// `eth_sendTransaction` is accepted (well before the block confirms) every
/// button re-enables, and a second click on it or a different wallet
/// submits the same payment again.
fn wallet_button_disabled(status: &WalletActionStatus) -> bool {
    matches!(
        status,
        WalletActionStatus::Connecting | WalletActionStatus::Sent
    )
}

/// Native-asset transfer value, hex-encoded for `eth_sendTransaction`.
///
/// `amount_base_units` must already be base units (wei), never a display
/// amount — `PaymentOptionResponse::amount` is documented in
/// `payserver-commons` as "amount in the asset's smallest unit", the same
/// contract `format_crypto_amount` and `payment_request_uri` already rely on
/// in `checkout.rs` for this same field. A display string like `"1"` (meaning
/// 1 ETH) would parse here without error and send 1 wei instead — refusing
/// non-integer input catches the obvious case of that mistake.
fn native_transfer_value_hex(amount_base_units: &str) -> Result<String, String> {
    amount_base_units
        .parse::<u128>()
        .map(|amount| format!("0x{amount:x}"))
        .map_err(|_| "malformed invoice amount".to_string())
}

/// ABI-encoded `transfer(address,uint256)` call — the selector is the first
/// four bytes of `keccak256("transfer(address,uint256)")`, `0xa9059cbb`, a
/// fixed constant rather than something to hash at runtime.
fn erc20_transfer_calldata(recipient: &str, amount_base_units: &str) -> Result<String, String> {
    let recipient_hex = recipient
        .strip_prefix("0x")
        .filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| "malformed recipient address".to_string())?;
    let amount: u128 = amount_base_units
        .parse()
        .map_err(|_| "malformed invoice amount".to_string())?;

    Ok(format!(
        "0xa9059cbb{:0>64}{amount:064x}",
        recipient_hex.to_lowercase()
    ))
}

#[cfg(test)]
mod wallet_action_tests {
    use super::*;

    /// `checkout.rs` calls this slot through `checkout_plugin()`, not
    /// `render_wallet_actions` directly — nothing else here would notice if
    /// a future edit dropped the field back to `None` or swapped in a
    /// no-op, silently reintroducing the unwired-slot bug this ticket
    /// exists to fix.
    #[test]
    fn checkout_plugin_wires_up_wallet_actions() {
        assert!(checkout_plugin().wallet_actions.is_some());
    }

    const RECIPIENT: &str = "0x66da354a361225a8C4FF5232f413A80af943C14c";

    #[test]
    fn erc20_calldata_targets_the_selector_and_pads_address_and_amount_to_32_bytes() {
        let calldata = erc20_transfer_calldata(RECIPIENT, "1000000").expect("calldata");
        assert_eq!(
            calldata,
            "0xa9059cbb\
             00000000000000000000000066da354a361225a8c4ff5232f413a80af943c14c\
             00000000000000000000000000000000000000000000000000000000000f4240"
        );
    }

    /// A masked or truncated address looks plausible but points nowhere — the
    /// same mistake `payment_request_uri` in `types` guards against, and just
    /// as costly here since this calldata is what the wallet actually sends.
    #[test]
    fn a_malformed_recipient_is_refused() {
        assert!(erc20_transfer_calldata("not-an-address", "1000000").is_err());
        assert!(erc20_transfer_calldata("0x1234", "1000000").is_err());
        assert!(erc20_transfer_calldata("", "1000000").is_err());
    }

    /// Base units only, like the EIP-681 URI path — a decimal amount here
    /// would encode the wrong quantity into a transaction a customer is about
    /// to sign.
    #[test]
    fn a_non_integer_amount_is_refused() {
        assert!(erc20_transfer_calldata(RECIPIENT, "0.047").is_err());
        assert!(erc20_transfer_calldata(RECIPIENT, "").is_err());
    }

    /// Wei-scale, not display-scale — the same magnitude `option.amount`
    /// actually carries from the checkout API (e.g. ~0.047 ETH is a
    /// 17-digit wei string, not `"0.047"`).
    #[test]
    fn native_transfer_value_hex_encodes_wei_scale_base_units() {
        assert_eq!(
            native_transfer_value_hex("47351100518494550").unwrap(),
            "0xa839933620f156"
        );
        assert_eq!(native_transfer_value_hex("0").unwrap(), "0x0");
    }

    /// A display amount like `"1"` (meaning 1 ETH) parses as `u128` without
    /// error and would silently send 1 wei — this only catches the
    /// non-integer shape of that mistake, but a decimal display amount
    /// specifically must still be refused rather than mis-encoded.
    #[test]
    fn a_decimal_display_amount_is_refused_not_silently_miscoded() {
        assert!(native_transfer_value_hex("0.047").is_err());
        assert!(native_transfer_value_hex("").is_err());
    }

    #[test]
    fn network_matches_accepts_an_exact_chain_id() {
        assert!(network_matches(Some("0x1"), "0x1"));
    }

    /// Wallets are inconsistent about hex case in `eth_chainId` replies —
    /// the comparison has to ignore it or a legitimately-matching wallet
    /// gets told it's on the wrong network.
    #[test]
    fn network_matches_ignores_hex_case() {
        assert!(network_matches(Some("0X1"), "0x1"));
        assert!(network_matches(Some("0xA86A"), "0xa86a"));
    }

    #[test]
    fn network_matches_rejects_a_different_chain() {
        assert!(!network_matches(Some("0x89"), "0x1"));
    }

    /// A provider that answers `eth_chainId` with something other than a
    /// plain string (`None` here stands in for `reported.as_string()`
    /// returning `None`) must fail closed, not be treated as a match.
    #[test]
    fn network_matches_rejects_a_non_string_reply() {
        assert!(!network_matches(None, "0x1"));
    }

    /// No accounts connected — the normal reply from a wallet the user has
    /// just declined to connect.
    #[test]
    fn choose_from_address_rejects_an_empty_accounts_array() {
        assert!(choose_from_address(&FirstAccountElement::Missing).is_none());
    }

    /// Element zero present but not a string — a distinct case from a
    /// missing account, must fail closed rather than forward whatever this
    /// is into a transaction's `from` field.
    #[test]
    fn choose_from_address_rejects_a_non_string_first_element() {
        assert!(choose_from_address(&FirstAccountElement::NotAString).is_none());
    }

    /// Looks redundant next to the string-type check and is the piece most
    /// likely to be dropped by a later refactor — an empty string is a
    /// string, so without this filter it would pass through as a `from`
    /// address.
    #[test]
    fn choose_from_address_rejects_an_empty_string_first_element() {
        assert!(choose_from_address(&FirstAccountElement::String(String::new())).is_none());
    }

    #[test]
    fn choose_from_address_carries_through_the_exact_address() {
        assert_eq!(
            choose_from_address(&FirstAccountElement::String(RECIPIENT.to_string())),
            Some(RECIPIENT)
        );
    }

    fn some_transfer() -> WalletActionsContext {
        WalletActionsContext {
            amount: "1000000".to_string(),
            token_address: None,
            sent_lock: leptos::prelude::RwSignal::new(false),
        }
    }

    /// A token invoice must target the token contract with ABI calldata,
    /// never the payment address directly — sending `value` instead of
    /// `data` here would move nothing, and sending `to = payment_address`
    /// would send the native asset instead of the token.
    #[test]
    fn build_transfer_tx_fields_targets_the_token_contract_for_a_token_invoice() {
        let transfer = WalletActionsContext {
            amount: "1000000".to_string(),
            token_address: Some("0xTokenContract".to_string()),
            sent_lock: leptos::prelude::RwSignal::new(false),
        };
        match build_transfer_tx_fields(RECIPIENT, &transfer).expect("fields") {
            TransferTxFields::Erc20 { to, data } => {
                assert_eq!(to, "0xTokenContract");
                assert_eq!(data, erc20_transfer_calldata(RECIPIENT, "1000000").unwrap());
            }
            TransferTxFields::Native { .. } => {
                panic!("a token invoice must not build a native transfer")
            }
        }
    }

    /// A native-asset invoice must target the payment address with a
    /// `value`, never the (absent) token contract.
    #[test]
    fn build_transfer_tx_fields_targets_the_payment_address_for_a_native_invoice() {
        let transfer = some_transfer();
        match build_transfer_tx_fields(RECIPIENT, &transfer).expect("fields") {
            TransferTxFields::Native { to, value } => {
                assert_eq!(to, RECIPIENT);
                assert_eq!(value, native_transfer_value_hex("1000000").unwrap());
            }
            TransferTxFields::Erc20 { .. } => {
                panic!("a native invoice must not build an ERC-20 transfer")
            }
        }
    }

    #[test]
    fn build_transfer_tx_fields_rejects_a_malformed_amount() {
        let transfer = WalletActionsContext {
            amount: "0.047".to_string(),
            token_address: None,
            sent_lock: leptos::prelude::RwSignal::new(false),
        };
        assert!(build_transfer_tx_fields(RECIPIENT, &transfer).is_err());
    }

    /// A non-EVM chain (or a malformed `eip155:` reference) has no chain id
    /// to check the wallet against — the slot must degrade to nothing rather
    /// than skip the network guard it exists to enforce.
    #[test]
    fn wallet_actions_gate_bails_on_a_non_evm_chain() {
        let tron = ChainId::parse("tron:728126428").unwrap();
        assert!(wallet_actions_gate(&tron, Some(some_transfer())).is_none());
    }

    /// Called from anywhere other than checkout's payment flow, there's no
    /// invoice data to prefill a transfer from — degrade to nothing rather
    /// than a wallet-connect UI with nothing to send.
    #[test]
    fn wallet_actions_gate_bails_on_missing_context() {
        assert!(wallet_actions_gate(&ChainId::evm(1), None).is_none());
    }

    #[test]
    fn wallet_actions_gate_passes_through_an_evm_chain_with_context() {
        let (expected_chain_hex, transfer) =
            wallet_actions_gate(&ChainId::evm(137), Some(some_transfer())).expect("gate open");
        assert_eq!(expected_chain_hex, "0x89");
        assert_eq!(transfer.amount, "1000000");
    }

    fn wallet(uuid: &str) -> crate::services::eip6963::DiscoveredWallet {
        crate::services::eip6963::DiscoveredWallet {
            uuid: uuid.to_string(),
            name: "Test Wallet".to_string(),
            icon: String::new(),
            provider: wasm_bindgen::JsValue::UNDEFINED,
        }
    }

    /// No wallet found is a normal state, not an error — copy-address/QR
    /// must stay the only thing shown.
    #[test]
    fn no_wallets_does_not_show_the_list() {
        assert!(!should_show_wallet_list(&[]));
    }

    #[test]
    fn a_discovered_wallet_shows_the_list() {
        assert!(should_show_wallet_list(&[wallet("one")]));
    }

    /// A send from this button is already in flight.
    #[test]
    fn a_connecting_wallet_button_is_disabled() {
        assert!(wallet_button_disabled(&WalletActionStatus::Connecting));
    }

    /// `eth_sendTransaction` already broadcast a payment for this invoice —
    /// nothing moves `status` back to `Idle`, so the button must stay
    /// disabled rather than let a second click submit the same payment
    /// again before the first one confirms.
    #[test]
    fn a_sent_wallet_button_stays_disabled() {
        assert!(wallet_button_disabled(&WalletActionStatus::Sent));
    }

    #[test]
    fn wallet_buttons_stay_enabled_outside_a_pending_or_completed_send() {
        assert!(!wallet_button_disabled(&WalletActionStatus::Idle));
        assert!(!wallet_button_disabled(&WalletActionStatus::WrongNetwork(
            "Ethereum".to_string()
        )));
        assert!(!wallet_button_disabled(&WalletActionStatus::Error(
            "boom".to_string()
        )));
    }

    /// This is the guard that closes the double-send gap: without it, a
    /// slot rebuilt after a poll tick or websocket update — which happens
    /// while a just-sent payment is still unconfirmed — would always start
    /// its status signal at `Idle`, re-enabling every wallet button
    /// regardless of `sent_lock`.
    #[test]
    fn initial_wallet_status_locks_immediately_if_a_previous_render_already_sent() {
        assert!(matches!(
            initial_wallet_status(true),
            WalletActionStatus::Sent
        ));
        assert!(wallet_button_disabled(&initial_wallet_status(true)));
    }

    #[test]
    fn initial_wallet_status_starts_idle_when_nothing_has_sent_yet() {
        assert!(matches!(
            initial_wallet_status(false),
            WalletActionStatus::Idle
        ));
        assert!(!wallet_button_disabled(&initial_wallet_status(false)));
    }
}

/// Mount the app into `#app`, clearing whatever placeholder is there first.
///
/// This is the single mounting implementation. Trunk builds the cdylib
/// (`data-target-name="payserver_client"` in index.html), so `init` below is
/// the entry point that actually ships; `src/main.rs` exists for the standalone
/// bin target. Both call this, so a fix to one cannot silently miss the other —
/// which is exactly how the first-paint loader survived its own removal code.
pub fn mount_app() {
    use leptos::mount::mount_to;
    use leptos::prelude::*;
    use wasm_bindgen::JsCast;

    console_error_panic_hook::set_once();
    // Forward panics / uncaught JS errors to the error telemetry backend. A
    // no-op unless the `telemetry-dsn` meta tag in index.html is filled in —
    // see `telemetry`.
    telemetry::init();
    web_sys::console::log_1(&"EVM PayServer: Starting app...".into());

    // Mount to the #app div
    let app_element = document()
        .get_element_by_id("app")
        .expect("Could not find #app element")
        .unchecked_into::<web_sys::HtmlElement>();

    web_sys::console::log_1(&"EVM PayServer: Found #app element, mounting...".into());

    // Remove the inline first-paint loader (#initial-loader in index.html).
    // Leptos `mount_to` appends to #app rather than replacing its contents, so
    // the placeholder must be cleared explicitly or it covers the mounted app
    // forever (it is position:fixed; inset:0).
    app_element.set_inner_html("");

    mount_to(app_element, App).forget();

    web_sys::console::log_1(&"EVM PayServer: App mounted successfully".into());
}

/// Initialize and mount the app (called when loaded as WASM module).
#[wasm_bindgen(start)]
pub fn init() {
    mount_app();
}
