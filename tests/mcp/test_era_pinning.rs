//! Connection-era pinning regressions (plan 02c Parts A/B/C).
//!
//! One stdio process is one connection. The opening exchange pins the era
//! exactly once (`Undecided` → `Legacy` on successful `initialize`, → modern
//! on a valid `2026-07-28` envelope). Later cross-era requests are rejected
//! with `ERA_MISMATCH` (-32600) preserving the JSON-RPC id. Invalid modern
//! envelopes and failed `initialize` validation never pin, so the client can
//! retry. Unversioned `server/discover` always answers without pinning.
//!
//! Also covers the generic-invoke output contract: discovery `tools/list`
//! omits `tool_invoke.outputSchema` in both eras, and modern `tool_invoke`
//! returns target-specific `structuredContent` without a facade-level schema.

use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

fn modern_meta() -> Value {
    serde_json::json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
    })
}

fn mcp_session(requests: &[String]) -> Vec<Value> {
    mcp_session_with_env(requests, &[])
}

fn mcp_session_sequential(requests: &[String]) -> Vec<Value> {
    mcp_session_sequential_with_env(requests, &[])
}

fn mcp_session_sequential_with_env(requests: &[String], envs: &[(&str, &str)]) -> Vec<Value> {
    use std::io::{BufRead, BufReader};
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_eggsact"));
    cmd.arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("Failed to spawn process");
    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut out = Vec::new();
    for req in requests {
        stdin.write_all(req.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
        // Sequential: one outstanding request, so the next line is its
        // response. This makes opening-order deterministic (the first
        // request pins before the second is sent), unlike batch mode where
        // a spawned modern task races inline `initialize`.
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).expect("read response");
            assert!(n > 0, "server closed stdout before response");
            if !line.trim().is_empty() {
                break;
            }
        }
        out.push(serde_json::from_str(line.trim()).expect("valid JSON-RPC"));
    }
    drop(stdin);
    let _ = child.wait();
    out
}

fn mcp_session_with_env(requests: &[String], envs: &[(&str, &str)]) -> Vec<Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_eggsact"));
    cmd.arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("Failed to spawn process");
    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        for req in requests {
            stdin.write_all(req.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON-RPC"))
        .collect()
}

fn by_id(responses: &[Value], id: i64) -> Value {
    responses
        .iter()
        .find(|r| r.get("id") == Some(&Value::Number(id.into())))
        .unwrap_or_else(|| panic!("missing response id {}", id))
        .clone()
}

fn by_id_map(responses: &[Value]) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    for r in responses {
        if let Some(id) = r.get("id") {
            map.insert(id.to_string(), r.clone());
        }
    }
    map
}

fn legacy_initialize(id: i64) -> String {
    serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize", "id": id,
        "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t"}},
    })
    .to_string()
}

fn assert_era_mismatch(response: &Value, expected_id: i64) {
    assert_eq!(
        response.get("id"),
        Some(&Value::Number(expected_id.into())),
        "era rejection must preserve request id"
    );
    let err = response.get("error").expect("ERA_MISMATCH error");
    assert_eq!(err.get("code"), Some(&Value::Number((-32600).into())));
    assert_eq!(
        err.pointer("/data/code"),
        Some(&Value::String("ERA_MISMATCH".to_string())),
        "data.code must be ERA_MISMATCH, got {}",
        response
    );
}

// ── Part B1: mixed-era pinning ───────────────────────────────────────────

