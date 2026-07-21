# Runtime Correctness and Evidence Final Corrective Pass

## Status

- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `16ec8726879d2e62cb49d6a47d17279270b03fb0`
- **Scope:** final corrective implementation pass only
- **Predecessor plans:**
  - `plans/2026-07-18-releases-1-3-final-correctness-plan.md`
  - `plans/2026-07-21-final-runtime-verification-fuzz-closure-plan.md`

## Purpose

This plan closes the remaining correctness defects that are still present after the Release 4 and Release 5 verification/fuzz implementation passes.

The repository now has strong CI, packaging, MSRV, fuzzing, property-test, and documentation infrastructure. However, the current source still does not satisfy four runtime guarantees that the closure evidence claims are complete:

1. timeout accounting does not distinguish queued work from running work;
2. `timed_out_handlers` can be decremented before its matching increment;
3. active-request cleanup is still best-effort because it depends on `try_lock` in `Drop`;
4. budget-aware synchronous registry calls do not enforce elapsed-time or worker-concurrency bounds.

The final evidence document also identifies an older commit while describing tests and changes added by a later commit. This pass must correct the implementation first, then regenerate evidence against one exact final commit.

This is not a new roadmap phase. It is the last narrowly scoped correctness pass required before Releases 1–5 can be marked closed and a release candidate can be prepared.

## Release-blocking findings

### 1. Queued requests are represented as running handlers

`src/mcp/server.rs` creates the handler lifecycle in `HANDLER_RUNNING` before awaiting `Semaphore::acquire_owned()`.

The response timeout includes time spent waiting for that permit. If the deadline expires while the request is queued, the timeout path can transition the lifecycle to `HANDLER_TIMED_OUT` and increment `timed_out_handlers`. No blocking closure was started, so no handler exit path exists to decrement the gauge.

This causes a permanent diagnostics leak and gives `timed_out_handlers` the wrong meaning.

### 2. Timeout accounting can transiently underflow

The timeout path currently:

1. publishes `HANDLER_TIMED_OUT` with `compare_exchange`;
2. increments `timed_out_handlers` afterward.

The handler exit path:

1. swaps the state to `HANDLER_FINISHED`;
2. decrements `timed_out_handlers` when the previous state was `HANDLER_TIMED_OUT`.

A finishing handler can therefore observe the timeout state and decrement before the timeout path has incremented. Because the gauge is an `AtomicUsize`, a diagnostics snapshot can observe a wrapped value.

### 3. Active-request cleanup is not guaranteed

`RequestGuard::drop` uses `try_lock()` on the active-request map. Under contention, cleanup is skipped. The current comment says a later operation or shutdown will remove the entry, but no deterministic stale-entry sweep exists.

A leaked entry can:

- reject later reuse of a completed request ID;
- consume one of the 32 in-flight slots permanently;
- eventually cause all new requests to be rejected;
- leave cancellation directed at a completed request.

Pointer-address matching reduces accidental removal of a replacement entry, but it does not guarantee cleanup and is not a durable generation identity.

### 4. Synchronous budget-aware registry calls are not bounded

`ToolRegistry::call_json_with_budget` checks serialized input size, invokes `call_json`, and truncates the returned response. It does not:

- execute through a worker pool;
- limit simultaneous expensive handlers;
- enforce `max_elapsed_ms`;
- return on timeout;
- retain worker occupancy after a caller-facing timeout;
- reject a saturated queue deterministically.

The same issue affects `call_json_with_context` and execution-context variants. `catch_unwind` in selected handlers catches panics but does not bound elapsed execution.

### 5. Existing race evidence does not exercise the affected path

The current “500-iteration race” test performs sequential `ToolRegistry::call_json("math_eval", ...)` calls. It does not enter MCP request registration, the Tokio semaphore, `spawn_blocking`, timeout lifecycle accounting, or concurrent request handling.

The runtime gauges remain zero because that code path does not modify them. The test is not evidence for the timeout or request-cleanup guarantees.

### 6. Closure evidence is tied to the wrong commit and overstates completion

`docs/releases/2026-07-final-closure-evidence.md` identifies commit `c207c74...`, while describing tests and lifecycle changes added by `16ec872...`.

It marks the four unresolved runtime guarantees complete and calls the current three-state lifecycle queue-aware, which does not match the source.

The evidence must be treated as provisional until this plan is implemented and all required CI/manual workflows have run against the final implementation commit.

