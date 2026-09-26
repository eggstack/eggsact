use eggsact::text::unicode_tools::{
    find_invisibles, has_security_invisible_hazard, unicode_casefold,
};
use eggsact::text::{
    bidi_skeleton_ltr, canonicalize_text,
    confusables::{are_confusable, confusable_skeleton},
    count_graphemes, has_confusables, internal_skeleton, is_mixed_script, resolved_script_set,
    unicode_policy_check,
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
        "\u{2C09B}",
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
    let inputs = ["hello", "apple", "аpple", "Æ", "ß", "", "café", "\u{2C09B}"];
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

#[test]
fn internal_and_bidi_skeleton_deterministic_no_panic() {
    // Determinism for both stages; idempotence only for the internal stage
    // (spec-guaranteed). Public bidi determinism is asserted without an
    // algebraic idempotence claim per the milestone plan.
    let inputs = [
        "",
        "hello",
        "apple",
        "аpple",
        "Æ",
        "café",
        "a\u{FE0F}b",
        "x\u{200B}y",
        "A1<\u{05E9}\u{05C2}",
        "\u{0391}\u{05E9}\u{05B9}>1",
        "\u{05D0}>b",
        "日本語テスト漢字",
        "한\u{6F22}A",
        "\u{0301}\u{0302}\u{0303}",
        "\u{2C09B}",
        "\u{1F600}",
    ];
    for input in &inputs {
        assert_eq!(internal_skeleton(input), internal_skeleton(input));
        let once = internal_skeleton(input);
        assert_eq!(
            internal_skeleton(&once),
            once,
            "internal skeleton must be idempotent for {input:?}"
        );
        assert_eq!(bidi_skeleton_ltr(input), bidi_skeleton_ltr(input));
        assert_eq!(confusable_skeleton(input), confusable_skeleton(input));
    }
}

#[test]
fn resolved_script_set_deterministic_common_inherited_neutral() {
    let inputs = [
        "",
        "Circle",
        "hello привеt",
        "日本語テスト漢字",
        "한\u{6F22}",
        "한\u{6F22}A",
        "123",
        "   ",
        "\u{0301}",
        "a\u{0301}",
        "Circ1e",
    ];
    for input in &inputs {
        assert_eq!(resolved_script_set(input), resolved_script_set(input));
        assert_eq!(is_mixed_script(input), is_mixed_script(input));
    }
    // Common/Inherited alone never mix; adding them never flips a verdict.
    for base in ["Circle", "hello привеt", "日本語"] {
        let base_mixed = is_mixed_script(base);
        for affix in ["123", " ", "\u{0301}"] {
            let extended = format!("{base}{affix}");
            assert_eq!(
                is_mixed_script(&extended),
                base_mixed,
                "Common/Inherited affix must not flip {base:?}"
            );
        }
    }
    assert!(!is_mixed_script("123"));
    assert!(!is_mixed_script("\u{0301}"));
}

#[test]
fn central_hazard_predicates_agree() {
    use eggsact::text::unicode_tools::is_invisible_char;
    for c in [
        'a', '\u{200B}', '\u{200C}', '\u{200D}', '\u{202E}', '\u{FE0F}', '\u{034F}', '\u{0301}',
        '\u{0001}', '\n',
    ] {
        assert_eq!(
            is_invisible_char(c),
            has_security_invisible_hazard(c),
            "invisible predicates must agree for U+{:04X}",
            c as u32
        );
    }
}