#[test]
fn legacy_initialize_pins_then_legacy_flow_succeeds() {
    let init = legacy_initialize(0);
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let list = serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 2}).to_string();
    let call = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 3,
        "params": {"name": "math_eval", "arguments": {"expression": "1+1"}},
    })
    .to_string();
    let res = mcp_session(&[init, notif, list, call]);
    assert!(by_id(&res, 0).get("result").is_some());
    assert!(!by_id(&res, 2)["result"]["tools"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(by_id(&res, 3)["result"]["content"].is_array());
}

#[test]
fn legacy_pinned_rejects_modern_envelope() {
    let init = legacy_initialize(10);
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let modern_list = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 11,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let modern_call = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 12,
        "params": {"name": "math_eval", "arguments": {"expression": "1+1"}, "_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[init, notif, modern_list, modern_call]);
    assert!(by_id(&res, 10).get("result").is_some());
    assert_era_mismatch(&by_id(&res, 11), 11);
    assert_era_mismatch(&by_id(&res, 12), 12);
    // Ids preserved even under concurrent dispatch.
    let ids = by_id_map(&res);
    assert!(ids.contains_key("11") && ids.contains_key("12"));
}

#[test]
fn modern_discover_pins_then_modern_flow_succeeds() {
    let d = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 20,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let l = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 21,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let c = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 22,
        "params": {"name": "math_eval", "arguments": {"expression": "2+3"}, "_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[d, l, c]);
    assert!(by_id(&res, 20).get("result").is_some());
    assert!(by_id(&res, 21).get("result").is_some());
    assert!(by_id(&res, 22).get("result").is_some());
}

#[test]
fn modern_pinned_rejects_legacy_initialize_without_state() {
    let d = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 30,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let init = legacy_initialize(31);
    let legacy_list =
        serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 32}).to_string();
    let modern_list = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 33,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    // Sequential: the opening discover must pin Modern before the legacy
    // follow-ups are sent; batch mode would race inline `initialize` against
    // the spawned discover task.
    let res = mcp_session_sequential(&[d, init, legacy_list, modern_list]);
    assert!(by_id(&res, 30).get("result").is_some());
    // initialize after modern pin is rejected, not ALREADY_INITIALIZED.
    assert_era_mismatch(&by_id(&res, 31), 31);
    // The rejected initialize created no legacy session: legacy list is an
    // era mismatch, not NOT_INITIALIZED, and modern still works.
    assert_era_mismatch(&by_id(&res, 32), 32);
    assert!(by_id(&res, 33).get("result").is_some());
}

#[test]
fn direct_modern_request_pins_without_discover() {
    let l = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 40,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let init = legacy_initialize(41);
    // Sequential: direct modern call must pin before `initialize` is sent.
    let res = mcp_session_sequential(&[l, init]);
    assert!(by_id(&res, 40).get("result").is_some());
    assert_era_mismatch(&by_id(&res, 41), 41);
}

#[test]
fn unsupported_version_does_not_pin_modern() {
    let bad = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 50,
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "1900-01-01",
            "io.modelcontextprotocol/clientCapabilities": {},
        }},
    })
    .to_string();
    let good = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 51,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[bad, good]);
    assert_eq!(by_id(&res, 50)["error"]["code"], -32022);
    assert_eq!(by_id(&res, 50).get("id"), Some(&Value::Number(50.into())));
    // No poisoned modern state: a valid modern request still pins and succeeds.
    assert!(by_id(&res, 51).get("result").is_some());
}

#[test]
fn unsupported_version_does_not_block_legacy_opening() {
    let bad = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 60,
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "1900-01-01",
            "io.modelcontextprotocol/clientCapabilities": {},
        }},
    })
    .to_string();
    let init = legacy_initialize(61);
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let list = serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 62}).to_string();
    let res = mcp_session(&[bad, init, notif, list]);
    assert_eq!(by_id(&res, 60)["error"]["code"], -32022);
    assert!(by_id(&res, 61).get("result").is_some());
    assert!(by_id(&res, 62)["result"]["tools"].as_array().is_some());
}

#[test]
fn malformed_envelope_does_not_poison_connection() {
    let malformed = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 70,
        "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}},
    })
    .to_string();
    let good = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 71,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[malformed, good]);
    assert_eq!(by_id(&res, 70)["error"]["code"], -32602);
    assert_eq!(by_id(&res, 70).get("id"), Some(&Value::Number(70.into())));
    assert!(by_id(&res, 71).get("result").is_some());
}

#[test]
fn malformed_envelope_does_not_block_legacy_opening() {
    let malformed = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 80,
        "params": {"_meta": {"io.modelcontextprotocol/clientCapabilities": {}}},
    })
    .to_string();
    let init = legacy_initialize(81);
    let res = mcp_session(&[malformed, init]);
    assert_eq!(by_id(&res, 80)["error"]["code"], -32602);
    assert!(by_id(&res, 81).get("result").is_some());
}

#[test]
fn failed_initialize_validation_does_not_pin() {
    let bad_init = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize", "id": 90,
        "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": ""}},
    })
    .to_string();
    let good_modern = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 91,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session_sequential(&[bad_init, good_modern]);
    // Empty clientInfo.name is invalid_request (-32600), not -32602, and must
    // not pin: the follow-up modern request still pins and succeeds.
    assert_eq!(by_id(&res, 90)["error"]["code"], -32600);
    // Failed legacy opening left Undecided: modern can still pin and succeed.
    assert!(by_id(&res, 91).get("result").is_some());
}

#[test]
fn unversioned_discover_never_pins() {
    // Unversioned probe then legacy opening: must still pin Legacy.
    let probe = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 100,
    })
    .to_string();
    let init = legacy_initialize(101);
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let list = serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 102}).to_string();
    let res = mcp_session(&[probe, init, notif, list]);
    assert!(by_id(&res, 100).get("result").is_some());
    assert!(by_id(&res, 101).get("result").is_some());
    assert!(by_id(&res, 102)["result"]["tools"].as_array().is_some());

    // Unversioned probe then modern: must still pin Modern.
    let probe2 = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 110,
    })
    .to_string();
    let modern = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 111,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res2 = mcp_session(&[probe2, modern]);
    assert!(by_id(&res2, 110).get("result").is_some());
    assert!(by_id(&res2, 111).get("result").is_some());
}

#[test]
fn unversioned_discover_still_answers_after_pinning() {
    // Legacy-pinned: unversioned discover answers without switching.
    let init = legacy_initialize(120);
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let probe = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 121,
    })
    .to_string();
    let modern_after = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 122,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[init, notif, probe, modern_after]);
    assert!(by_id(&res, 120).get("result").is_some());
    assert!(by_id(&res, 121).get("result").is_some());
    // The probe did not switch eras: modern is still rejected.
    assert_era_mismatch(&by_id(&res, 122), 122);

    // Modern-pinned: unversioned discover still answers.
    let d = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 130,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let probe2 = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 131,
    })
    .to_string();
    let res2 = mcp_session(&[d, probe2]);
    assert!(by_id(&res2, 130).get("result").is_some());
    assert!(by_id(&res2, 131).get("result").is_some());
}