---

# Goals

1. Give queued, running, timed-out-running, and finished work distinct lifecycle states.
2. Make timeout gauge accounting exact, underflow-safe, and race-safe.
3. Ensure a request that times out while queued can never begin executing later.
4. Guarantee active-request removal after normal completion, timeout, cancellation, panic, and response-generation failure.
5. Add a bounded process-wide synchronous executor for budget-aware in-process APIs.
6. Keep the MCP path on its existing Tokio semaphore/`spawn_blocking` boundary without nesting the synchronous executor.
7. Add deterministic synchronization hooks and tests that exercise the actual affected paths.
8. Regenerate release evidence against one exact commit and include real workflow run identifiers.

# Non-goals

This pass must not:

- add new user-facing tools;
- add new MCP methods or protocol revisions;
- redesign the tool registry broadly;
- replace Tokio;
- introduce a second async runtime;
- add unbounded detached threads;
- make the MCP server call the synchronous worker pool from inside `spawn_blocking`;
- change parser, regex, calculator, or configuration semantics except where required to propagate cancellation/timeout correctly;
- expand Release 4 or Release 5 scope beyond correcting evidence and running their existing gates;
- publish to crates.io through GitHub Actions;
- mark Releases 4 or 5 closed without real workflow evidence.

# Required sequencing

The implementation must proceed in this order:

1. Extract and correct the MCP execution lifecycle.
2. Replace best-effort active-request cleanup with awaited generation-aware cleanup.
3. Add bounded synchronous execution for budget-aware registry APIs.
4. Add deterministic concurrency/race tests.
5. Reconcile documentation and version/deprecation metadata.
6. Run local verification.
7. Run GitHub CI, manual release verification, and fuzz matrix workflows against the exact final commit.
8. Replace provisional closure evidence with commit- and run-specific evidence.

Do not update closure checkboxes before the relevant implementation and verification steps pass.

---

# Workstream 1 — Extract a testable MCP execution coordinator

## Objective

Move the bounded tool-execution logic out of the large request-dispatch match arm into a small internal coordinator with explicit lifecycle ownership and injectable synchronization seams.

This is a narrow extraction, not a general server refactor.

## Suggested files

- add `src/mcp/execution.rs`, or
- add a private `execution` submodule under `src/mcp/server.rs` if a new module would create unnecessary public surface.

Update:

- `src/mcp/mod.rs` only if the module is split into a new file;
- `src/mcp/server.rs` to delegate tool execution;
- `architecture/mcp-server.md` after implementation.

## Suggested internal interface

The exact types may vary, but the execution boundary should accept owned inputs and return a `ToolResponse`/join error classification:

```rust
pub(crate) async fn execute_tool_bounded(
    handler: ToolHandler,
    args: Value,
    tool_name: String,
    budget: ToolBudget,
    cancel_flag: Arc<AtomicBool>,
    semaphore: Arc<Semaphore>,
    metrics: &'static RuntimeMetrics,
    hooks: ExecutionHooks,
) -> ExecutionOutcome;
```

Production code uses no-op hooks. Tests use barriers, channels, or `Notify` values to control:

- permit acquisition;
- transition from queued to running;
- blocking closure entry;
- handler release/completion;
- timeout-accounting reservation;
- handler-exit accounting.

Do not use sleeps as the primary synchronization mechanism.

## Acceptance criteria

- The `tools/call` branch performs validation and response wrapping but delegates execution to the coordinator.
- The coordinator owns all lifecycle transitions and timeout metrics.
- The execution path is unit-testable without spawning the stdio binary.
- Production behavior and response envelopes remain compatible except for corrected diagnostics.

---

# Workstream 2 — Implement an exact queued/running timeout lifecycle

## Required lifecycle model

Use an explicit state model that distinguishes queueing from execution. The representation may be an atomic enum encoding or a small mutex-protected state object, but the observable invariants below are mandatory.

A suitable atomic model is:

```rust
const HANDLER_QUEUED: u8 = 0;
const HANDLER_RUNNING: u8 = 1;
const HANDLER_TIMEOUT_ACCOUNTING: u8 = 2;
const HANDLER_TIMED_OUT_ACCOUNTED: u8 = 3;
const HANDLER_FINISHED: u8 = 4;
const HANDLER_TIMED_OUT_QUEUED: u8 = 5;
```

