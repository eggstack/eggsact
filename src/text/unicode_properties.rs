//! Unicode 18.0.0 security-relevant property tables (UTS #39 / UAX #24 / UAX #9).
//!
//! Typed, deterministic source of truth for:
//! - `Default_Ignorable_Code_Point` (internal skeleton removal);
//! - `Bidi_Class`, `Bidi_Mirroring_Glyph`, `Bidi_Paired_Bracket` (bidiSkeleton);
//! - `Script` / `Script_Extensions` with UTS #39 augmented resolved sets.
//!
//! Data: `unicode_properties_generated.rs` (checksum-pinned UCD 18.0.0,
//! never hand-edited). Algorithm: `unicode-bidi` UAX #9 machinery consumed
//! through a custom [`Unicode18BidiData`] source so the version-correct
//! generated tables — not the crate's bundled Unicode 16 data — drive
//! security semantics. See `architecture/generated-assets.md`.

use std::collections::BTreeSet;

mod generated {
    include!("unicode_properties_generated.rs");
}

pub use generated::{
    BIDI_BRACKET_ENTRIES, BIDI_CLASS_DEFAULT_RANGES, BIDI_CLASS_RANGES, BIDI_MIRRORING_ENTRIES,
    DEFAULT_IGNORABLE_RANGES, SCRIPT_EXTENSION_OVERRIDES, SCRIPT_RANGES, SCRIPT_SHORT_TO_LONG,
};

/// Pinned Unicode version for all tables in this module.
pub const UNICODE_SECURITY_PROPERTIES_VERSION: &str = "18.0.0";

/// Pinned source URLs (authoritative UCD 18.0.0).
pub const UCD_BASE_URL: &str = "https://www.unicode.org/Public/18.0.0/ucd/";
pub const SRC_DERIVED_CORE_PROPERTIES: &str =
    "https://www.unicode.org/Public/18.0.0/ucd/DerivedCoreProperties.txt";
pub const SRC_SCRIPTS: &str = "https://www.unicode.org/Public/18.0.0/ucd/Scripts.txt";
pub const SRC_SCRIPT_EXTENSIONS: &str =
    "https://www.unicode.org/Public/18.0.0/ucd/ScriptExtensions.txt";
pub const SRC_DERIVED_BIDI_CLASS: &str =
    "https://www.unicode.org/Public/18.0.0/ucd/extracted/DerivedBidiClass.txt";
pub const SRC_BIDI_MIRRORING: &str = "https://www.unicode.org/Public/18.0.0/ucd/BidiMirroring.txt";
pub const SRC_BIDI_BRACKETS: &str = "https://www.unicode.org/Public/18.0.0/ucd/BidiBrackets.txt";
pub const SRC_PROPERTY_VALUE_ALIASES: &str =
    "https://www.unicode.org/Public/18.0.0/ucd/PropertyValueAliases.txt";

/// Pinned SHA-256 checksums for every generated input.
pub const SHA_DERIVED_CORE_PROPERTIES: &str =
    "09c928886a178fcafd93c29e4bd59073a058e5a100b716d425cb563ab50f68c9";
pub const SHA_SCRIPTS: &str = "0071fd81b6aeae25f6e8bce8efec3066a6476a91b49bdb2f52dc76e817862a6a";
pub const SHA_SCRIPT_EXTENSIONS: &str =
    "5c9d34a922f687726f2a8bcf57d49f905987e51f1b21b58c95a00fbe255cec23";
pub const SHA_DERIVED_BIDI_CLASS: &str =
    "d9e23222522551348ea1ccfbb4f62efbf98982afb95840f8959c08ed992c5607";
pub const SHA_BIDI_MIRRORING: &str =
    "cd54810ebf52f0e61a730c8b9cb25975de6c85f6d788a559b416afd548923fd6";
pub const SHA_BIDI_BRACKETS: &str =
    "4b3b62e4a14b84ee752808c810c602534921c09a4a1bf78cfbee566d66c125b3";
pub const SHA_PROPERTY_VALUE_ALIASES: &str =
    "06c4c8eaf7b0bf34abe73b113da1215bd784ac254d4c223600b90267caa4bbbd";

