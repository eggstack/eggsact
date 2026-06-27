use crate::parity::compare_tool_text_parity;

#[test]
fn test_text_transform_unicode_serialization() {
    let args = serde_json::json!({
        "text": "Café",
        "operations": ["casefold"],
    });
    let result = compare_tool_text_parity("text_transform", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_json_canonicalize_unicode_serialization() {
    let args = serde_json::json!({
        "text": r#"{"accent":"é","emoji":"😀"}"#,
    });
    let result = compare_tool_text_parity("json_canonicalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
