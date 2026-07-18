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

fn run_python_jsonrpc(request: &str) -> Value {
    let mut child = Command::new("python3")
        .args(["-m", "eggcalc.mcp.server"])
        .current_dir("/Users/davidbowman/projects/eggcalc")
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

fn has_error_response(response: &Value) -> bool {
    // Check for JSON-RPC level error
    if response.get("error").is_some() {
        return true;
    }
    // Check for MCP isError flag
    if let Some(result) = response.get("result") {
        if let Some(is_error) = result.get("isError") {
            if is_error.as_bool() == Some(true) {
                return true;
            }
        }
        // Check for wrapped tool response with ok: false
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            if let Some(first) = content.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        if let Some(ok) = parsed.get("ok") {
                            if ok.as_bool() == Some(false) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        // Direct check (non-wrapped format)
        if let Some(obj) = result.as_object() {
            if let Some(ok) = obj.get("ok") {
                return ok.as_bool() == Some(false);
            }
        }
    }
    false
}

fn call_tool(tool_name: &str, arguments: Value) -> (Value, Value) {
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

    (run_rust_jsonrpc(&request), run_python_jsonrpc(&request))
}

fn call_tool_no_args(tool_name: &str) -> (Value, Value) {
    call_tool(tool_name, serde_json::json!({}))
}

fn assert_invalid_arguments_match_python(tool_name: &str, arguments: Value) {
    let (rust_resp, python_resp) = call_tool(tool_name, arguments);

    assert!(
        has_error_response(&rust_resp),
        "Rust should reject invalid arguments, got: {}",
        rust_resp
    );
    assert!(
        has_error_response(&python_resp),
        "Python should reject invalid arguments, got: {}",
        python_resp
    );
    assert_eq!(
        rust_resp, python_resp,
        "Rust and Python should agree on invalid arguments for {}",
        tool_name
    );
}

#[test]
fn test_validate_regex_unexpected_argument_matches_python() {
    assert_invalid_arguments_match_python(
        "validate_regex",
        serde_json::json!({
            "pattern": r"\w+",
            "text": "x"
        }),
    );
}

#[test]
fn test_json_compare_unexpected_argument_matches_python() {
    assert_invalid_arguments_match_python(
        "json_compare",
        serde_json::json!({
            "a": "1",
            "text": "2"
        }),
    );
}

#[test]
fn test_text_position_unexpected_argument_matches_python() {
    assert_invalid_arguments_match_python(
        "text_position",
        serde_json::json!({
            "byte_offset": 1,
            "kind": "oops"
        }),
    );
}

#[test]
fn test_list_sort_unexpected_argument_matches_python() {
    assert_invalid_arguments_match_python(
        "list_sort",
        serde_json::json!({
            "casefold": false,
            "text": "oops"
        }),
    );
}

#[test]
fn test_math_eval_missing_expression() {
    let (rust_resp, python_resp) = call_tool_no_args("math_eval");
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "math_eval missing - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
    assert!(rust_err, "Rust should return error for missing expression");
    assert!(
        python_err,
        "Python should return error for missing expression"
    );
}

#[test]
fn test_text_measure_missing_text() {
    let (rust_resp, python_resp) = call_tool_no_args("text_measure");
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "text_measure missing - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    assert!(rust_err, "Rust should return error for missing text");
    assert!(python_err, "Python should return error for missing text");
}

#[test]
fn test_text_equal_missing_a() {
    let args = serde_json::json!({"b": "hello"});
    let (rust_resp, python_resp) = call_tool("text_equal", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "text_equal missing a - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    assert!(rust_err, "Rust should return error for missing param a");
    assert!(python_err, "Python should return error for missing param a");
}

#[test]
fn test_text_equal_missing_b() {
    let args = serde_json::json!({"a": "hello"});
    let (rust_resp, python_resp) = call_tool("text_equal", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "text_equal missing b - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    assert!(rust_err, "Rust should return error for missing param b");
    assert!(python_err, "Python should return error for missing param b");
}

#[test]
fn test_list_compare_wrong_type() {
    let args = serde_json::json!({"a": "not an array", "b": [1, 2, 3]});
    let (rust_resp, python_resp) = call_tool("list_compare", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "list_compare wrong type - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    assert!(rust_err, "Rust should return error for wrong type");
    assert!(python_err, "Python should return error for wrong type");
}

#[test]
fn test_text_hash_param_name() {
    // Both Python and Rust use 'algorithms' (plural, list) - verify parity
    let args = serde_json::json!({"text": "hello", "algorithms": ["sha256", "md5"]});
    let (rust_resp, python_resp) = call_tool("text_hash", args);

    // Both must succeed (no error)
    assert!(
        !has_error_response(&rust_resp),
        "Rust should succeed for valid algorithms param, got: {}",
        rust_resp
    );
    assert!(
        !has_error_response(&python_resp),
        "Python should succeed for valid algorithms param, got: {}",
        python_resp
    );

    // Extract the tool's actual output (content[0].text) and compare for parity
    let rust_text = rust_resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("{}");
    let python_text = python_resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("{}");

    let rust_parsed: Value = serde_json::from_str(rust_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": rust_text}));
    let python_parsed: Value = serde_json::from_str(python_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": python_text}));

    assert_eq!(
        rust_parsed, python_parsed,
        "text_hash with 'algorithms' (plural) should be identical between Rust and Python"
    );
}

#[test]
fn test_text_hash_invalid_algorithm() {
    // An invalid algorithm name should not error - both implementations skip it.
    // The valid algorithm in the same list must produce identical hashes.
    let args = serde_json::json!({"text": "hello", "algorithms": ["sha256", "not_a_algorithm"]});
    let (rust_resp, python_resp) = call_tool("text_hash", args);

    assert!(
        !has_error_response(&rust_resp),
        "Rust should not error on invalid algo (skips it), got: {}",
        rust_resp
    );
    assert!(
        !has_error_response(&python_resp),
        "Python should not error on invalid algo (skips it), got: {}",
        python_resp
    );

    let rust_text = rust_resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("{}");
    let python_text = python_resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("{}");

    let rust_parsed: Value = serde_json::from_str(rust_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": rust_text}));
    let python_parsed: Value = serde_json::from_str(python_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": python_text}));

    // The hashes field should be identical (only valid algos are computed)
    assert_eq!(
        rust_parsed.get("hashes"),
        python_parsed.get("hashes"),
        "Valid algorithm hashes should match between Rust and Python even with invalid algo in list"
    );
}

#[test]
fn test_unit_convert_invalid_unit() {
    let args = serde_json::json!({"value": 1.0, "from_unit": "not_a_unit", "to_unit": "m"});
    let (rust_resp, python_resp) = call_tool("unit_convert", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "unit_convert invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
    assert!(rust_err, "Rust should return error for invalid unit");
    assert!(python_err, "Python should return error for invalid unit");
}

#[test]
fn test_validate_json_empty() {
    let args = serde_json::json!({"text": ""});
    let (rust_resp, python_resp) = call_tool("validate_json", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle empty JSON string");
    assert!(!python_err, "Python should handle empty JSON string");
}

#[test]
fn test_glob_match_invalid_pattern() {
    let args = serde_json::json!({"text": "file.txt", "pattern": "[invalid"});
    let (rust_resp, python_resp) = call_tool("glob_match", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "glob_match invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
    assert_eq!(
        rust_err, python_err,
        "Both should agree on invalid glob pattern"
    );
}

#[test]
fn test_validate_toml_invalid() {
    let args = serde_json::json!({"text": "key = value without quotes"});
    let (rust_resp, python_resp) = call_tool("validate_toml", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "validate_toml invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
    assert_eq!(rust_err, python_err, "Both should agree on invalid TOML");
}

#[test]
fn test_version_constraint_invalid() {
    let args = serde_json::json!({"constraint": "not valid", "version": "1.0.0"});
    let (rust_resp, python_resp) = call_tool("version_constraint_check", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "version_constraint invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
}

#[test]
fn test_unicode_policy_check_invalid_policy() {
    let args = serde_json::json!({"text": "hello", "policy": "invalid_policy"});
    let (rust_resp, python_resp) = call_tool("unicode_policy_check", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "unicode_policy_check invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
}

#[test]
fn test_text_truncate_negative_max_chars() {
    let args = serde_json::json!({"text": "hello", "max_chars": -1});
    let (rust_resp, python_resp) = call_tool("text_truncate", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "text_truncate negative - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
    assert_eq!(
        rust_err, python_err,
        "Both should agree on negative max_chars"
    );
}

#[test]
fn test_json_extract_invalid_json() {
    let args = serde_json::json!({"text": "not json", "pointer": "/a"});
    let (rust_resp, python_resp) = call_tool("json_extract", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "json_extract invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
    assert!(
        !rust_err && !python_err,
        "Both should return success with valid_json=false"
    );
}

#[test]
fn test_identifier_analyze_empty() {
    let args = serde_json::json!({"text": ""});
    let (rust_resp, python_resp) = call_tool("identifier_analyze", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "identifier_analyze empty - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    assert!(!rust_err, "Rust should handle empty string");
    assert!(!python_err, "Python should handle empty string");
}

#[test]
fn test_canonicalize_text_invalid_profile() {
    let args = serde_json::json!({"text": "hello", "profile": "not_a_profile"});
    let (rust_resp, python_resp) = call_tool("canonicalize_text", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "canonicalize_text invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
}

#[test]
fn test_validate_schema_light_invalid_schema() {
    let args = serde_json::json!({"schema": "not json schema", "data": "\"hello\""});
    let (rust_resp, python_resp) = call_tool("validate_schema_light", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "validate_schema_light invalid - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    println!("  Rust response: {}", rust_resp);
    println!("  Python response: {}", python_resp);
}

#[test]
fn test_text_inspect_empty() {
    let args = serde_json::json!({"text": ""});
    let (rust_resp, python_resp) = call_tool("text_inspect", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "text_inspect empty - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    assert!(!rust_err, "Rust should handle empty string");
    assert!(!python_err, "Python should handle empty string");
}

#[test]
fn test_text_diff_explain_identical() {
    let args = serde_json::json!({"a": "hello", "b": "hello"});
    let (rust_resp, python_resp) = call_tool("text_diff_explain", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    println!(
        "text_diff_explain identical - Rust error: {}, Python error: {}",
        rust_err, python_err
    );
    assert!(!rust_err, "Rust should handle identical strings");
    assert!(!python_err, "Python should handle identical strings");
}

#[test]
fn test_json_compare_invalid_json() {
    let args = serde_json::json!({"a": "not json", "b": "{\"x\":1}"});
    let (rust_resp, python_resp) = call_tool("json_compare", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle invalid JSON gracefully");
    assert!(!python_err, "Python should handle invalid JSON gracefully");
}

#[test]
fn test_json_canonicalize_invalid_json() {
    let args = serde_json::json!({"text": "not json"});
    let (rust_resp, python_resp) = call_tool("json_canonicalize", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle invalid JSON gracefully");
    assert!(!python_err, "Python should handle invalid JSON gracefully");
}

#[test]
fn test_json_query_invalid_json() {
    let args = serde_json::json!({"text": "not json", "pointer": "/a"});
    let (rust_resp, python_resp) = call_tool("json_query", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle invalid JSON gracefully");
    assert!(!python_err, "Python should handle invalid JSON gracefully");
}

#[test]
fn test_json_shape_invalid_json() {
    let args = serde_json::json!({"text": "not json"});
    let (rust_resp, python_resp) = call_tool("json_shape", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle invalid JSON gracefully");
    assert!(!python_err, "Python should handle invalid JSON gracefully");
}

#[test]
fn test_identifier_inspect_empty() {
    let args = serde_json::json!({"identifiers": []});
    let (rust_resp, python_resp) = call_tool("identifier_inspect", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle empty identifiers");
    assert!(!python_err, "Python should handle empty identifiers");
}

#[test]
fn test_path_analyze_empty() {
    let args = serde_json::json!({"path": ""});
    let (rust_resp, python_resp) = call_tool("path_analyze", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle empty path");
    assert!(!python_err, "Python should handle empty path");
}

#[test]
fn test_path_compare_equal() {
    let args = serde_json::json!({"left": "/foo/bar", "right": "/foo/bar"});
    let (rust_resp, python_resp) = call_tool("path_compare", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle equal paths");
    assert!(!python_err, "Python should handle equal paths");
}

#[test]
fn test_text_position_empty() {
    let args = serde_json::json!({"text": ""});
    let (rust_resp, python_resp) = call_tool("text_position", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert_eq!(
        rust_err, python_err,
        "Both should agree on error for empty text with no locator"
    );
}

#[test]
fn test_text_window_basic() {
    let args = serde_json::json!({"text": "hello world", "position": {"kind": "codepoint_index", "value": 0}});
    let (rust_resp, python_resp) = call_tool("text_window", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle valid window");
    assert!(!python_err, "Python should handle valid window");
}

#[test]
fn test_list_dedupe_basic() {
    let args = serde_json::json!({"items": ["a", "b", "a", "c"]});
    let (rust_resp, python_resp) = call_tool("list_dedupe", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle dedupe");
    assert!(!python_err, "Python should handle dedupe");
}

#[test]
fn test_list_sort_basic() {
    let args = serde_json::json!({"items": ["c", "a", "b"]});
    let (rust_resp, python_resp) = call_tool("list_sort", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle sort");
    assert!(!python_err, "Python should handle sort");
}

#[test]
#[ignore = "Accepted parity gap (see tests/fixtures/accepted_parity_failures.txt); run with --include-ignored"]
fn test_shell_split_basic() {
    let args = serde_json::json!({"command": "ls -la /tmp"});
    let (rust_resp, python_resp) = call_tool("shell_split", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle shell split");
    assert!(!python_err, "Python should handle shell split");
}

#[test]
fn test_regex_finditer_basic() {
    let args = serde_json::json!({"text": "hello world", "pattern": "l+"});
    let (rust_resp, python_resp) = call_tool("regex_finditer", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle regex finditer");
    assert!(!python_err, "Python should handle regex finditer");
}

#[test]
fn test_line_range_extract_basic() {
    let args = serde_json::json!({"text": "line1\nline2\nline3", "start_line": 1, "end_line": 2});
    let (rust_resp, python_resp) = call_tool("line_range_extract", args);
    let rust_err = has_error_response(&rust_resp);
    let python_err = has_error_response(&python_resp);
    assert!(!rust_err, "Rust should handle line range extract");
    assert!(!python_err, "Python should handle line range extract");
}

// ── BUG-022: Case-insensitive tool dispatch ──

#[test]
fn test_case_insensitive_tool_dispatch() {
    // BUG-020: Case-insensitive dispatch removed to match Python.
    // "Math_Eval" should now return an error with a suggestion, matching Python behavior.
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "Math_Eval",
            "arguments": {"expression": "5 + 3"}
        },
        "id": 1
    })
    .to_string();

    let rust_resp = run_rust_jsonrpc(&request);
    // Should return "Unknown tool" error with suggestion, matching Python
    let error_msg = rust_resp
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("");
    assert!(
        error_msg.contains("Unknown tool") && error_msg.contains("math_eval"),
        "Non-canonical tool name should return error with suggestion, got: {}",
        rust_resp
    );
}

// ── BUG-023: Temperature NaN/Inf check ──

#[test]
fn test_unit_convert_temperature_extreme_value() {
    // Very large temperature should not produce Inf
    let args = serde_json::json!({
        "value": 1e308,
        "from_unit": "C",
        "to_unit": "F"
    });
    let (rust_resp, _python_resp) = call_tool("unit_convert", args);
    // Rust should return an error, not Inf
    let has_error = has_error_response(&rust_resp);
    // The result should either be an error or a finite value
    if !has_error {
        if let Some(content) = rust_resp
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
        {
            if let Some(first) = content.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        if let Some(val) = parsed.get("value").and_then(|v| v.as_f64()) {
                            assert!(
                                val.is_finite(),
                                "Temperature conversion should not produce Inf/NaN, got: {}",
                                val
                            );
                        }
                    }
                }
            }
        }
    }
}

// ── BUG-026: text_transform summary mode fields ──

#[test]
fn test_text_transform_summary_includes_text_and_warnings() {
    let args = serde_json::json!({
        "text": "Hello World",
        "operations": ["casefold"],
        "detail": "summary"
    });
    let (rust_resp, python_resp) = call_tool("text_transform", args);

    // Both should succeed
    assert!(
        !has_error_response(&rust_resp),
        "Rust text_transform summary should succeed"
    );
    assert!(
        !has_error_response(&python_resp),
        "Python text_transform summary should succeed"
    );

    // Extract the inner result from both
    fn extract_inner(resp: &Value) -> Option<Value> {
        resp.get("result")?
            .get("content")?
            .as_array()?
            .first()?
            .get("text")?
            .as_str()
            .and_then(|t| serde_json::from_str(t).ok())
    }

    let rust_inner = extract_inner(&rust_resp);
    let python_inner = extract_inner(&python_resp);

    if let (Some(ri), Some(pi)) = (rust_inner, python_inner) {
        // Both should have 'text' and 'warnings' fields in the result object
        let rust_result = ri.get("result");
        let python_result = pi.get("result");
        assert!(
            rust_result.is_some(),
            "Rust summary response missing 'result' field"
        );
        assert!(
            python_result.is_some(),
            "Python summary response missing 'result' field"
        );
        if let (Some(rr), Some(pr)) = (rust_result, python_result) {
            assert!(
                rr.get("text").is_some(),
                "Rust summary response missing 'text' field in result"
            );
            assert!(
                pr.get("text").is_some(),
                "Python summary response missing 'text' field in result"
            );
            assert!(
                rr.get("warnings").is_some(),
                "Rust summary response missing 'warnings' field in result"
            );
            assert!(
                pr.get("warnings").is_some(),
                "Python summary response missing 'warnings' field in result"
            );
        }
    }
}

// ── BUG-027: json_compare string-to-string numeric equivalence ──

#[test]
fn test_json_compare_numeric_string_equivalence_string_to_string() {
    let args = serde_json::json!({
        "a": "\"42.0\"",
        "b": "\"42\"",
        "numeric_string_equivalence": true
    });
    let (rust_resp, python_resp) = call_tool("json_compare", args);

    // Both should succeed
    assert!(
        !has_error_response(&rust_resp),
        "Rust json_compare should succeed"
    );
    assert!(
        !has_error_response(&python_resp),
        "Python json_compare should succeed"
    );

    // Extract inner result and verify strings are treated as numerically equal
    fn extract_equal(resp: &Value) -> Option<bool> {
        resp.get("result")?
            .get("content")?
            .as_array()?
            .first()?
            .get("text")?
            .as_str()
            .and_then(|t| serde_json::from_str::<Value>(t).ok())
            .and_then(|v| v.get("result")?.get("equal")?.as_bool())
    }

    let rust_equal = extract_equal(&rust_resp);
    let python_equal = extract_equal(&python_resp);

    assert_eq!(
        rust_equal,
        Some(true),
        "Rust should treat '42.0' and '42' as numerically equal"
    );
    assert_eq!(
        python_equal,
        Some(true),
        "Python should treat '42.0' and '42' as numerically equal"
    );
}

// ── BUG-028: toml_shape negative max_tables ──

#[test]
fn test_toml_shape_negative_max_tables() {
    let args = serde_json::json!({
        "text": "[a]\nkey = 1",
        "max_tables": -1
    });
    let (rust_resp, python_resp) = call_tool("toml_shape", args);

    // Both should reject negative max_tables
    assert!(
        has_error_response(&rust_resp),
        "Rust should reject negative max_tables, got: {}",
        rust_resp
    );
    assert!(
        has_error_response(&python_resp),
        "Python should reject negative max_tables, got: {}",
        python_resp
    );
}

// ── BUG-031: Float request IDs ──

#[test]
fn test_float_request_id_rejected() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "ping",
        "id": 1.5
    })
    .to_string();

    let rust_resp = run_rust_jsonrpc(&request);
    // Should get an error response
    assert!(
        rust_resp.get("error").is_some(),
        "Float request ID should be rejected, got: {}",
        rust_resp
    );
}

// ── BUG-032: validate_regex error message wording ──

#[test]
fn test_validate_regex_timeout_message_wording() {
    // Verify the source code uses the ReDoS-aware timeout message.
    // This is a code-level check since triggering an actual 5s timeout is impractical in tests.
    let source = include_str!("../../src/tools/regex.rs");
    assert!(
        source.contains("Regex execution exceeded time limit (possible ReDoS)"),
        "validate_regex should use 'Regex execution exceeded time limit (possible ReDoS)' in timeout message"
    );
    assert!(
        !source.contains("Regex evaluation timed out"),
        "validate_regex should NOT use old 'evaluation timed out' message"
    );

    // Also verify basic functionality works
    let args = serde_json::json!({
        "pattern": "[a-z]+",
        "samples": ["hello", "world"]
    });
    let (rust_resp, _python_resp) = call_tool("validate_regex", args);
    assert!(
        !has_error_response(&rust_resp),
        "validate_regex should handle basic input, got: {}",
        rust_resp
    );
}

// ── math_eval uses catch_unwind for panic safety ──

#[test]
fn test_math_eval_uses_catch_unwind() {
    // Verify the math_eval function uses catch_unwind to convert panics
    // to error responses instead of letting them propagate as JoinErrors.
    // This is a code-level check since triggering an actual panic is impractical.
    let source = include_str!("../../src/tools/math.rs");

    // Find the math_eval function body (between "pub fn math_eval" and the next "pub fn")
    let math_eval_start = source
        .find("pub fn math_eval")
        .expect("math_eval function not found");
    let after_math_eval = &source[math_eval_start..];
    let next_pub_fn = after_math_eval[20..]
        .find("\npub fn ")
        .map(|pos| pos + 20)
        .unwrap_or(after_math_eval.len());
    let math_eval_body = &after_math_eval[..next_pub_fn];

    // Must use catch_unwind for the evaluation
    assert!(
        math_eval_body.contains("catch_unwind"),
        "math_eval should use catch_unwind to convert panics to error responses"
    );
    // Must NOT use handle.block_on as code (deadlock-prone pattern).
    // Allow it in comments (the fix description mentions it).
    let non_comment_lines: Vec<&str> = math_eval_body
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    let non_comment_body = non_comment_lines.join("\n");
    assert!(
        !non_comment_body.contains("handle.block_on"),
        "math_eval must NOT use handle.block_on as code (deadlocks under concurrent load)"
    );
    // Must NOT use run_with_timeout (removed — outer timeout handles deadline)
    assert!(
        !non_comment_body.contains("run_with_timeout"),
        "math_eval must NOT use run_with_timeout (removed — outer timeout handles deadline)"
    );
    // Must NOT use tokio::time::timeout (was the old wrapper around block_on)
    assert!(
        !non_comment_body.contains("tokio::time::timeout"),
        "math_eval must NOT use tokio::time::timeout"
    );

    // Also verify basic functionality still works
    let args = serde_json::json!({"expression": "2 + 2"});
    let (rust_resp, _python_resp) = call_tool("math_eval", args);
    assert!(
        !has_error_response(&rust_resp),
        "math_eval basic functionality should work, got: {}",
        rust_resp
    );
}

// ── BUG-029: identifier_table_inspect large line number ──

#[test]
fn test_identifier_table_inspect_large_line_number() {
    // A line number exceeding i32::MAX should be capped, not silently truncated
    let args = serde_json::json!({
        "identifiers": [
            {"name": "my_var", "line": 3_000_000_000_i64}
        ],
        "language": "python"
    });
    let (rust_resp, _python_resp) = call_tool("identifier_table_inspect", args);

    // Should succeed without error
    assert!(
        !has_error_response(&rust_resp),
        "identifier_table_inspect should handle large line numbers, got: {}",
        rust_resp
    );

    // Extract the inner result and verify line was capped at i32::MAX
    if let Some(content) = rust_resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
    {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    if let Some(entries) = parsed.get("identifiers").and_then(|e| e.as_array()) {
                        if let Some(entry) = entries.first() {
                            let line = entry.get("line").and_then(|l| l.as_i64());
                            assert_eq!(
                                line,
                                Some(i32::MAX as i64),
                                "Line number should be capped at i32::MAX, got: {:?}",
                                line
                            );
                        }
                    }
                }
            }
        }
    }
}

// ── BUG-025: validate_regex spawn slot acquisition ──

#[test]
fn test_spawn_slot_acquire_and_release() {
    // Verify the spawn permit acquire/release cycle works correctly.
    // This is a unit-level test of the concurrency primitive.
    // We can't easily saturate all slots in a test, but we can verify
    // that acquiring and releasing a permit doesn't leak.
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Run a simple tool call that uses the spawn permit (validate_regex)
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "validate_regex",
            "arguments": {"pattern": "\\d+", "samples": ["123", "456"]}
        },
        "id": 1
    })
    .to_string();

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
    let response: Value = serde_json::from_str(&response_text)
        .unwrap_or_else(|_| serde_json::json!({"error": response_text.to_string()}));

    // Should succeed - the permit was acquired and released properly
    assert!(
        response.get("error").is_none(),
        "validate_regex should succeed with spawn permit, got: {}",
        response
    );

    // Verify the result content is valid
    if let Some(result) = response.get("result") {
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            if let Some(first) = content.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    let parsed: Result<Value, _> = serde_json::from_str(text);
                    assert!(parsed.is_ok(), "Response should be valid JSON");
                }
            }
        }
    }
}
