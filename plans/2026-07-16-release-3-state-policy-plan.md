# Release 3 — State Isolation and Policy Consistency

## Purpose

This release completes the transition from compatibility-oriented global state toward explicit per-call and per-session state. It also removes remaining disagreement between tool metadata lookup, listing, and execution policy.

The current context-aware calculator APIs are sound, but MCP startup still enables a process-global safe mode. `ExecutionContext` is cloned during dispatch, so its name may imply persistence that it does not provide. Registry metadata helpers also check profile membership without always applying audience restrictions.

The target is an embedding-safe API for codegg and other Rust consumers where state lifetime, mutability, permissions, and compatibility mode are explicit.

## Goals

1. Remove MCP dependence on process-global calculator mode and permission switches.
2. Preserve legacy APIs without allowing them to contaminate new context-aware paths.
3. Distinguish immutable execution templates from persistent mutable execution contexts.
4. Add a mutable dispatch API for stateful calculator sessions.
5. Make thread-local execution bridges panic-safe and nest-safe.
6. Apply profile/audience policy consistently to listing, metadata lookup, and execution.
7. Consolidate pre-execution policy checks into one internal path.
8. Add mixed-surface, persistence, panic-restoration, and policy-invariant tests.

## Non-goals

- Do not remove legacy `run()` or `evaluate()` in this release unless a major-version policy already permits it.
- Do not make MCP calculator calls stateful by default.
- Do not expose hidden or harness-only tools to models for convenience.
- Do not redesign every tool handler to accept a new context parameter directly.
- Do not split the crate or add feature gating in this release.

# Workstream 1 — Remove MCP process-global evaluator mutation

## Current behavior

The MCP server invokes an idempotent global `set_mcp_mode()`. That permanently changes global random and side-effect permission flags used by legacy calculator APIs in the process. An embedding that invokes MCP dispatch and legacy calculator calls can therefore observe cross-surface behavior changes.

## Target behavior

MCP math dispatch must use `EvalContext::mcp_mode()` through the existing execution-context bridge. Starting or using the MCP server must not mutate:

- global MCP mode flags;
- global random permission;
- global side-effect permission;
- global memory registers;
- global user variables;
- global PRNG state.

## Implementation steps

1. Trace `math_eval` from MCP dispatch through `ToolRegistry`, budget thread-locals, and calculator calls.
2. Ensure MCP constructs `ExecutionContext::mcp_default(profile, audience)` for every request or derives an equivalent immutable session template.
3. Pass that context into the shared dispatch path.
4. Remove `runtime::ensure_mcp_defaults()` from normal MCP request execution.
5. Mark global `set_mcp_mode()` as legacy compatibility behavior in docs and code comments.
6. Deprecate public access to global mode controls if semver permits.
7. Add tests proving MCP invocation leaves legacy calculator permission behavior unchanged.

Do not simply reset global flags after an MCP call; that remains race-prone. The new path must avoid mutating them.

# Workstream 2 — Clarify execution template versus persistent context

## API problem

`call_json_with_execution_context(&ExecutionContext)` clones `eval_ctx` before dispatch. State changes therefore do not persist. This is deterministic and useful, but the context name can imply a durable session.

## Proposed API model

Retain an immutable/template API and add a mutable/persistent API.

### Immutable template

Either preserve the current method with stronger naming/docs or add an alias such as:

```rust
pub fn call_json_with_execution_template(
    &self,
    name: &str,
    args: Value,
    template: &ExecutionContext,
) -> Result<ToolResponse, ToolCallError>
```

Semantics:

- clone calculator state before the call;
- changes never propagate to the caller;
- repeated calls with the same seed/template are reproducible;
- appropriate for ordinary deterministic agent calls.

Keep the old method as a deprecated forwarding alias if renaming is selected.

### Mutable persistent context

Add:

```rust
pub fn call_json_with_execution_context_mut(
    &self,
    name: &str,
    args: Value,
    ctx: &mut ExecutionContext,
) -> Result<ToolResponse, ToolCallError>
```

Semantics:

- use `ctx.eval_ctx` directly for calculator-backed tools;
- PRNG, memory, and user-variable mutations persist after successful dispatch;
- profile, audience, compatibility mode, budget, cancellation, request ID, and source remain explicit;
- non-calculator tools do not mutate calculator state;
- on pre-execution policy failure, calculator state remains unchanged.

