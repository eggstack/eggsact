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
pub const CONFUSABLES_UNICODE_VERSION: &str = "18.0.0";

/// SHA-256 of the pinned `confusables.txt` source bytes.
pub const CONFUSABLES_SOURCE_SHA256: &str =
    "6ed3ee967c9dfdf6677d563c9985182fbc50a2efb7d6059cd57b2e2ce18f5b92";

/// Number of checked-in confusable mappings at this data epoch.
pub const CONFUSABLES_ENTRY_COUNT: usize = 6712;

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

/// UTS #39 internal skeleton: NFD → remove `Default_Ignorable_Code_Point`
/// → confusables prototype mapping → NFD.
///
/// This is the internal transformation; the public whole-string operation is
/// [`confusable_skeleton`] (`skeleton(X) = bidiSkeleton(LTR, X)`). Exposed for
/// conformance testing and for callers that need the pre-bidi stage.
pub fn internal_skeleton(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut out = String::with_capacity(text.len());
    for c in text.nfd().collect::<String>().chars() {
        if crate::text::unicode_properties::is_default_ignorable(c) {
            continue;
        }
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

/// UTS #39 `bidiSkeleton(LTR, X)` (Revision 34, Unicode 18.0.0).
///
/// Steps: UAX #9 up to L2 with paragraph level LTR (via `unicode-bidi` with
/// the pinned Unicode 18 [`crate::text::unicode_properties::Unicode18BidiData`]
/// source) → L3 combining-mark fixup → L4 mirroring via Unicode 18
/// `Bidi_Mirroring_Glyph` → [`internal_skeleton`].
///
/// Fast path (per spec): when `X` contains no Bidi_Class R/AL characters,
/// the result equals [`internal_skeleton`] directly.
pub fn bidi_skeleton_ltr(text: &str) -> String {
    use unicode_bidi::{BidiInfo, Level};
    use unicode_normalization::UnicodeNormalization as _;

    // Fast path: no R/AL → internal skeleton.
    if !text
        .chars()
        .any(crate::text::unicode_properties::is_bidi_r_or_al)
    {
        return internal_skeleton(text);
    }

    let data = crate::text::unicode_properties::Unicode18BidiData;
    let info = BidiInfo::new_with_data_source(&data, text, Some(Level::ltr()));
    if info.paragraphs.is_empty() {
        return internal_skeleton(text);
    }

    // Reorder each paragraph independently; separators between paragraphs
    // stay in logical order (they are Common/B/WS and survive the skeleton
    // unless Default_Ignorable).
    let mut reordered = String::with_capacity(text.len());
    let mut last_end = 0usize;
    for para in &info.paragraphs {
        // Preserve inter-paragraph separators verbatim.
        if para.range.start > last_end {
            reordered.push_str(&text[last_end..para.range.start]);
        }
        let para_text = &text[para.range.clone()];
        let para_chars: Vec<char> = para_text.chars().collect();
        if para_chars.is_empty() {
            last_end = para.range.end;
            continue;
        }
        // Levels per character (logical order) after L1.
        let levels_per_char: Vec<Level> = {
            let levels = info.reordered_levels_per_char(para, para.range.clone());
            // `reordered_levels_per_char` returns one level per char.
            debug_assert_eq!(levels.len(), para_chars.len());
            if levels.len() == para_chars.len() {
                levels
            } else {
                // Fallback: should not happen; treat as LTR.
                vec![Level::ltr(); para_chars.len()]
            }
        };
        // L2: visual order map (visual index → logical index).
        let index_map = BidiInfo::reorder_visual(&levels_per_char);
        // Build visual-order chars + levels.
        let mut visual: Vec<(char, Level)> = Vec::with_capacity(para_chars.len());
        for &logical_idx in &index_map {
            if let Some(&c) = para_chars.get(logical_idx) {
                let lvl = levels_per_char
                    .get(logical_idx)
                    .copied()
                    .unwrap_or_else(Level::ltr);
                visual.push((c, lvl));
            }
        }
        // L3: move combining marks after their base in visual order.
        // Bubble each mark right past an immediately following base.
        let is_mark = |c: char| {
            unicode_general_category::get_general_category(c)
                .abbreviation()
                .starts_with('M')
        };
        let mut i = 0usize;
        while i + 1 < visual.len() {
            let (c, _) = visual[i];
            let (nxt, _) = visual[i + 1];
            if is_mark(c) && !is_mark(nxt) {
                visual.swap(i, i + 1);
                i = i.saturating_sub(1);
            } else {
                i += 1;
            }
        }
        // L4: mirror chars with odd (RTL) resolved levels.
        for (c, lvl) in visual.iter_mut() {
            if lvl.is_rtl() {
                if let Some(m) = crate::text::unicode_properties::bidi_mirror(*c) {
                    *c = m;
                }
            }
        }
        for (c, _) in visual {
            reordered.push(c);
        }
        last_end = para.range.end;
    }
    if last_end < text.len() {
        reordered.push_str(&text[last_end..]);
    }
    // Silence unused import in fast-path-only builds (NFD used in internal).
    let _ = || {
        let _: String = "".nfd().collect();
    };
    internal_skeleton(&reordered)
}

/// Whole-string UTS #39 confusable skeleton for the pinned data epoch.
///
/// Implements `skeleton(X) = bidiSkeleton(LTR, X)` (UTS #39 Revision 34,
/// Unicode 18.0.0): directional reordering/mirroring via the pinned UAX #9
/// tables, then the internal skeleton (NFD → Default_Ignorable removal →
/// confusables mapping → NFD).
///
/// Two strings are confusable if and only if their skeletons are exactly
/// equal (see [`are_confusable`]). A non-empty skeleton mapping on one
/// input alone is NOT a collision verdict.
pub fn confusable_skeleton(text: &str) -> String {
    bidi_skeleton_ltr(text)
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
        assert_eq!(CONFUSABLES_UNICODE_VERSION, "18.0.0");
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
        // Supplementary plane (Unicode 18 epoch)
        assert_eq!(lookup('\u{2C09B}'), Some("U+5341"));
        // Multi-code-point substitution
        assert_eq!(lookup('Æ'), Some("U+0041 U+0045"));
    }
}
