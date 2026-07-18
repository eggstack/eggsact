# Releases 1–3 Final Correctness Closure Plan

## Purpose

This is a narrowly scoped corrective pass for the remaining defects found after the first Releases 1–3 closure implementation. It is not a new roadmap phase. Its only purpose is to close the residual runtime, lifecycle, API-contract, test-evidence, and release-documentation gaps before Release 4 verification infrastructure is treated as authoritative.

Release 4 and Release 5 work may be prepared in parallel, but their final acceptance gates must run against a commit that satisfies this plan.

## Current state

The following earlier objectives are already substantially complete:

- MCP blocking work is executed through a bounded `spawn_blocking` path.
- The owned MCP worker permit remains inside the blocking closure until work exits.
- Tool-local detached timeout workers were removed from `math_eval`, regex handlers, and dotenv validation.
- Cancellation notifications bypass ordinary request rate limits and do not receive responses.
- Request-ID registration is atomic for capacity and duplicate checks.
- MCP initialization, lifecycle enforcement, protocol negotiation, and extension advertisement are implemented.
- Client capabilities are retained in `NegotiatedProtocol`.
- MCP dispatch no longer mutates process-global calculator mode.
- Audience-aware lookup and shared dispatch policy are implemented.
- The unsupported mutable-context persistence claim was withdrawn and the method was deprecated.

The remaining correctness gaps are:

1. The timeout lifecycle metric can leak for requests that time out while queued for a semaphore permit.
2. The timeout lifecycle metric can transiently underflow when handler completion races the timeout increment.
3. Removing handler-local timeout wrappers weakened timeout protection for synchronous in-process callers.
4. `RequestGuard::drop` uses `try_lock`, so completed active-request entries can remain permanently registered.
5. Request-form `notifications/initialized` messages can be consumed without any JSON-RPC response.
6. The required controlled race, queued-timeout, cleanup, and capability-retention tests were not added.
7. Package version, changelog release sections, deprecation metadata, and architecture documentation are inconsistent.

## Goals

1. Make timeout accounting exact for queued, running, timed-out, completed, cancelled, and panicked operations.
2. Preserve bounded timeout behavior for both MCP and synchronous in-process execution surfaces.
3. Guarantee active-request cleanup rather than treating cleanup as best effort.
4. Give request-form lifecycle misuse a deterministic response while keeping true notifications response-free.
5. Add deterministic regression tests for every remaining race and lifecycle edge.
6. Reconcile release and API documentation with the actual package version and published state.
7. Leave explicit evidence that Releases 1–3 are closed.

## Non-goals

This pass must not:

- Add new tools.
- Add new MCP protocol revisions.
- Add sampling, roots, elicitation, or server-initiated protocol features.
- Begin MSRV, cross-platform, cargo-deny, fuzzing, benchmarks, feature decomposition, or crate splitting.
- Replace Tokio or introduce a second asynchronous runtime.
- Restore unbounded detached workers.
- Weaken input, output, regex-safety, cancellation, or profile/audience limits.
- Implement persistent calculator state through generic tool dispatch.

---

# Workstream 1 — Correct timeout lifecycle accounting

## Problem A: queued timeout leak

The response timeout currently includes time spent waiting for the worker semaphore. The handler lifecycle begins in a generic `RUNNING` state before a permit is acquired. If the request times out while still queued, the timeout path can increment `timed_out_handlers`, but no blocking closure exists to perform the matching decrement.

`timed_out_handlers` must only count underlying work that is still running after a timeout response has been returned. A request that times out before work begins is not a timed-out handler.

## Problem B: timeout/finish underflow

The timeout path currently exposes `TIMED_OUT` before incrementing the gauge. A finishing handler may observe that state and decrement the unsigned counter before the timeout path increments it. The final value may recover, but transient underflow violates metric correctness and can produce nonsensical snapshots.

## Required state model

Use an explicit lifecycle that distinguishes queueing from execution. The exact representation may differ, but it must encode at least:

```rust
const HANDLER_QUEUED: u8 = 0;
const HANDLER_RUNNING: u8 = 1;
const HANDLER_TIMED_OUT_RUNNING: u8 = 2;
const HANDLER_FINISHED: u8 = 3;
const HANDLER_TIMED_OUT_QUEUED: u8 = 4;
```

A cleaner implementation may use a small internal enum plus atomic encoding.

## Required transitions

### Before semaphore acquisition

