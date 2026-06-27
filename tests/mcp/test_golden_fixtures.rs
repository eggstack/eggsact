use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn call_tool_and_get_result(request: &str) -> Value {
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
    let response_str = String::from_utf8_lossy(&output.stdout).to_string();
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

fn assert_json_eq(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Object(a_map), Value::Object(e_map)) => {
            for (key, e_val) in e_map {
                let a_val = a_map.get(key).unwrap_or(&Value::Null);
                assert_json_eq(a_val, e_val, &format!("{}.{}", path, key));
            }
        }
        (Value::Array(a_arr), Value::Array(e_arr)) => {
            assert_eq!(
                a_arr.len(),
                e_arr.len(),
                "Array length mismatch at {}",
                path
            );
            for (i, (a, e)) in a_arr.iter().zip(e_arr.iter()).enumerate() {
                assert_json_eq(a, e, &format!("{}[{}]", path, i));
            }
        }
        (Value::Number(a_n), Value::Number(e_n)) => {
            if e_n.is_f64() || a_n.is_f64() {
                let a_f = a_n.as_f64().unwrap_or(0.0);
                let e_f = e_n.as_f64().unwrap_or(0.0);
                assert!(
                    (a_f - e_f).abs() < 1e-10,
                    "Number mismatch at {}: actual={}, expected={}",
                    path,
                    a_f,
                    e_f
                );
            } else {
                assert_eq!(a_n, e_n, "Number mismatch at {}", path);
            }
        }
        _ => {
            assert_eq!(actual, expected, "Value mismatch at {}", path);
        }
    }
}

fn golden_test(result: &Value, expected: &Value) {
    if result.get("ok") != Some(&Value::Bool(true)) {
        panic!("Tool returned error: {}", result);
    }
    assert_json_eq(result, expected, "$");
}

// ---------------------------------------------------------------------------
// Golden fixture expected outputs (inline constants)
// ---------------------------------------------------------------------------

fn expected_math_eval_5plus3() -> Value {
    serde_json::json!({
        "ok": true,
        "result": {"value": "8", "type": "int"},
        "tool": "math_eval"
    })
}

fn expected_text_measure_hello() -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "bytes_utf8": 5,
            "codepoints": 5,
            "graphemes": 5,
            "words": 1,
            "unique_words_casefolded": 1,
            "lines": 1,
            "nonempty_lines": 1,
            "blank_lines": 0,
            "max_line_length_codepoints": 5,
            "chars_no_whitespace": 5,
            "ascii": 5,
            "non_ascii": 0,
            "letters": 5,
            "digits": 0,
            "punctuation": 0,
            "symbols": 0,
            "spaces": 0,
            "control_chars": 0,
            "combining_marks": 0,
            "invisible_chars": 0,
            "newline_style": "none",
            "ends_with_newline": false,
            "normalization": {
                "is_nfc": true,
                "is_nfd": true,
                "is_nfkc": true,
                "is_nfkd": true
            },
            "unicode_risks": {
                "contains_invisibles": false,
                "contains_bidi_controls": false,
                "mixed_scripts": false,
                "scripts": ["Latin"]
            },
            "warnings": []
        },
        "tool": "text_measure"
    })
}

fn expected_text_equal_hello_hello() -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "equal": true,
            "mode": {
                "normalization": "raw",
                "casefold": false,
                "trim": false,
                "ignore_newline_style": false,
                "ignore_trailing_whitespace": false,
                "ignore_final_newline": false
            },
            "raw_equal": true,
            "nfc_equal": true,
            "nfd_equal": true,
            "nfkc_equal": true,
            "nfkd_equal": true,
            "casefold_equal": true,
            "byte_equal": true,
            "lengths": {
                "a_codepoints": 5,
                "b_codepoints": 5,
                "a_bytes_utf8": 5,
                "b_bytes_utf8": 5
            },
            "first_difference": null,
            "classification": "exact_match"
        },
        "tool": "text_equal"
    })
}

fn expected_validate_json_valid() -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "valid": true,
            "line": null,
            "column": null,
            "position": null,
            "error": null
        },
        "tool": "validate_json"
    })
}

fn expected_unit_convert_1000m_km() -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "value": 1.0,
            "from_unit": "m",
            "to_unit": "km",
            "factor": 0.001
        },
        "tool": "unit_convert"
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_golden_math_eval_5plus3() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"5 + 3"}},"id":1}"#,
    );
    let expected = expected_math_eval_5plus3();
    golden_test(&result, &expected);
}

#[test]
fn test_golden_text_measure_hello() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_measure","arguments":{"text":"hello"}},"id":1}"#,
    );
    let expected = expected_text_measure_hello();
    golden_test(&result, &expected);
}

#[test]
fn test_golden_text_equal_hello_hello() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_equal","arguments":{"a":"hello","b":"hello"}},"id":1}"#,
    );
    let expected = expected_text_equal_hello_hello();
    golden_test(&result, &expected);
}

#[test]
fn test_golden_validate_json_valid() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"validate_json","arguments":{"text":"{\"a\":1}"}},"id":1}"#,
    );
    let expected = expected_validate_json_valid();
    golden_test(&result, &expected);
}

#[test]
fn test_golden_unit_convert_1000m_km() {
    let result = call_tool_and_get_result(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"unit_convert","arguments":{"value":1000,"from_unit":"m","to_unit":"km"}},"id":1}"#,
    );
    let expected = expected_unit_convert_1000m_km();
    golden_test(&result, &expected);
}

// ---------------------------------------------------------------------------
// UPDATE_GOLDEN mode: dump expected outputs for easy fixture regeneration
// ---------------------------------------------------------------------------

#[test]
fn test_golden_dump_all() {
    if std::env::var("UPDATE_GOLDEN").unwrap_or_default() != "1" {
        return;
    }

    let exp_math = expected_math_eval_5plus3();
    let exp_measure = expected_text_measure_hello();
    let exp_equal = expected_text_equal_hello_hello();
    let exp_json = expected_validate_json_valid();
    let exp_unit = expected_unit_convert_1000m_km();

    let fixtures: Vec<(&str, &str, &Value)> = vec![
        ("math_eval 5+3", "5+3", &exp_math),
        ("text_measure hello", "hello", &exp_measure),
        ("text_equal hello/hello", "hello", &exp_equal),
        ("validate_json", "{\"a\":1}", &exp_json),
        ("unit_convert 1000m->km", "1000m->km", &exp_unit),
    ];

    eprintln!("=== Golden Fixture Reference Outputs ===");

    let requests = vec![
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"5 + 3"}},"id":1}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_measure","arguments":{"text":"hello"}},"id":1}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_equal","arguments":{"a":"hello","b":"hello"}},"id":1}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"validate_json","arguments":{"text":"{\"a\":1}"}},"id":1}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"unit_convert","arguments":{"value":1000,"from_unit":"m","to_unit":"km"}},"id":1}"#,
    ];

    let tool_names = vec![
        "math_eval",
        "text_measure",
        "text_equal",
        "validate_json",
        "unit_convert",
    ];

    for (i, req) in requests.iter().enumerate() {
        let result = call_tool_and_get_result(req);
        eprintln!("\n--- {} ---", tool_names[i]);
        eprintln!(
            "Expected: {}",
            serde_json::to_string_pretty(fixtures[i].2).unwrap()
        );
        eprintln!(
            "Actual:   {}",
            serde_json::to_string_pretty(&result).unwrap()
        );
        assert_json_eq(&result, fixtures[i].2, &format!("{}: ", tool_names[i]));
    }
}
