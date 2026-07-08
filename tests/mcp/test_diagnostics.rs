use eggsact::agent::{Profile, ToolAudience, ToolCallError, ToolCallOutcome, ToolRegistry};
use eggsact::mcp::budget::{BudgetContext, ToolBudget};
use eggsact::mcp::machine_codes;
use eggsact::mcp::registry::{tools_for_profile_audience, ToolListAudience};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn full_harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
}

fn call_tool_response(name: &str, args: Value) -> eggsact::mcp::response::ToolResponse {
    let registry = full_harness_registry();
    registry
        .call_json(name, args)
        .unwrap_or_else(|e| panic!("Tool call to '{name}' failed at registry level: {e}"))
}

#[test]
fn test_runtime_diagnostics_returns_structured_output() {
    let resp = call_tool_response("runtime_diagnostics", json!({}));
    assert!(resp.ok, "runtime_diagnostics should succeed");
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("OK"),
        "machine_code should be OK"
    );
    let result = resp.result.as_ref().expect("result should be present");
    assert!(
        result.get("active_profile").is_some(),
        "result should contain active_profile"
    );
    assert!(
        result["active_profile"].is_string(),
        "active_profile should be a string"
    );
    assert!(
        result.get("active_audience").is_some(),
        "result should contain active_audience"
    );
    assert!(
        result["active_audience"].is_string(),
        "active_audience should be a string"
    );
    assert!(
        result.get("tool_count").is_some(),
        "result should contain tool_count"
    );
    assert!(
        result["tool_count"].is_number(),
        "tool_count should be a number"
    );
    assert!(
        result["tool_count"].as_i64().unwrap() >= 0,
        "tool_count should be >= 0"
    );
    assert!(
        result.get("route_critical_tools").is_some(),
        "result should contain route_critical_tools"
    );
    assert!(
        result["route_critical_tools"].is_array(),
        "route_critical_tools should be an array"
    );
    assert!(
        result.get("parity_available").is_some(),
        "result should contain parity_available"
    );
    assert!(
        result["parity_available"].is_boolean(),
        "parity_available should be a boolean"
    );
}

#[test]
fn test_runtime_diagnostics_expanded_fields() {
    let resp = call_tool_response("runtime_diagnostics", json!({}));
    assert!(resp.ok);
    let result = resp.result.as_ref().expect("result should be present");

    assert!(
        result.get("model_visible_tool_count").is_some(),
        "result should contain model_visible_tool_count"
    );
    assert!(
        result["model_visible_tool_count"].is_number(),
        "model_visible_tool_count should be a number"
    );
    assert!(
        result.get("harness_visible_tool_count").is_some(),
        "result should contain harness_visible_tool_count"
    );
    assert!(
        result["harness_visible_tool_count"].is_number(),
        "harness_visible_tool_count should be a number"
    );
    assert!(
        result.get("verification_command").is_some(),
        "result should contain verification_command"
    );
    assert!(
        result["verification_command"].is_string(),
        "verification_command should be a string"
    );
    assert!(
        result.get("generated_data").is_some(),
        "result should contain generated_data"
    );
    let gen_data = &result["generated_data"];
    assert!(
        gen_data.get("confusables_generated_rs").is_some(),
        "generated_data should contain confusables_generated_rs"
    );
    assert!(
        gen_data.get("tool_cards_md").is_some(),
        "generated_data should contain tool_cards_md"
    );
    assert!(
        result.get("runtime").is_some(),
        "result should contain runtime"
    );
    let runtime = &result["runtime"];
    assert!(
        runtime.get("schema_detail").is_some(),
        "runtime should contain schema_detail"
    );
    assert!(
        runtime.get("limits").is_some(),
        "runtime should contain limits"
    );
}

#[test]
fn test_runtime_diagnostics_harness_only_not_listed_to_model() {
    let model_tools = tools_for_profile_audience("full", ToolListAudience::Model);
    let has_runtime_diagnostics = model_tools.iter().any(|t| t.name == "runtime_diagnostics");
    assert!(
        !has_runtime_diagnostics,
        "runtime_diagnostics should NOT appear in Model audience listing for 'full' profile"
    );
}

#[test]
fn test_runtime_diagnostics_harness_audience_can_see() {
    let harness_tools = tools_for_profile_audience("full", ToolListAudience::Harness);
    let has_runtime_diagnostics = harness_tools
        .iter()
        .any(|t| t.name == "runtime_diagnostics");
    assert!(
        has_runtime_diagnostics,
        "runtime_diagnostics SHOULD appear in Harness audience listing for 'full' profile"
    );
}

