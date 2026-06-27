use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn mcp_request(request: &str) -> String {
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
    String::from_utf8(output.stdout).unwrap_or_default()
}

fn call_tool_and_get_result(request: &str) -> Value {
    let response = mcp_request(request);
    let parsed: Value = serde_json::from_str(&response).expect("Invalid JSON response");
    parsed["result"]
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_else(|| {
            panic!(
                "Failed to extract tool result from response: {}",
                &response[..response.len().min(2000)]
            )
        })
}

// ─── TEXT_DIFF_EXPLAIN: basic structural comparison ──────────────────

#[test]
fn test_mcp_text_diff_explain_basic() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "hello world", "b": "hello earth"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(false)));
    assert!(
        inner
            .get("classification")
            .and_then(|c| c.as_str())
            .is_some(),
        "Expected classification string"
    );
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        !diffs.is_empty(),
        "Expected non-empty diffs for differing strings"
    );
}

// ─── TEXT_DIFF_EXPLAIN: identical strings ────────────────────────────

#[test]
fn test_mcp_text_diff_explain_identical() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "same", "b": "same"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(true)));
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        diffs.is_empty(),
        "Expected empty diffs for identical strings, got: {:?}",
        diffs
    );
}

// ─── TEXT_DIFF_EXPLAIN: empty strings ────────────────────────────────

#[test]
fn test_mcp_text_diff_explain_empty() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "", "b": "hello"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(false)));
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        !diffs.is_empty(),
        "Expected non-empty diffs when one side is empty"
    );
}

// ─── TEXT_DIFF_EXPLAIN: max_diffs parameter ─────────────────────────

#[test]
fn test_mcp_text_diff_explain_max_diffs() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "aaaa", "b": "bbbb", "max_diffs": 1}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        diffs.len() <= 1,
        "Expected at most 1 diff with max_diffs=1, got: {:?}",
        diffs
    );
}

// ─── TEXT_DIFF_EXPLAIN: line-level difference ────────────────────────

#[test]
fn test_mcp_text_diff_explain_line_level() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "line1\nline2\nline3", "b": "line1\nmodified\nline3"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        diffs
            .iter()
            .any(|d| d.get("kind").and_then(|k| k.as_str()) != Some("equal")),
        "Expected a non-equal diff for line change, got: {:?}",
        diffs
    );
}

// ─── TEXT_DIFF_EXPLAIN: max_diffs edge cases ────────────────────────

#[test]
fn test_mcp_text_diff_explain_max_diffs_zero() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "abc", "b": "xyz", "max_diffs": 0}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        diffs.is_empty(),
        "Expected empty diffs with max_diffs=0, got: {:?}",
        diffs
    );
}

// ─── TEXT_DIFF_EXPLAIN: both empty ───────────────────────────────────

#[test]
fn test_mcp_text_diff_explain_both_empty() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "", "b": ""}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(true)));
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(diffs.is_empty());
}

// ─── TEXT_DIFF_EXPLAIN: unicode content ──────────────────────────────

#[test]
fn test_mcp_text_diff_explain_unicode() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "café", "b": "cafè"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(false)));
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        !diffs.is_empty(),
        "Expected non-empty diffs for Unicode differing strings, got: {:?}",
        diffs
    );
}

// ─── TEXT_DIFF_EXPLAIN: input validation ─────────────────────────────

#[test]
fn test_mcp_text_diff_explain_missing_arguments() {
    let response = mcp_request(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "hello"}},
            "id": 1
        })
        .to_string(),
    );
    let parsed: Value = serde_json::from_str(&response).expect("Invalid JSON response");
    // Missing required argument returns a JSON-RPC error
    assert!(
        parsed.get("error").is_some(),
        "Expected JSON-RPC error for missing argument, got: {}",
        &response[..response.len().min(2000)]
    );
}

#[test]
fn test_mcp_text_diff_explain_non_string_arguments() {
    let response = mcp_request(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": 123, "b": 456}},
            "id": 1
        })
        .to_string(),
    );
    let parsed: Value = serde_json::from_str(&response).expect("Invalid JSON response");
    // Non-string arguments return a JSON-RPC error
    assert!(
        parsed.get("error").is_some(),
        "Expected JSON-RPC error for non-string arguments, got: {}",
        &response[..response.len().min(2000)]
    );
}

// ─── TEXT_DIFF_EXPLAIN: detail levels ────────────────────────────────

#[test]
fn test_mcp_text_diff_explain_summary_detail() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "hello", "b": "jello", "detail": "summary"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(false)));
}

#[test]
fn test_mcp_text_diff_explain_full_detail() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "abc", "b": "axc", "detail": "full"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(!diffs.is_empty(), "Expected diffs for differing strings");
}

// ─── TEXT_DIFF_EXPLAIN: summary and metrics fields ───────────────────

#[test]
fn test_mcp_text_diff_explain_summary_fields() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "abc", "b": "axc"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let summary = inner.get("summary").unwrap();
    assert!(
        summary
            .get("edit_distance")
            .and_then(|v| v.as_u64())
            .is_some(),
        "Expected edit_distance in summary"
    );
    assert!(
        summary
            .get("common_prefix_len")
            .and_then(|v| v.as_u64())
            .is_some(),
        "Expected common_prefix_len in summary"
    );
    assert!(
        summary
            .get("common_suffix_len")
            .and_then(|v| v.as_u64())
            .is_some(),
        "Expected common_suffix_len in summary"
    );
    let a_metrics = inner.get("a_metrics").unwrap();
    assert!(
        a_metrics
            .get("codepoints")
            .and_then(|v| v.as_u64())
            .is_some(),
        "Expected codepoints in a_metrics"
    );
}

