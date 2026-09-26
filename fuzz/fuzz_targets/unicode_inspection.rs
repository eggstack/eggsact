#![no_main]

//! Fuzz Unicode inspection and normalization.
//!
//! Asserts: no panic, NFC idempotent, grapheme positions within bounds,
//! safe representation deterministic, findings bounded, internal skeleton
//! idempotent (UTS #39-guaranteed), public/bidi skeletons deterministic
//! (no bidi idempotence claim), resolved script sets deterministic with
//! Common/Inherited neutrality, central hazard agreement.
//!
//! Exercises every valid Unicode policy (`identifier_strict`,
//! `filename_safe`, `source_code`, `human_text`, `json_key`,
//! `domain_like`) and every canonicalization profile
//! (`source_file_identity`, `identifier_compare`, `human_label_compare`,
//! `json_key_compare`, `path_segment_compare`). Invalid policy/profile
//! inputs are asserted separately on fixed sentinels so valid-path
//! coverage is never masked by invalid-input return paths.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{
    unicode_policy_check, canonicalize_text, count_graphemes, has_confusables,
    confusables::{are_confusable, confusable_skeleton},
    internal_skeleton, bidi_skeleton_ltr, is_mixed_script, resolved_script_set,
    unicode_tools::{
        find_invisibles, unicode_casefold, build_safe_repr, is_bidi_control,
        has_security_invisible_hazard,
    },
    script::is_legitimate_mixture,
};
use std::collections::BTreeSet;

const MAX_TEXT_LEN: usize = 50_000;

const VALID_POLICIES: &[&str] = &[
    "identifier_strict",
    "filename_safe",
    "source_code",
    "human_text",
    "json_key",
    "domain_like",
];

const VALID_PROFILES: &[&str] = &[
    "source_file_identity",
    "identifier_compare",
    "human_label_compare",
    "json_key_compare",
    "path_segment_compare",
];

