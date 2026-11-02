use leptos::prelude::*;

use crate::api::Wallet;

use super::icons::*;

/// Addresses tab - shows derived addresses.
#[component]
pub(super) fn AddressesTab(wallet: Wallet) -> impl IntoView {
    // In production, addresses are derived server-side from the wallet's xpub.
    // For now, show the derivation index range that has been used.
    let used_count = wallet.derivation_index as usize;

    view! {
        <div class="wallet-tab-addresses">
            <div class="section-header">
                <div>
                    <h3 class="section-title">"Derived Addresses"</h3>
                    <p class="section-desc">"Addresses generated from your wallet's extended public key"</p>
                </div>
            </div>

            <div class="ps-card">
                <div class="ps-card-body">
                    <div class="form-group">
                        <label class="form-label">"Addresses Derived"</label>
                        <div class="form-static">
                            <code>{used_count}" addresses derived so far"</code>
                        </div>
                        <p class="form-help">"New addresses are derived automatically when invoices are created"</p>
                    </div>
                </div>
            </div>

            <div class="addresses-info">
                <IconInfo />
                <p>"Addresses are derived sequentially from your wallet's xpub using the BIP-44 standard. Each invoice uses a unique address for payment tracking."</p>
            </div>
        </div>
    }
}
