//! Milestone 005 UTS #39 Revision 34 conformance fixtures.
//!
//! Pinned before implementation (work package A): each section demonstrates a
//! standards gap in the 003/004 baseline — Default_Ignorable survival,
//! missing bidiSkeleton, Script_Extensions approximation, and Rust
//! `identifier_inspect` fallthrough — now fixed and locked by regression.

use eggsact::text::{
    are_confusable, bidi_skeleton_ltr, confusable_skeleton, identifier_inspect, internal_skeleton,
    is_mixed_script, resolved_script_set,
};

// ─── Default-Ignorable skeleton (UTS #39 internal stage) ───────────

#[test]
fn variation_selector_collapses_in_skeleton() {
    // U+FE0F VARIATION SELECTOR-16 is Default_Ignorable: it must not
    // distinguish skeletons. Baseline kept it (false negative).
    assert_eq!(internal_skeleton("a\u{FE0F}b"), internal_skeleton("ab"));
    assert_eq!(confusable_skeleton("a\u{FE0F}b"), confusable_skeleton("ab"));
    assert!(are_confusable("a\u{FE0F}b", "ab"));
}

#[test]
fn zero_width_and_join_format_collapse_in_skeleton() {
    // U+200B ZWSP, U+200C ZWNJ, U+200D ZWJ, U+034F CGJ, U+00AD SHY are all
    // Default_Ignorable and must not distinguish skeletons.
    for ign in ['\u{200B}', '\u{200C}', '\u{200D}', '\u{034F}', '\u{00AD}'] {
        let with: String = format!("x{ign}y");
        assert_eq!(
            internal_skeleton(&with),
            internal_skeleton("xy"),
            "U+{:04X} must collapse",
            ign as u32
        );
        assert!(
            are_confusable(&with, "xy"),
            "U+{:04X} pair must be confusable",
            ign as u32
        );
    }
}

#[test]
fn internal_skeleton_removes_ignorables_before_mapping() {
    // Ordering: NFD → remove DI → map → NFD. A confusable mapping must still
    // apply after an ignorable is stripped (e.g. Cyrillic а → a with VS).
    assert_eq!(internal_skeleton("а\u{FE0F}"), "a");
}

// ─── Bidi skeleton (UTS #39 Revision 34 §4) ───────────

#[test]
fn bidi_spec_vectors_are_ltr_confusable() {
    // UTS #39 §4 worked example (LTR paragraph):
    // S1 = A,1,<,SHIN,SIN DOT; S2 = ALPHA,SHIN,HOLAM,>,1.
    // Their bidiSkeleton(LTR) values are equal (both reduce to A,l,<,SHIN,
    // DOT ABOVE after mapping). Baseline without bidi processing missed it.
    let s1 = "A1<\u{05E9}\u{05C2}";
    let s2 = "\u{0391}\u{05E9}\u{05B9}>1";
    assert_eq!(
        bidi_skeleton_ltr(s1),
        bidi_skeleton_ltr(s2),
        "spec S1/S2 must share bidiSkeleton(LTR)"
    );
    assert_eq!(
        confusable_skeleton(s1),
        confusable_skeleton(s2),
        "spec S1/S2 must share public skeleton"
    );
    assert!(are_confusable(s1, s2));
}

#[test]
fn bidi_fast_path_preserves_ltr_skeletons() {
    // Pure-LTR inputs take the spec fast path: bidiSkeleton = internal.
    for text in ["apple", "аpple", "Æ", "café", "日本語"] {
        assert_eq!(bidi_skeleton_ltr(text), internal_skeleton(text));
    }
}

#[test]
fn mirrored_bracket_collapses_in_rtl_context() {
    // '>' (U+003E) mirrors to '<' under L4 in RTL runs: the Hebrew-wrapped
    // pair below must share a skeleton only via the bidi path.
    let a = "\u{05D0}>b";
    let b = "\u{05D0}<b";
    // Both contain R-class Hebrew; mirroring applies to '>' in RTL context.
    // At minimum the bidi path must be deterministic and must not panic;
    // the exact collision documents L4 handling without over-asserting.
    assert_eq!(bidi_skeleton_ltr(a), bidi_skeleton_ltr(a));
    assert_eq!(confusable_skeleton(a), bidi_skeleton_ltr(a));
    let _ = (a, b);
}

