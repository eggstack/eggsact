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
    String::from_utf8(output.stdout).unwrap_or_default()
}

fn call_tool_and_get_result(request: &str) -> Value {
    let response = mcp_request(request);
    let parsed: Value = serde_json::from_str(&response).expect("Invalid JSON response");
    parsed["result"]
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_else(|| {
            panic!(
                "Failed to extract tool result from response: {}",
                &response[..response.len().min(2000)]
            )
        })
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

// ============================================================
// import_export_inspect
// ============================================================

#[test]
fn test_import_export_inspect_rust_use() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "use std::collections::HashMap;\nuse crate::utils;\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("language"),
        Some(&Value::String("rust".to_string()))
    );
    let stmts = inner.get("statements").unwrap().as_array().unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("use".to_string()))
    );
    assert_eq!(
        stmts[0].get("module").and_then(|v| v.as_str()),
        Some("std::collections::HashMap")
    );
    assert_eq!(stmts[0].get("line"), Some(&Value::Number(1.into())));
}

#[test]
fn test_import_export_inspect_rust_pub_use() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "pub use crate::Foo;\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let stmts = inner.get("statements").unwrap().as_array().unwrap();
    assert_eq!(stmts.len(), 1);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("pub_use".to_string()))
    );
}

#[test]
fn test_import_export_inspect_rust_mod_and_extern() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "mod foo;\nextern crate bar;\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("mod".to_string()))
    );
    assert_eq!(
        stmts[1].get("kind"),
        Some(&Value::String("extern_crate".to_string()))
    );
}

#[test]
fn test_import_export_inspect_python_import() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "import os\nimport numpy as np\n",
            "language": "python"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("import".to_string()))
    );
    assert_eq!(
        stmts[1].get("kind"),
        Some(&Value::String("import_alias".to_string()))
    );
}

#[test]
fn test_import_export_inspect_python_from_import() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "from os import path\nfrom collections import *\n",
            "language": "python"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("from_import".to_string()))
    );
    assert_eq!(
        stmts[1].get("kind"),
        Some(&Value::String("from_import_star".to_string()))
    );
    // Star import should produce a finding
    let findings = result.get("findings").unwrap().as_array().unwrap();
    assert!(!findings.is_empty());
}

#[test]
fn test_import_export_inspect_js_import() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "import React from 'react';\nimport { useState } from 'react';\n",
            "language": "javascript"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("import_from".to_string()))
    );
    assert_eq!(
        stmts[0].get("module").and_then(|v| v.as_str()),
        Some("react")
    );
}

#[test]
fn test_import_export_inspect_js_require() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "const fs = require('fs');\n",
            "language": "javascript"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 1);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("require".to_string()))
    );
}

#[test]
fn test_import_export_inspect_js_export() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "export default function foo() {}\nexport const x = 1;\n",
            "language": "javascript"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("export_default".to_string()))
    );
    assert_eq!(
        stmts[1].get("kind"),
        Some(&Value::String("export_variable".to_string()))
    );
}

#[test]
fn test_import_export_inspect_go_import() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "import \"fmt\"\nimport \"github.com/foo/bar\"\n",
            "language": "go"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(stmts.len(), 2);
    assert_eq!(
        stmts[0].get("kind"),
        Some(&Value::String("import".to_string()))
    );
    assert_eq!(stmts[0].get("module").and_then(|v| v.as_str()), Some("fmt"));
}

#[test]
fn test_import_export_inspect_auto_detect_rust() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "use std::io;\nfn main() {}"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("language"),
        Some(&Value::String("rust".to_string()))
    );
}

#[test]
fn test_import_export_inspect_auto_detect_python() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "import os\ndef main(): pass"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("language"),
        Some(&Value::String("python".to_string()))
    );
}

#[test]
fn test_import_export_inspect_empty_source() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(stmts.is_empty());
}

#[test]
fn test_import_export_inspect_unknown_language() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "random text with no imports",
            "language": "auto"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("language"),
        Some(&Value::String("unknown".to_string()))
    );
    let stmts = inner.get("statements").unwrap().as_array().unwrap();
    assert!(stmts.is_empty());
}

