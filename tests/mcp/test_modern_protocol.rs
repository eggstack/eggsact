//! Modern (2026-07-28) dual-era protocol tests.
//!
//! Covers plan `mcp-surface-01-protocol-2026-07-28` Part F:
//! - `server/discover` before any handshake, without mutating legacy state
//! - direct modern `tools/list` / `tools/call` without `initialize`
//! - required request `_meta` parsing and malformed reserved metadata
//! - unsupported protocol version (`-32022`) behavior
//! - modern response server identity (`_meta`), cache hints, deterministic order
//! - `structuredContent` conformance for simple + route-critical tools
//! - legacy lifecycle still working in the same binary
//! - era-incompatible methods (modern `initialize` / `ping`)
//! - cross-era semantic parity for representative tools
//! - registry invariants: annotations, `_meta` namespace, schema roots

use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn modern_meta() -> Value {
    serde_json::json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
    })
}

fn modern_meta_with_info() -> Value {
    serde_json::json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "test-client", "version": "1.0"},
    })
}

fn mcp_session(requests: &[String]) -> Vec<Value> {
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

fn tool_text_envelope(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("content text");
    serde_json::from_str(text).expect("envelope JSON")
}

// ── server/discover ──────────────────────────────────────────────────────

#[test]
fn modern_discover_before_initialize() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 1,
        "params": {"_meta": modern_meta_with_info()},
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res.len(), 1);
    let result = res[0].get("result").expect("discover result");
    assert_eq!(
        result.get("resultType"),
        Some(&Value::String("complete".to_string()))
    );
    let versions = result["supportedVersions"].as_array().unwrap();
    let strs: Vec<&str> = versions.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(strs, vec!["2026-07-28", "2025-11-25", "2024-11-05"]);
    assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    assert!(result
        .get("instructions")
        .and_then(|v| v.as_str())
        .is_some());
    assert_eq!(result.get("ttlMs"), Some(&Value::Number(3_600_000.into())));
    assert_eq!(
        result.get("cacheScope"),
        Some(&Value::String("public".to_string()))
    );
    let server_info = &result["_meta"]["io.modelcontextprotocol/serverInfo"];
    assert_eq!(server_info["name"], "eggsact");
    assert!(server_info
        .get("version")
        .and_then(|v| v.as_str())
        .is_some());
}

#[test]
fn modern_discover_without_meta_still_answers() {
    // stdio backward-compat probe: no envelope, no prior state, no mutation.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 1,
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res.len(), 1);
    assert!(res[0].get("result").is_some());
    assert_eq!(
        res[0]["result"]["supportedVersions"][0],
        Value::String("2026-07-28".to_string())
    );
}

#[test]
fn modern_discover_then_modern_list_without_initialized() {
    let d = serde_json::json!({
        "jsonrpc": "2.0", "method": "server/discover", "id": 1,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let l = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 2,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[d, l]);
    assert_eq!(res.len(), 2);
    assert!(by_id(&res, 1).get("result").is_some());
    assert!(by_id(&res, 2).get("result").is_some());
}

// ── modern tools/list ────────────────────────────────────────────────────

#[test]
fn modern_list_without_initialize() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 2,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[req]);
    let result = res[0].get("result").expect("modern list result");
    assert_eq!(
        result.get("resultType"),
        Some(&Value::String("complete".to_string()))
    );
    assert_eq!(result.get("ttlMs"), Some(&Value::Number(3_600_000.into())));
    assert_eq!(
        result.get("cacheScope"),
        Some(&Value::String("public".to_string()))
    );
    let tools = result["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    // Modern Tool shape: standard fields + annotations + namespaced _meta,
    // no nonstandard top-level registry keys.
    for tool in tools {
        assert!(tool.get("name").and_then(|v| v.as_str()).is_some());
        assert!(tool.get("description").and_then(|v| v.as_str()).is_some());
        assert!(tool.get("inputSchema").is_some());
        let ann = tool.get("annotations").expect("annotations");
        assert_eq!(ann.get("readOnlyHint"), Some(&Value::Bool(true)));
        assert_eq!(ann.get("openWorldHint"), Some(&Value::Bool(false)));
        assert!(tool.get("_meta").is_some());
        assert!(tool["_meta"].get("io.github.eggstack/eggsact").is_some());
        assert!(
            tool.get("tier").is_none(),
            "tier must not be top-level in modern"
        );
        assert!(
            tool.get("tags").is_none(),
            "tags must not be top-level in modern"
        );
        assert!(
            tool.get("category").is_none(),
            "category must not be top-level in modern"
        );
        assert!(tool.get("llm_exposure").is_none());
        assert!(tool.get("cost").is_none());
    }
    let server_info = &result["_meta"]["io.modelcontextprotocol/serverInfo"];
    assert_eq!(server_info["name"], "eggsact");
}