- Initial state is `QUEUED`.
- No blocking-handler metric is incremented.
- If the response deadline fires while queued, atomically transition `QUEUED -> TIMED_OUT_QUEUED`.
- Increment `total_timeouts` because a timeout response was returned.
- Do not increment `timed_out_handlers`.
- Ensure the queued future is cancelled and cannot later acquire a permit and start work.

### After semaphore acquisition, before `spawn_blocking`

- Transition `QUEUED -> RUNNING` before work can become visible to the timeout path.
- If the request has already been cancelled or timed out, release the permit and do not spawn.
- Entering the blocking closure increments `active_blocking_handlers` through the existing RAII metric guard.

### Timeout while running

The transition and metric update must be ordered so the handler cannot decrement an increment that has not yet occurred.

Acceptable implementation patterns include:

#### Pattern A — metric-owned timeout token

- Create a small RAII `TimedOutHandlerGuard` only after the timeout path proves the handler is running.
- Increment `timed_out_handlers` inside guard construction.
- Publish the guard or an accounted state atomically to the handler-exit path.
- Exactly one owner drops the guard when the handler exits.

#### Pattern B — separate accounted state

Use states such as:

```rust
RUNNING
TIMEOUT_ACCOUNTING
TIMED_OUT_ACCOUNTED
FINISHED
```

The timeout path reserves the transition, increments the gauge, then publishes `TIMED_OUT_ACCOUNTED`. The handler-exit path either:

- transitions `RUNNING -> FINISHED`; or
- waits/spins only across the tiny `TIMEOUT_ACCOUNTING` critical transition; then transitions `TIMED_OUT_ACCOUNTED -> FINISHED` and decrements.

Do not use an ordering where `TIMED_OUT` is visible before the gauge increment.

### Handler completion

- Completion before timeout transitions `RUNNING -> FINISHED` and never touches `timed_out_handlers`.
- Completion after an accounted timeout decrements exactly once.
- Panic, cancellation, and normal return use the same completion guard.
- No path can decrement from zero.

## Metric semantics

Document and test these exact meanings:

- `active_requests`: request tasks currently registered for cancellation and duplicate-ID tracking.
- `active_blocking_handlers`: blocking closures that have actually begun executing.
- `timed_out_handlers`: blocking closures still executing after the server returned a timeout response.
- `total_timeouts`: cumulative timeout responses returned, including queue-wait timeouts.
- `peak_blocking_concurrency`: maximum simultaneous executing blocking closures, not queued requests.

## Required tests

Add unit tests for every state transition and integration tests using controlled barriers/channels rather than timing guesses.

Required cases:

1. Timeout while queued behind a saturated semaphore:
   - returns a timeout response;
   - never starts the queued handler;
   - increments `total_timeouts`;
   - never increments or leaks `timed_out_handlers`.
2. Handler completes before timeout.
3. Timeout occurs while handler is running, then handler completes.
4. Handler completion races timeout reservation before accounting.
5. Handler completion races after timeout accounting.
6. Handler panics after timeout.
7. Cancellation causes exit after timeout.
8. Hundreds of controlled race iterations leave all gauges at zero.
9. `timed_out_handlers` never exceeds `active_blocking_handlers` in a stable snapshot after transition synchronization.
10. No unsigned underflow is observable.

## Acceptance criteria

- Queued timeouts do not create timed-out-handler gauge entries.
- Every `timed_out_handlers` increment has exactly one matching decrement.
- No decrement can occur before its corresponding increment.
- All gauges return to zero after controlled workers terminate.
- Metric definitions in code, diagnostics, and architecture docs match implementation.

---

# Workstream 2 — Restore bounded synchronous in-process execution

## Problem

The raw handlers now run synchronously and rely on the outer MCP Tokio timeout for client-facing timeout behavior. `ToolRegistry::call_json` and related in-process entry points do not have that outer runtime boundary. Removing `run_with_timeout` therefore removed bounded timeout behavior for direct consumers of calculator, regex, and config-validation tools.

The in-process API is a first-class roadmap constraint. MCP hardening must not make direct embedding less safe.

## Required design

Create a single bounded synchronous execution boundary in the registry/orchestration layer. Do not put detached timeout workers back inside individual handlers.

### Preferred architecture

Add an internal bounded executor used by synchronous `ToolRegistry` entry points for tools that can perform non-trivial CPU-bound or backtracking work.

Suggested shape:

```rust
struct SyncExecutionPool {
    permits: ...,
}

fn execute_bounded_sync(
    tool: &str,
    budget: ToolBudget,
    cancel: Option<Arc<AtomicBool>>,
    operation: impl FnOnce() -> ToolResponse + Send + 'static,
) -> ToolResponse;
```

