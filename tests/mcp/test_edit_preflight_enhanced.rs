//! Integration tests for enhanced edit_preflight (Phase 6).
//!
//! Tests path scope, newline check, unicode check, fingerprint, metadata,
//! and full composition through the MCP server.

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
        stdin.write_all(request.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn call_tool(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
        "id": 1
    })
    .to_string();
    let response_str = mcp_request(&request);
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
// PATH SCOPE
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_path_scope_safe() {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "file_path": "src/main.rs",
            "workspace_root": cwd
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], true);
    let ps = r["result"]["path_scope"].as_object().unwrap();
    assert_eq!(ps["inside_root"], true);
    assert_eq!(ps["escapes_via_dotdot"], false);
}

#[test]
fn test_edit_preflight_path_scope_escape_blocks() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "file_path": "../etc/passwd",
            "workspace_root": "/workspace"
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], false);
    let mc = r["result"]["machine_code"].as_str().unwrap();
    assert!(
        mc.contains("PATH_HAS_TRAVERSAL"),
        "Expected PATH_HAS_TRAVERSAL, got: {}",
        mc
    );
    let ps = r["result"]["path_scope"].as_object().unwrap();
    assert_eq!(ps["escapes_via_dotdot"], true);
}

#[test]
fn test_edit_preflight_no_path_scope_without_workspace_root() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "file_path": "src/main.rs"
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], true);
    assert!(r["result"].get("path_scope").is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// NEWLINE CHECK
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_newline_check_detects_mixed() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "line1\nline2\r\nline3\n",
            "old": "line1",
            "new": "LINE1",
            "replacement_mode": "literal",
            "newline_policy": "check"
        }),
    );
    assert_eq!(r["ok"], true);
    let nc = r["result"]["newline_check"].as_object().unwrap();
    assert_eq!(nc["mixed"], true);
}

#[test]
fn test_edit_preflight_newline_skip_by_default() {
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
    assert!(r["result"].get("newline_check").is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// UNICODE CHECK
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_unicode_check_default_policy() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "unicode_policy": "default"
        }),
    );
    assert_eq!(r["ok"], true);
    let uc = r["result"]["unicode_check"].as_object().unwrap();
    assert_eq!(uc["verdict"], "allow");
}

#[test]
fn test_edit_preflight_unicode_skip_by_default() {
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
    assert!(r["result"].get("unicode_check").is_none());
}

#[test]
fn test_edit_preflight_unicode_source_code_clean_text() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "fn main() {}",
            "old": "fn main() {}",
            "new": "fn main() { println!(\"hello\"); }",
            "replacement_mode": "literal",
            "unicode_policy": "source_code"
        }),
    );
    assert_eq!(r["ok"], true);
    let uc = r["result"]["unicode_check"].as_object().unwrap();
    let verdict = uc["verdict"].as_str().unwrap();
    assert!(
        verdict == "allow" || verdict == "review",
        "Expected allow or review for clean ASCII, got: {}",
        verdict
    );
}

// ═══════════════════════════════════════════════════════════════════════
// FINGERPRINT
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_fingerprint_literal_match() {
    let fp_resp = call_tool(
        "text_fingerprint",
        serde_json::json!({"text": "hello world"}),
    );
    let expected = fp_resp["result"]["sha256"].as_str().unwrap();

    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "expected_fingerprint": expected
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], true);
    let fp = r["result"]["fingerprint"].as_object().unwrap();
    assert_eq!(fp["sha256"].as_str().unwrap(), expected);
}

#[test]
fn test_edit_preflight_fingerprint_literal_mismatch() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "expected_fingerprint": "0000000000000000000000000000000000000000000000000000000000000000"
        }),
    );
    assert_eq!(r["ok"], true);
    let findings = r["result"]["findings"].as_array().unwrap();
    let has_fp_mismatch = findings.iter().any(|f| f["code"] == "FINGERPRINT_MISMATCH");
    assert!(has_fp_mismatch, "Expected FINGERPRINT_MISMATCH finding");
}

#[test]
fn test_edit_preflight_fingerprint_in_result_when_provided() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "expected_fingerprint": "deadbeef"
        }),
    );
    assert_eq!(r["ok"], true);
    let fp = r["result"]["fingerprint"].as_object().unwrap();
    assert!(fp.get("sha256").is_some());
    assert!(fp.get("newline_style").is_some());
}

// ═══════════════════════════════════════════════════════════════════════
// EDIT METADATA
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_metadata_accepted() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "edit_metadata": {
                "description": "rename variable",
                "author": "test-agent"
            }
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], true);
}

// ═══════════════════════════════════════════════════════════════════════
// FULL COMPOSITION
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_full_composition_safe() {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let fp_resp = call_tool(
        "text_fingerprint",
        serde_json::json!({"text": "hello world"}),
    );
    let expected = fp_resp["result"]["sha256"].as_str().unwrap();

    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "file_path": "src/main.rs",
            "workspace_root": cwd,
            "newline_policy": "check",
            "unicode_policy": "default",
            "expected_fingerprint": expected,
            "edit_metadata": {"description": "test"}
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], true);
    assert!(r["result"].get("path_scope").is_some());
    assert!(r["result"].get("newline_check").is_some());
    assert!(r["result"].get("unicode_check").is_some());
    assert!(r["result"].get("fingerprint").is_some());
}

#[test]
fn test_edit_preflight_full_composition_path_escape_blocks() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal",
            "file_path": "../etc/passwd",
            "workspace_root": "/workspace",
            "newline_policy": "check",
            "unicode_policy": "default"
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], false);
    let findings = r["result"]["findings"].as_array().unwrap();
    let has_scope = findings.iter().any(|f| f["code"] == "PATH_SCOPE_ESCAPE");
    assert!(has_scope, "Expected PATH_SCOPE_ESCAPE finding");
}

// ═══════════════════════════════════════════════════════════════════════
// BACKWARD COMPAT
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_backward_compat_no_new_fields() {
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
    assert_eq!(r["result"]["ok_to_apply"], true);
    assert!(r["result"].get("path_scope").is_none());
    assert!(r["result"].get("newline_check").is_none());
    assert!(r["result"].get("unicode_check").is_none());
    assert!(r["result"].get("fingerprint").is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// PATCH MODE FINGERPRINT
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_edit_preflight_patch_fingerprint_match() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "line1\nold\nline3\n",
            "patch": "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3\n",
            "replacement_mode": "patch",
            "expected_fingerprint": "dummy_will_mismatch"
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["ok_to_apply"], true);
    let fp = r["result"]["fingerprint"].as_object().unwrap();
    let sha = fp["sha256"].as_str().unwrap();
    assert!(!sha.is_empty(), "fingerprint sha256 should be non-empty");
    let pa_fp = r["result"]["subresults"]["patch_apply_check"]["result_fingerprint"]
        .as_str()
        .unwrap();
    assert_eq!(
        sha, pa_fp,
        "fingerprint in output must match subresult's result_fingerprint"
    );
}

#[test]
fn test_edit_preflight_patch_fingerprint_mismatch() {
    let r = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "patch": "--- a/file\n+++ b/file\n@@ -1 +1 @@\n-hello world\n+hello rust\n",
            "replacement_mode": "patch",
            "expected_fingerprint": "0000000000000000000000000000000000000000000000000000000000000000"
        }),
    );
    assert_eq!(r["ok"], true);
    let findings = r["result"]["findings"].as_array().unwrap();
    let has_fp_mismatch = findings.iter().any(|f| f["code"] == "FINGERPRINT_MISMATCH");
    assert!(has_fp_mismatch, "Expected FINGERPRINT_MISMATCH finding");
}
