# Timeout, Synchronous Policy, and Evidence Final Corrective Pass

## Status

- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `20e5facbe60bb486a230c8549babb4709e19ef56`
- **Scope:** narrowly scoped corrective pass
- **Predecessor plans:**
  - `plans/2026-07-21-final-runtime-verification-fuzz-closure-plan.md`
  - `plans/2026-07-21-runtime-correctness-evidence-corrective-pass.md`

## Purpose

The previous corrective pass landed the intended architecture:

- a dedicated MCP execution coordinator;
- a six-state handler lifecycle;
- generation-aware active-request cleanup;
- a bounded synchronous worker pool;
- expanded runtime and context-isolation tests.

However, the implementation still contains correctness gaps in the exact areas the prior plan intended to close. This plan addresses only those remaining defects:

1. the MCP timeout state machine can still leak `timed_out_handlers` and overwrite `FINISHED`;
2. a stale queue-state observation can miss accounting for work that became running;
3. coordinator tests rely on timing and sleeps rather than deterministic transition control;
4. synchronous pool timeouts do not signal cooperative cancellation;
5. timed-out queued synchronous jobs may still execute later;
6. budget-aware registry calls no longer preserve the registry's profile, audience, compatibility mode, or pre-execution error contract;
7. the deprecated mutable execution-context path remains unbounded;
8. closure evidence and changelog claims do not match current `main`.

This is the final implementation-quality pass. It must not expand tool scope, protocol scope, or release scope.

---

# Current release-blocking defects

## 1. `TIMEOUT_ACCOUNTING` can be overwritten by handler completion

The current timeout path performs:

```text
RUNNING -> TIMEOUT_ACCOUNTING
increment timed_out_handlers
TIMEOUT_ACCOUNTING -> TIMED_OUT_ACCOUNTED
```

The handler completion path performs an unconditional swap to `FINISHED` and only decrements when the previous state is `TIMED_OUT_ACCOUNTED`.

The following interleaving is still possible:

```text
Timeout: RUNNING -> TIMEOUT_ACCOUNTING
Handler: TIMEOUT_ACCOUNTING -> FINISHED
Handler: no decrement
Timeout: timed_out_handlers += 1
Timeout: FINISHED -> TIMED_OUT_ACCOUNTED
```

The result is a permanently elevated `timed_out_handlers` gauge and a lifecycle that claims a finished handler is still timed out and running.

## 2. Timeout uses a stale state observation

The timeout path loads the lifecycle once and then attempts one state-specific compare-and-exchange.

A request can transition from queued to running between the load and the compare-and-exchange:

```text
Timeout loads QUEUED
Worker transitions QUEUED -> RUNNING
Timeout CAS QUEUED -> TIMED_OUT_QUEUED fails
Failure is ignored
```

The caller receives a timeout, the handler continues running, and `timed_out_handlers` is never incremented.

## 3. `RUNNING` is published before active execution accounting

The coordinator currently transitions to `RUNNING` before entering the blocking closure and incrementing `active_blocking_handlers`.

A timeout in that window can increment `timed_out_handlers` while `active_blocking_handlers` is still zero. That contradicts the documented meaning of both gauges.

## 4. Coordinator tests are not deterministic transition tests

The tests use very short timeouts and `sleep` calls to attempt to create races. They do not stop execution at the exact transition boundaries under review.

The current evidence states that barriers, channels, or hooks are used. The coordinator signature and tests do not support that claim.

## 5. Synchronous timeout does not set cancellation

`SyncExecutionPool::submit` waits with `recv_timeout`, but when the timeout expires it only returns `SyncPoolError::Timeout`.

It does not retain or set a cancellation flag. Consequently:

- a running handler receives no new cooperative stop signal;
- a queued job can later begin executing after its caller has already timed out;
- `call_json_with_budget`, which creates no cancellation flag, cannot cancel at all;
- `call_json_with_context` installs the caller flag in the worker but does not set it on timeout.

## 6. Budget-aware registry calls bypass registry policy

`call_json_with_budget` and `call_json_with_context` enqueue a closure that uses a static preparation helper with hard-coded policy:

- profile `full`;
- audience `Model`;
- default compatibility mode.

