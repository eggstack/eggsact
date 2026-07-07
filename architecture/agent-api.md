# Agent API (In-Process)

The `src/agent/` module provides a typed, synchronous API for calling eggsact tools directly without starting an MCP server. It is the primary integration point for codegg and other Rust consumers.

See also: [Preflight Wrappers](preflight.md), [MCP Server](mcp-server.md)

## Files

| File | Purpose |
|------|---------|
| `src/agent/mod.rs` | `ToolRegistry`, `Profile`, `ToolAudience`, `ExecutionContext`, `ToolCallError`, `ToolView` |

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

**Constructors:**

| Method | Profile | Audience | Compat Mode |
|--------|---------|----------|-------------|
| `ToolRegistry::new()` | Full | Model | StrictNative |
| `ToolRegistry::with_profile(profile)` | custom | Model | StrictNative |
| `ToolRegistry::with_profile_and_audience(profile, audience)` | custom | custom | StrictNative |
| `.with_compat_mode(mode)` | — | — | custom |

### `Profile` Enum

Controls which subset of tools is available. 11 named profiles + custom:

| Variant | String | Intended Use |
|---------|--------|-------------|
| `Full` | `"full"` | All non-hidden tools |
| `Default` | `"default"` | General MCP clients |
| `CodeggCoreMin` | `"codegg_core_min"` | Smallest model-visible profile |
| `CodeggCore` | `"codegg_core"` | Broader model-safe profile |
| `CodeggPreflight` | `"codegg_preflight"` | Harness preflight checks |
| `CodeggPatch` | `"codegg_patch"` | Edit/patch workflows |
| `CodeggConfig` | `"codegg_config"` | Config validation |
| `CodeggUnicodeSecurity` | `"codegg_unicode_security"` | Unicode security checks |
| `CodeggShell` | `"codegg_shell"` | Shell command tools |
| `CodeggRepoAudit` | `"codegg_repo_audit"` | Repository inspection |
| `HumanMath` | `"human_math"` | Calculator tools |
| `Custom(name)` | — | Explicit custom profile |

`Profile::from_str_opt()` is strict — returns `None` for unknown names. Use `Profile::custom(name)` to construct explicitly.

### `ToolAudience` Enum

Controls which exposure levels are included in tool listings and which tools may be executed:

| Audience | Includes | Excludes |
|----------|----------|----------|
| `Model` | Default, Contextual, ExpertOnly | HarnessOnly, Hidden |
| `Harness` | Default, Contextual, ExpertOnly, HarnessOnly | Hidden |
| `Debug` | All non-hidden | Hidden only |

`ToolAudience::can_execute_exposure(exposure)` answers whether a given audience may execute a tool with a specific exposure level. Enforced at dispatch time by `prepare_tool_call`.

### `ToolView`

Read-only metadata about a tool:

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

### `ToolCallError`

Errors that occur before tool execution:

| Variant | Meaning |
|---------|---------|
| `UnknownTool(name)` | Tool not found in registry |
| `ToolUnavailable { tool, profile }` | Tool exists but not in current profile |
| `ToolNotAllowedForAudience { tool, profile, audience, exposure }` | Audience cannot execute this tool |
| `InvalidArguments(msg)` | Schema validation failed |
| `Internal(msg)` | Internal error |

Tool-level failures (e.g., invalid input the tool handles gracefully) return `Ok(ToolResponse)` with `ok: false` instead.

## Dispatch Methods

### `call_json(name, args)` — Basic

Primary entry point. Performs: tool lookup → profile check → audience check → argument validation → handler execution.

```rust
pub fn call_json(&self, name: &str, args: Value) -> Result<ToolResponse, ToolCallError>
```

### `call_json_with_budget(name, args, budget)` — Budget-Aware

Extends `call_json` with resource limits. Pre-checks input size, executes, truncates output.

```rust
pub fn call_json_with_budget(
    &self, name: &str, args: Value, budget: Option<ToolBudget>
) -> Result<ToolResponse, ToolCallError>
```

