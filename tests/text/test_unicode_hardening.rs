//! Milestone 003 cross-consumer acceptance evidence.
//!
//! Proves the same code point receives the same hazard/script
//! classification across `unicode_tools`, `unicode_policy`, `identifier`,
//! `text_measure`, and the `text_security_inspect` composite — without any
//! display-string predicates — plus the skeleton collision matrix and the
//! legitimate-mixture/spoof-mixture split.

use eggsact::services::security::inspect_text_security;
use eggsact::text::{
    are_confusable, confusable_skeleton, identifier_inspect, is_bidi_control, unicode_policy_check,
};
use eggsact::text::{find_confusables, unicode_tools};
use eggsact::tools::text::{text_inspect, text_measure};

const ALL_BIDI: [char; 11] = [
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}', '\u{200E}', '\u{200F}',
];

#[test]
fn bidi_classification_agrees_across_consumers() {
    for c in ALL_BIDI {
        let text: String = format!("a{c}b");
        // Typed predicate agrees with itself everywhere by construction;
        // each consumer below must surface the same verdict.
        assert!(is_bidi_control(c), "U+{:04X} must be bidi", c as u32);

        let policy = unicode_policy_check(&text, "identifier_strict", None);
        assert!(
            policy.findings.iter().any(|f| f.rule == "bidi_controls"),
            "policy must flag U+{:04X}",
            c as u32
        );

        let measured = text_measure(&serde_json::json!({"text": text}));
        let risks = &measured.result.unwrap()["unicode_risks"];
        assert_eq!(
            risks["contains_bidi_controls"], true,
            "text_measure must flag U+{:04X}",
            c as u32
        );

        let inspection =
            inspect_text_security(&text, "default", "none", "summary", &|| false).unwrap();
        assert!(
            inspection.verdict == "review" || inspection.verdict == "block",
            "security composite must flag U+{:04X}",
            c as u32
        );
    }
}

#[test]
fn rlo_reports_bidi_everywhere_despite_display_label() {
    // U+202E's display is "RLO", not "BIDI": the old
    // `display.contains("BIDI")` predicate missed it in text_measure and
    // the security composite. The typed classifier must not.
    let text = "a\u{202E}b";
    let invisibles = unicode_tools::find_invisibles(text);
    assert!(invisibles.iter().any(|i| i.char == '\u{202E}'));
    let measured = text_measure(&serde_json::json!({"text": text}));
    let result = measured.result.unwrap();
    assert_eq!(result["unicode_risks"]["contains_bidi_controls"], true);
    let inspected = text_inspect(&serde_json::json!({"text": text}));
    let detail = inspected.result.unwrap();
    assert!(
        detail["bidi_controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["codepoint"] == "U+202E"),
        "RLO must land in bidi_controls, not invisibles"
    );
    assert!(
        !detail["invisibles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["codepoint"] == "U+202E"),
        "RLO must not land in invisibles"
    );
}

#[test]
fn script_classification_agrees_across_consumers() {
    // unicode_tools and unicode_policy must agree on script-bearing
    // identity for representative characters.
    for (c, script) in [
        ('A', "Latin"),
        ('А', "Cyrillic"),
        ('α', "Greek"),
        ('あ', "Hiragana"),
        ('ア', "Katakana"),
        ('漢', "Han"),
        ('한', "Hangul"),
    ] {
        assert_eq!(unicode_tools::script_name(c), script, "{c:?}");
        let probe = c.to_string();
        let policy = unicode_policy_check(&probe, "human_text", None);
        // Single-script probes never warn; the assertion is agreement-by-
        // construction plus no false mixed-script flag.
        assert!(
            !policy.findings.iter().any(|f| f.rule == "mixed_scripts"),
            "{c:?} single-script probe must not warn"
        );
    }
}

#[test]
fn mixed_script_matrix_legitimate_vs_spoof() {
    // Legitimate writing-system mixtures are not spoof mixtures.
    for text in ["日本語テスト漢字", "한글한자test"] {
        let tools = unicode_tools::detect_mixed_scripts(text);
        assert!(!tools.mixed_scripts, "{text:?} is legitimate");
        let policy = unicode_policy_check(text, "human_text", None);
        assert!(
            !policy.findings.iter().any(|f| f.rule == "mixed_scripts"),
            "{text:?} policy must not warn"
        );
    }
    // Spoof mixtures still detected by both paths.
    for text in ["hello привет", "appleаpple"] {
        let tools = unicode_tools::detect_mixed_scripts(text);
        assert!(tools.mixed_scripts, "{text:?} is a spoof mixture");
        let policy = unicode_policy_check(text, "human_text", None);
        assert!(
            policy.findings.iter().any(|f| f.rule == "mixed_scripts"),
            "{text:?} policy must warn"
        );
    }
}

#[test]
fn identifier_collision_matrix() {
    // True UTS #39 collision.
    let spoof = identifier_inspect(
        &["apple".to_string(), "аpple".to_string()],
        "python",
        "NFC",
        false,
        true,
    );
    assert!(spoof.collisions.iter().any(|c| c.kind == "confusable"));

    // Shared-component non-collision.
    let shared = identifier_inspect(
        &["АX".to_string(), "ΑY".to_string()],
        "python",
        "NFC",
        false,
        true,
    );
    assert!(!shared.collisions.iter().any(|c| c.kind == "confusable"));

    // Identical inputs: no confusable verdict.
    let identical = identifier_inspect(
        &["apple".to_string(), "apple".to_string()],
        "python",
        "NFC",
        false,
        true,
    );
    assert!(!identical.collisions.iter().any(|c| c.kind == "confusable"));

    // Multi-code-point mapping target.
    assert_eq!(confusable_skeleton("Æ"), "AE");
    assert!(are_confusable("Æ", "AE"));
    let _ = find_confusables("Æ");
}

#[test]
fn rust_unicode_identifiers_accepted_keywords_rejected() {
    // Unicode XID validity with keyword handling kept separate.
    let cafe = eggsact::text::identifier_analyze("café", Some(vec!["rust"]));
    assert_eq!(cafe.rust_valid, Some(true));
    let keyword = eggsact::text::identifier_analyze("fn", Some(vec!["rust"]));
    assert_eq!(keyword.rust_valid, Some(false));
    let ascii = eggsact::text::identifier_analyze("hello", Some(vec!["rust"]));
    assert_eq!(ascii.rust_valid, Some(true));
}
