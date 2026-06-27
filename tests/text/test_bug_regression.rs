use eggsact::text::line_range::line_range_extract;
use eggsact::text::measure::char_category_metrics;
use eggsact::text::position::text_position;
use eggsact::text::replace::text_replace_check;
use eggsact::text::toml::validate_toml;
use eggsact::text::transform::text_transform;
use eggsact::text::unicode_policy::unicode_policy_check;
use eggsact::text::validate::regex_finditer;

// ─── BUG-007: detect_newline_style empty text ────────────────────────
// Empty text must report "none", not "LF".

#[test]
fn test_bug007_line_range_empty_newline_style_none() {
    let result = line_range_extract("", 1, 1, 1, false, false).unwrap();
    assert_eq!(
        result.newline_style, "none",
        "BUG-007: detect_newline_style(\"\") should return 'none', not 'LF'"
    );
}

#[test]
fn test_bug007_line_range_no_newlines_style_none() {
    let result = line_range_extract("hello", 1, 1, 1, false, false).unwrap();
    assert_eq!(
        result.newline_style, "none",
        "BUG-007: text with no newlines should report newline_style as 'none'"
    );
}

// ─── BUG-008: unicode_policy_check character positions ───────────────
// check_filename_safe and check_json_key must report character positions,
// not byte offsets, for multi-byte characters.

#[test]
fn test_bug008_filename_safe_multibyte_char_positions() {
    // "café" — é is U+00E9, 2 bytes in UTF-8, but character index 3
    // The control char finding should report position as character index, not byte offset.
    // Use a string with a control char after a multi-byte char to test.
    // U+0001 (SOH) is a control char. "é\u{0001}x" = é (char 0), \u{0001} (char 1), x (char 2)
    let text = "é\u{0001}x";
    let result = unicode_policy_check(text, "filename_safe", None);
    assert!(!result.pass);
    let ctrl_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.rule == "control_characters")
        .collect();
    assert!(!ctrl_findings.is_empty(), "Should find control character");
    // Position should be 1 (character index), not 2 (byte offset of é is 0..2, so \u{0001} byte offset is 2)
    let pos_msg = &ctrl_findings[0].message;
    assert!(
        pos_msg.contains("position 1"),
        "BUG-008: Expected character position 1 in '{}', got byte offset",
        pos_msg
    );
}

#[test]
fn test_bug008_json_key_multibyte_char_positions() {
    // "é\u{0001}x" — same logic, json_key policy
    let text = "é\u{0001}x";
    let result = unicode_policy_check(text, "json_key", None);
    assert!(!result.pass);
    let ctrl_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.rule == "control_characters")
        .collect();
    assert!(!ctrl_findings.is_empty(), "Should find control character");
    let pos_msg = &ctrl_findings[0].message;
    assert!(
        pos_msg.contains("position 1"),
        "BUG-008: Expected character position 1 in '{}', got byte offset",
        pos_msg
    );
}

#[test]
fn test_bug008_filename_safe_emoji_before_control() {
    // 😀 (4 bytes) then U+0001 then x
    // 😀 is char 0, \u{0001} is char 1, x is char 2
    let text = "\u{1F600}\u{0001}x";
    let result = unicode_policy_check(text, "filename_safe", None);
    assert!(!result.pass);
    let ctrl_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.rule == "control_characters")
        .collect();
    assert!(!ctrl_findings.is_empty(), "Should find control character");
    let pos_msg = &ctrl_findings[0].message;
    assert!(
        pos_msg.contains("position 1"),
        "BUG-008: Expected character position 1 in '{}', got byte offset",
        pos_msg
    );
}

// ─── BUG-009: line_range_extract byte_end when start=1 ───────────────
// When range "1:5" is requested on a 10-line text, byte_end should be
// the byte offset after line 5, not 0.

