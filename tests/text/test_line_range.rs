use eggsact::text::{line_range_compare, line_range_extract};

// ─── line_range_extract ──────────────────────────────────────────────

#[test]
fn test_line_range_extract_basic() {
    let text = "line1\nline2\nline3\nline4\nline5";
    let result = line_range_extract(text, 2, 4, 1, false, false).unwrap();
    assert_eq!(result.lines.len(), 3);
    assert!(result.lines[0].text.contains("line2"));
    assert!(result.lines[1].text.contains("line3"));
    assert!(result.lines[2].text.contains("line4"));
}

#[test]
fn test_line_range_extract_single_line() {
    let text = "line1\nline2\nline3";
    let result = line_range_extract(text, 2, 2, 1, false, false).unwrap();
    assert_eq!(result.lines.len(), 1);
    assert!(result.lines[0].text.contains("line2"));
    assert_eq!(result.text, "line2");
    assert_eq!(&text[result.byte_start..result.byte_end], result.text);
}

#[test]
fn test_line_range_extract_with_line_numbers() {
    let text = "line1\nline2\nline3";
    let result = line_range_extract(text, 1, 3, 1, true, false).unwrap();
    assert_eq!(result.lines.len(), 3);
    assert!(result.lines[0].line.is_some());
}

#[test]
fn test_line_range_extract_zero_based() {
    let text = "line1\nline2\nline3";
    let result = line_range_extract(text, 0, 1, 0, false, false).unwrap();
    assert_eq!(result.lines.len(), 2);
}

#[test]
fn test_line_range_extract_out_of_bounds() {
    let text = "line1\nline2\nline3";
    let result = line_range_extract(text, 10, 20, 1, false, false);
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.lines.is_empty());
}

#[test]
fn test_line_range_extract_empty_text() {
    // Empty text may return 0 or 1 lines for any range
    let result = line_range_extract("", 1, 5, 1, false, false).unwrap();
    assert!(result.lines.len() <= 1);
}

#[test]
fn test_line_range_extract_with_fingerprint() {
    let text = "line1\nline2\nline3";
    let result = line_range_extract(text, 1, 3, 1, false, true).unwrap();
    assert!(!result.fingerprint.is_empty());
}

#[test]
fn test_line_range_extract_crlf() {
    let text = "line1\r\nline2\r\nline3";
    let result = line_range_extract(text, 1, 3, 1, false, false).unwrap();
    assert_eq!(result.lines.len(), 3);
    assert_eq!(result.lines[0].text, "line1");
    assert_eq!(result.lines[1].text, "line2");
    assert_eq!(result.lines[2].text, "line3");
    assert_eq!(result.text, "line1\r\nline2\r\nline3");
}

#[test]
fn test_line_range_extract_cr_only_exact() {
    let text = "line1\rline2\rline3";
    let result = line_range_extract(text, 1, 3, 1, false, false).unwrap();
    assert_eq!(result.lines.len(), 3);
    assert_eq!(result.lines[0].text, "line1");
    assert_eq!(result.lines[1].text, "line2");
    assert_eq!(result.lines[2].text, "line3");
    assert_eq!(result.text, "line1\rline2\rline3");
    assert_eq!(result.ends_with_newline, text.ends_with('\r'));
}

#[test]
fn test_line_range_extract_rejects_invalid_line_base() {
    let result = line_range_extract("line1\nline2", 1, 1, 2, false, false);
    assert!(result.is_err());
}

// ─── line_range_compare ──────────────────────────────────────────────

#[test]
fn test_line_range_compare_identical() {
    let text = "line1\nline2\nline3";
    let result = line_range_compare(text, text, 1, 3, 1, "exact").unwrap();
    assert!(result.equal);
}

#[test]
fn test_line_range_compare_different() {
    let left = "line1\nline2\nline3";
    let right = "line1\nmodified\nline3";
    let result = line_range_compare(left, right, 1, 3, 1, "exact").unwrap();
    assert!(!result.equal);
}

#[test]
fn test_line_range_compare_empty_both() {
    let result = line_range_compare("", "", 1, 5, 1, "exact").unwrap();
    assert!(result.equal);
}

#[test]
fn test_line_range_compare_start_past_end_returns_error() {
    let result = line_range_compare("line1", "line1", 100, 200, 1, "exact");
    assert!(result.is_err());
}

#[test]
fn test_line_range_compare_rejects_invalid_line_base() {
    let result = line_range_compare("line1", "line1", 1, 1, 2, "exact");
    assert!(result.is_err());
}

#[test]
fn test_line_range_compare_different_length() {
    let left = "line1\nline2";
    let right = "line1\nline2\nline3";
    let result = line_range_compare(left, right, 1, 3, 1, "exact").unwrap();
    assert!(!result.equal);
}

#[test]
fn test_line_range_compare_subset() {
    let left = "line1\nline2\nline3\nline4";
    let right = "line1\nline2\nline3\nline4";
    let result = line_range_compare(left, right, 2, 3, 1, "exact").unwrap();
    assert!(result.equal);
}

#[test]
fn test_line_range_compare_context_mode() {
    let left = "line1\nline2\nline3";
    let right = "line1\nmodified\nline3";
    let result = line_range_compare(left, right, 1, 3, 1, "context");
    assert!(result.is_err());
}

// ─── Mixed newline detection edge cases ─────────────────────────────

#[test]
fn test_line_range_extract_mixed_newlines() {
    // Text with both CRLF and standalone LF should work
    let text = "line1\r\nline2\nline3";
    let result = line_range_extract(text, 1, 3, 1, false, false).unwrap();
    assert_eq!(result.lines.len(), 3);
    assert_eq!(result.text, text);
    assert_eq!(&text[result.byte_start..result.byte_end], result.text);
}

#[test]
fn test_line_range_compare_mixed_newlines() {
    let left = "line1\r\nline2\nline3";
    let right = "line1\r\nline2\nline3";
    let result = line_range_compare(left, right, 1, 3, 1, "exact").unwrap();
    assert!(result.equal);
}

#[test]
fn test_line_range_compare_exact_detects_newline_differences() {
    let left = "line1\r\nline2";
    let right = "line1\nline2";
    let result = line_range_compare(left, right, 1, 2, 1, "exact").unwrap();
    assert!(!result.equal);
    assert_eq!(result.diff_summary, "differ at line 1");
}

#[test]
fn test_line_range_compare_normalize_newlines_accepts_newline_differences() {
    let left = "line1\r\nline2";
    let right = "line1\nline2";
    let result = line_range_compare(left, right, 1, 2, 1, "normalize_newlines").unwrap();
    assert!(result.equal);
}

// ─── Regression: byte_end beyond text ─────────────────────────────────

#[test]
fn test_line_range_extract_end_beyond_text() {
    let text = "line1\nline2\nline3";
    let result = line_range_extract(text, 2, 10, 1, false, false).unwrap();
    assert!(
        result.byte_end > 0,
        "byte_end should be set when end line exceeds text"
    );
}

// ─── BUG-005: Newline style "none" for text without newlines ───────────

#[test]
fn test_line_range_extract_newline_style_none() {
    let text = "hello";
    let result = line_range_extract(text, 1, 1, 1, false, false).unwrap();
    assert_eq!(
        result.newline_style, "none",
        "Text with no newlines should report newline_style as 'none'"
    );
}

#[test]
fn test_line_range_extract_newline_style_none_multiline_not_none() {
    let text = "line1\nline2";
    let result = line_range_extract(text, 1, 2, 1, false, false).unwrap();
    assert_eq!(result.newline_style, "LF");
}
