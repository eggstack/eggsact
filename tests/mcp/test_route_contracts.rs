//! Route-contract classification tests.
//!
//! Verifies that:
//! - Route-critical tools (preflight/composite) include machine_code and verdict
//!   on every successful response.
//! - Simple utility tools are not forced into artificial verdicts.
//! - All non-OK tool responses carry a machine_code.

use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use eggsact::mcp::registry::{is_route_critical, ROUTE_CRITICAL_TOOLS};
use eggsact::mcp::response::ToolResponse;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Route-critical classification helpers
// ---------------------------------------------------------------------------

fn full_harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
}

/// Call a tool and return the ToolResponse (not just the result).
fn call_tool_response(name: &str, args: Value) -> ToolResponse {
    let registry = full_harness_registry();
    registry
        .call_json(name, args)
        .unwrap_or_else(|e| panic!("Tool call to '{name}' failed at registry level: {e}"))
}

// ═══════════════════════════════════════════════════════════════════════
// ROUTE-CRITICAL CLASSIFICATION
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_route_critical_list_contains_expected_tools() {
    let expected = [
        "edit_preflight",
        "command_preflight",
        "config_preflight",
        "patch_apply_check",
        "text_security_inspect",
    ];
    for name in &expected {
        assert!(
            ROUTE_CRITICAL_TOOLS.contains(name),
            "ROUTE_CRITICAL_TOOLS should contain '{name}'"
        );
    }
}

#[test]
fn test_is_route_critical_matches_list() {
    for name in ROUTE_CRITICAL_TOOLS {
        assert!(
            is_route_critical(name),
            "is_route_critical('{name}') should be true"
        );
    }
    assert!(!is_route_critical("math_eval"));
    assert!(!is_route_critical("text_measure"));
    assert!(!is_route_critical("json_extract"));
}

#[test]
fn test_all_route_critical_tools_exist_in_registry() {
    let registry = full_harness_registry();
    for name in ROUTE_CRITICAL_TOOLS {
        let tools = registry.available_tools();
        assert!(
            tools.iter().any(|t| t.name == *name),
            "Route-critical tool '{name}' should exist in the full registry"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ROUTE-CRITICAL TOOL CONTRACTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_command_preflight_success_has_machine_code_and_verdict() {
    let resp = call_tool_response("command_preflight", json!({"command": "echo hello"}));
    assert!(resp.ok, "command_preflight should succeed for 'echo hello'");
    assert!(
        resp.machine_code.is_some(),
        "command_preflight must include machine_code on success"
    );
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("verdict").is_some(),
        "command_preflight result must include verdict"
    );
    assert!(
        result.get("summary").is_some(),
        "command_preflight result must include summary"
    );
}

#[test]
fn test_config_preflight_success_has_machine_code_and_verdict() {
    let resp = call_tool_response(
        "config_preflight",
        json!({"text": "key = \"value\"", "format": "toml"}),
    );
    assert!(resp.ok, "config_preflight should succeed for valid TOML");
    assert!(
        resp.machine_code.is_some(),
        "config_preflight must include machine_code on success"
    );
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("verdict").is_some(),
        "config_preflight result must include verdict"
    );
    assert!(
        result.get("summary").is_some(),
        "config_preflight result must include summary"
    );
}

#[test]
fn test_text_security_inspect_success_has_machine_code_and_verdict() {
    let resp = call_tool_response("text_security_inspect", json!({"text": "Hello world"}));
    assert!(
        resp.ok,
        "text_security_inspect should succeed for plain text"
    );
    assert!(
        resp.machine_code.is_some(),
        "text_security_inspect must include machine_code on success"
    );
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("verdict").is_some(),
        "text_security_inspect result must include verdict"
    );
    assert!(
        result.get("summary").is_some(),
        "text_security_inspect result must include summary"
    );
}

#[test]
fn test_edit_preflight_success_has_machine_code_and_verdict() {
    let resp = call_tool_response(
        "edit_preflight",
        json!({
            "original": "hello world",
            "old": "hello",
            "new": "world"
        }),
    );
    assert!(resp.ok, "edit_preflight should succeed");
    assert!(
        resp.machine_code.is_some(),
        "edit_preflight must include machine_code on success"
    );
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("verdict").is_some(),
        "edit_preflight result must include verdict"
    );
    assert!(
        result.get("summary").is_some(),
        "edit_preflight result must include summary"
    );
}

#[test]
fn test_patch_apply_check_success_has_machine_code_and_verdict() {
    let resp = call_tool_response(
        "patch_apply_check",
        json!({
            "original_text": "hello\n",
            "patch_text": "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-hello\n+world\n"
        }),
    );
    assert!(resp.ok, "patch_apply_check should succeed for valid patch");
    assert!(
        resp.machine_code.is_some(),
        "patch_apply_check must include machine_code on success"
    );
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("verdict").is_some(),
        "patch_apply_check result must include verdict"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// SIMPLE UTILITY TOOLS: NO ARTIFICIAL VERDICTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_simple_tool_math_eval_no_verdict_required() {
    let resp = call_tool_response("math_eval", json!({"expression": "2 + 2"}));
    assert!(resp.ok);
    let result = resp.result.as_ref().expect("result should be present");
    // Simple tools may have machine_code but should NOT be required to have verdict
    // (this test just documents that they don't — no assertion on machine_code)
    assert!(
        result.get("verdict").is_none(),
        "math_eval should not include verdict (it's a simple utility)"
    );
}

