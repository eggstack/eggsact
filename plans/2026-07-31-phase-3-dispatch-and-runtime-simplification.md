# Phase 3 — Dispatch and Runtime Simplification

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-07-31-lightweight-correctness-simplification-roadmap.md`
- **Depends on:** regex/MCP contract repair and deterministic-output/TOML correction phases
- **Scope:** consolidate duplicated in-process dispatch, make calculator context semantics explicit, and remove runtime machinery that exists only to support deprecated or test-only behavior
- **Expected change size:** medium; primarily deletion and delegation

## Objective

Reduce framework overhead around the existing tools without reducing tool coverage or weakening the MCP server's bounded-execution behavior.

The current in-process API has several overlapping dispatch methods. They repeat lookup, profile/audience policy, schema validation, budget resolution, input-size checks, worker submission, context installation, cancellation, and output truncation. The deprecated mutable-context path additionally maintains commit-slot machinery even though ordinary `math_eval` does not persist calculator mutations through generic dispatch.

This phase establishes:

1. one shared pre-execution policy path;
2. one shared in-process execution implementation with thin compatibility wrappers;
3. explicit stateless tool-call semantics for calculator-backed tools;
4. direct calculator APIs as the supported persistent-state surface;
5. test-only execution hooks compiled only for tests;
6. no reduction in MCP concurrency, cancellation, timeout, panic, or output-limit behavior.

---

# Hard constraints

This phase must not:

- add or remove tools;
- replace Tokio;
- replace stdio MCP transport;
- merge the synchronous and asynchronous executors into a new runtime framework;
- add an executor trait or generic async abstraction;
- add a new thread-pool dependency;
- change MCP protocol versions;
- remove bounded concurrency;
- promise forceful cancellation of blocking Rust code;
- make timing-sensitive tests part of ordinary CI;
- break public API solely to reduce line count;
- create a major-version release plan;
- change release automation;
- expand runtime metrics;
- create a persistent generic tool session.

The desired result is fewer implementations, not a more abstract implementation.

---

# Files to inspect first

At minimum inspect:

```text
src/agent/mod.rs
src/preflight/mod.rs
src/mcp/server.rs
src/mcp/execution.rs
src/mcp/sync_pool.rs
src/mcp/budget.rs
src/mcp/runtime.rs
src/mcp/response.rs
src/tools/math.rs
src/calc/context.rs
src/calc/evaluator.rs
src/calc/mod.rs
architecture/agent-api.md
architecture/budget-concurrency.md
architecture/calculator.md
architecture/coding-agent-integration.md
tests/
```

Search for:

```text
call_json
call_json_with_budget
call_json_with_context
call_json_with_execution_context
call_json_with_execution_template
call_json_with_execution_context_mut
prepare_tool_call
prepare_tool_call_with_policy
execute_handler_with_commit_slot
with_eval_context
current_eval_context
EvalContext::mcp_mode
EvalContext::new
SyncExecutionPool
ExecutionHooks
HandlerLifecycle
```

Before editing, create a temporary call-path table:

| Public method | Policy source | Executor | Eval context | Persistence | Output limits |
|---|---|---|---|---|---|
| `call_json` | registry defaults | caller thread | currently implicit/global fallback | ambiguous | limited |
| `call_json_with_budget` | registry defaults | sync pool | fresh MCP context | no | yes |
| `call_json_with_context` | registry defaults + cancel | sync pool | fresh MCP context | no | yes |
| `call_json_with_execution_context` | explicit context | sync pool | cloned context | no | yes |
| execution template alias | explicit context | sync pool | cloned context | no | yes |
| deprecated mutable context | explicit context | sync pool + commit slot | cloned/commit attempt | not for `math_eval` | yes |

Confirm actual behavior from code and tests rather than copying this table blindly.

---

# Target architecture

## Shared policy preparation

Retain one internal function that performs:

1. tool lookup;
2. profile membership check;
3. audience/exposure check;
4. schema validation.

Recommended input:

```rust
struct EffectiveCallPolicy<'a> {
    profile: &'a Profile,
    audience: ToolAudience,
    compatibility: CompatibilityMode,
}
```

A dedicated struct is optional. Three explicit arguments are acceptable if clearer.

`prepare_tool_call()` may remain as the public/default-policy wrapper. `prepare_tool_call_with_policy()` should contain the actual implementation or delegate to one shared private helper. Do not maintain two copies of the same checks.

## Shared in-process execution

Retain one private synchronous implementation that performs:

1. shared policy preparation;
2. effective budget resolution;
3. serialized input-size enforcement;
4. cancellation selection;
5. fresh or cloned eval-context selection;
6. sync-pool submission when bounded execution is requested;
7. handler execution with panic conversion;
8. output truncation and limits metadata.

Recommended conceptual entry point:

```rust
fn call_json_inner(
    &self,
    name: &str,
    args: Value,
    options: EffectiveCallOptions,
) -> Result<ToolResponse, ToolCallError>
```

Do not expose `EffectiveCallOptions` publicly unless it clearly improves the stable API. Existing `ExecutionContext` may supply the options.

## Two execution modes are sufficient

The implementation may retain:

- **direct mode:** execute synchronously on the calling thread for the simplest API;
- **bounded mode:** execute through the existing sync pool with budget/cancellation.

Do not create separate implementations for budget-only, cancellation-only, context, and template calls. Those are option combinations.

The MCP server may retain its Tokio `spawn_blocking` adapter because it has an asynchronous stdio/concurrency boundary. Share policy preparation and response finalization where practical, but do not force MCP through the synchronous pool.

---

# Workstream 1 — Consolidate policy preparation

## Required implementation

Refactor the registry policy checks so that default-policy and explicit-policy calls share one body.

Required ordering remains:

```text
lookup
profile
exposure/audience
schema validation
```

Preserve current error variants and messages unless correcting a known inconsistency.

The MCP server must continue using the shared policy path rather than reimplementing profile/audience/schema checks.

## Tests

Retain or add focused table-driven tests for:

- unknown tool;
- tool absent from profile;
- harness-only tool under model audience;
- valid harness call;
- invalid arguments;
- context override taking precedence over registry defaults;
- compatibility mode affecting only documented validation/message behavior.

Do not duplicate these tests for every public wrapper. Test the shared helper plus one wrapper delegation test per policy source.

## Acceptance criteria

- One implementation owns lookup/profile/audience/schema ordering.
- MCP and in-process dispatch use it.
- Error behavior remains compatible.

---

# Workstream 2 — Consolidate in-process call methods

## Required compatibility strategy

Do not remove public methods in a minor release merely to simplify internals.

Keep existing public methods as thin wrappers where necessary:

```rust
call_json(...)
call_json_with_budget(...)
call_json_with_context(...)
call_json_with_execution_context(...)
call_json_with_execution_template(...)
call_json_with_execution_context_mut(...)
```

After this phase, each wrapper should primarily construct effective options and delegate. It must not repeat the execution sequence.

If a cleaner public pair is already present or can be added without churn, prefer:

```rust
call_json(name, args)
call_json_with_execution_context(name, args, ctx)
```

A new `call_with` alias is optional, not required. Do not add API surface just to match the roadmap's example.

## Direct-call behavior

`call_json()` should remain the low-overhead direct path unless existing documentation promises budget enforcement.

It must still:

- use shared policy preparation;
- install an explicit fresh evaluation context for calculator-backed handlers;
- convert handler panic consistently if current public behavior expects a response rather than unwind;
- avoid legacy process-global calculator state.

If panic conversion on direct calls would be a behavioral change, document and test the chosen behavior. Prefer a deterministic error response over process unwind for agent-facing utility calls.

## Bounded-call behavior

All bounded wrappers use the same sync-pool submission helper and same input/output budget logic.

Remove duplicated blocks that separately:

- serialize args to count bytes;
- resolve default budgets;
- create cancellation flags;
- install eval context;
- submit to the pool;
- truncate responses;
- translate pool errors.

## Acceptance criteria

- Public wrappers contain no parallel dispatch implementations.
- Direct mode remains low overhead.
- Bounded mode retains timeout, cancellation, queue-full, and output-limit behavior.
- Existing typed preflight wrappers continue to function without changes to their public contracts.

---

# Workstream 3 — Make calculator state semantics explicit

## Current contradiction

Generic execution contexts contain `EvalContext`, but calculator-backed `math_eval` clones the current context before evaluation. Generic mutable-context dispatch maintains commit machinery, yet `math_eval` mutations do not persist through that path. Direct dispatch can fall back to legacy process-global state when no explicit context is installed.

## Required state model

Define two supported modes.

### Stateless tool mode

Used by MCP and generic `ToolRegistry` dispatch.

Properties:

- each call receives a fresh or explicitly cloned `EvalContext`;
- no mutation persists after the call;
- repeated calls with identical inputs/options are reproducible;
- MCP uses `EvalContext::mcp_mode()` and therefore preserves its existing random/side-effect restrictions;
- in-process direct calls use a documented fresh context, preferably `EvalContext::new()` if existing native behavior allows random and expression-local side effects;
- process-global calculator compatibility state is not used by tool dispatch.

### Stateful calculator mode

Used only through calculator-specific APIs:

```rust
evaluate_with_context(expr, &mut ctx)
run_with_context(expr, &mut ctx)
```

These remain the supported persistence surface for PRNG state, memory registers, and variables.

A small `CalculatorSession` wrapper is permitted only if it replaces repeated consumer boilerplate and is implementable as a thin owner of `EvalContext`. It is not required for this phase and must not become a generic tool session.

## `math_eval` correction

Change `math_eval` so it operates on the installed eval context without making a second unnecessary clone inside the handler.

The dispatch layer determines whether the installed context is fresh or cloned. The handler should not silently override that decision.

For stateless dispatch, the dispatch layer owns a local context that is discarded after the handler returns.

For direct calculator APIs, no tool handler is involved and caller mutation persists.

## Deprecated mutable generic context

`call_json_with_execution_context_mut()` is already deprecated and cannot truthfully persist ordinary `math_eval` state under its documented behavior.

Required action:

- remove `execute_handler_with_commit_slot()` and associated `Arc<Mutex<Option<EvalContext>>>` machinery unless another non-test production handler depends on it;
- implement the deprecated method as a thin delegation to stateless context dispatch;
- preserve its return behavior;
- update its documentation to state plainly that it is a compatibility wrapper and does not persist tool state;
- keep the deprecation notice directing persistent calculator users to direct calculator APIs.

Do not retain commit-slot code solely for tests.

## Tests

Required cases:

1. two stateless `math_eval` calls with the same seeded template produce the same first random value when random is allowed;
2. generic tool dispatch does not mutate the caller's context;
3. direct `run_with_context()` does persist PRNG/memory/variable state;
4. MCP-mode `math_eval` continues rejecting random/side-effect functions as documented;
5. direct in-process tool calls no longer share legacy global calculator state;
6. deprecated mutable generic context delegates and remains non-persistent.

Avoid tests that depend on parallel global state.

## Acceptance criteria

- Tool dispatch never implicitly uses process-global calculator state.
- Stateless and stateful semantics are documented separately.
- `math_eval` does not clone an already-installed context internally.
- Commit-slot machinery is removed when no longer needed.
- Direct context APIs preserve state correctly.

---

# Workstream 4 — Reduce production-compiled test hooks

## Current issue

The asynchronous bounded execution module includes gate and signal types that are present in production builds primarily so tests can pause exact lifecycle boundaries.

## Required result

Compile deterministic execution gates, slot arrays, test handlers, and hook-heavy entry points only under `#[cfg(test)]` wherever possible.