/// Entry/range counts at the pinned epoch (guards against silent drift).
pub const COUNT_DEFAULT_IGNORABLE_RANGES: usize = 27;
pub const COUNT_SCRIPT_RANGES: usize = 2321;
pub const COUNT_SCRIPT_EXTENSION_OVERRIDES: usize = 210;
pub const COUNT_BIDI_CLASS_RANGES: usize = 2356;
/// Ordered UAX #44 `@missing` Bidi_Class defaults (later overrides earlier).
pub const COUNT_BIDI_CLASS_DEFAULT_RANGES: usize = 24;
pub const COUNT_BIDI_MIRRORING_ENTRIES: usize = 438;
pub const COUNT_BIDI_BRACKET_ENTRIES: usize = 130;

fn binary_search_ranges(ranges: &[(u32, u32)], cp: u32) -> bool {
    let mut lo = 0usize;
    let mut hi = ranges.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (s, e) = ranges[mid];
        if cp < s {
            hi = mid;
        } else if cp > e {
            lo = mid + 1;
        } else {
            return true;
        }
    }
    false
}

fn binary_search_tagged<'a>(ranges: &[(u32, u32, &'a str)], cp: u32) -> Option<&'a str> {
    let mut lo = 0usize;
    let mut hi = ranges.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (s, e, tag) = ranges[mid];
        if cp < s {
            hi = mid;
        } else if cp > e {
            lo = mid + 1;
        } else {
            return Some(tag);
        }
    }
    None
}

fn binary_search_u32_pair(entries: &[(u32, u32)], cp: u32) -> Option<u32> {
    let mut lo = 0usize;
    let mut hi = entries.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (s, m) = entries[mid];
        if cp < s {
            hi = mid;
        } else if cp > s {
            lo = mid + 1;
        } else {
            return Some(m);
        }
    }
    None
}

/// Whether `c` has `Default_Ignorable_Code_Point=Yes` (Unicode 18.0.0).
pub fn is_default_ignorable(c: char) -> bool {
    binary_search_ranges(DEFAULT_IGNORABLE_RANGES, c as u32)
}

/// Bidi_Class short name for `c` (Unicode 18.0.0).
///
/// Lookup order is explicit `BIDI_CLASS_RANGES` rows first, then the ordered
/// UAX #44 `@missing` defaults with the last matching directive winning. The
/// global `0000..10FFFF; L` default covers every valid `char`, so falling off
/// the defaults is an internal data invariant violation, never a silent `L`.
pub fn bidi_class_name(c: char) -> &'static str {
    let cp = c as u32;
    if let Some(tag) = binary_search_tagged(BIDI_CLASS_RANGES, cp) {
        return tag;
    }
    for (lo, hi, tag) in BIDI_CLASS_DEFAULT_RANGES.iter().rev() {
        if cp >= *lo && cp <= *hi {
            return tag;
        }
    }
    unreachable!("invariant: global Bidi_Class @missing default must cover U+{cp:04X}");
}

/// Map a Bidi_Class name to the `unicode-bidi` enum.
pub fn bidi_class_of(c: char) -> unicode_bidi::BidiClass {
    use unicode_bidi::BidiClass as B;
    match bidi_class_name(c) {
        "L" => B::L,
        "R" => B::R,
        "AL" => B::AL,
        "EN" => B::EN,
        "ES" => B::ES,
        "ET" => B::ET,
        "AN" => B::AN,
        "CS" => B::CS,
        "NSM" => B::NSM,
        "BN" => B::BN,
        "FSI" => B::FSI,
        "LRI" => B::LRI,
        "RLI" => B::RLI,
        "PDI" => B::PDI,
        "LRO" => B::LRO,
        "RLO" => B::RLO,
        "PDF" => B::PDF,
        "LRE" => B::LRE,
        "RLE" => B::RLE,
        "WS" => B::WS,
        "ON" => B::ON,
        "B" => B::B,
        "S" => B::S,
        _ => B::L,
    }
}

/// Bidi_Mirroring_Glyph target for `c`, if any (Unicode 18.0.0).
pub fn bidi_mirror(c: char) -> Option<char> {
    binary_search_u32_pair(BIDI_MIRRORING_ENTRIES, c as u32).and_then(char::from_u32)
}