#[test]
fn modern_list_deterministic_order_and_bytes() {
    let mk = |id: i64| {
        serde_json::json!({
            "jsonrpc": "2.0", "method": "tools/list", "id": id,
            "params": {"_meta": modern_meta()},
        })
        .to_string()
    };
    let res = mcp_session(&[mk(20), mk(21)]);
    let r20 = by_id(&res, 20);
    let r21 = by_id(&res, 21);
    let names = |r: &Value| {
        r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&r20), names(&r21));
    let b20 = serde_json::to_string(&r20["result"]).unwrap();
    let b21 = serde_json::to_string(&r21["result"]).unwrap();
    assert_eq!(b20, b21);
}

// ── modern tools/call ────────────────────────────────────────────────────

#[test]
fn modern_call_without_initialize_simple() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 3,
        "params": {
            "name": "math_eval",
            "arguments": {"expression": "2 + 3"},
            "_meta": modern_meta(),
        },
    })
    .to_string();
    let res = mcp_session(&[req]);
    let result = res[0].get("result").expect("modern call result");
    assert_eq!(
        result.get("resultType"),
        Some(&Value::String("complete".to_string()))
    );
    assert!(result.get("structuredContent").is_some());
    assert!(result.get("_meta").is_some());
    let envelope = tool_text_envelope(&res[0]);
    assert_eq!(envelope.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(
        result["structuredContent"],
        envelope["result"].clone(),
        "structuredContent must equal ToolResponse.result"
    );
}

#[test]
fn modern_call_route_critical_structured() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 4,
        "params": {
            "name": "edit_preflight",
            "arguments": {"original": "hello", "old": "l", "new": "r", "replacement_mode": "literal"},
            "_meta": modern_meta(),
        },
    })
    .to_string();
    let res = mcp_session(&[req]);
    let result = &res[0]["result"];
    let structured = result.get("structuredContent").expect("structuredContent");
    assert!(structured.get("verdict").and_then(|v| v.as_str()).is_some());
    let envelope = tool_text_envelope(&res[0]);
    assert_eq!(structured, &envelope["result"]);
}

#[test]
fn modern_call_error_retains_is_error_without_structured() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 5,
        "params": {
            "name": "math_eval",
            "arguments": {"expression": ""},
            "_meta": modern_meta(),
        },
    })
    .to_string();
    let res = mcp_session(&[req]);
    let result = &res[0]["result"];
    // Empty expression is a tool-level error (ok=false) or a JSON-RPC error;
    // either way modern tool errors keep isError and omit structuredContent.
    if result.get("content").is_some() {
        assert_eq!(result.get("isError"), Some(&Value::Bool(true)));
        assert!(result.get("structuredContent").is_none());
    } else {
        assert!(res[0].get("error").is_some());
    }
}

// ── _meta validation ─────────────────────────────────────────────────────

#[test]
fn modern_missing_capabilities_rejected_invalid_params() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 6,
        "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}},
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res[0]["error"]["code"], -32602);
}

#[test]
fn modern_missing_version_rejected_invalid_params() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 7,
        "params": {"_meta": {"io.modelcontextprotocol/clientCapabilities": {}}},
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res[0]["error"]["code"], -32602);
}

#[test]
fn modern_malformed_client_info_rejected() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 8,
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": {"version": "1.0"},
        }},
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res[0]["error"]["code"], -32602);
}

#[test]
fn modern_unsupported_version_returns_32022() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 9,
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "1900-01-01",
            "io.modelcontextprotocol/clientCapabilities": {},
        }},
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res[0]["error"]["code"], -32022);
    let data = &res[0]["error"]["data"];
    assert_eq!(data["requested"], Value::String("1900-01-01".to_string()));
    let supported = data["supported"].as_array().unwrap();
    assert!(supported.contains(&Value::String("2026-07-28".to_string())));
}

// ── era-incompatible methods ─────────────────────────────────────────────

#[test]
fn modern_initialize_rejected() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize", "id": 10,
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "x"},
            "_meta": modern_meta(),
        },
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res[0]["error"]["code"], -32601);
}

#[test]
fn modern_ping_rejected_removed() {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": "ping", "id": 11,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[req]);
    assert_eq!(res[0]["error"]["code"], -32601);
}

#[test]
fn legacy_ping_still_allowed() {
    let req = serde_json::json!({"jsonrpc": "2.0", "method": "ping", "id": 12}).to_string();
    let res = mcp_session(&[req]);
    assert!(res[0].get("result").is_some());
}

// ── legacy still works + no state mutation ───────────────────────────────

