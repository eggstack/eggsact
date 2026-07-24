use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod test_bug_fixes;
mod test_error_handling;
mod test_semantic_parity;
mod test_tools_core;
mod test_tools_list;
mod test_tools_phase4;
mod test_tools_phase5;
mod test_tools_tier0;
mod test_tools_tier1;
mod test_tools_tier2;
mod test_tools_tier3;

pub struct ParityTestResult {
    pub passed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u32,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
}

fn python_command() -> Command {
    let mut command = Command::new("python3");
    command.args(["-m", "eggcalc.mcp.server"]);

    if let Some(root) = python_reference_root() {
        command.current_dir(root);
    }

    command
}

fn python_reference_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("EGGCALC_ROOT") {
        let root = PathBuf::from(root);
        if root.is_dir() {
            return Some(root);
        }
    }

    let sibling = Path::new(env!("CARGO_MANIFEST_DIR")).join("../eggcalc");
    sibling.is_dir().then_some(sibling)
}

fn initialize_request() -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "eggsact-parity",
                "version": env!("CARGO_PKG_VERSION")
            }
        },
        "id": 0
    })
}

fn write_session_request<W: Write>(stdin: &mut W, request: &str) -> Result<(), String> {
    let request_value: Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
    let messages = if request_value.get("method").and_then(Value::as_str) == Some("initialize") {
        vec![request.to_string()]
    } else {
        vec![
            serde_json::to_string(&initialize_request()).map_err(|e| e.to_string())?,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })
            .to_string(),
            request.to_string(),
        ]
    };

    for message in messages {
        stdin
            .write_all(message.as_bytes())
            .map_err(|e| e.to_string())?;
        stdin.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn parse_last_response(response_text: &str) -> Result<JsonRpcResponse, String> {
    response_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|e| e.to_string()))
        .last()
        .ok_or_else(|| "MCP server returned no JSON-RPC response".to_string())?
}

fn parse_last_value(response_text: &str) -> Value {
    response_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .last()
        .unwrap_or_else(|| serde_json::json!({"parse_error": response_text}))
}

fn run_jsonrpc(mut command: Command, request: &str) -> Value {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("Failed to spawn MCP process: {error}"));

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        write_session_request(&mut stdin, request).expect("Failed to write MCP session");
    }

    let output = child
        .wait_with_output()
        .expect("Failed to wait for MCP process");
    parse_last_value(&String::from_utf8_lossy(&output.stdout))
}

pub fn run_rust_jsonrpc(request: &str) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eggsact"));
    command.arg("--mcp");
    run_jsonrpc(command, request)
}

pub fn run_python_jsonrpc(request: &str) -> Value {
    run_jsonrpc(python_command(), request)
}

fn run_python_request(tool_name: &str, arguments: Value, request_id: u32) -> Option<Value> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        }),
        id: request_id,
    };

    let request_str = serde_json::to_string(&request).unwrap();

    let mut child = python_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        write_session_request(&mut stdin, &request_str).ok()?;
    }

    let output = child.wait_with_output().ok()?;
    let response_text = String::from_utf8_lossy(&output.stdout);

    let response = parse_last_response(&response_text).ok()?;

    if let Some(error) = response.error {
        return Some(serde_json::json!({
            "ok": false,
            "error": error["message"]
        }));
    }

    response
        .result?
        .get("content")?
        .as_array()?
        .first()?
        .get("text")
        .cloned()
}

fn run_rust_tool(tool_name: &str, arguments: Value) -> Option<Value> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        }),
        id: 1,
    };

    let request_str = serde_json::to_string(&request).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        write_session_request(&mut stdin, &request_str).ok()?;
    }

    let output = child.wait_with_output().ok()?;
    let response_text = String::from_utf8_lossy(&output.stdout);

    let response = parse_last_response(&response_text).ok()?;

    if let Some(error) = response.error {
        return Some(serde_json::json!({
            "ok": false,
            "error": error["message"]
        }));
    }

    response
        .result?
        .get("content")?
        .as_array()?
        .first()?
        .get("text")
        .cloned()
}

