//! Fiat amount display shared by every page that shows an invoice total.

use ui_kit::{round_amount, trim_amount};

/// Minor-unit digits of the ISO 4217 currencies that do not have two: zero for
/// the ones with no subunit, three for the dinar-style ones, four for the two
/// unit-of-account currencies. Codes are matched case-insensitively. Anything
/// else returns `None`, because an invoice currency is not guaranteed to be
/// fiat and a crypto-denominated amount must not be rounded to two places.
fn irregular_minor_units(currency: &str) -> Option<usize> {
    match currency.to_ascii_uppercase().as_str() {
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => Some(0),
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => Some(3),
        "CLF" | "UYW" => Some(4),
        _ => None,
    }
}

/// The invoice's total with its currency, e.g. `20.00 USD`. The API sends the
/// amount at the column's full 18 decimals. A currency whose minor unit is not
/// two is rounded to it; every other amount only has its trailing zeros
/// trimmed down to two places, so no digit of a value is ever rounded away
/// on a guess about the currency.
pub fn format_fiat_amount(amount: &str, currency: &str) -> String {
    let shown = match irregular_minor_units(currency) {
        Some(scale) => round_amount(amount, scale),
        // `trim_amount` returns a fraction-less input untouched.
        None if !amount.contains('.') => round_amount(amount, 2),
        None => trim_amount(amount, 2),
    };
    format!("{shown} {currency}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_precision_usd_shows_two_decimals() {
        assert_eq!(
            format_fiat_amount("20.000000000000000000", "USD"),
            "20.00 USD"
        );
    }

    #[test]
    fn zero_decimal_currency_shows_no_fraction() {
        assert_eq!(
            format_fiat_amount("2000.000000000000000000", "JPY"),
            "2000 JPY"
        );
    }

    #[test]
    fn three_decimal_currency_keeps_three() {
        assert_eq!(
            format_fiat_amount("1.500000000000000000", "KWD"),
            "1.500 KWD"
        );
    }

    #[test]
    fn rounds_half_up_rather_than_truncating() {
        assert_eq!(format_fiat_amount("19.9995", "KWD"), "20.000 KWD");
        assert_eq!(format_fiat_amount("19.9994", "KWD"), "19.999 KWD");
        assert_eq!(format_fiat_amount("0.999", "JPY"), "1 JPY");
        assert_eq!(format_fiat_amount("0.499", "JPY"), "0 JPY");
    }

    #[test]
    fn short_or_missing_fraction_is_padded() {
        assert_eq!(format_fiat_amount("20", "USD"), "20.00 USD");
        assert_eq!(format_fiat_amount("20.5", "USD"), "20.50 USD");
    }

    #[test]
    fn unlisted_currency_never_rounds_a_digit_away() {
        assert_eq!(format_fiat_amount("0.001", "USD"), "0.001 USD");
        assert_eq!(
            format_fiat_amount("0.000123450000000000", "BTC"),
            "0.00012345 BTC"
        );
    }

    #[test]
    fn whole_number_in_a_zero_decimal_currency_is_unchanged() {
        assert_eq!(format_fiat_amount("2000", "JPY"), "2000 JPY");
    }

    #[test]
    fn lowercase_code_uses_the_same_precision() {
        assert_eq!(format_fiat_amount("2000.0", "jpy"), "2000 jpy");
    }

    #[test]
    fn four_decimal_unit_of_account_keeps_four() {
        assert_eq!(format_fiat_amount("1.5", "CLF"), "1.5000 CLF");
    }
}
