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

/// Send multiple requests in one session (initialize + actual request).
fn mcp_session(requests: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Build an initialize request with the given ID.
fn initialize_request(id: i32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": id,
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test-client"}
        }
    })
    .to_string()
}

/// Build the initialized notification.
fn initialized_notification() -> &'static str {
    r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
}

/// Standard lifecycle: initialize → initialized notification.
fn lifecycle_prereqs(id: i32) -> Vec<String> {
    vec![
        initialize_request(id),
        initialized_notification().to_string(),
    ]
}

fn parse_response(response: &str) -> Value {
    serde_json::from_str(response).expect("Failed to parse JSON-RPC response")
}

fn parse_responses(response: &str) -> Vec<Value> {
    response
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("Failed to parse JSON-RPC response"))
        .collect()
}

#[test]
fn test_initialize_response() {
    let response_str = mcp_request(&initialize_request(1));
    let response = parse_response(&response_str);

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(1.into())));

    let result = response.get("result").expect("Missing result field");

    let server_info = result.get("serverInfo").expect("Missing serverInfo");
    assert_eq!(
        server_info.get("name"),
        Some(&Value::String("eggsact".to_string()))
    );
    assert!(
        server_info.get("version").is_some(),
        "Missing version in serverInfo"
    );

    let capabilities = result.get("capabilities").expect("Missing capabilities");
    let tools = capabilities
        .get("tools")
        .expect("Missing tools in capabilities");
    assert_eq!(tools.get("listChanged"), Some(&Value::Bool(false)));

    // New: verify experimental eggsact extensions
    let experimental = capabilities
        .get("experimental")
        .expect("Missing experimental in capabilities");
    let eggsact = experimental
        .get("eggsact")
        .expect("Missing eggsact in experimental");
    assert_eq!(eggsact.get("profiles"), Some(&Value::Bool(true)));
    assert_eq!(eggsact.get("schemaDetail"), Some(&Value::Bool(true)));
    assert_eq!(eggsact.get("audienceFiltering"), Some(&Value::Bool(true)));

    assert_eq!(
        result.get("protocolVersion"),
        Some(&Value::String("2024-11-05".to_string()))
    );
}

#[test]
fn test_initialize_with_preferred_version() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "test-client", "version": "1.0"}
        }
    })
    .to_string();
    let response_str = mcp_request(&request);
    let response = parse_response(&response_str);
    let result = response.get("result").expect("Missing result field");
    assert_eq!(
        result.get("protocolVersion"),
        Some(&Value::String("2025-11-25".to_string()))
    );
}

#[test]
fn test_initialize_with_unsupported_version_falls_back() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "9999-01-01",
            "capabilities": {},
            "clientInfo": {"name": "test-client"}
        }
    })
    .to_string();
    let response_str = mcp_request(&request);
    let response = parse_response(&response_str);
    let result = response.get("result").expect("Missing result field");
    // Should fall back to preferred version
    assert_eq!(
        result.get("protocolVersion"),
        Some(&Value::String("2025-11-25".to_string()))
    );
}

#[test]
fn test_duplicate_initialize_rejected() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        stdin.write_all(initialize_request(1).as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        // Second initialize with same session
        stdin.write_all(initialize_request(2).as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let responses = parse_responses(&stdout);

    // First should succeed
    assert!(
        responses[0].get("result").is_some(),
        "First initialize should succeed"
    );
    // Second should fail with already_initialized
    let err = responses[1]
        .get("error")
        .expect("Second initialize should fail");
    let data = err.get("data").expect("Should have data field");
    assert_eq!(
        data.get("code"),
        Some(&Value::String("ALREADY_INITIALIZED".to_string()))
    );
}

#[test]
fn test_tools_list_before_initialize_rejected() {
    let response_str = mcp_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":2}"#);
    let response = parse_response(&response_str);

    let error = response
        .get("error")
        .expect("Missing error field for tools/list before init");
    let data = error.get("data").expect("Should have data field");
    assert_eq!(
        data.get("code"),
        Some(&Value::String("NOT_INITIALIZED".to_string()))
    );
}

#[test]
fn test_tools_call_before_initialize_rejected() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "math_eval", "arguments": {"expression": "1+1"}},
        "id": 3
    })
    .to_string();
    let response_str = mcp_request(&request);
    let response = parse_response(&response_str);

    let error = response
        .get("error")
        .expect("Missing error field for tools/call before init");
    let data = error.get("data").expect("Should have data field");
    assert_eq!(
        data.get("code"),
        Some(&Value::String("NOT_INITIALIZED".to_string()))
    );
}

