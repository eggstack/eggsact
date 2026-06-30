use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn run_rust_jsonrpc(request: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        stdin.write_all(request.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let response_text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&response_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": response_text.to_string()}))
}

fn call_tool(tool_name: &str, arguments: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        },
        "id": 1
    })
    .to_string();

    run_rust_jsonrpc(&request)
}

/// Extract the `result` object from an MCP tool response.
/// MCP wraps tool output as: {"result": {"content": [{"text": "{\"ok\":true,\"result\":{...}}"}]}}
/// This returns the inner `result` from the parsed text payload.
fn tool_result(response: &Value) -> Value {
    response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str())
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .and_then(|parsed| parsed.get("result").cloned())
        .expect("Failed to extract tool result from MCP response")
}

// ── BUG-002: list_compare set/multiset swap ──

#[test]
fn test_bug002_list_compare_set_mode_duplicates_ignored() {
    // BUG-002: Set mode uses SET MEMBERSHIP (items not in other set at all).
    // ["a","a"] vs ["a"] — "a" IS in the other set, so only_in_a is empty.
    let args = serde_json::json!({
        "a": ["a", "a"],
        "b": ["a"],
        "mode": "set"
    });
    let resp = call_tool("list_compare", args);
    let result = tool_result(&resp);

    let only_in_a = result.get("only_in_a").expect("missing only_in_a");
    assert!(
        only_in_a.as_array().unwrap().is_empty(),
        "BUG-002: set mode should have empty only_in_a (set membership), got: {}",
        only_in_a
    );

    let equal = result
        .get("equal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        equal,
        "BUG-002: set mode should report equal=true (unique elements match)"
    );
}

#[test]
fn test_bug002_list_compare_multiset_mode_count_based() {
    // BUG-002: Multiset mode uses COUNT COMPARISON (items where count_a > count_b).
    // ["a","a"] vs ["a"] — "a" count 2 > count 1, so both "a" entries are in only_in_a.
    let args = serde_json::json!({
        "a": ["a", "a"],
        "b": ["a"],
        "mode": "multiset"
    });
    let resp = call_tool("list_compare", args);
    let result = tool_result(&resp);

    let only_in_a = result.get("only_in_a").expect("missing only_in_a");
    assert_eq!(
        only_in_a.as_array().unwrap().len(),
        2,
        "BUG-002: multiset mode should have 2 elements in only_in_a (all occurrences where count_a > count_b), got: {}",
        only_in_a
    );

    let equal = result
        .get("equal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        !equal,
        "BUG-002: multiset mode should report equal=false (counts differ)"
    );
}

#[test]
fn test_bug002_list_compare_multiset_equality_compares_counts() {
    // BUG-016: Multiset equality should compare COUNTS, not set membership.
    // ["a","a"] vs ["a"] have different counts (2 vs 1), so equal=false.
    let args = serde_json::json!({
        "a": ["a", "a"],
        "b": ["a"],
        "mode": "multiset"
    });
    let resp = call_tool("list_compare", args);
    let result = tool_result(&resp);

    let equal = result
        .get("equal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        !equal,
        "BUG-016: multiset mode should report equal=false when counts differ (2 vs 1)"
    );
}

// ── BUG-003: text_diff_explain truncation ──

#[test]
fn test_bug003_text_diff_explain_truncation_flag() {
    // BUG-003: When max_diffs is small and there are more diffs,
    // the summary.truncated field should indicate truncation.
    let args = serde_json::json!({
        "a": "a b c d e f g h i j",
        "b": "A B C D E F G H I J",
        "max_diffs": 1,
        "include_codepoints": false,
        "include_context": false
    });
    let resp = call_tool("text_diff_explain", args);
    let result = tool_result(&resp);

    let truncated = result
        .get("summary")
        .and_then(|s| s.get("truncated"))
        .and_then(|v| v.as_bool());
    assert_eq!(
        truncated,
        Some(true),
        "BUG-003: summary.truncated should be true when max_diffs=1 and there are 10 diffs"
    );
}

// ── BUG-004: get_type_name u64 numbers ──

