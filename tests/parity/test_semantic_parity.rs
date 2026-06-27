use crate::parity::{
    compare_tool_parity, compare_tool_text_parity, run_python_mcp_request, run_rust_mcp_request,
};

// === Gap 3: tools/list filter validation (tier: true as int) ===

#[test]
fn test_tools_list_tier_true_as_bool() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {"tier": true},
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    let py_names: Vec<&str> = py["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    let rs_names: Vec<&str> = rs["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(py_names, rs_names, "tier:true filter mismatch");
}

#[test]
fn test_tools_list_tier_false_as_bool() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {"tier": false},
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    let py_names: Vec<&str> = py["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    let rs_names: Vec<&str> = rs["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(py_names, rs_names, "tier:false filter mismatch");
}

#[test]
fn test_tools_list_tier_int() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {"tier": 0},
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    let py_names: Vec<&str> = py["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    let rs_names: Vec<&str> = rs["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(py_names, rs_names, "tier:0 filter mismatch");
}

// === Gap 6: Regex span semantics (byte vs char offsets) ===

#[test]
fn test_regex_finditer_nonascii_spans() {
    let args = serde_json::json!({"pattern": r"\w+", "text": "é e_1"});
    let result = compare_tool_parity("regex_finditer", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_finditer_emoji_spans() {
    let args = serde_json::json!({"pattern": r".", "text": "a😀b"});
    let result = compare_tool_parity("regex_finditer", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_finditer_combining_marks() {
    let args = serde_json::json!({"pattern": r"\w+", "text": "café"});
    let result = compare_tool_parity("regex_finditer", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_finditer_named_groups_nonascii() {
    let args = serde_json::json!({"pattern": r"(?P<word>\w+)", "text": "café résumé"});
    let result = compare_tool_parity("regex_finditer", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// NOTE: regex_finditer with \b (word boundary) pattern was fixed in BUG-011.
// The standard regex crate handles \b natively; fancy-regex fallback is only
// used for lookahead/lookbehind patterns. See test_bug011_* for regression tests.

// === Gap 7: Shell tokenization (comment handling) ===

#[test]
fn test_shell_split_comment_handling() {
    let args = serde_json::json!({"command": "echo hi # comment"});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_quoted_hash() {
    let args = serde_json::json!({"command": "echo \"# not comment\""});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_single_quotes() {
    let args = serde_json::json!({"command": "echo 'hello world'"});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_double_quotes() {
    let args = serde_json::json!({"command": "echo \"hello world\""});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_backslash_escape() {
    let args = serde_json::json!({"command": "echo hello\\ world"});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_backslash_in_double_quotes() {
    let args = serde_json::json!({"command": "echo \"hello\\\"world\""});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_pipes() {
    let args = serde_json::json!({"command": "echo foo | grep bar"});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_empty_string() {
    let args = serde_json::json!({"command": ""});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Gap 8: Unicode names and character classification ===

#[test]
fn test_text_position_emoji_name() {
    let args = serde_json::json!({"text": "a😀b", "utf16_offset": 2});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_position_copyright_symbol() {
    let args = serde_json::json!({"text": "©", "codepoint_index": 0});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_emoji_metrics() {
    let args = serde_json::json!({"text": "😀"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_cjk_text() {
    let args = serde_json::json!({"text": "你好世界"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_combining_accents() {
    let args = serde_json::json!({"text": "e\u{0301}"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_precomposed_vs_decomposed() {
    let args_pre = serde_json::json!({"text": "é"}); // U+00E9
    let args_dec = serde_json::json!({"text": "e\u{301}"}); // e + combining acute
    let result_pre = compare_tool_parity("text_inspect", args_pre);
    let result_dec = compare_tool_parity("text_inspect", args_dec);
    assert!(
        result_pre.passed,
        "Precomposed parity failed: {:?}",
        result_pre.error
    );
    assert!(
        result_dec.passed,
        "Decomposed parity failed: {:?}",
        result_dec.error
    );
}

#[test]
fn test_text_inspect_variation_selectors() {
    let args = serde_json::json!({"text": "👍\u{FE0F}"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_zero_width_joiner() {
    let args = serde_json::json!({"text": "a\u{200B}b"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_regional_indicators() {
    let args = serde_json::json!({"text": "\u{1F1FA}\u{1F1F8}"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_measure_emoji() {
    let args = serde_json::json!({"text": "hello 😀 world"});
    let result = compare_tool_parity("text_measure", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_position_codepoint_emoji() {
    let args = serde_json::json!({"text": "a😀b", "codepoint_index": 1});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_position_byte_offset_emoji() {
    let args = serde_json::json!({"text": "a😀b", "byte_offset": 1});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Gap 9: Path findings envelope ===

#[test]
fn test_path_analyze_traversal_windows() {
    let args = serde_json::json!({"path": r"C:\foo\..\bar", "style": "windows"});
    let result = compare_tool_parity("path_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_analyze_traversal_posix() {
    let args = serde_json::json!({"path": "/foo/../bar", "style": "posix"});
    let result = compare_tool_parity("path_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_analyze_hidden_path() {
    let args = serde_json::json!({"path": ".gitignore", "style": "posix"});
    let result = compare_tool_parity("path_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_analyze_absolute_posix() {
    let args = serde_json::json!({"path": "/usr/local/bin", "style": "posix"});
    let result = compare_tool_parity("path_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_analyze_mixed_separators() {
    let args = serde_json::json!({"path": "C:\\foo/bar\\baz", "style": "windows"});
    let result = compare_tool_parity("path_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Gap 10: Version compare semantics ===

#[test]
fn test_version_compare_prerelease_semver() {
    let args = serde_json::json!({"a": "1.0.0-alpha", "b": "1.0.0", "scheme": "semver"});
    let result = compare_tool_parity("version_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_compare_build_metadata_semver() {
    let args = serde_json::json!({"a": "1.0.0+build", "b": "1.0.0", "scheme": "semver"});
    let result = compare_tool_parity("version_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_compare_equal_semver() {
    let args = serde_json::json!({"a": "1.0.0", "b": "1.0.0", "scheme": "semver"});
    let result = compare_tool_parity("version_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_compare_loose() {
    let args = serde_json::json!({"a": "1.2", "b": "1.10", "scheme": "loose"});
    let result = compare_tool_parity("version_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_compare_invalid_semver() {
    let args = serde_json::json!({"a": "abc", "b": "1.0.0", "scheme": "semver"});
    let result = compare_tool_parity("version_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Gap 5: Text serialization (raw content[0].text parity) ===

#[test]
fn test_text_serialization_text_transform_unicode() {
    let args = serde_json::json!({"text": "Café", "operations": ["casefold"]});
    let result = compare_tool_text_parity("text_transform", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_serialization_json_canonicalize_unicode() {
    let args = serde_json::json!({"text": r#"{"accent":"é","emoji":"😀"}"#});
    let result = compare_tool_text_parity("json_canonicalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_serialization_math_eval() {
    let args = serde_json::json!({"expression": "5 + 3"});
    let result = compare_tool_text_parity("math_eval", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_serialization_text_inspect_unicode() {
    let args = serde_json::json!({"text": "café résumé"});
    let result = compare_tool_text_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_serialization_escape_text() {
    let args = serde_json::json!({"text": "<test>", "mode": "html_text"});
    let result = compare_tool_text_parity("escape_text", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Protocol/catalog parity ===

#[test]
fn test_initialize_parity() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        },
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    // Compare protocol compatibility. The Rust port intentionally identifies
    // with its crate/server name while the Python reference reports eggcalc.
    assert_eq!(py["serverInfo"]["name"], "eggcalc");
    assert_eq!(rs["serverInfo"]["name"], "eggsact");
    assert!(py["serverInfo"]["version"].is_string());
    assert!(rs["serverInfo"]["version"].is_string());
    assert_eq!(
        py["protocolVersion"], rs["protocolVersion"],
        "protocol version mismatch"
    );
}

#[test]
fn test_ping_parity() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "ping",
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    assert_eq!(py, rs, "ping response mismatch");
}

#[test]
fn test_profiles_list_parity() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "profiles/list",
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    assert_eq!(py, rs, "profiles/list mismatch");
}

#[test]
fn test_tools_list_full_schema_parity() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {"schema_detail": "full"},
        "id": 1,
    });
    let py = run_python_mcp_request(&request).expect("Python MCP failed");
    let rs = run_rust_mcp_request(&request).expect("Rust MCP failed");
    let py_tools = py["tools"].as_array().expect("missing tools");
    let rs_tools = rs["tools"].as_array().expect("missing tools");
    assert_eq!(py_tools.len(), rs_tools.len(), "tool count mismatch");
    for (i, (py_t, rs_t)) in py_tools.iter().zip(rs_tools.iter()).enumerate() {
        assert_eq!(py_t["name"], rs_t["name"], "tool name mismatch at {}", i);
        assert_eq!(
            py_t.get("inputSchema"),
            rs_t.get("inputSchema"),
            "inputSchema mismatch for {}",
            py_t["name"]
        );
        assert_eq!(
            py_t.get("outputSchema"),
            rs_t.get("outputSchema"),
            "outputSchema mismatch for {}",
            py_t["name"]
        );
        assert_eq!(
            py_t.get("tier"),
            rs_t.get("tier"),
            "tier mismatch for {}",
            py_t["name"]
        );
        assert_eq!(
            py_t.get("tags"),
            rs_t.get("tags"),
            "tags mismatch for {}",
            py_t["name"]
        );
        assert_eq!(
            py_t.get("category"),
            rs_t.get("category"),
            "category mismatch for {}",
            py_t["name"]
        );
        assert_eq!(
            py_t.get("deprecated"),
            rs_t.get("deprecated"),
            "deprecated mismatch for {}",
            py_t["name"]
        );
    }
}

// === Error handling parity ===

#[test]
fn test_error_unknown_tool() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "nonexistent_tool", "arguments": {}},
        "id": 1,
    });
    let py = run_python_mcp_request(&request);
    let rs = run_rust_mcp_request(&request);
    assert!(py.is_err(), "Python should reject unknown tool");
    assert!(rs.is_err(), "Rust should reject unknown tool");
    assert_eq!(py.unwrap_err(), rs.unwrap_err(), "error messages differ");
}

#[test]
fn test_error_missing_required_arg() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "math_eval", "arguments": {}},
        "id": 1,
    });
    let py = run_python_mcp_request(&request);
    let rs = run_rust_mcp_request(&request);
    assert!(py.is_err(), "Python should reject missing arg");
    assert!(rs.is_err(), "Rust should reject missing arg");
    assert_eq!(py.unwrap_err(), rs.unwrap_err(), "error messages differ");
}

#[test]
fn test_error_unexpected_arg() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "math_eval", "arguments": {"expression": "1+1", "bogus": true}},
        "id": 1,
    });
    let py = run_python_mcp_request(&request);
    let rs = run_rust_mcp_request(&request);
    assert!(py.is_err(), "Python should reject unexpected arg");
    assert!(rs.is_err(), "Rust should reject unexpected arg");
    assert_eq!(py.unwrap_err(), rs.unwrap_err(), "error messages differ");
}

// === Additional edge cases ===

#[test]
fn test_text_position_line_column() {
    let args = serde_json::json!({"text": "hello\nworld", "line": 2, "column": 1});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_json_unicode() {
    let args = serde_json::json!({"text": r#"{"key": "café"}"#});
    let result = compare_tool_parity("validate_json", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_identifier_analyze_unicode() {
    let args = serde_json::json!({"name": "café"});
    let result = compare_tool_parity("identifier_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unicode_policy_check_confusable() {
    let args = serde_json::json!({"text": "héllo", "policy": "identifier_strict"});
    let result = compare_tool_parity("unicode_policy_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_canonicalize_text_unicode() {
    let args = serde_json::json!({"text": "café", "profile": "identifier"});
    let result = compare_tool_parity("canonicalize_text", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_hash_unicode() {
    let args = serde_json::json!({"text": "café résumé", "algorithms": ["sha256"]});
    let result = compare_tool_parity("text_hash", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_fingerprint_unicode() {
    let args = serde_json::json!({"text": "café résumé"});
    let result = compare_tool_parity("text_fingerprint", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_equal_unicode() {
    let args = serde_json::json!({"a": "café", "b": "café", "mode": "nfc"});
    let result = compare_tool_parity("text_equal", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_count_unicode() {
    let args = serde_json::json!({"text": "café résumé", "pattern": "é"});
    let result = compare_tool_parity("text_count", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_glob_match_unicode() {
    let args = serde_json::json!({"pattern": "café*", "text": "café résumé"});
    let result = compare_tool_parity("glob_match", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_dedupe_unicode() {
    let args = serde_json::json!({"items": ["café", "café", "résumé"]});
    let result = compare_tool_parity("list_dedupe", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_list_sort_unicode() {
    let args = serde_json::json!({"items": ["résumé", "café", "alpha"]});
    let result = compare_tool_parity("list_sort", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_brackets_unicode() {
    let args = serde_json::json!({"text": "café [résumé]"});
    let result = compare_tool_parity("validate_brackets", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_normalize_unicode() {
    let args = serde_json::json!({"path": "café/../résumé"});
    let result = compare_tool_parity("path_normalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_truncate_unicode() {
    let args = serde_json::json!({"text": "café résumé", "max_chars": 5});
    let result = compare_tool_parity("text_truncate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_window_unicode() {
    let args = serde_json::json!({"text": "café résumé", "start": 0, "end": 4});
    let result = compare_tool_parity("text_window", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_diff_explain_unicode() {
    let args = serde_json::json!({"text_a": "café", "text_b": "cafe"});
    let result = compare_tool_parity("text_diff_explain", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_line_range_extract_basic() {
    let args = serde_json::json!({"text": "line1\nline2\nline3", "start": 1, "end": 2});
    let result = compare_tool_parity("line_range_extract", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_line_range_compare_equal() {
    let args = serde_json::json!({"text_a": "line1\nline2", "text_b": "line1\nline2", "start": 1, "end": 2});
    let result = compare_tool_parity("line_range_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_regex_unicode() {
    let args = serde_json::json!({"pattern": r"\w+", "text": "café"});
    let result = compare_tool_parity("validate_regex", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_safety_check_complex() {
    let args = serde_json::json!({"pattern": r"(a+)+"});
    let result = compare_tool_parity("regex_safety_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Gap A: regex_safety_check MAX_PATTERN_LENGTH uses char count, not byte count ===

#[test]
fn test_regex_safety_cjk_pattern_parity() {
    // 600 CJK chars = 1800 bytes. Python uses len(pattern)=600 < 1000, Rust should too.
    let args = serde_json::json!({"pattern": "你好世界测试".repeat(100)});
    let result = compare_tool_parity("regex_safety_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_regex_safety_cjk_pattern_risk_level() {
    // 600 CJK chars should be risk=low in both Python and Rust
    let pattern = "你好世界测试".repeat(100);
    let py = run_python_mcp_request(&serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "regex_safety_check", "arguments": {"pattern": &pattern}}, "id": 1,
    }))
    .expect("Python MCP failed");
    let rs = run_rust_mcp_request(&serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "regex_safety_check", "arguments": {"pattern": &pattern}}, "id": 1,
    }))
    .expect("Rust MCP failed");
    let py_text = py["content"][0]["text"].as_str().unwrap();
    let rs_text = rs["content"][0]["text"].as_str().unwrap();
    let py_val: serde_json::Value = serde_json::from_str(py_text).unwrap();
    let rs_val: serde_json::Value = serde_json::from_str(rs_text).unwrap();
    assert_eq!(
        py_val["result"]["risk"], rs_val["result"]["risk"],
        "CJK pattern risk mismatch: both should be low"
    );
    assert_eq!(py_val["result"]["risk"], "low");
}

// === Gap B: validate_json position field uses char offset, not byte offset ===

#[test]
fn test_validate_json_unicode_error_position_parity() {
    // Invalid JSON after CJK characters: position should be char-based
    let args = serde_json::json!({"text": "{\"你好\": }"});
    let py = run_python_mcp_request(&serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "validate_json", "arguments": args}, "id": 1,
    }))
    .expect("Python MCP failed");
    let rs = run_rust_mcp_request(&serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "validate_json", "arguments": args}, "id": 1,
    }))
    .expect("Rust MCP failed");
    let py_text = py["content"][0]["text"].as_str().unwrap();
    let rs_text = rs["content"][0]["text"].as_str().unwrap();
    let py_val: serde_json::Value = serde_json::from_str(py_text).unwrap();
    let rs_val: serde_json::Value = serde_json::from_str(rs_text).unwrap();
    assert_eq!(
        py_val["result"]["position"], rs_val["result"]["position"],
        "position mismatch for CJK JSON error"
    );
    assert_eq!(
        py_val["result"]["column"], rs_val["result"]["column"],
        "column mismatch for CJK JSON error"
    );
}

#[test]
fn test_validate_json_unicode_error_full_parity() {
    // Full parity check for validate_json with non-ASCII error
    let args = serde_json::json!({"text": "{\"café\": }"});
    let result = compare_tool_parity("validate_json", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// === Gap C: text_inspect variation selector metrics.warnings parity ===

#[test]
fn test_text_inspect_variation_selector_metrics_warnings_parity() {
    // Variation selector should NOT add extra metrics.warnings entry
    let py = run_python_mcp_request(&serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "text_inspect", "arguments": {"text": "text\u{FE0F}", "detail": "full"}}, "id": 1,
    }))
    .expect("Python MCP failed");
    let rs = run_rust_mcp_request(&serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "text_inspect", "arguments": {"text": "text\u{FE0F}", "detail": "full"}}, "id": 1,
    }))
    .expect("Rust MCP failed");
    let py_text = py["content"][0]["text"].as_str().unwrap();
    let rs_text = rs["content"][0]["text"].as_str().unwrap();
    let py_val: serde_json::Value = serde_json::from_str(py_text).unwrap();
    let rs_val: serde_json::Value = serde_json::from_str(rs_text).unwrap();

    // Both should have 'invisible_character' in result.warnings
    let py_warnings_kinds: Vec<&str> = py_val["result"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w["kind"].as_str())
        .collect();
    let rs_warnings_kinds: Vec<&str> = rs_val["result"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w["kind"].as_str())
        .collect();
    assert_eq!(
        py_warnings_kinds, rs_warnings_kinds,
        "result.warnings kinds mismatch"
    );

    // Both Python and Rust include variation selector warnings in metrics.warnings
    let py_metrics_warnings = py_val["result"]["metrics"]["warnings"].as_array().unwrap();
    let rs_metrics_warnings = rs_val["result"]["metrics"]["warnings"].as_array().unwrap();
    let py_has_vs = py_metrics_warnings
        .iter()
        .any(|w| w.as_str().unwrap_or("").contains("variation selector"));
    let rs_has_vs = rs_metrics_warnings
        .iter()
        .any(|w| w.as_str().unwrap_or("").contains("variation selector"));
    assert_eq!(
        py_has_vs, rs_has_vs,
        "metrics.warnings variation selector mismatch"
    );
}
