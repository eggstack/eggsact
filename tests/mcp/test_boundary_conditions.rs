use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use serde_json::Value;

fn call_tool(name: &str, args: Value) -> Value {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
    let resp = registry
        .call_json(name, args)
        .unwrap_or_else(|e| panic!("Tool call to '{name}' failed: {e}"));
    let mut map = serde_json::Map::new();
    map.insert("ok".into(), Value::Bool(resp.ok));
    if let Some(result) = resp.result {
        map.insert("result".into(), result);
    }
    if let Some(error) = resp.error {
        map.insert("error".into(), Value::String(error));
    }
    if let Some(mc) = resp.machine_code {
        map.insert("machine_code".into(), Value::String(mc));
    }
    Value::Object(map)
}

fn is_error(result: &Value) -> bool {
    result.get("ok") == Some(&Value::Bool(false))
}

#[test]
fn test_empty_string_tools() {
    // text_measure with empty string should succeed
    let r = call_tool("text_measure", serde_json::json!({"text": ""}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["codepoints"], 0);
    assert_eq!(r["result"]["bytes_utf8"], 0);

    // text_equal with empty strings
    let r = call_tool("text_equal", serde_json::json!({"a": "", "b": ""}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["equal"], true);

    // text_inspect with empty string
    let r = call_tool("text_inspect", serde_json::json!({"text": ""}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));

    // validate_json with empty string
    let r = call_tool("validate_json", serde_json::json!({"text": ""}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["valid"], false);

    // json_extract with empty string
    let r = call_tool("json_extract", serde_json::json!({"text": ""}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["valid_json"], false);
}

#[test]
fn test_max_text_length_approach() {
    // Test with text just under the limit (99999 chars)
    let text_99999: String = "a".repeat(99_999);
    let args = serde_json::json!({"text": &text_99999});
    let r = call_tool("text_measure", args);
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["codepoints"], 99_999);
}

#[test]
fn test_max_text_length_exceed() {
    // Test with text exceeding MAX_TEXT_LENGTH (100001 chars)
    let text_100001: String = "b".repeat(100_001);
    let args = serde_json::json!({"text": &text_100001});

    let r = call_tool("text_measure", args.clone());
    assert!(
        is_error(&r),
        "text_measure should reject text exceeding limit"
    );

    let r = call_tool("text_inspect", args.clone());
    assert!(
        is_error(&r),
        "text_inspect should reject text exceeding limit"
    );

    let r = call_tool("text_count", serde_json::json!({"text": &text_100001}));
    assert!(
        is_error(&r),
        "text_count should reject text exceeding limit"
    );
}

#[test]
fn test_zero_value_math() {
    let r = call_tool("math_eval", serde_json::json!({"expression": "0 + 0"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["value"], "0");

    let r = call_tool("math_eval", serde_json::json!({"expression": "0 * 1000"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["value"], "0");

    // Python parity: '/' is true division, so 0/1 is 0.0 (float), not 0 (int).
    let r = call_tool("math_eval", serde_json::json!({"expression": "0 / 1"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["value"], "0.0");
    assert_eq!(r["result"]["type"], "float");

    // Division by zero
    let r = call_tool("math_eval", serde_json::json!({"expression": "1 / 0"}));
    assert!(is_error(&r), "division by zero should produce an error");
}

#[test]
fn test_huge_number_math() {
    let r = call_tool(
        "math_eval",
        serde_json::json!({"expression": "99999999999999999999 + 1"}),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));

    let r = call_tool("math_eval", serde_json::json!({"expression": "2 ** 100"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));

    let r = call_tool(
        "math_eval",
        serde_json::json!({"expression": "999999999 * 999999999"}),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_deeply_nested_json() {
    // Build a 10-level deep nested JSON
    let mut inner = r#""leaf""#.to_string();
    for _ in 0..10 {
        inner = format!(r#"{{"nested": {}}}"#, inner);
    }

    let r = call_tool(
        "json_extract",
        serde_json::json!({
            "text": inner,
            "pointer": "/nested/nested/nested/nested/nested/nested/nested/nested/nested/nested"
        }),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["found"], true);
    assert_eq!(r["result"]["value"], "leaf");
}

#[test]
fn test_empty_json_objects() {
    let r = call_tool(
        "json_extract",
        serde_json::json!({
            "text": "{}",
            "pointer": ""
        }),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["valid_json"], true);
    assert_eq!(r["result"]["value_type"], "object");

    let r = call_tool(
        "json_extract",
        serde_json::json!({
            "text": "[]",
            "pointer": ""
        }),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["value_type"], "array");

    let r = call_tool(
        "json_compare",
        serde_json::json!({
            "a": "{}",
            "b": "{}"
        }),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["equal"], true);

    let r = call_tool(
        "json_compare",
        serde_json::json!({
            "a": "[]",
            "b": "[]"
        }),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["equal"], true);
}

#[test]
fn test_unicode_only_input() {
    // Emoji
    let r = call_tool("text_measure", serde_json::json!({"text": "😀🎉🚀"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["graphemes"], 3);

    // CJK characters
    let r = call_tool("text_measure", serde_json::json!({"text": "你好世界"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["codepoints"], 4);

    // RTL text
    let r = call_tool("text_inspect", serde_json::json!({"text": "مرحبا"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));

    // Mixed scripts
    let r = call_tool(
        "text_inspect",
        serde_json::json!({"text": "hello 你好 مرحبا"}),
    );
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    let mixed = r["result"]["metrics"]["unicode_risks"]["mixed_scripts"]
        .as_bool()
        .unwrap();
    assert!(mixed, "should detect mixed scripts");
}

#[test]
fn test_whitespace_only_input() {
    let r = call_tool("text_measure", serde_json::json!({"text": "   \t\n  "}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["words"], 0);

    let r = call_tool("text_equal", serde_json::json!({"a": "  ", "b": "  "}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["equal"], true);

    let r = call_tool("text_inspect", serde_json::json!({"text": "   "}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_single_char_input() {
    let r = call_tool("text_measure", serde_json::json!({"text": "x"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["codepoints"], 1);
    assert_eq!(r["result"]["graphemes"], 1);
    assert_eq!(r["result"]["bytes_utf8"], 1);
    assert_eq!(r["result"]["words"], 1);

    let r = call_tool("text_equal", serde_json::json!({"a": "a", "b": "a"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["equal"], true);

    let r = call_tool("text_equal", serde_json::json!({"a": "a", "b": "b"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["equal"], false);

    // Single multi-byte character
    let r = call_tool("text_measure", serde_json::json!({"text": "é"}));
    assert_eq!(r.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(r["result"]["codepoints"], 1);
    assert_eq!(r["result"]["bytes_utf8"], 2);
}
