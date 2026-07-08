# Agent API (In-Process)

The `src/agent/` module provides a typed, synchronous API for calling eggsact tools directly without starting an MCP server. It is the primary integration point for codegg and other Rust consumers. The entire module lives in a single file (`src/agent/mod.rs`) and re-exports types from `src/mcp/` for convenience.

See also: [Preflight Wrappers](preflight.md), [MCP Server](mcp-server.md), [Compatibility Mode](compatibility.md)

## Files

| File | Purpose |
|------|---------|
| `src/agent/mod.rs` | `ToolRegistry`, `Profile`, `ToolAudience`, `ToolView`, `ToolSpecView`, `ToolCallError`, `ToolCallOutcome`, `ExecutionContext`, `ExecutionSource`, `ExecutionContextBuilder`, all dispatch methods |
| `src/preflight/` | Typed preflight wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`) built on top of `ToolRegistry` |
| `tests/test_context_isolation.rs` | Integration tests for profile, audience, compat, budget, and eval-context isolation |
| `tests/mcp/test_route_contracts.rs` | Route-contract tests that exercise `prepare_tool_call` through the in-process API |

## Quick Start

```rust
use eggsact::agent::{ToolRegistry, Profile};

// Default registry: full profile, Model audience, StrictNative compat
let registry = ToolRegistry::default();
let response = registry.call_json("text_equal", serde_json::json!({
    "a": "hello",
    "b": "hello",
})).unwrap();
assert!(response.ok);
```

For typed preflight workflows, see the [Preflight Wrappers](preflight.md) doc. The preflight wrappers parse tool responses into structured Rust types with fail-closed contract enforcement.

---

## Core Types

### `ToolRegistry`

The central type for in-process tool dispatch. Wraps the consolidated tool registry (`src/mcp/registry/`) with profile filtering, audience filtering, and argument validation.

```rust
pub struct ToolRegistry {
    profile: Profile,
    audience: ToolAudience,
    compat_mode: CompatibilityMode,
}
```

All three fields are private. The registry is immutable after construction.

#### Constructors

| Method | Profile | Audience | Compat Mode | Description |
|--------|---------|----------|-------------|-------------|
| `ToolRegistry::new()` | `Full` | `Model` | `StrictNative` | Default. All non-hidden tools, model-safe listing. |
| `ToolRegistry::with_profile(profile)` | custom | `Model` | `StrictNative` | Profile-specific registry. |
| `ToolRegistry::with_profile_and_audience(profile, audience)` | custom | custom | `StrictNative` | Full control over profile and audience. |
| `.with_compat_mode(mode)` | — | — | custom | Builder-style setter for compat mode. |

**Getters:**

| Method | Returns |
|--------|---------|
| `profile()` | `&Profile` |
| `audience()` | `ToolAudience` |
| `compat_mode()` | `CompatibilityMode` |

`ToolRegistry` also implements `Default`, which delegates to `new()`.

---

### `Profile` Enum

Controls which subset of tools is available. Each variant maps to a string profile name used in the MCP registry. There are 11 named variants plus a `Custom` escape hatch:

| Variant | String Value | Intended Use |
|---------|--------------|-------------|
| `Full` | `"full"` | All non-hidden tools. Default for general use. |
| `Default` | `"default"` | General MCP clients. |
| `CodeggCoreMin` | `"codegg_core_min"` | Smallest model-visible profile. Excludes most tools. |
| `CodeggCore` | `"codegg_core"` | Broader model-safe profile with more text/JSON tools. |
| `CodeggPreflight` | `"codegg_preflight"` | Harness preflight checks (shell, path, config tools). |
| `CodeggPatch` | `"codegg_patch"` | Edit/patch workflows (patch_apply_check, edit_preflight). |
| `CodeggConfig` | `"codegg_config"` | Config validation (dotenv, INI, TOML, config_preflight). |
| `CodeggUnicodeSecurity` | `"codegg_unicode_security"` | Unicode security checks (confusables, policy). |
| `CodeggShell` | `"codegg_shell"` | Shell command tools (shell_split, command_preflight). |
| `CodeggRepoAudit` | `"codegg_repo_audit"` | Repository inspection (manifest, tree, language detect). |
| `HumanMath` | `"human_math"` | Calculator tools only (math_eval, unit_convert, etc.). |
| `Custom(name)` | name | Explicit custom profile for non-built-in names. |

#### Parsing

```rust
// Strict — returns None for unknown names
let p = Profile::from_str_opt("codegg_core");  // Some(Profile::CodeggCore)
let p = Profile::from_str_opt("typo");          // None

