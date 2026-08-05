/// Sorted static table of Unicode confusable mappings.
/// Generated from confusables.txt (Unicode UTS #39).
/// Key: source code point (u32). Value: substitution string (e.g. "U+0041").
pub static CONFUSABLES: &[(u32, &str)] = &include!("confusables_generated.rs");

/// Look up the confusable substitution for a single character.
pub fn lookup(c: char) -> Option<&'static str> {
    let cp = c as u32;
    CONFUSABLES
        .binary_search_by_key(&cp, |(code_point, _)| *code_point)
        .ok()
        .map(|idx| CONFUSABLES[idx].1)
}

pub fn has_confusables(text: &str) -> bool {
    text.chars().any(|c| lookup(c).is_some())
}

pub fn find_confusables(text: &str) -> Vec<(char, &'static str)> {
    text.chars()
        .filter_map(|c| lookup(c).map(|sub| (c, sub)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confusables_loaded() {
        assert!(CONFUSABLES.len() > 1400, "Confusables should have entries");
    }

    #[test]
    fn test_cyrillic_a_confusable() {
        assert_eq!(lookup('А'), Some("U+0041"));
    }

    #[test]
    fn test_sorted_no_duplicates() {
        for window in CONFUSABLES.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "Table must be strictly sorted: 0x{:04X} >= 0x{:04X}",
                window[0].0,
                window[1].0
            );
        }
    }

    #[test]
    fn test_entry_count() {
        assert_eq!(CONFUSABLES.len(), 6565);
    }

    #[test]
    fn test_representative_substitutions() {
        // ASCII
        assert_eq!(lookup('"'), Some("U+0027 U+0027"));
        // Greek
        assert_eq!(lookup('µ'), Some("U+03BC"));
        // Cyrillic
        assert_eq!(lookup('А'), Some("U+0041"));
        // Supplementary plane
        assert_eq!(lookup('\u{2FA1D}'), Some("U+2A600"));
        // Multi-code-point substitution
        assert_eq!(lookup('Æ'), Some("U+0041 U+0045"));
    }
}