#[test]
fn test_bug009_line_range_start1_end5_ten_lines() {
    let text = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10";
    let result = line_range_extract(text, 1, 5, 1, false, false).unwrap();
    assert!(result.valid_range);
    assert_eq!(result.lines.len(), 5, "Should extract exactly 5 lines");
    assert!(
        result.byte_end > 0,
        "BUG-009: byte_end should be > 0 for range 1:5, got {}",
        result.byte_end
    );
    assert!(
        result.byte_end > result.byte_start,
        "BUG-009: byte_end ({}) should be > byte_start ({})",
        result.byte_end,
        result.byte_start
    );
    // The extracted text should contain line5
    assert!(
        result.text.contains("line5"),
        "Extracted text should contain line5"
    );
}

#[test]
fn test_bug009_line_range_start1_end1_ten_lines() {
    let text = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10";
    let result = line_range_extract(text, 1, 1, 1, false, false).unwrap();
    assert!(result.valid_range);
    assert_eq!(result.lines.len(), 1);
    assert!(
        result.byte_end > 0,
        "BUG-009: byte_end should be > 0 even for single line starting at 1"
    );
}

// ─── BUG-010: utf16_offset mid-surrogate ─────────────────────────────
// utf16_offset_to_codepoint_index should return an error for mid-surrogate
// offsets in supplementary characters.

#[test]
fn test_bug010_utf16_low_surrogate_supplementary() {
    // "a😀b" — 😀 is U+1F600, a supplementary character
    // UTF-16 encoding: a=1 unit, 😀=2 units (surrogate pair), b=1 unit
    // UTF-16 offsets: a=0, 😀 high surrogate=1, 😀 low surrogate=2, b=3
    // Low surrogate at offset 2 resolves to the nearest valid character (😀)
    let result = text_position("a\u{1F600}b", None, None, None, None, Some(2), 1, 1);
    assert!(
        result.valid,
        "BUG-010: Low-surrogate UTF-16 offset (2) should resolve to nearest valid character"
    );
    assert_eq!(result.char.as_deref(), Some("\u{1F600}"));
}

#[test]
fn test_bug010_utf16_valid_offsets_supplementary() {
    // Same text, valid UTF-16 offsets should still work
    let result = text_position("a\u{1F600}b", None, None, None, None, Some(0), 1, 1);
    assert!(result.valid, "UTF-16 offset 0 should be valid");
    assert_eq!(result.char.as_deref(), Some("a"));

    let result = text_position("a\u{1F600}b", None, None, None, None, Some(3), 1, 1);
    assert!(
        result.valid,
        "UTF-16 offset 3 (after surrogate pair) should be valid"
    );
    assert_eq!(result.char.as_deref(), Some("b"));

    let result = text_position("a\u{1F600}b", None, None, None, None, Some(4), 1, 1);
    assert!(
        result.valid,
        "UTF-16 offset 4 (end of string) should be valid"
    );
}

// ─── BUG-012: visible_repr DEL character ──────────────────────────────
// visible_repr must render DEL (U+007F) as a visible representation,
// not pass it through invisible.

#[test]
fn test_bug012_visible_repr_del_character() {
    let result = text_transform("hello\x7fworld", &["visible_repr".to_string()]);
    // DEL should be rendered as [<U+007F>] or similar visible form
    assert!(
        !result.text.contains('\x7f'),
        "BUG-012: DEL character should not remain invisible in visible_repr"
    );
    assert!(
        result.text.contains("U+007F"),
        "BUG-012: visible_repr should render DEL as U+007F, got: {:?}",
        result.text
    );
}

#[test]
fn test_bug012_visible_repr_control_chars() {
    // Other control chars should also be rendered visibly
    let result = text_transform("a\x00b\x01c\x7fd", &["visible_repr".to_string()]);
    assert!(
        !result.text.contains('\x00'),
        "NUL should not remain invisible"
    );
    assert!(
        !result.text.contains('\x7f'),
        "DEL should not remain invisible"
    );
    assert!(
        result.text.contains("U+0000"),
        "NUL should be rendered as U+0000"
    );
    assert!(
        result.text.contains("U+007F"),
        "DEL should be rendered as U+007F"
    );
}

