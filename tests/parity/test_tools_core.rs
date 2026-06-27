use crate::parity::compare_tool_parity;

#[test]
fn test_math_eval_simple() {
    let args = serde_json::json!({"expression": "5 + 3"});
    let result = compare_tool_parity("math_eval", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_math_eval_nl() {
    let args = serde_json::json!({"expression": "thirty plus five"});
    let result = compare_tool_parity("math_eval", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_math_eval_multiplication() {
    let args = serde_json::json!({"expression": "12 * 14"});
    let result = compare_tool_parity("math_eval", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_math_eval_division() {
    let args = serde_json::json!({"expression": "100 / 4"});
    let result = compare_tool_parity("math_eval", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_math_eval_power() {
    let args = serde_json::json!({"expression": "2^10"});
    let result = compare_tool_parity("math_eval", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_measure_basic() {
    let args = serde_json::json!({"text": "hello world"});
    let result = compare_tool_parity("text_measure", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_measure_full() {
    let args = serde_json::json!({"text": "hello world", "detail": "full"});
    let result = compare_tool_parity("text_measure", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_measure_unicode() {
    let args = serde_json::json!({"text": "héllo wörld"});
    let result = compare_tool_parity("text_measure", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_equal_simple() {
    let args = serde_json::json!({"a": "hello", "b": "hello"});
    let result = compare_tool_parity("text_equal", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_equal_casefold() {
    let args = serde_json::json!({"a": "hello", "b": "HELLO", "casefold": true});
    let result = compare_tool_parity("text_equal", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_equal_normalize() {
    let args = serde_json::json!({"a": "café", "b": "cafe\u{0301}", "normalize": "NFC"});
    let result = compare_tool_parity("text_equal", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_json_valid() {
    let args = serde_json::json!({"text": "{\"key\": \"value\"}"});
    let result = compare_tool_parity("validate_json", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_json_invalid() {
    let args = serde_json::json!({"text": "{not json"});
    let result = compare_tool_parity("validate_json", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_json_array() {
    let args = serde_json::json!({"text": "[1, 2, 3]"});
    let result = compare_tool_parity("validate_json", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_brackets_balanced() {
    let args = serde_json::json!({"text": "(hello [world])"});
    let result = compare_tool_parity("validate_brackets", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_brackets_unbalanced() {
    let args = serde_json::json!({"text": "(hello [world)"});
    let result = compare_tool_parity("validate_brackets", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_brackets_nested() {
    let args = serde_json::json!({"text": "(([[{{}}]]))"});
    let result = compare_tool_parity("validate_brackets", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_compare_ordered() {
    let args = serde_json::json!({"a": [1, 2, 3], "b": [1, 2, 3], "mode": "ordered"});
    let result = compare_tool_parity("list_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_compare_set() {
    let args = serde_json::json!({"a": [1, 2, 3], "b": [3, 2, 1], "mode": "set"});
    let result = compare_tool_parity("list_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_compare_multiset() {
    let args = serde_json::json!({"a": [1, 1, 2], "b": [1, 2, 2], "mode": "multiset"});
    let result = compare_tool_parity("list_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_compare_different() {
    let args = serde_json::json!({"a": [1, 2, 3], "b": [4, 5, 6]});
    let result = compare_tool_parity("list_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_truncate_basic() {
    let args = serde_json::json!({"text": "hello world", "max_chars": 5});
    let result = compare_tool_parity("text_truncate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_truncate_grapheme() {
    let args = serde_json::json!({"text": "héllo wörld", "max_chars": 5, "by_graphemes": true});
    let result = compare_tool_parity("text_truncate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_regex_valid() {
    let args = serde_json::json!({"pattern": r"\d+", "test_strings": ["123", "abc"]});
    let result = compare_tool_parity("validate_regex", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_regex_invalid() {
    let args = serde_json::json!({"pattern": r"[", "test_strings": ["test"]});
    let result = compare_tool_parity("validate_regex", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_count_basic() {
    let args = serde_json::json!({"text": "hello world hello", "pattern": "hello"});
    let result = compare_tool_parity("text_count", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_count_regex() {
    let args =
        serde_json::json!({"text": "hello123world456", "pattern": r"\d+", "count_mode": "regex"});
    let result = compare_tool_parity("text_count", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
