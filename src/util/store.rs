//! Store display helpers shared by the invoice and payment list pages.

/// Shorten a store ID for display, or say so when there is nothing to shorten.
///
/// Used as the fallback label in the "All Stores" views when the
/// server could not resolve a store name: a row still has to say which store it
/// came from, and a bare UUID is better than a blank cell.
///
/// Truncates by chars, not bytes: the ID is a UUID in practice, but a byte
/// slice through a multi-byte char would panic, and a panic in WASM is a blank
/// page rather than one ugly cell.
#[must_use]
pub fn short_store_id(store_id: &str) -> String {
    if store_id.is_empty() {
        return "Unknown store".to_string();
    }
    if store_id.chars().count() > 8 {
        return format!("{}…", store_id.chars().take(8).collect::<String>());
    }
    store_id.to_string()
}

/// Shorten `s` to at most `max` characters, eliding the middle with '…'.
///
/// Counts characters, not bytes, and never panics or splits a UTF-8 boundary.
#[must_use]
pub fn truncate_middle(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_string();
    }
    let head = (max - 1).div_ceil(2);
    let tail = (max - 1) / 2;
    let head_part: String = chars[..head].iter().collect();
    let tail_part: String = chars[chars.len() - tail..].iter().collect();
    format!("{head_part}…{tail_part}")
}

#[cfg(test)]
mod tests {
    use super::{short_store_id, truncate_middle};

    #[test]
    fn shortens_a_uuid() {
        assert_eq!(
            short_store_id("11111111-2222-3333-4444-555555555555"),
            "11111111\u{2026}"
        );
    }

    #[test]
    fn leaves_short_ids_alone() {
        assert_eq!(short_store_id("short"), "short");
        assert_eq!(short_store_id("12345678"), "12345678");
    }

    #[test]
    fn names_the_absence_of_an_id() {
        assert_eq!(short_store_id(""), "Unknown store");
    }

    #[test]
    fn truncates_on_char_boundaries() {
        // Not a real store ID, but a byte slice here would panic and take the
        // whole page down with it.
        assert_eq!(
            short_store_id("ééééééééé"),
            "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{2026}"
        );
    }

    #[test]
    fn truncate_middle_leaves_short_and_exact_alone() {
        assert_eq!(truncate_middle("abc", 10), "abc");
        assert_eq!(truncate_middle("abcdef", 6), "abcdef");
        assert_eq!(truncate_middle("", 0), "");
    }

    #[test]
    fn truncate_middle_yields_exactly_max_chars() {
        // Even max: the extra char goes to the head.
        assert_eq!(truncate_middle("abcdefghij", 6), "abc…ij");
        // Odd max: head and tail are equal.
        assert_eq!(truncate_middle("abcdefghij", 5), "ab…ij");
        for max in 2..10 {
            assert_eq!(truncate_middle("abcdefghij", max).chars().count(), max);
        }
    }

    #[test]
    fn truncate_middle_handles_zero_and_one() {
        assert_eq!(truncate_middle("abcdef", 0), "");
        assert_eq!(truncate_middle("abcdef", 1), "…");
    }

    #[test]
    fn truncate_middle_counts_chars_not_bytes() {
        let out = truncate_middle("ααααααααββββββββ", 7);
        assert_eq!(out, "ααα…βββ");
        assert_eq!(out.chars().count(), 7);
        let out = truncate_middle("😀😀😀😀😀😀😀😀", 4);
        assert_eq!(out, "😀😀…😀");
    }
}
