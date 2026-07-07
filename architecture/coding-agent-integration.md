# Coding-Agent Integration Guide

How to integrate eggsact into coding-agent harnesses, codegg-style workflows, and MCP-based tool pipelines.

## Transport Options

### MCP Stdio (Recommended for External Agents)

```bash
eggsact --mcp
```

Starts a JSON-RPC 2.0 server over stdin/stdout. The agent sends `tools/list` and `tools/call` requests; eggsact responds with JSON-RPC results. This is the standard MCP transport and works with any MCP-compatible harness.

Key properties:
- Transport: stdio (stdin/stdout)
- Protocol: JSON-RPC 2.0
- MCP version: `2024-11-05`
- Responses may arrive out of request order (concurrent dispatch via `JoinSet`). **Clients must correlate responses by JSON-RPC `id`**, not by arrival position.

### In-Process API (Recommended for Rust Hosts)

```rust
use eggsact::agent::{ToolRegistry, Profile};

let registry = ToolRegistry::default();
let response = registry.call_json("text_equal", serde_json::json!({
    "a": "hello",
    "b": "hello",
})).unwrap();
assert!(response.ok);
```

No subprocess, no JSON serialization overhead. The `ToolRegistry` resolves the profile and audience at construction time and validates tool calls against the active profile.

For per-request state isolation (reproducible PRNG, user variables, memory registers):

```rust
use eggsact::agent::{ToolRegistry, ExecutionContext};
use eggsact::calc::EvalContext;

let ctx = ExecutionContext::default();
let registry = ToolRegistry::with_execution_context(ctx);
let response = registry.call_json_with_execution_context(
    "math_eval",
    serde_json::json!({ "expression": "pi ** 2" }),
).unwrap();
```

## Audience Model

Tools have typed `ToolExposure` and `ToolListAudience` enums:

| Audience | Excludes | Use Case |
|----------|----------|----------|
| `Model` | HarnessOnly, Hidden | Ordinary coder-agent sessions. Only tools safe for direct model consumption. |
| `Harness` | Hidden | Harness-driven safety checks. Includes preflight tools that models should not call directly. |
| `Debug` | (none non-hidden) | Full tool listing for development and debugging. |

**Use `ToolAudience::Model` for model-facing integrations.** Harness-only tools (preflight, config inspection, etc.) should not be presented to the model in ordinary sessions. Use `ToolAudience::Harness` when the harness itself drives safety checks programmatically.

### Available-Tools APIs

```rust
// Model-safe tools only (recommended for model-facing lists)
let tools = registry.available_tools_model_safe();

// Audience-specific filtering
let tools = registry.available_tools_for_audience(ToolAudience::Harness);

// Current audience (set at construction time)
let tools = registry.available_tools_for_current_audience();
```

## Profile Selection

Profiles control which tools are registered in the `ToolRegistry`. Each profile is a named subset of the 71 available tools. The active profile is set once at server startup via `EGGCALC_MCP_PROFILE` (MCP) or at `ToolRegistry` construction (in-process).

### Recommended Profiles by Workflow

| Profile | Tool Count | Audience | Use Case |
|---------|-----------|----------|----------|
| `codegg_core_min` | minimal | Model | Minimal model-visible tools for constrained sessions |
| `codegg_core` | moderate | Model | Normal coding sessions with text, math, path, JSON tools |
| `codegg_preflight` | broad | Harness | Harness-driven edit/command/config preflight checks |
| `codegg_patch` | focused | Model | Edit and patch workflows (edit_preflight, patch_apply_check) |
| `codegg_config` | focused | Model | Config validation (config_preflight, dotenv_validate, ini_validate) |
| `codegg_unicode_security` | focused | Model | Suspicious text/identifier review (unicode_policy_check, text_security_inspect) |
| `codegg_shell` | focused | Model | Command planning and preflight (command_preflight, shell_split, argv_compare) |
| `codegg_repo_audit` | focused | Model | Repository inspection (repo_manifest_inspect, config_file_inspect) |

### Profile Construction (In-Process)

```rust
use eggsact::agent::{ToolRegistry, Profile};
use eggsact::mcp::registry::ToolAudience;

// From a named profile
let profile = Profile::from_str_opt("codegg_core").unwrap();
let registry = ToolRegistry::with_profile_and_audience(profile, ToolAudience::Model);

// Custom profile (explicit tool list)
let profile = Profile::custom("my_custom");
let registry = ToolRegistry::with_profile_and_audience(profile, ToolAudience::Harness);
```

`Profile::from_str_opt` is strict — returns `None` for unknown names. Use `Profile::custom(name)` for ad-hoc profiles.

## Route-Critical Tools

These tools must always emit `machine_code` and `verdict` in their response envelope:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `text_security_inspect`

Harnesses should verify that `machine_code` and `verdict` are present in the response for these tools. Missing fields indicate a contract violation.

## Concurrency Contract

The MCP server dispatches requests concurrently via `JoinSet`. Responses may arrive in completion order, not request order. **JSON-RPC clients must correlate responses by `id`**, not by arrival position.

Notifications (no `id`) produce no response by JSON-RPC contract.

## Compatibility Mode

| Mode | Default For | Behavior |
|------|-------------|----------|
| `EggcalcPython` | MCP server | Python-parity error messages. Preserves backward compatibility with `eggcalc` clients. |
| `StrictNative` | In-process API | Standard JSON Schema error messages. Stricter validation. |

The MCP server uses `EggcalcPython` by default to maintain parity with the Python reference. The in-process API uses `StrictNative`. Override via `ExecutionContext::compat_mode()` if needed.

## Input Limits

| Limit | Value |
|-------|-------|
| Max text length | 100 KB |
| Max expression length | 10 KB |
| Max list items | 10,000 |
| Max regex samples | 100 |
| Max pattern length | 1 KB |
| Max request bytes | 1 MB |
| Max output bytes | 1 MB |

Input is checked against `budget.max_input_bytes` before dispatch. Oversized input fails with `INPUT_TOO_LARGE` (high, blocking) instead of wasting compute.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `EGGCALC_MCP_PROFILE` | Set active profile at MCP server startup |
| `EGGCALC_MCP_AUDIENCE` | Set audience for MCP tool listings (default: `Model`) |
| `EGGCALC_MCP_SCHEMA_DETAIL` | Control schema detail level in tool listings |
| `EGGCALC_NO_CONFIG=1` | Disable config file loading (set automatically by `main.rs`) |