This regresses the established API contract. Examples:

- a `HumanMath` registry may execute a tool outside its profile;
- a `Harness` registry may reject a harness-only tool it should allow;
- an `EggcalcPython` registry may validate with native/default semantics;
- pre-execution failures become an `Ok(ToolResponse)` internal error instead of the original `Err(ToolCallError)`.

## 7. Mutable execution-context dispatch remains unbounded

`call_json_with_execution_context_mut` still invokes the handler directly on the calling thread. It resolves a budget but does not enforce `max_elapsed_ms` or the synchronous pool worker bound.

Its deprecation does not justify inaccurate budget semantics.

## 8. Evidence and changelog overstate closure

The closure document identifies an older implementation commit and marks all runtime/synchronous guarantees complete. Current `main` contains later CI and test changes.

The changelog also retains an obsolete three-state “race-free timeout metrics” entry in addition to the six-state lifecycle entry.

---

# Goals

1. Make MCP timeout lifecycle transitions linearizable and easy to prove.
2. Ensure queued work cannot execute after a caller-facing timeout.
3. Ensure every timed-out-running increment has exactly one decrement.
4. Ensure handler start accounting and lifecycle state are published consistently.
5. Replace timing-dependent race tests with deterministic synchronization.
6. Set cooperative cancellation whenever a synchronous caller times out.
7. Prevent expired queued synchronous jobs from invoking handlers.
8. Restore exact profile, audience, compatibility, and error behavior for all budget-aware registry calls.
9. Enforce bounded execution for the deprecated mutable-context path without applying late mutations after timeout.
10. Regenerate truthful release evidence against the final implementation and actual workflows.

# Non-goals

This pass must not:

- add tools or MCP methods;
- change MCP protocol versions;
- change default profiles or exposure classifications;
- redesign calculator, regex, parser, or diff semantics;
- replace Tokio;
- add a second async runtime;
- add per-call detached threads;
- make MCP dispatch use the synchronous worker pool;
- publish to crates.io through CI;
- raise MSRV;
- add a broad telemetry subsystem;
- claim Release 4 or Release 5 closure before evidence exists.

---

# Required sequencing

1. Replace the fragile atomic lifecycle transition implementation.
2. Add deterministic lifecycle hooks and isolated test metrics.
3. Add cancellable/deadline-aware synchronous jobs.
4. Restore caller-thread policy preparation and error contracts.
5. Bound the mutable execution-context path.
6. Add regression tests for all API-policy combinations.
7. Correct documentation and provisional evidence.
8. Run local, CI, release-verification, and fuzz evidence gates.

Do not update closure checkboxes before the corresponding code and tests pass.

---

# Workstream 1 — Replace the timeout state machine with mutex-owned transitions

## Decision

Replace the current `AtomicU8` lifecycle transition protocol with a small `std::sync::Mutex`-protected lifecycle object.

This state is per invocation, transitions are extremely short, and no `.await` occurs while the lock is held. A mutex makes state and metric ownership explicit and removes the need for an intermediate externally visible accounting state.

Do not attempt another one-off atomic patch unless the replacement includes a complete transition proof and deterministic tests for every failed-CAS interleaving.

## Required lifecycle

Use an enum similar to:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandlerPhase {
    Queued,
    Running,
    TimedOutQueued,
    TimedOutRunning,
    Finished,
}
```

Wrap it in a lifecycle owner:

```rust
struct HandlerLifecycle {
    phase: Mutex<HandlerPhase>,
}
```

The lifecycle owner should expose narrow methods rather than allowing direct state mutation:

```rust
fn begin_running(&self, metrics: &RuntimeMetrics) -> BeginRunning;
fn record_timeout(&self, metrics: &RuntimeMetrics) -> TimeoutDisposition;
fn finish(&self, metrics: &RuntimeMetrics);
```

Suggested return values:

```rust
enum BeginRunning {
    Run,
    CancelledBeforeStart,
}