A `std::sync::Mutex<HandlerPhase>` is also acceptable if transitions remain extremely short and no async await occurs while the mutex is held. Avoid adding a dependency solely for this state machine.

## Required state semantics

### Initial queue state

- Every tool call starts in `QUEUED`.
- `active_blocking_handlers` is zero until the blocking closure actually begins.
- `timed_out_handlers` is zero while queued.

### Permit acquisition

After `acquire_owned()` succeeds and before `spawn_blocking` is submitted:

- atomically transition `QUEUED -> RUNNING`;
- if the state is already `TIMED_OUT_QUEUED`, release the permit and do not spawn;
- if cancellation is already set, transition to `FINISHED`, release the permit, and return `CANCELLED`;
- no path may start the blocking closure after a queued timeout response has been returned.

### Timeout while queued

When the response deadline fires and the state is `QUEUED`:

- transition `QUEUED -> TIMED_OUT_QUEUED`;
- increment `total_timeouts`;
- do not increment `timed_out_handlers`;
- cancel/drop the semaphore-acquisition future;
- ensure later permit availability cannot start the request.

### Timeout while running

The timeout path must not publish a decrementable timeout state before the gauge is incremented.

Preferred atomic sequence:

1. reserve `RUNNING -> TIMEOUT_ACCOUNTING`;
2. increment `timed_out_handlers`;
3. publish `TIMEOUT_ACCOUNTING -> TIMED_OUT_ACCOUNTED`;
4. return the timeout response.

The handler-exit path must handle the short `TIMEOUT_ACCOUNTING` transition without decrementing early. It may retry/yield until the state becomes `TIMED_OUT_ACCOUNTED`, then perform exactly one decrement.

A mutex-protected state implementation may update the phase and gauge in one critical section instead.

### Completion before timeout

- transition `RUNNING -> FINISHED`;
- do not touch `timed_out_handlers`;
- a later timeout observation must see `FINISHED` and count only the returned timeout if the outer deadline genuinely fired, without creating a running-handler gauge entry.

### Completion after timeout

- transition `TIMED_OUT_ACCOUNTED -> FINISHED`;
- decrement `timed_out_handlers` exactly once;
- normal return, cancellation, and panic use the same completion guard.

### Panic handling

The blocking closure must use a completion guard whose `Drop` finalizes lifecycle accounting even if the handler panics.

The Tokio `JoinError` response behavior may remain, but metrics and semaphore occupancy must be correct.

## Metric definitions

Document and enforce:

- `active_requests`: registered request tasks that have not completed cleanup;
- `active_blocking_handlers`: blocking closures that have begun execution and have not exited;
- `timed_out_handlers`: blocking closures still running after their caller-facing timeout response was returned;
- `total_timeouts`: cumulative timeout responses, including queue-wait timeouts;
- `peak_blocking_concurrency`: maximum simultaneous executing blocking closures, excluding queued requests.

## Required implementation invariants

- `timed_out_handlers <= active_blocking_handlers` at synchronized stable snapshots.
- No `fetch_sub` can run without a preceding matching `fetch_add`.
- A queued timeout never changes `timed_out_handlers`.
- Every running-timeout increment has exactly one decrement.
- All gauges return to zero after controlled workers finish.

## Required tests

Add deterministic coordinator tests for:

1. timeout while queued behind a saturated semaphore;
2. queued timeout never enters the handler after a permit is released;
3. handler completes before timeout;
4. timeout while running, followed by normal completion;
5. completion racing timeout reservation before gauge accounting;
6. completion racing after gauge accounting;
7. handler panic after timeout;
8. cooperative cancellation after timeout;
9. hundreds of controlled timeout/completion races;
10. stable snapshots never show underflow or `timed_out_handlers > active_blocking_handlers`;
11. all gauges return to zero after each test;
12. semaphore occupancy never exceeds `MAX_TOOL_WORKERS`.

Remove or rename any existing test whose name claims to cover this lifecycle but does not enter the coordinator.

---

# Workstream 3 — Guarantee active-request cleanup

## Objective

Stop depending on `Drop + try_lock` for correctness.

## Required design

Replace pointer-address identity with an explicit monotonically increasing generation/token.

Suggested structures:

```rust
struct ActiveRequestEntry {
    generation: u64,
    method: String,
    cancel_flag: Arc<AtomicBool>,
}

struct RequestRegistration {
    id: Value,
    generation: u64,
}
```

