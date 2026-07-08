# Coding-Agent Integration Guide

How to integrate eggsact into coding-agent harnesses, codegg-style workflows, and MCP-based tool pipelines.

## Overview

Eggsact provides 80 tools across 20 categories (math, text, JSON, regex, path, shell, config, patch, dependency, analysis, and more). It integrates with coding agents through two transport mechanisms:

- **MCP stdio** — a JSON-RPC 2.0 server over stdin/stdout, compatible with any MCP client
- **In-process API** — a synchronous Rust `ToolRegistry` that avoids subprocess and serialization overhead

Both paths share the same tool registry, profile filtering, audience enforcement, and budget system. The in-process API additionally supports `ExecutionContext` for per-request state isolation (reproducible PRNG, user variables, calculator memory registers).

---

## Transport Options

### MCP Stdio (External Agents)

```bash
eggsact --mcp
```

Starts a JSON-RPC 2.0 server over stdin/stdout. The agent sends `tools/list` and `tools/call` requests; eggsact responds with JSON-RPC results.

| Property | Value |
|----------|-------|
| Transport | stdio (stdin/stdout) |
| Protocol | JSON-RPC 2.0 |
| MCP version | `2024-11-05` |
| Max in-flight requests | 32 |
| Rate limit | 10 req/s |
| Tool concurrency | 16 workers |

**Key caveats:**

- **Out-of-order responses** — the server dispatches requests concurrently via `JoinSet`. Responses may arrive in completion order, not request order. **Clients must correlate by JSON-RPC `id`**, not arrival position.
- **Notifications produce no response** — `notifications/initialized` and `notifications/cancelled` are fire-and-forget. The cancel notification sets a cooperative flag on the targeted request.
- **Batch requests rejected** — the server returns an error for JSON arrays at the top level.
- **Timeout is cooperative** — on budget timeout the cancel flag is set but the handler may continue briefly. The response includes `TIMEOUT` machine code.

### In-Process API (Rust Hosts)

```rust
use eggsact::agent::{ToolRegistry, Profile};

let registry = ToolRegistry::default();
let response = registry.call_json("text_equal", serde_json::json!({
    "a": "hello",
    "b": "hello",
})).unwrap();
assert!(response.ok);
```

No subprocess, no JSON serialization overhead. `ToolRegistry` resolves profile and audience at construction time and validates tool calls against the active profile.

**API variants:**

| Method | Budget | Cancellation | Use Case |
|--------|--------|-------------|----------|
| `call_json(name, args)` | default | no | Simple calls without resource limits |
| `call_json_with_budget(name, args, budget)` | explicit | no | Calls requiring deterministic resource discipline |
| `call_json_with_context(name, args, budget, cancel_flag)` | explicit | yes | Calls needing cooperative cancellation |
| `call_json_with_execution_context(name, args, ctx)` | from ctx | from ctx | Full per-request state isolation |
| `call_json_value(name, args)` | default | no | Convenience — returns `result` Value or `null` |

### In-Process with ExecutionContext (Per-Request State Isolation)

For per-call profile/audience/compat/budget/cancellation overrides:

```rust
use eggsact::agent::{ToolRegistry, ExecutionContext, Profile, ToolAudience};
use eggsact::mcp::compat::CompatibilityMode;

let ctx = ExecutionContext::builder()
    .profile(Profile::CodeggCore)
    .audience(ToolAudience::Model)
    .compatibility_mode(CompatibilityMode::StrictNative)
    .build();

let registry = ToolRegistry::new();
let response = registry.call_json_with_execution_context(
    "math_eval",
    serde_json::json!({ "expression": "pi ** 2" }),
    &ctx,
).unwrap();
```

Or using the convenience factory:

```rust
let ctx = ExecutionContext::agent_default(Profile::CodeggCore, ToolAudience::Model);
let response = registry.call_json_with_execution_context(
    "text_equal",
    serde_json::json!({ "a": "foo", "b": "foo" }),
    &ctx,
).unwrap();
```

**Eval-context handling:** `ctx.eval_ctx` is **cloned** at dispatch. PRNG draws, memory mutations, and variable assignments inside `math_eval` are confined to the clone and do **not** persist back to the caller's `ExecutionContext`. Two calls with identical seeds produce the same first random value.

> **MCP compatibility:** `call_json_with_execution_context` is an in-process API. It does not change the MCP JSON-RPC wire protocol. The MCP server resolves its active profile from `EGGCALC_MCP_PROFILE` at startup.

---

## Audience Model

Tools have typed `ToolExposure` levels (`Default`, `Contextual`, `ExpertOnly`, `HarnessOnly`, `Hidden`) and audiences control which exposure levels are visible and executable.