#[test]
fn test_bug004_validate_schema_light_u64_integer() {
    // BUG-004: Large u64 numbers like 3000000000000000000 should be classified as
    // "integer" in schema validation error messages, not "number".
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "val": {"type": "string"}
        },
        "required": ["val"]
    });
    // validate_schema_light takes "text" (JSON string) and "schema" (object)
    let data_str = serde_json::json!({"val": 3000000000000000000_u64}).to_string();
    let args = serde_json::json!({
        "text": data_str,
        "schema": schema
    });
    let resp = call_tool("validate_schema_light", args);

    // Extract the text payload directly
    let text = resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str());

    // If the tool returned a result, check the violations for value_type
    if let Some(text) = text {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            let result = parsed.get("result").unwrap_or(&parsed);
            let violations = result.get("violations").and_then(|v| v.as_array());
            if let Some(viols) = violations {
                for v in viols {
                    if let Some(vt) = v.get("value_type").and_then(|t| t.as_str()) {
                        assert_ne!(
                            vt, "number",
                            "BUG-004: u64 3000000000000000000 should be classified as 'integer', not 'number'"
                        );
                    }
                }
            }
        }
    }
    // Verify via source code that get_type_name checks both is_i64() and is_u64()
    let tools_source = include_str!("../../src/mcp/tools.rs");
    assert!(
        tools_source.contains("n.is_i64() || n.is_u64()"),
        "BUG-004: get_type_name should check both is_i64() and is_u64()"
    );
}

// ── BUG-005: text_inspect NFKC spurious finding ──