Preferred approach:

- keep the production `execute_tool_bounded()` path direct and readable;
- keep test-only helper types and gated execution variants in a `#[cfg(test)]` module or conditionally compiled functions;
- share only the minimal lifecycle operations needed to test production logic;
- do not introduce a generic hook trait or dynamic callback framework.

If one small no-op hook field must remain to avoid duplicating the production body, document why and confirm it compiles away. The default expectation is that test gates do not appear in release code.

## Test preservation

Retain high-value tests proving:

- queued timeout does not execute the handler;
- running timeout sets cancellation;
- panic becomes an error response;
- semaphore limits blocking concurrency;
- metrics do not underflow;
- late completion after timeout does not corrupt a later call.

Delete redundant race permutations that test the test harness rather than public behavior. Keep deterministic gates for difficult race boundaries under `cfg(test)`.

## Acceptance criteria

- release builds do not contain test gate types/slot handlers.
- production bounded execution remains understandable without reading test scaffolding.
- retained concurrency tests are deterministic and focused.

---

# Workstream 5 — Evaluate lifecycle/metric simplification conservatively

This workstream is secondary. Do not force it if it risks correctness.

## Inspection question

Determine whether the mutex-protected five-phase `HandlerLifecycle` exists primarily to maintain exact values for:

```text
active_blocking_handlers
timed_out_handlers
total_timeouts
peak_blocking_concurrency
```