pub fn compare_tool_parity(tool_name: &str, arguments: Value) -> ParityTestResult {
    let python_out = run_python_request(tool_name, arguments.clone(), 1);
    let rust_out = run_rust_tool(tool_name, arguments.clone());

    if python_out.is_none() || rust_out.is_none() {
        let msg = match (python_out.is_none(), rust_out.is_none()) {
            (true, true) => format!(
                "Parity test '{}': both Python and Rust MCP servers unavailable",
                tool_name
            ),
            (true, false) => format!("Parity test '{}': Python MCP server unavailable", tool_name),
            (false, true) => format!("Parity test '{}': Rust MCP server unavailable", tool_name),
            (false, false) => unreachable!(),
        };
        return ParityTestResult {
            passed: false,
            error: Some(msg),
        };
    }

    let python_out = python_out.unwrap();
    let rust_out = rust_out.unwrap();

    let r_str = rust_out.as_str().unwrap_or("{}");
    let p_str = python_out.as_str().unwrap_or("{}");
    let r_val: Value =
        serde_json::from_str(r_str).unwrap_or_else(|_| serde_json::json!({"parse_error": r_str}));
    let p_val: Value =
        serde_json::from_str(p_str).unwrap_or_else(|_| serde_json::json!({"parse_error": p_str}));

    let passed = r_val == p_val;

    ParityTestResult {
        passed,
        error: if !passed {
            Some("Output mismatch".to_string())
        } else {
            None
        },
    }
}

pub fn compare_tool_parity_superset(tool_name: &str, arguments: Value) -> ParityTestResult {
    let python_out = run_python_request(tool_name, arguments.clone(), 1);
    let rust_out = run_rust_tool(tool_name, arguments.clone());

    if python_out.is_none() || rust_out.is_none() {
        let msg = match (python_out.is_none(), rust_out.is_none()) {
            (true, true) => format!(
                "Parity test '{}': both Python and Rust MCP servers unavailable",
                tool_name
            ),
            (true, false) => format!("Parity test '{}': Python MCP server unavailable", tool_name),
            (false, true) => format!("Parity test '{}': Rust MCP server unavailable", tool_name),
            (false, false) => unreachable!(),
        };
        return ParityTestResult {
            passed: false,
            error: Some(msg),
        };
    }

    let python_out = python_out.unwrap();
    let rust_out = rust_out.unwrap();

    let r_str = rust_out.as_str().unwrap_or("{}");
    let p_str = python_out.as_str().unwrap_or("{}");
    let r_val: Value =
        serde_json::from_str(r_str).unwrap_or_else(|_| serde_json::json!({"parse_error": r_str}));
    let p_val: Value =
        serde_json::from_str(p_str).unwrap_or_else(|_| serde_json::json!({"parse_error": p_str}));

    // Check that Python output is a subset of Rust output (Rust may have extra fields)
    let passed = is_subset(&p_val, &r_val);

    ParityTestResult {
        passed,
        error: if !passed {
            Some(format!(
                "Rust output is not a superset of Python output\nPython: {}\nRust: {}",
                p_val, r_val
            ))
        } else {
            None
        },
    }
}

