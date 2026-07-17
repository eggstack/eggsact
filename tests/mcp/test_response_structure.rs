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
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.write_all(request.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    stdout.lines().last().unwrap_or("").to_string()
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

fn call_tool(name: &str, args: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": name, "arguments": args},
        "id": 1
    })
    .to_string();
    call_tool_and_get_result(&request)
}

#[test]
fn test_math_eval_response_structure() {
    let result = call_tool("math_eval", serde_json::json!({"expression": "2 + 3"}));

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("value").is_some(), "missing result.value");
    assert!(res.get("type").is_some(), "missing result.type");
    let typ = res["type"].as_str().unwrap();
    assert!(
        typ == "int" || typ == "integer",
        "expected int or integer, got {}",
        typ
    );
    assert_eq!(res["value"], "5");
    assert_eq!(
        result.get("tool"),
        Some(&Value::String("math_eval".to_string()))
    );
}

#[test]
fn test_text_measure_response_structure() {
    let result = call_tool("text_measure", serde_json::json!({"text": "hello world"}));

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    let expected_fields = [
        "bytes_utf8",
        "codepoints",
        "graphemes",
        "words",
        "unique_words_casefolded",
        "lines",
        "nonempty_lines",
        "blank_lines",
        "max_line_length_codepoints",
        "chars_no_whitespace",
        "ascii",
        "non_ascii",
        "letters",
        "digits",
        "punctuation",
        "symbols",
        "spaces",
        "control_chars",
        "combining_marks",
        "invisible_chars",
        "newline_style",
        "ends_with_newline",
        "normalization",
        "unicode_risks",
        "warnings",
    ];
    for field in &expected_fields {
        assert!(
            res.get(*field).is_some(),
            "text_measure response missing field: {}",
            field
        );
    }
    assert_eq!(result["tool"], "text_measure");
}

#[test]
fn test_text_equal_response_structure() {
    let result = call_tool(
        "text_equal",
        serde_json::json!({"a": "hello", "b": "hello"}),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("equal").is_some(), "missing equal");
    assert!(res.get("mode").is_some(), "missing mode");
    assert!(
        res.get("classification").is_some(),
        "missing classification"
    );
    assert_eq!(res["equal"], true);
    assert_eq!(res["classification"], "exact_match");
    assert_eq!(result["tool"], "text_equal");
}

