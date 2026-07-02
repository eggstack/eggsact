# Final Fix Plan for Tightening Phases 01–05

## Purpose

This plan closes the last two known gaps in the phase 01–05 tightening work. The repository is otherwise in good shape for this line of work: profile/audience dispatch is implemented, compatibility semantics are documented, typed preflight parsing is strict, route-critical tools are classified, generated docs/tool cards are in place, hidden tools are filtered from generated public docs, and the generated profile reference distinguishes model and harness visibility.

The remaining fixes are deliberately small:

1. Add the missing MCP active-profile `tools/call` regression test.
2. Correct the stale-doc generator help text from `generate_docs` to `generate-docs`.

Do not expand phase 06 functionality in this pass.

## Current State

The latest implementation pass addressed the generator/profile-reference items:

- `generate_readme_tools()` filters out `ToolExposure::Hidden` before rendering README tool tables.
- `generate_tool_cards()` defensively filters out hidden tools.
- Generator tests assert hidden tools are excluded from README and tool-card output.
- `generate_profile_reference()` includes model tool counts, harness tool counts, model-visible names, and harness-only names.
- Architecture docs explain MCP active-profile semantics and route-critical tools.

The remaining issue is that the original MCP-specific regression is not yet guarded by a subprocess MCP test. The current tests cover the in-process registry, but they do not prove that the MCP `tools/call` path honors `EGGCALC_MCP_PROFILE` at server startup.

## Fix 1: Add MCP Active-Profile `tools/call` Regression Test

### Goal

Add a test that would fail if MCP `tools/call` regresses to using `ToolRegistry::default()` or any other full-profile registry instead of the active profile resolved from `EGGCALC_MCP_PROFILE`.

### Why This Matters

The original high-priority bug was MCP-specific. `tools/list` could be restricted while `tools/call` used a full-profile default registry. In-process registry tests do not catch this class of bug because they bypass the MCP server path.

The closure test must exercise the binary as a subprocess with environment configuration, not just call `ToolRegistry` directly.

### Target Test File

Use the existing MCP integration test area. Preferred location:

```text
tests/mcp/test_hardening_and_gaps.rs
```

If test organization has shifted, place it in the closest MCP subprocess test module and add it to `tests/mcp/mod.rs` if needed.

### Add a Helper

Add a helper that mirrors the existing MCP request helper but accepts env vars:

```rust
fn mcp_request_with_env(request: &str, envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_eggsact"));
    cmd.arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    for (k, v) in envs {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().expect("Failed to spawn process");
    {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        stdin.write_all(request.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}
```

Also add a multi-request variant if the test needs to initialize and then call a tool in one process. A single `tools/call` request should be enough unless the server requires `initialize` first in current behavior.

### Test Name

```rust
#[test]
fn test_mcp_tools_call_honors_active_profile_env()
```

### Test Strategy

1. Start MCP with:

```rust
[("EGGCALC_MCP_PROFILE", "codegg_core_min")]
```

2. Call a known tool that is available in full/model but not in `codegg_core_min`.

3. Provide valid arguments for that tool so the rejection must be profile-based, not schema-based.

4. Assert the MCP response is a JSON-RPC error with profile-unavailable semantics.

### Choosing a Tool

Use a deterministic tool known to be outside `codegg_core_min`. Avoid selecting an arbitrary first excluded tool unless the test can also generate valid arguments for it.

Recommended pattern:

```rust
let restricted = ToolRegistry::with_profile_and_audience(Profile::CodeggCoreMin, ToolAudience::Model);
let full = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);

let candidates = [
    ("math_eval", json!({"expression": "1+1"})),
    ("text_measure", json!({"text": "hello"})),
    ("json_shape", json!({"text": "{\"a\":1}"})),
    ("regex_safety_check", json!({"pattern": "a+"})),
    ("path_analyze", json!({"path": "src/main.rs"})),
];

let (tool, args) = candidates
    .into_iter()
    .find(|(name, _)| full.has_tool(name) && !restricted.has_tool(name))
    .expect("test requires at least one candidate outside codegg_core_min");
```