Use a process- or session-local `AtomicU64` to assign generations. The generation must not be derived from an allocation address.

## Registration

`register_request` must atomically:

- enforce the in-flight limit;
- reject duplicate active IDs;
- allocate/record the generation;
- insert the entry;
- return `RequestRegistration`.

## Completion

Add an awaited function:

```rust
async fn complete_request(
    active: &ActiveRequests,
    registration: &RequestRegistration,
) -> bool;
```

It must:

- acquire the map lock with `.lock().await`;
- remove the entry only when ID and generation match;
- return whether removal occurred;
- be called before the outer request task exits.

## Panic-safe task structure

Because cleanup is async, do not rely on an async-unaware `Drop` implementation.

Use an outer task that always performs cleanup and an inner task that may panic:

```rust
join_set.spawn(async move {
    let inner = tokio::spawn(async move {
        handle_request_async(...).await
    });

    let outcome = inner.await;
    complete_request(&active_requests, &registration).await;
    send_response_for_outcome(outcome).await;
});
```

The exact layout may differ, but cleanup must occur after:

- normal handler return;
- timeout response generation;
- cancellation;
- handler panic captured as `JoinError`;
- serialization/response construction failure.

Inline `initialize` processing must also call awaited cleanup explicitly.

A `Drop` fallback may remain only for diagnostics/debug assertions; it must not be the correctness mechanism.

## Required tests

1. Hold the active-request lock while a request finishes; after releasing the lock, awaited cleanup completes.
2. Reuse the same request ID immediately after cleanup.
3. A stale generation cannot remove a newer request using the same ID.
4. Handler panic still removes the request.
5. Timeout still removes the request while the underlying blocking worker may continue.
6. Cancellation still removes the request.
7. Repeated contention does not reduce effective `MAX_IN_FLIGHT_REQUESTS`.
8. After hundreds of controlled completions, the map is empty and `active_requests == 0`.

## Acceptance criteria

- No correctness path uses `try_lock` to remove active requests.
- No completed request can remain registered indefinitely.
- Duplicate-ID and capacity behavior recover immediately after completion cleanup.
- Cancellation can never target a completed generation after cleanup.

---

# Workstream 4 — Add bounded synchronous in-process execution

## Objective

Make the budget-aware synchronous APIs enforce both elapsed-time and concurrency limits without restoring unbounded per-call threads.

## API policy

Preserve backward compatibility deliberately:

- `ToolRegistry::call_json` remains the low-level direct synchronous call unless a breaking API decision is explicitly approved. Its documentation must state that it performs no caller-facing elapsed-time enforcement.
- `call_json_with_budget` must become the primary bounded synchronous call.
- `call_json_with_context` and execution-context variants must use the same bounded executor.
- the MCP server continues to invoke the prepared handler directly inside its Tokio-owned `spawn_blocking` closure and must not call the synchronous executor.

## Required executor architecture

Add one process-wide or registry-shared bounded worker pool, for example:

```rust
struct SyncExecutionPool {
    sender: SyncSender<SyncJob>,
    worker_count: usize,
    queue_capacity: usize,
}
```

Use only a fixed number of long-lived worker threads. Do not spawn a new thread per call.

A standard-library implementation is sufficient:

- `std::sync::mpsc::sync_channel` for a bounded queue;
- a shared receiver protected only for receiving jobs;
- one response channel per submitted job;
- `recv_timeout` for the caller-facing deadline;
- `try_send` or equivalent deterministic queue-saturation behavior.

The implementation may choose another bounded design, but it must not add unbounded background work.

## Job contents

Each job must own everything needed for execution:

- handler function pointer;
- owned JSON arguments;
- tool name;
- resolved `ToolBudget`;
- cancellation flag;
- optional cloned `EvalContext`/execution context needed by the handler;
- compatibility/profile/audience decisions already resolved before enqueueing;
- reply channel.

The worker installs cancellation and eval-context thread-local guards before invoking the handler.

## Timeout behavior

On caller-facing timeout:

- set the cancellation flag;
- return a structured `TIMEOUT` `ToolResponse` through the existing `Result<ToolResponse, ToolCallError>` contract;
- leave the worker slot occupied until the handler exits;
- never spawn a replacement worker solely because the caller timed out.

Repeated timed-out calls must never exceed the configured worker count.