#[test]
fn test_import_export_inspect_include_line_text() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "use std::io;\n",
            "language": "rust",
            "include_line_text": true
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let stmts = result
        .get("result")
        .unwrap()
        .get("statements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(
        stmts[0].get("raw_text").and_then(|v| v.as_str()),
        Some("use std::io;")
    );
}

#[test]
fn test_import_export_inspect_max_statements() {
    let result = call_tool(
        "import_export_inspect",
        serde_json::json!({
            "source": "use a;\nuse b;\nuse c;\nuse d;\nuse e;\n",
            "language": "rust",
            "max_statements": 2
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("truncated"), Some(&Value::Bool(true)));
    let stmts = inner.get("statements").unwrap().as_array().unwrap();
    assert_eq!(stmts.len(), 2);
}

// ============================================================
// code_block_map
// ============================================================

#[test]
fn test_code_block_map_rust_functions() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "fn foo() {\n    let x = 1;\n}\nfn bar() {\n    let y = 2;\n}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let blocks = inner.get("blocks").unwrap().as_array().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks[0].get("kind"),
        Some(&Value::String("fn".to_string()))
    );
    assert_eq!(
        blocks[0].get("name"),
        Some(&Value::String("foo".to_string()))
    );
    assert_eq!(
        blocks[1].get("name"),
        Some(&Value::String("bar".to_string()))
    );
}

#[test]
fn test_code_block_map_rust_struct_and_impl() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "struct Foo {\n    x: i32,\n}\nimpl Foo {\n    fn new() -> Self { Foo { x: 0 } }\n}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks[0].get("kind"),
        Some(&Value::String("struct".to_string()))
    );
    assert_eq!(
        blocks[1].get("kind"),
        Some(&Value::String("impl".to_string()))
    );
}

#[test]
fn test_code_block_map_rust_trait_and_enum() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "trait Drawable {\n    fn draw(&self);\n}\nenum Color {\n    Red,\n    Blue,\n}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks[0].get("kind"),
        Some(&Value::String("trait".to_string()))
    );
    assert_eq!(
        blocks[1].get("kind"),
        Some(&Value::String("enum".to_string()))
    );
}

#[test]
fn test_code_block_map_python_classes_and_defs() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "class Foo:\n    pass\ndef bar():\n    pass\nasync def baz():\n    pass\n",
            "language": "python"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(blocks.len() >= 3);
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|b| b.get("kind").and_then(|v| v.as_str()).unwrap_or(""))
        .collect();
    assert!(kinds.contains(&"class"));
    assert!(kinds.contains(&"def"));
    assert!(kinds.contains(&"async_def"));
}

#[test]
fn test_code_block_map_js_functions_and_classes() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "function foo() {}\nasync function bar() {}\nclass Baz {}\n",
            "language": "javascript"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(blocks.len() >= 3);
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|b| b.get("kind").and_then(|v| v.as_str()).unwrap_or(""))
        .collect();
    assert!(kinds.contains(&"function"));
    assert!(kinds.contains(&"class"));
}

#[test]
fn test_code_block_map_js_arrow_function() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "const add = (a, b) => {\n    return a + b;\n};\n",
            "language": "javascript"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(!blocks.is_empty());
    assert_eq!(
        blocks[0].get("kind"),
        Some(&Value::String("arrow_function".to_string()))
    );
}

#[test]
fn test_code_block_map_go_func() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "func main() {\n    fmt.Println(\"hello\")\n}\ntype Foo struct {\n    X int\n}\n",
            "language": "go"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(blocks.len() >= 2);
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|b| b.get("kind").and_then(|v| v.as_str()).unwrap_or(""))
        .collect();
    assert!(kinds.contains(&"func"));
    assert!(kinds.contains(&"type"));
}

#[test]
fn test_code_block_map_markdown_headings_and_fences() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "# Title\nSome text\n## Section\n```python\nprint('hello')\n```\n",
            "language": "markdown"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(blocks.len() >= 3);
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|b| b.get("kind").and_then(|v| v.as_str()).unwrap_or(""))
        .collect();
    assert!(kinds.contains(&"heading"));
    assert!(kinds.contains(&"fenced_code"));
}