#[test]
fn legacy_lifecycle_still_works() {
    let init = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize", "id": 0,
        "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t"}},
    })
    .to_string();
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let list = serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 2}).to_string();
    let call = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 3,
        "params": {"name": "math_eval", "arguments": {"expression": "1+1"}},
    })
    .to_string();
    let res = mcp_session(&[init, notif, list, call]);
    assert_eq!(by_id(&res, 0)["result"]["protocolVersion"], "2024-11-05");
    assert!(!by_id(&res, 2)["result"]["tools"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(by_id(&res, 3)["result"]["content"].is_array());
    // Legacy results must not gain modern fields.
    assert!(by_id(&res, 2)["result"].get("resultType").is_none());
    assert!(by_id(&res, 2)["result"].get("ttlMs").is_none());
}

#[test]
fn modern_does_not_mutate_legacy_session() {
    let modern = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "id": 20,
        "params": {"_meta": modern_meta()},
    })
    .to_string();
    let legacy =
        serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "id": 21}).to_string();
    let res = mcp_session(&[modern, legacy]);
    assert!(by_id(&res, 20).get("result").is_some());
    let err = by_id(&res, 21);
    assert!(err.get("error").is_some());
    assert_eq!(err["error"]["data"]["code"], "NOT_INITIALIZED");
}

// ── cross-era golden parity ──────────────────────────────────────────────

fn legacy_call_result(tool: &str, args: Value) -> Value {
    let init = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize", "id": 0,
        "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t"}},
    })
    .to_string();
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string();
    let call = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 30,
        "params": {"name": tool, "arguments": args},
    })
    .to_string();
    let res = mcp_session(&[init, notif, call]);
    tool_text_envelope(&by_id(&res, 30))
}

fn modern_call_full(tool: &str, args: Value) -> (Value, Value) {
    let call = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 31,
        "params": {"name": tool, "arguments": args, "_meta": modern_meta()},
    })
    .to_string();
    let res = mcp_session(&[call]);
    let full = by_id(&res, 31);
    let envelope = tool_text_envelope(&full);
    (full, envelope)
}

#[test]
fn cross_era_parity_math_eval() {
    let args = serde_json::json!({"expression": "2 + 3"});
    let legacy = legacy_call_result("math_eval", args.clone());
    let (full, modern) = modern_call_full("math_eval", args);
    assert_eq!(legacy, modern);
    assert_eq!(
        full["result"]["structuredContent"],
        modern["result"].clone()
    );
}

#[test]
fn cross_era_parity_validate_json() {
    let args = serde_json::json!({"text": r#"{"a": 1}"#});
    let legacy = legacy_call_result("validate_json", args.clone());
    let (full, modern) = modern_call_full("validate_json", args);
    assert_eq!(legacy, modern);
    assert_eq!(
        full["result"]["structuredContent"],
        modern["result"].clone()
    );
}

#[test]
fn cross_era_parity_edit_preflight() {
    let args = serde_json::json!({
        "original": "hello", "old": "l", "new": "r", "replacement_mode": "literal",
    });
    let legacy = legacy_call_result("edit_preflight", args.clone());
    let (full, modern) = modern_call_full("edit_preflight", args);
    assert_eq!(legacy, modern);
    assert!(modern["result"].get("verdict").is_some());
    assert!(modern.get("machine_code").is_some());
    assert_eq!(
        full["result"]["structuredContent"],
        modern["result"].clone()
    );
}

// ── registry invariants (in-process, no subprocess) ──────────────────────

#[test]
fn all_model_visible_tools_advertise_local_annotations() {
    use eggsact::mcp::registry::{annotations_for_spec, modern_tool_value};
    use eggsact::mcp::registry::{tools_for_profile_audience, ToolListAudience};
    let specs = tools_for_profile_audience("full", ToolListAudience::Model);
    assert!(!specs.is_empty());
    for spec in specs {
        let ann = annotations_for_spec(spec);
        assert!(ann.read_only_hint, "{}", spec.name);
        assert!(!ann.open_world_hint, "{}", spec.name);
        assert!(!ann.destructive_hint, "{}", spec.name);
        assert!(ann.idempotent_hint, "{}", spec.name);
        // Modern serialization keeps standard fields top-level only.
        let v = modern_tool_value(
            spec,
            spec.description.to_string(),
            (spec.input_schema)(),
            Some((spec.output_schema)()),
            false,
        );
        assert!(v.get("tier").is_none(), "{}", spec.name);
        let meta = &v["_meta"]["io.github.eggstack/eggsact"];
        assert!(meta.get("category").is_some(), "{}", spec.name);
    }
}

#[test]
fn all_emitted_schemas_satisfy_modern_input_root_constraint() {
    use eggsact::mcp::registry::all_tools_list;
    for spec in all_tools_list() {
        let input = (spec.input_schema)();
        assert_eq!(
            input.get("type"),
            Some(&Value::String("object".to_string())),
            "input root must be object for {}",
            spec.name
        );
        // Output schemas describe ToolResponse.result; structuredContent carries
        // exactly that value, so outputs must remain objects in this line.
        let output = (spec.output_schema)();
        assert!(
            output.is_object(),
            "output schema must be an object for {}",
            spec.name
        );
    }
}