## Queue saturation behavior

When the bounded queue cannot accept work:

- return a structured `RESOURCE_EXHAUSTED` or documented `TIMEOUT` response immediately;
- do not block indefinitely waiting to enqueue;
- include the tool name and configured worker/queue limits in diagnostics without leaking internal addresses.

## Context behavior

- `call_json_with_context` must propagate the caller’s cancellation flag.
- An internal flag may be created when none is supplied.
- `call_json_with_execution_context` must clone and install the eval context exactly as documented.
- The deprecated mutable-context path must preserve its current non-persistence limitation unless separately redesigned.
- Timeout/cancellation must not leak thread-local context into the next worker job.

## Suggested configuration

Define internal constants with documented rationale, for example:

```rust
const MAX_SYNC_TOOL_WORKERS: usize = 8;
const MAX_SYNC_TOOL_QUEUE: usize = 32;
```

The exact values should be chosen based on existing `MAX_TOOL_WORKERS`, expected embedding use, and SBC deployment constraints. They must be test-overridable through an internal constructor so tests do not require eight blocked threads.

## Required tests

Use an internal test executor with two workers and a small queue.

1. Two blocking jobs occupy both workers; a third job queues.
2. Caller timeout returns within the configured bound.
3. Timed-out worker remains occupied until the test barrier releases it.
4. Concurrent active jobs never exceed the worker count.
5. Queue saturation returns the documented structured error.
6. Pool recovers after timed-out workers eventually exit.
7. Panic in one job does not kill the worker permanently or leak context.
8. Cancellation flag is visible inside the handler.
9. Eval-context/thread-local state is restored before the next job.
10. `call_json_with_budget` actually uses the executor and honors `max_elapsed_ms`.
11. MCP coordinator tests prove it invokes the raw handler and does not nest the sync pool.
12. Repeated timeouts do not increase process thread count beyond the fixed pool plus runtime-owned threads.

## Documentation

Update:

- Rustdoc on `call_json`;
- Rustdoc on all budget/context calls;
- `architecture/agent-api.md` or the current in-process API document;
- `architecture/mcp-server.md` to distinguish MCP and synchronous boundaries;
- `README.md` only if it currently makes timeout claims about direct calls.

## Acceptance criteria

- Budget-aware synchronous APIs enforce elapsed deadlines.
- Worker and queue bounds are hard limits.
- Timed-out work cannot create unbounded detached threads.
- MCP does not nest the synchronous executor.
- Direct unbounded behavior, if retained for `call_json`, is documented explicitly rather than implied safe.

---

# Workstream 5 — Replace misleading tests with controlled evidence

## Objective

Ensure test names and closure evidence correspond to the code paths actually exercised.

## Required changes

### Remove or rename the current sequential “race” test

The existing 500-call direct-registry test may remain as a basic repetition test, but it must not be named or documented as MCP runtime race evidence.

Suitable replacement name:

```text
test_repeated_direct_math_calls_do_not_modify_mcp_runtime_metrics
```

That test can assert that direct calls do not affect MCP metrics, which is the actual property it demonstrates.

### Add real controlled race tests

The extracted execution coordinator must be exercised concurrently with deterministic hooks. Include at least 500 controlled iterations across the timeout-before-finish and finish-before-timeout transition boundaries.

Do not rely on catastrophic regex timing to create races. Use test handlers blocked on channels/barriers.

### Add request-cleanup contention tests

Explicitly hold the active-request map lock while completion occurs, then release it and verify cleanup. This directly proves the old `try_lock` leak is gone.

### Add synchronous-pool containment tests

Use test-only worker and queue sizes to prove timeout and saturation behavior without long sleeps.

## Test location guidance

- state-machine and executor internals: unit tests adjacent to the implementation;
- MCP protocol behavior: `tests/mcp/test_protocol.rs`;
- stdio integration behavior: `tests/mcp/test_execution_safety.rs` only where subprocess coverage is materially needed;
- synchronous registry behavior: add a focused agent/execution integration module rather than overloading MCP tests.

## Acceptance criteria

- Every test claiming race coverage performs concurrent controlled transitions.
- Every metric assertion is made after synchronization establishes a stable snapshot.
- No closure test passes trivially because it bypasses the affected subsystem.
- Test comments identify which production invariant is being exercised.

---

# Workstream 6 — Correct changelog, deprecation, and closure metadata