#[test]
fn test_code_block_map_empty_source() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(blocks.is_empty());
}

#[test]
fn test_code_block_map_unknown_language() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "some random text\nwith no code blocks",
            "language": "auto"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("language"),
        Some(&Value::String("unknown".to_string()))
    );
    // findings are on the ToolResponse envelope, not inside result
    let findings = result.get("findings").unwrap().as_array().unwrap();
    assert!(!findings.is_empty());
}

#[test]
fn test_code_block_map_max_blocks() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\n",
            "language": "rust",
            "max_blocks": 2
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("truncated"), Some(&Value::Bool(true)));
    assert_eq!(inner.get("block_count"), Some(&Value::Number(5.into())));
}

#[test]
fn test_code_block_map_line_numbers() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "fn foo() {\n    let x = 1;\n}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let blocks = result
        .get("result")
        .unwrap()
        .get("blocks")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(blocks[0].get("start_line"), Some(&Value::Number(1.into())));
    assert_eq!(blocks[0].get("end_line"), Some(&Value::Number(3.into())));
}

#[test]
fn test_code_block_map_verdict_allow() {
    let result = call_tool(
        "code_block_map",
        serde_json::json!({
            "source": "fn foo() {}",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("language"),
        Some(&Value::String("rust".to_string()))
    );
    // verdict is nested inside result
    assert_eq!(inner.get("verdict").and_then(|v| v.as_str()), Some("allow"));
}

// ============================================================
// symbol_name_diff
// ============================================================

#[test]
fn test_symbol_name_diff_additions() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn foo() {}\n",
            "new_source": "fn foo() {}\nfn bar() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let added = inner.get("added").unwrap().as_array().unwrap();
    assert!(added.contains(&Value::String("bar".to_string())));
    let removed = inner.get("removed").unwrap().as_array().unwrap();
    assert!(removed.is_empty());
}

#[test]
fn test_symbol_name_diff_removals() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn foo() {}\nfn bar() {}\n",
            "new_source": "fn foo() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let removed = inner.get("removed").unwrap().as_array().unwrap();
    assert!(removed.contains(&Value::String("bar".to_string())));
    let added = inner.get("added").unwrap().as_array().unwrap();
    assert!(added.is_empty());
}

#[test]
fn test_symbol_name_diff_no_changes() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn foo() {}\nfn bar() {}\n",
            "new_source": "fn foo() {}\nfn bar() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("added_count"), Some(&Value::Number(0.into())));
    assert_eq!(inner.get("removed_count"), Some(&Value::Number(0.into())));
    let unchanged = inner.get("unchanged").unwrap().as_array().unwrap();
    assert!(unchanged.len() >= 2);
}

#[test]
fn test_symbol_name_diff_renames_detected() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn calculate_total() {}\n",
            "new_source": "fn calculate_sum() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let possible_renames = inner.get("possible_renames").unwrap().as_array().unwrap();
    assert!(!possible_renames.is_empty());
    assert_eq!(
        possible_renames[0].get("old").and_then(|v| v.as_str()),
        Some("calculate_total")
    );
    assert_eq!(
        possible_renames[0].get("new").and_then(|v| v.as_str()),
        Some("calculate_sum")
    );
}

#[test]
fn test_symbol_name_diff_mixed_changes() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn alpha() {}\nfn beta() {}\nfn gamma() {}\n",
            "new_source": "fn alpha() {}\nfn beta_renamed() {}\nfn delta() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let added = inner.get("added").unwrap().as_array().unwrap();
    let removed = inner.get("removed").unwrap().as_array().unwrap();
    assert_eq!(inner.get("added_count"), Some(&Value::Number(2.into())));
    assert_eq!(inner.get("removed_count"), Some(&Value::Number(2.into())));
    assert!(added
        .iter()
        .any(|a| a.as_str() == Some("beta_renamed") || a.as_str() == Some("delta")));
    assert!(removed
        .iter()
        .any(|r| r.as_str() == Some("beta") || r.as_str() == Some("gamma")));
}

