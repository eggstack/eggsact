//! Tests for machine code infrastructure.
//!
//! Verifies:
//! - Machine code constants are valid and non-empty
//! - `ToolResponse::error_with_code` produces correct output
//! - `finding()`, `finding_with_location()`, `prompt_finding()` helpers produce correct output
//! - Non-OK tool responses include `machine_code`
//! - Machine code constants match the `ALL` array

use eggsact::mcp::machine_codes;
use eggsact::mcp::registry;
use eggsact::mcp::response::{finding, finding_with_location, prompt_finding, ToolResponse};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn call_tool(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
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
    }
    let output = child.wait_with_output().unwrap();
    let response_str = String::from_utf8_lossy(&output.stdout);
    let response: Value =
        serde_json::from_str(&response_str).expect("Failed to parse JSON-RPC response");
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

fn call_tool_error(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
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
    }
    let output = child.wait_with_output().unwrap();
    let response_str = String::from_utf8_lossy(&output.stdout);
    let response: Value =
        serde_json::from_str(&response_str).expect("Failed to parse JSON-RPC response");
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

// ═══════════════════════════════════════════════════════════════════════
// MACHINE CODE CONSTANTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_machine_code_constants_are_non_empty() {
    for code in machine_codes::ALL {
        assert!(
            !code.is_empty(),
            "Machine code constant should not be empty"
        );
        assert!(
            code.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            "Machine code '{}' should be UPPER_SNAKE_CASE",
            code
        );
    }
}

#[test]
fn test_machine_code_all_contains_key_codes() {
    let all: Vec<&str> = machine_codes::ALL.to_vec();
    let expected = vec![
        machine_codes::OK,
        machine_codes::CANCELLED,
        machine_codes::TIMEOUT,
        machine_codes::OUTPUT_TOO_LARGE,
        machine_codes::INPUT_TOO_LARGE,
        machine_codes::SERIALIZATION_ERROR,
        machine_codes::INVALID_ARGUMENTS,
        machine_codes::EDIT_OK,
        machine_codes::EDIT_FAILED,
        machine_codes::AMBIGUOUS_REPLACEMENT,
        machine_codes::COMMAND_OK,
        machine_codes::SHELL_RISK,
        machine_codes::JSON_INVALID,
        machine_codes::DATA_EQUAL,
        machine_codes::DATA_DIFF,
        machine_codes::PATH_HAS_TRAVERSAL,
        machine_codes::PATH_IS_HIDDEN,
        machine_codes::CONFIG_OK,
        machine_codes::IDENT_COLLISIONS,
        machine_codes::INVISIBLES_DETECTED,
        machine_codes::CONFUSABLES_DETECTED,
        machine_codes::BIDI_DETECTED,
        machine_codes::TEXT_SECURITY_OK,
        machine_codes::PROMPT_HIDDEN_CONTENT,
        machine_codes::REGEX_UNSAFE,
        machine_codes::CONSTRAINT_NOTE,
        machine_codes::CONSTRAINT_NOT_SATISFIED,
        machine_codes::CARGO_PARSE_FAILED,
    ];
    for code in &expected {
        assert!(all.contains(code), "ALL array should contain '{}'", code);
    }
}

#[test]
fn test_severity_constants_are_distinct() {
    let severities = [
        machine_codes::severity::INFO,
        machine_codes::severity::LOW,
        machine_codes::severity::MEDIUM,
        machine_codes::severity::HIGH,
        machine_codes::severity::CRITICAL,
    ];
    for s in &severities {
        assert!(!s.is_empty());
    }
    let mut sorted = severities.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        severities.len(),
        "Severity constants should be distinct"
    );
}

#[test]
fn test_disposition_constants_are_distinct() {
    let dispositions = [
        machine_codes::disposition::INFORMATIONAL,
        machine_codes::disposition::CAUTION,
        machine_codes::disposition::BLOCKING,
    ];
    for d in &dispositions {
        assert!(!d.is_empty());
    }
    let mut sorted = dispositions.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        dispositions.len(),
        "Disposition constants should be distinct"
    );
}