## Deprecation versions

Audit each `#[deprecated(since = ...)]` against actual published crate history.

Rules:

- A deprecation introduced in the current `Unreleased` section must use the next planned release version when known, likely `1.3.0`, or omit `since` until release preparation.
- Do not retroactively label a newly introduced deprecation as `1.0.0` merely because 1.0.0 was the first stable release.
- Existing deprecations should retain the earliest version in which they actually shipped.
- Changelog text and Rust attributes must agree.

At minimum reconcile:

- `call_json_with_execution_context_mut`;
- `available_tools`;
- `ensure_mcp_defaults`;
- `set_mcp_mode`;
- any test-only deprecated response constructor.

## Changelog

Keep the current work under `Unreleased` until release preparation.

Correct claims that currently say the three-state timeout lifecycle is race-free. Describe the new queued/running/accounted state machine only after it is implemented.

## Closure evidence

Treat `docs/releases/2026-07-final-closure-evidence.md` as provisional during implementation.

After all code and tests land:

1. identify the exact final implementation commit;
2. run all local commands against that commit;
3. push without modifying the tree afterward, or regenerate evidence if another commit is required;
4. run ordinary CI against the same commit;
5. dispatch `Release Verification` against the same commit;
6. dispatch the extended fuzz matrix and sanitizer jobs against the same commit;
7. record workflow run URLs/IDs and per-job conclusions;
8. record artifact names and checksums where applicable;
9. mark Release 4 and Release 5 complete only after required runs succeed.

The evidence document must not claim it “identifies workflow runs” unless actual run identifiers are present.

## Release-state acceptance criteria

- `Cargo.toml`, `Cargo.lock`, changelog, deprecation metadata, docs, and evidence refer to the same release state.
- The evidence SHA contains every test and source change cited by the document.
- No unchecked item is hidden behind a general PASS statement.
- No Release 4/5 completion statement appears before manual workflow evidence exists.

---

# Workstream 7 — CI and verification execution

## Local canonical gate

Run from a clean checkout of the final implementation commit:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo run --locked --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

## Focused runtime gate

Add and run explicit filters for:

```bash
cargo test --locked --all-features handler_lifecycle -- --test-threads=1
cargo test --locked --all-features active_request_cleanup -- --test-threads=1
cargo test --locked --all-features sync_execution_pool -- --test-threads=1
cargo test --locked --all-features timeout_accounting -- --test-threads=1
```

Use the actual final test module/filter names in evidence.

Run the controlled transition suite repeatedly:

```bash
for i in $(seq 1 50); do
  cargo test --locked --all-features timeout_accounting -- --test-threads=1 || exit 1
done
```

The repeated command must exercise the real coordinator, not the direct registry bypass.

## MSRV gate

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

If the synchronous pool implementation uses APIs unavailable on MSRV, replace them with compatible standard-library primitives rather than raising MSRV in this corrective pass.

## Fuzz/property gate

```bash
cargo test --locked --all-features --tests property
RUSTUP_TOOLCHAIN=nightly cargo fuzz list
RUSTUP_TOOLCHAIN=nightly cargo fuzz build
```

No new fuzz target is required unless the implementation exposes a parser-like surface. Ordinary deterministic concurrency tests are the correct validation for the runtime state machine.

## GitHub evidence gate

Required successful runs against the exact final commit:

- ordinary `CI` push or PR workflow;
- manual `Release Verification` workflow;
- manual or scheduled `Fuzz Extended` matrix;
- sanitizer matrix;
- parity drift workflow if Release 4 closure requires current parity evidence;
- latest-compatible dependency workflow if Release 4 closure requires current dependency evidence.

Record:

- workflow name;
- run ID and URL;
- commit SHA;
- trigger type;
- start/end timestamps;
- every job conclusion;
- uploaded artifact names;
- relevant provenance checksums.

---

# Suggested implementation commits

Keep implementation reviewable with narrowly scoped commits.

## Commit 1 — MCP lifecycle extraction and accounting

Suggested message:

```text
fix(mcp): make queued and running timeout accounting exact
```

Contents:

- extract execution coordinator;
- add explicit queued/running/accounted states;
- add completion guard;
- add deterministic lifecycle tests;
- correct metric documentation.

## Commit 2 — Active-request cleanup

Suggested message:

```text
fix(mcp): guarantee generation-aware request cleanup
```

