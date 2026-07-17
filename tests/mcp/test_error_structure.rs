use eggsact::tools::{list_compare, list_dedupe, list_sort};
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
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.write_all(request.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    stdout.lines().last().unwrap_or("").to_string()
}

fn call_tool_raw(request: &str) -> Value {
    let response_str = mcp_request(request);
    serde_json::from_str(&response_str).expect("Failed to parse JSON-RPC response")
}

fn call_tool_full_jsonrpc(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
        "id": 1
    })
    .to_string();
    call_tool_raw(&request)
}

/// Check if a response is a JSON-RPC level error (code in "error" key)
fn is_jsonrpc_error(response: &Value) -> bool {
    response.get("error").is_some()
}

/// Check if a response is a tool-level error (ok: false in content)
fn is_tool_error(response: &Value) -> bool {
    if let Some(content) = response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
    {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    if let Some(ok) = parsed.get("ok") {
                        return ok.as_bool() == Some(false);
                    }
                }
            }
        }
    }
    false
}

/// Get error message from tool-level error
fn get_tool_error_msg(response: &Value) -> String {
    if let Some(content) = response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
    {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    if let Some(err) = parsed.get("error").and_then(|e| e.as_str()) {
                        return err.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

#[test]
fn test_missing_required_param_error() {
    // math_eval missing expression - JSON-RPC level error from schema validation
    let r = call_tool_full_jsonrpc("math_eval", serde_json::json!({}));
    assert!(
        is_jsonrpc_error(&r),
        "missing expression should return JSON-RPC error, got: {}",
        r
    );
    let msg = r["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("expression") || msg.contains("missing"),
        "error message should mention the missing field: {}",
        msg
    );

    // text_measure missing text
    let r = call_tool_full_jsonrpc("text_measure", serde_json::json!({}));
    assert!(
        is_jsonrpc_error(&r),
        "missing text should return JSON-RPC error, got: {}",
        r
    );

    // text_equal missing a
    let r = call_tool_full_jsonrpc("text_equal", serde_json::json!({"b": "hello"}));
    assert!(
        is_jsonrpc_error(&r),
        "missing 'a' should return JSON-RPC error, got: {}",
        r
    );

    // text_equal missing b
    let r = call_tool_full_jsonrpc("text_equal", serde_json::json!({"a": "hello"}));
    assert!(
        is_jsonrpc_error(&r),
        "missing 'b' should return JSON-RPC error, got: {}",
        r
    );

    // list_compare missing a
    let r = call_tool_full_jsonrpc("list_compare", serde_json::json!({"b": [1]}));
    assert!(
        is_jsonrpc_error(&r),
        "missing 'a' should return JSON-RPC error, got: {}",
        r
    );

    // validate_json missing text
    let r = call_tool_full_jsonrpc("validate_json", serde_json::json!({}));
    assert!(
        is_jsonrpc_error(&r),
        "missing 'text' should return JSON-RPC error, got: {}",
        r
    );
}

#[test]
fn test_wrong_type_param_error() {
    // math_eval expression with wrong type (integer instead of string)
    let r = call_tool_full_jsonrpc("math_eval", serde_json::json!({"expression": 42}));
    assert!(
        is_jsonrpc_error(&r),
        "wrong type should return JSON-RPC error, got: {}",
        r
    );
    let msg = r["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("expected") || msg.contains("string") || msg.contains("type"),
        "error message should mention type mismatch: {}",
        msg
    );

    // text_equal with non-string a
    let r = call_tool_full_jsonrpc("text_equal", serde_json::json!({"a": 123, "b": "hello"}));
    assert!(
        is_jsonrpc_error(&r),
        "non-string param should return error, got: {}",
        r
    );

    // list_compare with non-array a
    let r = call_tool_full_jsonrpc(
        "list_compare",
        serde_json::json!({"a": "not an array", "b": [1, 2]}),
    );
    assert!(
        is_jsonrpc_error(&r),
        "non-array param should return error, got: {}",
        r
    );

    // unit_convert with non-number value
    let r = call_tool_full_jsonrpc(
        "unit_convert",
        serde_json::json!({"value": "not a number", "from_unit": "km", "to_unit": "m"}),
    );
    assert!(
        is_jsonrpc_error(&r),
        "non-number value should return error, got: {}",
        r
    );
}

#[test]
fn test_list_tools_return_structured_errors_without_schema_preflight() {
    let cases = [
        (
            "list_compare missing b",
            list_compare(&serde_json::json!({"a": []})),
        ),
        (
            "list_compare wrong a",
            list_compare(&serde_json::json!({"a": "not an array", "b": []})),
        ),
        (
            "list_dedupe wrong items",
            list_dedupe(&serde_json::json!({"items": "not an array"})),
        ),
        (
            "list_sort wrong items",
            list_sort(&serde_json::json!({"items": "not an array"})),
        ),
    ];

    for (label, response) in cases {
        assert!(!response.ok, "{} should fail", label);
        assert_eq!(response.error_type.as_deref(), Some("invalid_arguments"));
        assert!(
            response.error.as_deref().unwrap_or("").contains("list"),
            "{} should explain the expected list type: {:?}",
            label,
            response.error
        );
    }
}

#[test]
fn test_input_too_large_error() {
    let oversized = "a".repeat(100_001);

    // text_measure with oversized input - tool-level error
    let r = call_tool_full_jsonrpc("text_measure", serde_json::json!({"text": &oversized}));
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(
        is_err,
        "oversized input should produce an error, got: {}",
        r
    );

    // text_inspect with oversized input
    let r = call_tool_full_jsonrpc("text_inspect", serde_json::json!({"text": &oversized}));
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(
        is_err,
        "oversized input should produce an error, got: {}",
        r
    );

    // text_count with oversized input
    let r = call_tool_full_jsonrpc("text_count", serde_json::json!({"text": &oversized}));
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(
        is_err,
        "oversized input should produce an error, got: {}",
        r
    );
}

#[test]
fn test_invalid_enum_value_error() {
    // text_equal with invalid normalization
    let r = call_tool_full_jsonrpc(
        "text_equal",
        serde_json::json!({
            "a": "hello", "b": "hello", "normalization": "INVALID"
        }),
    );
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(
        is_err,
        "invalid normalization should return error, got: {}",
        r
    );

    // text_measure with invalid detail
    let r = call_tool_full_jsonrpc(
        "text_measure",
        serde_json::json!({
            "text": "hello", "detail": "super_detailed"
        }),
    );
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(
        is_err,
        "invalid detail level should return error, got: {}",
        r
    );

    // list_compare with invalid mode
    let r = call_tool_full_jsonrpc(
        "list_compare",
        serde_json::json!({
            "a": ["x"], "b": ["y"], "mode": "fuzzy"
        }),
    );
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(is_err, "invalid mode should return error, got: {}", r);
}

#[test]
fn test_unknown_tool_error() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "nonexistent_tool", "arguments": {}},
        "id": 1
    })
    .to_string();
    let r = call_tool_raw(&request);

    assert!(
        is_jsonrpc_error(&r),
        "unknown tool should return JSON-RPC error, got: {}",
        r
    );
    let code = r["error"]["code"].as_i64().unwrap_or(0);
    assert!(
        code == -32601 || code == -32602,
        "unknown tool error code should be -32601 or -32602, got {}",
        code
    );

    let msg = r["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("nonexistent_tool"),
        "error message should mention the tool name: {}",
        msg
    );
}

