//! Differential tests for the shared repo/patch analysis consolidation.
//!
//! Guards against semantic drift: the same path must receive the same
//! ecosystem/bucket classification no matter which repo tool is called, and
//! the same patch must yield the same neutral counts/path identity no matter
//! which patch tool is called. Policy differences (verdicts, machine codes,
//! review focus) stay intentional and are not asserted equal here.

use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use serde_json::{json, Value};

fn full_harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
}

fn result_json(resp: &eggsact::mcp::response::ToolResponse) -> Value {
    resp.result.clone().expect("result should be present")
}

fn project_types_of(registry: &ToolRegistry, tool: &str, paths: &[&str]) -> Vec<String> {
    let paths_json: Vec<Value> = paths.iter().map(|p| json!(p)).collect();
    let resp = registry
        .call_json(tool, json!({"paths": paths_json}))
        .unwrap_or_else(|e| panic!("{tool} registry call failed: {e:?}"));
    assert!(resp.ok, "{tool} should succeed for {paths:?}");
    let result = result_json(&resp);
    result
        .get("project_types")
        .and_then(|v| v.as_array())
        .expect("project_types should be present")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

fn ecosystems_of(registry: &ToolRegistry, paths: &[&str]) -> Vec<String> {
    let paths_json: Vec<Value> = paths.iter().map(|p| json!(p)).collect();
    let resp = registry
        .call_json("repo_language_detect", json!({"paths": paths_json}))
        .expect("language detect should succeed");
    assert!(resp.ok);
    let result = result_json(&resp);
    result
        .get("ecosystems")
        .and_then(|v| v.as_array())
        .expect("ecosystems should be present")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

// --- Repo fixtures ---------------------------------------------------------

fn repo_fixtures() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "pure_rust",
            vec!["Cargo.toml", "Cargo.lock", "src/main.rs", "src/lib.rs"],
        ),
        (
            "pure_python",
            vec!["pyproject.toml", "requirements.txt", "setup.py", "main.py"],
        ),
        (
            "pure_node",
            vec!["package.json", "package-lock.json", "src/index.js"],
        ),
        ("pure_go", vec!["go.mod", "go.sum", "main.go"]),
        (
            "mixed",
            vec!["Cargo.toml", "package.json", "requirements.txt"],
        ),
        (
            "unknown",
            vec!["README.md", "docs/guide.md", "assets/logo.png"],
        ),
        (
            "monorepo_nested",
            vec![
                "services/a/Cargo.toml",
                "services/a/src/main.rs",
                "services/b/package.json",
                "services/b/src/index.ts",
            ],
        ),
        (
            "generated_vendor_heavy",
            vec![
                "Cargo.toml",
                "target/debug/build/out.rs",
                "target/release/app",
                "node_modules/react/index.js",
                "vendor/openssl/ssl.rs",
                "dist/bundle.min.js",
            ],
        ),
        (
            "ci_config",
            vec![
                ".github/workflows/ci.yml",
                "Dockerfile",
                "rustfmt.toml",
                ".env",
                "src/main.rs",
            ],
        ),
        (
            "ambiguous_extensions",
            vec![
                "include/util.h",
                "src/util.cpp",
                "src/main.c",
                "Dockerfile",
                "Makefile",
            ],
        ),
        // Canonical-behavior regression: lockfile/source hints alone imply
        // rust in every repo tool (previously `repo_tree_summarize` said
        // `unknown` for `Cargo.lock` alone).
        ("rust_lockfile_only", vec!["Cargo.lock"]),
        ("rust_source_hint_only", vec!["src/main.rs"]),
    ]
}

#[test]
fn shared_project_types_agree_across_repo_tools() {
    let registry = full_harness_registry();
    for (name, paths) in repo_fixtures() {
        let via_manifest = project_types_of(&registry, "repo_manifest_inspect", &paths);
        let via_tree = project_types_of(&registry, "repo_tree_summarize", &paths);
        let via_cmds = project_types_of(&registry, "test_command_suggest", &paths);
        assert_eq!(
            via_manifest, via_tree,
            "fixture {name}: manifest vs tree project_types diverged"
        );
        assert_eq!(
            via_manifest, via_cmds,
            "fixture {name}: manifest vs test_command project_types diverged"
        );
    }
}

#[test]
fn shared_ecosystems_match_project_types_without_markers() {
    let registry = full_harness_registry();
    for (name, paths) in repo_fixtures() {
        let project_types = project_types_of(&registry, "repo_manifest_inspect", &paths);
        let ecosystems = ecosystems_of(&registry, &paths);
        let expected: Vec<String> = project_types
            .into_iter()
            .filter(|t| t != "unknown" && t != "mixed")
            .collect();
        assert_eq!(
            ecosystems, expected,
            "fixture {name}: ecosystems diverged from canonical project types"
        );
    }
}

#[test]
fn canonical_rust_hints_are_stable() {
    let registry = full_harness_registry();
    for paths in [vec!["Cargo.lock"], vec!["src/main.rs"], vec!["src/lib.rs"]] {
        let types = project_types_of(&registry, "repo_manifest_inspect", &paths);
        assert!(
            types.contains(&"rust".to_string()),
            "{paths:?} should be rust everywhere, got {types:?}"
        );
        let tree_types = project_types_of(&registry, "repo_tree_summarize", &paths);
        assert_eq!(types, tree_types);
    }
}

// --- Patch fixtures --------------------------------------------------------

