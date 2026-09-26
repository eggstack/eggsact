//! Milestone 006 Unicode-conformance edge-case fixtures (UTS #39 Rev 34 /
//! UAX #9 / UAX #24 / UAX #44, Unicode 18.0.0).
//!
//! Pinned before production edits (work package A): each section demonstrates
//! a residual Milestone 005 defect — discarded `DerivedBidiClass.txt`
//! `@missing` defaults, whole-text/per-paragraph level-vector mismatch with
//! silent all-LTR fallback, and `Script_Extensions={Unknown}`/Zzzz treated as
//! the ALL identity — now fixed and locked by regression.

use eggsact::text::unicode_properties::{
    augmented_script_set, bidi_class_name, is_bidi_r_or_al, BIDI_CLASS_RANGES,
};
use eggsact::text::{bidi_skeleton_ltr, internal_skeleton, is_mixed_script, resolved_script_set};

// ─── Bidi_Class `@missing` defaults (UAX #44 ordered overrides) ───

/// Assert `cp` is covered only by a `@missing` default, never by an explicit
/// `DerivedBidiClass.txt` row, so the test exercises default semantics.
fn assert_no_explicit_bidi_row(cp: char) {
    let cp = cp as u32;
    for (lo, hi, _) in BIDI_CLASS_RANGES {
        assert!(
            cp < *lo || cp > *hi,
            "U+{cp:04X} must be default-only (found explicit range U+{lo:04X}..U+{hi:04X})"
        );
    }
}

#[test]
fn bidi_default_only_r_classifies_r() {
    // U+0590 is unassigned in the Hebrew block: no explicit row, but the
    // ordered `@missing: 0590..05FF; Right_To_Left` override applies.
    assert_no_explicit_bidi_row('\u{0590}');
    assert_eq!(bidi_class_name('\u{0590}'), "R");
    assert!(is_bidi_r_or_al('\u{0590}'));
}

#[test]
fn bidi_default_only_al_classifies_al() {
    // U+070E is unassigned in the Syriac range: no explicit row, but the
    // ordered `@missing: 0600..07BF; Arabic_Letter` override applies.
    assert_no_explicit_bidi_row('\u{070E}');
    assert_eq!(bidi_class_name('\u{070E}'), "AL");
    assert!(is_bidi_r_or_al('\u{070E}'));
}

#[test]
fn bidi_default_only_et_classifies_et() {
    // U+20C5 is unassigned in the Currency Symbols block: no explicit row,
    // but `@missing: 20A0..20CF; European_Terminator` applies.
    assert_no_explicit_bidi_row('\u{20C5}');
    assert_eq!(bidi_class_name('\u{20C5}'), "ET");
    assert!(!is_bidi_r_or_al('\u{20C5}'));
}

#[test]
fn bidi_global_l_default_still_applies() {
    // U+0378 is unassigned Greek: no explicit row and no narrow override, so
    // the global `@missing: 0000..10FFFF; Left_To_Right` default applies.
    assert_no_explicit_bidi_row('\u{0378}');
    assert_eq!(bidi_class_name('\u{0378}'), "L");
    assert!(!is_bidi_r_or_al('\u{0378}'));
}

// ─── Multi-paragraph bidi skeleton (UAX #9 paragraph-local L1/L2) ───

/// Reference: each paragraph's skeleton computed independently, joined by the
/// verbatim separator. UAX #9 processes paragraphs independently, so the
/// whole-text skeleton must agree with this construction.
///
/// The separator itself passes through the internal stage: LF/CRLF survive
/// verbatim, while U+2029 maps to U+0020 via confusables (hence the join uses
/// the separator's own internal skeleton).
fn per_paragraph_reference(text: &str, sep: &str) -> String {
    let sep_skeleton = internal_skeleton(sep);
    text.split(sep)
        .map(bidi_skeleton_ltr)
        .collect::<Vec<_>>()
        .join(&sep_skeleton)
}

#[test]
fn multiparagraph_later_rtl_paragraph_reorders_lf() {
    // First paragraph LTR, second Hebrew (R) requiring L2 visual reordering.
    // Baseline fell back to all-LTR levels for the second paragraph because
    // `reordered_levels_per_char` returns a whole-text vector.
    let text = "ABC\n\u{05D0}\u{05D1}\u{05D2}";
    assert_eq!(bidi_skeleton_ltr(text), per_paragraph_reference(text, "\n"));
    // The Hebrew run must actually reorder (visual גבא), not stay logical.
    assert!(bidi_skeleton_ltr(text).contains("\u{05D2}\u{05D1}\u{05D0}"));
}