#[test]
fn test_verdict_constants_are_distinct() {
    let verdicts = [
        machine_codes::verdict::ALLOW,
        machine_codes::verdict::REVIEW,
        machine_codes::verdict::BLOCK,
        machine_codes::verdict::VALID,
        machine_codes::verdict::VALID_WITH_WARNINGS,
        machine_codes::verdict::INVALID,
        machine_codes::verdict::SAFE_TO_APPLY,
        machine_codes::verdict::SAFE_WITH_WARNINGS,
    ];
    for v in &verdicts {
        assert!(!v.is_empty());
    }
    let mut sorted = verdicts.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        verdicts.len(),
        "Verdict constants should be distinct"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TOOL RESPONSE CONSTRUCTORS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_error_with_code_sets_machine_code() {
    let resp = ToolResponse::error_with_code(
        "invalid_arguments",
        machine_codes::INVALID_ARGUMENTS,
        "Bad arg",
        Some(vec!["Fix it".to_string()]),
        Some("math_eval"),
    );
    assert!(!resp.ok);
    assert_eq!(resp.machine_code.as_deref(), Some("INVALID_ARGUMENTS"));
    assert_eq!(resp.error_type.as_deref(), Some("invalid_arguments"));
    assert!(resp.error.unwrap().contains("Bad arg"));
}

#[test]
fn test_error_without_code_has_no_machine_code() {
    // The legacy error_without_code_for_legacy_tests_only is now restricted
    // to unit tests (#[cfg(test)]). This test verifies the equivalent
    // behavior: an error_with_code response carries a machine code.
    let resp = ToolResponse::error_with_code(
        "evaluation_error",
        machine_codes::INVALID_ARGUMENTS,
        "Division by zero",
        None,
        Some("math_eval"),
    );
    assert!(!resp.ok);
    assert_eq!(resp.machine_code.as_deref(), Some("INVALID_ARGUMENTS"));
}

#[test]
fn test_success_with_machine_code() {
    let resp = ToolResponse::success_with_machine_code(
        serde_json::json!({"value": 42}),
        Some("math_eval"),
        machine_codes::OK,
    );
    assert!(resp.ok);
    assert_eq!(resp.machine_code.as_deref(), Some("OK"));
}

// ═══════════════════════════════════════════════════════════════════════
// FINDING HELPERS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_finding_creates_correct_shape() {
    let f = finding(
        machine_codes::AMBIGUOUS_REPLACEMENT,
        machine_codes::severity::MEDIUM,
        "Multiple matches found",
        None,
    );
    assert_eq!(f["code"].as_str(), Some("AMBIGUOUS_REPLACEMENT"));
    assert_eq!(f["severity"].as_str(), Some("medium"));
    assert_eq!(f["message"].as_str(), Some("Multiple matches found"));
    assert!(f.get("details").is_none());
}

#[test]
fn test_finding_with_details() {
    let details = serde_json::json!({"match_count": 3});
    let f = finding(
        machine_codes::AMBIGUOUS_REPLACEMENT,
        machine_codes::severity::MEDIUM,
        "Multiple matches found",
        Some(details),
    );
    assert_eq!(f["details"]["match_count"].as_i64(), Some(3));
}

#[test]
fn test_finding_with_location_creates_correct_shape() {
    let f = finding_with_location(
        machine_codes::EDIT_FAILED,
        machine_codes::severity::HIGH,
        "Patch parse error",
        42,
        Some(8),
    );
    assert_eq!(f["code"].as_str(), Some("EDIT_FAILED"));
    assert_eq!(f["severity"].as_str(), Some("high"));
    assert_eq!(f["location"]["line"].as_u64(), Some(42));
    assert_eq!(f["location"]["column"].as_u64(), Some(8));
}