Requirements:

- The worker acquires an owned permit before starting.
- The permit is moved into the worker and remains held until the worker exits.
- A caller-facing timeout may return before the worker exits, but repeated timeouts cannot exceed the configured worker bound.
- The worker inherits the cancellation flag and evaluator context snapshot required by the selected API.
- Queue saturation behavior is deterministic and returns a structured `TIMEOUT` or `RESOURCE_EXHAUSTED` response.
- The executor is process-wide or registry-owned with an explicit maximum; never create a new pool per call.
- The MCP server must not nest this executor inside its already bounded blocking closure.

### Avoiding nested execution under MCP

Introduce an internal execution-boundary marker or explicit dispatch mode:

```rust
enum ExecutionBoundary {
    DirectSynchronous,
    ExternallyBounded,
}
```

The MCP path invokes the prepared handler with `ExternallyBounded`, meaning the handler runs directly inside the existing semaphore-owned `spawn_blocking` closure.

The synchronous registry path invokes with `DirectSynchronous`, meaning designated high-risk work is submitted through the bounded synchronous executor.

Do not use environment variables or process-global mutable booleans to select the boundary. A panic-safe thread-local guard is acceptable if changing the handler signature is disproportionate, but an explicit internal argument is preferable.

## Tool classification

Do not scatter name checks across call sites. Add one registry-level execution classification, for example:

```rust
enum ExecutionClass {
    Inline,
    BoundedCpu,
}
```

Initial `BoundedCpu` candidates must include at least:

- `math_eval`
- `validate_regex`
- `regex_finditer`
- `dotenv_validate` when a caller-provided regex is used

Audit other parser-heavy or quadratic tools and document why they remain inline or are classified as bounded CPU work.

## Timeout semantics

Define and document:

- Queue timeout.
- Execution-response timeout.
- Underlying worker lifetime after response timeout.
- Permit lifetime.
- Cancellation behavior.
- Whether a direct call can return `TIMEOUT` while bounded work continues.

The direct API must not claim that a timed-out thread was killed.

## Raw handler visibility

Audit whether direct `src::tools::*` handlers are part of the intended stable public API.

- If they are internal implementation details, reduce visibility where semver permits and direct consumers to `ToolRegistry` or typed wrappers.
- If they must remain public, document that raw handlers execute synchronously without orchestration guarantees and provide a safe bounded public entry point.
- Do not leave users believing raw handler calls have the same timeout guarantees as registry dispatch.

## Required tests

1. Direct `ToolRegistry::call_json` timeout returns within the configured response bound.
2. Timed-out direct workers retain their permits until exit.
3. Repeated direct timeouts never exceed the configured direct-worker bound.
4. MCP calls do not consume both MCP and direct-executor permits.
5. Cancellation propagates to direct workers where handlers support cooperative checks.
6. Safe/fast inline tools remain synchronous and do not pay worker-thread overhead.
7. A direct worker panic becomes a structured internal error and releases its permit.
8. Separate registry instances cannot bypass a process-wide worker bound unless the chosen design explicitly documents per-registry bounds.

## Acceptance criteria

- Direct in-process execution again has documented bounded timeout behavior.
- MCP dispatch has exactly one blocking concurrency boundary.
- No execution surface can create unbounded abandonable CPU workers.
- Worker classification is centralized and test-covered.

---

# Workstream 3 — Guarantee active-request cleanup

## Problem

`RequestGuard::drop` currently uses `try_lock` on a Tokio mutex. If the active-request map is contended, cleanup is skipped. No later operation purges the stale entry. This can permanently reserve request IDs and eventually exhaust the in-flight capacity.

Cleanup is part of correctness, not best-effort diagnostics.

## Preferred solution

Replace the active-request map’s Tokio mutex with `std::sync::Mutex` or another synchronous mutex already available in the dependency set.

Rationale:

- Critical sections are tiny: check length, check key, insert, clone a cancel flag, remove.
- No lock is held across `.await`.
- Synchronous locking allows `RequestGuard::drop` to remove deterministically.
- Cancellation can clone the flag under the lock, release the lock, then set the atomic flag outside the critical section.

Requirements:

- Handle poisoned locks by recovering the inner map where appropriate; do not silently skip cleanup.
- Preserve atomic capacity/duplicate registration.
- Preserve generation matching so an old guard cannot remove a newer request that reused the same ID.
- Update `register_request` and `apply_cancellation` signatures consistently.
- Keep the hidden/test-only API status explicit.

## Acceptable alternative

