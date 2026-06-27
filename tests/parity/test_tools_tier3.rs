use crate::parity::compare_tool_parity;

#[test]
fn test_unicode_policy_check_identifier_strict() {
    let args = serde_json::json!({"text": "valid_name", "policy": "identifier_strict"});
    let result = compare_tool_parity("unicode_policy_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unicode_policy_check_with_confusable() {
    let args = serde_json::json!({"text": "paypal", "policy": "identifier_strict"});
    let result = compare_tool_parity("unicode_policy_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_canonicalize_text_identifier() {
    let args = serde_json::json!({"text": "HELLO", "profile": "identifier_compare", "include_mapping": false});
    let result = compare_tool_parity("canonicalize_text", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_canonicalize_text_human_label() {
    let args = serde_json::json!({"text": "  HELLO   WORLD  ", "profile": "human_label_compare", "include_mapping": false});
    let result = compare_tool_parity("canonicalize_text", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_identifier_table_inspect_basic() {
    let args = serde_json::json!({"texts": ["paypal", "PayPal", "PAYPAL"]});
    let result = compare_tool_parity("identifier_table_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_constraint_check_valid() {
    let args = serde_json::json!({"constraint": "^1.0.0", "version": "1.5.0"});
    let result = compare_tool_parity("version_constraint_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_constraint_check_invalid() {
    let args = serde_json::json!({"constraint": "^2.0.0", "version": "1.5.0"});
    let result = compare_tool_parity("version_constraint_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_cargo_toml_inspect_basic() {
    let args = serde_json::json!({"text": "[package]\nname = \"my crate\"\nversion = \"1.0.0\"\n\n[dependencies]\nserde = \"1.0\""});
    let result = compare_tool_parity("cargo_toml_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_schema_light_valid() {
    let args = serde_json::json!({"schema": "{\"type\": \"string\"}", "data": "\"hello\""});
    let result = compare_tool_parity("validate_schema_light", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_validate_schema_light_invalid() {
    let args = serde_json::json!({"schema": "{\"type\": \"string\"}", "data": "123"});
    let result = compare_tool_parity("validate_schema_light", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_replace_check_basic() {
    let args =
        serde_json::json!({"text": "hello world", "pattern": "world", "replacement": "rust"});
    let result = compare_tool_parity("text_replace_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_replace_check_no_match() {
    let args = serde_json::json!({"text": "hello world", "pattern": "xyz", "replacement": "rust"});
    let result = compare_tool_parity("text_replace_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_prompt_input_inspect_clean() {
    let args = serde_json::json!({"text": "This is a normal prompt"});
    let result = compare_tool_parity("prompt_input_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_prompt_input_inspect_with_hidden_chars() {
    let args = serde_json::json!({"text": "Hello\u{200b}World"});
    let result = compare_tool_parity("prompt_input_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_version_compare_basic() {
    let args = serde_json::json!({"a": "1.0.0", "b": "2.0.0"});
    let result = compare_tool_parity("version_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_diff_explain_basic() {
    let args = serde_json::json!({"a": "hello", "b": "hallo"});
    let result = compare_tool_parity("text_diff_explain", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_inspect_with_confusable() {
    let args = serde_json::json!({"text": "а"});
    let result = compare_tool_parity("text_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

// ---------------------------------------------------------------------------
// Composite tool parity tests
// ---------------------------------------------------------------------------

#[test]
fn test_text_security_inspect_clean() {
    let args = serde_json::json!({"text": "hello world"});
    let result = compare_tool_parity("text_security_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_security_inspect_with_hidden() {
    let args = serde_json::json!({"text": "hello\u{200b}world"});
    let result = compare_tool_parity("text_security_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_edit_preflight_basic() {
    let args = serde_json::json!({"original": "hello world", "old": "world", "new": "rust", "replacement_mode": "literal"});
    let result = compare_tool_parity("edit_preflight", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_command_preflight_safe() {
    let args = serde_json::json!({"command": "ls -la"});
    let result = compare_tool_parity("command_preflight", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_command_preflight_dangerous() {
    let args = serde_json::json!({"command": "echo $(rm -rf /)"});
    let result = compare_tool_parity("command_preflight", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_config_preflight_json() {
    let args = serde_json::json!({"text": "{\"key\": \"value\", \"num\": 42}", "format": "json"});
    let result = compare_tool_parity("config_preflight", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_config_preflight_toml() {
    let args = serde_json::json!({"text": "[package]\nname = \"test\"\nversion = \"0.1.0\"\n", "format": "toml"});
    let result = compare_tool_parity("config_preflight", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_structured_data_compare_identical() {
    let args = serde_json::json!({"a": "{\"x\": 1, \"y\": 2}", "b": "{\"x\": 1, \"y\": 2}"});
    let result = compare_tool_parity("structured_data_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_structured_data_compare_different() {
    let args = serde_json::json!({"a": "{\"x\": 1}", "b": "{\"x\": 2}"});
    let result = compare_tool_parity("structured_data_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