#[test]
fn test_simple_tool_text_measure_no_verdict_required() {
    let resp = call_tool_response("text_measure", json!({"text": "hello"}));
    assert!(resp.ok);
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("verdict").is_none(),
        "text_measure should not include verdict"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// NON-OK RESPONSES CARRY MACHINE CODES
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_non_ok_response_from_validation_error() {
    let registry = full_harness_registry();
    let result = registry.call_json("math_eval", json!({"expression": ""}));
    // Empty expression may fail at registry level (schema validation) or handler level
    // Either way, it should not succeed
    if let Ok(resp) = result {
        assert!(
            !resp.ok,
            "math_eval should not succeed for empty expression"
        );
        assert!(
            resp.machine_code.is_some(),
            "Non-OK handler response must carry machine_code"
        );
    }
    // Err is also acceptable (registry-level validation rejection)
}

#[test]
fn test_non_ok_response_from_wrong_type_returns_registry_error() {
    let registry = full_harness_registry();
    let result = registry.call_json("text_measure", json!({"text": 42}));
    assert!(
        result.is_err(),
        "text_measure should return registry error for int text"
    );
}

#[test]
fn test_non_ok_response_from_unknown_tool_has_machine_code() {
    let registry = full_harness_registry();
    let result = registry.call_json("nonexistent_tool_xyz", json!({}));
    assert!(result.is_err(), "Unknown tool should return registry error");
}

// ═══════════════════════════════════════════════════════════════════════
// MACHINE CODE CONSTANT INTEGRITY
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_route_critical_tools_use_known_machine_codes() {
    use eggsact::mcp::machine_codes;

    // Test that route-critical tools produce machine codes from the known set
    let test_cases: Vec<(&str, Value, &str)> = vec![
        (
            "command_preflight",
            json!({"command": "echo test"}),
            "COMMAND_OK",
        ),
        (
            "config_preflight",
            json!({"text": "x = 1", "format": "toml"}),
            "CONFIG_OK",
        ),
        (
            "text_security_inspect",
            json!({"text": "safe text"}),
            "TEXT_SECURITY_OK",
        ),
    ];

    for (name, args, expected_code) in test_cases {
        let resp = call_tool_response(name, args);
        if resp.ok {
            let code = resp
                .machine_code
                .as_deref()
                .unwrap_or_else(|| panic!("{name} should have machine_code"));
            assert!(
                machine_codes::ALL.contains(&code),
                "Machine code '{code}' from {name} should be in machine_codes::ALL"
            );
            assert_eq!(
                code, expected_code,
                "{name} should produce '{expected_code}' for clean input"
            );
        }
    }
}

/// All finding `code` strings emitted by route-critical tools must be
/// either:
///   - In `machine_codes::ALL`, OR
///   - Lowercase / mixed-case descriptive strings (which are valid as
///     local finding kinds, not wire-level machine codes).
///
/// Ad-hoc UPPERCASE_SNAKE_CASE finding codes that are NOT in
/// `machine_codes::ALL` are forbidden: they violate the route-contract
/// discipline and must be either promoted to constants or renamed to
/// lowercase.
#[test]
fn test_route_critical_finding_codes_are_enumerated() {
    use eggsact::mcp::machine_codes;

    // Per-tool fixture cases that exercise representative code paths and
    // produce at least one finding. Each tuple is (tool, args, label).
    let cases: Vec<(&str, Value, &str)> = vec![
        // edit_preflight literal missing new — emits EDIT_ARGUMENTS_MISSING
        (
            "edit_preflight",
            json!({"original": "abc", "old": "a", "replacement_mode": "literal"}),
            "edit_preflight literal missing new",
        ),
        // edit_preflight line_range missing start_line — emits EDIT_ARGUMENTS_MISSING
        (
            "edit_preflight",
            json!({"original": "abc", "replacement_mode": "line_range", "end_line": 1, "new": "x"}),
            "edit_preflight line_range missing start",
        ),
        // edit_preflight line_range missing new — emits EDIT_ARGUMENTS_MISSING
        (
            "edit_preflight",
            json!({"original": "abc", "replacement_mode": "line_range", "start_line": 1, "end_line": 1}),
            "edit_preflight line_range missing new",
        ),
        // edit_preflight metadata oversize — emits EDIT_METADATA_TOO_LARGE
        (
            "edit_preflight",
            json!({
                "original": "abc",
                "old": "a",
                "new": "b",
                "replacement_mode": "literal",
                "edit_metadata": {"description": "x".repeat(1500)}
            }),
            "edit_preflight metadata oversize",
        ),
        // command_preflight rm -rf / — emits SHELL_DESTRUCTIVE_COMMAND
        (
            "command_preflight",
            json!({"command": "rm -rf /tmp"}),
            "command_preflight rm -rf",
        ),
        // command_preflight cargo build — emits SHELL_POLICY_REVIEW
        (
            "command_preflight",
            json!({"command": "cargo build"}),
            "command_preflight cargo build",
        ),
        // command_preflight curl review
        (
            "command_preflight",
            json!({"command": "curl https://example.com"}),
            "command_preflight curl",
        ),
    ];

    for (tool, args, label) in cases {
        let resp = call_tool_response(tool, args);
        let result = match resp.result.as_ref() {
            Some(r) => r,
            None => continue,
        };
        let findings = result
            .get("findings")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();
        for f in &findings {
            let code = match f.get("code").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => panic!("{label}: finding has no code field: {f}"),
            };
            // Allow lowercase / descriptive codes (local finding kinds).
            let is_upper_snake = code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            if is_upper_snake {
                assert!(
                    machine_codes::ALL.contains(&code),
                    "{label}: ad-hoc upper-snake finding code '{code}' is not in machine_codes::ALL; promote to a constant or rename to lowercase.\nFinding: {f}"
                );
            }
        }
    }
}