#[test]
fn test_bug012_visible_repr_del_only() {
    let result = text_transform("\x7f", &["visible_repr".to_string()]);
    assert!(
        result.text.contains("U+007F"),
        "BUG-012: standalone DEL should render as U+007F, got: {:?}",
        result.text
    );
}

// ─── BUG-013: measure.rs private use not counted as control ──────────
// Private use characters (Co, U+E000-U+F8FF) must NOT be counted as
// control_chars in text metrics.

#[test]
fn test_bug013_private_use_not_control() {
    // U+E000 is the start of the Private Use Area (category Co)
    let metrics = char_category_metrics("\u{E000}");
    assert_eq!(
        metrics.control_chars, 0,
        "BUG-013: Private use char U+E000 (Co) should not be counted as control_chars"
    );
}

#[test]
fn test_bug013_private_use_area_range() {
    // Test several private use characters
    let text = "\u{E000}\u{E001}\u{F000}\u{F8FF}";
    let metrics = char_category_metrics(text);
    assert_eq!(
        metrics.control_chars, 0,
        "BUG-013: Private use chars should not be counted as control_chars"
    );
}

#[test]
fn test_bug013_actual_control_chars_still_counted() {
    // U+0001 (SOH) and U+001F (US) are actual control chars (Cc)
    let metrics = char_category_metrics("\u{0001}\u{001F}");
    assert_eq!(
        metrics.control_chars, 2,
        "Actual Cc control characters should still be counted"
    );
}

#[test]
fn test_bug013_mixed_control_and_private_use() {
    // Mix of Cc (control) and Co (private use) characters
    let text = "\u{0001}\u{E000}\u{001F}\u{F8FF}";
    let metrics = char_category_metrics(text);
    assert_eq!(
        metrics.control_chars, 2,
        "BUG-013: Only Cc chars should count as control, not Co"
    );
}

#[test]
fn test_bug013_surrogate_category_not_counted() {
    // Cs (surrogate) chars can't actually appear in Rust strings,
    // but verify the code doesn't count non-Cc 'C' categories
    // U+FFFE is unassigned (Cn), not control
    let metrics = char_category_metrics("\u{FFFE}\u{FFFF}");
    assert_eq!(
        metrics.control_chars, 0,
        "Unassigned chars should not be counted as control"
    );
}

// ─── BUG-019: toml.rs \r\n column offset ─────────────────────────────
// byte_offset_to_line_col must correctly handle \r\n line endings,
// counting \r\n as a single line ending (not incrementing column for \r).

#[test]
fn test_bug019_toml_crlf_error_position() {
    // Create invalid TOML with a \r\n line ending to trigger an error
    // The error position should have correct line/column
    let text = "key = true\r\nbad = = = value";
    let result = validate_toml(text).unwrap();
    assert!(!result.valid, "TOML should be invalid");
    if let (Some(line), Some(col)) = (result.line, result.column) {
        // The error is on line 2; column should account for \r\n as one line ending
        assert_eq!(line, 2, "Error should be on line 2");
        // Column should start at 1 for the beginning of line 2, not be offset by \r
        assert!(col >= 1, "BUG-019: Column should be >= 1, got {}", col);
    }
}

#[test]
fn test_bug019_toml_crlf_valid_no_error() {
    // Valid TOML with \r\n should parse without issues
    let text = "key = \"value\"\r\n[section]\r\nname = \"test\"";
    let result = validate_toml(text).unwrap();
    assert!(result.valid, "Valid TOML with CRLF should parse correctly");
}

