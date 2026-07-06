//! Integration tests for typed preflight wrappers.
//!
//! Verifies that:
//! - Wrappers construct arguments using canonical schema field names.
//! - Wrappers construct profiles/audiences that can execute their underlying tool.
//! - parse_response() correctly extracts typed fields from a ToolResponse.
//! - Missing required fields surface as PreflightError rather than silent defaults.

use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use eggsact::mcp::response::ToolResponse;
use eggsact::preflight::{
    CommandPolicy, CommandPreflight, CommandPreflightInput, ConfigFormat, ConfigPreflight,
    ConfigPreflightInput, EditNewlinePolicy, EditPreflight, EditPreflightInput, EditUnicodePolicy,
    PatchApplyCheck, PatchApplyCheckInput, ReplacementMode, TextSecurityInspect,
    TextSecurityInspectInput,
};

fn harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
}

// ═══════════════════════════════════════════════════════════════════════════════
// PatchApplyCheck — Task 1
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn patch_apply_check_run_succeeds_for_valid_patch() {
    let input = PatchApplyCheckInput {
        original_text: "hello\n".to_string(),
        patch_text: "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-hello\n+world\n".to_string(),
        return_result_text: false,
        strict: false,
    };
    let result = PatchApplyCheck::run(&input);
    assert!(
        result.is_ok(),
        "PatchApplyCheck::run should succeed for valid patch: {:?}",
        result.err()
    );
    let output = result.unwrap();
    assert!(output.patch_parse_ok);
    assert!(output.applies);
    assert!(
        !output.machine_code.is_empty(),
        "machine_code should be set"
    );
}

#[test]
fn patch_apply_check_run_uses_canonical_argument_names() {
    // This test confirms the wrapper passes schema-required arg names.
    // If the wrapper regresses to "patch"/"original", schema validation
    // rejects the call and run() returns PreflightError::ToolCall.
    let input = PatchApplyCheckInput {
        original_text: "abc\n".to_string(),
        patch_text: "--- a\n+++ b\n@@ -1 +1 @@\n-abc\n+xyz\n".to_string(),
        return_result_text: false,
        strict: false,
    };
    match PatchApplyCheck::run(&input) {
        Ok(_) => {}
        Err(eggsact::preflight::PreflightError::ToolCall(msg)) => {
            panic!("PatchApplyCheck::run returned ToolCall error (likely wrong arg names): {msg}");
        }
        Err(e) => {
            // Tool rejection is acceptable (e.g., malformed patch),
            // but ToolCall means registry/schema rejection which is the bug.
            eprintln!("PatchApplyCheck tool rejection (acceptable): {e:?}");
        }
    }
}

#[test]
fn patch_apply_check_run_with_registry_harness_audience() {
    // Direct test: run with harness audience must succeed.
    let registry = harness_registry();
    let input = PatchApplyCheckInput {
        original_text: "foo\n".to_string(),
        patch_text: "--- a\n+++ b\n@@ -1 +1 @@\n-foo\n+bar\n".to_string(),
        return_result_text: false,
        strict: false,
    };
    let result = PatchApplyCheck::run_with_registry(&registry, &input);
    assert!(
        result.is_ok(),
        "run_with_registry should succeed: {:?}",
        result.err()
    );
}

