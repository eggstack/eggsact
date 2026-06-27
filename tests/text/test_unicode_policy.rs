use eggsact::text::{canonicalize_text, unicode_policy_check};

// ─── unicode_policy_check ────────────────────────────────────────────

#[test]
fn test_unicode_policy_check_ascii() {
    let result = unicode_policy_check("hello world", "human_text", None);
    assert!(result.pass);
    assert!(result.findings.is_empty());
}

#[test]
fn test_unicode_policy_check_empty() {
    let result = unicode_policy_check("", "human_text", None);
    assert!(result.pass);
    assert!(result.findings.is_empty());
}

#[test]
fn test_unicode_policy_check_confusables() {
    let result = unicode_policy_check("\u{0430}pple", "human_text", None);
    assert!(
        result.pass,
        "Confusable finding should be warning, not error, under human_text"
    );
    assert!(result.findings.iter().any(|f| f.rule == "confusables"));
}

#[test]
fn test_unicode_policy_check_bidi() {
    let result = unicode_policy_check("hello\u{202a}world", "human_text", None);
    assert!(
        result.pass,
        "Bidi finding should be warning, not error, under human_text"
    );
    assert!(result.findings.iter().any(|f| f.rule == "bidi_controls"));
}

#[test]
fn test_unicode_policy_check_invisible() {
    let result = unicode_policy_check("hello\u{200b}world", "human_text", None);
    assert!(
        result.pass,
        "Zero-width finding should be warning, not error, under human_text"
    );
    assert!(result
        .findings
        .iter()
        .any(|f| f.rule == "zero_width_characters"));
}

#[test]
fn test_unicode_policy_check_normalization_nfc() {
    let result = unicode_policy_check("caf\u{00e9}", "human_text", Some("NFC"));
    assert!(result.pass);
    assert_eq!(result.normalized_form, "caf\u{00e9}");
}

#[test]
fn test_unicode_policy_check_normalization_nfd() {
    let result = unicode_policy_check("caf\u{00e9}", "human_text", Some("NFD"));
    assert!(result.pass);
}

#[test]
fn test_unicode_policy_check_strict() {
    let result = unicode_policy_check("hello\u{200b}world", "json_key", None);
    assert!(
        !result.pass,
        "Zero-width characters should be error under json_key policy"
    );
    assert!(result
        .findings
        .iter()
        .any(|f| f.rule == "zero_width_characters" && f.severity == "error"));
}

// ─── canonicalize_text ───────────────────────────────────────────────

#[test]
fn test_canonicalize_ascii() {
    let result = canonicalize_text("hello", "source_file_identity", false);
    assert!(result.base.text.contains("hello"));
}

#[test]
fn test_canonicalize_empty() {
    let result = canonicalize_text("", "source_file_identity", false);
    assert_eq!(result.base.text, "\n");
    assert!(result.base.changed);
}

#[test]
fn test_canonicalize_identifier_compare() {
    let result = canonicalize_text("Hello", "identifier_compare", false);
    assert_eq!(result.base.text, "hello");
    assert!(result.base.changed);
}

#[test]
fn test_canonicalize_human_label_compare() {
    let result = canonicalize_text("caf\u{00e9}", "human_label_compare", false);
    assert!(!result.base.text.is_empty());
}

#[test]
fn test_canonicalize_path_segment_compare() {
    let result = canonicalize_text("caf\u{00e9}", "path_segment_compare", false);
    assert!(!result.base.text.is_empty());
}

#[test]
fn test_canonicalize_with_mapping() {
    let result = canonicalize_text("Hello", "identifier_compare", true);
    assert!(result.base.changed);
    assert!(result.mapping.is_some());
}

#[test]
fn test_canonicalize_no_mapping() {
    let result = canonicalize_text("Hello", "identifier_compare", false);
    assert!(result.mapping.is_none());
}

#[test]
fn test_canonicalize_fingerprint() {
    let result = canonicalize_text("Hello", "identifier_compare", false);
    assert!(!result.base.fingerprint_before.is_empty());
    assert!(!result.base.fingerprint_after.is_empty());
}

#[test]
fn test_canonicalize_operations() {
    let result = canonicalize_text("Hello", "identifier_compare", false);
    assert!(!result.base.operations_applied.is_empty());
}
