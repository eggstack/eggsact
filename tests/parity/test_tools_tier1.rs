use crate::parity::{compare_tool_parity, compare_tool_parity_superset};

#[test]
fn test_text_transform_lowercase() {
    let args = serde_json::json!({"text": "HELLO World", "transform": "lowercase"});
    let result = compare_tool_parity("text_transform", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_transform_uppercase() {
    let args = serde_json::json!({"text": "hello world", "transform": "uppercase"});
    let result = compare_tool_parity("text_transform", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_transform_reverse() {
    let args = serde_json::json!({"text": "hello", "transform": "reverse"});
    let result = compare_tool_parity("text_transform", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_transform_title_case() {
    let args = serde_json::json!({"text": "hello world", "transform": "title_case"});
    let result = compare_tool_parity("text_transform", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_position_basic() {
    let args =
        serde_json::json!({"text": "hello world", "pattern": "world", "position_type": "index"});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_position_grapheme() {
    let args = serde_json::json!({"text": "héllo wörld", "pattern": "wörld", "position_type": "grapheme_offset"});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_hash_sha256() {
    let args = serde_json::json!({"text": "hello world", "algorithm": "sha256"});
    let result = compare_tool_parity("text_hash", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_hash_md5() {
    let args = serde_json::json!({"text": "hello world", "algorithm": "md5"});
    let result = compare_tool_parity("text_hash", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_escape_text_html() {
    let args = serde_json::json!({"text": "<div>&\"test\"</div>", "format": "html"});
    let result = compare_tool_parity("escape_text", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unescape_text_html() {
    let args = serde_json::json!({"text": "&lt;div&gt;&amp;&quot;test&quot;&lt;/div&gt;", "format": "html"});
    let result = compare_tool_parity("unescape_text", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_window_basic() {
    let args = serde_json::json!({"text": "hello world", "start": 0, "end": 5});
    let result = compare_tool_parity("text_window", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_window_negative_index() {
    let args = serde_json::json!({"text": "hello world", "start": -6, "end": -1});
    let result = compare_tool_parity("text_window", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_toml_valid() {
    let args = serde_json::json!({"text": "key = \"value\""});
    let result = compare_tool_parity("validate_toml", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_toml_invalid() {
    let args = serde_json::json!({"text": "key ="});
    let result = compare_tool_parity("validate_toml", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_json_extract_basic() {
    let args = serde_json::json!({"text": "{\"a\": {\"b\": 1}}", "path": "/a/b"});
    let result = compare_tool_parity("json_extract", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_json_compare_equal() {
    let args = serde_json::json!({"a": "{\"x\": 1}", "b": "{\"x\": 1}"});
    let result = compare_tool_parity("json_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_json_canonicalize_basic() {
    let args = serde_json::json!({"text": "{\"b\": 1, \"a\": 2}"});
    let result = compare_tool_parity("json_canonicalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_json_query_basic() {
    let args = serde_json::json!({"text": "{\"a\": 1, \"b\": 2}", "path": "/a"});
    let result = compare_tool_parity("json_query", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_dedupe_basic() {
    let args = serde_json::json!({"items": [1, 2, 2, 3, 1]});
    let result = compare_tool_parity("list_dedupe", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_sort_basic() {
    let args = serde_json::json!({"items": [3, 1, 2], "order": "asc"});
    let result = compare_tool_parity("list_sort", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_sort_stable() {
    let args =
        serde_json::json!({"items": ["b", "a", "B", "A"], "order": "asc", "case_sensitive": false});
    let result = compare_tool_parity("list_sort", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_glob_match_basic() {
    let args = serde_json::json!({"text": "testfile.txt", "pattern": "*.txt"});
    let result = compare_tool_parity("glob_match", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_glob_match_nested() {
    let args = serde_json::json!({"text": "path/to/file.txt", "pattern": "**/*.txt"});
    let result = compare_tool_parity("glob_match", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_finditer_basic() {
    let args = serde_json::json!({"text": "abc123def456", "pattern": r"\d+"});
    let result = compare_tool_parity_superset("regex_finditer", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_safety_check_complex() {
    let args = serde_json::json!({"pattern": r"(a+)+b", "test_strings": ["aaaaax"]});
    let result = compare_tool_parity("regex_safety_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