enum TimeoutDisposition {
    Queued,
    Running,
    AlreadyFinished,
}
```

## Required transition behavior

### Initial state

- Every invocation begins in `Queued`.
- Acquiring a semaphore permit does not by itself mean the handler is running.
- Time after permit acquisition but before the blocking closure starts is still treated as queued/not executing.

### Blocking closure start

Inside the blocking closure, before invoking the handler:

1. lock the lifecycle;
2. if phase is `TimedOutQueued`, release the lock and return without invoking the handler;
3. if phase is `Queued`:
   - increment `active_blocking_handlers`;
   - update `peak_blocking_concurrency`;
   - set phase to `Running`;
4. release the lifecycle lock;
5. invoke the handler.

The active counter must be incremented before publishing `Running`.

### Timeout path

When the outer Tokio deadline expires:

1. set the cancellation flag;
2. increment `total_timeouts` exactly once;
3. lock the lifecycle;
4. match the current phase:
   - `Queued` -> `TimedOutQueued`; do not change `timed_out_handlers`;
   - `Running` -> increment `timed_out_handlers`, then set `TimedOutRunning`;
   - `Finished` -> leave gauges unchanged;
   - already timed-out states -> treat as an internal invariant violation in debug builds;
5. release the lock;
6. return the timeout response.

There must be no load-then-CAS gap and no externally visible “accounting in progress” phase.

### Completion path

After normal return or caught panic:

1. lock the lifecycle;
2. match the phase:
   - `Running` -> set `Finished`, then decrement `active_blocking_handlers`;
   - `TimedOutRunning` -> decrement `timed_out_handlers`, set `Finished`, then decrement `active_blocking_handlers`;
   - `TimedOutQueued` -> no handler should have been invoked; return/assert;
   - `Finished` -> debug assertion for double completion;
3. release the lock.

For `TimedOutRunning`, decrement `timed_out_handlers` before decrementing `active_blocking_handlers`.

## Panic safety

Keep `catch_unwind`, but place lifecycle finalization in a guard or one explicit post-catch path that runs for both success and panic.

The lifecycle finalizer must be idempotent only for defensive shutdown paths; ordinary execution must complete exactly once.

## Snapshot semantics

Document that cross-counter invariants are guaranteed at synchronized/quiescent snapshots. Do not claim that independent atomic loads form a globally atomic snapshot while other threads are mutating counters.

## Acceptance criteria

- No `HANDLER_TIMEOUT_ACCOUNTING` state remains.
- No state transition can overwrite `Finished`.
- No timeout branch ignores a failed state transition.
- Timeout while queued never invokes the handler later.
- Timeout while running increments exactly once.
- Completion after timeout decrements exactly once.
- Handler start increments `active_blocking_handlers` before `Running` is visible.
- All stable post-barrier snapshots satisfy `timed_out_handlers <= active_blocking_handlers`.

---

# Workstream 2 — Add deterministic lifecycle test controls

## Objective

Make the exact transition interleavings testable without relying on scheduler luck, catastrophic inputs, or millisecond sleeps.

## Required design

Add a test-only execution configuration. Production should continue calling the simple coordinator entry point.

Example:

```rust
pub(crate) async fn execute_tool_bounded(...) -> ExecutionOutcome {
    execute_tool_bounded_inner(..., ExecutionHooks::none(), &RUNTIME_METRICS).await
}