#[test]
fn test_runtime_diagnostics_no_env_var_values() {
    let resp = call_tool_response("runtime_diagnostics", json!({}));
    assert!(resp.ok);
    let result = resp.result.as_ref().expect("result should be present");
    let known_env_vars = result["known_env_vars"]
        .as_array()
        .expect("known_env_vars should be an array");
    for var in known_env_vars {
        let s = var
            .as_str()
            .expect("known_env_vars entries should be strings");
        assert!(
            !s.contains('='),
            "known_env_vars should contain names only, not key=value pairs; found: {}",
            s
        );
        assert!(
            !s.starts_with('/') && !s.contains("://"),
            "known_env_vars should not contain paths or URLs; found: {}",
            s
        );
    }
}

#[test]
fn test_profile_rejection_includes_tool_name() {
    let registry = ToolRegistry::with_profile(Profile::custom("nonexistent"));
    let outcome = registry.prepare_tool_call("math_eval", &json!({}));
    match outcome {
        ToolCallOutcome::PreExecutionError(err) => {
            let display = format!("{}", err);
            assert!(
                display.contains("math_eval"),
                "error Display output should include the tool name 'math_eval', got: {}",
                display
            );
        }
        _ => {
            panic!("Expected PreExecutionError for nonexistent profile");
        }
    }
}

#[test]
fn test_audience_rejection_includes_exposure_info() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let outcome = registry.prepare_tool_call("runtime_diagnostics", &json!({}));
    match outcome {
        ToolCallOutcome::PreExecutionError(err) => {
            let display = format!("{}", err);
            assert!(
                display.contains("harness_only"),
                "error Display output should include the exposure level 'harness_only', got: {}",
                display
            );
            if let ToolCallError::ToolNotAllowedForAudience { exposure, .. } = &err {
                assert_eq!(
                    exposure, "harness_only",
                    "exposure field should be harness_only"
                );
            } else {
                panic!("Expected ToolNotAllowedForAudience variant");
            }
        }
        _ => {
            panic!("Expected PreExecutionError for HarnessOnly tool with Model audience");
        }
    }
}

#[test]
fn test_timeout_preserves_machine_code() {
    assert_eq!(machine_codes::TIMEOUT, "TIMEOUT");
    assert_eq!(machine_codes::CANCELLED, "CANCELLED");
    let budget = ToolBudget::CHEAP.with_max_elapsed_ms(1);
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let ctx = BudgetContext::new(budget).with_cancellation(cancel_flag);
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(
        ctx.is_expired(),
        "BudgetContext should be expired after sleeping past its deadline"
    );
    let result = ctx.check_deadline("test_tool");
    assert!(
        result.is_err(),
        "check_deadline should return Err on expiry"
    );
    let err_response = result.unwrap_err();
    assert_eq!(
        err_response.machine_code.as_deref(),
        Some("TIMEOUT"),
        "timeout error should carry TIMEOUT machine_code"
    );
}

#[test]
fn test_cancelled_is_distinct_from_timeout() {
    assert_ne!(
        machine_codes::TIMEOUT,
        machine_codes::CANCELLED,
        "TIMEOUT and CANCELLED must be distinct machine code constants"
    );
}

// ---------------------------------------------------------------------------
// profile_inspect tests
// ---------------------------------------------------------------------------

#[test]
fn test_profile_inspect_returns_structured_output() {
    let resp = call_tool_response("profile_inspect", json!({"profile": "full"}));
    assert!(resp.ok, "profile_inspect should succeed");
    assert_eq!(resp.machine_code.as_deref(), Some("OK"));
    let result = resp.result.as_ref().expect("result should be present");
    assert_eq!(result["name"], "full");
    assert!(result["tool_count"].is_number());
    assert!(result["tool_count"].as_i64().unwrap() > 0);
    assert!(result["model_visible_tool_count"].is_number());
    assert!(result["harness_visible_tool_count"].is_number());
    assert!(result["intended_audience"].is_string());
    assert!(result["purpose"].is_string());
    assert!(result["contains_route_critical_tools"].is_boolean());
    assert!(result["contains_harness_only_tools"].is_boolean());
    assert!(result["representative_tools"].is_array());
    assert!(result["warnings"].is_array());
}

