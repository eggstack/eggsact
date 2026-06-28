use eggsact::text::{text_replace_check, text_replace_check_with_options, TextReplaceCheckOptions};

// ─── text_replace_check ──────────────────────────────────────────────

#[test]
fn test_replace_check_simple() {
    let result = text_replace_check(
        "hello world",
        "world",
        "rust",
        "exact",
        None,
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert_eq!(result.match_count, 1);
    assert!(result.would_change);
    assert!(result.unique_match);
}

#[test]
fn test_replace_check_with_options_api() {
    let result = text_replace_check_with_options(
        "hello world",
        "world",
        "rust",
        TextReplaceCheckOptions {
            return_preview: true,
            max_preview_chars: 100,
            ..TextReplaceCheckOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.match_count, 1);
    assert_eq!(result.preview_after, "hello rust");
}

#[test]
fn test_replace_check_no_match() {
    let result = text_replace_check(
        "hello world",
        "xyz",
        "rust",
        "exact",
        None,
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert_eq!(result.match_count, 0);
    assert!(!result.would_change);
    assert!(!result.unique_match);
    assert!(result.positions.is_empty());
}

#[test]
fn test_replace_check_first_only() {
    let result =
        text_replace_check("aaa", "a", "b", "exact", None, false, "preserve", false, 0).unwrap();
    assert_eq!(result.match_count, 3);
    assert!(result.would_change);
}

#[test]
fn test_replace_check_preview() {
    let result = text_replace_check(
        "hello world",
        "world",
        "rust",
        "exact",
        None,
        false,
        "preserve",
        true,
        100,
    )
    .unwrap();
    assert!(result.would_change);
    assert!(!result.preview_before.is_empty());
    assert!(!result.preview_after.is_empty());
}

#[test]
fn test_replace_check_expected_count() {
    let result = text_replace_check(
        "aaa",
        "a",
        "b",
        "exact",
        Some(3),
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert_eq!(result.match_count, 3);
    assert!(result.expected_count_met);
    assert!(result.would_change);
}

#[test]
fn test_replace_check_empty_old() {
    let result = text_replace_check("hello", "", "x", "exact", None, false, "preserve", false, 0);
    // Empty old string: function finds matches at every position
    match result {
        Ok(r) => assert!(r.match_count > 0),
        Err(_) => {}
    }
}

#[test]
fn test_replace_check_empty_old_multibyte_is_byte_safe() {
    let result =
        text_replace_check("é😀", "", "x", "exact", None, true, "preserve", true, 100).unwrap();
    assert_eq!(result.match_count, 3);
    assert_eq!(result.positions[0].byte_start, 0);
    assert_eq!(result.positions[1].byte_start, 2);
    assert_eq!(result.positions[2].byte_start, 6);
    assert_eq!(result.preview_after, "xéx😀x");
}

#[test]
fn test_replace_check_empty_text() {
    let result =
        text_replace_check("", "a", "b", "exact", None, false, "preserve", false, 0).unwrap();
    assert_eq!(result.match_count, 0);
    assert!(!result.would_change);
}

#[test]
fn test_replace_check_multiline() {
    let result = text_replace_check(
        "line1\nline2\nline3",
        "line",
        "item",
        "exact",
        None,
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert_eq!(result.match_count, 3);
    assert!(result.would_change);
}

#[test]
fn test_replace_check_mode_all() {
    let result =
        text_replace_check("aaa", "a", "b", "exact", None, false, "preserve", false, 0).unwrap();
    assert_eq!(result.match_count, 3);
    assert!(result.would_change);
}

#[test]
fn test_replace_check_mode_first() {
    let result =
        text_replace_check("aaa", "a", "b", "exact", None, true, "preserve", false, 0).unwrap();
    assert_eq!(result.match_count, 3);
    assert!(result.would_change);
}

#[test]
fn test_replace_check_findings() {
    let result = text_replace_check(
        "hello world",
        "world",
        "rust",
        "exact",
        None,
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert!(!result.newline_style_before.is_empty());
    assert!(!result.changed_text_fingerprint.is_empty());
}

// ─── BUG-005: Newline style "none" for text without newlines ───────────

#[test]
fn test_replace_check_newline_style_none() {
    let result = text_replace_check(
        "hello", "hello", "world", "exact", None, false, "preserve", false, 0,
    )
    .unwrap();
    assert_eq!(
        result.newline_style_before, "LF",
        "Text with no newlines should report newline_style_before as 'LF' (matching Python)"
    );
}

#[test]
fn test_replace_check_normalize_lf_policy() {
    let result = text_replace_check(
        "hello\r\nworld",
        "world",
        "rust",
        "exact",
        None,
        false,
        "normalize_lf",
        true,
        100,
    )
    .unwrap();
    assert_eq!(result.newline_style_after, "LF");
    assert_eq!(result.preview_after, "hello\nrust");
}

#[test]
fn test_replace_check_normalize_crlf_policy() {
    let result = text_replace_check(
        "hello\nworld",
        "world",
        "rust",
        "exact",
        None,
        false,
        "normalize_crlf",
        true,
        100,
    )
    .unwrap();
    assert_eq!(result.newline_style_after, "CRLF");
    assert_eq!(result.preview_after, "hello\r\nrust");
}
