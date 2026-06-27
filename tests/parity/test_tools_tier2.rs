use crate::parity::compare_tool_parity;

#[test]
fn test_identifier_analyze_python() {
    let args = serde_json::json!({"text": "def_function", "scheme": "python"});
    let result = compare_tool_parity("identifier_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_identifier_analyze_snake_case() {
    let args = serde_json::json!({"text": "my_variable_name", "scheme": "snake_case"});
    let result = compare_tool_parity("identifier_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_identifier_analyze_camel_case() {
    let args = serde_json::json!({"text": "myVariableName", "scheme": "camel_case"});
    let result = compare_tool_parity("identifier_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_identifier_inspect_confusable() {
    let args = serde_json::json!({"text": "paypaI", "context": "identifier"});
    let result = compare_tool_parity("identifier_inspect", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_analyze_basic() {
    let args = serde_json::json!({"path": "/foo/bar/file.txt"});
    let result = compare_tool_parity("path_analyze", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_compare_equal() {
    let args = serde_json::json!({"a": "/foo/bar", "b": "/foo/bar"});
    let result = compare_tool_parity("path_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_scope_check_valid() {
    let args = serde_json::json!({"path": "/project/src/main.rs", "scope": "/project"});
    let result = compare_tool_parity("path_scope_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_scope_check_outside() {
    let args = serde_json::json!({"path": "/etc/passwd", "scope": "/project"});
    let result = compare_tool_parity("path_scope_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_json_shape_basic() {
    let args = serde_json::json!({"text": "{\"a\": 1, \"b\": {\"c\": 2}}", "max_depth": 3});
    let result = compare_tool_parity("json_shape", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_toml_shape_basic() {
    let args = serde_json::json!({"text": "key = \"value\"\n[section]\nname = \"test\""});
    let result = compare_tool_parity("toml_shape", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_line_range_extract_basic() {
    let args =
        serde_json::json!({"text": "line1\nline2\nline3\nline4\nline5", "start": 1, "end": 3});
    let result = compare_tool_parity("line_range_extract", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_line_range_compare_equal() {
    let args = serde_json::json!({"a": "line1\nline2\nline3", "b": "line1\nline2\nline3"});
    let result = compare_tool_parity("line_range_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_line_range_compare_different() {
    let args = serde_json::json!({"a": "line1\nline2\nline3", "b": "line1\nlineX\nline3"});
    let result = compare_tool_parity("line_range_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_basic() {
    let args = serde_json::json!({"text": "ls -la /tmp"});
    let result = compare_tool_parity("shell_split", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_quote_join_basic() {
    let args = serde_json::json!({"argv": ["ls", "-la", "path with spaces"]});
    let result = compare_tool_parity("shell_quote_join", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_argv_compare_equal() {
    let args = serde_json::json!({"a": ["ls", "-la"], "b": ["ls", "-la"]});
    let result = compare_tool_parity("argv_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_argv_compare_different() {
    let args = serde_json::json!({"a": ["ls", "-la"], "b": ["ls", "-l"]});
    let result = compare_tool_parity("argv_compare", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_markdown_structure_basic() {
    let args = serde_json::json!({"text": "# Title\n\nParagraph with `code`.\n\n## Section\n\nMore content."});
    let result = compare_tool_parity("markdown_structure", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_code_fence_extract_basic() {
    let args = serde_json::json!({"text": "```python\nprint('hello')\n```\n\nRegular text\n\n```rust\nfn main() {}\n```"});
    let result = compare_tool_parity("code_fence_extract", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_dotenv_validate_valid() {
    let args = serde_json::json!({"text": "KEY=value\nDEBUG=true\nPORT=8080"});
    let result = compare_tool_parity("dotenv_validate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_dotenv_validate_invalid() {
    let args = serde_json::json!({"text": "KEY=value\nINVALID_NO_EQUALS\nDEBUG=true"});
    let result = compare_tool_parity("dotenv_validate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_ini_validate_valid() {
    let args = serde_json::json!({"text": "[section]\nkey=value\n\n[another]\nname=test"});
    let result = compare_tool_parity("ini_validate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_ini_validate_invalid() {
    let args = serde_json::json!({"text": "no_header_key=value\n[section]\nkey=value"});
    let result = compare_tool_parity("ini_validate", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_patch_apply_check_valid() {
    let args = serde_json::json!({"patch": "--- original\n+++ modified\n@@ -1 +1 @@\n-old\n+new", "target": "old"});
    let result = compare_tool_parity("patch_apply_check", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_patch_summary_basic() {
    let args = serde_json::json!({"patch": "--- original\n+++ modified\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3"});
    let result = compare_tool_parity("patch_summary", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