#[test]
fn test_tools_list_response() {
    let mut prereqs = lifecycle_prereqs(0);
    prereqs.push(
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 2
        })
        .to_string(),
    );
    let response_str = mcp_session(&prereqs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let responses = parse_responses(&response_str);
    // Last response is tools/list
    let response = responses.last().expect("Missing tools/list response");

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(2.into())));

    let result = response.get("result").expect("Missing result field");
    assert!(
        result.is_object(),
        "Expected tools/list result to be an object"
    );

    let tools = result.get("tools").expect("Missing 'tools' key in result");
    assert!(tools.is_array(), "Expected tools to be an array");
    let tools = tools.as_array().expect("Not an array");
    assert!(!tools.is_empty(), "Expected at least one tool");

    for tool in tools {
        assert!(tool.get("name").is_some(), "Tool missing 'name' field");
        assert!(
            tool.get("description").is_some(),
            "Tool missing 'description' field"
        );
        assert!(
            tool.get("inputSchema").is_some(),
            "Tool missing 'inputSchema' field"
        );
        assert!(
            tool.get("name").unwrap().is_string(),
            "Tool 'name' should be a string"
        );
    }

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(tool_names.contains(&"math_eval"), "Expected math_eval tool");
    assert!(
        tool_names.contains(&"text_measure"),
        "Expected text_measure tool"
    );
    assert!(
        tool_names.contains(&"validate_json"),
        "Expected validate_json tool"
    );
}

#[test]
fn test_unknown_method_error() {
    let mut prereqs = lifecycle_prereqs(0);
    prereqs.push(r#"{"jsonrpc":"2.0","method":"unknown_method","id":3}"#.to_string());
    let response_str = mcp_session(&prereqs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let responses = parse_responses(&response_str);
    let response = responses.last().expect("Missing response");

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(3.into())));

    let error = response
        .get("error")
        .expect("Missing error field for unknown method");
    assert_eq!(error.get("code"), Some(&Value::Number((-32601).into())));
    let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        message.contains("Method not found"),
        "Expected 'Method not found' in error message, got: {}",
        message
    );
}

#[test]
fn test_missing_method_error() {
    let response_str = mcp_request(r#"{"jsonrpc":"2.0","id":4}"#);
    let response = parse_response(&response_str);

    let error = response
        .get("error")
        .expect("Missing error field for request without method");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert!(
        code < 0,
        "Expected negative error code for missing method, got: {}",
        code
    );
    let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(!message.is_empty(), "Error message should not be empty");
}

#[test]
fn test_ping_returns_empty() {
    // ping is allowed before initialization
    let response_str = mcp_request(r#"{"jsonrpc":"2.0","method":"ping","id":5}"#);
    let response = parse_response(&response_str);

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(5.into())));

    let result = response
        .get("result")
        .expect("Missing result field for ping");
    let inner = result.get("result").unwrap_or(result);
    assert_eq!(
        inner,
        &serde_json::json!({}),
        "Ping result should contain empty object"
    );
}

#[test]
fn test_profiles_list_returns_profiles() {
    let mut prereqs = lifecycle_prereqs(0);
    prereqs.push(r#"{"jsonrpc":"2.0","method":"profiles/list","id":6}"#.to_string());
    let response_str = mcp_session(&prereqs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let responses = parse_responses(&response_str);
    let response = responses.last().expect("Missing profiles/list response");

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(6.into())));

    let result = response
        .get("result")
        .expect("Missing result field for profiles/list");

    assert!(
        result.get("active_profile").is_some(),
        "Missing active_profile in result"
    );
    assert!(
        result.get("profiles").is_some(),
        "Missing profiles in result"
    );
    assert!(
        result.get("available_profiles").is_some(),
        "Missing available_profiles in result"
    );

    let available = result
        .get("available_profiles")
        .and_then(|a| a.as_array())
        .expect("available_profiles is not an array");
    assert!(
        available.iter().any(|p| p.as_str() == Some("full")),
        "Expected 'full' in available_profiles"
    );

    let profiles = result
        .get("profiles")
        .and_then(|p| p.as_object())
        .expect("profiles is not an object");
    let full_profile = profiles.get("full").expect("Missing 'full' profile");
    assert!(
        full_profile.get("tools").is_some(),
        "full profile missing tools"
    );
    assert!(
        full_profile.get("tool_count").is_some(),
        "full profile missing tool_count"
    );
}

#[test]
fn test_batch_request_rejected() {
    let response_str = mcp_request(r#"[{"jsonrpc":"2.0","method":"ping","id":1}]"#);
    let response = parse_response(&response_str);

    let error = response
        .get("error")
        .expect("Missing error field for batch request");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert!(
        code < 0,
        "Expected negative error code for batch request, got: {}",
        code
    );
    let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        !message.is_empty(),
        "Error message should not be empty for batch rejection"
    );
}