/// Bidi_Paired_Bracket entry for `c`: `(pair, is_open)`, if any.
pub fn bidi_bracket(c: char) -> Option<(char, bool)> {
    let cp = c as u32;
    let mut lo = 0usize;
    let mut hi = BIDI_BRACKET_ENTRIES.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (code, pair, is_open) = BIDI_BRACKET_ENTRIES[mid];
        if cp < code {
            hi = mid;
        } else if cp > code {
            lo = mid + 1;
        } else {
            return char::from_u32(pair).map(|p| (p, is_open));
        }
    }
    None
}

/// Short Script code for `c` (e.g. `Latn`); `Zzzz` when unlisted.
pub fn script_short_of(c: char) -> &'static str {
    binary_search_tagged(SCRIPT_RANGES, c as u32).unwrap_or("Zzzz")
}

/// Long Script name for diagnostics (e.g. `Latin`); falls back to short code.
pub fn script_long_of(c: char) -> &'static str {
    let short = script_short_of(c);
    script_long_for_short(short).unwrap_or(short)
}

fn script_long_for_short(short: &str) -> Option<&'static str> {
    // SCRIPT_SHORT_TO_LONG is sorted by short code; binary search.
    let mut lo = 0usize;
    let mut hi = SCRIPT_SHORT_TO_LONG.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (s, l) = SCRIPT_SHORT_TO_LONG[mid];
        if short < s {
            hi = mid;
        } else if short > s {
            lo = mid + 1;
        } else {
            return Some(l);
        }
    }
    None
}

/// Script_Extensions set for `c` as short codes.
///
/// Falls back to the single `Script` value when no explicit override exists;
/// unlisted code points yield `{Zzzz}` (Unknown).
pub fn script_extensions(c: char) -> Vec<&'static str> {
    let cp = c as u32;
    if let Some(list) = binary_search_tagged(SCRIPT_EXTENSION_OVERRIDES, cp) {
        list.split(' ').filter(|s| !s.is_empty()).collect()
    } else {
        vec![script_short_of(c)]
    }
}

/// UTS #39 §5.1 augmented script set for one character.
///
/// Rules (Unicode 18, exact):
/// - Hani → + Hanb, Hntl, Jpan, Kore
/// - Hira → + Jpan; Kana → + Jpan; Hang → + Kore; Bopo → + Hanb; Latn → + Hntl
/// - Zyyy/Zinh treated as ALL by callers (returned here as empty marker).
/// - Zzzz (Unknown) is an ordinary singleton set and constrains the resolved
///   intersection like any other non-ALL set.
pub fn augmented_script_set(c: char) -> BTreeSet<&'static str> {
    let ext = script_extensions(c);
    if ext.iter().any(|s| *s == "Zyyy" || *s == "Zinh") {
        // Common/Inherited handled as ALL by resolved-set intersection
        // (caller skips); return empty to signal ALL.
        return BTreeSet::new();
    }
    let mut set: BTreeSet<&'static str> = ext.into_iter().collect();
    if set.contains("Hani") {
        set.insert("Hanb");
        set.insert("Hntl");
        set.insert("Jpan");
        set.insert("Kore");
    }
    if set.contains("Hira") {
        set.insert("Jpan");
    }
    if set.contains("Kana") {
        set.insert("Jpan");
    }
    if set.contains("Hang") {
        set.insert("Kore");
    }
    if set.contains("Bopo") {
        set.insert("Hanb");
    }
    if set.contains("Latn") {
        set.insert("Hntl");
    }
    set
}

/// UTS #39 §5.1 resolved script set: intersection of augmented sets.
///
/// Characters with Common/Inherited (ALL) do not constrain the
/// intersection. A string of only such characters resolves to ALL
/// (represented here as empty set + `all=true`? No — return empty meaning
/// unconstrained? Instead return the full ALL marker as empty set and let
/// `is_mixed_script` treat empty-augmentation as non-mixed).
/// For API clarity: returns the intersected set; empty means either mixed
/// (no common script) or unconstrained (all ALL). Use [`is_mixed_script`]
/// for the verdict and [`resolved_script_set_detailed`] for the distinction.
pub fn resolved_script_set(text: &str) -> BTreeSet<&'static str> {
    let (set, _) = resolved_script_set_detailed(text);
    set
}