// Explicit custom — allows any name, including known ones
let p = Profile::custom("my_custom");           // Profile::Custom("my_custom")
```

**Key rule:** `from_str_opt` is strict and rejects unknown names. Use `Profile::custom(name)` when you intentionally want a profile not in the built-in set.

`Profile` implements `Display` (delegates to `as_str`), `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`, and `Default` (defaults to `Full`).

---

### `ToolAudience` Enum

Controls which exposure levels are included in tool listings and which tools may be executed. Three audiences with progressively inclusive filtering:

| Audience | Included Exposures | Excluded | Typical Use |
|----------|-------------------|----------|-------------|
| `Model` | Default, Contextual, ExpertOnly | HarnessOnly, Hidden | Model-facing codegg integrations |
| `Harness` | Default, Contextual, ExpertOnly, HarnessOnly | Hidden | Automatic preflight checks |
| `Debug` | All non-hidden (Default, Contextual, ExpertOnly, HarnessOnly) | Hidden only | Internal/debug listing |

#### `can_execute_exposure(exposure)`

Answers whether a given audience may execute a tool with a specific exposure level. Enforced at dispatch time by `prepare_tool_call` — the MCP server uses the same logic via `ToolRegistry::prepare_tool_call`.

```rust
assert!(ToolAudience::Model.can_execute_exposure(ToolExposure::Default));
assert!(!ToolAudience::Model.can_execute_exposure(ToolExposure::HarnessOnly));
assert!(ToolAudience::Harness.can_execute_exposure(ToolExposure::HarnessOnly));
assert!(!ToolAudience::Debug.can_execute_exposure(ToolExposure::Hidden));
```

`ToolAudience` implements `Clone`, `Copy`, `Debug`, `Default` (defaults to `Model`), `PartialEq`, `Eq`, `Hash`.

---

### `ToolView` — Read-Only Tool Metadata

A read-only snapshot of a tool's metadata, returned by listing methods. Does not include schemas.

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

Constructed internally from `ToolSpec` via `ToolView::from_spec()` (not public).

### `ToolSpecView` — Full Tool Metadata

Extends `ToolView` with input and output JSON schemas. Returned by `get_tool()`.

```rust
pub struct ToolSpecView {
    pub view: ToolView,
    pub input_schema: Value,
    pub output_schema: Value,
}
```

The schemas are produced by calling `spec.input_schema()` and `spec.output_schema()` — the schema builder functions registered in `ToolSpec`.

---

### `ToolCallError` — Pre-Execution Errors

Errors that occur before tool execution. Tool-level failures (e.g., invalid input the tool handles gracefully) return `Ok(ToolResponse)` with `ok: false` instead.

| Variant | Fields | Meaning |
|---------|--------|---------|
| `UnknownTool(name)` | `String` | Tool not found in the registry. |
| `ToolUnavailable { tool, profile }` | `String, String` | Tool exists but is not in the current profile. |
| `ToolNotAllowedForAudience { tool, profile, audience, exposure }` | all `String` | Audience cannot execute this tool due to exposure level. |
| `InvalidArguments(msg)` | `String` | Schema validation failed (missing required field, wrong type, etc.). |
| `Internal(msg)` | `String` | Internal error during lookup or dispatch. |

`ToolCallError` implements `Display` (human-readable messages) and `std::error::Error`.

---

### `ToolCallOutcome` — Dispatch Result

Returned by `prepare_tool_call`. Allows the MCP server to handle pre-execution errors as JSON-RPC errors while executing tools via the shared handler path.

```rust
pub enum ToolCallOutcome {
    Ready { handler: registry::ToolHandler },
    PreExecutionError(ToolCallError),
}
```

---

## Dispatch Methods

`ToolRegistry` provides five dispatch methods at increasing levels of sophistication, plus the shared `prepare_tool_call` core. All methods perform the same four-step pipeline:

1. **Tool lookup** — find the handler by name in the registry
2. **Profile check** — verify the tool is in the current profile
3. **Audience check** — verify the audience can execute the tool's exposure level
4. **Argument validation** — validate against the tool's input schema

Methods that accept a budget also perform **input pre-check** (rejects oversized serialized args before dispatch) and **output truncation** (truncates findings and result to fit budget limits).

### `call_json(name, args)` — Basic

The primary entry point for in-process tool execution.

```rust
pub fn call_json(&self, name: &str, args: Value) -> Result<ToolResponse, ToolCallError>
```

**Behavior:** Calls `prepare_tool_call`, then invokes the handler directly if ready. No budget enforcement, no output truncation. Returns `Ok(ToolResponse)` with `ok: false` for tool-level failures; `Err(ToolCallError)` for registry-level failures.

### `call_json_with_budget(name, args, budget)` — Budget-Aware

Extends `call_json` with resource limits and output truncation.

```rust
pub fn call_json_with_budget(
    &self, name: &str, args: Value, budget: Option<ToolBudget>
) -> Result<ToolResponse, ToolCallError>
```

**Behavior:**
1. Resolves the tool's default budget from its declared `ToolCost` (via `budget_for_tool`)
2. Merges with any explicit `budget` override
3. Pre-checks serialized input size against `budget.max_input_bytes` — returns `INPUT_TOO_LARGE` machine code on failure
4. Executes the handler via `call_json`
5. Truncates output findings and result to fit within budget limits
6. Populates `limits_applied` on the response

Use `None` for budget to use the tool's default. Use `Some(ToolBudget::CHEAP)` or custom budgets for tighter limits.

### `call_json_with_context(name, args, budget, cancel_flag)` — With Cancellation

Extends budget-aware dispatch with cooperative cancellation.

```rust
pub fn call_json_with_context(
    &self, name: &str, args: Value,
    budget: Option<ToolBudget>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<ToolResponse, ToolCallError>
```

**Behavior:** Same as `call_json_with_budget`, but sets the cancellation flag as a thread-local during handler execution. High-risk handlers that create their own `BudgetContext` (via `budget::for_handler`) will inherit the cancellation flag via `budget::with_cancel_flag`.

The flag is cooperative — setting it requests cancellation but does not interrupt executing code. Handlers check `BudgetContext::should_stop()` at pipeline stages.

### `call_json_with_execution_context(name, args, ctx)` — Full Context (Recommended)

The context-aware entry point. Honors all dispatch-scoped state from the `ExecutionContext`.

```rust
pub fn call_json_with_execution_context(
    &self, name: &str, args: Value, ctx: &ExecutionContext
) -> Result<ToolResponse, ToolCallError>
```

**Behavior:**
1. Resolves budget from `ctx.budget` (falls back to tool default)
2. Pre-checks input size
3. Resolves effective profile/audience from `ctx` (falls back to registry defaults)
4. Sets cancellation flag via `budget::with_cancel_flag`
5. Clones `ctx.eval_ctx` and sets it via `budget::with_eval_context`
6. Performs lookup, profile check, audience check, schema validation
7. Executes handler and truncates output

**Key semantics:**
- `ctx.profile` / `ctx.audience` take precedence over the registry's stored values when `Some`
- `ctx.eval_ctx` is **cloned** before dispatch — handler mutations do not persist back
- `ctx.compatibility_mode` is used for schema validation
- `ctx.cancellation` enables cooperative cancellation

This is the recommended method for new code. Legacy methods (`call_json`, `call_json_with_budget`, `call_json_with_context`) remain for backward compatibility.

### `call_json_value(name, args)` — Convenience

Returns only the `result` Value, or `null` on error.

```rust
pub fn call_json_value(&self, name: &str, args: Value) -> Value
```

Wraps `call_json` and extracts `response.result.unwrap_or(Value::Null)`. Discards `ToolCallError` and non-ok responses silently. Useful for quick prototyping or when the caller does not need error details.

### `prepare_tool_call(name, args)` — Shared Core

Performs lookup, profile check, audience check, and argument validation without executing the handler. Returns a `ToolCallOutcome` that the caller can match on.

```rust
pub fn prepare_tool_call(&self, name: &str, args: &Value) -> ToolCallOutcome
```

This is the shared core used by both the agent API (`call_json` calls it) and the MCP server (`tools/call` dispatch uses the same logic). It ensures consistent validation across both integration paths.

**Four-step pipeline:**

1. `registry::tool_handler_for(name)` — look up the handler function
2. `registry::tools_for_profile(self.profile.as_str())` — check profile membership
3. `self.audience.can_execute_exposure(spec.exposure)` — check audience/exposure compatibility
4. `schema_validation::validate_arguments(name, args, self.compat_mode)` — validate arguments against the tool's input schema

---

## Listing Methods

| Method | Description | Model-Safe? |
|--------|-------------|-------------|
| `available_tools()` | **Deprecated** (since 0.3.0). Filters only `Hidden`. | No |
| `available_tools_model_safe()` | Excludes `HarnessOnly` + `Hidden`. | Yes |
| `available_tools_for_audience(audience)` | Filters by the specified audience. | Depends on audience |
| `available_tools_for_current_audience()` | Uses the registry's stored audience. | Depends on stored audience |
| `get_tool(name)` | Returns `Option<ToolSpecView>` with full metadata and schemas. | Profile-gated |
| `has_tool(name)` | Returns `bool` — whether the tool is in the current profile. | N/A |

**Deprecation note:** `available_tools()` is deprecated because it only filters `Hidden` and is not model-safe — it may expose `HarnessOnly` tools. Use `available_tools_model_safe()` or `available_tools_for_audience(audience)` instead.

All listing methods delegate to `registry::tools_for_profile()` or `registry::tools_for_profile_audience()` from `src/mcp/registry/listing.rs`.

---

## `ExecutionContext`

Bundles all per-request dispatch state into a single explicit parameter. This replaces implicit reliance on global statics (`ACTIVE_PROFILE`, thread-local cancel flags) for new code paths.

### Fields

```rust
pub struct ExecutionContext {
    pub eval_ctx: EvalContext,
    pub compatibility_mode: CompatibilityMode,
    pub profile: Option<Profile>,
    pub audience: Option<ToolAudience>,
    pub budget: Option<ToolBudget>,
    pub cancellation: Option<Arc<AtomicBool>>,
    pub request_id: Option<String>,
    pub source: ExecutionSource,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `eval_ctx` | `EvalContext` | Calculator state (PRNG seed, memory registers, user variables). Cloned at dispatch. |
| `compatibility_mode` | `CompatibilityMode` | Validation behavior (`EggcalcPython` vs `StrictNative`). |
| `profile` | `Option<Profile>` | Active profile for tool filtering. `None` falls back to registry default. |
| `audience` | `Option<ToolAudience>` | Audience for exposure filtering. `None` falls back to registry default. |
| `budget` | `Option<ToolBudget>` | Per-tool resource limits. `None` uses the tool's declared default budget. |
| `cancellation` | `Option<Arc<AtomicBool>>` | Cooperative cancellation flag. Checked at pipeline stages by `BudgetContext::should_stop()`. |
| `request_id` | `Option<String>` | Optional request ID for tracing. Passthrough; not used by dispatch logic. |
| `source` | `ExecutionSource` | Origin of the call, for logging and auditing. |

### `ExecutionSource` Enum

Distinguishes the caller's integration point:

| Variant | Description |
|---------|-------------|
| `Cli` | Direct CLI invocation. |
| `Library` | Library/API call. |
| `Mcp` | MCP server dispatch. |
| `Agent` | In-process agent call. |
| `Test` | Test invocation (default). |

### Named Constructors

| Method | Profile | Audience | Compat | Source | EvalContext |
|--------|---------|----------|--------|--------|-------------|
| `cli_default()` | `None` | `None` | `StrictNative` | `Cli` | `EvalContext::new()` |
| `library_default()` | `None` | `None` | `StrictNative` | `Library` | `EvalContext::new()` |
| `mcp_default(profile, audience)` | `Some` | `Some` | `EggcalcPython` | `Mcp` | `EvalContext::mcp_mode()` |
| `agent_default(profile, audience)` | `Some` | `Some` | `StrictNative` | `Agent` | `EvalContext::new()` |
| `test_default()` | `None` | `None` | `StrictNative` | `Test` | `EvalContext::new()` |

`ExecutionContext::default()` delegates to `test_default()`.

### Builder Pattern

For named field initialization without relying on constructor defaults:

```rust
let ctx = ExecutionContext::builder()
    .profile(Profile::CodeggCoreMin)
    .audience(ToolAudience::Model)
    .compatibility_mode(CompatibilityMode::StrictNative)
    .budget(ToolBudget::with_max_output_bytes(500_000))
    .cancellation(Arc::new(AtomicBool::new(false)))
    .request_id("req-abc-123")
    .source(ExecutionSource::Agent)
    .build();
```

The builder fills in defaults for unset fields: `eval_ctx` → `EvalContext::new()`, `compatibility_mode` → default, `source` → `Test`, `profile`/`audience`/`budget`/`cancellation`/`request_id` → `None`.

### Builder-Style Setters (on `ExecutionContext`)

```rust
let ctx = ExecutionContext::test_default()
    .with_budget(ToolBudget::CHEAP)
    .with_cancellation(Arc::new(AtomicBool::new(false)))
    .with_request_id("tracing-id")
    .with_eval_context(&mut eval_ctx);
```

These clone the input and return `Self` for chaining.

### Eval-Context Bridge

The `eval_ctx` field holds calculator state (PRNG seed, memory registers, user-defined variables). Only calculator-backed tools that opt into the eval-context bridge read from it; the sole current consumer is **`math_eval`**.

**Clone semantics:**
1. `with_eval_context()` clones the caller's `EvalContext` into the `ExecutionContext`
2. `call_json_with_execution_context()` clones `ctx.eval_ctx` before dispatch
3. The handler receives a thread-local reference to the clone via `budget::with_eval_context()`
4. PRNG draws, memory mutations, and variable assignments inside the handler are confined to the clone
5. The caller's `ExecutionContext.eval_ctx` is **never mutated**

Two calls with identical seeds produce the same first random value. For persistent mutable state across calls, use `evaluate_with_context()` / `run_with_context()` directly (which operate on the caller's `EvalContext` without cloning).

**Do not mix** `call_json_with_execution_context` and `evaluate_with_context` for the same `EvalContext` — the former clones and discards mutations, the latter persists them.

---

## Relationship to MCP Server

The MCP server (`src/mcp/server.rs`) and the in-process agent API share the same core dispatch logic:

| Aspect | MCP Server | In-Process Agent API |
|--------|-----------|---------------------|
| Dispatch core | Uses `registry::tool_handler_for`, `tools_for_profile`, `schema_validation::validate_arguments` directly | Uses `prepare_tool_call` which wraps the same functions |
| Profile resolution | `EGGCALC_MCP_PROFILE` env var at startup | `Profile` field on `ToolRegistry` |
| Audience resolution | `EGGCALC_MCP_AUDIENCE` env var (defaults to `Model`) | `ToolAudience` field on `ToolRegistry` |
| Compat mode | `EggcalcPython` (Python-parity errors) | `StrictNative` (default) |
| Cancellation | Creates `Arc<AtomicBool>` per request, attached via `with_cancellation()` | Optional, via `call_json_with_context` or `call_json_with_execution_context` |
| Budget | Resolved from `ToolSpec.cost` per tool | Same, or explicit `ToolBudget` parameter |
| Input pre-check | Yes, `budget.max_input_bytes` | Yes, same check |
| Output truncation | Yes, via `truncate_response` | Yes, same function |

`call_json_with_execution_context` is an **in-process** API. It does not change the MCP JSON-RPC wire protocol. Per-request context over the wire would require a future MCP request-level context API.

---

## Testing

### Unit Tests

Inline tests in `src/agent/mod.rs` (inside `#[cfg(test)] mod tests`):

- Profile filtering (full vs restricted profiles)
- Audience enforcement (Model rejects HarnessOnly, Harness allows it)
- Unknown tool errors
- Invalid arguments errors
- `call_json`, `call_json_with_budget` success and error paths
- `call_json_value` convenience wrapper
- `get_tool` metadata retrieval
- `Profile::from_str_opt` and `Profile::custom` behavior
- `ToolAudience::can_execute_exposure` for all audience/exposure combinations

### Integration Tests

| Test File | Focus |
|-----------|-------|
| `tests/test_context_isolation.rs` | Profile isolation, audience isolation, compat mode isolation, budget isolation, eval-context clone semantics, concurrent access patterns |
| `tests/mcp/test_route_contracts.rs` | Route-contract tests using `prepare_tool_call` through the in-process API, including `RouteFixture` struct and `all_fixtures()` |
| `tests/mcp/test_hardening_and_gaps.rs` | Profile snapshot tests, audience enforcement at dispatch, schema validation edge cases |
| `tests/mcp/test_cancellation.rs` | Cooperative cancellation via `call_json_with_context` |
| `tests/mcp/test_preflight_wrappers.rs` | Typed preflight wrappers built on `ToolRegistry` |
| `tests/mcp/test_diagnostics.rs` | Diagnostic tools via `ToolRegistry` |
| `tests/mcp/test_determinism_concurrency.rs` | Concurrent access and determinism guarantees |

### Running Tests

```bash
cargo test --lib                                         # unit tests in src/ only
cargo test --lib agent                                   # agent module unit tests
cargo test --test test_context_isolation                 # context isolation integration tests
cargo test --test lib -- test_strict_native              # integration tests
cargo test --all-features --tests -- --skip parity       # all integration tests (excluding parity)
```