#[test]
fn test_tools_call_simple_expression() {
    let mut prereqs = lifecycle_prereqs(0);
    prereqs.push(
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "math_eval", "arguments": {"expression": "2 + 3"}},
            "id": 10
        })
        .to_string(),
    );
    let response_str = mcp_session(&prereqs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let responses = parse_responses(&response_str);
    let response = responses.last().expect("Missing tools/call response");

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(10.into())));

    let result = response
        .get("result")
        .expect("Missing result field for tools/call");
    let content = result
        .get("content")
        .expect("Missing content in result")
        .as_array()
        .expect("content should be array");
    assert!(!content.is_empty(), "content array should not be empty");

    let text = content[0]
        .get("text")
        .expect("Missing text in content item")
        .as_str()
        .expect("text should be a string");
    let parsed: Value = serde_json::from_str(text).expect("text content should be valid JSON");
    assert_eq!(parsed.get("ok"), Some(&Value::Bool(true)));

    let res = parsed.get("result").expect("Missing result in parsed text");
    let val = res.get("value").expect("Missing value in result");
    assert!(
        val == &Value::Number(5.into()) || val == &Value::String("5".to_string()),
        "2 + 3 should equal 5, got {:?}",
        val
    );
}

#[test]
fn test_tools_call_unknown_tool() {
    let mut prereqs = lifecycle_prereqs(0);
    prereqs.push(
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "nonexistent_tool_xyz", "arguments": {}},
            "id": 11
        })
        .to_string(),
    );
    let response_str = mcp_session(&prereqs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let responses = parse_responses(&response_str);
    let response = responses.last().expect("Missing response");

    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(11.into())));

    let error = response
        .get("error")
        .expect("Missing error field for unknown tool");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert!(
        code < 0,
        "Expected negative error code for unknown tool, got: {}",
        code
    );
}

#[test]
fn test_tools_call_missing_params() {
    let mut prereqs = lifecycle_prereqs(0);
    prereqs.push(
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {},
            "id": 12
        })
        .to_string(),
    );
    let response_str = mcp_session(&prereqs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let responses = parse_responses(&response_str);
    let response = responses.last().expect("Missing response");

    let error = response
        .get("error")
        .expect("Missing error field for missing params");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert!(
        code < 0,
        "Expected negative error code for missing params, got: {}",
        code
    );
}

#[test]
fn test_oversized_request_rejected() {
    let oversized_expression = "x".repeat(1_100_000);
    let request = format!(
        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"math_eval","arguments":{{"expression":"{}"}}}},"id":7}}"#,
        oversized_expression
    );

    let response_str = mcp_request(&request);
    let response = parse_response(&response_str);

    let error = response
        .get("error")
        .expect("Missing error field for oversized request");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert!(
        code < 0,
        "Expected negative error code for oversized request, got: {}",
        code
    );
    let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        message.contains("size") || message.contains("exceeds") || message.contains("limit"),
        "Expected size-related error message, got: {}",
        message
    );
}

#[test]
fn test_null_id_rejected() {
    let request = r#"{"jsonrpc":"2.0","method":"ping","id":null}"#;
    let response_str = mcp_request(request);
    let response = parse_response(&response_str);

    let error = response
        .get("error")
        .expect("Missing error for null ID request");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_eq!(
        code, -32600,
        "Null ID should return -32600 (Invalid Request)"
    );
    let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        message.contains("null"),
        "Error should mention null, got: {}",
        message
    );
}

#[test]
fn test_notification_has_no_response() {
    // notifications/initialized has no id — server should produce no response
    let request = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let response_str = mcp_request(request);
    let trimmed = response_str.trim();
    assert!(
        trimmed.is_empty(),
        "Notification should produce no response, got: {}",
        trimmed
    );
}

#[test]
fn test_duplicate_request_id_rejected() {
    // Two requests with the same ID — the second should be rejected.
    // Use a long-running tool to keep the first request active.
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize first
        stdin.write_all(initialize_request(0).as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(initialized_notification().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // First request — use a tool that takes some time
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_diff_explain","arguments":{"a":"hello world foo bar baz","b":"hello world qux bar baz"}},"id":1}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Second request with same ID
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":1}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Third request with different ID (should succeed)
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":2}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Should have at least 2 responses (the duplicate is rejected, the ping succeeds)
    // The first response is for id=1 (tool result), the second for id=2 (ping).
    // The duplicate id=1 should produce an error response.
    let has_error = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("error").is_some() && v.get("id") == Some(&Value::Number(1.into()))
        } else {
            false
        }
    });
    let has_ping = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(2.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(
        has_error || lines.len() >= 2,
        "Expected duplicate ID error or multiple responses, got {} lines: {:?}",
        lines.len(),
        lines
    );
    assert!(has_ping, "Ping with different ID should succeed");
}

#[test]
fn test_tool_call_after_initialize_but_before_initialized_notification_rejected() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize (transitions to AwaitingInitialized)
        stdin.write_all(initialize_request(1).as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        // tools/call before initialized notification — should be rejected
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":2}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let responses = parse_responses(&stdout);

    // First response is initialize result (success)
    assert!(
        responses[0].get("result").is_some(),
        "Initialize should succeed"
    );
    // Second response should be an error (not initialized)
    let err = responses[1]
        .get("error")
        .expect("tools/call before initialized should fail");
    let data = err.get("data").expect("Should have data field");
    assert_eq!(
        data.get("code"),
        Some(&Value::String("NOT_INITIALIZED".to_string()))
    );
}
