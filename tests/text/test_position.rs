use eggsact::text::position::{text_position, text_window, TextWindowPosition};

// ─── text_position: byte offset ──────────────────────────────────────

#[test]
fn test_text_position_byte_offset_ascii() {
    let result = text_position("hello world", Some(0), None, None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.byte_offset, Some(0));
    assert_eq!(result.line, Some(1));
    assert_eq!(result.column, Some(1));
    assert_eq!(result.char.as_deref(), Some("h"));
}

#[test]
fn test_text_position_byte_offset_middle() {
    let result = text_position("hello world", Some(5), None, None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.byte_offset, Some(5));
    assert_eq!(result.char.as_deref(), Some(" "));
}

#[test]
fn test_text_position_byte_offset_end() {
    let result = text_position("hello", Some(4), None, None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.byte_offset, Some(4));
    assert_eq!(result.char.as_deref(), Some("o"));
}

#[test]
fn test_text_position_out_of_bounds() {
    let result = text_position("hello", Some(100), None, None, None, None, 1, 1);
    assert!(!result.valid);
    assert!(result.error.is_some());
}

// ─── text_position: line/column ──────────────────────────────────────

#[test]
fn test_text_position_line_column() {
    let result = text_position("hello\nworld", None, None, Some(2), Some(1), None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.line, Some(2));
    assert_eq!(result.column, Some(1));
    assert_eq!(result.char.as_deref(), Some("w"));
}

#[test]
fn test_text_position_line_column_middle() {
    let result = text_position("hello\nworld", None, None, Some(2), Some(3), None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("r"));
}

#[test]
fn test_text_position_line_out_of_bounds() {
    let result = text_position("hello", None, None, Some(5), Some(1), None, 1, 1);
    assert!(!result.valid);
}

// ─── text_position: codepoint index ──────────────────────────────────

#[test]
fn test_text_position_codepoint_ascii() {
    let result = text_position("hello", None, Some(2), None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.codepoint_index, Some(2));
    assert_eq!(result.char.as_deref(), Some("l"));
}

#[test]
fn test_text_position_codepoint_unicode() {
    // "café" — é is one codepoint
    let result = text_position("café", None, Some(3), None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("é"));
}

#[test]
fn test_text_position_codepoint_emoji() {
    let result = text_position("hi 👋", None, Some(3), None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("👋"));
}

// ─── text_position: UTF-16 offset ────────────────────────────────────

#[test]
fn test_text_position_utf16_ascii() {
    let result = text_position("hello", None, None, None, None, Some(3), 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("l"));
}

#[test]
fn test_text_position_utf16_bmp() {
    // U+00E9 (é) is a BMP character, 1 UTF-16 code unit
    let result = text_position("caf\u{00e9}", None, None, None, None, Some(3), 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("\u{00e9}"));
}

// ─── text_position: CRLF handling ────────────────────────────────────

#[test]
fn test_text_position_crlf() {
    let result = text_position("line1\r\nline2", None, None, Some(2), Some(1), None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("l"));
}

#[test]
fn test_text_position_cr_only() {
    let result = text_position("line1\rline2", None, None, Some(2), Some(1), None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.char.as_deref(), Some("l"));
}

// ─── text_position: line_base / column_base ──────────────────────────

#[test]
fn test_text_position_zero_based() {
    let result = text_position("hello", None, None, Some(0), Some(0), None, 0, 0);
    assert!(result.valid);
    assert_eq!(result.line, Some(0));
    assert_eq!(result.column, Some(0));
    assert_eq!(result.char.as_deref(), Some("h"));
}

// ─── text_position: empty input ──────────────────────────────────────

#[test]
fn test_text_position_empty() {
    let result = text_position("", Some(0), None, None, None, None, 1, 1);
    assert!(result.valid);
    assert_eq!(result.byte_offset, Some(0));
    assert_eq!(result.codepoint_index, Some(0));
    assert_eq!(result.line, Some(1));
    assert_eq!(result.column, Some(1));
}

// ─── text_window ─────────────────────────────────────────────────────

#[test]
fn test_text_window_basic() {
    let pos = TextWindowPosition {
        kind: "codepoint_index".to_string(),
        value: Some(5),
        byte_offset: None,
        codepoint_index: None,
        grapheme_index: None,
        line: None,
        column: None,
        line_base: None,
        column_base: None,
    };
    let result = text_window("hello world", &pos, 0, false);
    assert!(result.line_text.contains("hello"));
}

#[test]
fn test_text_window_start() {
    let pos = TextWindowPosition {
        kind: "codepoint_index".to_string(),
        value: Some(0),
        byte_offset: None,
        codepoint_index: None,
        grapheme_index: None,
        line: None,
        column: None,
        line_base: None,
        column_base: None,
    };
    let result = text_window("hello", &pos, 0, false);
    assert!(!result.line_text.is_empty());
}

#[test]
fn test_text_window_multiline() {
    let text = "line1\nline2\nline3";
    let pos = TextWindowPosition {
        kind: "codepoint_index".to_string(),
        value: Some(7),
        byte_offset: None,
        codepoint_index: None,
        grapheme_index: None,
        line: None,
        column: None,
        line_base: None,
        column_base: None,
    };
    let result = text_window(text, &pos, 1, false);
    // Should include context lines
    assert!(!result.line_text.is_empty());
}

#[test]
fn test_text_window_unicode() {
    let text = "café résumé";
    let pos = TextWindowPosition {
        kind: "codepoint_index".to_string(),
        value: Some(4),
        byte_offset: None,
        codepoint_index: None,
        grapheme_index: None,
        line: None,
        column: None,
        line_base: None,
        column_base: None,
    };
    let result = text_window(text, &pos, 0, false);
    assert!(!result.line_text.is_empty());
}

#[test]
fn test_text_window_byte_offset() {
    let pos = TextWindowPosition {
        kind: "byte_offset".to_string(),
        value: Some(6),
        byte_offset: None,
        codepoint_index: None,
        grapheme_index: None,
        line: None,
        column: None,
        line_base: None,
        column_base: None,
    };
    let result = text_window("hello world", &pos, 0, false);
    assert!(!result.line_text.is_empty());
}