#[cfg(test)]
async fn execute_tool_bounded_with_hooks(
    ...,
    hooks: ExecutionHooks,
    metrics: Arc<RuntimeMetrics>,
) -> ExecutionOutcome;
```

Use hooks based on barriers/channels/`Notify`, for example:

- permit acquired;
- blocking closure entered before lifecycle start;
- running state/accounting established;
- timeout about to lock lifecycle;
- timeout transition complete;
- handler about to finish;
- lifecycle completion complete.

The hook representation can use optional `Arc<Barrier>`, channels, or callback traits. It must not affect the public crate API.

## Isolated metrics

Allow tests to inject a fresh `RuntimeMetrics` instance. Do not use tolerance such as “baseline + 1” to account for unrelated parallel tests.

Expose test-only constructors as needed:

```rust
#[cfg(test)]
impl RuntimeMetrics {
    fn new_for_test() -> Self;
}
```

## Required deterministic tests

1. **Queued timeout before permit:** hold a zero-permit semaphore, trigger timeout, release permit, prove handler did not run.
2. **Timeout after permit but before closure start:** pause the closure before `begin_running`, trigger timeout, release closure, prove handler did not run.
3. **Running timeout:** pause after `begin_running`, trigger timeout, assert timed-out-running gauge is exactly one, release handler, assert exact zero.
4. **Completion wins:** pause timeout before lifecycle lock, finish handler, release timeout, prove no timed-out-running increment.
5. **Timeout wins:** pause handler before finish, complete timeout transition, release handler, prove exactly one decrement.
6. **Panic after timeout:** complete timeout transition, release a handler that panics, prove both gauges return to zero.
7. **Cancellation after timeout:** prove the handler observes the set flag.
8. **No double completion:** exercise defensive completion behavior.
9. **500 controlled interleavings:** alternate completion-wins and timeout-wins using barriers, with no sleeps used to create the race.
10. **Worker bound:** exercise the actual coordinator, not a standalone semaphore test, and prove executing handlers never exceed the configured permit count.

## Acceptance criteria

- Race tests stop execution at exact transition boundaries.
- Sleep is not used as the primary race-generation mechanism.
- No metric assertion uses tolerance for unrelated tests.
- Tests fail against the current `AtomicU8` implementation.
- Tests pass repeatedly under single-threaded and ordinary parallel test execution.

---

# Workstream 3 — Make synchronous jobs cancellation- and deadline-aware

## Required job model

Extend `SyncJob` to own:

```rust
struct SyncJob {
    handler: Box<dyn FnOnce() -> ToolResponse + Send + 'static>,
    reply: SyncSender<ToolResponse>,
    cancel_flag: Arc<AtomicBool>,
    deadline: Instant,
}
```

Store queue capacity on the pool for diagnostics.

## Submission contract

Replace or supplement `submit` with a cancellable form:

```rust
fn submit(
    &self,
    handler: impl FnOnce() -> ToolResponse + Send + 'static,
    timeout: Duration,
    cancel_flag: Arc<AtomicBool>,
) -> Result<ToolResponse, SyncPoolError>;
```

The pool must retain a clone of the cancellation flag outside the worker closure.

## Caller timeout

When `recv_timeout` returns `Timeout`:

1. set `cancel_flag = true` before returning;
2. return `SyncPoolError::Timeout`;
3. do not spawn replacement workers;
4. do not remove a running worker from the worker bound.

Map `RecvTimeoutError::Disconnected` to `SyncPoolError::Shutdown`, not `Timeout`.

## Worker preflight

Before invoking a queued job:

- if `cancel_flag` is already set, do not invoke the handler;
- if `Instant::now() >= deadline`, set the flag and do not invoke the handler;
- send a cancellation/timeout response if the reply channel is still open;
- continue to the next job.

This guarantees that work that timed out while waiting in the queue cannot execute later.

## Worker execution

The registry wrapper must install the same cancellation flag in the thread-local budget context before invoking the handler.

A running handler may continue if it does not cooperate, but it must receive the signal and retain the worker slot until exit.

## Queue saturation

When `try_send` returns full:

- do not set the cancellation flag, because the job was never accepted;
- return `RESOURCE_EXHAUSTED` immediately;
- include worker count and queue capacity in the diagnostic response.

## Required tests

Use a pool with one or two workers and barriers.

1. timeout sets the provided cancellation flag before returning;
2. a running cooperative handler observes the flag and exits;
3. a queued job times out and never invokes its handler later;
4. a timed-out running job retains worker occupancy until released;
5. queue saturation returns `RESOURCE_EXHAUSTED` without setting cancellation;
6. disconnected worker channel maps to shutdown, not timeout;
7. repeated timeouts do not increase worker count;
8. expired queued jobs are discarded and the pool recovers;
9. panic does not kill the worker or leak thread-local state.

## Acceptance criteria

- Every accepted synchronous job has a cancellation flag and deadline.
- Caller timeout always sets cancellation.
- Expired queued jobs never invoke handlers.
- Running timeouts remain bounded by the fixed worker count.
- Pool error classification distinguishes timeout, saturation, and shutdown.

---

# Workstream 4 — Restore registry policy and error contracts

## Objective

All policy preparation must occur on the caller thread using the actual registry/context configuration. The worker pool must only execute an already-approved handler.

## `call_json_with_budget`

Required sequence:

1. call `self.prepare_tool_call(name, &args)`;
2. if it returns `PreExecutionError`, return the original `Err(ToolCallError)`;
3. resolve the tool specification and effective budget;
4. enforce serialized input size;
5. create an internal cancellation flag;
6. enqueue the approved handler, owned args, budget context, and flag;
7. truncate the successful response.

Do not call `prepare_tool_call_static` inside the worker.

## `call_json_with_context`

Required sequence:

1. prepare with `self.prepare_tool_call` on the caller thread;
2. preserve the registry's profile, audience, and compatibility mode;
3. use the supplied cancellation flag or create an internal one;
4. pass the same flag to both the pool and thread-local context;
5. preserve original `ToolCallError` results.

## `call_json_with_execution_context`

Required sequence:

1. calculate effective profile, audience, and compatibility mode from the context and registry fallback;
2. call `prepare_tool_call_with_policy` on the caller thread;
3. return original `ToolCallError` on failure;
4. enqueue only the approved handler and cloned execution state;
5. use the same cancellation flag for pool timeout and handler context.

## Remove static policy fallbacks

Delete `prepare_tool_call_static` and any hard-coded full/Model/default path if they are no longer needed.

A static helper with explicit policy may remain only if it is a pure implementation helper called with values already resolved from the caller. It must not invent defaults.

## Required regression matrix

Add tests covering:

1. `HumanMath` + `call_json_with_budget` rejects `text_equal` with `ToolUnavailable`;
2. `Full/Harness` + budget call allows `shell_split`;
3. `Full/Model` + budget call rejects `shell_split` with `ToolNotAllowedForAudience`;
4. registry compatibility mode is honored by budget calls;
5. execution-context profile override is honored;
6. execution-context audience override is honored;
7. execution-context compatibility override is honored;
8. unknown tool remains `Err(UnknownTool)`;
9. schema failure remains `Err(InvalidArguments)`;
10. queue saturation remains a structured `Ok(ToolResponse)` runtime failure because policy already passed;
11. timeout remains a structured `Ok(ToolResponse)` runtime failure;
12. direct `call_json` behavior is unchanged.

## Acceptance criteria

- No budget-aware call hard-codes profile, audience, or compatibility.
- Pre-execution failures preserve `Err(ToolCallError)`.
- Runtime execution failures preserve structured `ToolResponse` envelopes.
- All registry and explicit-context policy combinations match direct call behavior.

---

# Workstream 5 — Bound the mutable execution-context path transactionally

## Objective

Enforce elapsed-time and worker bounds for `call_json_with_execution_context_mut` without allowing late worker completion to mutate the caller after timeout.

## Required design

1. resolve policy on the caller thread;
2. clone `ctx.eval_ctx` into a worker-local value;
3. execute with that local value through the synchronous pool;
4. store the completed local context in an `Arc<Mutex<Option<EvalContext>>>` or equivalent result slot;
5. if the worker returns before the deadline and the response is eligible to commit, copy the returned context into `ctx.eval_ctx`;
6. if the call times out, is cancelled, panics, or returns a transaction-failure response, leave `ctx.eval_ctx` unchanged;
7. late worker completion after timeout may write only to the detached result slot, never to the caller's context.

## Commit policy

Preserve the method's documented transaction semantics:

- pre-execution failure: unchanged;
- parse/evaluation/tool failure: unchanged;
- timeout/cancellation: unchanged;
- successful completion: apply the worker-local context if the handler actually used/mutated it.

The existing `math_eval` non-persistence limitation may remain if that handler still clones internally, but the outer API must be bounded and must not falsely claim direct mutation for that tool.

## Required tests

1. timeout returns within the configured budget;
2. caller context is unchanged after timeout;
3. late completion cannot mutate caller context;
4. pre-execution error leaves context unchanged;
5. tool failure leaves context unchanged;
6. successful test-only/context-aware handler commits expected mutation, if a suitable registered surface exists;
7. cancellation flag is set and visible;
8. pool saturation leaves context unchanged.

## Acceptance criteria

- The deprecated mutable path no longer executes directly on the caller thread.
- It uses the fixed synchronous pool.
- It never applies late mutations after timeout.
- Documentation accurately describes the remaining `math_eval` limitation.

---

# Workstream 6 — Correct tests, changelog, and evidence

## Test naming and claims

- Keep the renamed direct-call metrics test as a direct-call baseline only.
- Do not describe timing-based loops as deterministic race tests.
- Coordinator race coverage must reference the new hook-driven tests.
- Remove tolerance-based metric assertions.

## Changelog

Remove the obsolete three-state “race-free timeout metrics” entry.

Describe only the final lifecycle implementation. If the mutex-backed lifecycle replaces the six-state atomic model, update the changelog to reflect the actual final design rather than preserving implementation-history prose as a current feature claim.

## Deprecation metadata

Re-audit `since` values against actual published versions. Do not change them merely to silence inconsistency. Changelog and attributes must agree.

## Evidence model

Replace the current ambiguous single-SHA claim with explicit fields:

```text
Code-under-test SHA: <implementation commit>
Evidence-recording commit: <documentation commit, when known>
```

The code-under-test SHA must contain every runtime and test change cited by the evidence.

If a documentation-only evidence commit follows successful runs:

- state clearly that workflows ran against the code-under-test SHA;
- run ordinary CI on the evidence-recording head;
- do not claim manual fuzz/release workflows ran against the evidence commit unless they did.

## Required evidence

Record actual values, not placeholders:

- ordinary CI run ID and URL;
- every required job conclusion;
- manual release-verification run ID and URL;
- provenance artifact name and checksum;
- extended fuzz matrix run ID and all target conclusions;
- sanitizer matrix run ID and conclusions;
- parity and latest-compatible conclusions if required for Release 4 closure;
- exact Rust stable, MSRV, nightly, and cargo-fuzz versions;
- exact test filters and counts.

Do not mark Release 4 or Release 5 closed before all evidence-dependent items succeed.

## CI reproducibility note

The current workflows install `cargo-deny` without an explicit crate version. Pin the exact supported `cargo-deny` version consistently in CI and release verification, or document and justify why floating latest is intentional. A release gate should not change behavior because a new cargo-deny release appeared.

---

# Workstream 7 — Verification gates

## Focused lifecycle gate

Use the final test names, but include equivalent commands:

```bash
cargo test --locked --all-features --lib mcp::execution::lifecycle_tests -- --test-threads=1
cargo test --locked --all-features --lib mcp::execution::coordinator_tests -- --test-threads=1
```

Run the deterministic race suite repeatedly:

```bash
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::execution::coordinator_tests -- --test-threads=1 || exit 1
done
```

The suite must use hooks/barriers and must not depend on random scheduler timing.

## Focused synchronous gate

```bash
cargo test --locked --all-features --lib mcp::sync_pool -- --test-threads=1
cargo test --locked --all-features sync_policy -- --test-threads=1
cargo test --locked --all-features context_isolation -- --test-threads=1
```

## Canonical local release gate

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

## MSRV gate

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

Do not raise MSRV to accommodate the fix.

## Fuzz/property gate

```bash
cargo test --locked --all-features --tests property
RUSTUP_TOOLCHAIN=nightly cargo fuzz list
RUSTUP_TOOLCHAIN=nightly cargo fuzz build
```

No new fuzz target is required for lifecycle synchronization. Deterministic concurrency tests are the appropriate evidence.

## GitHub gate

Required successful runs for the code-under-test SHA:

- CI;
- Release Verification;
- Fuzz Extended matrix;
- sanitizer matrix;
- parity drift workflow;
- latest-compatible workflow.

---

# Suggested implementation commits

## Commit 1 — Lifecycle correctness

```text
fix(mcp): make timeout lifecycle transitions linearizable
```

Contents:

- replace `AtomicU8` protocol with mutex-owned lifecycle;
- correct handler-start accounting order;
- remove stale load/CAS behavior;
- add isolated metrics and deterministic hooks;
- add exact interleaving tests.

## Commit 2 — Cancellable synchronous pool

```text
fix(agent): cancel expired synchronous jobs
```

Contents:

- add job deadline and cancellation flag;
- set cancellation on caller timeout;
- discard expired queued jobs;
- distinguish timeout/disconnect errors;
- add worker occupancy and recovery tests.

## Commit 3 — Policy and mutable-context repair

```text
fix(agent): preserve registry policy in bounded dispatch
```

Contents:

- prepare calls on caller thread;
- remove hard-coded static defaults;
- preserve `ToolCallError` behavior;
- route mutable context through bounded transactional execution;
- add profile/audience/compatibility regression matrix.

## Commit 4 — Documentation and evidence preparation

```text
docs(release): correct runtime closure claims
```

Contents:

- remove obsolete changelog entry;
- update architecture and API docs;
- mark evidence provisional;
- record local verification against the code-under-test SHA;
- leave workflow-dependent items unchecked.

## Final evidence commit

After required workflows succeed:

```text
docs(release): record final verified closure evidence
```

Record exact run IDs, conclusions, and artifacts. Do not modify runtime code in this commit.

---

# Final acceptance checklist

## MCP lifecycle

- [ ] Lifecycle transitions are mutex-owned or equivalently linearizable.
- [ ] No intermediate accounting state can be overwritten by completion.
- [ ] No timeout transition depends on a stale one-time state load.
- [ ] Handler invocation cannot begin after a queued timeout.
- [ ] Active handler count is incremented before running state is published.
- [ ] Running timeout increments exactly once.
- [ ] Completion after timeout decrements exactly once.
- [ ] Panic after timeout returns all gauges to baseline.
- [ ] Stable synchronized snapshots satisfy documented invariants.

## Deterministic tests

- [ ] Coordinator exposes test-only transition hooks.
- [ ] Tests use isolated metrics.
- [ ] Exact queued, start, timeout, and completion boundaries are controlled.
- [ ] No tolerance-based metric assertions remain.
- [ ] 500 controlled interleavings pass.
- [ ] Repeated test loops pass without timing flakes.

## Synchronous pool

- [ ] Every accepted job owns a cancellation flag and deadline.
- [ ] Caller timeout sets cancellation before returning.
- [ ] Expired queued jobs never invoke handlers.
- [ ] Running timed-out work retains worker occupancy until exit.
- [ ] Queue saturation does not set cancellation for unaccepted work.
- [ ] Disconnection maps to shutdown rather than timeout.
- [ ] Worker and queue bounds remain fixed.

## Policy preservation

- [ ] Budget calls use the registry's actual profile.
- [ ] Budget calls use the registry's actual audience.
- [ ] Budget calls use the registry's actual compatibility mode.
- [ ] Execution-context overrides are honored.
- [ ] Unknown/profile/audience/schema failures remain `Err(ToolCallError)`.
- [ ] Timeout and saturation remain structured runtime responses.
- [ ] Hard-coded full/Model/default preparation is removed.

## Mutable execution context

- [ ] Mutable-context calls use the bounded pool.
- [ ] Timeout leaves caller context unchanged.
- [ ] Late completion cannot mutate caller context.
- [ ] Success applies only eligible completed state.
- [ ] Documentation matches actual persistence behavior.

## Release evidence

- [ ] Obsolete three-state changelog claim is removed.
- [ ] Evidence identifies the exact code-under-test SHA.
- [ ] Ordinary CI passed.
- [ ] Release Verification passed.
- [ ] Extended fuzz matrix passed.
- [ ] Sanitizer matrix passed.
- [ ] Required parity/latest-compatible evidence passed.
- [ ] Run IDs, URLs, conclusions, and artifacts are recorded.
- [ ] Release 4 and Release 5 remain open until all evidence exists.

# Completion definition

This pass is complete only when the timeout lifecycle is linearizable, synchronous timeouts signal and suppress expired work, bounded APIs preserve registry policy and error contracts, mutable-context execution is bounded transactionally, deterministic transition tests pass repeatedly, and final evidence is tied to actual successful workflow runs.

Passing the current sleep-driven tests or updating closure documentation without correcting these production paths is not completion.