#[test]
fn multiparagraph_later_rtl_paragraph_mirrors_lf() {
    // '>' (U+003E) in an RTL run takes an odd resolved level, so L4 must
    // mirror it to '<'. The all-LTR fallback kept '>' unmirrored.
    let text = "ABC\n\u{05D0}>\u{05D1}";
    assert_eq!(bidi_skeleton_ltr(text), per_paragraph_reference(text, "\n"));
    assert!(
        bidi_skeleton_ltr(text).contains('<'),
        "RTL '>' must mirror to '<', got {:?}",
        bidi_skeleton_ltr(text)
    );
}

#[test]
fn multiparagraph_rtl_reordering_crlf() {
    let text = "ABC\r\n\u{05D0}\u{05D1}\u{05D2}";
    assert_eq!(
        bidi_skeleton_ltr(text),
        per_paragraph_reference(text, "\r\n")
    );
    assert!(bidi_skeleton_ltr(text).contains("\u{05D2}\u{05D1}\u{05D0}"));
}

#[test]
fn multiparagraph_rtl_reordering_paragraph_separator() {
    // U+2029 PARAGRAPH SEPARATOR is a UAX #9 paragraph boundary.
    let text = "ABC\u{2029}\u{05D0}\u{05D1}\u{05D2}";
    assert_eq!(
        bidi_skeleton_ltr(text),
        per_paragraph_reference(text, "\u{2029}")
    );
    assert!(bidi_skeleton_ltr(text).contains("\u{05D2}\u{05D1}\u{05D0}"));
}

#[test]
fn multiparagraph_three_paragraphs_stay_independent() {
    // Guards against a "second paragraph only" fix: RTL content in the
    // middle paragraph and mirroring in the last must both resolve locally.
    let text = "ABC\n\u{05D0}\u{05D1}\nXYZ\u{05D0}>\u{05D1}";
    assert_eq!(bidi_skeleton_ltr(text), per_paragraph_reference(text, "\n"));
    assert_eq!(bidi_skeleton_ltr(text), bidi_skeleton_ltr(text));
}

#[test]
fn multiparagraph_skeleton_deterministic() {
    for text in [
        "ABC\n\u{05D0}\u{05D1}\u{05D2}",
        "ABC\r\n\u{05D0}>\u{05D1}",
        "ABC\u{2029}\u{05D0}\u{05D1}\u{05D2}",
        "A\n\u{05D0}\u{05D1}\nB",
    ] {
        assert_eq!(bidi_skeleton_ltr(text), bidi_skeleton_ltr(text));
    }
}

// ─── Unknown/Zzzz resolved-script semantics (UTS #39 §5.1) ───

#[test]
fn unknown_private_use_resolves_to_zzzz() {
    // U+E000 (private use) has Script_Extensions={Unknown}: an ordinary
    // non-empty set, not the ALL identity reserved for Zyyy/Zinh.
    let aug = augmented_script_set('\u{E000}');
    assert_eq!(aug.iter().copied().collect::<Vec<_>>(), vec!["Zzzz"]);
    assert_eq!(
        resolved_script_set("\u{E000}"),
        ["Zzzz"].into_iter().collect()
    );
    assert!(!is_mixed_script("\u{E000}"));
}

#[test]
fn common_plus_unknown_resolves_to_zzzz() {
    // Common (Zyyy = ALL) intersects to identity, leaving {Zzzz}.
    assert_eq!(
        resolved_script_set("0\u{E000}"),
        ["Zzzz"].into_iter().collect()
    );
    assert!(!is_mixed_script("0\u{E000}"));
}

#[test]
fn latin_plus_unknown_is_mixed() {
    // {Latn,Hntl} ∩ {Zzzz} is empty with script-bearing characters: mixed.
    assert!(is_mixed_script("A\u{E000}"));
}

#[test]
fn unknown_plus_unknown_is_not_mixed() {
    assert!(!is_mixed_script("\u{E000}\u{E001}"));
}
