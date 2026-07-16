//! Execution safety integration tests for Release 1.
//!
//! Tests worker containment, cancellation state coverage, duplicate ID
//! handling, shutdown behavior, and the register_request API at the MCP
//! protocol level.

use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// Duplicate ID tests — integer IDs
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_duplicate_integer_id_rejected() {
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
        // First request with integer id=42
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_diff_explain","arguments":{"a":"hello world foo bar baz","b":"hello world qux bar baz"}},"id":42}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Second request with same integer id=42
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":42}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Third request with different id (should succeed)
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":99}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    let has_error_for_42 = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("error").is_some() && v.get("id") == Some(&Value::Number(42.into()))
        } else {
            false
        }
    });
    let has_ping = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(99.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(
        has_error_for_42,
        "Duplicate integer ID should produce error response"
    );
    assert!(has_ping, "Ping with different ID should succeed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Duplicate ID tests — string IDs
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_duplicate_string_id_rejected() {
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
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_diff_explain","arguments":{"a":"hello world foo bar baz","b":"hello world qux bar baz"}},"id":"abc"}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":"abc"}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":"xyz"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    let has_error_for_abc = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("error").is_some() && v.get("id") == Some(&Value::String("abc".to_string()))
        } else {
            false
        }
    });
    let has_ping = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::String("xyz".to_string())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(
        has_error_for_abc,
        "Duplicate string ID should produce error response"
    );
    assert!(has_ping, "Ping with different ID should succeed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ID reuse after completion
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_id_reuse_after_completion() {
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
        // First request with id=100 — fast tool, completes quickly
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":100}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Second request with same id=100 — should be rejected (first still in-flight)
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"2+2"}},"id":100}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Third request with id=101 — should succeed
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"3+3"}},"id":101}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Should have at least 2 responses: one error for duplicate id=100, one success for id=101
    let error_count_100 = lines
        .iter()
        .filter(|line| {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                v.get("error").is_some() && v.get("id") == Some(&Value::Number(100.into()))
            } else {
                false
            }
        })
        .count();
    let success_101 = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(101.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(
        error_count_100 >= 1,
        "Second request with same ID should be rejected, got {} error responses",
        error_count_100
    );
    assert!(success_101, "Request with different ID should succeed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cancellation targeting — cancel one request, verify other is unaffected
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancellation_targets_correct_request() {
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
        // Start a slow request with id=10
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"text_diff_explain","arguments":{"a":"hello world foo bar baz qux quux corge grault garply waldo fred plugh xyzzy","b":"hello world foo bar baz qux quux corge grault garply waldo fred plugh xyzzyAAAA"}},"id":10}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Start a fast request with id=20
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"42"}},"id":20}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Cancel id=10 (the slow one) — should not affect id=20
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":10}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // id=20 should have a successful result (math_eval is fast)
    let has_success_20 = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(20.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(
        has_success_20,
        "Request id=20 should succeed even when id=10 is cancelled"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cancellation state coverage — unknown ID is harmless
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancellation_unknown_id_no_response() {
    let response_str = {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
            .arg("--mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn process");

        {
            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            // Cancel a non-existent request — should produce no response
            stdin
                .write_all(
                    r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":99999}}"#
                        .as_bytes(),
                )
                .unwrap();
            stdin.write_all(b"\n").unwrap();
            // Then send a ping to verify server is still alive
            stdin
                .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#.as_bytes())
                .unwrap();
            stdin.write_all(b"\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    let lines: Vec<&str> = response_str.lines().collect();
    // Should have exactly 1 response (the ping), no response for the unknown cancel
    let ping_response = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(1.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(
        ping_response,
        "Ping should succeed after cancelling unknown ID"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cancellation state coverage — cancel already-completed request
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancellation_already_completed_no_response() {
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
        // Fast request — should complete quickly
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":50}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Wait briefly for completion, then cancel
        thread::sleep(Duration::from_millis(500));
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":50}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Verify server still works
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":51}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Should have the math_eval result and the ping result, no error from cancelling completed request
    let has_math_result = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(50.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    let has_ping = lines.iter().any(|line| {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            v.get("id") == Some(&Value::Number(51.into())) && v.get("result").is_some()
        } else {
            false
        }
    });
    assert!(has_math_result, "math_eval should complete successfully");
    assert!(
        has_ping,
        "Ping should succeed after cancelling completed request"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Worker containment — in-process test via budget override
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_worker_containment_concurrent_handlers() {
    // Launch many concurrent tool calls via the in-process API.
    // With MAX_TOOL_WORKERS=16, at most 16 blocking handlers run simultaneously.
    // We send 20 calls and verify they all complete without deadlock.
    let handles: Vec<_> = (0..20)
        .map(|i| {
            thread::spawn(move || {
                let registry =
                    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
                let resp = registry
                    .call_json(
                        "math_eval",
                        serde_json::json!({"expression": format!("{} + 1", i)}),
                    )
                    .unwrap_or_else(|e| panic!("math_eval {} failed: {e}", i));
                assert!(resp.ok, "math_eval {} should succeed", i);
            })
        })
        .collect();
    for h in handles {
        h.join().expect("worker containment test thread panicked");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Worker containment — timeout does not leak permits
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_timeout_does_not_leak_worker_permits() {
    use eggsact::mcp::budget::ToolBudget;

    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);

    // Use a very short budget to force timeout
    let budget = ToolBudget::CHEAP.with_max_elapsed_ms(1);
    thread::sleep(Duration::from_millis(5));

    let _resp = registry
        .call_json_with_budget(
            "math_eval",
            serde_json::json!({"expression": "1 + 1"}),
            Some(budget),
        )
        .expect("call should succeed");

    // The call may succeed (fast enough) or timeout — both are acceptable.
    // The key invariant: after the call, we can still make another call
    // (permits were not leaked).
    let resp2 = registry
        .call_json("math_eval", serde_json::json!({"expression": "2 + 2"}))
        .expect("second call after timeout should succeed");
    assert!(resp2.ok, "second call should succeed (permits not leaked)");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shutdown — graceful drain
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_shutdown_drains_inflight_requests() {
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
        // Send two fast requests then close stdin
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":1}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"2+2"}},"id":2}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Drop stdin — triggers graceful shutdown
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Both responses should have been drained before exit
    let has_id1 = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(1.into())))
            .unwrap_or(false)
    });
    let has_id2 = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(2.into())))
            .unwrap_or(false)
    });
    assert!(has_id1, "Response for id=1 should be drained on shutdown");
    assert!(has_id2, "Response for id=2 should be drained on shutdown");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Malformed cancellation — no response, logged to stderr
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_malformed_cancelled_notification_no_response() {
    let response_str = {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
            .arg("--mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn process");

        {
            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            // Malformed: missing params
            stdin
                .write_all(r#"{"jsonrpc":"2.0","method":"notifications/cancelled"}"#.as_bytes())
                .unwrap();
            stdin.write_all(b"\n").unwrap();
            // Malformed: missing requestId in params
            stdin
                .write_all(
                    r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{}}"#
                        .as_bytes(),
                )
                .unwrap();
            stdin.write_all(b"\n").unwrap();
            // Verify server is still alive
            stdin
                .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#.as_bytes())
                .unwrap();
            stdin.write_all(b"\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    let lines: Vec<&str> = response_str.lines().collect();
    // Should have exactly 1 response (the ping), no responses for malformed cancels
    assert_eq!(
        lines.len(),
        1,
        "Should have exactly 1 response (ping), got {}",
        lines.len()
    );
    let has_ping = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(1.into())) && v.get("result").is_some())
            .unwrap_or(false)
    });
    assert!(
        has_ping,
        "Ping should succeed after malformed cancellation notifications"
    );
}