#[test]
fn test_profile_inspect_known_profiles() {
    let profiles = [
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
    for name in &profiles {
        let resp = call_tool_response("profile_inspect", json!({"profile": name}));
        assert!(resp.ok, "profile_inspect for '{}' should succeed", name);
        let result = resp.result.as_ref().unwrap();
        assert_eq!(result["name"], *name);
        assert!(
            result["tool_count"].as_i64().unwrap() > 0,
            "profile '{}' should have at least one tool",
            name
        );
    }
}

#[test]
fn test_profile_inspect_unknown_profile_returns_error() {
    let resp = call_tool_response("profile_inspect", json!({"profile": "nonexistent_profile"}));
    assert!(!resp.ok, "profile_inspect for unknown profile should fail");
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("INVALID_ARGUMENTS"),
        "should use INVALID_ARGUMENTS machine code"
    );
}

#[test]
fn test_profile_inspect_harness_only_excluded_from_model() {
    let model_tools = tools_for_profile_audience("full", ToolListAudience::Model);
    let has_profile_inspect = model_tools.iter().any(|t| t.name == "profile_inspect");
    assert!(
        !has_profile_inspect,
        "profile_inspect should NOT appear in Model audience listing"
    );
}

#[test]
fn test_profile_inspect_codegg_preflight_has_harness_only() {
    let resp = call_tool_response("profile_inspect", json!({"profile": "codegg_preflight"}));
    assert!(resp.ok);
    let result = resp.result.as_ref().unwrap();
    assert_eq!(result["contains_harness_only_tools"], true);
}

#[test]
fn test_profile_inspect_full_has_route_critical() {
    let resp = call_tool_response("profile_inspect", json!({"profile": "full"}));
    assert!(resp.ok);
    let result = resp.result.as_ref().unwrap();
    assert_eq!(result["contains_route_critical_tools"], true);
}

// ---------------------------------------------------------------------------
// tool_availability_explain tests
// ---------------------------------------------------------------------------

#[test]
fn test_tool_availability_explain_existing_model_safe_tool() {
    let resp = call_tool_response("tool_availability_explain", json!({"tool": "math_eval"}));
    assert!(resp.ok, "tool_availability_explain should succeed");
    let result = resp.result.as_ref().expect("result should be present");
    assert_eq!(result["exists"], true);
    assert_eq!(result["available_in_profile"], true);
    assert!(result["exposure"].is_string());
    assert!(result["profiles"].is_array());
    assert!(result["reason"].is_string());
}

#[test]
fn test_tool_availability_explain_harness_only_tool_in_model() {
    let resp = call_tool_response(
        "tool_availability_explain",
        json!({"tool": "runtime_diagnostics", "audience": "Model"}),
    );
    assert!(resp.ok);
    let result = resp.result.as_ref().unwrap();
    assert_eq!(result["exists"], true);
    assert_eq!(result["available_in_profile"], true);
    assert_eq!(result["callable_by_audience"], false);
    assert_eq!(result["exposure"], "harness_only");
    assert_eq!(result["suggested_audience"], "Harness");
}

#[test]
fn test_tool_availability_explain_unknown_tool() {
    let resp = call_tool_response(
        "tool_availability_explain",
        json!({"tool": "nonexistent_tool_xyz"}),
    );
    assert!(resp.ok);
    let result = resp.result.as_ref().unwrap();
    assert_eq!(result["exists"], false);
    assert_eq!(result["available_in_profile"], false);
    assert_eq!(result["callable_by_audience"], false);
}

#[test]
fn test_tool_availability_explain_tool_not_in_profile() {
    let resp = call_tool_response(
        "tool_availability_explain",
        json!({"tool": "text_equal", "profile": "human_math"}),
    );
    assert!(resp.ok);
    let result = resp.result.as_ref().unwrap();
    assert_eq!(result["exists"], true);
    assert_eq!(result["available_in_profile"], false);
    assert!(result["suggested_profile"].is_string());
}

#[test]
fn test_tool_availability_explain_harness_only_not_listed_to_model() {
    let model_tools = tools_for_profile_audience("full", ToolListAudience::Model);
    let has_tool_availability_explain = model_tools
        .iter()
        .any(|t| t.name == "tool_availability_explain");
    assert!(
        !has_tool_availability_explain,
        "tool_availability_explain should NOT appear in Model audience listing"
    );
}

#[test]
fn test_tool_availability_explain_missing_tool_arg() {
    let result = eggsact::tools::tool_availability_explain(&json!({}));
    assert!(!result.ok, "missing tool arg should fail");
    assert_eq!(
        result.machine_code.as_deref(),
        Some("INVALID_ARGUMENTS"),
        "should use INVALID_ARGUMENTS machine code"
    );
}
