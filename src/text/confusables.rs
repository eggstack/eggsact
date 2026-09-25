/// Sorted static table of Unicode confusable mappings.
///
/// Generated from confusables.txt (Unicode UTS #39, Intentionally
/// Confusable / Mixed-Script detection data).
///
/// Key: source code point (u32). Value: substitution string (e.g. "U+0041").
///
/// # Semantic distinction
///
/// - [`lookup`], [`has_confusables`], and [`find_confusables`] report
///   **source mappings**: whether individual characters appear in the
///   confusables table. They do NOT deliver a whole-string collision
///   verdict.
/// - [`confusable_skeleton`] and [`are_confusable`] implement the
///   **whole-string UTS #39 skeleton relation**: two strings are confusable
///   when their skeletons are exactly equal. Collision detection must use
///   the skeleton relation, not shared per-character mapping overlap.
pub static CONFUSABLES: &[(u32, &str)] = &include!("confusables_generated.rs");

/// Unicode Security data epoch backing [`CONFUSABLES`].
///
/// Single source of truth for provenance. Must match the
/// `// Unicode version:` header in `confusables_generated.rs` and the pin
/// in `scripts/generate_confusables.py`.
pub const CONFUSABLES_UNICODE_VERSION: &str = "17.0.0";

/// SHA-256 of the pinned `confusables.txt` source bytes.
pub const CONFUSABLES_SOURCE_SHA256: &str =
    "091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a";

/// Number of checked-in confusable mappings at this data epoch.
pub const CONFUSABLES_ENTRY_COUNT: usize = 6565;

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

/// Expand one confusable substitution string (e.g. `"U+0041 U+0045"`) into
/// its mapped characters, skipping unparseable components.
fn expand_substitution(sub: &str) -> Vec<char> {
    let mut out = Vec::new();
    for part in sub.split_whitespace() {
        let hex = match part.strip_prefix("U+") {
            Some(hex) => hex,
            None => continue,
        };
        let cp = match u32::from_str_radix(hex, 16) {
            Ok(cp) => cp,
            Err(_) => continue,
        };
        if let Some(c) = char::from_u32(cp) {
            out.push(c);
        }
    }
    out
}

/// Whole-string UTS #39 confusable skeleton for the pinned data epoch.
///
/// Algorithm (compatible with the pinned UTS #39 / Unicode 17 security
/// data): NFD-normalize the input, replace each character by its confusable
/// mapping target characters when the table contains one, then NFD-normalize
/// the result so the skeleton is idempotent.
///
/// Two strings are confusable if and only if their skeletons are exactly
/// equal (see [`are_confusable`]). A non-empty skeleton mapping on one
/// input alone is NOT a collision verdict.
pub fn confusable_skeleton(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut out = String::with_capacity(text.len());
    for c in text.nfd().collect::<String>().chars() {
        match lookup(c) {
            Some(sub) => {
                let expanded = expand_substitution(sub);
                if expanded.is_empty() {
                    out.push(c);
                } else {
                    out.extend(expanded);
                }
            }
            None => out.push(c),
        }
    }
    out.nfd().collect()
}

/// Whole-string confusability: true when both inputs share the exact same
/// [`confusable_skeleton`] and the raw inputs differ.
///
/// Identical inputs are trivially equal, not confusable: this reports
/// whether two *distinct* strings would be confused with each other.
pub fn are_confusable(a: &str, b: &str) -> bool {
    a != b && confusable_skeleton(a) == confusable_skeleton(b)
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
        // Exact count at the pinned epoch: the table is regenerated from
        // version-pinned Unicode Security data, so any shift must come with
        // an intentional data-epoch change (Milestone 004), never silently.
        assert_eq!(
            CONFUSABLES.len(),
            CONFUSABLES_ENTRY_COUNT,
            "Confusables table drifted from the pinned epoch count"
        );
    }

    #[test]
    fn test_provenance_matches_generated_header() {
        let generated = include_str!("confusables_generated.rs");
        assert!(
            generated.starts_with(&format!(
                "// Unicode version: {}",
                CONFUSABLES_UNICODE_VERSION
            )),
            "generated header version must match CONFUSABLES_UNICODE_VERSION"
        );
        assert!(
            generated.contains(CONFUSABLES_SOURCE_SHA256),
            "generated header checksum must match CONFUSABLES_SOURCE_SHA256"
        );
        assert_eq!(CONFUSABLES_UNICODE_VERSION, "17.0.0");
    }

    #[test]
    fn test_skeleton_cyrillic_spoof_collides() {
        // Latin "apple" vs Cyrillic-"аpple" (U+0430 -> U+0061): the canonical
        // Milestone 003 true-positive example.
        assert_eq!(confusable_skeleton("apple"), "apple");
        assert_eq!(confusable_skeleton("аpple"), "apple");
        assert!(are_confusable("apple", "аpple"));
    }

    #[test]
    fn test_skeleton_multi_code_point_target() {
        // Æ maps to the two-code-point sequence U+0041 U+0045.
        assert_eq!(confusable_skeleton("Æ"), "AE");
        assert!(are_confusable("Æ", "AE"));
    }

    #[test]
    fn test_skeleton_shared_component_is_not_collision() {
        // Cyrillic А (U+0410) and Greek Α (U+0391) both map to U+0041, but
        // whole-string skeletons "AX" vs "AY" differ: shared per-character
        // mapping overlap is NOT a collision verdict. The old heuristic
        // flagged this class as confusable (false positive).
        assert!(!are_confusable("АX", "ΑY"));
        assert!(!are_confusable("apple", "orange"));
    }

    #[test]
    fn test_skeleton_identical_inputs_are_not_confusable() {
        assert!(!are_confusable("apple", "apple"));
        assert!(!are_confusable("", ""));
    }

    #[test]
    fn test_skeleton_deterministic_and_idempotent() {
        for input in ["hello", "apple", "аpple", "Æ", "ß", "", "café", "日本語"] {
            assert_eq!(confusable_skeleton(input), confusable_skeleton(input));
            let once = confusable_skeleton(input);
            assert_eq!(
                confusable_skeleton(&once),
                once,
                "skeleton must be idempotent for {input:?}"
            );
        }
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