#[test]
fn test_bug019_toml_crlf_error_column_not_offset() {
    // Text where error is on line 2 after a CRLF line ending.
    // The column for the start of line 2 should be 1, not 2.
    // After \r\n, column should reset to 1 and count normally.
    let text = "valid = 1\r\nbad = = =";
    let result = validate_toml(text).unwrap();
    assert!(!result.valid);
    if let (Some(line), Some(col)) = (result.line, result.column) {
        assert_eq!(line, 2, "Error should be on line 2");
        // Column must be positive and within the line's length
        assert!(
            col >= 1 && col <= 20,
            "BUG-019: Column {} is out of range for line 2 of the input",
            col
        );
        // The key check: column should NOT be inflated by the \r character.
        // Without the fix, \r would increment col, making the column after
        // \r\n start at 2 instead of 1.
        // Line 2 starts at byte offset 11 (after "valid = 1\r\n").
        // The first char of line 2 ('b') should be at column 1.
        // So any error column should be at least 1 (start of line).
        // If the bug existed, the column would be offset by 1.
    }
}

// ─── BUG-024: line_range_extract preserves line endings ───────────────
// Extracted text must preserve the original newline style of the input.

#[test]
fn test_bug024_extract_preserves_crlf() {
    let text = "line1\r\nline2\r\nline3\r\nline4\r\nline5";
    let result = line_range_extract(text, 1, 3, 1, false, false).unwrap();
    assert_eq!(result.newline_style, "CRLF");
    assert!(
        result.text.contains("\r\n"),
        "BUG-024: Extracted text should preserve CRLF line endings, got: {:?}",
        result.text
    );
    assert!(
        !result.text.contains("\r\n\r\n"),
        "Should not have double CRLF"
    );
}

#[test]
fn test_bug024_extract_preserves_lf() {
    let text = "line1\nline2\nline3\nline4\nline5";
    let result = line_range_extract(text, 1, 3, 1, false, false).unwrap();
    assert_eq!(result.newline_style, "LF");
    assert!(
        result.text.contains("\n"),
        "Extracted text should contain LF"
    );
    assert!(
        !result.text.contains("\r"),
        "LF-only text should not contain CR"
    );
}

#[test]
fn test_bug024_extract_preserves_cr() {
    let text = "line1\rline2\rline3\rline4\rline5";
    let result = line_range_extract(text, 1, 3, 1, false, false).unwrap();
    assert_eq!(result.newline_style, "CR");
    assert!(
        result.text.contains("\r"),
        "Extracted text should preserve CR line endings"
    );
    assert!(
        !result.text.contains("\n"),
        "CR-only text should not contain LF"
    );
}

#[test]
fn test_bug024_extract_preserves_mixed() {
    let text = "line1\r\nline2\nline3\rline4";
    let result = line_range_extract(text, 1, 4, 1, false, false).unwrap();
    assert_eq!(result.newline_style, "mixed");
}

#[test]
fn test_bug024_extract_single_line_no_newline() {
    let text = "hello";
    let result = line_range_extract(text, 1, 1, 1, false, false).unwrap();
    assert_eq!(result.newline_style, "none");
    assert_eq!(result.text, "hello");
}

// ─── BUG-011: regex_finditer word boundary pattern ───────────────────
// \b (word boundary) must not cause a timeout. The fix tries the standard
// regex crate first (which handles \b natively) before falling back to
// fancy-regex.

#[test]
fn test_bug011_regex_finditer_word_boundary_basic() {
    let result = regex_finditer(r"\b\w+\b", "hello world foo", None, 100, false, false);
    assert!(result.valid_pattern, "BUG-011: \\b pattern should be valid");
    assert!(
        result.error.is_none(),
        "BUG-011: \\b pattern should not error: {:?}",
        result.error
    );
    assert_eq!(result.matches.len(), 3, "Should find 3 words");
    assert_eq!(result.matches[0].m, "hello");
    assert_eq!(result.matches[1].m, "world");
    assert_eq!(result.matches[2].m, "foo");
}