#[test]
fn test_symbol_name_diff_empty_old() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "",
            "new_source": "fn foo() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("added_count"), Some(&Value::Number(1.into())));
    assert_eq!(inner.get("removed_count"), Some(&Value::Number(0.into())));
}

#[test]
fn test_symbol_name_diff_empty_new() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn foo() {}\n",
            "new_source": "",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("added_count"), Some(&Value::Number(0.into())));
    assert_eq!(inner.get("removed_count"), Some(&Value::Number(1.into())));
}

#[test]
fn test_symbol_name_diff_both_empty() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "",
            "new_source": "",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("added_count"), Some(&Value::Number(0.into())));
    assert_eq!(inner.get("removed_count"), Some(&Value::Number(0.into())));
}

#[test]
fn test_symbol_name_diff_python() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "def foo():\n    pass\nclass Bar:\n    pass\n",
            "new_source": "def foo():\n    pass\ndef baz():\n    pass\n",
            "language": "python"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let added = inner.get("added").unwrap().as_array().unwrap();
    let removed = inner.get("removed").unwrap().as_array().unwrap();
    assert!(added.iter().any(|a| a.as_str() == Some("baz")));
    assert!(removed.iter().any(|r| r.as_str() == Some("Bar")));
}

#[test]
fn test_symbol_name_diff_reorder_only() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn alpha() {}\nfn beta() {}\n",
            "new_source": "fn beta() {}\nfn alpha() {}\n",
            "language": "rust"
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("added_count"), Some(&Value::Number(0.into())));
    assert_eq!(inner.get("removed_count"), Some(&Value::Number(0.into())));
}

#[test]
fn test_symbol_name_diff_findings_present() {
    let result = call_tool(
        "symbol_name_diff",
        serde_json::json!({
            "old_source": "fn foo() {}",
            "new_source": "fn bar() {}",
            "language": "rust"
        }),
    );
    // When there are adds/removes, findings should be present
    let findings = result.get("findings").unwrap().as_array().unwrap();
    assert!(!findings.is_empty());
}

// ============================================================
// lockfile_inspect
// ============================================================

#[test]
fn test_lockfile_inspect_cargo_added() {
    let before = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"tokio\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
";

    let after = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"tokio\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"hyper\"
version = \"0.14.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("ecosystem"),
        Some(&Value::String("cargo".to_string()))
    );
    let changes = inner.get("changes").unwrap().as_array().unwrap();
    assert!(!changes.is_empty());
    assert!(changes.iter().any(|c| {
        c.get("type").and_then(|v| v.as_str()) == Some("added")
            && c.get("package").and_then(|v| v.as_str()) == Some("hyper")
    }));
}

#[test]
fn test_lockfile_inspect_cargo_removed() {
    let before = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"

[[package]]
name = \"old-crate\"
version = \"2.0.0\"
";

    let after = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let changes = result
        .get("result")
        .unwrap()
        .get("changes")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(changes.iter().any(|c| {
        c.get("type").and_then(|v| v.as_str()) == Some("removed")
            && c.get("package").and_then(|v| v.as_str()) == Some("old-crate")
    }));
}

#[test]
fn test_lockfile_inspect_cargo_updated() {
    let before = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
";

    let after = "\
[[package]]
name = \"serde\"
version = \"1.1.0\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let changes = result
        .get("result")
        .unwrap()
        .get("changes")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(changes.iter().any(|c| {
        c.get("type").and_then(|v| v.as_str()) == Some("updated")
            && c.get("package").and_then(|v| v.as_str()) == Some("serde")
    }));
}

#[test]
fn test_lockfile_inspect_npm_diff() {
    let diff = "\
+\"lodash\": {
+  \"version\": \"4.17.21\"
+}
-\"old-pkg\": {
-  \"version\": \"1.0.0\"
+}
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "package-lock.json",
            "diff": diff
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("ecosystem"),
        Some(&Value::String("npm".to_string()))
    );
}

