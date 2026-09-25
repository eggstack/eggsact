use eggsact::text::unicode_tools::{find_invisibles, unicode_casefold};
use eggsact::text::{
    canonicalize_text,
    confusables::{are_confusable, confusable_skeleton},
    count_graphemes, has_confusables, unicode_policy_check,
};

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

#[test]
fn unicode_policy_deterministic() {
    let inputs = ["hello", "🌍", "\u{202e}test"];
    for input in &inputs {
        for policy in VALID_POLICIES {
            let r1 = unicode_policy_check(input, policy, None);
            let r2 = unicode_policy_check(input, policy, None);
            assert_eq!(r1, r2, "policy {policy} not deterministic for {input:?}");
        }
    }
}

#[test]
fn unicode_policy_matrix_enters_valid_logic() {
    // Every valid policy accepts clean ASCII (no invalid-policy error path).
    for policy in VALID_POLICIES {
        let r = unicode_policy_check("hello", policy, None);
        assert!(
            !r.findings.iter().any(|f| f.rule == "invalid_policy"),
            "valid policy {policy} took the invalid-policy path"
        );
    }
    // Invalid policies remain separately covered.
    for bad in ["permissive", "strict", "nonsense"] {
        let r = unicode_policy_check("hello", bad, None);
        assert!(!r.pass);
        assert!(r.findings.iter().any(|f| f.rule == "invalid_policy"));
    }
}

#[test]
fn unicode_policy_detects_bidi_and_spoof() {
    let rlo = unicode_policy_check("\u{202e}test", "identifier_strict", None);
    assert!(rlo.findings.iter().any(|f| f.rule == "bidi_controls"));
    let spoof = unicode_policy_check("аpple", "identifier_strict", None);
    assert!(spoof.findings.iter().any(|f| f.rule == "confusables"));
}

#[test]
fn canonicalize_text_idempotent() {
    let inputs = ["hello", "café", "\u{0065}\u{0301}", "naïve"];
    for input in &inputs {
        for profile in VALID_PROFILES {
            let r1 = canonicalize_text(input, profile, false);
            assert!(
                !r1.base
                    .findings
                    .iter()
                    .any(|f| f.starts_with("Invalid profile")),
                "valid profile {profile} took the invalid-profile path"
            );
            let r2 = canonicalize_text(&r1.base.text, profile, false);
            assert_eq!(
                r1.base.text, r2.base.text,
                "canonicalize not idempotent for profile {profile} input: {:?}",
                input
            );
        }
    }
}

#[test]
fn canonicalize_invalid_profile_stays_covered() {
    for bad in ["nfc", "nfkc", "NFC", "bogus"] {
        let r = canonicalize_text("hello", bad, false);
        assert!(!r.base.changed || !r.base.findings.is_empty());
        assert!(
            r.base
                .findings
                .iter()
                .any(|f| f.starts_with("Invalid profile")),
            "obsolete profile name {bad:?} must take the invalid-profile path"
        );
    }
}

#[test]
fn canonicalize_seeded_security_corpus() {
    // Bidi controls, canonical equivalents, homoglyphs, legitimate Japanese
    // mixture, one-to-many casefold, supplementary-plane confusable,
    // variation selector.
    let seeds = [
        "\u{202e}test\u{202c}",
        "caf\u{e9}",
        "cafe\u{301}",
        "apple",
        "аpple",
        "日本語テスト漢字",
        "ß",
        "\u{2FA1D}",
        "a\u{FE0F}b",
    ];
    for seed in &seeds {
        for profile in VALID_PROFILES {
            let r1 = canonicalize_text(seed, profile, true);
            let r2 = canonicalize_text(&r1.base.text, profile, false);
            assert_eq!(
                r1.base.text, r2.base.text,
                "profile {profile} seed {seed:?}"
            );
        }
    }
}

#[test]
fn confusable_skeleton_determinism_and_idempotence() {
    let inputs = ["hello", "apple", "аpple", "Æ", "ß", "", "café", "\u{2FA1D}"];
    for input in &inputs {
        assert_eq!(confusable_skeleton(input), confusable_skeleton(input));
        let once = confusable_skeleton(input);
        assert_eq!(
            confusable_skeleton(&once),
            once,
            "skeleton not idempotent for {input:?}"
        );
    }
    assert!(are_confusable("apple", "аpple"));
    assert!(!are_confusable("apple", "apple"));
    assert!(!are_confusable("apple", "orange"));
}

#[test]
fn count_graphemes_within_bounds() {
    let inputs = ["", "hello", "é", "🌍", "👨‍👩‍👧‍👦", "\u{0301}\u{0302}\u{0303}"];
    for input in &inputs {
        let gc = count_graphemes(input);
        assert!(gc <= input.len(), "Grapheme count exceeds byte length");
    }
}

#[test]
fn casefold_preserves_content() {
    let inputs = ["Hello", "CAFÉ", "straße", "Ωμέγα"];
    for input in &inputs {
        let cf = unicode_casefold(input);
        assert!(!cf.is_empty());
        assert!(
            cf.len() >= input.len() / 2,
            "Casefold drastically reduced content"
        );
    }
}

#[test]
fn find_invisibles_bounded() {
    let inputs = ["", "hello", "\u{200b}test\u{200c}", "\u{202e}rtl"];
    for input in &inputs {
        let inv = find_invisibles(input);
        assert!(inv.len() <= input.len());
    }
}

#[test]
fn has_confusables_deterministic() {
    let inputs = ["hello", "α", "a", "Ελληνικά"];
    for input in &inputs {
        let h1 = has_confusables(input);
        let h2 = has_confusables(input);
        assert_eq!(h1, h2);
    }
}