#[test]
fn legacy_ping_does_not_pin_but_modern_pin_rejects_it() {
    // Legacy ping before any opening leaves Undecided: modern can still pin.
    let ping = serde_json::json!({"jsonrpc": "2.0", "method": "ping", "id": 140}).to_string();
    let modern = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 141,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[ping, modern]);
    assert!(by_id(&res, 140).get("result").is_some());
    assert!(by_id(&res, 141).get("result").is_some());

    // After modern pinning, legacy ping is a cross-era request (sequential to
    // avoid the spawn-vs-inline opening race).
    let d = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 150,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let ping2 = serde_json::json!({"jsonrpc": "2.0", "method": "ping", "id": 151}).to_string();
    let res2 = mcp_session_sequential(&[d, ping2]);
    assert!(by_id(&res2, 150).get("result").is_some());
    assert_era_mismatch(&by_id(&res2, 151), 151);
}

// ── Part C3: generic-invoke output contract ──────────────────────────────

#[test]
fn discovery_list_omits_invoke_output_schema_both_eras() {
    for surface_env in [("EGGSACT_MCP_SURFACE", "discovery")] {
        // Legacy shape.
        let list =
            serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 200}).to_string();
        // Need a legacy session for tools/list; use initialize first.
        let init = legacy_initialize(199);
        let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
        let res = mcp_session_with_env(&[init, notif, list], &[surface_env]);
        let tools = by_id(&res, 200)["result"]["tools"]
            .as_array()
            .unwrap()
            .clone();
        assert!(!tools.is_empty());
        let invoke = tools
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("tool_invoke"))
            .expect("discovery must advertise tool_invoke");
        assert!(
            invoke.get("outputSchema").is_none(),
            "tool_invoke must not advertise outputSchema (legacy), got {}",
            invoke
        );

        // Modern shape.
        let modern_list = serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/list", "id": 201,
            "params": {"_meta": modern_meta()},
        })
        .to_string();
        let res2 = mcp_session_with_env(&[modern_list], &[surface_env]);
        let mtools = by_id(&res2, 201)["result"]["tools"]
            .as_array()
            .unwrap()
            .clone();
        let minvoke = mtools
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("tool_invoke"))
            .expect("discovery modern must advertise tool_invoke");
        assert!(
            minvoke.get("outputSchema").is_none(),
            "tool_invoke must not advertise outputSchema (modern), got {}",
            minvoke
        );
        // No discovery entry advertises outputSchema.
        for t in &mtools {
            assert!(
                t.get("outputSchema").is_none(),
                "discovery modern must omit outputSchema, got {}",
                t
            );
        }
    }
}

fn tool_text_envelope(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("content text");
    serde_json::from_str(text).expect("envelope JSON")
}

#[test]
fn modern_tool_invoke_returns_target_specific_structured_content() {
    let envs = [("EGGSACT_MCP_SURFACE", "discovery")];
    // Two materially different targets: simple math vs route-critical preflight.
    let math = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 210,
        "params": {
            "name": "tool_invoke",
            "arguments": {"name": "math_eval", "arguments": {"expression": "2+3"}},
            "_meta": modern_meta(),
        },
    })
    .to_string();
    let preflight = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 211,
        "params": {
            "name": "tool_invoke",
            "arguments": {"name": "edit_preflight", "arguments": {"original": "hello", "old": "l", "new": "r", "replacement_mode": "literal"}},
            "_meta": modern_meta(),
        },
    })
    .to_string();
    let res = mcp_session_with_env(&[math, preflight], &envs);
    let math_full = by_id(&res, 210);
    let pre_full = by_id(&res, 211);
    for (full, id) in [(&math_full, 210), (&pre_full, 211)] {
        assert_eq!(
            full["result"].get("resultType"),
            Some(&Value::String("complete".to_string())),
            "invoke {} must be modern complete",
            id
        );
        assert!(
            full["result"].get("structuredContent").is_some(),
            "invoke {} must carry structuredContent",
            id
        );
        let envelope = tool_text_envelope(full);
        assert_eq!(
            full["result"]["structuredContent"],
            envelope["result"].clone(),
            "structuredContent must equal target ToolResponse.result for {}",
            id
        );
    }
    // Target shapes differ: math has value/type, preflight has verdict.
    // A single narrow facade schema claiming only tool/ok could not describe both.
    let math_struct = &math_full["result"]["structuredContent"];
    let pre_struct = &pre_full["result"]["structuredContent"];
    assert!(
        math_struct.get("value").is_some(),
        "math target must have value"
    );
    assert!(
        pre_struct.get("verdict").and_then(|v| v.as_str()).is_some(),
        "preflight target must have verdict"
    );
    assert_ne!(
        serde_json::to_string(math_struct).unwrap(),
        serde_json::to_string(pre_struct).unwrap()
    );
}