// ─── TEXT_DIFF_EXPLAIN: agent_instruction field ──────────────────────

#[test]
fn test_mcp_text_diff_explain_agent_instruction() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": "hello", "b": "jello"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert!(
        inner
            .get("agent_instruction")
            .and_then(|v| v.as_str())
            .is_some(),
        "Expected agent_instruction string"
    );
}

// ─── TEXT_DIFF_EXPLAIN: large strings ────────────────────────────────

#[test]
fn test_mcp_text_diff_explain_large_strings() {
    let a = "x".repeat(10_000);
    let b = "x".repeat(9_999) + "y";
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_diff_explain", "arguments": {"a": a, "b": b}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let diffs = inner.get("diffs").unwrap().as_array().unwrap();
    assert!(
        diffs
            .iter()
            .any(|d| d.get("kind").and_then(|k| k.as_str()) != Some("equal")),
        "Expected a non-equal diff for last char diff in large strings, got: {:?}",
        diffs
    );
}

// ─── TEXT_FINGERPRINT: basic fingerprinting ───────────────────────────

#[test]
fn test_mcp_text_fingerprint_basic() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": "hello world"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert!(
        inner.get("sha256").and_then(|s| s.as_str()).is_some(),
        "Expected sha256 hash in fingerprint"
    );
}

#[test]
fn test_mcp_text_fingerprint_identical_inputs() {
    let r1 = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": "test input"}},
            "id": 1
        })
        .to_string(),
    );
    let r2 = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": "test input"}},
            "id": 2
        })
        .to_string(),
    );
    let h1 = r1["result"]["sha256"].as_str().unwrap();
    let h2 = r2["result"]["sha256"].as_str().unwrap();
    assert_eq!(
        h1, h2,
        "Identical inputs should produce identical fingerprints"
    );
}

#[test]
fn test_mcp_text_fingerprint_different_inputs() {
    let r1 = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": "input one"}},
            "id": 1
        })
        .to_string(),
    );
    let r2 = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": "input two"}},
            "id": 2
        })
        .to_string(),
    );
    let h1 = r1["result"]["sha256"].as_str().unwrap();
    let h2 = r2["result"]["sha256"].as_str().unwrap();
    assert_ne!(
        h1, h2,
        "Different inputs should produce different fingerprints"
    );
}

#[test]
fn test_mcp_text_fingerprint_empty_text() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": ""}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    assert!(
        result["result"]["sha256"].as_str().is_some(),
        "Expected sha256 for empty text"
    );
}

#[test]
fn test_mcp_text_fingerprint_unicode_text() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": "café résumé"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    assert!(
        result["result"]["sha256"].as_str().is_some(),
        "Expected sha256 for Unicode text"
    );
}

#[test]
fn test_mcp_text_fingerprint_input_too_large() {
    let long_text = "a".repeat(100_001);
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "text_fingerprint", "arguments": {"text": long_text}},
            "id": 1
        })
        .to_string(),
    );
    // Should return an error envelope with ok: false and error_type: "input_too_large"
    assert_eq!(result.get("ok"), Some(&Value::Bool(false)));
    assert_eq!(
        result.get("error_type").and_then(|c| c.as_str()),
        Some("input_too_large")
    );
}

// ─── LIST_COMPARE: set mode duplicates ───────────────────────────────

fn call_tool(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
        "id": 1
    })
    .to_string();
    call_tool_and_get_result(&request)
}

// ─── BUG-008: text_fingerprint MAX_TEXT_LENGTH ─────────────────────────

#[test]
fn test_mcp_text_fingerprint_input_too_large_legacy() {
    let long_text = "a".repeat(100_001);
    let result = call_tool("text_fingerprint", serde_json::json!({"text": long_text}));
    // Should return an error envelope with ok: false and error_type: "input_too_large"
    assert_eq!(result.get("ok"), Some(&Value::Bool(false)));
    assert_eq!(
        result.get("error_type").and_then(|c| c.as_str()),
        Some("input_too_large")
    );
}

// ─── BUG-009: list_compare set mode duplicates ─────────────────────────

#[test]
fn test_mcp_list_compare_set_mode_duplicates() {
    let result = call_tool(
        "list_compare",
        serde_json::json!({
            "a": ["apple", "banana", "apple", "cherry"],
            "b": ["banana", "cherry", "date"],
            "mode": "set"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    // "apple" appears twice in a, so it should be in duplicates_in_a
    let dup_a = inner.get("duplicates_in_a").unwrap().as_array().unwrap();
    assert!(
        dup_a.iter().any(|d| d.as_str() == Some("apple")),
        "Expected 'apple' in duplicates_in_a, got: {:?}",
        dup_a
    );
    // b has no duplicates
    let dup_b = inner.get("duplicates_in_b").unwrap().as_array().unwrap();
    assert!(
        dup_b.is_empty(),
        "Expected empty duplicates_in_b, got: {:?}",
        dup_b
    );
}

#[test]
fn test_mcp_list_compare_set_mode_duplicates_both() {
    let result = call_tool(
        "list_compare",
        serde_json::json!({
            "a": ["x", "x", "y"],
            "b": ["x", "y", "y"],
            "mode": "set"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let dup_a = inner.get("duplicates_in_a").unwrap().as_array().unwrap();
    assert!(
        dup_a.iter().any(|d| d.as_str() == Some("x")),
        "Expected 'x' in duplicates_in_a, got: {:?}",
        dup_a
    );
    let dup_b = inner.get("duplicates_in_b").unwrap().as_array().unwrap();
    assert!(
        dup_b.iter().any(|d| d.as_str() == Some("y")),
        "Expected 'y' in duplicates_in_b, got: {:?}",
        dup_b
    );
}