#[test]
fn patch_apply_check_run_with_model_audience_rejected() {
    // The Model audience should be rejected for HarnessOnly tool.
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let input = PatchApplyCheckInput {
        original_text: "x\n".to_string(),
        patch_text: "--- a\n+++ b\n@@ -1 +1 @@\n-x\n+y\n".to_string(),
        return_result_text: false,
        strict: false,
    };
    let result = PatchApplyCheck::run_with_registry(&registry, &input);
    assert!(
        matches!(result, Err(eggsact::preflight::PreflightError::ToolCall(_))),
        "Model audience should be rejected for HarnessOnly tool, got: {:?}",
        result
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TextSecurityInspect — Task 2 (Default exposure, should work via Model audience)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn text_security_inspect_run_succeeds_for_clean_text() {
    let input = TextSecurityInspectInput {
        text: "hello world".to_string(),
        policy: "default".to_string(),
        detail: None,
    };
    let result = TextSecurityInspect::run(&input);
    assert!(
        result.is_ok(),
        "TextSecurityInspect::run should succeed: {:?}",
        result.err()
    );
    let output = result.unwrap();
    assert!(!output.verdict.is_empty(), "verdict should be set");
    assert!(output.machine_code.contains("TEXT_SECURITY"));
}

#[test]
fn text_security_inspect_run_with_registry() {
    let registry = harness_registry();
    let input = TextSecurityInspectInput {
        text: "test".to_string(),
        policy: "default".to_string(),
        detail: None,
    };
    let result = TextSecurityInspect::run_with_registry(&registry, &input);
    assert!(
        result.is_ok(),
        "run_with_registry should succeed: {:?}",
        result.err()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// EditPreflight — Task 2 (Default exposure)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn edit_preflight_run_succeeds_for_literal_match() {
    let input = EditPreflightInput {
        original: "hello world".to_string(),
        mode: ReplacementMode::Literal,
        old: Some("hello".to_string()),
        new: Some("goodbye".to_string()),
        patch: None,
        start_line: None,
        end_line: None,
        expected_fingerprint: None,
        strict: false,
        file_path: None,
        workspace_root: None,
        newline_policy: EditNewlinePolicy::Skip,
        unicode_policy: EditUnicodePolicy::Skip,
        edit_metadata: None,
    };
    let result = EditPreflight::run(&input);
    assert!(
        result.is_ok(),
        "EditPreflight::run should succeed: {:?}",
        result.err()
    );
    let output = result.unwrap();
    assert!(output.ok_to_apply);
}

#[test]
fn edit_preflight_run_with_registry() {
    let registry = harness_registry();
    let input = EditPreflightInput {
        original: "foo".to_string(),
        mode: ReplacementMode::Literal,
        old: Some("foo".to_string()),
        new: Some("bar".to_string()),
        patch: None,
        start_line: None,
        end_line: None,
        expected_fingerprint: None,
        strict: false,
        file_path: None,
        workspace_root: None,
        newline_policy: EditNewlinePolicy::Skip,
        unicode_policy: EditUnicodePolicy::Skip,
        edit_metadata: None,
    };
    let result = EditPreflight::run_with_registry(&registry, &input);
    assert!(
        result.is_ok(),
        "run_with_registry should succeed: {:?}",
        result.err()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CommandPreflight — Task 2 (Default exposure)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn command_preflight_run_succeeds_for_safe_command() {
    let input = CommandPreflightInput {
        command: "ls -la".to_string(),
        platform: "auto".to_string(),
        policy: CommandPolicy::Strict,
        working_directory: None,
        policy_config: None,
    };
    let result = CommandPreflight::run(&input);
    assert!(
        result.is_ok(),
        "CommandPreflight::run should succeed: {:?}",
        result.err()
    );
}

#[test]
fn command_preflight_run_with_registry() {
    let registry = harness_registry();
    let input = CommandPreflightInput {
        command: "echo hello".to_string(),
        platform: "auto".to_string(),
        policy: CommandPolicy::Strict,
        working_directory: None,
        policy_config: None,
    };
    let result = CommandPreflight::run_with_registry(&registry, &input);
    assert!(
        result.is_ok(),
        "run_with_registry should succeed: {:?}",
        result.err()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ConfigPreflight — Task 2 (Default exposure)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn config_preflight_run_succeeds_for_valid_json() {
    let input = ConfigPreflightInput {
        text: r#"{"key": "value"}"#.to_string(),
        format: ConfigFormat::Json,
        schema: None,
        strict: false,
    };
    let result = ConfigPreflight::run(&input);
    assert!(
        result.is_ok(),
        "ConfigPreflight::run should succeed: {:?}",
        result.err()
    );
    let output = result.unwrap();
    assert!(output.valid);
}

#[test]
fn config_preflight_run_with_registry() {
    let registry = harness_registry();
    let input = ConfigPreflightInput {
        text: "[1, 2, 3]".to_string(),
        format: ConfigFormat::Json,
        schema: None,
        strict: false,
    };
    let result = ConfigPreflight::run_with_registry(&registry, &input);
    assert!(
        result.is_ok(),
        "run_with_registry should succeed: {:?}",
        result.err()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// parse_response contract tests — confirms successful parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn patch_apply_check_parse_response_extracts_fields() {
    let registry = harness_registry();
    let input = PatchApplyCheckInput {
        original_text: "x\n".to_string(),
        patch_text: "--- a\n+++ b\n@@ -1 +1 @@\n-x\n+y\n".to_string(),
        return_result_text: false,
        strict: false,
    };
    let response = registry
        .call_json(
            "patch_apply_check",
            serde_json::json!({
                "patch_text": input.patch_text,
                "original_text": input.original_text,
                "return_result_text": false,
                "strict": false,
            }),
        )
        .expect("registry call should succeed");
    assert!(response.ok, "Tool should succeed");
    let parsed = PatchApplyCheck::parse_response(response).expect("parse should succeed");
    assert!(parsed.patch_parse_ok);
    assert!(parsed.applies);
    assert_eq!(parsed.hunks_total, 1);
    assert_eq!(parsed.hunks_applied, 1);
}

#[test]
fn patch_apply_check_parse_response_rejects_missing_fields() {
    // Construct a response with ok=true but missing mandatory fields
    let response = ToolResponse {
        ok: true,
        tool: Some("patch_apply_check".to_string()),
        result: Some(serde_json::json!({})), // missing patch_parse_ok, applies, etc.
        error_type: None,
        error: None,
        hints: None,
        warnings: None,
        limits_applied: None,
        findings: None,
        machine_code: Some("PATCH_APPLY_OK".to_string()),
        recommended_next_tool: None,
    };
    let result = PatchApplyCheck::parse_response(response);
    assert!(
        matches!(
            result,
            Err(eggsact::preflight::PreflightError::ContractViolation { .. })
        ),
        "Missing required fields must be ContractViolation, got: {:?}",
        result
    );
}

#[test]
fn patch_apply_check_parse_response_rejects_tool_rejection() {
    let response = ToolResponse {
        ok: false,
        tool: Some("patch_apply_check".to_string()),
        result: None,
        error_type: Some("invalid_arguments".to_string()),
        error: Some("bad patch".to_string()),
        hints: None,
        warnings: None,
        limits_applied: None,
        findings: None,
        machine_code: Some("INVALID_ARGUMENTS".to_string()),
        recommended_next_tool: None,
    };
    let result = PatchApplyCheck::parse_response(response);
    assert!(
        matches!(
            result,
            Err(eggsact::preflight::PreflightError::ToolRejected { .. })
        ),
        "Tool rejection must surface as PreflightError::ToolRejected, got: {:?}",
        result
    );
}
