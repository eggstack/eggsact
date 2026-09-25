//! Authoritative Unicode script classification for security decisions.
//!
//! This module is the single source of truth for script identity used by
//! `unicode_tools`, `unicode_policy`, and `identifier` analysis. The three
//! former call sites maintained independent hand-maintained range tables
//! with divergent boundaries; they now delegate here.
//!
//! The table is intentionally conservative and standards-shaped (Script-like
//! values with `Common`/`Inherited` handling), not a full
//! Script_Extensions implementation. Ranges follow the Unicode 17.0.0 code
//! charts for the scripts eggsact classifies; assignments new in Unicode 18
//! surface as `"Other"` (filtered from spoof analysis — the safe direction,
//! never a false spoof flag). Characters outside the known ranges report
//! `"Other"` (tools) — policy layers map unknown/non-spacing marks to
//! `Unknown`/`Inherited` as before, preserving their wire behavior. See the
//! provider epoch inventory in `architecture/generated-assets.md`.
//!
//! Mixed-script legitimacy: Japanese text legitimately mixes Han, Hiragana,
//! and Katakana; Korean text legitimately mixes Hangul, Han, and Latin.
//! [`is_legitimate_mixture`] encodes exactly those two allowances so
//! Latin/Cyrillic (and other) spoof mixtures still report as mixed.

use std::collections::BTreeSet;

/// Script ranges as `(start, end, script)`, sorted for readability.
const SCRIPT_RANGES: &[(u32, u32, &str)] = &[
    (0x0041, 0x005A, "Latin"), // ASCII uppercase
    (0x0061, 0x007A, "Latin"), // ASCII lowercase
    (0x00C0, 0x00FF, "Latin"), // Latin-1 supplement letters
    (0x0100, 0x017F, "Latin"), // Latin extended-A
    (0x0180, 0x024F, "Latin"), // Latin extended-B
    (0x1E00, 0x1EFF, "Latin"), // Latin extended additional
    (0x0370, 0x03FF, "Greek"),
    (0x1F00, 0x1FFF, "Greek"), // Greek extended
    (0x0400, 0x04FF, "Cyrillic"),
    (0x0500, 0x052F, "Cyrillic"), // Cyrillic supplement
    (0x0530, 0x058F, "Armenian"),
    (0x0590, 0x05FF, "Hebrew"),
    (0x0600, 0x06FF, "Arabic"),
    (0x0750, 0x077F, "Arabic"), // Arabic supplement
    (0x0900, 0x097F, "Devanagari"),
    (0x0E00, 0x0E7F, "Thai"),
    (0x10A0, 0x10FF, "Georgian"),
    (0x13A0, 0x13FF, "Cherokee"),
    (0x1400, 0x167F, "Canadian_Aboriginal"),
    (0x3000, 0x303F, "CJK"), // CJK symbols/punctuation block label (legacy)
    (0x3040, 0x309F, "Hiragana"),
    (0x30A0, 0x30FF, "Katakana"),
    (0x3400, 0x4DBF, "Han"), // CJK ext-A
    (0x4E00, 0x9FFF, "Han"), // CJK unified
    (0xAC00, 0xD7AF, "Hangul"),
];

/// Code points that are never script-bearing for spoof analysis.
fn is_common_inherited(cp: u32) -> Option<&'static str> {
    if (0x0030..=0x0039).contains(&cp) {
        return Some("Common"); // ASCII digits
    }
    if cp == 0x200C || cp == 0x200D || (0x0300..=0x036F).contains(&cp) {
        return Some("Inherited");
    }
    // Known invisible/format controls are Common for script purposes.
    if matches!(
        cp,
        0x200B
            | 0x200E
            | 0x200F
            | 0xFEFF
            | 0x00A0
            | 0x2028
            | 0x2029
            | 0x202A
            | 0x202B
            | 0x202C
            | 0x202D
            | 0x202E
            | 0x2066
            | 0x2067
            | 0x2068
            | 0x2069
            | 0x2060
    ) {
        return Some("Common");
    }
    None
}

/// Authoritative script identity for one character.
///
/// Returns `"Common"`, `"Inherited"`, a script name, or `"Other"`.
pub fn script_of(c: char) -> &'static str {
    let cp = c as u32;
    if let Some(common) = is_common_inherited(cp) {
        return common;
    }
    // Combining marks inherit the script of their base character.
    if unicode_general_category::get_general_category(c)
        .abbreviation()
        .starts_with('M')
    {
        return "Inherited";
    }
    for &(start, end, name) in SCRIPT_RANGES {
        if start <= cp && cp <= end {
            return name;
        }
    }
    "Other"
}

/// Script identity with the policy-layer `"Unknown"` spelling for `"Other"`.
pub fn policy_script_of(cp: u32) -> &'static str {
    match char::from_u32(cp).map(script_of) {
        Some("Other") | None => "Unknown",
        Some(s) => s,
    }
}

/// Whether a set of observed scripts is a legitimate writing-system mixture.
///
/// Allows exactly:
/// - Japanese: any non-empty subset of {Han, Hiragana, Katakana}
///   (with Common/Inherited already filtered by callers);
/// - Korean: any non-empty subset of {Hangul, Han, Latin} that contains
///   Hangul.
///
/// Everything else with more than one script is a genuine mixture.
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
        assert_eq!(script_of('Ἀ'), "Greek"); // Greek extended now covered
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
    fn japanese_and_korean_mixtures_are_legitimate() {
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
}