| Audience | Includes | Excludes | Use Case |
|----------|----------|----------|----------|
| `Model` | Default, Contextual, ExpertOnly | HarnessOnly, Hidden | Ordinary coder-agent sessions. Only tools safe for direct model consumption. |
| `Harness` | Default, Contextual, ExpertOnly, HarnessOnly | Hidden | Harness-driven safety checks. Includes preflight tools that models should not call directly. |
| `Debug` | Default, Contextual, ExpertOnly, HarnessOnly | Hidden | Full tool listing for development and debugging. |

**Audience enforcement at dispatch:** `ToolRegistry::prepare_tool_call` checks `audience.can_execute_exposure(spec.exposure)` before executing any tool. A Model audience calling a HarnessOnly tool returns `ToolNotAllowedForAudience` error.

### Available-Tools APIs

```rust
// Model-safe tools only (recommended for model-facing lists)
let tools = registry.available_tools_model_safe();

// Audience-specific filtering
let tools = registry.available_tools_for_audience(ToolAudience::Harness);

// Current audience (set at construction time)
let tools = registry.available_tools_for_current_audience();

// Legacy (deprecated since 0.3.0) — only filters Hidden, not model-safe
#[allow(deprecated)]
let tools = registry.available_tools();
```

`ToolView` provides read-only metadata per tool:

```rust
pub struct ToolView {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tier: u8,
    pub profiles: Vec<String>,
    pub tags: Vec<String>,
    pub exposure: String,
    pub cost: String,
    pub stability: String,
    pub composite: bool,
}
```

---

## Profile Selection

Profiles control which subset of tools is available. The active profile is set once at server startup via `EGGCALC_MCP_PROFILE` (MCP) or at `ToolRegistry` construction (in-process).

| Profile | Tools | Audience | Use Case |
|---------|-------|----------|----------|
| `full` | 80 | Model | All non-hidden tools |
| `default` | varies | Model | Default tool set |
| `codegg_core_min` | 6 | Model | Minimal model-visible tools for constrained sessions |
| `codegg_core` | 19 | Model | Normal coding sessions with text, math, path, JSON, analysis |
| `codegg_preflight` | 13 | Harness | Harness-driven edit/command/config/patch/dependency preflight |
| `codegg_patch` | 12 | Model | Edit, patch, symbol-diff, and patch-risk workflows |
| `codegg_config` | 14 | Model | Config validation (JSON, TOML, dotenv, INI, Cargo.toml, structured) |
| `codegg_unicode_security` | 8 | Model | Suspicious text/identifier review (unicode, confusables, invisible chars) |
| `codegg_shell` | 6 | Model | Command planning, shell parsing, and preflight checks |
| `codegg_repo_audit` | 18 | Model | Repository inspection (manifest, config, lockfile, language, structure) |
| `human_math` | 4 | Model | Calculator-only: math_eval, unit_convert, unit_info, constant_lookup |

### Profile Construction (In-Process)

```rust
use eggsact::agent::{ToolRegistry, Profile, ToolAudience};

// From a named profile
let profile = Profile::from_str_opt("codegg_core").unwrap();
let registry = ToolRegistry::with_profile_and_audience(profile, ToolAudience::Model);

// Custom profile (ad-hoc name)
let profile = Profile::custom("my_custom");
let registry = ToolRegistry::with_profile_and_audience(profile, ToolAudience::Harness);
```

`Profile::from_str_opt` is strict — returns `None` for unknown names. Use `Profile::custom(name)` to construct an explicit custom profile. `Profile::custom("full")` creates a `Custom("full")` variant, which is **not** equivalent to `Profile::Full` — the registry distinguishes them.

---

## Route-Critical Tools

These tools must always emit `machine_code` and `verdict` in their response envelope:

| Tool | Purpose |
|------|---------|
| `edit_preflight` | Edit/range analysis, file scope, fingerprinting |
| `command_preflight` | Command safety analysis, platform checks |
| `config_preflight` | Config file validation (JSON, TOML, dotenv, INI) |
| `patch_apply_check` | Patch application safety, diff risk |
| `text_security_inspect` | Unicode security, invisible characters, confusables |

