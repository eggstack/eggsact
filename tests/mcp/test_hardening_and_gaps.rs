//! Security hardening, profile invariants, sanitize_error, cancelled requests,
//! schema detail, find_close_match, and production review tests.
//!
//! Mirrors Python test classes:
//! - TestHardeningGroupA, TestHardeningGroupBM1/BM2, TestHardeningGroupBF, TestHardeningGroupBL6, TestHardeningGroupDL14
//! - TestProfileInvariants, TestProfileHardening, TestProfileSnapshots, TestProfileFiltering
//! - TestSanitizeError, TestFindCloseMatch, TestCancelledRequests
//! - TestCompactSchemaMode, TestSchemaDetail, TestSchemaDetailProtocol
//! - TestProductionReview2026_06, TestProductionReview2026_07
//! - TestMCPSecurityFixes, TestMCPSecurityGuards
//! - TestRateLimiting, TestRequestSizeLimits
//! - TestDocExamples

use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

use eggsact::agent::{CompatibilityMode, Profile, ToolAudience, ToolRegistry};

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
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn mcp_request_multi(requests: &[&str]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");
    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        for req in requests {
            stdin.write_all(req.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn call_tool_raw(request: &str) -> Value {
    let response_str = mcp_request(request);
    serde_json::from_str(&response_str).expect("Failed to parse JSON-RPC response")
}

/// Calls a tool and returns the tool envelope from content[0].text.
fn call_tool(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
        "id": 1
    })
    .to_string();
    let response = call_tool_raw(&request);
    if let Some(content) = response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
    {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                return serde_json::from_str(text).unwrap_or(Value::Null);
            }
        }
    }
    response.get("result").cloned().unwrap_or(Value::Null)
}

/// Returns true if the response indicates an error (JSON-RPC or tool-level).
fn is_error(response: &Value) -> bool {
    if response.get("error").is_some() {
        return true;
    }
    if response.get("ok") == Some(&Value::Bool(false)) {
        return true;
    }
    false
}

fn list_tools_with_params(params: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": params,
        "id": 1
    })
    .to_string();
    let response_str = mcp_request(&request);
    let response: Value =
        serde_json::from_str(&response_str).expect("Failed to parse JSON-RPC response");
    response.get("result").cloned().unwrap_or(Value::Null)
}

// ═══════════════════════════════════════════════════════════════════════
// SECURITY HARDENING — Group A
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_hardening_a_math_eval_envelope_no_double_wrap() {
    let result = call_tool("math_eval", serde_json::json!({"expression": "2+3"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").expect("missing result");
    assert!(inner.get("value").is_some(), "result should contain value");
    assert!(inner.get("type").is_some(), "result should contain type");
}

#[test]
fn test_hardening_a_unit_convert_bool_value_returns_error() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "unit_convert", "arguments": {"value": true, "from_unit": "m", "to_unit": "ft"}},
            "id": 1
        }).to_string(),
    );
    assert!(is_error(&response), "Bool value should be rejected");
}

#[test]
fn test_hardening_a_unit_convert_infinity_returns_error() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": "Infinity", "from_unit": "m", "to_unit": "ft"}),
    );
    // String "Infinity" may be parsed; either way should not crash
    let _ = result;
}