fn is_subset(python: &Value, rust: &Value) -> bool {
    match (python, rust) {
        (Value::Object(p_obj), Value::Object(r_obj)) => {
            for (key, p_val) in p_obj {
                match r_obj.get(key) {
                    Some(r_val) => {
                        if !is_subset(p_val, r_val) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        (Value::Array(p_arr), Value::Array(r_arr)) => {
            if p_arr.len() != r_arr.len() {
                return false;
            }
            p_arr.iter().zip(r_arr.iter()).all(|(p, r)| is_subset(p, r))
        }
        _ => python == rust,
    }
}

pub fn compare_tool_text_parity(tool_name: &str, arguments: Value) -> ParityTestResult {
    let python_out = run_python_request(tool_name, arguments.clone(), 1);
    let rust_out = run_rust_tool(tool_name, arguments);

    if python_out.is_none() || rust_out.is_none() {
        let msg = match (python_out.is_none(), rust_out.is_none()) {
            (true, true) => format!(
                "Text parity test '{}': both Python and Rust MCP servers unavailable",
                tool_name
            ),
            (true, false) => format!(
                "Text parity test '{}': Python MCP server unavailable",
                tool_name
            ),
            (false, true) => format!(
                "Text parity test '{}': Rust MCP server unavailable",
                tool_name
            ),
            (false, false) => unreachable!(),
        };
        return ParityTestResult {
            passed: false,
            error: Some(msg),
        };
    }

    let python_out = python_out.unwrap();
    let rust_out = rust_out.unwrap();

    let python_text = match python_out.as_str() {
        Some(text) => text,
        None => {
            return ParityTestResult {
                passed: false,
                error: Some(format!(
                    "Text parity test '{}': Python content[0].text was not a string",
                    tool_name
                )),
            };
        }
    };
    let rust_text = match rust_out.as_str() {
        Some(text) => text,
        None => {
            return ParityTestResult {
                passed: false,
                error: Some(format!(
                    "Text parity test '{}': Rust content[0].text was not a string",
                    tool_name
                )),
            };
        }
    };

    let python_parsed: Value = serde_json::from_str(python_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": python_text}));
    let rust_parsed: Value = serde_json::from_str(rust_text)
        .unwrap_or_else(|_| serde_json::json!({"parse_error": rust_text}));

    if python_text != rust_text {
        return ParityTestResult {
            passed: false,
            error: Some(format!(
                "Text parity test '{}' raw mismatch: python={:?}, rust={:?}",
                tool_name, python_text, rust_text
            )),
        };
    }

    if python_parsed != rust_parsed {
        return ParityTestResult {
            passed: false,
            error: Some(format!(
                "Text parity test '{}' parsed payload mismatch",
                tool_name
            )),
        };
    }

    ParityTestResult {
        passed: true,
        error: None,
    }
}

pub fn run_python_mcp_request(request: &Value) -> Result<Value, String> {
    let response = run_python_jsonrpc(&serde_json::to_string(request).map_err(|e| e.to_string())?);

    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Python MCP request failed");
        return Err(message.to_string());
    }

    response
        .get("result")
        .cloned()
        .ok_or_else(|| "Python MCP response missing result".to_string())
}

fn run_rust_mcp_request(request: &Value) -> Result<Value, String> {
    let response = run_rust_jsonrpc(&serde_json::to_string(request).map_err(|e| e.to_string())?);

    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Rust MCP request failed");
        return Err(message.to_string());
    }

    response
        .get("result")
        .cloned()
        .ok_or_else(|| "Rust MCP response missing result".to_string())
}

fn extract_tool_names(result: &Value) -> Result<Vec<String>, String> {
    let tools = result
        .get("tools")
        .and_then(|tools| tools.as_array())
        .ok_or_else(|| "tools/list response missing tools array".to_string())?;

    tools
        .iter()
        .enumerate()
        .map(|(idx, tool)| {
            tool.get("name")
                .and_then(|name| name.as_str())
                .map(|name| name.to_string())
                .ok_or_else(|| {
                    format!(
                        "tools/list response tool at index {} is missing a name",
                        idx
                    )
                })
        })
        .collect()
}

pub fn compare_tools_list_parity(schema_detail: &str) -> ParityTestResult {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {"schema_detail": schema_detail},
        "id": 1,
    });

    let python_result = match run_python_mcp_request(&request) {
        Ok(result) => result,
        Err(error) => {
            return ParityTestResult {
                passed: false,
                error: Some(format!(
                    "Parity test 'tools/list' ({}) failed against Python MCP server: {}",
                    schema_detail, error
                )),
            };
        }
    };

    let rust_result = match run_rust_mcp_request(&request) {
        Ok(result) => result,
        Err(error) => {
            return ParityTestResult {
                passed: false,
                error: Some(format!(
                    "Parity test 'tools/list' ({}) failed against Rust MCP server: {}",
                    schema_detail, error
                )),
            };
        }
    };

    let python_names = match extract_tool_names(&python_result) {
        Ok(names) => names,
        Err(error) => {
            return ParityTestResult {
                passed: false,
                error: Some(format!(
                    "Parity test 'tools/list' ({}) could not read Python tool names: {}",
                    schema_detail, error
                )),
            };
        }
    };

    let rust_names = match extract_tool_names(&rust_result) {
        Ok(names) => names,
        Err(error) => {
            return ParityTestResult {
                passed: false,
                error: Some(format!(
                    "Parity test 'tools/list' ({}) could not read Rust tool names: {}",
                    schema_detail, error
                )),
            };
        }
    };

    if python_names != rust_names {
        let first_difference = python_names
            .iter()
            .zip(rust_names.iter())
            .position(|(python_name, rust_name)| python_name != rust_name);
        let error = match first_difference {
            Some(idx) => format!(
                "Parity test 'tools/list' ({}) order mismatch at index {}: python={:?}, rust={:?}",
                schema_detail,
                idx,
                python_names.get(idx),
                rust_names.get(idx)
            ),
            None => format!(
                "Parity test 'tools/list' ({}) length mismatch: python={}, rust={}",
                schema_detail,
                python_names.len(),
                rust_names.len()
            ),
        };
        return ParityTestResult {
            passed: false,
            error: Some(error),
        };
    }

    ParityTestResult {
        passed: true,
        error: None,
    }
}
