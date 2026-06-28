use eggsact::text::{
    json_canonicalize, json_compare, json_extract, regex_finditer, regex_test, validate_brackets,
    validate_json, validate_regex, RegexTestResult,
};

#[test]
fn test_validate_brackets_balanced() {
    let result = validate_brackets("(a + b)").unwrap();
    assert!(result.balanced);
    assert!(result.unmatched_openers.is_empty());
    assert!(result.unmatched_closers.is_empty());

    let result = validate_brackets("[1, 2, 3]").unwrap();
    assert!(result.balanced);

    let result = validate_brackets("{a: 1}").unwrap();
    assert!(result.balanced);
}

#[test]
fn test_validate_brackets_unbalanced() {
    let result = validate_brackets("(a + b").unwrap();
    assert!(!result.balanced);
    assert_eq!(result.unmatched_openers.len(), 1);

    let result = validate_brackets("a + b)").unwrap();
    assert!(!result.balanced);
    assert_eq!(result.unmatched_closers.len(), 1);
}

#[test]
fn test_validate_json_valid() {
    let result = validate_json("{}").unwrap();
    assert!(result.valid);
    assert!(result.error.is_none());
    assert_eq!(result.json_type, Some("object".to_string()));

    let result = validate_json("[]").unwrap();
    assert!(result.valid);
    assert_eq!(result.json_type, Some("array".to_string()));
}

#[test]
fn test_validate_json_invalid() {
    let result = validate_json("{").unwrap();
    assert!(!result.valid);
    assert!(result.error.is_some());
    assert!(result.line.is_some());
    assert!(result.column.is_some());
}

#[test]
fn test_validate_regex_match() {
    assert_eq!(validate_regex(r"\d+", "123"), Ok(true));
    assert_eq!(validate_regex(r"\w+", "hello"), Ok(true));
    assert_eq!(validate_regex(r"^hello", "hello world"), Ok(true));
}

#[test]
fn test_validate_regex_no_match() {
    assert_eq!(validate_regex(r"\d+", "abc"), Ok(false));
    assert_eq!(validate_regex(r"^\d+$", "abc123def"), Ok(false));
}

#[test]
fn test_validate_regex_invalid_pattern() {
    assert!(validate_regex("[", "text").is_err());
    assert!(validate_regex("(?P<name>", "text").is_err());
}

#[test]
fn test_regex_test_basic() {
    let result = regex_test(r"\d+", &["123", "abc"], None, false, false, false, false);
    assert!(result.valid_pattern);
    assert!(result.error.is_none());
    assert_eq!(result.results.len(), 2);
    assert!(result.results[0].matches);
    assert!(result.results[0].fullmatch);
    assert!(!result.results[1].matches);
    assert!(!result.results[1].fullmatch);
}

#[test]
fn test_regex_test_with_groups() {
    let result = regex_test(
        r"(\d+)-(\d+)",
        &["123-456", "abc-def"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert_eq!(result.results[0].groups, vec!["123", "456"]);
    assert!(!result.results[1].matches);
}

#[test]
fn test_regex_test_fullmatch() {
    let result = regex_test(r"\d+", &["123", "abc123"], None, false, false, false, false);
    assert!(result.results[0].fullmatch);
    assert!(!result.results[1].fullmatch);
}

#[test]
fn test_regex_test_span() {
    let result = regex_test(r"\d+", &["abc123xyz"], None, false, false, false, false);
    assert!(result.results[0].matches);
    assert_eq!(result.results[0].span, Some(vec![3, 6]));
}

#[test]
fn test_regex_test_no_match_span() {
    let result = regex_test(r"\d+", &["abc"], None, false, false, false, false);
    assert!(!result.results[0].matches);
    assert_eq!(result.results[0].span, None);
}

#[test]
fn test_regex_test_with_flags_ignorecase() {
    let flags = vec!["IGNORECASE".to_string()];
    let result = regex_test(
        r"hello",
        &["HELLO", "hello", "HeLLo"],
        Some(&flags),
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[1].matches);
    assert!(result.results[2].matches);
}

#[test]
fn test_bug025_regex_finditer_dynamic_named_groups() {
    let result = regex_finditer(r"(?P<foo>\d+)", "abc123def456", None, 100, false, true);
    assert!(result.valid_pattern);
    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.matches[0].m, "123");
    assert_eq!(
        result.matches[0].group_dict.get("foo"),
        Some(&"123".to_string())
    );
    assert_eq!(result.matches[1].m, "456");
    assert_eq!(
        result.matches[1].group_dict.get("foo"),
        Some(&"456".to_string())
    );
}

#[test]
fn test_bug025_regex_finditer_multiple_custom_groups() {
    let result = regex_finditer(
        r"(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})",
        "date: 2024-01-15 and 2025-12-31",
        None,
        100,
        false,
        true,
    );
    assert!(result.valid_pattern);
    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.matches[0].m, "2024-01-15");
    assert_eq!(
        result.matches[0].group_dict.get("year"),
        Some(&"2024".to_string())
    );
    assert_eq!(
        result.matches[0].group_dict.get("month"),
        Some(&"01".to_string())
    );
    assert_eq!(
        result.matches[0].group_dict.get("day"),
        Some(&"15".to_string())
    );
    assert_eq!(result.matches[1].m, "2025-12-31");
    assert_eq!(
        result.matches[1].group_dict.get("year"),
        Some(&"2025".to_string())
    );
    assert_eq!(
        result.matches[1].group_dict.get("month"),
        Some(&"12".to_string())
    );
    assert_eq!(
        result.matches[1].group_dict.get("day"),
        Some(&"31".to_string())
    );
}

