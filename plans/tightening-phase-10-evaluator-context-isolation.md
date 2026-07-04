# Phase 10 Plan: Evaluator Context Isolation

## Purpose

Phase 10 isolates mutable runtime/evaluator state so tool calls are deterministic, testable, and safe under concurrent MCP and in-process agent use. Earlier phases hardened MCP profiles, route-critical tool contracts, parser-backed preflights, and cooperative budgets. This phase addresses the next architectural boundary: explicit per-call context instead of hidden process-global behavior.

This phase should not redesign the calculator grammar or add tools. It should make existing behavior safer and more predictable for agents, especially codegg.

## Background

The crate currently supports two broad execution paths:

- CLI/library calculator calls such as `run()` and `evaluate()`;
- MCP/agent tool calls through `ToolRegistry`, `tools/call`, and handler functions.

The MCP side now has profile/audience enforcement, budget checks, and cooperative cancellation. However, evaluator/runtime behavior still has risk areas common to single-process tool servers:

- hidden global state;
- implicit environment-derived behavior;
- unclear per-call isolation boundaries;
- concurrency assumptions that are not represented in function signatures;
- tests that may pass serially but fail under parallel execution or future server concurrency.

Phase 10 turns these into explicit context objects and isolation tests.

## Goals

- Define a per-call execution context for calculator/evaluator and MCP tool execution.
- Separate immutable configuration from mutable per-call state.
- Avoid hidden process-global mutation except for documented, controlled compatibility shims.
- Make concurrent tool calls deterministic.
- Provide tests that catch state leakage between calls, profiles, audiences, budgets, and compatibility modes.

## Non-Goals

- Do not rewrite the calculator parser/evaluator from scratch.
- Do not change public math semantics unless a bug is discovered and documented.
- Do not add a persistent database or long-term memory store.
- Do not alter phase 06–09 response contracts except where context propagation requires internal plumbing.
- Do not implement process-level sandboxing.

## Workstream A: Inventory Mutable State and Implicit Context

### Required Work

1. Search for global/static/mutable state.

   Commands:

   ```bash
   rg "static|lazy_static|OnceLock|Mutex|RwLock|thread_local|Atomic" src tests
   rg "env::|std::env|EGGCALC|EGG" src tests
   rg "set_|current_|global|default" src/mcp src/calc src/agent src/tools
   ```

2. Classify each state source:

   - immutable cache/table;
   - compatibility configuration;
   - environment-derived runtime config;
   - request-scoped state;
   - process-global mutable state;
   - thread-local shim.

3. Produce a short internal inventory comment or doc section in `architecture/overview.md` or `architecture/mcp-server.md`.

4. Identify which state sources need no change because they are immutable generated data or pure caches.

### Acceptance Criteria

- Mutable state inventory exists in docs or comments.
- All process-global or thread-local state has an explicit rationale.
- No unclassified global mutable state remains.

## Workstream B: Define `ExecutionContext`

### Required Work

Create a context type that can carry execution-scoped settings without forcing all callers to pass many independent parameters.

Recommended shape:

```rust
pub struct ExecutionContext {
    pub compatibility_mode: CompatibilityMode,
    pub profile: Option<Profile>,
    pub audience: Option<ToolAudience>,
    pub budget: Option<ToolBudget>,
    pub cancellation: Option<Arc<AtomicBool>>,
    pub request_id: Option<String>,
    pub source: ExecutionSource,
}

pub enum ExecutionSource {
    Cli,
    Library,
    Mcp,
    Agent,
    Test,
}
```

Keep the first implementation minimal. Do not expose fields publicly if builder methods are cleaner.

2. Add constructors:

- `ExecutionContext::cli_default()`;
- `ExecutionContext::library_default()`;
- `ExecutionContext::mcp_default(profile, audience)`;
- `ExecutionContext::agent_default(profile, audience)`;
- `ExecutionContext::test_default()`.

3. Add builder methods for optional budget/cancellation/request id.

4. Ensure defaults preserve current behavior.

### Acceptance Criteria

- A context type exists and compiles.
- Defaults preserve existing CLI/library/MCP behavior.
- Tests prove default construction and builder behavior.

## Workstream C: Context-Aware Calculator API

### Required Work

1. Add context-aware internal APIs without breaking public API.

Recommended functions:

```rust
pub fn run_with_context(input: &str, ctx: &ExecutionContext) -> Result<(String, String), Error>;
pub fn evaluate_with_context(input: &str, ctx: &ExecutionContext) -> Result<(String, String), Error>;
```

2. Keep existing `run()` and `evaluate()` as wrappers using `ExecutionContext::library_default()` or current-equivalent defaults.

3. Move environment/config lookups out of inner evaluator paths where practical.

4. Ensure context can eventually carry per-call evaluator options, but do not add unused complexity.

### Acceptance Criteria

- Public API remains backward compatible.
- Context-aware API exists for internal/future use.
- Existing calculator tests pass unchanged.
- New tests verify wrapper equivalence between old and context-aware APIs.

## Workstream D: Context-Aware Tool Dispatch

### Required Work

1. Extend `ToolRegistry` with context-aware dispatch.

Potential API:

```rust
pub fn call_json_with_execution_context(
    &self,
    name: &str,
    args: Value,
    ctx: &ExecutionContext,
) -> Result<ToolResponse, ToolCallError>;
```

2. Reuse existing behavior:

- profile/audience checks from `prepare_tool_call`;
- budget checks from `call_json_with_budget`;
- cancellation propagation from `call_json_with_context`.

3. Keep existing APIs as wrappers to avoid caller breakage.

4. Do not force every handler to accept `ExecutionContext` yet. This phase can bridge by setting the thread-local cancellation flag and using existing handler signatures.

### Acceptance Criteria

- Existing `call_json`, `call_json_with_budget`, and `call_json_with_context` still work.
- New context-aware dispatch path works for at least one simple tool and one route-critical tool.
- Tests verify profile, audience, compatibility mode, budget, and cancellation behavior through the context-aware path.

## Workstream E: Remove or Contain Environment Coupling

### Required Work

1. Identify environment variables used at runtime.

Known examples include:

- `EGGCALC_MCP_PROFILE`;
- `EGGCALC_MCP_SCHEMA_DETAIL`;
- `EGGCALC_NO_CONFIG`.

2. Keep environment parsing at process/startup boundaries.

3. Convert environment-derived state into explicit context/config objects before handler dispatch.

4. Add tests that set environment variables only around startup/config parsing, not deep inside tool calls.

5. Ensure parallel tests do not race on environment mutation. Use serial tests where unavoidable and document why.

### Acceptance Criteria

- Deep evaluator/tool code does not directly read mutable environment where context/config can be passed.
- Startup environment behavior remains compatible.
- Tests avoid environment races or explicitly serialize them.

## Workstream F: Concurrency and Isolation Tests

### Required Work

Add tests that would fail if context leaks between calls.

Required cases:

1. Two registries with different profiles run in the same process and enforce different tool availability.

2. A model-audience registry rejects harness-only tools while a harness-audience registry allows them.

3. StrictNative and EggcalcPython compatibility modes do not leak between calls.

4. A cancelled context does not poison later uncancelled calls.

5. A tiny-budget call fails with `INPUT_TOO_LARGE` or truncates as expected, while a later normal-budget call succeeds.

6. Parallel calls to simple tools produce deterministic results.

Use Rust threads or `tokio` tests only if deterministic and not flaky.

### Acceptance Criteria

- Isolation tests are deterministic.
- No shared context leakage is observed.
- Cancellation and budget state do not persist across independent calls.

## Workstream G: Documentation

### Required Work

Update architecture docs to explain:

- what `ExecutionContext` is;
- which APIs are context-aware;
- which legacy APIs are wrappers;
- how MCP startup env vars become runtime context;
- why handler signatures remain unchanged for compatibility;
- what state remains global/thread-local and why.

Update `AGENTS.md` if agent-facing guidance changes.

### Acceptance Criteria

- Docs reflect the actual context boundary.
- Agent guidance includes when to use context-aware APIs.
- No docs imply hard isolation where only cooperative/context isolation exists.

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Also run focused tests for the new isolation module.

## Final Acceptance Criteria

Phase 10 is complete when:

- mutable state has been inventoried and classified;
- `ExecutionContext` exists with stable defaults;
- calculator and tool dispatch have context-aware paths;
- legacy APIs remain compatible wrappers;
- environment coupling is contained at startup/config boundaries where practical;
- isolation/concurrency tests cover profile, audience, compatibility, cancellation, budget, and parallel calls;
- docs explain the new context boundary;
- full verification passes or exact failures are documented.