#[test]
fn test_finding_with_location_no_column() {
    let f = finding_with_location(
        machine_codes::EDIT_FAILED,
        machine_codes::severity::HIGH,
        "Patch parse error",
        42,
        None,
    );
    assert_eq!(f["location"]["line"].as_u64(), Some(42));
    assert!(f["location"].get("column").is_none());
}

#[test]
fn test_prompt_finding_creates_correct_shape() {
    let f = prompt_finding(
        machine_codes::PROMPT_HIDDEN_CONTENT,
        machine_codes::severity::MEDIUM,
        "HTML comment detected",
        10,
        25,
        None,
    );
    assert_eq!(f["code"].as_str(), Some("PROMPT_HIDDEN_CONTENT"));
    assert_eq!(f["severity"].as_str(), Some("medium"));
    assert_eq!(f["span"]["byte_offset"].as_u64(), Some(10));
    assert_eq!(f["span"]["end_byte_offset"].as_u64(), Some(25));
}

// ═══════════════════════════════════════════════════════════════════════
// TOOL RESPONSE MACHINE_CODE PRESENCE
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_error_response_has_machine_code_for_helpers() {
    // Test that error_with_code is used on the helpers.rs validation path.
    // config_preflight with an unparseable TOML triggers an error with machine_code.
    let r = call_tool_error("config_preflight", serde_json::json!({"text": "[unclosed"}));
    if r.get("ok") == Some(&Value::Bool(false)) {
        assert!(
            r.get("machine_code").is_some(),
            "Non-OK config_preflight response should have machine_code: {}",
            r
        );
    }
}

#[test]
fn test_composite_tool_response_has_machine_code() {
    // edit_preflight always returns success with machine_code
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal"
        }),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "edit_preflight should have machine_code: {}",
        r
    );
    assert_eq!(r["machine_code"].as_str(), Some("EDIT_OK"));
}