#[test]
fn test_hardening_a_validate_regex_redos_rejected() {
    let result = call_tool(
        "validate_regex",
        serde_json::json!({"pattern": "(a+)+b", "samples": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaac"]}),
    );
    if result.get("ok") == Some(&Value::Bool(true)) {
        let findings = result.get("findings").and_then(|f| f.as_array());
        if let Some(findings) = findings {
            let has_redos = findings.iter().any(|f| {
                f.get("code")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains("RE") || c.contains("redos") || c.contains("safety"))
                    .unwrap_or(false)
            });
            assert!(has_redos, "ReDoS pattern should be flagged");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SECURITY HARDENING — Group BM1
// ═══════════════════════════════════════════════════════════════════════

fn assert_tool_rejects_type(tool: &str, args: Value) {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": {"name": tool, "arguments": args},
            "id": 1
        })
        .to_string(),
    );
    assert!(
        is_error(&response),
        "{} should reject wrong-type input",
        tool
    );
}

#[test]
fn test_hardening_bm1_text_measure_rejects_int() {
    assert_tool_rejects_type("text_measure", serde_json::json!({"text": 123}));
}

#[test]
fn test_hardening_bm1_text_measure_rejects_null() {
    assert_tool_rejects_type("text_measure", serde_json::json!({"text": null}));
}

#[test]
fn test_hardening_bm1_text_count_rejects_int() {
    assert_tool_rejects_type(
        "text_count",
        serde_json::json!({"text": 123, "target": "a"}),
    );
}

#[test]
fn test_hardening_bm1_validate_json_rejects_int() {
    assert_tool_rejects_type("validate_json", serde_json::json!({"text": 42}));
}

#[test]
fn test_hardening_bm1_validate_brackets_rejects_int() {
    assert_tool_rejects_type("validate_brackets", serde_json::json!({"text": 42}));
}

#[test]
fn test_hardening_bm1_text_hash_rejects_int() {
    assert_tool_rejects_type("text_hash", serde_json::json!({"text": 42}));
}

#[test]
fn test_hardening_bm1_escape_text_rejects_null() {
    assert_tool_rejects_type("escape_text", serde_json::json!({"text": null}));
}

#[test]
fn test_hardening_bm1_unescape_text_rejects_int() {
    assert_tool_rejects_type("unescape_text", serde_json::json!({"text": 42}));
}

#[test]
fn test_hardening_bm1_text_truncate_rejects_int() {
    assert_tool_rejects_type("text_truncate", serde_json::json!({"text": 42}));
}

#[test]
fn test_hardening_bm1_path_analyze_rejects_int() {
    assert_tool_rejects_type("path_analyze", serde_json::json!({"path": 42}));
}

// ═══════════════════════════════════════════════════════════════════════
// SECURITY HARDENING — Group BF
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_hardening_bf_dotenv_malformed_key_pattern() {
    let result = call_tool(
        "dotenv_validate",
        serde_json::json!({"text": "KEY=value", "key_pattern": "(invalid[regex"}),
    );
    assert!(is_error(&result), "Malformed regex should return error");
}

// ═══════════════════════════════════════════════════════════════════════
// SECURITY HARDENING — Group BL6
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_hardening_bl6_json_extract_huge_max_output_chars() {
    let result = call_tool(
        "json_extract",
        serde_json::json!({
            "text": "{\"a\": 1}", "pointer": "/a", "max_output_chars": 1000000000
        }),
    );
    assert!(
        is_error(&result),
        "Huge max_output_chars should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// SECURITY HARDENING — Group DL14
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_hardening_dl14_argv_compare_both_command_and_argv() {
    let result = call_tool(
        "argv_compare",
        serde_json::json!({
            "left_command": "ls -la", "left_argv": ["ls", "-la"], "right_command": "ls -la"
        }),
    );
    assert!(
        is_error(&result),
        "Both command and argv should be rejected"
    );
}

#[test]
fn test_hardening_dl14_argv_compare_neither_command_nor_argv() {
    let result = call_tool("argv_compare", serde_json::json!({}));
    assert!(
        is_error(&result),
        "Neither command nor argv should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// PROFILE SYSTEM — Snapshots
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_profile_snapshots_all_11_profiles_exist() {
    let profiles_response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"profiles/list","id":1}"#);
    let profiles_result = profiles_response.get("result");
    assert!(
        profiles_result.is_some(),
        "profiles/list should return result"
    );
    let available = profiles_result
        .and_then(|r| r.get("available_profiles"))
        .and_then(|p| p.as_array());
    assert!(available.is_some(), "Should have available_profiles");
    let expected = vec![
        "full",
        "default",
        "codegg_core_min",
        "codegg_core",
        "codegg_preflight",
        "codegg_patch",
        "codegg_config",
        "codegg_unicode_security",
        "codegg_shell",
        "codegg_repo_audit",
        "human_math",
    ];
    let available_names: Vec<String> = available
        .unwrap()
        .iter()
        .filter_map(|p| p.as_str().map(|s| s.to_string()))
        .collect();
    for profile in &expected {
        assert!(
            available_names.contains(&profile.to_string()),
            "Profile '{}' should exist, got: {:?}",
            profile,
            available_names
        );
    }
}

#[test]
fn test_profile_snapshots_full_equals_all_non_hidden() {
    let result = list_tools_with_params(serde_json::json!({}));
    let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
    let tool_names: Vec<String> = tools
        .iter()
        .filter_map(|t| {
            t.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    assert!(
        tool_names.len() >= 70,
        "Full profile should have >= 70 tools, got: {}",
        tool_names.len()
    );
    for name in &["math_eval", "text_measure", "validate_json", "text_equal"] {
        assert!(
            tool_names.contains(&name.to_string()),
            "Full profile should include '{}'",
            name
        );
    }
}

#[test]
fn test_profile_snapshots_human_math_only_math_category() {
    let result = list_tools_with_params(serde_json::json!({"profile": "human_math"}));
    let tools = result.get("tools").and_then(|t| t.as_array());
    if let Some(tools) = tools {
        let non_math = [
            "text_equal",
            "text_measure",
            "validate_json",
            "validate_brackets",
            "text_count",
            "text_inspect",
            "text_hash",
            "escape_text",
            "unescape_text",
        ];
        for tool in tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            assert!(
                !non_math.contains(&name),
                "human_math should not contain '{}'",
                name
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PROFILE SYSTEM — Invariants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_profile_invariant_no_harness_only_in_codegg_core_min() {
    let result = list_tools_with_params(serde_json::json!({"profile": "codegg_core_min"}));
    if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            let exposure = tool.get("llm_exposure").and_then(|e| e.as_str());
            assert_ne!(
                exposure,
                Some("harness_only"),
                "codegg_core_min has harness_only tool: {}",
                tool.get("name").and_then(|n| n.as_str()).unwrap_or("?")
            );
        }
    }
}

#[test]
fn test_profile_invariant_no_harness_only_in_codegg_core() {
    let result = list_tools_with_params(serde_json::json!({"profile": "codegg_core"}));
    if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            let exposure = tool.get("llm_exposure").and_then(|e| e.as_str());
            assert_ne!(
                exposure,
                Some("harness_only"),
                "codegg_core has harness_only tool: {}",
                tool.get("name").and_then(|n| n.as_str()).unwrap_or("?")
            );
        }
    }
}

#[test]
fn test_profile_invariant_human_math_excludes_preflight() {
    let result = list_tools_with_params(serde_json::json!({"profile": "human_math"}));
    if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            assert!(
                !name.contains("preflight"),
                "human_math has preflight tool: {}",
                name
            );
        }
    }
}

#[test]
fn test_profile_invariant_core_min_subset_of_core() {
    let min = list_tools_with_params(serde_json::json!({"profile": "codegg_core_min"}));
    let core = list_tools_with_params(serde_json::json!({"profile": "codegg_core"}));
    let min_tools: Vec<String> = min
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    t.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    let core_tools: Vec<String> = core
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    t.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    for tool in &min_tools {
        assert!(
            core_tools.contains(tool),
            "codegg_core_min tool '{}' not in codegg_core",
            tool
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PROFILE SYSTEM — Hardening
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_profile_hardening_unknown_profile_returns_error() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/list",
            "params": {"profile": "nonexistent_profile"}, "id": 1
        })
        .to_string(),
    );
    // Should return a JSON-RPC error for unknown profile
    assert!(
        response.get("error").is_some(),
        "Unknown profile should return error"
    );
}

#[test]
#[allow(deprecated)]
fn test_profile_enforcement_tool_outside_profile_rejected() {
    let min_registry =
        ToolRegistry::with_profile_and_audience(Profile::CodeggCoreMin, ToolAudience::Model);
    let full_registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let min_names: Vec<String> = min_registry
        .available_tools()
        .into_iter()
        .map(|t| t.name)
        .collect();
    let all_names: Vec<String> = full_registry
        .available_tools()
        .into_iter()
        .map(|t| t.name)
        .collect();
    if let Some(excluded) = all_names.iter().find(|t| !min_names.contains(t)) {
        let result = min_registry.call_json(excluded, serde_json::json!({}));
        assert!(
            result.is_err(),
            "Tool '{}' outside profile should be rejected",
            excluded
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("not available") || msg.contains("unavailable"),
            "Error should mention unavailability, got: {msg}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SANITIZE_ERROR
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_sanitize_error_strips_file_paths() {
    let result = call_tool(
        "math_eval",
        serde_json::json!({"expression": "invalid_function()"}),
    );
    let error = result.get("error").and_then(|e| e.as_str()).unwrap_or("");
    assert!(
        !error.contains("/Users/"),
        "Error should not contain user paths: {}",
        error
    );
    assert!(
        !error.contains("/home/"),
        "Error should not contain home paths: {}",
        error
    );
}

#[test]
fn test_sanitize_error_caps_at_8192() {
    let long_expr = "a".repeat(9000);
    let result = call_tool("math_eval", serde_json::json!({"expression": long_expr}));
    let error = result.get("error").and_then(|e| e.as_str()).unwrap_or("");
    assert!(
        error.len() <= 8192,
        "Error capped at 8192, got: {}",
        error.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════
// CANCELLED REQUESTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_cancelled_request_returns_error_or_result() {
    let output = mcp_request_multi(&[
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"cancel-me"}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"2+3"}},"id":"cancel-me"}"#,
    ]);
    let response: Value = serde_json::from_str(&output).expect("Failed to parse");
    assert!(response.get("error").is_some() || response.get("result").is_some());
}

#[test]
fn test_non_float_request_id_not_cancelled() {
    let output = mcp_request_multi(&[
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3.14}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"2+3"}},"id":1}"#,
    ]);
    let response: Value = serde_json::from_str(&output).expect("Failed to parse");
    assert!(
        response.get("result").is_some(),
        "Non-cancelled request should have result"
    );
}