Inspect which metrics are public through diagnostics and which are internal tests only.

## Allowed simplification

If the same documented metric contract can be preserved with a smaller design, use:

- an RAII active-handler guard inside `spawn_blocking`;
- one total-timeout increment in the async timeout branch;
- a small atomic/flag to prevent double decrement;
- cooperative cancellation through the existing flag.

Do not sacrifice gauge correctness merely to remove an enum if diagnostics publicly depend on it.

## Acceptable outcome

Retain `HandlerLifecycle` if it is the smallest reliable implementation after test hooks and duplicate dispatch code are removed.

The phase succeeds without deleting this state machine. The required simplification is duplicated API/runtime plumbing, not a predetermined line-count target.

## Acceptance criteria

- any lifecycle change has focused deterministic tests;
- metrics remain nonnegative and internally consistent;
- timeout responses remain bounded and truthful;
- no forceful-cancellation claim is added.

---

# Workstream 6 — Documentation cleanup

Update existing documents:

```text
architecture/agent-api.md
architecture/budget-concurrency.md
architecture/calculator.md
architecture/coding-agent-integration.md
architecture/overview.md only if the high-level flow changes
README.md only if public examples are affected
```

Remove descriptions of deleted commit-slot or parallel dispatch internals.

Document:

- which calls are direct versus bounded;
- stateless tool-call semantics;
- direct calculator context persistence;
- cooperative timeout limitations;
- compatibility wrappers/deprecations.