#[test]
fn test_bug025_regex_finditer_custom_group_not_in_hardcoded_list() {
    let result = regex_finditer(
        r"(?P<foo>\d+)|(?P<bar>[a-z]+)",
        "abc123",
        None,
        100,
        false,
        true,
    );
    assert!(result.valid_pattern);
    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.matches[0].m, "abc");
    assert!(result.matches[0].group_dict.contains_key("bar"));
    assert_eq!(
        result.matches[0].group_dict.get("bar"),
        Some(&"abc".to_string())
    );
    assert_eq!(result.matches[1].m, "123");
    assert!(result.matches[1].group_dict.contains_key("foo"));
    assert_eq!(
        result.matches[1].group_dict.get("foo"),
        Some(&"123".to_string())
    );
}

#[test]
fn test_regex_test_with_flags_multiline() {
    let flags = vec!["MULTILINE".to_string()];
    let result = regex_test(
        r"^def",
        &["abc\ndef\nxyz", "abc\ndefxyz"],
        Some(&flags),
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[1].matches);
}

#[test]
fn test_regex_test_invalid_pattern() {
    let result = regex_test(r"[", &[], None, false, false, false, false);
    assert!(!result.valid_pattern);
    assert!(result.error.is_some());
}

#[test]
fn test_regex_test_empty_samples() {
    let result: RegexTestResult = regex_test(r"\d+", &[], None, false, false, false, false);
    assert!(result.valid_pattern);
    assert!(result.results.is_empty());
}

