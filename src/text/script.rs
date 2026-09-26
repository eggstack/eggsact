//! Authoritative Unicode script classification for security decisions.
//!
//! Thin compatibility layer over [`crate::text::unicode_properties`] (Unicode
//! 18.0.0 UCD: `Scripts.txt` + `ScriptExtensions.txt`).
//!
//! - [`script_of`] / [`policy_script_of`] preserve the diagnostic spellings
//!   (`"Common"`, `"Inherited"`, long script names, `"Other"` for unassigned)
//!   so existing DTO shapes are stable.
//! - Security verdicts MUST use [`resolved_script_set`] / [`is_mixed_script`]
//!   (UTS #39 §5.1 Script_Extensions + augmented resolved sets), never the
//!   legacy [`is_legitimate_mixture`] writing-system allowance.
//!
//! [`is_legitimate_mixture`] is retained for 1.x source compatibility only;
//! it encodes restriction-level-friendly Japanese/Korean allowances and does
//! NOT implement the UTS #39 mixed-script predicate (notably, Hangul+Han+Latin
//! is mixed under resolved sets). New code must call [`is_mixed_script`].

use std::collections::BTreeSet;

/// Authoritative script identity for one character (diagnostics).
///
/// Returns `"Common"`, `"Inherited"`, a long script name (e.g. `"Latin"`),
/// or `"Other"` for unassigned code points. Delegates to the pinned Unicode
/// 18 tables; combining marks report their Script_Extensions-derived identity
/// (usually `"Inherited"` via the generated data path when unlisted).
pub fn script_of(c: char) -> &'static str {
    let short = crate::text::unicode_properties::script_short_of(c);
    match short {
        "Zzzz" => "Other",
        "Zyyy" => "Common",
        "Zinh" => "Inherited",
        _ => script_long_for_short_cached(short),
    }
}

fn script_long_for_short_cached(short: &'static str) -> &'static str {
    // Linear scan over the small generated map is deterministic and cheap;
    // the map is sorted by short code.
    let table = crate::text::unicode_properties::SCRIPT_SHORT_TO_LONG;
    let mut lo = 0usize;
    let mut hi = table.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (s, l) = table[mid];
        if short < s {
            hi = mid;
        } else if short > s {
            lo = mid + 1;
        } else {
            return l;
        }
    }
    short
}

/// Script identity with the policy-layer `"Unknown"` spelling for `"Other"`.
pub fn policy_script_of(cp: u32) -> &'static str {
    match char::from_u32(cp).map(script_of) {
        Some("Other") | None => "Unknown",
        Some(s) => s,
    }
}

/// Legacy writing-system allowance (Japanese/Korean), NOT the UTS #39 verdict.
///
/// Retained for source compatibility. Security decisions must use
/// [`is_mixed_script`] instead: Hangul+Han+Latin is `true` (mixed) under
/// resolved Script_Extensions even though this legacy helper returns `true`
/// (legitimate).
pub fn is_legitimate_mixture(scripts: &BTreeSet<&str>) -> bool {
    if scripts.len() < 2 {
        return false;
    }
    let japanese = ["Han", "Hiragana", "Katakana"];
    if scripts.iter().all(|s| japanese.contains(s)) {
        return true;
    }
    if scripts.contains("Hangul")
        && scripts
            .iter()
            .all(|s| matches!(*s, "Hangul" | "Han" | "Latin"))
    {
        return true;
    }
    false
}

/// UTS #39 §5.1 resolved script set (short codes, e.g. `Latn`, `Jpan`).
///
/// Intersection of augmented Script_Extensions over `text`; Common/Inherited/
/// Unknown do not constrain. Empty with script-bearing input means mixed.
pub fn resolved_script_set(text: &str) -> BTreeSet<&'static str> {
    crate::text::unicode_properties::resolved_script_set(text)
}

/// UTS #39 §5.1 mixed-script verdict (authoritative security predicate).
pub fn is_mixed_script(text: &str) -> bool {
    crate::text::unicode_properties::is_mixed_script(text)
}

/// Filter helper shared by mixed-script detectors: keep only script-bearing
/// values (drops Common/Inherited/Unknown/Other).
pub fn is_script_bearing(script: &str) -> bool {
    !matches!(script, "Common" | "Inherited" | "Unknown" | "Other")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_authoritative_source_spot_checks() {
        assert_eq!(script_of('A'), "Latin");
        assert_eq!(script_of('é'), "Latin");
        assert_eq!(script_of('Α'), "Greek");
        assert_eq!(script_of('Ἀ'), "Greek");
        assert_eq!(script_of('А'), "Cyrillic");
        assert_eq!(script_of('а'), "Cyrillic");
        assert_eq!(script_of('あ'), "Hiragana");
        assert_eq!(script_of('ア'), "Katakana");
        assert_eq!(script_of('漢'), "Han");
        assert_eq!(script_of('한'), "Hangul");
        assert_eq!(script_of('0'), "Common");
        assert_eq!(script_of('\u{0301}'), "Inherited");
    }

    #[test]
    fn japanese_and_korean_mixtures_are_legitimate_legacy() {
        let ja: BTreeSet<&str> = ["Han", "Hiragana", "Katakana"].into_iter().collect();
        assert!(is_legitimate_mixture(&ja));
        let ja2: BTreeSet<&str> = ["Hiragana", "Katakana"].into_iter().collect();
        assert!(is_legitimate_mixture(&ja2));
        let ko: BTreeSet<&str> = ["Hangul", "Han"].into_iter().collect();
        assert!(is_legitimate_mixture(&ko));
    }

    #[test]
    fn latin_cyrillic_mixture_is_spoof() {
        let spoof: BTreeSet<&str> = ["Latin", "Cyrillic"].into_iter().collect();
        assert!(!is_legitimate_mixture(&spoof));
    }

    #[test]
    fn resolved_sets_are_authoritative() {
        assert!(!is_mixed_script("Circle"));
        assert!(is_mixed_script("hello привеt"));
        assert!(!is_mixed_script("日本語テスト漢字"));
        // Restriction-friendly but mixed under UTS #39.
        assert!(is_mixed_script("한한자test"));
    }
}
