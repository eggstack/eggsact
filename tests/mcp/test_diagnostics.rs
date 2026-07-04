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