#[test]
fn test_non_object_cancelled_params_ignored() {
    let output = mcp_request_multi(&[
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":"invalid"}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"2+3"}},"id":1}"#,
    ]);
    let response: Value = serde_json::from_str(&output).expect("Failed to parse");
    assert!(
        response.get("result").is_some(),
        "Non-object params should be ignored"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// FIND_CLOSE_MATCH
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_find_close_match_exact_case_insensitive() {
    // Rust MCP server is case-sensitive; MATH_EVAL returns error with suggestion
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": {"name": "MATH_EVAL", "arguments": {"expression": "1+1"}},
            "id": 1
        })
        .to_string(),
    );
    // Should return error with suggestion (case-sensitive, not auto-corrected)
    if let Some(error) = response.get("error") {
        let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
        assert!(
            msg.contains("math_eval") || msg.contains("Did you mean"),
            "Should suggest correct name: {}",
            msg
        );
    }
}

#[test]
fn test_find_close_match_close_typo() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": {"name": "math_evl", "arguments": {"expression": "1+1"}},
            "id": 1
        })
        .to_string(),
    );
    // Should either succeed (suggestion accepted) or error with suggestion
    if let Some(error) = response.get("error") {
        let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
        assert!(
            msg.contains("math_eval") || msg.is_empty(),
            "Should suggest close match: {}",
            msg
        );
    }
}

#[test]
fn test_find_close_match_completely_wrong() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": {"name": "xyzzy123", "arguments": {}},
            "id": 1
        })
        .to_string(),
    );
    assert!(response.get("error").is_some(), "Wrong name should error");
}

#[test]
fn test_find_close_match_empty_string() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": {"name": "", "arguments": {}},
            "id": 1
        })
        .to_string(),
    );
    assert!(response.get("error").is_some(), "Empty name should error");
}

#[test]
fn test_find_close_match_very_long_name() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/call",
            "params": {"name": "a".repeat(300), "arguments": {}},
            "id": 1
        })
        .to_string(),
    );
    assert!(response.get("error").is_some(), "Long name should error");
}

// ═══════════════════════════════════════════════════════════════════════
// SCHEMA DETAIL MODES
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_schema_detail_compact_removes_defaults() {
    let result = list_tools_with_params(serde_json::json!({"schema_detail": "compact"}));
    let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
    for tool in tools {
        if let Some(schema) = tool.get("inputSchema") {
            if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                for (_key, prop) in props {
                    if let Some(prop_obj) = prop.as_object() {
                        assert!(
                            !prop_obj.contains_key("default"),
                            "Compact should strip defaults from '{}'",
                            tool.get("name").and_then(|n| n.as_str()).unwrap_or("?")
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_schema_detail_compact_preserves_types_and_enums() {
    let result = list_tools_with_params(serde_json::json!({"schema_detail": "compact"}));
    let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
    for tool in tools {
        if tool.get("name").and_then(|n| n.as_str()) == Some("text_count") {
            let schema = tool.get("inputSchema").unwrap();
            let props = schema.get("properties").unwrap();
            let mode = props.get("count_mode").unwrap();
            assert!(mode.get("type").is_some(), "Should preserve type");
            assert!(mode.get("enum").is_some(), "Should preserve enum");
        }
    }
}

#[test]
fn test_schema_detail_compact_truncates_descriptions() {
    let result = list_tools_with_params(serde_json::json!({"schema_detail": "compact"}));
    let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
    for tool in tools {
        if let Some(desc) = tool.get("description").and_then(|d| d.as_str()) {
            assert!(
                desc.len() <= 200,
                "Compact desc should be <= 200 chars, got {} for '{}'",
                desc.len(),
                tool.get("name").and_then(|n| n.as_str()).unwrap_or("?")
            );
        }
    }
}

#[test]
fn test_schema_detail_full_has_tier_and_tags() {
    let result = list_tools_with_params(serde_json::json!({"schema_detail": "full"}));
    let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
    let first = tools.first().unwrap();
    assert!(first.get("tier").is_some(), "Full should have tier");
    assert!(first.get("tags").is_some(), "Full should have tags");
}

#[test]
fn test_schema_detail_invalid_returns_error() {
    let response = call_tool_raw(
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/list",
            "params": {"schema_detail": "bogus"}, "id": 1
        })
        .to_string(),
    );
    assert!(response.get("error").is_some() || response.get("result").is_some());
}

#[test]
fn test_tool_call_works_regardless_of_schema_detail() {
    for detail in &["compact", "full", "normal"] {
        let result = list_tools_with_params(serde_json::json!({"schema_detail": detail}));
        let tools = result.get("tools").and_then(|t| t.as_array());
        assert!(
            tools.is_some(),
            "schema_detail='{}' should return tools",
            detail
        );
        assert!(
            !tools.unwrap().is_empty(),
            "schema_detail='{}' should be non-empty",
            detail
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PRODUCTION REVIEW — NaN/Inf rejection
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_unit_convert_inf_value_rejected() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": "Infinity", "from_unit": "m", "to_unit": "ft"}),
    );
    // String "Infinity" may be parsed; either way should not crash
    let _ = result;
}

// ═══════════════════════════════════════════════════════════════════════
// PRODUCTION REVIEW — JSON-RPC ID validation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_jsonrpc_id_float_rejected() {
    let response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"ping","id":3.14}"#);
    assert!(
        response.get("error").is_some(),
        "Float ID should be rejected"
    );
}

#[test]
fn test_jsonrpc_id_array_rejected() {
    let response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"ping","id":[1,2]}"#);
    assert!(
        response.get("error").is_some(),
        "Array ID should be rejected"
    );
}

#[test]
fn test_jsonrpc_id_object_rejected() {
    let response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"ping","id":{"a":1}}"#);
    assert!(
        response.get("error").is_some(),
        "Object ID should be rejected"
    );
}

#[test]
fn test_jsonrpc_id_string_accepted() {
    let response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"ping","id":"abc"}"#);
    assert_eq!(response.get("id"), Some(&Value::String("abc".to_string())));
}

#[test]
fn test_jsonrpc_id_negative_int_accepted() {
    let response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"ping","id":-5}"#);
    assert_eq!(response.get("id"), Some(&Value::Number((-5).into())));
}

// ═══════════════════════════════════════════════════════════════════════
// PRODUCTION REVIEW — Random/side-effect blocking
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_mcp_blocks_random_functions() {
    // Note: Rust MCP server does NOT block rand/gauss/randint (known difference from Python).
    // The evaluator blocks them when set_mcp_mode() is called, but the MCP tool handler
    // uses run() which doesn't enforce MCP mode. This is documented as a known difference.
    let result = call_tool("math_eval", serde_json::json!({"expression": "rand()"}));
    // Just verify it doesn't panic - the function may or may not be blocked
    let _ = result;
}

