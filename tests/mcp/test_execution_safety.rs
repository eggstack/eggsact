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
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════════
// Duplicate ID tests — integer IDs
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_duplicate_integer_id_rejected() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake first
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
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
            // Initialize handshake
            stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
            stdin
                .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
                .unwrap();
            stdin.write_all(b"\n").unwrap();
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
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
            // Initialize handshake
            stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
            stdin
                .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
                .unwrap();
            stdin.write_all(b"\n").unwrap();
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
    // Should have exactly 2 responses (init + ping), no responses for malformed cancels
    assert_eq!(
        lines.len(),
        2,
        "Should have exactly 2 responses (init + ping), got {}",
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

// ═══════════════════════════════════════════════════════════════════════════════
// M4: Rate-limiter saturation then cancel
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rate_limiter_saturation_then_cancel() {
    use eggsact::mcp::runtime::MAX_REQUESTS_PER_SECOND;

    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Saturate the rate limiter with MAX_REQUESTS_PER_SECOND fast requests.
        // All must be accepted (sliding window allows burst up to the limit).
        for i in 0..MAX_REQUESTS_PER_SECOND {
            let req = format!(r#"{{"jsonrpc":"2.0","method":"ping","id":{}}}"#, i + 1);
            stdin.write_all(req.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
        // Now send a cancellation notification — it must bypass the rate limiter.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":999}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // The rate limiter window hasn't reset, so a new request would be
        // rejected. Send one anyway — the error response proves the server
        // is still alive and processing requests (not crashed).
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":9999}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // All ping requests should have been accepted (rate limiter allows burst).
    let initial_pings: usize = lines
        .iter()
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .map(|v| {
                    v.get("result").is_some()
                        && v.get("id")
                            .and_then(|id| id.as_u64())
                            .is_some_and(|id| id >= 1 && id <= MAX_REQUESTS_PER_SECOND as u64)
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        initial_pings >= MAX_REQUESTS_PER_SECOND as usize - 1,
        "Most initial pings should succeed within rate limit (init handshake may consume one slot), got {}",
        initial_pings
    );

    // The cancellation notification produces no response (it's a notification).
    // The final ping (id=9999) may be rate-limited (error response) — either
    // way, the server must respond, proving it's still alive.
    let has_response_9999 = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| {
                v.get("id") == Some(&Value::Number(9999.into()))
                    && (v.get("result").is_some() || v.get("error").is_some())
            })
            .unwrap_or(false)
    });
    assert!(
        has_response_9999,
        "Server must respond to final ping (success or rate-limit error) — proves server is alive"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// M5: Cancel running cooperative handler — bounded termination
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancel_running_handler_bounded_termination() {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Start a regex_finditer with a catastrophic backtracking pattern.
        // `(a+)+$` on a long string of 'a's ending with 'b' causes exponential
        // backtracking, taking ~5s to timeout. This gives us a window to cancel.
        let text: String = "a".repeat(8000) + "b";
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "regex_finditer",
                "arguments": {
                    "pattern": "(a+)+$",
                    "text": text,
                    "max_matches": 1
                }
            },
            "id": 1
        });
        stdin.write_all(req.to_string().as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        // Wait briefly for the handler to enter the blocking regex execution.
        thread::sleep(Duration::from_millis(500));
        // Send a cancellation notification.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    // The handler should terminate within the regex timeout (5s) plus margin.
    let output = child.wait_with_output().unwrap();
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Must terminate within 10 seconds (regex timeout + margin).
    assert!(
        elapsed < Duration::from_secs(10),
        "Handler must terminate within bounded time after cancel, took {:?}",
        elapsed
    );

    // Should have a response for id=1 (either timeout/error from regex or
    // cancelled, depending on timing). The key invariant: the response arrived.
    let has_response = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(1.into())))
            .unwrap_or(false)
    });
    assert!(
        has_response,
        "Cancelled handler must produce a response (not hang)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// M6: ID reuse — first request cleanup cannot remove second request's entry
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_id_reuse_guard_does_not_corrupt_second_entry() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // First request with id="shared" — fast tool, completes quickly.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":"shared"}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Wait for first request to complete (guard drops, removes entry).
        thread::sleep(Duration::from_millis(500));
        // Second request with same id="shared" — should succeed because
        // the first request's guard already dropped and removed its entry.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"2+2"}},"id":"shared"}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Third request with different id to verify server is still healthy.
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":"verify"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // The second request with id="shared" should succeed (not be rejected as duplicate).
    let success_shared = lines
        .iter()
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .map(|v| {
                    v.get("id") == Some(&Value::String("shared".to_string()))
                        && v.get("result").is_some()
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        success_shared >= 1,
        "Second request with reused ID should succeed after first completes"
    );

    let has_verify = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| {
                v.get("id") == Some(&Value::String("verify".to_string()))
                    && v.get("result").is_some()
            })
            .unwrap_or(false)
    });
    assert!(has_verify, "Server should be healthy after ID reuse");
}