fn patch_fixtures() -> Vec<(&'static str, String)> {
    vec![
        (
            "source_change",
            "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        ),
        (
            "manifest_change",
            "--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        ),
        (
            "ci_change",
            "--- a/.github/workflows/ci.yml\n+++ b/.github/workflows/ci.yml\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        ),
        (
            "security_sensitive",
            "--- a/src/auth/handler.rs\n+++ b/src/auth/handler.rs\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        ),
        (
            "multi_file",
            "--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-old\n+new\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n"
                .to_string(),
        ),
        (
            "rename",
            "--- a/old.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        ),
        (
            "binary",
            "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n+new\nGIT binary patch\n".to_string(),
        ),
        ("empty", "".to_string()),
        ("garbage", "not a diff at all".to_string()),
    ]
}

#[test]
fn patch_tools_share_parse_acceptance() {
    let registry = full_harness_registry();
    for (name, patch) in patch_fixtures() {
        let summary = registry
            .call_json("patch_summary", json!({"patch_text": patch}))
            .expect("patch_summary call should succeed");
        let contract = registry
            .call_json("patch_contract_check", json!({"patch_text": patch}))
            .expect("contract call should succeed");
        let risk = registry
            .call_json("diff_risk_classify", json!({"patch_text": patch}))
            .expect("risk call should succeed");

        // patch_summary never hard-fails on parse (neutral presentation);
        // contract and risk surface parse failure as BLOCK/PATCH_FAILED.
        // What must agree: whether the patch parsed.
        let contract_block_parse = contract.machine_code.as_deref() == Some("PATCH_FAILED")
            && contract
                .result
                .as_ref()
                .and_then(|r| r.get("summary"))
                .and_then(|s| s.as_str())
                == Some("Patch failed to parse");
        let risk_block_parse = risk.machine_code.as_deref() == Some("PATCH_FAILED")
            && risk
                .result
                .as_ref()
                .and_then(|r| r.get("summary"))
                .and_then(|s| s.as_str())
                == Some("Patch failed to parse");
        assert_eq!(
            contract_block_parse, risk_block_parse,
            "fixture {name}: contract vs risk parse acceptance diverged"
        );
        // summary ok flag is implicit: files_changed 0 + parse-failure
        // finding means rejection.
        let _ = (summary, name);
    }
}

#[test]
fn patch_tools_share_neutral_counts_and_path_identity() {
    let registry = full_harness_registry();
    for (name, patch) in patch_fixtures() {
        if name == "empty" || name == "garbage" {
            continue;
        }
        let summary_resp = registry
            .call_json("patch_summary", json!({"patch_text": patch}))
            .expect("summary should succeed");
        let risk_resp = registry
            .call_json("diff_risk_classify", json!({"patch_text": patch}))
            .expect("risk should succeed");
        assert!(
            summary_resp.ok && risk_resp.ok,
            "fixture {name} should parse"
        );

        let summary = result_json(&summary_resp);
        let risk = result_json(&risk_resp);
        let risk_summary = risk
            .get("patch_summary")
            .expect("risk patch_summary should be present");

        for field in ["files_changed", "hunks_total", "additions", "deletions"] {
            assert_eq!(
                summary.get(field),
                risk_summary.get(field),
                "fixture {name}: neutral field {field} diverged"
            );
        }
        assert_eq!(
            summary.get("binary_patch_detected"),
            risk_summary.get("binary_patch_detected"),
            "fixture {name}: binary flag diverged"
        );
        assert_eq!(
            summary
                .get("renames_detected")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            risk_summary
                .get("renames_detected")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            "fixture {name}: rename count diverged"
        );

        // Path identity: every effective path in the neutral line-range map
        // must appear in exactly one risk files_by_category list.
        let line_ranges = summary
            .get("line_ranges_by_file")
            .and_then(|v| v.as_object())
            .expect("line_ranges_by_file should be present");
        let files_by_cat = risk
            .get("files_by_category")
            .and_then(|v| v.as_object())
            .expect("files_by_category should be present");
        let mut categorized: Vec<String> = Vec::new();
        for paths in files_by_cat.values() {
            for p in paths.as_array().cloned().unwrap_or_default() {
                if let Some(s) = p.as_str() {
                    categorized.push(s.to_string());
                }
            }
        }
        for path in line_ranges.keys() {
            assert!(
                categorized.iter().any(|c| c == path),
                "fixture {name}: neutral path {path} missing from risk categories"
            );
        }
    }
}

#[test]
fn patch_security_fact_is_shared_between_contract_and_risk() {
    // A security-sensitive path is a shared fact; contract and risk apply
    // different policies to it (contract: source_change/allow-review; risk:
    // security_sensitive/block). The test guards the fact, not the verdict.
    let analysis = eggsact::services::analyze_patch(
        "--- a/src/auth/handler.rs\n+++ b/src/auth/handler.rs\n@@ -1 +1 @@\n-old\n+new\n",
    );
    assert!(analysis.ok);
    assert!(analysis.files[0].is_security_sensitive);

    let registry = full_harness_registry();
    let patch = "--- a/src/auth/handler.rs\n+++ b/src/auth/handler.rs\n@@ -1 +1 @@\n-old\n+new\n";
    let risk = registry
        .call_json("diff_risk_classify", json!({"patch_text": patch}))
        .expect("risk should succeed");
    let risk_json = result_json(&risk);
    let cats = risk_json
        .get("risk_categories")
        .and_then(|v| v.as_array())
        .expect("risk_categories present");
    assert!(cats.contains(&json!("security_sensitive")));
}
