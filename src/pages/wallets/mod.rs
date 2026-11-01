//! Wallet management pages.
//!
//! Wallets contain HD wallet (xpub) configurations that can be used
//! across stores for payment method setup.

mod addresses_tab;
mod detail;
mod general_tab;
mod icons;
mod list;

pub use detail::WalletDetailPage;
pub use list::WalletsPage;

/// Format ISO date string for display.
pub(super) fn format_date(iso: &str) -> String {
    if iso.len() >= 10 {
        let date_part = &iso[..10];
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            let month = match parts[1] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                _ => parts[1],
            };
            return format!("{} {}, {}", month, parts[2], parts[0]);
        }
    }
    iso.to_string()
}

/// Truncate address for display.
pub(super) fn truncate_address(address: &str) -> String {
    if address.len() > 16 {
        format!("{}...{}", &address[..8], &address[address.len() - 6..])
    } else {
        address.to_string()
    }
}