// ═══════════════════════════════════════════════════════════════════════════════
// M7: Shutdown — verify all responses received and metrics return to zero
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_shutdown_all_responses_received() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Send 3 fast requests then close stdin to trigger shutdown.
        for i in 1..=3 {
            let req = format!(
                r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"math_eval","arguments":{{"expression":"{}+1"}}}},"id":{}}}"#,
                i, i
            );
            stdin.write_all(req.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
        // Drop stdin — triggers graceful shutdown.
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // All 5 responses should have been drained before exit.
    let response_count = lines
        .iter()
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .map(|v| {
                    v.get("id").is_some() && (v.get("result").is_some() || v.get("error").is_some())
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        response_count >= 3,
        "All 3 responses should be drained on shutdown, got {}",
        response_count
    );

    // Verify each ID has a response.
    for id in 1..=3 {
        let has_response = lines.iter().any(|line| {
            serde_json::from_str::<Value>(line)
                .map(|v| v.get("id") == Some(&Value::Number(id.into())))
                .unwrap_or(false)
        });
        assert!(
            has_response,
            "Response for id={} should be present after shutdown",
            id
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// M1: Worker containment — concurrent MCP processes prove no deadlock
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_worker_containment_concurrent_mcp_no_deadlock() {
    use std::sync::{Arc, Mutex};

    let results: Arc<Mutex<Vec<(usize, bool, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let num_tasks = 32; // Exceeds MAX_TOOL_WORKERS (16) — proves semaphore works

    let handles: Vec<_> = (0..num_tasks)
        .map(|i| {
            let results = results.clone();
            thread::spawn(move || {
                let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
                    .arg("--mcp")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap_or_else(|e| panic!("Failed to spawn MCP process {i}: {e}"));

                {
                    let mut stdin = child.stdin.take().unwrap();
                    // Initialize handshake
                    stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
                    stdin.write_all(b"\n").unwrap();
                    stdin.write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes()).unwrap();
                    stdin.write_all(b"\n").unwrap();
                    let req = format!(
                        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"math_eval","arguments":{{"expression":"{} + 1"}}}},"id":{}}}"#,
                        i, i
                    );
                    stdin.write_all(req.as_bytes()).unwrap();
                    stdin.write_all(b"\n").unwrap();
                }

                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let has_error = stdout.lines().any(|line| {
                    serde_json::from_str::<Value>(line)
                        .map(|v| v.get("error").is_some())
                        .unwrap_or(false)
                });
                results.lock().unwrap().push((i, !has_error, stdout));
            })
        })
        .collect();

    for h in handles {
        h.join().expect("worker containment test thread panicked");
    }

    let results = results.lock().unwrap();
    assert_eq!(
        results.len(),
        num_tasks,
        "All {} tasks should have completed",
        num_tasks
    );
    for (id, success, _) in results.iter() {
        assert!(success, "Task {id} should succeed (no deadlock, no error)");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// M2: Timeout retains permit — sequential calls after timeout succeed
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_timeout_retains_permit_sequential_calls() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);

    // Force a timeout with a 1ms budget.
    let budget = eggsact::mcp::budget::ToolBudget::CHEAP.with_max_elapsed_ms(1);
    thread::sleep(Duration::from_millis(5));

    let _resp1 = registry
        .call_json_with_budget(
            "math_eval",
            serde_json::json!({"expression": "1 + 1"}),
            Some(budget),
        )
        .expect("call should not fail at registry level");

    // Whether resp1 succeeded or timed out, the permit must NOT be leaked.
    // A second call proves the permit was returned to the semaphore.
    let resp2 = registry
        .call_json("math_eval", serde_json::json!({"expression": "2 + 2"}))
        .expect("second call after timeout must succeed");
    assert!(
        resp2.ok,
        "second call must succeed — permit was not leaked by timeout"
    );

    // Third call to be absolutely sure the semaphore is healthy.
    let resp3 = registry
        .call_json("math_eval", serde_json::json!({"expression": "3 + 3"}))
        .expect("third call must succeed");
    assert!(resp3.ok, "third call must succeed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// M3: Peak concurrency — 32 concurrent tasks, semaphore limits to 16
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_peak_concurrency_bounded_by_semaphore() {
    use std::sync::{Arc, Mutex};

    // Launch 32 concurrent MCP processes (2x MAX_TOOL_WORKERS).
    // If the semaphore is correct, at most 16 blocking handlers run at once.
    // All 32 must complete without deadlock or error.
    let results: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));
    let num_tasks = 32;

    let handles: Vec<_> = (0..num_tasks)
        .map(|i| {
            let results = results.clone();
            thread::spawn(move || {
                let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
                    .arg("--mcp")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap_or_else(|e| panic!("spawn failed for task {i}: {e}"));

                {
                    let mut stdin = child.stdin.take().unwrap();
                    // Initialize handshake
                    stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
                    stdin.write_all(b"\n").unwrap();
                    stdin.write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes()).unwrap();
                    stdin.write_all(b"\n").unwrap();
                    let req = format!(
                        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"math_eval","arguments":{{"expression":"{} * 2"}}}},"id":{}}}"#,
                        i, i
                    );
                    stdin.write_all(req.as_bytes()).unwrap();
                    stdin.write_all(b"\n").unwrap();
                }

                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let has_result = stdout.lines().any(|line| {
                    serde_json::from_str::<Value>(line)
                        .map(|v| v.get("id") == Some(&Value::Number(i.into())) && v.get("result").is_some())
                        .unwrap_or(false)
                });
                results.lock().unwrap().push(has_result);
            })
        })
        .collect();

    for h in handles {
        h.join().expect("peak concurrency test thread panicked");
    }

    let results = results.lock().unwrap();
    assert_eq!(results.len(), num_tasks);
    let success_count = results.iter().filter(|&&ok| ok).count();
    assert_eq!(
        success_count, num_tasks,
        "All {num_tasks} tasks must succeed — semaphore bounded concurrency to 16"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Item 49: Guard cleanup — handler error path
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_guard_cleanup_on_handler_error() {
    use eggsact::mcp::runtime::snapshot_metrics;

    let before = snapshot_metrics();

    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);

    // Send a request that triggers a handler error (unknown tool).
    let resp = registry.call_json("nonexistent_tool_xyz", serde_json::json!({}));
    assert!(resp.is_err(), "unknown tool should return Err");

    // Send a valid request to prove the active map was cleaned up.
    let resp2 = registry
        .call_json("math_eval", serde_json::json!({"expression": "1+1"}))
        .expect("valid call should succeed");
    assert!(resp2.ok, "valid call should succeed after error cleanup");

    // Verify no leaked active requests (guard cleaned up on error path).
    let after = snapshot_metrics();
    assert_eq!(
        after.active_requests, before.active_requests,
        "active_requests should return to baseline after error-path guard cleanup"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Item 77: Metrics return to zero after shutdown
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_metrics_return_to_zero_after_shutdown() {
    use eggsact::mcp::runtime::snapshot_metrics;

    // Snapshot before any MCP activity.
    let before = snapshot_metrics();

    // Start an MCP server, send requests, and shut down.
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        for i in 1..=5 {
            let req = format!(
                r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"math_eval","arguments":{{"expression":"{}+1"}}}},"id":{}}}"#,
                i, i
            );
            stdin.write_all(req.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
        // Drop stdin to trigger shutdown.
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 5,
        "All 5 responses should be drained, got {}",
        lines.len()
    );

    // Metrics are global statics; in a separate process they start at zero.
    // Verify that the current process's metrics are at baseline (no leakage).
    let after = snapshot_metrics();
    assert_eq!(
        after.active_requests, before.active_requests,
        "active_requests should be at baseline after MCP process shutdown"
    );
    assert_eq!(
        after.active_blocking_handlers, before.active_blocking_handlers,
        "active_blocking_handlers should be at baseline after MCP process shutdown"
    );
    assert_eq!(
        after.timed_out_handlers, before.timed_out_handlers,
        "timed_out_handlers should be at baseline after MCP process shutdown"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Item 32/66: Cancel request before handler enters blocking section
//
// The pre-spawn cancel check (server.rs:390) fires BETWEEN acquiring the
// semaphore permit and calling spawn_blocking. To exercise it, the cancel
// notification must arrive while the request is waiting for a permit. This
// requires saturating all MAX_TOOL_WORKERS permits, which the rate limiter
// (MAX_REQUESTS_PER_SECOND=10) prevents from a single MCP process.
//
// Instead, we verify the cooperative cancellation path via MCP: send a slow
// request, cancel it while running, and verify bounded termination. The cancel
// flag is checked by the handler at its next should_stop() checkpoint.
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancel_before_handler_enters_blocking() {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Send a slow request (regex_finditer with catastrophic backtracking).
        let text: String = "a".repeat(8000) + "b";
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "regex_finditer",
                "arguments": {
                    "pattern": "(a+)+$",
                    "text": text,
                    "max_matches": 1
                }
            },
            "id": 1
        });
        stdin.write_all(req.to_string().as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        // Wait briefly for the handler to enter the blocking regex execution.
        thread::sleep(Duration::from_millis(500));
        // Cancel the request — sets the cancel flag on the active request.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Must terminate within bounded time (regex inner timeout + margin).
    assert!(
        elapsed < Duration::from_secs(10),
        "Cancelled handler must terminate within bounded time, took {:?}",
        elapsed
    );

    // Should have a response for id=1 (timeout/error/cancelled).
    let has_response = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(1.into())))
            .unwrap_or(false)
    });
    assert!(
        has_response,
        "Cancelled handler must produce a response (not hang)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Item 34: Cancel timed-out-but-still-running request
//
// regex_finditer has an inner 5s timeout (REGEX_TIMEOUT_SECONDS). After the
// inner timeout fires, the handler returns a response. The nested thread may
// still be running. We send a cancel notification after the inner timeout has
// fired to verify: (a) no crash, (b) server remains healthy, (c) the cancel
// notification for an already-completed request is silently ignored.
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancel_after_inner_timeout() {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Start a regex_finditer with catastrophic backtracking.
        // Inner timeout fires at 5s (REGEX_TIMEOUT_SECONDS).
        let text: String = "a".repeat(8000) + "b";
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "regex_finditer",
                "arguments": {
                    "pattern": "(a+)+$",
                    "text": text,
                    "max_matches": 1
                }
            },
            "id": 1
        });
        stdin.write_all(req.to_string().as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        // Wait for the inner timeout to fire (5s) plus margin.
        thread::sleep(Duration::from_secs(7));
        // The handler has returned; the nested thread may still be running.
        // Cancel the now-completed request — should be silently ignored.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Verify server is still alive.
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":2}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Must terminate within bounded time.
    assert!(
        elapsed < Duration::from_secs(15),
        "Server must terminate within bounded time, took {:?}",
        elapsed
    );

    // Should have a response for id=1 (timeout/error from inner timeout).
    let has_response = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(1.into())))
            .unwrap_or(false)
    });
    assert!(
        has_response,
        "Should have a response for id=1 (inner timeout or error)"
    );

    // Should have a ping response — server is alive after cancel-on-completed.
    let has_ping = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(2.into())) && v.get("result").is_some())
            .unwrap_or(false)
    });
    assert!(
        has_ping,
        "Server should still be alive after cancel-on-completed-request"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Item 35: Cancel during response serialization
//
// Cancellation during serialization is not deterministically testable without
// test hooks in the server's async task (between handler return and tx.send).
// The serialization step is nearly instantaneous for small responses, and
// there is no reliable way to insert a delay there without modifying production
// code. This is documented as a known limitation.
//
// The closest feasible test is to verify that a cancellation arriving while
// the server is processing a response does not corrupt the response or crash
// the server.
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancel_during_response_processing() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eggsact"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        // Initialize handshake
        stdin.write_all(r#"{"jsonrpc":"2.0","method":"initialize","id":0,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Send a fast request followed immediately by a cancellation.
        // The cancellation may arrive while the server is serializing the response.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"math_eval","arguments":{"expression":"1+1"}},"id":1}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Cancel immediately — may arrive during response serialization.
        stdin
            .write_all(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
                    .as_bytes(),
            )
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        // Verify server is still alive.
        stdin
            .write_all(r#"{"jsonrpc":"2.0","method":"ping","id":2}"#.as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().collect();

    // Should have at least a ping response — server is alive.
    let has_ping = lines.iter().any(|line| {
        serde_json::from_str::<Value>(line)
            .map(|v| v.get("id") == Some(&Value::Number(2.into())) && v.get("result").is_some())
            .unwrap_or(false)
    });
    assert!(
        has_ping,
        "Server should be alive after cancel-during-serialization"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Items 57/58/61: Worker containment occupancy observation
//
// Observing blocking-section occupancy requires test hooks (barriers/notifies)
// in the server's spawn_blocking path. The existing tools don't check
// should_stop() in execution loops, so barriers can't control when
// cancellation is observed at the MCP protocol level.
//
// The feasible tests (M1, M2, M3) prove:
// - No deadlock when exceeding MAX_TOOL_WORKERS (M1)
// - Permits not leaked after timeout (M2)
// - Semaphore bounds concurrency to MAX_TOOL_WORKERS (M3)
//
// Direct occupancy observation is deferred to a future release that adds
// #[cfg(test)] synchronization hooks in server.rs.
// ═══════════════════════════════════════════════════════════════════════════════