### `call_json_with_context(name, args, budget, cancel_flag)` — With Cancellation

Extends budget-aware dispatch with cooperative cancellation flag.

```rust
pub fn call_json_with_context(
    &self, name: &str, args: Value,
    budget: Option<ToolBudget>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<ToolResponse, ToolCallError>
```

### `call_json_with_execution_context(name, args, ctx)` — Full Context (Recommended)

The context-aware entry point. Honors profile, audience, compat mode, budget, cancellation, and EvalContext from the `ExecutionContext`.

```rust
pub fn call_json_with_execution_context(
    &self, name: &str, args: Value, ctx: &ExecutionContext
) -> Result<ToolResponse, ToolCallError>
```

**Key behavior**: `ctx.eval_ctx` is **cloned** before dispatch. Calculator-backed handlers (e.g., `math_eval`) read from the clone via a thread-local. Handler mutations **do not persist** back to the caller's `ExecutionContext`.

### `call_json_value(name, args)` — Convenience

Returns only the `result` Value, or `null` on error.

### `prepare_tool_call(name, args)` — Pre-Execution Check

Returns a `ToolCallOutcome::Ready { handler }` or `ToolCallOutcome::PreExecutionError(error)`. Used by both the agent API and the MCP server to share lookup/validation logic.

## Listing Methods

| Method | Description |
|--------|-------------|
| `available_tools()` | **Deprecated.** Filters only Hidden. Not model-safe. |
| `available_tools_model_safe()` | Excludes HarnessOnly + Hidden. For model-facing codegg. |
| `available_tools_for_audience(audience)` | Filters by specific audience. |
| `available_tools_for_current_audience()` | Uses registry's stored audience. |
| `get_tool(name)` | Detailed `ToolSpecView` for a specific tool. |
| `has_tool(name)` | Check if tool is in current profile. |

## `ExecutionContext`

Bundles all per-request dispatch state:

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

### `ExecutionSource` Enum

Distinguishes callers: `Cli`, `Library`, `Mcp`, `Agent`, `Test`.

### Named Constructors

| Method | Profile | Audience | Compat | Source |
|--------|---------|----------|--------|--------|
| `cli_default()` | None | None | StrictNative | Cli |
| `library_default()` | None | None | StrictNative | Library |
| `mcp_default(profile, audience)` | Some | Some | EggcalcPython | Mcp |
| `agent_default(profile, audience)` | Some | Some | StrictNative | Agent |
| `test_default()` | None | None | StrictNative | Test |

### Builder Pattern

```rust
let ctx = ExecutionContext::builder()
    .profile(Profile::CodeggCoreMin)
    .audience(ToolAudience::Model)
    .budget(ToolBudget::with_max_output_bytes(500_000))
    .cancellation(Arc::new(AtomicBool::new(false)))
    .build();
```

### Eval-Context Bridge

`eval_ctx` is cloned at dispatch. Calculator-backed tools read from the clone. PRNG draws, memory mutations, and variable assignments inside the handler are confined to the clone. For persistent mutable state across calls, use `evaluate_with_context()`/`run_with_context()` directly.

## Relationship to MCP Server

The MCP server (`src/mcp/server.rs`) resolves its active profile from `EGGCALC_MCP_PROFILE` at startup and creates a `ToolRegistry` per `tools/call` request. It uses `EggcalcPython` compatibility mode (Python-parity errors). The in-process agent API defaults to `StrictNative` mode.

`call_json_with_execution_context` is an **in-process** API. It does not change the MCP JSON-RPC wire protocol.

## Tests

Unit tests in `src/agent/mod.rs` verify profile filtering, audience enforcement, unknown tool errors, and argument validation. Integration tests in `tests/mcp/test_hardening_and_gaps.rs` exercise the full dispatch path.

```bash
cargo test --lib agent                    # unit tests
cargo test --test lib -- test_strict_native  # integration tests
```