Document behavior if a handler returns a tool-level error after partial mutation. Preferred approach: calculator evaluation should either commit deterministically or document operation-specific mutation semantics. Consider cloning and committing only on successful evaluation if transactional state is feasible.

## Builder and naming cleanup

Review `ExecutionContext::with_eval_context(&mut EvalContext)`: it currently clones despite accepting `&mut`. Change the parameter to `&EvalContext` or accept by value. Avoid signatures that imply mutation when none occurs.

Add explicit constructors or aliases:

- `ExecutionContext::deterministic_agent_template(...)`;
- `ExecutionContext::mcp_template(...)`;
- `ExecutionContext::stateful_library(...)` where useful.

Do not proliferate constructors unnecessarily; prioritize clear semantics.

# Workstream 3 — Panic-safe thread-local bridges

## Scope

Audit thread-local bridges in `src/mcp/budget.rs` and related modules for:

- cancellation flag;
- evaluation context;
- budget/deadline state;
- nested dispatch behavior.

## RAII guards

Replace manual set/restore patterns with guard objects whose `Drop` restores the previous value. Requirements:

- nested scopes restore the immediate parent value;
- panics restore the prior thread-local state during unwind;
- a handler cannot leak cancellation or eval context into the next task on the same blocking thread;
- mutable context references do not outlive the dispatch closure.

Use `catch_unwind` only where converting panics to structured errors is already part of policy. RAII restoration must work regardless.

## Tests

- Set an outer cancellation flag, nest an inner flag, panic, catch unwind, and verify the outer flag is restored.
- Repeat for eval context.
- Execute a panicking test handler followed by a normal handler on reused blocking threads; verify no state leakage.
- Verify concurrent registries on different threads remain isolated.

# Workstream 4 — Consolidate pre-execution policy

## Current duplication

Profile lookup, audience checks, and schema validation are implemented in `prepare_tool_call`, but `call_json_with_execution_context` repeats similar logic to apply context overrides. MCP also constructs its own registry and maps errors separately.

## Target abstraction

Create one internal policy preparation function, for example:

```rust
struct EffectiveDispatchPolicy<'a> {
    profile: &'a Profile,
    audience: ToolAudience,
    compatibility_mode: CompatibilityMode,
}

struct PreparedToolCall {
    spec: &'static ToolSpec,
    handler: ToolHandler,
}

fn prepare_tool_call_with_policy(
    name: &str,
    args: &Value,
    policy: EffectiveDispatchPolicy<'_>,
) -> Result<PreparedToolCall, ToolCallError>
```

All dispatch surfaces should use this function:

- `call_json`;
- `call_json_with_budget`;
- immutable execution-template dispatch;
- mutable execution-context dispatch;
- MCP tools/call;
- typed preflight wrappers indirectly through the registry.

Keep transport-specific error mapping in MCP, but do not duplicate policy decisions there.

## Budget and cancellation ordering

Define one consistent order:

1. Registry lookup.
2. Profile check.
3. Audience/exposure check.
4. Schema validation.
5. Serialized input-budget check.
6. Cancellation-before-execution check.
7. Handler execution.
8. Output truncation and limits metadata.

Document any deliberate exception.

# Workstream 5 — Audience-consistent registry inspection

## Current inconsistency

`available_tools_for_current_audience` is policy-aware, but `get_tool` and `has_tool` check profile membership without applying the registry audience. A model-facing registry may therefore discover metadata for a harness-only tool that it cannot execute.

## Required changes

Change default semantics:

- `get_tool(name)` returns metadata only when the tool is available for the registry's current profile and audience.
- `has_tool(name)` returns true only when the tool is executable under current profile and audience, before argument validation.

Add explicit administrative methods:

- `get_tool_unfiltered(name)`;
- `has_registered_tool(name)`;
- `all_registered_tools_unfiltered()` if needed.

Names must clearly indicate policy bypass.

## Invariant tests

For every built-in profile and each audience:

1. Every listed tool passes `has_tool`.
2. Every listed tool returns `Some` from `get_tool`.
3. Every tool accepted by `has_tool` appears in the corresponding listing.
4. Hidden tools never appear or execute.
5. Harness-only tools are absent for Model and present only where profile membership permits for Harness/Debug.
6. Expert/contextual exposure behavior matches current policy definitions.
7. Unfiltered methods can inspect registered hidden tools only where intentionally allowed by the internal API.

# Workstream 6 — Stateful calculator semantics

## Persistent operations

Add integration tests for sequences such as:

- seed random generator, draw twice, and verify state advances;
- store and recall a memory register across calls;
- set, get, delete, and list user variables across calls;
- two mutable contexts with identical seeds progress independently;
- immutable template calls repeat the first result instead of advancing;
- MCP template rejects random and side-effect operations without modifying caller state.

## Transaction behavior

Decide whether failed stateful expressions can partially mutate context. Preferred rule: no externally visible mutation on parse or evaluation failure.

Implementation option:

1. Clone `EvalContext` before a stateful call.
2. Execute against the clone.
3. Commit the clone back only when the calculator operation succeeds.

Measure the cost; the maps are expected to be bounded. If partial mutation is preserved for compatibility, document and test it explicitly.

## Capacity limits

Ensure mutable contexts retain user-variable and memory limits. Add tests for limit exhaustion and cleanup. No stateful API should permit unbounded map growth.

# Workstream 7 — Mixed-surface isolation tests

Add scenarios in `tests/test_context_isolation.rs`:

### MCP versus library

- Call legacy/library random function before MCP dispatch.
- Perform an MCP math call.
- Verify library random/side-effect permission was not globally disabled.

### Immutable versus mutable

- Two immutable calls with the same template yield reproducible first-state behavior.
- Two mutable calls advance state.
- Immutable calls do not modify the source context.

### Profile and audience

- A Model registry cannot inspect or execute harness-only tools through default methods.
- A Harness registry can inspect and execute them when included in the selected profile.
- Context overrides use the same policy as registry defaults.

### Panic and thread reuse

- A panicking handler does not leak cancellation, eval context, profile, audience, or compatibility state to the next call.

### Concurrent state

- Multiple mutable contexts on separate threads remain independent.
- Shared mutable context requires caller synchronization; document `Send`/`Sync` behavior rather than hiding it.

# Workstream 8 — Public API and migration handling

## Deprecations

Potential deprecations:

- `set_mcp_mode()` for new code;
- ambiguous `call_json_with_execution_context` if renamed;
- `with_eval_context(&mut EvalContext)` signature;
- legacy audience-insensitive expectations for `get_tool` and `has_tool`.

Use Rust deprecation notes with replacement methods and a planned removal horizon. Record all changes in `CHANGELOG.md`.

## Semver review

Before implementation, identify whether changing `get_tool`/`has_tool` behavior is acceptable in a minor release. It changes semantics but corrects an authorization-policy inconsistency. If compatibility risk is high:

1. Add new policy-aware methods first.
2. Deprecate old methods.
3. Switch defaults in the next major release.

The plan executor should document the chosen route in a release decision record.

## Documentation

Update:

- `architecture/agent-api.md` with template versus mutable context diagrams;
- `architecture/calculator.md` with legacy/global and context-aware state boundaries;
- `architecture/compatibility.md` with global-state deprecation policy;
- `architecture/mcp-server.md` with MCP template construction;
- README library examples to favor context-aware APIs;
- docs.rs examples for persistent and deterministic sessions.

# Validation sequence

Focused:

```bash
cargo test --all-features --test test_context_isolation -- --nocapture
cargo test --all-features --test lib mcp::test_hardening_and_gaps -- --nocapture
cargo test --all-features --lib calc:: -- --nocapture
cargo test --all-features --lib agent:: -- --nocapture
```

Full:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Run parity tests where available. Changes to global-state behavior may intentionally differ from the Python reference on the in-process Rust surface, but MCP parity should remain stable unless explicitly revised.

# Acceptance criteria

Release 3 is complete only when:

- MCP dispatch performs no process-global calculator-mode mutation;
- immutable template and mutable persistent context semantics are explicit and separately tested;
- stateful calculator operations persist only through the mutable API;
- failed stateful calls follow a documented transaction rule;
- thread-local bridges restore prior state after nesting and panic;
- all dispatch surfaces use one profile/audience/schema policy preparation path;
- default metadata lookup agrees with execution eligibility, or a documented semver-safe migration is in place;
- mixed MCP/library calls do not contaminate one another;
- full verification and documentation generation pass.

# Handoff notes

Begin with tests that demonstrate the global MCP-mode leak and metadata-policy disagreement. Next introduce the shared policy preparation function without changing behavior. Then migrate MCP dispatch to explicit execution templates and remove global-mode activation. Add the mutable API after the immutable path is stable. Finish with audience-aware metadata semantics and deprecations, because those require the clearest semver decision.