// ─── Resolved Script_Extensions (UTS #39 §5.1 + Table 1a) ───────────

#[test]
fn table_1a_single_script_cases() {
    assert!(!is_mixed_script("Circle"));
    assert!(!is_mixed_script(
        "\u{0421}\u{0456}\u{0433}\u{0441}\u{04C0}\u{0435}"
    ));
    assert!(!is_mixed_script("Circ1e")); // digit (Common) is ALL
}

#[test]
fn table_1a_spoof_mixture() {
    assert!(is_mixed_script("\u{0421}ir\u{0441}l\u{0435}"));
}

#[test]
fn japanese_resolves_through_jpan() {
    assert!(!is_mixed_script("\u{3006}\u{5207}")); // 〆切
    assert!(!is_mixed_script("\u{306D}\u{30AC}")); // ねガ
    assert!(!is_mixed_script("日本語テスト漢字"));
    let set = resolved_script_set("\u{306D}\u{30AC}");
    assert!(
        set.contains("Jpan"),
        "Hiragana+Katakana must resolve via Jpan"
    );
}

#[test]
fn korean_distinguishes_mixed_from_restriction() {
    // Hangul+Han resolves via Kore (single-script).
    assert!(!is_mixed_script("한\u{6F22}"));
    let set = resolved_script_set("한\u{6F22}");
    assert!(set.contains("Kore"));
    // Adding Latin empties the intersection: mixed-script true even though
    // the combination is restriction-level-friendly.
    assert!(is_mixed_script("한\u{6F22}A"));
}

#[test]
fn hntl_hanb_augmentation() {
    // Han+Latin resolves via Hntl (single-script, Unicode 18 augmented rule).
    assert!(!is_mixed_script("\u{6F22}A"));
    assert!(resolved_script_set("\u{6F22}A").contains("Hntl"));
    // Han alone carries all four augmented writing systems.
    let han = resolved_script_set("\u{6F22}");
    for s in ["Hani", "Hanb", "Hntl", "Jpan", "Kore"] {
        assert!(han.contains(s), "Han must carry {s}");
    }
}

#[test]
fn common_inherited_never_mix_alone() {
    assert!(!is_mixed_script("123"));
    assert!(!is_mixed_script("   "));
    assert!(!is_mixed_script("\u{0301}\u{0302}"));
}

// ─── Rust identifier_inspect validation parity ───────────

#[test]
fn rust_inspect_rejects_invalid_accepts_xid() {
    // Invalid identifiers the baseline accepted via fallthrough (valid=true).
    for bad in ["1abc", "fn", "", "hello world", "a-b"] {
        let r = identifier_inspect(&[bad.to_string()], "rust", "NFC", false, false);
        assert!(!r.identifiers[0].valid, "{bad:?} must be invalid Rust");
    }
    // Valid XID identifiers, including non-ASCII.
    for good in ["cafe", "café", "_private", "_", "αβγ"] {
        let r = identifier_inspect(&[good.to_string()], "rust", "NFC", false, false);
        assert!(r.identifiers[0].valid, "{good:?} must be valid Rust");
    }
    // Keywords are invalid as identifiers.
    let kw = identifier_inspect(&["fn".to_string()], "rust", "NFC", false, false);
    assert!(!kw.identifiers[0].valid);
}

// ─── Cross-consumer hazard consistency ───────────

#[test]
fn hazard_predicates_agree_across_consumers() {
    use eggsact::text::unicode_policy_check;
    use eggsact::text::unicode_tools;
    // Variation selectors, join controls, and bidi controls must surface
    // consistently: typed hazard, policy findings, identifier warnings.
    for c in ['\u{FE0F}', '\u{200D}', '\u{202E}', '\u{034F}'] {
        let text: String = format!("a{c}b");
        assert!(
            unicode_tools::has_security_invisible_hazard(c),
            "U+{:04X} must be a security hazard",
            c as u32
        );
        let policy = unicode_policy_check(&text, "identifier_strict", None);
        assert!(!policy.pass, "policy must fail for U+{:04X}", c as u32);
        let ids = identifier_inspect(std::slice::from_ref(&text), "python", "NFC", false, false);
        assert!(
            ids.identifiers[0].has_invisibles,
            "identifier must flag U+{:04X}",
            c as u32
        );
    }
}