**Harness contract:** Harnesses must verify that `machine_code` and `verdict` are present in the response for these tools. Missing fields indicate a contract violation. The typed preflight wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`) enforce this with `ContractViolation` errors on missing mandatory fields.

Fixture-backed tests in `tests/mcp/test_route_contracts.rs` (`RouteFixture` struct, `all_fixtures()`) verify these contracts with table-driven assertions covering happy paths, error paths, finding subsets, and MCP stdio wire coverage.

---

## Concurrency Contract

The MCP server dispatches requests concurrently via `JoinSet`. Responses may arrive in completion order, not request order.

**JSON-RPC clients must correlate responses by `id`**, not by arrival position.

Notifications (no `id`) produce no response by JSON-RPC contract.

Key concurrency constants:

| Constant | Value |
|----------|-------|
| `MAX_IN_FLIGHT_REQUESTS` | 32 |
| `MAX_REQUESTS_PER_SECOND` | 10 |
| `MAX_TOOL_WORKERS` | 16 |

Test helpers that send multiple requests in one session must correlate by id. The canonical helper `mcp_request_multi()` in `tests/mcp/test_comprehensive_parity.rs` does this and reorders responses to match request slice order.

---

## Compatibility Mode

| Mode | Default For | Behavior |
|------|-------------|----------|
| `EggcalcPython` | MCP server | Python-parity error messages (`NoneType`, `int`, `float`, `str`, `list`, `dict`). Preserves backward compatibility with `eggcalc` clients. |
| `StrictNative` | In-process API | Standard JSON Schema type names (`null`, `integer`, `number`, `string`, `array`, `object`). Stricter validation. |

Both modes reject JSON booleans for numeric schema fields (`integer`/`number`). Only selected error-message wording and a few validation behaviors differ.

Override via `ExecutionContext`:

```rust
let ctx = ExecutionContext::builder()
    .compatibility_mode(CompatibilityMode::EggcalcPython)
    .build();
```

Or on the registry:

```rust
let registry = ToolRegistry::new()
    .with_compat_mode(CompatibilityMode::EggcalcPython);
```

---

## Input Limits

| Limit | Constant | Value |
|-------|----------|-------|
| Max text length | `MAX_TEXT_LENGTH` | 100 KB |
| Max expression length | `MAX_EXPRESSION_LENGTH` | 10 KB |
| Max list items | `MAX_LIST_ITEMS` | 10,000 |
| Max regex samples | `MAX_REGEX_SAMPLES` | 100 |
| Max pattern length | `MAX_PATTERN_LENGTH` | 1 KB |
| Max request bytes | `MAX_REQUEST_BYTES` | 1 MB |
| Max output bytes | `MAX_OUTPUT_BYTES` | 1 MB |

Input is checked against `budget.max_input_bytes` **before** dispatch. Oversized input fails with `INPUT_TOO_LARGE` (high, blocking) instead of wasting compute.

Response truncation is automatic: `truncate_response()` caps findings and output when a tool exceeds its budget limits. Check `limits_applied` in the response envelope to detect truncation. Findings cap reserves one slot for a synthetic `OUTPUT_TOO_LARGE` notice.

### Budget Tiers

| Tier | Max Elapsed | Max Output | Use Case |
|------|-------------|------------|----------|
| `CHEAP` | 10 s | 1 MB | Fast tools: unit conversion, text compare, validation |
| `MODERATE` | 30 s | 1 MB | Heavier text/regex work, config validation |
| `HEAVY` | 30 s | 2 MB | Composite tools that spawn sub-tools (edit_preflight, etc.) |

---

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `EGGCALC_MCP_PROFILE` | `full` | Set active profile at MCP server startup. Must be a known profile name. |
| `EGGCALC_MCP_AUDIENCE` | `Model` | Set audience for MCP tool listings and dispatch. Case-insensitive: `Model`, `Harness`, `Debug`. Invalid values default to `Model`. |
| `EGGCALC_MCP_SCHEMA_DETAIL` | `full` | Control schema detail level in `tools/list` output: `compact`, `normal`, `full`. Invalid values warn to stderr and default to `full`. |
| `EGGCALC_NO_CONFIG` | — | Disable config file loading. Set automatically by `main.rs`. |

---

## Recommended Integration Patterns

### Pattern 1: Simple Model-Facing Agent

For agents where the model directly calls tools:

```rust
use eggsact::agent::{ToolRegistry, Profile, ToolAudience};

let registry = ToolRegistry::with_profile_and_audience(
    Profile::CodeggCoreMin,
    ToolAudience::Model,
);

// List tools for the model
let tools = registry.available_tools_model_safe();

// Call a tool
let response = registry.call_json("validate_json", serde_json::json!({
    "text": r#"{"key": "value"}"#,
})).unwrap();
```

### Pattern 2: Harness-Driven Preflight

For harnesses that run safety checks before executing edits:

```rust
use eggsact::preflight::{EditPreflight, EditPreflightInput};