Keep the Tokio mutex but perform explicit awaited removal in every task terminal path and use a separate fallback mechanism that cannot silently lose cleanup. This alternative must prove cleanup on panic, cancellation, serialization failure, writer-channel failure, and shutdown.

A `Drop` implementation that merely spawns an untracked asynchronous cleanup task is not sufficient unless shutdown drains those tasks.

## Required tests

1. Lock contention during request completion cannot leave an entry behind.
2. Request IDs are reusable after completion.
3. A panicking handler removes its active entry.
4. A timed-out response keeps the entry active only while underlying work remains cancellable, then removes it.
5. Writer-channel closure does not leak registration.
6. Repeated completion under contention cannot fill `MAX_IN_FLIGHT_REQUESTS` with stale entries.
7. Generation matching prevents stale cleanup from removing a newer request with the same ID.
8. Cancellation still sets the flag outside the map lock.

## Acceptance criteria

- Active-request cleanup is guaranteed on all terminal paths.
- No comment or documentation describes cleanup as best effort.
- Long-running stress tests can reuse IDs and never accumulate stale entries.

---

# Workstream 4 — Correct request-form lifecycle misuse

## Problem

A true `notifications/initialized` notification has no ID and must not receive a response. A malformed request-form message using the same method with an ID is still a JSON-RPC request and must receive a deterministic response. The current dispatcher can consume it and return `None`, leaving the caller waiting indefinitely.

## Required behavior

Classify absent-ID notifications before request registration, as today.

For `notifications/initialized`:

- Absent ID:
  - perform the valid state transition if awaiting initialization;
  - otherwise ignore or log according to the documented policy;
  - never send a response.
- Present non-null ID:
  - treat as invalid request or method-not-found according to one documented policy;
  - return a JSON-RPC error with the same ID;
  - do not perform the lifecycle transition.
- Null ID:
  - continue following the repository’s explicit null-ID rejection policy.

Apply the same audit to `notifications/cancelled` and any future notification-only methods. A request-form notification method must not disappear without a response.

## Required tests

1. Valid initialized notification transitions to `Ready` and produces no response.
2. Initialized notification before initialize produces no response and no transition.
3. Initialized notification after `Ready` produces no response and no duplicate transition.
4. Request-form initialized with an integer ID returns a deterministic error.
5. Request-form initialized with a string ID returns a deterministic error.
6. Request-form initialized does not transition state.
7. Request-form cancellation method returns a deterministic error and does not cancel the target.

## Acceptance criteria

- Every request with a valid non-null ID receives a result or error.
- Notification-only methods remain response-free only when the ID is absent.
- Lifecycle documentation distinguishes request form from notification form.

---

# Workstream 5 — Complete missing regression evidence

## Required test organization

Prefer adding focused modules rather than further expanding one very large integration file:

- `tests/mcp/test_timeout_lifecycle.rs`
- `tests/mcp/test_request_cleanup.rs`
- `tests/mcp/test_notification_request_forms.rs`
- `tests/test_bounded_in_process_execution.rs`

Use test-only hooks in `src/mcp/runtime.rs` or a dedicated test-support module for deterministic synchronization.

## Determinism requirements

- Do not depend on `sleep` alone to create races.
- Use barriers, channels, latches, or injected handlers.
- Keep production behavior unchanged when test hooks are disabled.
- Avoid globally mutating metrics without serializing tests or resetting counters safely.
- Tests that manipulate global worker pools or metrics must use a process-level test lock or run in isolated subprocesses.

## Capability-retention tests

Add non-default capability values and verify they survive both lifecycle transitions:

- roots object
- sampling object
- elicitation object
- experimental object

Also verify:

- duplicate initialize does not overwrite retained capabilities;
- unsupported requested protocol fallback retains the accepted request’s capabilities;
- malformed known capability shapes follow the typed parsing policy;
- unknown permitted fields remain forward-compatible according to the documented serde behavior.

## Acceptance criteria

- Every defect named in this plan has a failing-before/passing-after regression test.
- Race tests exercise controlled interleavings rather than merely repeating wall-clock timing.
- Capability retention is asserted with actual values, not only `Default` construction.

---

# Workstream 6 — Reconcile versions, changelog, rustdoc, and architecture docs

## Version audit

Before editing release history, determine which versions are actually published on crates.io and which tags exist remotely.

Record:

- current `Cargo.toml` version;
- latest crates.io version;
- existing release tags;
- whether changelog sections `1.1.5`, `1.2.0`, and `1.3.0` correspond to published artifacts;
- intended next release version.