If `has_tool()` does not include audience semantics, also confirm the candidate appears in `full.available_tools_model_safe()` and not in `restricted.available_tools_model_safe()`.

### MCP Request

```rust
let request = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {"name": tool, "arguments": args},
    "id": 1
}).to_string();

let response_str = mcp_request_with_env(&request, &[("EGGCALC_MCP_PROFILE", "codegg_core_min")]);
let response: Value = serde_json::from_str(&response_str).expect("valid JSON-RPC response");
```

### Assertions

Assert all of the following:

```rust
assert!(response.get("error").is_some(), "out-of-profile tool should be rejected");
let error = response.get("error").unwrap();
assert_eq!(error.get("code").and_then(|v| v.as_i64()), Some(-32602));
let message = error.get("message").and_then(|v| v.as_str()).unwrap_or("");
assert!(
    message.contains("not available in profile") || message.contains("profile"),
    "error should mention profile unavailability, got: {message}"
);
```

Also assert that the selected candidate actually succeeds under full/profile model registry before the MCP call:

```rust
let full_result = full.call_json(tool, args.clone());
assert!(full_result.is_ok(), "candidate should be valid under full profile");
```

This prevents a false positive caused by invalid test arguments.

### Acceptance Criteria

- The test fails if MCP `tools/call` uses a default full registry while `EGGCALC_MCP_PROFILE=codegg_core_min`.
- The test cannot pass because of schema rejection.
- The test documents active-profile behavior and no per-call profile override.
- Existing in-process profile tests remain in place.

## Fix 2: Correct Generated Docs Command Name in Error Message

### Problem

The generator binary is registered as:

```toml
[[bin]]
name = "generate-docs"
path = "src/bin/generate_docs.rs"
```

CI correctly runs:

```bash
cargo run --bin generate-docs -- --check
```

But the stale-doc error message currently instructs users to run:

```bash
cargo run --bin generate_docs
```

That command name is wrong for Cargo.

### Required Change

In `src/bin/generate_docs.rs`, update the stale-doc message from:

```rust
eprintln!("Run `cargo run --bin generate_docs` to regenerate.");
```

to:

```rust
eprintln!("Run `cargo run --bin generate-docs` to regenerate.");
```

If docs mention the underscore variant anywhere else, update them to `generate-docs` as well.

### Recommended Test

Add a small unit test for command text if practical:

```rust
#[test]
fn stale_docs_message_uses_cargo_bin_name() {
    const REGEN_COMMAND: &str = "cargo run --bin generate-docs";
    assert!(REGEN_COMMAND.contains("generate-docs"));
}
```

A cleaner implementation is to introduce a constant:

```rust
const REGENERATE_COMMAND: &str = "cargo run --bin generate-docs";
```

Use it in the error message and assert it in a test:

```rust
assert_eq!(REGENERATE_COMMAND, "cargo run --bin generate-docs");
```

### Acceptance Criteria

- The generator stale-doc message uses `generate-docs`.
- CI command, docs, and error message all agree.

## Verification Commands

Run the full gate after both fixes:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If a local environment cannot run all commands, the implementation handoff must state which commands were run and which were not.

## Final Acceptance Criteria

This final fix plan is complete when:

- A subprocess MCP test proves `tools/call` honors `EGGCALC_MCP_PROFILE=codegg_core_min`.
- The selected out-of-profile tool has valid full-profile arguments and is rejected specifically for profile unavailability under MCP.
- The generator stale-doc message uses `cargo run --bin generate-docs`.
- Full test/CI verification passes or failures are documented with exact output.

## Non-Goals

Do not add phase 06 features.

Do not change profile semantics.

Do not add per-call profile override to MCP `tools/call`.

Do not require MCP to expose harness-only execution. Harness-oriented execution should remain in-process unless a separate capability model is designed later.