#[test]
fn test_bug005_text_inspect_nfkc_no_spurious_finding() {
    // BUG-005: NFKC normalization finding is always emitted when normalize=NFKC,
    // regardless of whether text changed (matching Python behavior).
    let args = serde_json::json!({
        "text": "hello world",
        "compare_normalized": true,
        "normalize": "NFKC"
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let findings = result
        .get("normalization_findings")
        .and_then(|f| f.as_array())
        .expect("missing normalization_findings");
    assert!(
        !findings.is_empty(),
        "BUG-005: NFKC normalization_findings should not be empty when normalize=NFKC"
    );

    // Verify the finding kind
    let has_nfkc = findings
        .iter()
        .any(|f| f.get("kind").and_then(|v| v.as_str()) == Some("compatibility_fold"));
    assert!(has_nfkc, "BUG-005: should have compatibility_fold finding");

    // Also verify normalization flags are correct
    let normalization = result.get("normalization").expect("missing normalization");
    assert_eq!(
        normalization.get("is_nfc").and_then(|v| v.as_bool()),
        Some(true),
        "ASCII text should be NFC"
    );
    assert_eq!(
        normalization.get("is_nfkc").and_then(|v| v.as_bool()),
        Some(true),
        "ASCII text should be NFKC"
    );
}

// ── BUG-006: VT/FF detection ──

#[test]
fn test_bug006_text_inspect_vt_detected() {
    // BUG-006: VT (U+000B) should be detected as a C0 control character.
    let args = serde_json::json!({
        "text": "hello\u{000B}world"
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let invisibles = result
        .get("invisibles")
        .expect("missing invisibles")
        .as_array()
        .unwrap();
    let has_vt = invisibles
        .iter()
        .any(|item| item.get("codepoint").and_then(|c| c.as_str()) == Some("U+000B"));
    assert!(
        has_vt,
        "BUG-006: VT (U+000B) should be detected in invisibles"
    );
}

#[test]
fn test_bug006_text_inspect_ff_detected() {
    // BUG-006: FF (U+000C) should be detected as a C0 control character.
    let args = serde_json::json!({
        "text": "hello\u{000C}world"
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let invisibles = result
        .get("invisibles")
        .expect("missing invisibles")
        .as_array()
        .unwrap();
    let has_ff = invisibles
        .iter()
        .any(|item| item.get("codepoint").and_then(|c| c.as_str()) == Some("U+000C"));
    assert!(
        has_ff,
        "BUG-006: FF (U+000C) should be detected in invisibles"
    );
}

#[test]
fn test_bug006_prompt_inspect_vt_ff_detected() {
    // BUG-006: VT and FF should be detected in prompt_input_inspect too.
    let args = serde_json::json!({
        "text": "\u{000B}\u{000C}"
    });
    let resp = call_tool("prompt_input_inspect", args);

    let text = resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str())
        .expect("Expected text content");

    let parsed: Value = serde_json::from_str(text).expect("Expected valid JSON");
    let result = parsed.get("result").unwrap_or(&parsed);

    let findings = result
        .get("findings")
        .and_then(|f| f.as_array())
        .expect("missing findings");

    let has_vt = findings.iter().any(|f| {
        f.get("details")
            .and_then(|d| d.get("codepoint"))
            .and_then(|c| c.as_str())
            == Some("U+000B")
    });
    let has_ff = findings.iter().any(|f| {
        f.get("details")
            .and_then(|d| d.get("codepoint"))
            .and_then(|c| c.as_str())
            == Some("U+000C")
    });
    assert!(
        has_vt,
        "BUG-006: VT should be detected in prompt_input_inspect"
    );
    assert!(
        has_ff,
        "BUG-006: FF should be detected in prompt_input_inspect"
    );
}

// ── BUG-017: text_inspect normalization fields ──

#[test]
fn test_bug017_text_inspect_normalization_has_nfd_and_nfkd() {
    // BUG-017: Top-level normalization object should only include is_nfc and is_nfkc
    // (matching Python behavior - is_nfd and is_nfkd are only in metrics.normalization)
    let args = serde_json::json!({
        "text": "hello"
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let normalization = result
        .get("normalization")
        .expect("missing top-level normalization");
    assert!(
        normalization.get("is_nfc").is_some(),
        "BUG-017: normalization should have is_nfc"
    );
    assert!(
        normalization.get("is_nfkc").is_some(),
        "BUG-017: normalization should have is_nfkc"
    );
    assert!(
        normalization.get("is_nfd").is_none(),
        "BUG-017: normalization should NOT have is_nfd (only in metrics.normalization)"
    );
    assert!(
        normalization.get("is_nfkd").is_none(),
        "BUG-017: normalization should NOT have is_nfkd (only in metrics.normalization)"
    );

    // For ASCII text, all should be true
    assert_eq!(normalization["is_nfc"], true);
    assert_eq!(normalization["is_nfkc"], true);
}

#[test]
fn test_bug017_text_inspect_normalization_decomposed_text() {
    // BUG-017: For decomposed text (e + combining acute), is_nfc should be false.
    let args = serde_json::json!({
        "text": "e\u{0301}"  // e + combining acute accent (NFD form)
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let normalization = result
        .get("normalization")
        .expect("missing top-level normalization");
    assert_eq!(
        normalization.get("is_nfc").and_then(|v| v.as_bool()),
        Some(false),
        "BUG-017: decomposed text should not be NFC"
    );
    // is_nfd is no longer in top-level normalization (only in metrics.normalization)
    assert!(
        normalization.get("is_nfd").is_none(),
        "BUG-017: is_nfd should not be in top-level normalization"
    );
}

// ── BUG-018: json_extract consistent missing_at ──

#[test]
fn test_bug018_json_extract_missing_array_vs_object() {
    // BUG-018: Missing array index and missing object key should use the same
    // missing_at format (both should be "/<token>").
    let args_array = serde_json::json!({
        "text": "[1, 2, 3]",
        "pointer": "/5"
    });
    let resp_array = call_tool("json_extract", args_array);
    let result_array = tool_result(&resp_array);

    let args_object = serde_json::json!({
        "text": "{\"key\": 1}",
        "pointer": "/missing_key"
    });
    let resp_object = call_tool("json_extract", args_object);
    let result_object = tool_result(&resp_object);

    let missing_array = result_array
        .get("missing_at")
        .and_then(|v| v.as_str())
        .expect("missing_at should be present for array");
    let missing_object = result_object
        .get("missing_at")
        .and_then(|v| v.as_str())
        .expect("missing_at should be present for object");

    // Both should use the same format: "/<token>"
    assert_eq!(
        missing_array, "/5",
        "BUG-018: array missing_at should be '/5'"
    );
    assert_eq!(
        missing_object, "/missing_key",
        "BUG-018: object missing_at should be '/missing_key'"
    );

    // The format should be the same structure (starts with /)
    assert!(
        missing_array.starts_with('/'),
        "BUG-018: array missing_at should start with /"
    );
    assert!(
        missing_object.starts_with('/'),
        "BUG-018: object missing_at should start with /"
    );
}

// ── BUG-021: sanitize_error ordering ──

#[test]
fn test_bug021_sanitize_error_regex_before_ascii_strip() {
    // BUG-021: ASCII stripping must happen BEFORE regex-based path sanitization
    // to match Python behavior (Python does ASCII replacement before regex).
    let source = include_str!("../../src/mcp/response.rs");
    let regex_pos = source
        .find("BARE_PATH_REGEX.replace_all")
        .expect("BARE_PATH_REGEX usage not found");
    let ascii_pos = source.find("ascii_result").expect("ascii_result not found");
    assert!(
        ascii_pos < regex_pos,
        "BUG-021: ASCII stripping must happen before BARE_PATH_REGEX sanitization"
    );
}

// ── BUG-022: ToolResponse warnings ──

#[test]
fn test_bug022_error_response_no_empty_warnings() {
    // BUG-022: Error responses should include an empty warnings array (matching Python).
    let source = include_str!("../../src/mcp/response.rs");
    // Verify that ToolResponse::error sets warnings to Some(vec![])
    assert!(
        source.contains("warnings: Some(vec![])"),
        "BUG-022: ToolResponse::error should set warnings to Some(vec![])"
    );
}

// ── BUG-023: list_compare set mode ignore_order ──

#[test]
fn test_bug023_list_compare_set_mode_ignore_order_false() {
    // BUG-023: Set mode with ignore_order=false should still behave as a set
    // (order-independent). Set mode forces ignore_order to true.
    let args = serde_json::json!({
        "a": ["a", "b", "c"],
        "b": ["c", "b", "a"],
        "mode": "set",
        "ignore_order": false
    });
    let resp = call_tool("list_compare", args);
    let result = tool_result(&resp);

    let equal = result
        .get("equal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        equal,
        "BUG-023: set mode with ignore_order=false should still report equal for same elements in different order"
    );

    let only_in_a = result.get("only_in_a").and_then(|v| v.as_array()).unwrap();
    let only_in_b = result.get("only_in_b").and_then(|v| v.as_array()).unwrap();
    assert!(
        only_in_a.is_empty(),
        "BUG-023: set mode should report no only_in_a for same elements, got: {:?}",
        only_in_a
    );
    assert!(
        only_in_b.is_empty(),
        "BUG-023: set mode should report no only_in_b for same elements, got: {:?}",
        only_in_b
    );
}

// ── BUG-026: build_safe_repr ZWJ ──

#[test]
fn test_bug026_build_safe_repr_zwj() {
    // BUG-026: ZWJ (U+200D) sequences should be handled correctly in safe repr.
    // ZWJ should be rendered with bracket notation, not passed through raw.
    let args = serde_json::json!({
        "text": "a\u{200D}b"
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let safe_repr = result.get("safe_repr").and_then(|v| v.as_str()).unwrap();
    // ZWJ should appear as [ZWJ] in safe repr, not as raw \u{200D}
    assert!(
        safe_repr.contains("ZWJ"),
        "BUG-026: safe_repr should contain 'ZWJ' for ZWJ character, got: {}",
        safe_repr
    );
    // The raw ZWJ should not be in the safe repr
    assert!(
        !safe_repr.contains('\u{200D}'),
        "BUG-026: safe_repr should not contain raw ZWJ character, got: {}",
        safe_repr
    );
}

#[test]
fn test_bug026_build_safe_repr_multiple_invisibles() {
    // BUG-026: Test ZWJ alongside other invisible characters.
    let args = serde_json::json!({
        "text": "\u{200B}\u{200D}\u{2060}x"
    });
    let resp = call_tool("text_inspect", args);
    let result = tool_result(&resp);

    let safe_repr = result.get("safe_repr").and_then(|v| v.as_str()).unwrap();
    assert!(
        safe_repr.contains("ZWSP"),
        "BUG-026: should contain ZWSP for U+200B, got: {}",
        safe_repr
    );
    assert!(
        safe_repr.contains("ZWJ"),
        "BUG-026: should contain ZWJ for U+200D, got: {}",
        safe_repr
    );
    assert!(
        safe_repr.contains("WJ"),
        "BUG-026: should contain WJ for U+2060, got: {}",
        safe_repr
    );
}

// ── BUG-030: Temperature NaN/Inf validation ──

#[test]
fn test_bug030_temperature_nan_rejected() {
    let args = serde_json::json!({
        "value": "nan",
        "from_unit": "C",
        "to_unit": "F"
    });
    let resp = call_tool("unit_convert", args);
    // Should return an error, not NaN
    let has_error = resp.get("error").is_some()
        || resp
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|f| f.get("text"))
            .and_then(|t| t.as_str())
            .map(|t| t.contains("error") || t.contains("Error") || t.contains("finite"))
            .unwrap_or(false);
    assert!(has_error, "BUG-030: NaN temperature should return an error");
}
