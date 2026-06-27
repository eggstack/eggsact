use eggsact::text::regex_safety_check;

// ─── regex_safety_check ──────────────────────────────────────────────

#[test]
fn test_regex_safety_simple() {
    let result = regex_safety_check(r"^hello$");
    assert!(result.valid_pattern);
}

#[test]
fn test_regex_safety_catastrophic_backtracking() {
    let result = regex_safety_check(r"(a+)+b");
    assert!(result.risk != "low");
    assert!(!result.findings.is_empty());
}

#[test]
fn test_regex_safety_nested_quantifiers() {
    let result = regex_safety_check(r"(a*)*b");
    assert!(result.risk != "low");
    assert!(!result.findings.is_empty());
}

#[test]
fn test_regex_safety_alternation_backtrack() {
    let result = regex_safety_check(r"(a|a)+b");
    assert!(result.valid_pattern);
    assert_eq!(result.risk, "low");
}

#[test]
fn test_regex_safety_simple_alternation() {
    let result = regex_safety_check(r"a|b|c");
    assert!(result.valid_pattern);
}

#[test]
fn test_regex_safety_character_class() {
    let result = regex_safety_check(r"[a-z]+");
    assert!(result.valid_pattern);
}

#[test]
fn test_regex_safety_empty_pattern() {
    let result = regex_safety_check("");
    assert!(result.valid_pattern);
}

#[test]
fn test_regex_safety_complex_safe() {
    let result = regex_safety_check(r"\b\w+\b");
    assert!(result.valid_pattern);
}

#[test]
fn test_regex_safety_lookahead() {
    let result = regex_safety_check(r"\d+(?=px)");
    assert!(result.valid_pattern);
}

#[test]
fn test_regex_safety_lookbehind() {
    let result = regex_safety_check(r"(?<=\$)\d+");
    assert!(result.valid_pattern);
}
