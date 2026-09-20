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
    if let Ok(r) = result {
        assert!(r.match_count > 0)
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
        result.newline_style_before, "none",
        "Text with no newlines should report newline_style_before as 'none'"
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

#[test]
fn test_replace_check_preserves_historical_newline_positions() {
    let cases = [
        ("lf", "a\nb", "\n", (1, 1, 2, 1, 2)),
        ("cr", "a\rb", "\r", (1, 1, 2, 1, 2)),
        ("crlf_cr", "a\r\nb", "\r", (1, 1, 2, 1, 2)),
        ("crlf_lf", "a\r\nb", "\n", (2, 2, 2, 1, 3)),
        ("after_lf", "a\nb", "b", (2, 2, 2, 1, 3)),
        ("after_cr", "a\rb", "b", (2, 2, 2, 1, 3)),
        ("after_crlf", "a\r\nb", "b", (3, 3, 2, 1, 4)),
        ("unicode_after_lf", "é\n😀", "😀", (2, 3, 2, 1, 7)),
    ];

    for (name, text, old, (codepoint_index, byte_start, line, column, byte_end)) in cases {
        let result =
            text_replace_check(text, old, "X", "exact", None, true, "preserve", false, 0).unwrap();
        assert_eq!(result.positions.len(), 1, "{name}");
        let position = &result.positions[0];
        assert_eq!(
            (
                position.codepoint_index,
                position.byte_start,
                position.line,
                position.column,
                position.byte_end,
            ),
            (codepoint_index, byte_start, line, column, byte_end),
            "{name}",
        );
    }

    for (text, expected) in [
        (
            "a\nb",
            vec![(0, 0, 1, 1), (1, 1, 2, 1), (2, 2, 2, 1), (3, 3, 2, 2)],
        ),
        (
            "a\rb",
            vec![(0, 0, 1, 1), (1, 1, 2, 1), (2, 2, 2, 1), (3, 3, 2, 2)],
        ),
        (
            "a\r\nb",
            vec![
                (0, 0, 1, 1),
                (1, 1, 2, 1),
                (2, 2, 2, 1),
                (3, 3, 2, 1),
                (4, 4, 2, 2),
            ],
        ),
    ] {
        let result =
            text_replace_check(text, "", "X", "exact", None, true, "preserve", false, 0).unwrap();
        let positions: Vec<_> = result
            .positions
            .iter()
            .map(|p| (p.codepoint_index, p.byte_start, p.line, p.column))
            .collect();
        assert_eq!(positions, expected, "empty matches in {text:?}");
    }
}