/// Detailed resolved set plus whether any script-bearing character was seen.
pub fn resolved_script_set_detailed(text: &str) -> (BTreeSet<&'static str>, bool) {
    let mut acc: Option<BTreeSet<&'static str>> = None;
    let mut any_bearing = false;
    for c in text.chars() {
        let aug = augmented_script_set(c);
        if aug.is_empty() {
            // ALL (Common/Inherited): intersects to identity.
            continue;
        }
        any_bearing = true;
        acc = Some(match acc {
            None => aug,
            Some(prev) => prev.intersection(&aug).copied().collect(),
        });
        if acc.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
            break;
        }
    }
    match acc {
        None => (BTreeSet::new(), any_bearing),
        Some(s) => (s, any_bearing),
    }
}

/// UTS #39 §5.1 mixed-script verdict: true iff resolved set is empty
/// with at least one script-bearing character.
pub fn is_mixed_script(text: &str) -> bool {
    let (set, any_bearing) = resolved_script_set_detailed(text);
    any_bearing && set.is_empty()
}

/// Whether `c` has Bidi_Class R or AL (fast-path gate for bidiSkeleton).
pub fn is_bidi_r_or_al(c: char) -> bool {
    matches!(bidi_class_name(c), "R" | "AL")
}

/// Custom UAX #9 data source backed by the pinned Unicode 18 tables.
///
/// This is the only `BidiDataSource` used for security semantics; the
/// `unicode-bidi` crate's bundled (Unicode 16) hardcoded data is never
/// consulted because we build with `default-features = false` (no
/// `hardcoded-data`) and always call `*_with_data_source`.
pub struct Unicode18BidiData;

impl unicode_bidi::BidiDataSource for Unicode18BidiData {
    fn bidi_class(&self, c: char) -> unicode_bidi::BidiClass {
        bidi_class_of(c)
    }

