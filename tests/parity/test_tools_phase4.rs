use crate::parity::compare_tool_parity;

#[test]
fn test_regex_finditer_unicode_spans() {
    let args = serde_json::json!({"text": "é e_1", "pattern": r"\w+"});
    let result = compare_tool_parity("regex_finditer", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_position_emoji_utf16_offset() {
    let args = serde_json::json!({"text": "a😀b", "utf16_offset": 2});
    let result = compare_tool_parity("text_position", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_shell_split_edge_cases() {
    for command in [
        "echo hi # comment",
        "echo \"# not comment\"",
        "echo hi\\ there",
        "echo foo|bar",
        "echo foo | bar",
        "echo hi\\",
        "echo \"unterminated",
    ] {
        let args = serde_json::json!({"command": command});
        let result = compare_tool_parity("shell_split", args);
        assert!(
            result.passed,
            "Parity failed for {:?}: {:?}",
            command, result.error
        );
    }
}

#[test]
fn test_version_compare_phase4_cases() {
    for (a, b, scheme) in [
        ("1.0.0-alpha", "1.0.0", "semver"),
        ("1.0.0+build", "1.0.0", "semver"),
        ("1.2", "1.10", "loose"),
        ("1.0.0", "2.0.0", "pep440"),
        ("abc", "1.0.0", "semver"),
        ("1.0.0", "2.0.0", "unknown"),
    ] {
        let args = serde_json::json!({"a": a, "b": b, "scheme": scheme});
        let result = compare_tool_parity("version_compare", args);
        assert!(
            result.passed,
            "Parity failed for ({a:?}, {b:?}, {scheme:?}): {:?}",
            result.error
        );
    }
}

#[test]
fn test_path_analyze_phase4_cases() {
    let confusable_path = format!("C:\\f{}{}\\bar", '\u{43e}', '\u{43e}');
    for args in [
        serde_json::json!({"path": r"C:\foo\..\bar", "style": "windows"}),
        serde_json::json!({"path": ".gitignore", "style": "posix"}),
        serde_json::json!({"path": confusable_path, "style": "windows"}),
    ] {
        let result = compare_tool_parity("path_analyze", args);
        assert!(result.passed, "Parity failed: {:?}", result.error);
    }
}

#[test]
fn test_prompt_input_inspect_phase4_cases() {
    let long_line = "a".repeat(1001);
    for text in [
        "😀",
        "a\u{200b}b",
        "\u{202e}abc",
        "<!-- hi -->",
        "this is jailbreak",
        "foo [bar](http://x)",
        long_line.as_str(),
    ] {
        let args = serde_json::json!({"text": text});
        let result = compare_tool_parity("prompt_input_inspect", args);
        assert!(
            result.passed,
            "Parity failed for {:?}: {:?}",
            text, result.error
        );
    }
}

#[test]
fn test_prompt_input_inspect_ansi_case() {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "prompt_input_inspect",
            "arguments": {"text": "echo\u{1b}[31m"},
        }
    });
    let req_text = serde_json::to_string(&req).expect("request serialization failed");

    let python_out = std::process::Command::new("python3")
        .args(["-m", "eggcalc.mcp.server"])
        .current_dir("/Users/davidbowman/projects/eggcalc")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            {
                let mut stdin = child.stdin.take().expect("python stdin");
                use std::io::Write;
                stdin.write_all(req_text.as_bytes()).expect("write request");
                stdin.write_all(b"\n").expect("write newline");
            }
            child.wait_with_output()
        })
        .expect("Python MCP request failed");

    let rust_out = std::process::Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .current_dir("/Users/davidbowman/projects/eggcalc/eggsact")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            {
                let mut stdin = child.stdin.take().expect("rust stdin");
                use std::io::Write;
                stdin.write_all(req_text.as_bytes()).expect("write request");
                stdin.write_all(b"\n").expect("write newline");
            }
            child.wait_with_output()
        })
        .expect("Rust MCP request failed");

    let python_json: serde_json::Value =
        serde_json::from_slice(&python_out.stdout).expect("Python response parse failed");
    let rust_json: serde_json::Value =
        serde_json::from_slice(&rust_out.stdout).expect("Rust response parse failed");

    let python_text = python_json["result"]["content"][0]["text"]
        .as_str()
        .expect("Python text missing");
    let rust_text = rust_json["result"]["content"][0]["text"]
        .as_str()
        .expect("Rust text missing");

    let python_val: serde_json::Value =
        serde_json::from_str(python_text).expect("Python payload parse failed");
    let rust_val: serde_json::Value =
        serde_json::from_str(rust_text).expect("Rust payload parse failed");

    assert_eq!(python_val, rust_val, "Output mismatch");
}