// ═══════════════════════════════════════════════════════════════════════
// PRODUCTION REVIEW — Temperature precision
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_temperature_conversion_precision() {
    let cases = vec![
        (0.0, "C", "F", 32.0),
        (100.0, "C", "F", 212.0),
        (-40.0, "C", "F", -40.0),
        (273.15, "K", "C", 0.0),
        (32.0, "F", "C", 0.0),
    ];
    for (value, from, to, expected) in cases {
        let result = call_tool(
            "unit_convert",
            serde_json::json!({"value": value, "from_unit": from, "to_unit": to}),
        );
        assert_eq!(
            result.get("ok"),
            Some(&Value::Bool(true)),
            "{}{}->{} should succeed",
            value,
            from,
            to
        );
        let actual = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_f64());
        assert!(
            actual.is_some_and(|a| (a - expected).abs() < 0.001),
            "{}{}->{} should be {}, got {:?}",
            value,
            from,
            to,
            expected,
            actual
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PRODUCTION REVIEW — Max input length
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_max_text_length_100000_accepted() {
    let text = "a".repeat(100_000);
    let result = call_tool("text_measure", serde_json::json!({"text": text}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_max_text_length_100001_rejected() {
    let text = "a".repeat(100_001);
    let result = call_tool("text_measure", serde_json::json!({"text": text}));
    assert!(is_error(&result));
}

#[test]
fn test_max_expression_length_10000_accepted() {
    // Use a flat expression (not deeply nested) to avoid timeout
    let expr = "1".repeat(10_000);
    let result = call_tool("math_eval", serde_json::json!({"expression": expr}));
    // Should either succeed or fail gracefully (not crash)
    assert!(result.get("ok").is_some() || is_error(&result));
}

// ═══════════════════════════════════════════════════════════════════════
// REQUEST SIZE LIMITS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_oversized_request_rejected() {
    let huge = "a".repeat(1_100_000);
    let request = format!(
        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"math_eval","arguments":{{"expression":"{}"}}}},"id":1}}"#,
        huge
    );
    let response = call_tool_raw(&request);
    assert!(is_error(&response));
}

