//! Formatting and status helpers for payment rows.
//!
//! Shared across the payments pages and the dashboard's recent-payments panel,
//! so a payment reads the same wherever it is shown.

use crate::api::Payment;
use std::collections::HashMap;
use ui_kit::{trim_amount, units_to_decimal};

/// Helper to determine payment status display.
pub(crate) fn payment_status(payment: &Payment) -> &'static str {
    if payment.reorged {
        "reorged"
    } else if payment.confirmed_at.is_some() {
        "confirmed"
    } else {
        "pending"
    }
}

/// CSS class for payment status badge.
pub(crate) fn payment_status_class(payment: &Payment) -> &'static str {
    if payment.reorged {
        "badge badge-error"
    } else if payment.confirmed_at.is_some() {
        "badge badge-success"
    } else {
        "badge badge-warning"
    }
}

/// Format ISO date string for display.
pub(crate) fn format_date(iso: &str) -> String {
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

/// Truncate address/hash for display.
pub(crate) fn truncate_hash(hash: &str, prefix: usize, suffix: usize) -> String {
    if hash.len() > prefix + suffix + 3 {
        format!("{}...{}", &hash[..prefix], &hash[hash.len() - suffix..])
    } else {
        hash.to_string()
    }
}

/// Fractional digits each asset's amounts are padded to in the list.
///
/// Compression is decided per value, so a column mixing `0.000139292` with
/// `0.0₄85129` cannot be compared down the page. The list shows every amount
/// of an asset as a plain literal at the widest scale on the page instead, so
/// the digits line up; subscript compression stays on the single-value
/// summaries (dashboard metric tiles) where there is nothing to compare against.
pub(crate) fn column_scales(payments: &[Payment]) -> HashMap<String, usize> {
    let mut scales = HashMap::new();
    for payment in payments {
        let literal = trim_amount(&units_to_decimal(&payment.amount, payment.decimals), 0);
        let scale = literal.split_once('.').map_or(0, |(_, frac)| frac.len());
        let widest = scales.entry(payment.asset_symbol.clone()).or_insert(0);
        *widest = (*widest).max(scale);
    }
    scales
}
