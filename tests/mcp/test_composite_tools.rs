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

fn call_tool_and_get_result(request: &str) -> Value {
    let response_str = mcp_request(request);
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

// text_security_inspect tests

#[test]
fn test_text_security_inspect_clean() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_security_inspect","arguments":{"text":"hello world"}},"id":1}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("verdict"),
        Some(&Value::String("allow".to_string()))
    );
    assert_eq!(
        inner.get("policy"),
        Some(&Value::String("default".to_string()))
    );
    assert!(inner
        .get("findings")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn test_text_security_inspect_with_hidden() {
    // U+200B ZERO WIDTH SPACE is a hidden/format character
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_security_inspect","arguments":{"text":"hello\u200bworld"}},"id":2}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let verdict = inner.get("verdict").unwrap().as_str().unwrap();
    assert!(verdict == "allow" || verdict == "review" || verdict == "block");
    // subresults is only present when detail is "normal" or "full"
    if let Some(sub) = inner.get("subresults") {
        assert!(sub.is_object());
    }
    assert!(inner.get("findings").unwrap().is_array());
}

// edit_preflight tests

#[test]
fn test_edit_preflight_basic() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"edit_preflight","arguments":{"original":"hello world","old":"world","new":"rust","replacement_mode":"literal"}},"id":3}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("ok_to_apply"), Some(&Value::Bool(true)));
    assert_eq!(
        inner.get("mode"),
        Some(&Value::String("literal".to_string()))
    );
    assert!(inner
        .get("findings")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

// command_preflight tests

#[test]
fn test_command_preflight_safe() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"command_preflight","arguments":{"command":"ls -la"}},"id":4}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("verdict"),
        Some(&Value::String("allow".to_string()))
    );
    assert_eq!(
        inner.get("platform"),
        Some(&Value::String("posix".to_string()))
    );
    assert!(inner
        .get("findings")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn test_command_preflight_dangerous() {
    // Command substitution triggers RISKY_SHELL_FEATURE warning → review verdict (matching Python)
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"command_preflight","arguments":{"command":"echo $(rm -rf /)"}},"id":5}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("verdict"),
        Some(&Value::String("review".to_string()))
    );
    let findings = inner.get("findings").unwrap().as_array().unwrap();
    assert!(!findings.is_empty());
    let has_substitution = findings.iter().any(|f| {
        f.get("message").and_then(|v| v.as_str()).unwrap_or("") == "has_command_substitution"
    });
    assert!(has_substitution);
    assert_eq!(
        inner.get("machine_code"),
        Some(&Value::String("SHELL_RISK".to_string()))
    );
}

// config_preflight tests

#[test]
fn test_config_preflight_json() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"config_preflight","arguments":{"text":"{\"key\": \"value\", \"num\": 42}","format":"json"}},"id":6}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("verdict"),
        Some(&Value::String("valid".to_string()))
    );
    assert_eq!(
        inner.get("format"),
        Some(&Value::String("json".to_string()))
    );
    assert!(inner
        .get("findings")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn test_config_preflight_toml() {
    let toml_text = "[package]\nname = \"test\"\nversion = \"0.1.0\"\n";
    let escaped = toml_text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let request = format!(
        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"config_preflight","arguments":{{"text":"{}","format":"toml"}}}},"id":7}}"#,
        escaped
    );
    let result = call_tool_and_get_result(&request);
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("verdict"),
        Some(&Value::String("valid".to_string()))
    );
    assert_eq!(
        inner.get("format"),
        Some(&Value::String("toml".to_string()))
    );
    assert!(inner
        .get("findings")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

// structured_data_compare tests

#[test]
fn test_structured_data_compare_identical() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"structured_data_compare","arguments":{"a":"{\"x\": 1, \"y\": 2}","b":"{\"x\": 1, \"y\": 2}"}},"id":8}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(true)));
    assert_eq!(inner.get("valid_a"), Some(&Value::Bool(true)));
    assert_eq!(inner.get("valid_b"), Some(&Value::Bool(true)));
    assert!(inner
        .get("findings")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn test_structured_data_compare_different() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"structured_data_compare","arguments":{"a":"{\"x\": 1}","b":"{\"x\": 2}"}},"id":9}"#,
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("equal"), Some(&Value::Bool(false)));
    assert_eq!(inner.get("valid_a"), Some(&Value::Bool(true)));
    assert_eq!(inner.get("valid_b"), Some(&Value::Bool(true)));
}