let input = EditPreflightInput {
    file_path: Some("src/main.rs".to_string()),
    workspace_root: Some("/project".to_string()),
    old_text: "fn main() {}".to_string(),
    new_text: "fn main() { println!(\"hi\"); }".to_string(),
    ..Default::default()
};

match EditPreflight::run(&input) {
    Ok(output) => {
        // output.verdict -> EditVerdict (Proceed, ProceedWithCaution, Block, etc.)
        // output.machine_code -> machine-readable code
        // output.findings -> Vec<Finding> with severity/disposition
    }
    Err(e) => { /* PreflightError::ToolCall | ToolRejected | ContractViolation */ }
}
```

### Pattern 3: Per-Request State Isolation

For agents that need reproducible calculator state across calls:

```rust
use eggsact::agent::{ToolRegistry, ExecutionContext, Profile, ToolAudience};

let registry = ToolRegistry::new();

// Each request gets its own context
let ctx = ExecutionContext::agent_default(Profile::Full, ToolAudience::Model);
let r1 = registry.call_json_with_execution_context(
    "math_eval",
    serde_json::json!({ "expression": "rand()" }),
    &ctx,
).unwrap();
// r1 and r2 with the same seed produce different values,
// but two calls with the same seed produce the same first value
```

### Pattern 4: MCP Server Integration

For harnesses that spawn eggsact as a subprocess:

```json
{
  "command": "eggsact",
  "args": ["--mcp"],
  "env": {
    "EGGCALC_MCP_PROFILE": "codegg_core",
    "EGGCALC_MCP_AUDIENCE": "Model"
  }
}
```

Then send standard MCP messages:

```json
{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}
{"jsonrpc":"2.0","method":"tools/list","id":2,"params":{}}
{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"text_equal","arguments":{"a":"hello","b":"hello"}}}
```

---

## Codegg Integration Guide

### Profile + Audience Combinations by Workflow

| Workflow | Profile | Audience | Notes |
|----------|---------|----------|-------|
| **Ordinary coder session** | `codegg_core_min` | `Model` | Minimal tool set for constrained model-facing sessions |
| **Full coding session** | `codegg_core` | `Model` | 19 tools: text, math, path, JSON, analysis |
| **Edit preflight check** | `codegg_preflight` | `Harness` | Harness runs `edit_preflight` before applying edits |
| **Command preflight check** | `codegg_shell` | `Model` | Plan commands, parse shell syntax |
| **Command preflight (harness)** | `codegg_preflight` | `Harness` | Harness runs `command_preflight` before execution |
| **Config validation** | `codegg_config` | `Model` | JSON, TOML, dotenv, INI, Cargo.toml validation |
| **Unicode security review** | `codegg_unicode_security` | `Model` | Confusables, invisible chars, identifier analysis |
| **Patch/edit workflow** | `codegg_patch` | `Model` | Edit, patch, symbol-diff, patch-risk |
| **Repo audit** | `codegg_repo_audit` | `Model` | Manifest, config, lockfile, language, source structure |
| **Calculator only** | `human_math` | `Model` | Math eval, unit conversion, constants |
| **Debug/introspection** | `full` | `Debug` | All non-hidden tools, including HarnessOnly |

### Typical Codegg Session Flow

1. **Harness starts** with `codegg_preflight` + `Harness` audience
2. **Harness calls `tool_availability_explain`** to discover available tools
3. **Model session** uses `codegg_core_min` + `Model` audience for tool listings
4. **Before each edit**, harness calls `edit_preflight` (HarnessOnly) to get verdict
5. **Before each command**, harness calls `command_preflight` (HarnessOnly) for safety
6. **Before config writes**, harness calls `config_preflight` (HarnessOnly) to validate
7. **After edit**, harness calls `patch_apply_check` to verify the patch is safe
8. **Security checks** use `text_security_inspect` for Unicode anomalies

### Switching Profiles Mid-Session

The MCP server's profile is fixed at startup. To switch profiles:

- **In-process:** Construct a new `ToolRegistry` with the desired profile
- **MCP:** Restart the server with a different `EGGCALC_MCP_PROFILE` env var
- **In-process with context:** Use `call_json_with_execution_context` with `ctx.profile = Some(Profile::CodeggCore)` to override per-call

### Schema Detail Levels

Control `tools/list` output verbosity:

| Level | Description |
|-------|-------------|
| `full` | Complete input/output schemas (default) |
| `normal` | Input schemas only |
| `compact` | Names and descriptions only, no schemas |

Set via `EGGCALC_MCP_SCHEMA_DETAIL` env var or `tools/list` `schema_detail` parameter.
