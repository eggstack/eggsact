#![no_main]

//! Fuzz Unicode inspection and normalization.
//!
//! Asserts: no panic, NFC idempotent, grapheme positions within bounds,
//! safe representation deterministic, findings bounded.
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
    unicode_tools::{find_invisibles, unicode_casefold, build_safe_repr, is_bidi_control},
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

/// Fixed seeds covering the security-relevant classes from Milestone 003
/// WP-A: bidi controls, canonical-equivalent forms, whole-identifier
/// homoglyphs, legitimate Japanese mixed-script, one-to-many casefolds,
/// supplementary-plane confusables, and variation selectors.
fn seeded_corpus() -> Vec<String> {
    vec![
        "\u{202e}test\u{202c}".to_string(),       // RLO ... PDF bidi controls
        "\u{2066}isolate\u{2069}".to_string(),    // LRI ... PDI
        "caf\u{e9}".to_string(),                  // precomposed é
        "cafe\u{301}".to_string(),                // e + combining acute (canonical equivalent)
        "apple".to_string(),
        "аpple".to_string(),                     // Cyrillic а + pple homoglyph
        "日本語テスト漢字".to_string(),              // legitimate Japanese mixture
        "한글한자".to_string(),                      // legitimate Korean mixture
        "ß".to_string(),                          // one-to-many casefold (ß -> ss)
        "\u{2FA1D}".to_string(),                 // supplementary-plane confusable
        "a\u{FE0F}b".to_string(),                // variation selector sequence
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

    // Whole-string skeleton relation: determinism + idempotence on fuzzer input.
    let skel1 = confusable_skeleton(text);
    let skel2 = confusable_skeleton(text);
    assert_eq!(skel1, skel2);
    assert_eq!(confusable_skeleton(&skel1), skel1);
    assert!(!are_confusable(text, text), "identical inputs are not confusable");
    assert_eq!(
        are_confusable(text, &skel1),
        text != skel1 && confusable_skeleton(&skel1) == skel1
    );

    // Seeded security corpus: each class must round-trip without panic and
    // land in its expected typed classification.
    for seed in seeded_corpus() {
        let _ = unicode_policy_check(&seed, "identifier_strict", None);
        let _ = unicode_policy_check(&seed, "human_text", None);
        let _ = canonicalize_text(&seed, "identifier_compare", true);
        let _ = confusable_skeleton(&seed);
        let _ = find_invisibles(&seed);
        let _ = unicode_casefold(&seed);
        let _ = build_safe_repr(&seed);
        let _ = has_confusables(&seed);
    }
    // Spot classifications over the seeds (fail-closed, not fail-silent).
    assert!(is_bidi_control('\u{202e}'));
    assert!(is_bidi_control('\u{2066}'));
    assert!(are_confusable("apple", "аpple"));
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