Contents:

- replace pointer identity with generation tokens;
- add awaited completion removal;
- restructure spawned request task for panic-safe cleanup;
- add contention/reuse/panic tests.

## Commit 3 — Bounded synchronous executor

Suggested message:

```text
fix(agent): bound budget-aware synchronous execution
```

Contents:

- add fixed worker pool and bounded queue;
- route budget/context APIs through it;
- preserve raw MCP handler execution;
- add timeout/saturation/context-restoration tests;
- update API documentation.

## Commit 4 — Evidence and metadata reconciliation

Suggested message:

```text
docs(release): regenerate correctness closure evidence
```

Contents:

- correct deprecation versions;
- update changelog and architecture docs;
- replace misleading test/evidence language;
- record exact final commit and local results;
- leave workflow-dependent items unchecked until runs complete.

If workflow evidence must be committed afterward, use one final documentation-only commit and rerun any evidence that must refer to that new commit. Prefer generating evidence for the true release-candidate commit rather than creating an endless evidence-SHA update loop.

---

# Final acceptance checklist

## Runtime lifecycle

- [ ] Initial handler state is queued, not running.
- [ ] A queued timeout never starts blocking work later.
- [ ] Queued timeout increments `total_timeouts` only.
- [ ] Running timeout increments `timed_out_handlers` before publishing a decrementable state.
- [ ] No handler path can decrement before the matching increment.
- [ ] Handler completion decrements the timed-out-running gauge exactly once.
- [ ] Panic and cancellation use the same completion accounting.
- [ ] Stable snapshots never show unsigned underflow.
- [ ] `timed_out_handlers <= active_blocking_handlers` at synchronized snapshots.
- [ ] All gauges return to zero after controlled workers exit.

## Active requests

- [ ] Active-request identity uses an explicit generation/token.
- [ ] Completion cleanup uses awaited locking.
- [ ] No correctness path relies on `try_lock` in `Drop`.
- [ ] Normal return removes the request.
- [ ] Timeout removes the request.
- [ ] Cancellation removes the request.
- [ ] Handler panic removes the request.
- [ ] Response serialization failure removes the request.
- [ ] A stale generation cannot remove a replacement request.
- [ ] Request IDs are reusable immediately after cleanup.

## Synchronous execution

- [ ] A fixed worker-count executor exists.
- [ ] The submission queue is bounded.
- [ ] Budget-aware synchronous calls enforce `max_elapsed_ms`.
- [ ] Queue saturation returns a structured bounded error.
- [ ] Timed-out work retains worker occupancy until it exits.
- [ ] Repeated timeouts do not create unbounded threads.
- [ ] Cancellation and eval-context state are installed and restored per job.
- [ ] MCP does not call the synchronous executor from inside `spawn_blocking`.
- [ ] Raw `call_json` timeout semantics are documented accurately.

## Tests

- [ ] Lifecycle races use barriers/channels/hooks rather than timing guesses.
- [ ] Queued timeout is tested with a saturated semaphore.
- [ ] Pre-accounting and post-accounting completion races are tested.
- [ ] Panic-after-timeout is tested.
- [ ] Request-map lock contention is tested.
- [ ] Synchronous worker saturation and recovery are tested.
- [ ] The old sequential “500-iteration race” claim is removed or renamed.
- [ ] Repeated controlled race runs pass without gauge leaks.

## Documentation and evidence

- [ ] Changelog describes the implemented lifecycle accurately.
- [ ] Deprecation `since` metadata matches actual shipping history.
- [ ] Architecture docs distinguish MCP and in-process execution boundaries.
- [ ] Closure evidence identifies the exact commit containing all cited changes.
- [ ] Local command results were produced from a clean checkout of that commit.
- [ ] Ordinary CI passed for that commit.
- [ ] Manual release-verification workflow passed for that commit.
- [ ] Extended fuzz and sanitizer matrices passed for that commit.
- [ ] Workflow run IDs/URLs and artifact names are recorded.
- [ ] Release 4 and Release 5 remain open until their evidence-dependent items pass.

# Completion definition

This corrective pass is complete only when all four implementation defects are removed from production code, deterministic tests exercise the actual affected paths, and closure evidence points to a single verified commit with real GitHub workflow results.

Passing existing tests without changing the three-state lifecycle, `RequestGuard::try_lock`, or direct budget-aware registry execution is not completion.