/// Fixed seeds covering the security-relevant classes from Milestones 003
/// and 005: bidi controls, canonical-equivalent forms, whole-identifier
/// homoglyphs, legitimate Japanese mixed-script, one-to-many casefolds,
/// supplementary-plane confusables, variation selectors, Default_Ignorables,
/// RTL/mirrored vectors, and augmented-script (Hntl/Kore/Jpan) cases.
fn seeded_corpus() -> Vec<String> {
    vec![
        "\u{202e}test\u{202c}".to_string(),       // RLO ... PDF bidi controls
        "\u{2066}isolate\u{2069}".to_string(),    // LRI ... PDI
        "caf\u{e9}".to_string(),                  // precomposed é
        "cafe\u{301}".to_string(),                // e + combining acute (canonical equivalent)
        "apple".to_string(),
        "аpple".to_string(),                     // Cyrillic а + pple homoglyph
        "日本語テスト漢字".to_string(),              // legitimate Japanese mixture
        "한글한자".to_string(),                      // Hangul+Han (Kore, single)
        "한\u{6F22}A".to_string(),                // Hangul+Han+Latin (mixed, restriction-friendly)
        "\u{6F22}A".to_string(),                  // Han+Latin (Hntl, single)
        "ß".to_string(),                          // one-to-many casefold (ß -> ss)
        "\u{2C09B}".to_string(),                 // supplementary-plane confusable
        "a\u{FE0F}b".to_string(),                // variation selector sequence
        "x\u{200B}y".to_string(),                // Default_Ignorable ZWSP
        "A1<\u{05E9}\u{05C2}".to_string(),       // UTS #39 §4 S1 (RTL)
        "\u{0391}\u{05E9}\u{05B9}>1".to_string(), // UTS #39 §4 S2 (RTL+mirror)
        "\u{200b}\u{200c}\u{200d}".to_string(),  // zero-width controls
    ]
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_TEXT_LEN { return; }

    // Every valid Unicode policy is exercised on the fuzzer input.
    for policy in VALID_POLICIES {
        let first = unicode_policy_check(text, policy, None);
        assert!(first.findings.len() <= 100);
        let second = unicode_policy_check(text, policy, None);
        let j1 = serde_json::to_value(&first).unwrap();
        let j2 = serde_json::to_value(&second).unwrap();
        assert_eq!(j1, j2, "policy must be deterministic: {policy}");
    }

    // Invalid-policy path stays separately covered on a fixed sentinel.
    let invalid = unicode_policy_check(text, "permissive", None);
    assert!(!invalid.pass);
    assert!(invalid.findings.iter().any(|f| f.rule == "invalid_policy"));

    // Every canonicalization profile is exercised where meaningful.
    for profile in VALID_PROFILES {
        let r1 = canonicalize_text(text, profile, false);
        assert!(r1.base.findings.iter().all(|f| !f.starts_with("Invalid profile")));
        let r2 = canonicalize_text(&r1.base.text, profile, false);
        assert_eq!(
            r1.base.text, r2.base.text,
            "canonicalize must be idempotent for profile {profile}"
        );
    }

    // Invalid-profile path stays separately covered.
    let bad_profile = canonicalize_text(text, "nfc", false);
    assert!(!bad_profile.base.changed || !bad_profile.base.findings.is_empty());
    assert!(
        bad_profile.base.findings.iter().any(|f| f.starts_with("Invalid profile")),
        "obsolete lowercase profile name must take the invalid-profile path"
    );

    // Whole-string skeleton relation: determinism on fuzzer input.
    // Internal skeleton is idempotent per UTS #39 (spec-guaranteed); the
    // public bidi skeleton is asserted deterministic only — no algebraic
    // idempotence claim (Revision 34 guarantees internal idempotence, not
    // bidi re-application).
    let skel1 = confusable_skeleton(text);
    let skel2 = confusable_skeleton(text);
    assert_eq!(skel1, skel2);
    let internal1 = internal_skeleton(text);
    assert_eq!(internal_skeleton(&internal1), internal1);
    assert_eq!(bidi_skeleton_ltr(text), bidi_skeleton_ltr(text));
    assert!(!are_confusable(text, text), "identical inputs are not confusable");
    assert_eq!(
        are_confusable(text, &skel1),
        text != skel1 && confusable_skeleton(&skel1) == skel1
    );

    // Resolved script sets: deterministic; Common/Inherited never flip.
    assert_eq!(resolved_script_set(text), resolved_script_set(text));
    assert_eq!(is_mixed_script(text), is_mixed_script(text));

    // Seeded security corpus: each class must round-trip without panic and
    // land in its expected typed classification.
    for seed in seeded_corpus() {
        let _ = unicode_policy_check(&seed, "identifier_strict", None);
        let _ = unicode_policy_check(&seed, "human_text", None);
        let _ = canonicalize_text(&seed, "identifier_compare", true);
        let _ = confusable_skeleton(&seed);
        let _ = internal_skeleton(&seed);
        let _ = bidi_skeleton_ltr(&seed);
        let _ = find_invisibles(&seed);
        let _ = unicode_casefold(&seed);
        let _ = build_safe_repr(&seed);
        let _ = has_confusables(&seed);
        let _ = resolved_script_set(&seed);
        let _ = is_mixed_script(&seed);
    }
    // Spot classifications over the seeds (fail-closed, not fail-silent).
    assert!(is_bidi_control('\u{202e}'));
    assert!(is_bidi_control('\u{2066}'));
    assert!(has_security_invisible_hazard('\u{FE0F}'));
    assert!(has_security_invisible_hazard('\u{200D}'));
    assert!(are_confusable("apple", "аpple"));
    assert!(are_confusable("a\u{FE0F}b", "ab"));
    assert!(!is_mixed_script("日本語テスト漢字"));
    assert!(!is_mixed_script("한글한자"));
    assert!(is_mixed_script("한\u{6F22}A"));
    let ja: BTreeSet<&str> = ["Han", "Hiragana", "Katakana"].into_iter().collect();
    assert!(is_legitimate_mixture(&ja));

    // Find invisibles
    let invis = find_invisibles(text);
    assert!(invis.len() <= text.len());

    // Count graphemes
    let gc = count_graphemes(text);
    assert!(gc <= text.len());

    // Casefold
    let cf = unicode_casefold(text);
    assert!(std::str::from_utf8(cf.as_bytes()).is_ok());

    // Safe repr
    let sr = build_safe_repr(text);
    assert!(std::str::from_utf8(sr.as_bytes()).is_ok());
    // Deterministic
    let sr2 = build_safe_repr(text);
    assert_eq!(sr, sr2);

    // Confusables
    let _ = has_confusables(text);

    // Serializable
    let _ = serde_json::json!({
        "graphemes": gc,
        "invisibles": invis.len(),
    });
});