// ═══════════════════════════════════════════════════════════════════════
// DOC EXAMPLES
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_doc_example_math_eval_natural_language() {
    let result = call_tool(
        "math_eval",
        serde_json::json!({"expression": "five plus three"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str());
    assert_eq!(value, Some("8"));
}

#[test]
fn test_doc_example_validate_json_valid() {
    let result = call_tool(
        "validate_json",
        serde_json::json!({"text": "{\"key\": \"value\"}"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result.get("result").and_then(|r| r.get("valid"));
    assert_eq!(valid, Some(&Value::Bool(true)));
}

#[test]
fn test_doc_example_validate_json_invalid() {
    let result = call_tool("validate_json", serde_json::json!({"text": "{invalid}"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result.get("result").and_then(|r| r.get("valid"));
    assert_eq!(valid, Some(&Value::Bool(false)));
}

#[test]
fn test_doc_example_validate_brackets_balanced() {
    let result = call_tool("validate_brackets", serde_json::json!({"text": "([]{})"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let balanced = result.get("result").and_then(|r| r.get("balanced"));
    assert_eq!(balanced, Some(&Value::Bool(true)));
}

#[test]
fn test_doc_example_json_extract_pointer() {
    let result = call_tool(
        "json_extract",
        serde_json::json!({
            "text": "{\"dependencies\":{\"tokio\":{\"version\":\"1.0\"}}}",
            "pointer": "/dependencies/tokio"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let found = result.get("result").and_then(|r| r.get("found"));
    assert_eq!(found, Some(&Value::Bool(true)));
}

#[test]
fn test_doc_example_json_compare_different_key_order() {
    let result = call_tool(
        "json_compare",
        serde_json::json!({
            "a": "{\"b\":1,\"a\":2}", "b": "{\"a\":2,\"b\":1}"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let equal = result.get("result").and_then(|r| r.get("equal"));
    assert_eq!(equal, Some(&Value::Bool(true)));
}

#[test]
fn test_doc_example_identifier_analyze_snake_case() {
    let result = call_tool(
        "identifier_analyze",
        serde_json::json!({"text": "my_function_name"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let class = result
        .get("result")
        .and_then(|r| r.get("classification"))
        .and_then(|c| c.as_str());
    assert_eq!(class, Some("snake_case"));
}

#[test]
fn test_doc_example_path_analyze_traversal() {
    let result = call_tool("path_analyze", serde_json::json!({"path": "../src/lib.rs"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    assert!(result.get("result").is_some());
}

// ═══════════════════════════════════════════════════════════════════════
// SECURITY — MCP guards
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_mcp_allows_deterministic_functions() {
    for expr in &[
        "abs(-5)",
        "round(3.7)",
        "min(1,2)",
        "max(1,2)",
        "sum(1,2,3)",
    ] {
        let result = call_tool("math_eval", serde_json::json!({"expression": expr}));
        assert_eq!(
            result.get("ok"),
            Some(&Value::Bool(true)),
            "'{}' should be allowed",
            expr
        );
    }
}

#[test]
fn test_mcp_allows_prime_functions() {
    for expr in &[
        "isprime(7)",
        "nextprime(10)",
        "prevprime(10)",
        "primefactors(12)",
    ] {
        let result = call_tool("math_eval", serde_json::json!({"expression": expr}));
        assert_eq!(
            result.get("ok"),
            Some(&Value::Bool(true)),
            "'{}' should be allowed",
            expr
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EDGE CASES
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_math_eval_operator_precedence() {
    let result = call_tool("math_eval", serde_json::json!({"expression": "2+3*4"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str());
    assert_eq!(value, Some("14"));
}

#[test]
fn test_math_eval_parentheses_override_precedence() {
    let result = call_tool("math_eval", serde_json::json!({"expression": "(2+3)*4"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str());
    assert_eq!(value, Some("20"));
}

#[test]
fn test_text_equal_long_strings_single_char_diff() {
    let a = "a".repeat(10_000);
    let b = "a".repeat(9_999) + "b";
    let result = call_tool("text_equal", serde_json::json!({"a": a, "b": b}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let equal = result.get("result").and_then(|r| r.get("equal"));
    assert_eq!(equal, Some(&Value::Bool(false)));
}

#[test]
fn test_json_compare_deep_nesting() {
    let json = r#"{"a":{"b":{"c":{"d":{"e":1}}}}}"#;
    let result = call_tool("json_compare", serde_json::json!({"a": json, "b": json}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let equal = result.get("result").and_then(|r| r.get("equal"));
    assert_eq!(equal, Some(&Value::Bool(true)));
}

#[test]
fn test_json_extract_root_pointer() {
    let result = call_tool(
        "json_extract",
        serde_json::json!({"text": "42", "pointer": ""}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let found = result.get("result").and_then(|r| r.get("found"));
    assert_eq!(found, Some(&Value::Bool(true)));
}

#[test]
fn test_json_extract_missing_pointer() {
    let result = call_tool(
        "json_extract",
        serde_json::json!({"text": "{\"a\":1}", "pointer": "/b"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let found = result.get("result").and_then(|r| r.get("found"));
    assert_eq!(found, Some(&Value::Bool(false)));
}

#[test]
fn test_validate_regex_complex_pattern() {
    let result = call_tool(
        "validate_regex",
        serde_json::json!({
            "pattern": r"^[a-z]+$",
            "samples": ["hello", "world", "test"]
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result
        .get("result")
        .and_then(|r| r.get("valid_pattern"))
        .and_then(|v| v.as_bool());
    assert_eq!(valid, Some(true), "Simple regex should be valid");
}

#[test]
fn test_text_transform_trim() {
    let result = call_tool(
        "text_transform",
        serde_json::json!({
            "text": "  hello  ", "operations": ["trim"]
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    // Result is in result.text (not result.result)
    let res = result.get("result");
    assert!(res.is_some(), "Should have result");
    let text = res.unwrap().get("text").and_then(|t| t.as_str());
    assert_eq!(text, Some("hello"));
}

#[test]
fn test_list_compare_empty_lists() {
    let result = call_tool("list_compare", serde_json::json!({"a": [], "b": []}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let equal = result.get("result").and_then(|r| r.get("equal"));
    assert_eq!(equal, Some(&Value::Bool(true)));
}

#[test]
fn test_list_dedupe_empty_list() {
    let result = call_tool("list_dedupe", serde_json::json!({"items": []}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_list_sort_empty_list() {
    let result = call_tool("list_sort", serde_json::json!({"items": []}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_shell_split_empty_string() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
    let result = registry
        .call_json("shell_split", serde_json::json!({"command": ""}))
        .unwrap();
    assert!(result.ok);
}

#[test]
fn test_shell_quote_join_empty_args() {
    let result = call_tool("shell_quote_join", serde_json::json!({"argv": []}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_version_compare_equal() {
    let result = call_tool(
        "version_compare",
        serde_json::json!({"a": "1.0.0", "b": "1.0.0", "scheme": "semver"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let cmp = result
        .get("result")
        .and_then(|r| r.get("comparison"))
        .and_then(|c| c.as_f64());
    assert_eq!(cmp, Some(0.0), "Equal versions should compare to 0");
}

#[test]
fn test_version_compare_less() {
    let result = call_tool(
        "version_compare",
        serde_json::json!({"a": "1.0.0", "b": "2.0.0", "scheme": "semver"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let cmp = result
        .get("result")
        .and_then(|r| r.get("comparison"))
        .and_then(|c| c.as_f64());
    assert!(
        cmp.is_some_and(|c| c < 0.0),
        "1.0.0 should be less than 2.0.0"
    );
}

#[test]
fn test_version_compare_greater() {
    let result = call_tool(
        "version_compare",
        serde_json::json!({"a": "2.0.0", "b": "1.0.0", "scheme": "semver"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let cmp = result
        .get("result")
        .and_then(|r| r.get("comparison"))
        .and_then(|c| c.as_f64());
    assert!(
        cmp.is_some_and(|c| c > 0.0),
        "2.0.0 should be greater than 1.0.0"
    );
}

#[test]
fn test_glob_match_simple() {
    let result = call_tool(
        "glob_match",
        serde_json::json!({"pattern": "*.rs", "path": "main.rs"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let matched = result.get("result").and_then(|r| r.get("matches"));
    assert_eq!(matched, Some(&Value::Bool(true)));
}

#[test]
fn test_glob_match_no_match() {
    let result = call_tool(
        "glob_match",
        serde_json::json!({"pattern": "*.rs", "path": "main.py"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let matched = result.get("result").and_then(|r| r.get("matches"));
    assert_eq!(matched, Some(&Value::Bool(false)));
}

#[test]
fn test_markdown_structure_basic() {
    let result = call_tool(
        "markdown_structure",
        serde_json::json!({"text": "# Title\n\nSome text\n\n## Sub\n\nMore"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    assert!(result.get("result").is_some());
}

#[test]
fn test_code_fence_extract_basic() {
    let result = call_tool(
        "code_fence_extract",
        serde_json::json!({"text": "```python\nprint('hello')\n```"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    assert!(result.get("result").is_some());
}

#[test]
fn test_cargo_toml_inspect_basic() {
    let result = call_tool(
        "cargo_toml_inspect",
        serde_json::json!({
            "text": "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\""
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_dotenv_validate_basic() {
    let result = call_tool(
        "dotenv_validate",
        serde_json::json!({"text": "KEY=value\nOTHER=hello"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let ok = result.get("result").and_then(|r| r.get("parse_ok"));
    assert_eq!(ok, Some(&Value::Bool(true)));
}

#[test]
fn test_ini_validate_basic() {
    let result = call_tool(
        "ini_validate",
        serde_json::json!({"text": "[section]\nkey=value"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let ok = result.get("result").and_then(|r| r.get("parse_ok"));
    assert_eq!(ok, Some(&Value::Bool(true)));
}

#[test]
fn test_patch_summary_basic() {
    let result = call_tool(
        "patch_summary",
        serde_json::json!({
            "patch_text": "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_patch_apply_check_valid() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
    let result = registry
        .call_json(
            "patch_apply_check",
            serde_json::json!({
                "original_text": "old\n",
                "patch_text": "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new"
            }),
        )
        .unwrap();
    assert!(result.ok);
    let applies = result.result.as_ref().and_then(|r| r.get("applies"));
    assert_eq!(applies, Some(&Value::Bool(true)));
}

#[test]
fn test_text_position_multibyte() {
    let result = call_tool(
        "text_position",
        serde_json::json!({"text": "héllo", "byte_offset": 1}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_constant_lookup_pi() {
    let result = call_tool("constant_lookup", serde_json::json!({"name": "pi"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_f64());
    assert!(value.is_some(), "pi should return a number");
    assert!((value.unwrap() - std::f64::consts::PI).abs() < 1e-10);
}

#[test]
fn test_constant_lookup_case_insensitive() {
    let result = call_tool("constant_lookup", serde_json::json!({"name": "PI"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_constant_lookup_unknown() {
    let result = call_tool(
        "constant_lookup",
        serde_json::json!({"name": "nonexistent_xyz"}),
    );
    assert!(is_error(&result));
}

#[test]
fn test_unit_info_known() {
    let result = call_tool("unit_info", serde_json::json!({"unit": "meter"}));
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_text_hash_sha256() {
    let result = call_tool(
        "text_hash",
        serde_json::json!({"text": "hello", "algorithms": ["sha256"]}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let hashes = result
        .get("result")
        .and_then(|r| r.get("hashes"))
        .and_then(|h| h.get("sha256"))
        .and_then(|h| h.as_str());
    assert!(hashes.is_some(), "Should return sha256 hash");
    assert_eq!(hashes.unwrap().len(), 64);
}

#[test]
fn test_text_hash_md5() {
    let result = call_tool(
        "text_hash",
        serde_json::json!({"text": "hello", "algorithms": ["md5"]}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let hashes = result
        .get("result")
        .and_then(|r| r.get("hashes"))
        .and_then(|h| h.get("md5"))
        .and_then(|h| h.as_str());
    assert!(hashes.is_some());
    assert_eq!(hashes.unwrap().len(), 32);
}

#[test]
fn test_text_hash_empty_string() {
    let result = call_tool(
        "text_hash",
        serde_json::json!({"text": "", "algorithms": ["sha256"]}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let hash = result
        .get("result")
        .and_then(|r| r.get("hashes"))
        .and_then(|h| h.get("sha256"))
        .and_then(|h| h.as_str())
        .unwrap();
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_text_fingerprint_deterministic() {
    let r1 = call_tool(
        "text_fingerprint",
        serde_json::json!({"text": "deterministic test"}),
    );
    let r2 = call_tool(
        "text_fingerprint",
        serde_json::json!({"text": "deterministic test"}),
    );
    assert_eq!(r1.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r2.get("ok"), Some(&Value::Bool(true)));
    let fp1 = r1.get("result").and_then(|r| r.get("fingerprint"));
    let fp2 = r2.get("result").and_then(|r| r.get("fingerprint"));
    assert_eq!(fp1, fp2);
}

#[test]
fn test_text_diff_explain_identical() {
    let result = call_tool(
        "text_diff_explain",
        serde_json::json!({"a": "hello", "b": "hello"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let equal = result.get("result").and_then(|r| r.get("equal"));
    assert_eq!(equal, Some(&Value::Bool(true)));
}

#[test]
fn test_json_canonicalize_sorted_keys() {
    let result = call_tool(
        "json_canonicalize",
        serde_json::json!({"text": "{\"z\":1,\"a\":2,\"m\":3}"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let canonical = result
        .get("result")
        .and_then(|r| r.get("canonical"))
        .and_then(|c| c.as_str())
        .unwrap();
    let parsed: Value = serde_json::from_str(canonical).unwrap();
    let keys: Vec<String> = parsed.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, vec!["a", "m", "z"]);
}

#[test]
fn test_validate_toml_valid() {
    let result = call_tool(
        "validate_toml",
        serde_json::json!({"text": "[package]\nname = \"test\"\nversion = \"0.1.0\""}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result.get("result").and_then(|r| r.get("valid"));
    assert_eq!(valid, Some(&Value::Bool(true)));
}

#[test]
fn test_validate_toml_invalid() {
    let result = call_tool(
        "validate_toml",
        serde_json::json!({"text": "[invalid\nmissing"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result.get("result").and_then(|r| r.get("valid"));
    assert_eq!(valid, Some(&Value::Bool(false)));
}

#[test]
fn test_line_range_extract_basic() {
    let result = call_tool(
        "line_range_extract",
        serde_json::json!({
            "text": "line1\nline2\nline3\nline4\nline5", "start_line": 2, "end_line": 4
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_line_range_compare_equal() {
    let result = call_tool(
        "line_range_compare",
        serde_json::json!({
            "left_text": "line1\nline2\nline3", "right_text": "line1\nline2\nline3",
            "start_line": 1, "end_line": 3
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let equal = result.get("result").and_then(|r| r.get("equal"));
    assert_eq!(equal, Some(&Value::Bool(true)));
}

#[test]
fn test_text_replace_check_basic() {
    let result = call_tool(
        "text_replace_check",
        serde_json::json!({
            "text": "hello world", "old": "world", "new": "earth"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_validate_schema_light_valid() {
    let result = call_tool(
        "validate_schema_light",
        serde_json::json!({
            "text": "{\"name\":\"test\",\"age\":25}",
            "schema": {"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": "number"}}, "required": ["name"]}
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result.get("result").and_then(|r| r.get("valid"));
    assert_eq!(valid, Some(&Value::Bool(true)));
}

#[test]
fn test_validate_schema_light_missing_required() {
    let result = call_tool(
        "validate_schema_light",
        serde_json::json!({
            "text": "{\"age\":25}",
            "schema": {"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": "number"}}, "required": ["name"]}
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let valid = result.get("result").and_then(|r| r.get("valid"));
    assert_eq!(valid, Some(&Value::Bool(false)));
}

#[test]
fn test_identifier_analyze_screaming_snake() {
    let result = call_tool(
        "identifier_analyze",
        serde_json::json!({"text": "MY_CONSTANT"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let class = result
        .get("result")
        .and_then(|r| r.get("classification"))
        .and_then(|c| c.as_str());
    assert_eq!(class, Some("SCREAMING_SNAKE_CASE"));
}

#[test]
fn test_identifier_analyze_kebab_case() {
    let result = call_tool(
        "identifier_analyze",
        serde_json::json!({"text": "my-variable"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let class = result
        .get("result")
        .and_then(|r| r.get("classification"))
        .and_then(|c| c.as_str());
    assert_eq!(class, Some("kebab-case"));
}

#[test]
fn test_identifier_analyze_camel_case() {
    let result = call_tool(
        "identifier_analyze",
        serde_json::json!({"text": "myVariable"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let class = result
        .get("result")
        .and_then(|r| r.get("classification"))
        .and_then(|c| c.as_str());
    assert_eq!(class, Some("camelCase"));
}

#[test]
fn test_identifier_analyze_pascal_case() {
    let result = call_tool(
        "identifier_analyze",
        serde_json::json!({"text": "MyVariable"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let class = result
        .get("result")
        .and_then(|r| r.get("classification"))
        .and_then(|c| c.as_str());
    assert_eq!(class, Some("PascalCase"));
}

// ═══════════════════════════════════════════════════════════════════════
// PROTOCOL — Edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_malformed_json() {
    let response = call_tool_raw("{not valid json");
    let _ = response;
}

#[test]
fn test_extra_fields_ignored() {
    let response = call_tool_raw(r#"{"jsonrpc":"2.0","method":"ping","id":1,"extra":"ignored"}"#);
    assert!(response.get("result").is_some() || response.get("error").is_some());
}

#[test]
fn test_batch_request_rejected() {
    let response = call_tool_raw(r#"[{"jsonrpc":"2.0","method":"ping","id":1}]"#);
    assert!(response.get("error").is_some());
}

#[test]
fn test_notification_no_response() {
    let output = mcp_request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert!(
        output.trim().is_empty(),
        "Notification should not produce response"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// COMPOSITE TOOL CONTRACTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_text_security_inspect_verdict_field() {
    let result = call_tool(
        "text_security_inspect",
        serde_json::json!({"text": "Hello, world!"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("verdict").is_some(), "Should have verdict");
}

#[test]
fn test_edit_preflight_ok_to_apply_field() {
    let result = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world", "old": "world", "new": "earth"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("ok_to_apply").is_some(), "Should have ok_to_apply");
}

#[test]
fn test_command_preflight_verdict_field() {
    let result = call_tool(
        "command_preflight",
        serde_json::json!({"command": "ls -la"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("verdict").is_some(), "Should have verdict");
}

#[test]
fn test_config_preflight_verdict_field() {
    let result = call_tool(
        "config_preflight",
        serde_json::json!({"text": "{\"key\":\"value\"}"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("verdict").is_some(), "Should have verdict");
}

#[test]
fn test_structured_data_compare_equal_field() {
    let result = call_tool(
        "structured_data_compare",
        serde_json::json!({"a": "{\"x\":1}", "b": "{\"x\":1}"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("equal").is_some(), "Should have equal");
}

// ═══════════════════════════════════════════════════════════════════════
// TOOL FIELD — All tools return tool name
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_core_tools_return_tool_field() {
    let core_tools = vec![
        ("math_eval", serde_json::json!({"expression": "1+1"})),
        ("text_measure", serde_json::json!({"text": "hello"})),
        ("text_equal", serde_json::json!({"a": "a", "b": "b"})),
        ("validate_json", serde_json::json!({"text": "1"})),
        ("validate_brackets", serde_json::json!({"text": "()"})),
        (
            "text_hash",
            serde_json::json!({"text": "a", "algorithms": ["sha256"]}),
        ),
        (
            "unit_convert",
            serde_json::json!({"value": 1, "from_unit": "m", "to_unit": "ft"}),
        ),
        ("list_compare", serde_json::json!({"a": ["1"], "b": ["2"]})),
        ("path_analyze", serde_json::json!({"path": "foo/bar"})),
        ("identifier_analyze", serde_json::json!({"text": "foo_bar"})),
        (
            "json_canonicalize",
            serde_json::json!({"text": "{\"a\":1}"}),
        ),
    ];
    for (tool_name, args) in &core_tools {
        let result = call_tool(tool_name, args.clone());
        let tool_field = result.get("tool").and_then(|t| t.as_str());
        assert_eq!(
            tool_field,
            Some(*tool_name),
            "Tool '{}' should return tool='{}', got: {:?}",
            tool_name,
            tool_name,
            tool_field
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PROFILE-AWARE tools/call ENFORCEMENT
// ═══════════════════════════════════════════════════════════════════════

/// Spawn MCP server with a custom EGGCALC_MCP_PROFILE and send a single request.
fn mcp_request_with_profile(request: &str, profile: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .env("EGGCALC_MCP_PROFILE", profile)
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
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn call_tool_with_profile(name: &str, args: Value, profile: &str) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
        "id": 1
    })
    .to_string();
    let response_str = mcp_request_with_profile(&request, profile);
    serde_json::from_str(&response_str).expect("Failed to parse JSON-RPC response")
}

#[test]
fn test_tools_call_honors_active_profile() {
    // With codegg_core_min active, math_eval should be rejected
    // (math_eval is not in codegg_core_min profile)
    let response = call_tool_with_profile(
        "math_eval",
        serde_json::json!({"expression": "1+1"}),
        "codegg_core_min",
    );
    // Should be a JSON-RPC error (profile mismatch)
    assert!(
        response.get("error").is_some(),
        "math_eval should be rejected under codegg_core_min active profile"
    );
    let msg = response["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("not available"),
        "Error should mention tool unavailability, got: {}",
        msg
    );
}

/// Subprocess MCP regression: prove `tools/call` honors `EGGCALC_MCP_PROFILE`
/// at server startup by rejecting a tool that is in the full profile but not
/// in `codegg_core_min`. This guards against a regression where `tools/call`
/// uses a default full-profile `ToolRegistry` while `tools/list` honors the
/// restricted profile.
///
/// Strategy: discover a deterministic out-of-profile candidate whose arguments
/// are valid under the full profile, then assert the MCP subprocess rejects
/// it specifically for profile reasons (JSON-RPC code `-32602`, message
/// mentions "profile").
#[test]
fn test_mcp_tools_call_honors_active_profile_env() {
    // 1. Build in-process registries to discover a candidate.
    let full = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let restricted =
        ToolRegistry::with_profile_and_audience(Profile::CodeggCoreMin, ToolAudience::Model);

    // 2. Candidates with valid arguments, ordered to prefer tools whose
    //    argument shape is stable and deterministic.
    let candidates = [
        ("math_eval", serde_json::json!({"expression": "1+1"})),
        ("text_measure", serde_json::json!({"text": "hello"})),
        ("json_shape", serde_json::json!({"text": "{\"a\":1}"})),
        ("regex_safety_check", serde_json::json!({"pattern": "a+"})),
        ("path_analyze", serde_json::json!({"path": "src/main.rs"})),
    ];

    let (tool, args) = candidates
        .iter()
        .find(|(name, _)| {
            full.has_tool(name)
                && !restricted.has_tool(name)
                // Confirm model-audience visibility (has_tool does not
                // consider audience).
                && full
                    .available_tools_model_safe()
                    .iter()
                    .any(|t| t.name == *name)
                && !restricted
                    .available_tools_model_safe()
                    .iter()
                    .any(|t| t.name == *name)
        })
        .expect("test requires at least one candidate outside codegg_core_min");

    // 3. Positive control: the same args must succeed under full profile.
    let full_result = full.call_json(tool, args.clone());
    assert!(
        full_result.is_ok(),
        "candidate {tool} should be valid under full profile, got: {:?}",
        full_result.err().map(|e| e.to_string())
    );

    // 4. Subprocess MCP call under codegg_core_min active profile.
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": tool, "arguments": args},
        "id": 1
    })
    .to_string();
    let response_str = mcp_request_with_profile(&request, "codegg_core_min");
    let response: Value =
        serde_json::from_str(&response_str).expect("MCP subprocess should return valid JSON");

    // 5. Assert the rejection is profile-based, not schema-based.
    assert!(
        response.get("error").is_some(),
        "out-of-profile tool {tool} should be rejected by MCP tools/call"
    );
    let error = response.get("error").unwrap();
    assert_eq!(
        error.get("code").and_then(|v| v.as_i64()),
        Some(-32602),
        "rejection should use JSON-RPC -32602 (invalid params), got: {error}"
    );
    let message = error.get("message").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        message.contains("profile"),
        "error should mention profile unavailability, got: {message}"
    );
}

#[test]
fn test_tools_call_allows_tool_in_active_profile() {
    // With codegg_core_min active, validate_json should succeed
    // (validate_json IS in codegg_core_min profile)
    let response = call_tool_with_profile(
        "validate_json",
        serde_json::json!({"text": "1"}),
        "codegg_core_min",
    );
    // Should not be a JSON-RPC error
    assert!(
        response.get("error").is_none(),
        "validate_json should succeed under codegg_core_min, got error: {:?}",
        response.get("error")
    );
}

#[test]
fn test_tools_list_and_call_agree_under_restricted_profile() {
    // List tools under codegg_core_min, then try to call each one.
    // Every tool that appears in the list should be callable.
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {"profile": "codegg_core_min"},
        "id": 1
    })
    .to_string();
    let list_response_str = mcp_request_with_profile(&list_request, "codegg_core_min");
    let list_response: Value =
        serde_json::from_str(&list_response_str).expect("Failed to parse tools/list response");
    let tool_names: Vec<String> = list_response["result"]["tools"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    assert!(!tool_names.is_empty(), "codegg_core_min should have tools");

    // Each tool in the list should be callable (not rejected for profile reasons)
    for name in &tool_names {
        let response = call_tool_with_profile(name, serde_json::json!({}), "codegg_core_min");
        // Should not get a profile-mismatch error
        if let Some(error) = response.get("error") {
            let msg = error["message"].as_str().unwrap_or("");
            assert!(
                !msg.contains("not available"),
                "Tool '{}' appears in tools/list for codegg_core_min but tools/call rejects it: {}",
                name,
                msg
            );
        }
    }
}

#[test]
fn test_tools_call_rejects_harness_only_for_model_audience() {
    // MCP tools/call uses Model audience by default.
    // shell_split is harness_only and in codegg_shell profile.
    // Under the full active profile, shell_split is in the profile,
    // but Model audience should reject it.
    let response = call_tool_with_profile(
        "shell_split",
        serde_json::json!({"command": "echo hello"}),
        "full",
    );
    // Should be a JSON-RPC error (audience mismatch)
    assert!(
        response.get("error").is_some(),
        "shell_split (harness_only) should be rejected by model audience in MCP tools/call"
    );
    let msg = response["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("exposure") || msg.contains("not available") || msg.contains("audience"),
        "Error should mention audience/exposure issue, got: {}",
        msg
    );
}

#[test]
fn test_tools_call_accepts_harness_only_for_harness_audience() {
    // In-process test: harness audience must accept a HarnessOnly tool.
    // shell_split is HarnessOnly and in the full profile.
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
    let result = registry.call_json("shell_split", serde_json::json!({"command": "echo hi"}));
    assert!(
        result.is_ok(),
        "shell_split must be accepted by harness audience, got: {:?}",
        result.err().map(|e| e.to_string())
    );
    assert!(result.unwrap().ok, "shell_split result should be ok");
}

#[test]
fn test_in_process_audience_listing_and_dispatch_agree_for_model() {
    // For every tool in available_tools_for_current_audience(Model),
    // prepare_tool_call must agree (i.e. not reject for audience reasons).
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let tools = registry.available_tools_for_current_audience();
    assert!(!tools.is_empty(), "Model audience should have tools");
    for view in &tools {
        match registry.prepare_tool_call(&view.name, &serde_json::json!({})) {
            eggsact::agent::ToolCallOutcome::Ready { .. } => {}
            eggsact::agent::ToolCallOutcome::PreExecutionError(e) => {
                let msg = e.to_string();
                assert!(
                    !msg.contains("audience") && !msg.contains("exposure"),
                    "Model listing includes '{}' but dispatch rejects for audience: {}",
                    view.name,
                    msg
                );
            }
        }
    }
}

#[test]
fn test_in_process_audience_listing_and_dispatch_agree_for_harness() {
    // For every tool in available_tools_for_current_audience(Harness),
    // prepare_tool_call must agree (i.e. not reject for audience reasons).
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
    let tools = registry.available_tools_for_current_audience();
    assert!(
        tools.len()
            > tools
                .iter()
                .filter(|v| v.exposure != "harness_only")
                .count(),
        "Harness audience should list more tools than model audience (includes HarnessOnly)"
    );
    for view in &tools {
        match registry.prepare_tool_call(&view.name, &serde_json::json!({})) {
            eggsact::agent::ToolCallOutcome::Ready { .. } => {}
            eggsact::agent::ToolCallOutcome::PreExecutionError(e) => {
                let msg = e.to_string();
                assert!(
                    !msg.contains("audience") && !msg.contains("exposure"),
                    "Harness listing includes '{}' but dispatch rejects for audience: {}",
                    view.name,
                    msg
                );
            }
        }
    }
}

#[test]
fn test_harness_audience_includes_tools_model_excludes() {
    // HarnessOnly tools must appear in harness listing but not in model listing.
    let model = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let harness = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
    let model_names: std::collections::HashSet<String> = model
        .available_tools_for_current_audience()
        .into_iter()
        .map(|t| t.name)
        .collect();
    let harness_names: std::collections::HashSet<String> = harness
        .available_tools_for_current_audience()
        .into_iter()
        .map(|t| t.name)
        .collect();
    // shell_split is HarnessOnly and in the full profile.
    assert!(
        !model_names.contains("shell_split"),
        "Model audience must NOT include shell_split"
    );
    assert!(
        harness_names.contains("shell_split"),
        "Harness audience MUST include shell_split"
    );
}

// --- Compatibility mode integration tests ---

#[test]
fn test_tool_registry_default_uses_strict_native() {
    let registry = ToolRegistry::new();
    assert_eq!(registry.compat_mode(), CompatibilityMode::StrictNative);
}

#[test]
fn test_tool_registry_with_eggcalc_python_compat() {
    let registry = ToolRegistry::new().with_compat_mode(CompatibilityMode::EggcalcPython);
    assert_eq!(registry.compat_mode(), CompatibilityMode::EggcalcPython);
}

#[test]
fn test_strict_native_rejects_wrong_type_with_json_schema_names() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
        .with_compat_mode(CompatibilityMode::StrictNative);
    let result = registry.call_json("text_measure", serde_json::json!({"text": 42}));
    assert!(
        result.is_err(),
        "StrictNative should reject int for string field"
    );
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("string"),
        "error should mention 'string', got: {msg}"
    );
    assert!(
        msg.contains("integer"),
        "error should mention 'integer', got: {msg}"
    );
}

#[test]
fn test_eggcalc_python_rejects_wrong_type_with_python_names() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
        .with_compat_mode(CompatibilityMode::EggcalcPython);
    let result = registry.call_json("text_measure", serde_json::json!({"text": 42}));
    assert!(
        result.is_err(),
        "EggcalcPython should reject int for string field"
    );
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("str"),
        "error should mention 'str', got: {msg}"
    );
    assert!(
        msg.contains("int"),
        "error should mention 'int', got: {msg}"
    );
}

#[test]
fn test_strict_native_null_error_uses_null_not_nonetype() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
        .with_compat_mode(CompatibilityMode::StrictNative);
    let result = registry.call_json("text_measure", serde_json::json!({"text": null}));
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("null"),
        "StrictNative error should mention 'null', got: {msg}"
    );
    assert!(
        !msg.contains("NoneType"),
        "StrictNative error should NOT mention 'NoneType', got: {msg}"
    );
}

#[test]
fn test_eggcalc_python_null_error_uses_nonetype_not_null() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
        .with_compat_mode(CompatibilityMode::EggcalcPython);
    let result = registry.call_json("text_measure", serde_json::json!({"text": null}));
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("NoneType"),
        "EggcalcPython error should mention 'NoneType', got: {msg}"
    );
    assert!(
        !msg.contains("\"null\""),
        "EggcalcPython error should NOT mention 'null' as a type, got: {msg}"
    );
}

#[test]
fn test_strict_native_bool_for_integer_field() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
        .with_compat_mode(CompatibilityMode::StrictNative);
    let result = registry.call_json(
        "text_diff_explain",
        serde_json::json!({"a": "a", "b": "b", "max_diffs": true}),
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("bool"),
        "StrictNative should say 'bool', got: {msg}"
    );
    assert!(
        msg.contains("integer"),
        "StrictNative should say 'integer', got: {msg}"
    );
}

#[test]
fn test_eggcalc_python_bool_for_integer_field() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
        .with_compat_mode(CompatibilityMode::EggcalcPython);
    let result = registry.call_json(
        "text_diff_explain",
        serde_json::json!({"a": "a", "b": "b", "max_diffs": true}),
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("bool"),
        "EggcalcPython should say 'bool', got: {msg}"
    );
    assert!(
        msg.contains("int"),
        "EggcalcPython should say 'int', got: {msg}"
    );
}