Do not add a new dispatch architecture document.

---

# Execution sequence for a smaller implementation agent

1. confirm phases 1 and 2 are complete and tests pass;
2. inventory all `ToolRegistry` call methods and their behavior;
3. consolidate policy preparation first;
4. add one private in-process call implementation;
5. migrate one wrapper at a time, running focused tests after each;
6. install explicit eval contexts for direct and bounded tool calls;
7. remove the internal clone from `math_eval`;
8. verify stateless versus stateful calculator tests;
9. replace deprecated mutable-context implementation with a thin wrapper;
10. delete commit-slot machinery and tests that only validate it;
11. move execution gates/hooks under `cfg(test)`;
12. evaluate, but do not force, lifecycle metric simplification;
13. update existing architecture docs;
14. run full verification;
15. commit once and fill the completion record.

Do not combine this phase with Tokio feature or binary-size changes. That belongs to phase 4 and requires a stable behavioral baseline.

---

# Required verification

## Focused

Run existing tests covering:

```text
agent registry policy and profiles
execution context overrides
sync pool timeout/cancellation/queue full
MCP bounded execution
calculator context persistence
math_eval MCP restrictions
typed preflight wrappers
runtime diagnostics metrics
```

## Full

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

No new workflow is required.

---

# Acceptance checklist

- [x] Lookup/profile/audience/schema policy has one implementation.
- [x] MCP and in-process dispatch share policy preparation.
- [x] In-process bounded wrappers share one execution implementation.
- [x] Direct dispatch remains low overhead.
- [x] Public compatibility methods are thin wrappers.
- [x] Generic tool dispatch installs explicit calculator context.
- [x] Generic tool dispatch does not use legacy global calculator state.
- [x] `math_eval` does not clone the installed context internally.
- [x] Stateless tool semantics are reproducible.
- [x] Direct calculator context APIs persist state.
- [x] Deprecated mutable generic context is a thin non-persistent wrapper.
- [x] Commit-slot machinery is removed if no production consumer remains.
- [x] Test gates/hooks are excluded from release builds where practical.
- [x] MCP bounded concurrency remains.
- [x] Cooperative cancellation remains.
- [x] Panic conversion remains.
- [x] Output budget enforcement remains.
- [x] No executor/runtime dependency was added.
- [x] Full verification passes.

---

# Completion record

- **Status:** complete
- **Dispatch consolidation commit:** `63bac39b87596e2f7721c4042f369afe92a41bcd`
- **Calculator/test-hook completion commit:** `021795bc72eee444510ff9f4472e16a611418b6d`
- **Direct-dispatch corrective commit:** `1cb0ce581849b540e41fd8cc5ae130c63c449727`
- **Public wrappers retained:** 6 thin wrappers, all delegating to shared inner dispatch
- **Duplicated implementations removed:** policy preparation consolidated, sync-pool submission consolidated, context installation unified
- **Calculator stateless contract:** tool dispatch uses fresh `EvalContext`; no process-global state
- **Calculator stateful contract:** `evaluate_with_context()`/`run_with_context()` persist PRNG/memory/variables
- **Commit-slot disposition:** removed `execute_handler_with_commit_slot()` and associated `Arc<Mutex<Option<EvalContext>>>` machinery
- **Lifecycle state-machine disposition:** retained `HandlerPhase` (smallest reliable implementation after cleanup)
- **Focused tests:** 534 unit tests, 55 property tests, calculator state isolation tests, dispatch consolidation tests — all pass
- **Full verification:** fmt ✓, clippy ✓, tests ✓ (skip parity), doc ✓, generate-docs --check ✓, package ✓
- **Deferred findings:** none

Record closure here. Do not create an evidence-only plan.