    fn bidi_matched_opening_bracket(
        &self,
        c: char,
    ) -> Option<unicode_bidi::data_source::BidiMatchedOpeningBracket> {
        let (pair, is_open) = bidi_bracket(c)?;
        // Normalized opening: for an opening bracket the pair's counterpart
        // is the closing; the normalized opening is `c` itself when open,
        // else `pair`.
        let opening = if is_open { c } else { pair };
        Some(unicode_bidi::data_source::BidiMatchedOpeningBracket { opening, is_open })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_counts_match_generated_tables() {
        assert_eq!(
            DEFAULT_IGNORABLE_RANGES.len(),
            COUNT_DEFAULT_IGNORABLE_RANGES
        );
        assert_eq!(SCRIPT_RANGES.len(), COUNT_SCRIPT_RANGES);
        assert_eq!(
            SCRIPT_EXTENSION_OVERRIDES.len(),
            COUNT_SCRIPT_EXTENSION_OVERRIDES
        );
        assert_eq!(BIDI_CLASS_RANGES.len(), COUNT_BIDI_CLASS_RANGES);
        assert_eq!(
            BIDI_CLASS_DEFAULT_RANGES.len(),
            COUNT_BIDI_CLASS_DEFAULT_RANGES
        );
        assert_eq!(BIDI_MIRRORING_ENTRIES.len(), COUNT_BIDI_MIRRORING_ENTRIES);
        assert_eq!(BIDI_BRACKET_ENTRIES.len(), COUNT_BIDI_BRACKET_ENTRIES);
        assert_eq!(UNICODE_SECURITY_PROPERTIES_VERSION, "18.0.0");
    }

    #[test]
    fn generated_header_matches_provenance() {
        let src = include_str!("unicode_properties_generated.rs");
        assert!(src.starts_with("// Auto-generated Unicode 18.0.0"));
        for sha in [
            SHA_DERIVED_CORE_PROPERTIES,
            SHA_SCRIPTS,
            SHA_SCRIPT_EXTENSIONS,
            SHA_DERIVED_BIDI_CLASS,
            SHA_BIDI_MIRRORING,
            SHA_BIDI_BRACKETS,
            SHA_PROPERTY_VALUE_ALIASES,
        ] {
            assert!(src.contains(sha), "missing checksum {sha}");
        }
        assert!(
            src.contains(&format!(
                "bidi_class_defaults={COUNT_BIDI_CLASS_DEFAULT_RANGES} ranges"
            )),
            "generated header must expose the @missing default count"
        );
    }

    #[test]
    fn default_ignorable_boundaries() {
        // Members.
        for c in [
            '\u{00AD}',
            '\u{034F}',
            '\u{200B}',
            '\u{200C}',
            '\u{200D}',
            '\u{200E}',
            '\u{200F}',
            '\u{202A}',
            '\u{FE00}',
            '\u{FE0F}',
            '\u{FEFF}',
            '\u{E0100}',
        ] {
            assert!(is_default_ignorable(c), "U+{:04X} must be DI", c as u32);
        }
        // Non-members.
        for c in ['A', 'a', '0', ' ', '\u{0301}', '\u{00A0}', '\u{2028}'] {
            assert!(
                !is_default_ignorable(c),
                "U+{:04X} must not be DI",
                c as u32
            );
        }
    }

    #[test]
    fn bidi_class_spot_checks() {
        assert_eq!(bidi_class_name('A'), "L");
        assert_eq!(bidi_class_name('0'), "EN");
        assert_eq!(bidi_class_name('<'), "ON");
        assert_eq!(bidi_class_name('\u{05D0}'), "R"); // Hebrew Alef
        assert_eq!(bidi_class_name('\u{0627}'), "AL"); // Arabic Alef
        assert_eq!(bidi_class_name('\u{202E}'), "RLO");
        assert_eq!(bidi_class_name('\u{2066}'), "LRI");
    }

    #[test]
    fn mirroring_spot_checks() {
        assert_eq!(bidi_mirror('('), Some(')'));
        assert_eq!(bidi_mirror(')'), Some('('));
        assert_eq!(bidi_mirror('<'), Some('>'));
        assert_eq!(bidi_mirror('>'), Some('<'));
        assert_eq!(bidi_mirror('A'), None);
    }

    #[test]
    fn script_extensions_spot_checks() {
        assert_eq!(script_short_of('A'), "Latn");
        assert_eq!(script_short_of('\u{0410}'), "Cyrl");
        assert_eq!(script_short_of('\u{4E00}'), "Hani");
        assert_eq!(script_short_of('\u{3041}'), "Hira");
        assert_eq!(script_short_of('\u{30A1}'), "Kana");
        assert_eq!(script_short_of('\u{AC00}'), "Hang");
        assert_eq!(script_short_of('0'), "Zyyy");
        assert_eq!(script_short_of('\u{0301}'), "Zinh");
        // MIDDLE DOT has an explicit multi-script extension list.
        let ext = script_extensions('\u{00B7}');
        assert!(ext.contains(&"Latn") && ext.contains(&"Grek") && ext.contains(&"Hani"));
    }

    #[test]
    fn augmented_sets_follow_uts39() {
        let han = augmented_script_set('\u{4E00}');
        for s in ["Hani", "Hanb", "Hntl", "Jpan", "Kore"] {
            assert!(han.contains(s), "Han must contain {s}");
        }
        let hira = augmented_script_set('\u{3041}');
        assert!(hira.contains("Jpan"));
        let latn = augmented_script_set('A');
        assert!(latn.contains("Latn") && latn.contains("Hntl"));
        assert!(augmented_script_set('0').is_empty()); // Common = ALL
        assert!(augmented_script_set('\u{FE00}').is_empty()); // Inherited (no override) = ALL
                                                              // Combining marks with explicit Script_Extensions constrain normally.
        assert!(!augmented_script_set('\u{0301}').is_empty());
    }

    #[test]
    fn resolved_sets_match_table_1a() {
        // Circle (Latin single).
        assert!(!is_mixed_script("Circle"));
        // Cyrillic single.
        assert!(!is_mixed_script(
            "\u{0421}\u{0456}\u{0433}\u{0441}\u{04C0}\u{0435}"
        ));
        // Latin/Cyrillic spoof mixture.
        assert!(is_mixed_script("\u{0421}ir\u{0441}l\u{0435}"));
        // Digit (Common) does not create mixture.
        assert!(!is_mixed_script("Circ1e"));
        // Japanese via Jpan.
        assert!(!is_mixed_script("\u{3006}\u{5207}"));
        assert!(!is_mixed_script("\u{306D}\u{30AC}"));
        // Han+Latin resolves via Hntl (single).
        assert!(!is_mixed_script("\u{6F22}A"));
        // Hangul+Han resolves via Kore (single).
        assert!(!is_mixed_script("한\u{6F22}"));
        // Hangul+Han+Latin is mixed (restriction-friendly, not single-script).
        assert!(is_mixed_script("한\u{6F22}A"));
    }
}
