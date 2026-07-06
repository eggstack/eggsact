use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use serde_json::{json, Value};

// ═══════════════════════════════════════════════════════════════════════════════
// repo_tree_summarize
// ═══════════════════════════════════════════════════════════════════════════════

fn repo_audit_harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::CodeggRepoAudit, ToolAudience::Harness)
}

fn result_json(resp: &eggsact::mcp::response::ToolResponse) -> Value {
    resp.result.clone().expect("result should be present")
}

#[test]
fn repo_tree_summarize_empty_paths_succeeds() {
    let registry = repo_audit_harness_registry();
    let resp = registry
        .call_json("repo_tree_summarize", json!({"paths": []}))
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = resp.result.expect("result should be present");
    assert_eq!(result.get("path_count").unwrap(), 0);
    assert!(result.get("machine_code").is_some());
    assert!(result.get("verdict").is_some());
}

#[test]
fn repo_tree_summarize_single_path_with_file_sizes() {
    let registry = repo_audit_harness_registry();
    let resp = registry
        .call_json(
            "repo_tree_summarize",
            json!({
                "paths": ["Cargo.toml"],
                "file_sizes": {"Cargo.toml": 1024}
            }),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = result_json(&resp);
    assert_eq!(result.get("path_count").unwrap(), 1);
    assert!(result.get("machine_code").is_some());
    assert!(result.get("verdict").is_some());
    let project_types = result.get("project_types").unwrap().as_array().unwrap();
    assert!(project_types.contains(&json!("rust")));
}

#[test]
fn repo_tree_summarize_max_paths_cap() {
    let registry = repo_audit_harness_registry();
    let paths: Vec<String> = (0..1500).map(|i| format!("file_{}.txt", i)).collect();
    let resp = registry
        .call_json("repo_tree_summarize", json!({"paths": paths}))
        .expect("registry call should succeed");
    assert!(!resp.ok);
    let error = resp.error.expect("error should be present");
    assert!(
        error.contains("max_paths") || error.contains("exceeds"),
        "Error should mention max_paths cap: {error}"
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("INPUT_TOO_LARGE"),
        "machine_code should be INPUT_TOO_LARGE"
    );
}

#[test]
fn repo_tree_summarize_finds_and_machine_code_present() {
    let registry = repo_audit_harness_registry();
    let resp = registry
        .call_json(
            "repo_tree_summarize",
            json!({
                "paths": ["Cargo.toml", "package.json", "requirements.txt"],
            }),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    assert!(
        resp.findings.is_some(),
        "findings should be present for mixed repo"
    );
    let result = resp.result.expect("result should be present");
    assert!(result.get("machine_code").is_some());
    assert!(result.get("verdict").is_some());
}

#[test]
fn repo_tree_summarize_verdict_allow_for_known_project() {
    let registry = repo_audit_harness_registry();
    let resp = registry
        .call_json(
            "repo_tree_summarize",
            json!({"paths": ["Cargo.toml", "Cargo.lock", "src/main.rs"]}),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = result_json(&resp);
    let verdict = result.get("verdict").unwrap().as_str().unwrap();
    assert_eq!(verdict, "allow");
}

// ═══════════════════════════════════════════════════════════════════════════════
// diff_risk_classify
// ═══════════════════════════════════════════════════════════════════════════════

fn full_model_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model)
}

#[test]
fn diff_risk_classify_missing_patch_text_rejected() {
    let registry = full_model_registry();
    let result = registry.call_json("diff_risk_classify", json!({}));
    assert!(
        result.is_err(),
        "Missing patch_text should cause registry-level error or tool rejection"
    );
}

#[test]
fn diff_risk_classify_oversized_patch_text_rejected() {
    let registry = full_model_registry();
    let oversized = "a".repeat(100_001);
    let resp = registry
        .call_json("diff_risk_classify", json!({"patch_text": oversized}))
        .expect("registry call should succeed");
    assert!(!resp.ok);
    let error = resp.error.expect("error should be present");
    assert!(
        error.contains("exceeds") || error.contains("INPUT_TOO_LARGE"),
        "Error should mention input too large: {error}"
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("INPUT_TOO_LARGE"),
        "machine_code should be INPUT_TOO_LARGE"
    );
}

#[test]
fn diff_risk_classify_small_diff_classifies() {
    let registry = full_model_registry();
    let patch = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";
    let resp = registry
        .call_json("diff_risk_classify", json!({"patch_text": patch}))
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = resp.result.expect("result should be present");
    assert!(result.get("machine_code").is_some());
    assert!(result.get("verdict").is_some());
}

#[test]
fn diff_risk_classify_security_sensitive_diff_returns_block() {
    let registry = full_model_registry();
    let patch = "--- a/src/auth/handler.rs\n+++ b/src/auth/handler.rs\n@@ -1 +1 @@\n-old\n+new\n";
    let resp = registry
        .call_json("diff_risk_classify", json!({"patch_text": patch}))
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = resp.result.expect("result should be present");
    let verdict = result.get("verdict").unwrap().as_str().unwrap();
    assert_eq!(verdict, "block");
    assert_eq!(
        result.get("machine_code").unwrap().as_str().unwrap(),
        "DIFF_RISK_BLOCK"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// path_batch_scope_check
// ═══════════════════════════════════════════════════════════════════════════════

fn preflight_harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::CodeggPreflight, ToolAudience::Harness)
}

#[test]
fn path_batch_scope_check_missing_root_rejected() {
    let registry = preflight_harness_registry();
    let result = registry.call_json(
        "path_batch_scope_check",
        json!({"targets": ["src/main.rs"]}),
    );
    assert!(
        result.is_err(),
        "Missing root should cause registry-level error"
    );
}

#[test]
fn path_batch_scope_check_missing_targets_rejected() {
    let registry = preflight_harness_registry();
    let result = registry.call_json("path_batch_scope_check", json!({"root": "/workspace"}));
    assert!(
        result.is_err(),
        "Missing targets should cause registry-level error"
    );
}

#[test]
fn path_batch_scope_check_in_scope_targets_allow() {
    let registry = preflight_harness_registry();
    let resp = registry
        .call_json(
            "path_batch_scope_check",
            json!({
                "root": "/workspace",
                "targets": ["src/main.rs", "src/lib.rs"]
            }),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = resp.result.expect("result should be present");
    assert_eq!(result.get("verdict").unwrap().as_str().unwrap(), "allow");
    assert_eq!(
        result.get("machine_code").unwrap().as_str().unwrap(),
        "PATH_BATCH_OK"
    );
    assert_eq!(result.get("all_inside_root").unwrap(), true);
}

#[test]
fn path_batch_scope_check_out_of_scope_targets_review() {
    let registry = preflight_harness_registry();
    let resp = registry
        .call_json(
            "path_batch_scope_check",
            json!({
                "root": "/workspace",
                "targets": ["../escape/main.rs"]
            }),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = result_json(&resp);
    assert_eq!(result.get("verdict").unwrap().as_str().unwrap(), "review");
    assert_eq!(
        result.get("machine_code").unwrap().as_str().unwrap(),
        "PATH_BATCH_REVIEW"
    );
    assert!(
        !result
            .get("findings")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty(),
        "findings should be present for escaping target"
    );
}

#[test]
fn path_batch_scope_check_max_targets_cap() {
    let registry = preflight_harness_registry();
    let targets: Vec<String> = (0..1500).map(|i| format!("file_{}.txt", i)).collect();
    let resp = registry
        .call_json(
            "path_batch_scope_check",
            json!({"root": "/workspace", "targets": targets}),
        )
        .expect("registry call should succeed");
    assert!(!resp.ok);
    let error = resp.error.expect("error should be present");
    assert!(
        error.contains("max_targets") || error.contains("exceeds"),
        "Error should mention max_targets cap: {error}"
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("INPUT_TOO_LARGE"),
        "machine_code should be INPUT_TOO_LARGE"
    );
}

#[test]
fn path_batch_scope_check_posix_platform_default() {
    let registry = preflight_harness_registry();
    let resp = registry
        .call_json(
            "path_batch_scope_check",
            json!({
                "root": "/workspace",
                "targets": ["src/main.rs"],
                "platform": "posix"
            }),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = resp.result.expect("result should be present");
    assert_eq!(result.get("verdict").unwrap().as_str().unwrap(), "allow");
}

#[test]
fn path_batch_scope_check_empty_targets_list() {
    let registry = preflight_harness_registry();
    let resp = registry
        .call_json(
            "path_batch_scope_check",
            json!({"root": "/workspace", "targets": []}),
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
    let result = resp.result.expect("result should be present");
    assert_eq!(result.get("verdict").unwrap().as_str().unwrap(), "allow");
    assert_eq!(result.get("targets_checked").unwrap(), 0);
}