#[test]
fn test_regex_test_lookahead_positive() {
    let result = regex_test(
        r"\d+(?=px)",
        &["100px", "200em", "300"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(!result.results[1].matches);
    assert!(!result.results[2].matches);
}

#[test]
fn test_regex_test_lookahead_negative() {
    let result = regex_test(
        r"\d+(?!px)",
        &["100em", "200px"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[1].matches);
}

#[test]
fn test_regex_test_lookbehind_positive() {
    let result = regex_test(
        r"(?<=\$)\d+",
        &["$100", "€200", "100"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(!result.results[1].matches);
    assert!(!result.results[2].matches);
}

#[test]
fn test_regex_test_lookbehind_negative() {
    let result = regex_test(
        r"(?<!\$)\d+",
        &["100", "$200"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[1].matches);
}

#[test]
fn test_regex_test_backreferences() {
    let result = regex_test(
        r"(\w)\1",
        &["aa", "bb", "ab"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[1].matches);
    assert!(!result.results[2].matches);
}

#[test]
fn test_regex_test_alternation() {
    let result = regex_test(
        r"cat|dog",
        &["cat", "dog", "bird"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[1].matches);
    assert!(!result.results[2].matches);
}

#[test]
fn test_regex_test_complex_pattern() {
    let result = regex_test(
        r"(\d{4})-(\d{2})-(\d{2})",
        &["2024-01-15", "2024-13-45", "invalid"],
        None,
        false,
        false,
        false,
        false,
    );
    assert!(result.valid_pattern);
    assert!(result.results[0].matches);
    assert!(result.results[0].fullmatch);
    assert_eq!(result.results[0].groups, vec!["2024", "01", "15"]);
    assert!(result.results[1].matches);
    assert!(!result.results[2].matches);
}

#[test]
fn test_validate_json_string_type_is_str() {
    let result = validate_json(r#""hello""#).unwrap();
    assert!(result.valid);
    assert_eq!(result.json_type, Some("str".to_string()));
}

#[test]
fn test_json_canonicalize_bool_type() {
    let result = json_canonicalize("true", false, None, false, false, false).unwrap();
    assert_eq!(result.top_level_type.as_deref(), Some("bool"));
}

#[test]
fn test_json_canonicalize_none_type() {
    let result = json_canonicalize("null", false, None, false, false, false).unwrap();
    assert_eq!(result.top_level_type.as_deref(), Some("NoneType"));
}

#[test]
fn test_validate_regex_unmatched_close_paren_rejected() {
    let result = validate_regex("a)", "test");
    assert!(result.is_err());
}

#[test]
fn test_json_compare_case_rename_path() {
    let a = r#"{"Foo": 1}"#;
    let b = r#"{"foo": 1}"#;
    let result = json_compare(a, b, true, false, false, false, false, 100).unwrap();
    if let Some(diff) = result.diffs.first() {
        assert!(
            !diff.path.contains("/->"),
            "Path should not have extra '/': {}",
            diff.path
        );
    }
}

#[test]
fn test_validate_brackets_input_too_long() {
    let long = "a".repeat(200_000);
    let result = validate_brackets(&long);
    assert!(result.is_err());
}

#[test]
fn test_regex_test_flags_used_present() {
    let result = regex_test(r"(?i)hello", &["Hello"], None, true, false, false, false);
    assert!(
        result.flags_used.is_some(),
        "flags_used should be populated"
    );
}

#[test]
fn test_json_compare_key_mismatch_no_spurious_value_changed() {
    // BUG-005: with ignore_object_order=false, key mismatches should NOT produce spurious value_changed diffs
    let a = r#"{"x": 1, "y": 2}"#;
    let b = r#"{"y": 99, "x": 1}"#;
    let result = json_compare(a, b, false, false, false, false, false, 100).unwrap();
    // Should have key_missing diffs but NO value_changed diffs
    let value_changed: Vec<_> = result
        .diffs
        .iter()
        .filter(|d| d.kind == "value_changed")
        .collect();
    assert!(
        value_changed.is_empty(),
        "BUG-005: should not have value_changed diffs when keys differ at same position, got: {:?}",
        value_changed
    );
    // Should have at least one key_missing diff
    let key_missing: Vec<_> = result
        .diffs
        .iter()
        .filter(|d| d.kind == "key_missing_in_b")
        .collect();
    assert!(
        !key_missing.is_empty(),
        "BUG-005: should have key_missing_in_b diff"
    );
}

#[test]
fn test_json_compare_same_type_true_for_key_mismatch() {
    // BUG-007: when both values are objects but keys differ, same_type should remain true
    let a = r#"{"a": 1}"#;
    let b = r#"{"b": 2}"#;
    let result = json_compare(a, b, false, false, false, false, false, 100).unwrap();
    assert!(result.same_type,
        "BUG-007: same_type should be true when both are objects with different keys, got same_type={}",
        result.same_type);
}

// ─── BUG-010: json_extract summary on invalid JSON ─────────────────────

#[test]
fn test_json_extract_invalid_json_summary_has_error() {
    let result = json_extract("{invalid", "", 1000).unwrap();
    assert!(!result.valid_json, "BUG-010: should detect invalid JSON");
    assert!(
        !result.summary.is_empty(),
        "BUG-010: invalid JSON summary should not be empty, got: {:?}",
        result.summary
    );
    assert!(
        result.summary.contains("Invalid JSON"),
        "BUG-010: summary should contain error message, got: {:?}",
        result.summary
    );
}