Do not rewrite published history inaccurately.

## Required corrections

### Deprecation metadata

`#[deprecated(since = "0.4.0")]` is inconsistent with the current 1.x package line. Set `since` to the real release in which the deprecation will ship, or omit `since` until that version is selected.

### Changelog

- Keep unpublished fixes under `[Unreleased]`.
- Do not mark a version as released merely because implementation landed on `main`.
- Preserve corrections to previously published release claims in a new entry rather than rewriting immutable history, when applicable.
- Explicitly describe direct in-process timeout semantics after Workstream 2.

### Architecture documentation

Remove stale references to:

- `SpawnSemaphore`
- `SpawnPermit`
- `MAX_CONCURRENT_SPAWNED`
- `SPAWN_ACQUIRE_TIMEOUT`
- handler-local `REGEX_TIMEOUT_SECONDS`

Update:

- `architecture/tools.md`
- `architecture/mcp-server.md`
- `architecture/agent-api.md`
- `architecture/testing.md`
- `docs/library-api.md`
- `README.md`
- `AGENTS.md`
- relevant skill files

### API contract

State clearly:

- immutable execution-context calls clone evaluator state;
- deprecated mutable generic tool dispatch does not provide persistent calculator sessions;
- persistent calculator sessions use `evaluate_with_context` / `run_with_context`;
- direct registry execution uses the bounded synchronous execution policy;
- raw handler calls, if still public, have explicitly documented guarantees.

## Acceptance criteria

- Package version, changelog, tags, and deprecation metadata tell one coherent story.
- Generated docs contain no removed helper names or constants.
- `cargo run --bin generate-docs -- --check` passes.
- No current documentation claims timeout or persistence behavior the code does not implement.

---

# Suggested implementation sequence

1. Add deterministic timeout-lifecycle test hooks.
2. Implement queued/running/accounted timeout states.
3. Add transition and gauge tests.
4. Replace best-effort request cleanup with guaranteed removal.
5. Add request-cleanup contention tests.
6. Introduce centralized execution classification.
7. Implement bounded synchronous in-process execution.
8. Confirm MCP bypasses nested direct-executor submission.
9. Correct request-form lifecycle notification handling.
10. Add real capability-retention value tests.
11. Audit crates.io versions and tags.
12. Reconcile changelog, deprecation metadata, and architecture docs.
13. Run focused and full gates.

Recommended commit structure:

1. `fix(runtime): make timeout accounting queue-aware and race-free`
2. `fix(runtime): guarantee active request cleanup`
3. `fix(agent): restore bounded synchronous tool execution`
4. `fix(mcp): reject request-form notification methods`
5. `test: add releases 1-3 final correctness regressions`
6. `docs: reconcile runtime contracts and release metadata`

---

# Verification commands

Run at minimum:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
cargo publish --dry-run
```

Focused tests should include filters or dedicated test binaries for:

```bash
cargo test --all-features timeout_lifecycle
cargo test --all-features request_cleanup
cargo test --all-features notification_request_forms
cargo test --all-features bounded_in_process
cargo test --all-features client_capabilities
```

Where the Python reference package is available, run the parity suite and confirm no new accepted failures were added solely to make the suite pass.

---

# Final closure criteria

Releases 1–3 are closed only when all of the following are true:

## Runtime

- Queue-wait timeouts cannot leak `timed_out_handlers`.
- Timeout/finish races cannot underflow or leak metrics.
- All runtime gauges return to zero after controlled work ends.
- MCP and direct execution each have one explicit bounded worker model.
- No abandonable worker escapes its permit lifetime.

## Request tracking

- Active-request cleanup is guaranteed under contention, panic, timeout, cancellation, writer failure, and shutdown.
- Completed IDs are reusable.
- Stale entries cannot exhaust in-flight capacity.

## Protocol

- Every non-null-ID request receives a response.
- True notifications remain response-free.
- Client capability values survive the session lifecycle.

## API and documentation

- Direct in-process timeout behavior is bounded and documented.
- Mutable calculator-state guidance is accurate.
- Package version, changelog, tags, and deprecation metadata are coherent.
- No stale removed-worker documentation remains.

## Evidence

- Focused regression tests pass.
- Full local gate passes.
- Package and publish dry-runs pass.
- Current GitHub CI is green, or unavailable external evidence is recorded without claiming success.
- A concise closure note records the state model, worker model, cleanup model, version audit, and commands executed.

Only after this gate is satisfied should Release 4 be marked complete or Release 5 fuzz findings be triaged against the runtime as a stable baseline.