#[test]
fn test_error_has_tool_field() {
    // Tool-level errors should include the tool name
    let r = call_tool_full_jsonrpc(
        "unit_convert",
        serde_json::json!({
            "value": 1.0,
            "from_unit": "not_a_unit",
            "to_unit": "m"
        }),
    );
    let is_err = is_jsonrpc_error(&r) || is_tool_error(&r);
    assert!(is_err, "invalid unit should return error, got: {}", r);

    // Check tool field in tool-level error response
    if is_tool_error(&r) {
        let tool_field = get_tool_error_field(&r);
        assert!(
            tool_field.contains("unit_convert"),
            "error response should include tool name: {}",
            tool_field
        );
    }

    // constant_lookup with unknown constant - tool-level error
    let r = call_tool_full_jsonrpc(
        "constant_lookup",
        serde_json::json!({
            "name": "nonexistent_constant_xyz"
        }),
    );
    assert!(
        is_tool_error(&r),
        "unknown constant should return tool error, got: {}",
        r
    );
    let err_msg = get_tool_error_msg(&r);
    assert!(!err_msg.is_empty(), "error should have an error message");
}

fn get_tool_error_field(response: &Value) -> String {
    if let Some(content) = response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
    {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    if let Some(tool) = parsed.get("tool").and_then(|t| t.as_str()) {
                        return tool.to_string();
                    }
                }
            }
        }
    }
    String::new()
}