#[test]
fn test_json_extract_response_structure() {
    let result = call_tool(
        "json_extract",
        serde_json::json!({
            "text": r#"{"key": "value", "num": 42}"#,
            "pointer": "/key"
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("valid_json").is_some(), "missing valid_json");
    assert!(res.get("found").is_some(), "missing found");
    assert!(res.get("pointer").is_some(), "missing pointer");
    assert!(res.get("value").is_some(), "missing value");
    assert_eq!(res["valid_json"], true);
    assert_eq!(res["found"], true);
    assert_eq!(res["pointer"], "/key");
    assert_eq!(res["value"], "value");
    assert_eq!(result["tool"], "json_extract");
}

#[test]
fn test_json_compare_response_structure() {
    let result = call_tool(
        "json_compare",
        serde_json::json!({
            "a": r#"{"x": 1}"#,
            "b": r#"{"x": 1}"#
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("equal").is_some(), "missing equal");
    assert!(res.get("valid_json_a").is_some(), "missing valid_json_a");
    assert!(res.get("valid_json_b").is_some(), "missing valid_json_b");
    assert!(res.get("diffs").is_some(), "missing diffs");
    assert_eq!(res["equal"], true);
    assert_eq!(res["valid_json_a"], true);
    assert_eq!(res["valid_json_b"], true);
    assert!(res["diffs"].as_array().unwrap().is_empty());
    assert_eq!(result["tool"], "json_compare");
}

#[test]
fn test_validate_json_response_structure() {
    let result = call_tool("validate_json", serde_json::json!({"text": r#"{"a": 1}"#}));

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("valid").is_some(), "missing valid");
    assert!(res.get("error").is_some(), "missing error field");
    assert_eq!(res["valid"], true);
    assert_eq!(result["tool"], "validate_json");
}

#[test]
fn test_list_compare_response_structure() {
    let result = call_tool(
        "list_compare",
        serde_json::json!({
            "a": ["x", "y"],
            "b": ["x", "z"],
            "mode": "set"
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("equal").is_some(), "missing equal");
    assert!(res.get("only_in_a").is_some(), "missing only_in_a");
    assert!(res.get("only_in_b").is_some(), "missing only_in_b");
    assert_eq!(res["equal"], false);
    assert!(res["only_in_a"]
        .as_array()
        .unwrap()
        .contains(&Value::String("y".to_string())));
    assert!(res["only_in_b"]
        .as_array()
        .unwrap()
        .contains(&Value::String("z".to_string())));
    assert_eq!(result["tool"], "list_compare");
}

#[test]
fn test_unit_convert_response_structure() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({
            "value": 1.0,
            "from_unit": "km",
            "to_unit": "m"
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("value").is_some(), "missing value");
    assert!(res.get("from_unit").is_some(), "missing from_unit");
    assert!(res.get("to_unit").is_some(), "missing to_unit");
    assert!(res.get("factor").is_some(), "missing factor");
    assert_eq!(res["from_unit"], "km");
    assert_eq!(res["to_unit"], "m");
    assert_eq!(result["tool"], "unit_convert");
}

#[test]
fn test_unit_convert_temperature_c_to_f() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": 100.0, "from_unit": "C", "to_unit": "F"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("value").is_some(), "missing value");
    assert!(res.get("from_unit").is_some(), "missing from_unit");
    assert!(res.get("to_unit").is_some(), "missing to_unit");
    assert_eq!(res["from_unit"], "C");
    assert_eq!(res["to_unit"], "F");
    // 100 C = 212 F
    let val = res["value"].as_f64().expect("value should be a number");
    assert!(
        (val - 212.0).abs() < 1e-10,
        "100 C should be 212 F, got {}",
        val
    );
    // Temperature conversions have null factor
    assert_eq!(res["factor"], Value::Null);
}

#[test]
fn test_unit_convert_temperature_f_to_c() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": 32.0, "from_unit": "F", "to_unit": "C"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    let val = res["value"].as_f64().expect("value should be a number");
    assert!((val - 0.0).abs() < 1e-10, "32 F should be 0 C, got {}", val);
    assert_eq!(res["from_unit"], "F");
    assert_eq!(res["to_unit"], "C");
    assert_eq!(res["factor"], Value::Null);
}

#[test]
fn test_unit_convert_temperature_k_to_c() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": 273.15, "from_unit": "K", "to_unit": "C"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    let val = res["value"].as_f64().expect("value should be a number");
    assert!(
        (val - 0.0).abs() < 1e-10,
        "273.15 K should be 0 C, got {}",
        val
    );
    assert_eq!(res["from_unit"], "K");
    assert_eq!(res["to_unit"], "C");
    assert_eq!(res["factor"], Value::Null);
}

#[test]
fn test_unit_convert_temperature_f_to_k() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": 32.0, "from_unit": "F", "to_unit": "K"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    let val = res["value"].as_f64().expect("value should be a number");
    assert!(
        (val - 273.15).abs() < 1e-10,
        "32 F should be 273.15 K, got {}",
        val
    );
}

#[test]
fn test_unit_convert_temperature_negative() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": -40.0, "from_unit": "C", "to_unit": "F"}),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    let val = res["value"].as_f64().expect("value should be a number");
    assert!(
        (val - (-40.0)).abs() < 1e-10,
        "-40 C should be -40 F, got {}",
        val
    );
    assert_eq!(res["factor"], Value::Null);
}

#[test]
fn test_unit_convert_cross_category_rejected() {
    let result = call_tool(
        "unit_convert",
        serde_json::json!({"value": 1.0, "from_unit": "m", "to_unit": "kg"}),
    );
    // Tool-level error: ok is false, error field contains the message
    assert_eq!(result.get("ok"), Some(&Value::Bool(false)));
    let error = result
        .get("error")
        .expect("missing error in tool-level error result");
    let msg = error.as_str().unwrap_or("");
    assert!(
        msg.contains("incompatible") || msg.contains("category"),
        "Expected cross-category rejection error, got: {}",
        msg
    );
    assert_eq!(result["tool"], "unit_convert");
}

#[test]
fn test_text_inspect_response_structure() {
    let result = call_tool("text_inspect", serde_json::json!({"text": "hello"}));

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("metrics").is_some(), "missing metrics");
    assert!(res.get("confusables").is_some(), "missing confusables");
    assert!(res.get("warnings").is_some(), "missing warnings");
    assert!(res.get("invisibles").is_some(), "missing invisibles");
    assert!(res.get("bidi_controls").is_some(), "missing bidi_controls");
    assert!(res.get("normalization").is_some(), "missing normalization");
    assert!(res.get("safe_repr").is_some(), "missing safe_repr");

    let metrics = res.get("metrics").unwrap();
    let expected_metric_fields = [
        "bytes_utf8",
        "codepoints",
        "graphemes",
        "words",
        "lines",
        "nonempty_lines",
        "blank_lines",
        "ascii",
        "non_ascii",
        "letters",
        "digits",
        "newline_style",
        "ends_with_newline",
        "normalization",
        "unicode_risks",
    ];
    for field in &expected_metric_fields {
        assert!(
            metrics.get(*field).is_some(),
            "text_inspect metrics missing field: {}",
            field
        );
    }
    assert_eq!(result["tool"], "text_inspect");
}

#[test]
fn test_text_security_inspect_response_structure() {
    let result = call_tool(
        "text_security_inspect",
        serde_json::json!({"text": "hello world"}),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("verdict").is_some(), "missing verdict");
    assert!(res.get("policy").is_some(), "missing policy");
    assert!(res.get("findings").is_some(), "missing findings");
    assert!(res.get("findings").unwrap().as_array().unwrap().is_empty());
    assert_eq!(result["tool"], "text_security_inspect");
}

#[test]
fn test_edit_preflight_response_structure() {
    let result = call_tool(
        "edit_preflight",
        serde_json::json!({
            "original": "hello world",
            "old": "world",
            "new": "rust",
            "replacement_mode": "literal"
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("ok_to_apply").is_some(), "missing ok_to_apply");
    assert!(res.get("mode").is_some(), "missing mode");
    assert!(res.get("findings").is_some(), "missing findings");
    assert_eq!(result["tool"], "edit_preflight");
}

#[test]
fn test_command_preflight_response_structure() {
    let result = call_tool(
        "command_preflight",
        serde_json::json!({"command": "ls -la"}),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("verdict").is_some(), "missing verdict");
    assert!(res.get("platform").is_some(), "missing platform");
    assert!(res.get("findings").is_some(), "missing findings");
    assert_eq!(result["tool"], "command_preflight");
}

#[test]
fn test_config_preflight_response_structure() {
    let result = call_tool(
        "config_preflight",
        serde_json::json!({
            "text": "{\"key\": \"value\"}",
            "format": "json"
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("verdict").is_some(), "missing verdict");
    assert!(res.get("format").is_some(), "missing format");
    assert!(res.get("findings").is_some(), "missing findings");
    assert_eq!(result["tool"], "config_preflight");
}

#[test]
fn test_structured_data_compare_response_structure() {
    let result = call_tool(
        "structured_data_compare",
        serde_json::json!({
            "a": "{\"x\": 1}",
            "b": "{\"x\": 1}"
        }),
    );

    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let res = result.get("result").expect("missing result");
    assert!(res.get("equal").is_some(), "missing equal");
    assert!(res.get("valid_a").is_some(), "missing valid_a");
    assert!(res.get("valid_b").is_some(), "missing valid_b");
    assert!(res.get("findings").is_some(), "missing findings");
    assert_eq!(result["tool"], "structured_data_compare");
}