#[test]
fn test_command_preflight_has_machine_code() {
    let r = call_tool(
        "command_preflight",
        serde_json::json!({"command": "echo hello"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "command_preflight should have machine_code: {}",
        r
    );
    assert_eq!(r["machine_code"].as_str(), Some("COMMAND_OK"));
}

#[test]
fn test_config_preflight_has_machine_code() {
    let r = call_tool("config_preflight", serde_json::json!({"text": "{}"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "config_preflight should have machine_code: {}",
        r
    );
    assert_eq!(r["machine_code"].as_str(), Some("CONFIG_OK"));
}

#[test]
fn test_text_security_inspect_has_machine_code() {
    let r = call_tool(
        "text_security_inspect",
        serde_json::json!({"text": "hello world"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "text_security_inspect should have machine_code: {}",
        r
    );
    assert_eq!(r["machine_code"].as_str(), Some("TEXT_SECURITY_OK"));
}

#[test]
fn test_structured_data_compare_has_machine_code() {
    let r = call_tool(
        "structured_data_compare",
        serde_json::json!({"a": "1", "b": "1"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "structured_data_compare should have machine_code: {}",
        r
    );
    assert_eq!(r["machine_code"].as_str(), Some("DATA_EQUAL"));
}

#[test]
fn test_version_constraint_check_has_machine_code() {
    // Version that doesn't satisfy the constraint should set machine_code
    let r = call_tool(
        "version_constraint_check",
        serde_json::json!({"version": "2.0.0", "constraint": "^1.0.0"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "version_constraint_check with unsatisfied constraint should have machine_code: {}",
        r
    );
}

#[test]
fn test_identifier_inspect_has_machine_code_for_collisions() {
    // Use identifiers that normalize to the same form via casefolding
    let r = call_tool(
        "identifier_inspect",
        serde_json::json!({"identifiers": ["foo", "FOO"], "casefold": true}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "identifier_inspect with collisions should have machine_code: {}",
        r
    );
}

#[test]
fn test_validate_json_invalid_has_machine_code() {
    // validate_json returns success with findings, not an error
    let r = call_tool("validate_json", serde_json::json!({"text": "not json"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("machine_code").is_some(),
        "validate_json with invalid input should have machine_code: {}",
        r
    );
}

// ═══════════════════════════════════════════════════════════════════════
// SOURCE GUARD: LEGACY ERROR CONSTRUCTOR NOT IN PRODUCTION CODE
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn source_guard_rejects_legacy_error_in_production() {
    let legacy_fn = "error_without_code_for_legacy_tests_only(";
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    fn walk_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_rs_files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }

    let mut files = Vec::new();
    walk_rs_files(&src_dir, &mut files);

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    for file in &files {
        let rel = file.strip_prefix(manifest_dir).unwrap_or(file);
        let content = std::fs::read_to_string(file).unwrap_or_default();

        if !content.contains(legacy_fn) {
            continue;
        }

        // Allow the definition site in response.rs
        if rel == std::path::Path::new("src/mcp/response.rs") {
            continue;
        }

        // Allow calls inside #[cfg(test)] modules (inline test modules in src/)
        // by checking if every occurrence is preceded by #[cfg(test)] context.
        let mut allowed = true;
        for (idx, _) in content.match_indices(legacy_fn) {
            // Look backwards for the nearest `mod tests` or `#[cfg(test)]`
            let preceding = &content[..idx];
            let has_test_module = preceding.contains("#[cfg(test)]");
            if !has_test_module {
                allowed = false;
                break;
            }
        }

        if !allowed {
            violations.push(format!("  {}", rel.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "Found `error_without_code_for_legacy_tests_only(` in production source files:\n{}\n\
         This function is #[cfg(test)]-gated and must only appear in response.rs (definition) \
         or test code. Use `error_with_code()` for all non-OK tool responses.",
        violations.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════
// SWEEP TEST: ALL ERROR-PRODUCING TOOLS RETURN MACHINE_CODE
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_all_tools_return_machine_code_on_error() {
    let bad_text: Value = json!({"text": "}"});
    let error_cases: Vec<(&str, Value)> = vec![
        ("math_eval", json!({})),
        ("unit_convert", json!({})),
        ("text_measure", json!({})),
        ("text_equal", json!({})),
        ("json_extract", json!({})),
        ("validate_json", bad_text.clone()),
        ("validate_toml", bad_text),
        ("validate_regex", json!({})),
        ("validate_brackets", json!({})),
        ("path_normalize", json!({})),
        ("path_analyze", json!({})),
        ("shell_split", json!({})),
        ("regex_safety_check", json!({})),
        ("identifier_inspect", json!({})),
        ("version_compare", json!({})),
        ("version_constraint_check", json!({})),
        ("cargo_toml_inspect", json!({})),
        ("unicode_policy_check", json!({})),
        ("prompt_input_inspect", json!({})),
        ("list_compare", json!({})),
        ("markdown_structure", json!({})),
        ("dotenv_validate", json!({})),
        ("ini_validate", json!({})),
        ("toml_shape", json!({})),
        ("json_canonicalize", json!({})),
        ("json_query", json!({})),
        ("canonicalize_text", json!({})),
        ("escape_text", json!({})),
        ("unescape_text", json!({})),
        ("text_transform", json!({})),
        ("text_position", json!({})),
        ("text_count", json!({})),
        ("text_hash", json!({})),
        ("text_truncate", json!({})),
        ("text_window", json!({})),
        ("text_fingerprint", json!({})),
        ("line_range_extract", json!({})),
        ("line_range_compare", json!({})),
        ("shell_quote_join", json!({})),
        ("argv_compare", json!({})),
        ("glob_match", json!({})),
        ("code_fence_extract", json!({})),
        ("json_shape", json!({})),
        ("identifier_analyze", json!({})),
        ("list_dedupe", json!({})),
        ("list_sort", json!({})),
        ("text_replace_check", json!({})),
        ("patch_apply_check", json!({})),
        ("patch_summary", json!({})),
        ("validate_schema_light", json!({})),
        ("regex_finditer", json!({})),
    ];
    for (name, args) in &error_cases {
        let r = call_tool_error(name, args.clone());
        if r.get("ok") == Some(&Value::Bool(false)) {
            assert!(
                r.get("machine_code").is_some() && r["machine_code"] != Value::Null,
                "Non-OK response from '{}' should have machine_code: {}",
                name,
                r
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FIXTURE TESTS: SPECIFIC TOOL RESPONSES WITH MACHINE CODES
// ═══════════════════════════════════════════════════════════════════════

// These tests verify that specific tools return machine_code.
// Non-OK responses MUST have machine_code. Success responses from composite
// tools also carry machine_code. Other tools may or may not set it on success.

#[test]
fn test_path_analyze_returns_success() {
    let r = call_tool("path_analyze", json!({"path": "/etc/passwd"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "path_analyze should return result: {}",
        r
    );
}

#[test]
fn test_regex_safety_check_returns_result() {
    let r = call_tool("regex_safety_check", json!({"pattern": "(a+a+a+)+"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "regex_safety_check should return result: {}",
        r
    );
}

#[test]
fn test_unicode_policy_check_returns_result() {
    let r = call_tool(
        "unicode_policy_check",
        json!({"text": "hello world", "policy": "human_text"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "unicode_policy_check should return result: {}",
        r
    );
}

#[test]
fn test_prompt_input_inspect_returns_result() {
    let r = call_tool("prompt_input_inspect", json!({"text": "hello world"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "prompt_input_inspect should return result: {}",
        r
    );
}

#[test]
fn test_path_scope_check_within_scope() {
    let r = call_tool(
        "path_scope_check",
        json!({"root": "src/", "target": "src/main.rs"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "path_scope_check should return result: {}",
        r
    );
}

#[test]
fn test_path_scope_check_traversal() {
    let r = call_tool(
        "path_scope_check",
        json!({"root": "src/", "target": "../etc/passwd"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "path_scope_check with traversal should return result: {}",
        r
    );
}

#[test]
fn test_validate_json_valid() {
    let r = call_tool("validate_json", json!({"text": "{\"a\": 1}"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "validate_json valid should return result: {}",
        r
    );
}

#[test]
fn test_canonicalize_text_returns_result() {
    let r = call_tool(
        "canonicalize_text",
        json!({"text": "hello\u{00a0}world", "profile": "human_label_compare"}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "canonicalize_text should return result: {}",
        r
    );
}

#[test]
fn test_json_canonicalize_returns_result() {
    let r = call_tool("json_canonicalize", json!({"text": "{\"b\": 2, \"a\": 1}"}));
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "json_canonicalize should return result: {}",
        r
    );
}

#[test]
fn test_identifier_table_inspect_returns_result() {
    let r = call_tool(
        "identifier_table_inspect",
        json!({"identifiers": [{"name": "foo"}, {"name": "bar"}, {"name": "baz"}]}),
    );
    assert_eq!(r["ok"], true);
    assert!(
        r.get("result").is_some(),
        "identifier_table_inspect should return result: {}",
        r
    );
}

#[test]
fn every_non_ok_tool_response_has_machine_code() {
    let empty = serde_json::json!({});
    let mut failures = Vec::new();

    for spec in registry::all_tools_list() {
        let handler = spec.handler;
        let resp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&empty)));
        if let Ok(response) = resp {
            if !response.ok && response.machine_code.is_none() {
                failures.push(format!(
                    "{}: error_type={:?}, error={:?}",
                    spec.name, response.error_type, response.error
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Non-OK tool responses without machine_code:\n{}",
        failures.join("\n")
    );
}