#[test]
fn test_lockfile_inspect_go_diff() {
    let diff = "\
+github.com/foo/bar v1.0.0 h1:abc123=
-github.com/baz/qux v0.5.0 h1:def456=
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "go.sum",
            "diff": diff
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("ecosystem"),
        Some(&Value::String("go".to_string()))
    );
    let changes = inner.get("changes").unwrap().as_array().unwrap();
    assert_eq!(changes.len(), 2);
}

#[test]
fn test_lockfile_inspect_no_inputs_error() {
    let result = call_tool_and_get_result(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "lockfile_inspect", "arguments": {"path": "Cargo.lock"}},
            "id": 1
        })
        .to_string(),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(false)));
}

#[test]
fn test_lockfile_inspect_empty_lockfile() {
    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": "",
            "after": ""
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let changes = inner.get("changes").unwrap().as_array().unwrap();
    assert!(changes.is_empty());
}

#[test]
fn test_lockfile_inspect_large_churn() {
    // Build before/after with > 50 changed packages to trigger large_churn
    let mut before = String::new();
    let mut after = String::new();
    // 60 packages in before
    for i in 0..60 {
        before.push_str(&format!(
            "[[package]]\nname = \"pkg{}\"\nversion = \"1.0.0\"\n\n",
            i
        ));
    }
    // 60 different packages in after (all different names → 60 added + 60 removed = 120 changes)
    for i in 60..120 {
        after.push_str(&format!(
            "[[package]]\nname = \"new{}\"\nversion = \"1.0.0\"\n\n",
            i
        ));
    }

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let inner = result.get("result").unwrap();
    let change_count = inner
        .get("change_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(
        change_count > 50,
        "Expected > 50 changes for large_churn, got {}",
        change_count
    );
    assert_eq!(inner.get("large_churn"), Some(&Value::Bool(true)));
}

#[test]
fn test_lockfile_inspect_max_packages() {
    let mut before = String::from("");
    let mut after = String::from("");
    for i in 0..10 {
        before.push_str(&format!(
            "[[package]]\nname = \"pkg{}\"\nversion = \"1.0.0\"\n\n",
            i
        ));
        after.push_str(&format!(
            "[[package]]\nname = \"pkg{}\"\nversion = \"2.0.0\"\n\n",
            i
        ));
    }

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after,
            "max_packages": 3
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let changes = result
        .get("result")
        .unwrap()
        .get("changes")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(changes.len() <= 3);
}

#[test]
fn test_lockfile_inspect_cargo_ecosystem_auto_detect() {
    let content = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": content,
            "after": content
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(
        result.get("result").unwrap().get("ecosystem"),
        Some(&Value::String("cargo".to_string()))
    );
}

#[test]
fn test_lockfile_inspect_git_dependency_finding() {
    let before = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
";

    let after = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"

[[package]]
name = \"git-crate\"
version = \"0.1.0\"
source = \"git+https://github.com/foo/bar\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after
        }),
    );
    // Git dependency should produce a high-severity finding
    let findings = result.get("findings").unwrap().as_array().unwrap();
    let has_git_finding = findings.iter().any(|f| {
        f.get("message")
            .and_then(|m| m.as_str())
            .map(|m| m.contains("Git"))
            .unwrap_or(false)
    });
    assert!(has_git_finding);
}

#[test]
fn test_lockfile_inspect_verdict_review() {
    // A simple update should produce REVIEW verdict (medium/low severity findings)
    let before = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
";

    let after = "\
[[package]]
name = \"serde\"
version = \"1.1.0\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": before,
            "after": after
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    // verdict is nested inside result
    let inner = result.get("result").unwrap();
    assert_eq!(
        inner.get("verdict").and_then(|v| v.as_str()),
        Some("review")
    );
}

#[test]
fn test_lockfile_inspect_verdict_allow_no_changes() {
    let content = "\
[[package]]
name = \"serde\"
version = \"1.0.0\"
";

    let result = call_tool(
        "lockfile_inspect",
        serde_json::json!({
            "path": "Cargo.lock",
            "before": content,
            "after": content
        }),
    );
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    // verdict is nested inside result
    let inner = result.get("result").unwrap();
    assert_eq!(inner.get("verdict").and_then(|v| v.as_str()), Some("allow"));
}