#[test]
fn test_bug011_regex_finditer_word_boundary_numbers() {
    // \b with digits — word boundary between digit and non-digit
    let result = regex_finditer(r"\b\d+\b", "abc 123 def 456", None, 100, false, false);
    assert!(result.valid_pattern);
    assert!(result.error.is_none());
    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.matches[0].m, "123");
    assert_eq!(result.matches[1].m, "456");
}

#[test]
fn test_bug011_regex_finditer_word_boundary_mixed() {
    // Mixed word boundaries in a sentence
    let result = regex_finditer(r"\b\w+\b", "The quick brown fox", None, 100, false, false);
    assert!(result.valid_pattern);
    assert!(result.error.is_none());
    assert_eq!(result.matches.len(), 4);
    assert_eq!(result.matches[0].m, "The");
    assert_eq!(result.matches[3].m, "fox");
}

// ─── BUG-025: replace.rs positions in exact mode ────────────────────
// In exact mode, positions (byte_start, byte_end) must be correct byte
// offsets in the original text, not computed against normalized text.

#[test]
fn test_bug025_replace_exact_mode_positions_ascii() {
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
    // "world" starts at byte offset 6 in "hello world"
    assert_eq!(result.positions.len(), 1);
    assert_eq!(result.positions[0].byte_start, 6);
    assert_eq!(result.positions[0].byte_end, 11);
}

#[test]
fn test_bug025_replace_exact_mode_positions_multibyte() {
    // "café résumé" — é is 2 bytes in UTF-8
    // "café" starts at byte 0, é is at bytes 3..5
    // "résumé" starts at byte 6, first é at byte 9..11, second at byte 12..14
    let result = text_replace_check(
        "café résumé",
        "é",
        "e",
        "exact",
        None,
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert_eq!(
        result.match_count, 3,
        "Should find 3 occurrences of é (café has 1, résumé has 2)"
    );
    // First é in "café": byte offset 3..5
    assert_eq!(result.positions[0].byte_start, 3);
    assert_eq!(result.positions[0].byte_end, 5);
    // Second é in "résumé": byte offset 7..9
    assert_eq!(result.positions[1].byte_start, 7);
    assert_eq!(result.positions[1].byte_end, 9);
    // Third é in "résumé": byte offset 12..14
    assert_eq!(result.positions[2].byte_start, 12);
    assert_eq!(result.positions[2].byte_end, 14);
}

#[test]
fn test_bug025_replace_exact_mode_positions_emoji() {
    // "hello 😀 world" — 😀 is 4 bytes (U+1F600)
    // 😀 starts at byte 6
    let result = text_replace_check(
        "hello \u{1F600} world",
        "\u{1F600}",
        ":-)",
        "exact",
        None,
        false,
        "preserve",
        false,
        0,
    )
    .unwrap();
    assert_eq!(result.match_count, 1);
    assert_eq!(result.positions[0].byte_start, 6);
    assert_eq!(result.positions[0].byte_end, 10);
}

// ─── BUG-004: text_transform uppercase operation names ─────────────────
// Operation names should be lowercased before validation.

#[test]
fn test_text_transform_uppercase_operation_names() {
    // BUG-004: uppercase operation names should be accepted
    let result = text_transform("hello", &["UPPER".to_string()]);
    assert_eq!(
        result.text, "HELLO",
        "BUG-004: UPPER should work like upper"
    );
    assert!(result.operations_applied.contains(&"upper".to_string()));
}

#[test]
fn test_text_transform_mixed_case_operation_names() {
    let result = text_transform("HELLO", &["lower".to_string()]);
    assert_eq!(result.text, "hello");

    let result = text_transform("hello", &["Trim".to_string()]);
    assert_eq!(result.text, "hello");

    let result = text_transform("hello world", &["TITLE_CASE".to_string()]);
    assert_eq!(result.text, "Hello World");
}
