# Final Cancellation, Lifecycle, and Evidence Closure Pass

## Status

- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `e20f45e79ebd1ae23219b1ac40cfb262082debb9`
- **Scope:** final narrowly scoped corrective pass
- **Predecessor plans:**
  - `plans/2026-07-21-runtime-correctness-evidence-corrective-pass.md`
  - `plans/2026-07-22-timeout-sync-policy-final-corrective-pass.md`

## Purpose

The prior pass fixed the original atomic timeout-accounting race and restored caller-thread policy resolution. It also introduced a bounded synchronous pool and a transactional mutable-context path.

The remaining work is limited to six closure defects:

1. three bounded registry APIs do not pass the same cancellation flag to both the pool and the handler;
2. the synchronous worker preflight checks deadline expiry but not a pre-set cancellation flag;
3. `recv_timeout` disconnection is still classified as a timeout;
4. MCP lifecycle accounting begins before the blocking closure actually starts;
5. tests and evidence still describe timing-based loops as deterministic hook-driven race coverage;
6. mutable-context state is committed after any pool-level success, even when `response.ok == false`.

This pass must close those defects without adding tools, protocols, dependencies, or release scope.

---

# Stop-ship findings

## 1. Cancellation identity is split across the pool and handler

The bounded execution contract requires one `Arc<AtomicBool>` per accepted job. The same allocation must be:

- owned by the caller;
- cloned into the pool job;
- installed as the handler thread-local cancellation flag;
- set by the pool before returning a timeout;
- observable by the handler after the caller-facing timeout.

Current problem patterns include:

```rust
let cancel_clone = ctx.cancellation.clone();

sync_pool().submit_cancellable(
    move || with_cancel_flag(cancel_clone, || handler(&args)),
    timeout,
    Arc::new(AtomicBool::new(false)), // WRONG: different flag
)
```

and:

```rust
let flag = Arc::new(AtomicBool::new(false));

sync_pool().submit_cancellable(
    move || handler(&args), // WRONG: flag not installed for handler
    timeout,
    flag,
)
```

These patterns make cancellation appear implemented while preventing the handler from observing the signal.

## 2. Cancelled queued jobs can still execute

The worker checks `Instant::now() >= deadline`, but it does not reject a job whose cancellation flag was already set before the worker dequeued it.

A caller can therefore cancel a queued job while it is still waiting, yet a non-cooperative handler may run later.

## 3. Reply-channel disconnection is reported as timeout

The current reply wait maps all `recv_timeout` errors to `SyncPoolError::Timeout`.

`RecvTimeoutError::Disconnected` must map to `SyncPoolError::Shutdown`. Otherwise worker/pool failure is misreported as caller budget exhaustion.

## 4. MCP `Running` accounting begins too early

The lifecycle transitions from `Queued` to `Running` before `spawn_blocking` enters the blocking closure.

This means:

- `active_blocking_handlers` includes work not yet running;
- a permit-acquired but not-yet-started task is represented as an executing handler;
- the implementation does not match the lifecycle semantics prescribed by the previous plan.

The transition and counter increment must occur inside the blocking closure immediately before invoking the handler.

## 5. Race tests do not use the hooks they claim to use

`ExecutionHooks` exists, but most lifecycle tests pass `ExecutionHooks::none()` and use short timeouts plus sleeps.

The 500-iteration test alternates fast and slow handlers, but does not force exact completion-wins or timeout-wins transition order.

The worker-bound test exercises a standalone semaphore rather than the coordinator.

This is not deterministic transition evidence.

## 6. Mutable-context commit is not conditioned on tool success

The mutable path commits the worker-local `EvalContext` after any `Ok(ToolResponse)` from the pool.

A tool-level failure also returns `Ok(ToolResponse)` with `response.ok == false`. The caller context must remain unchanged in that case.

The current `math_eval` failure test cannot prove the generic transaction rule because `math_eval` clones its context internally.

---

# Goals

1. Use one shared cancellation flag for every accepted bounded registry job.
2. Skip queued jobs that are cancelled or expired before handler invocation.
3. Distinguish timeout, saturation, disconnection, and tool failure accurately.
4. Publish MCP `Running` only from inside the blocking closure.
5. Replace timing-generated race claims with exact synchronization-point tests.
6. Commit mutable execution state only after a successful, uncancelled, commit-eligible response.
7. Pin release-gate tooling versions consistently.
8. Produce exact local and GitHub workflow evidence for one code-under-test commit.

# Non-goals

Do not:

- add tools or MCP methods;
- change protocol versions;
- change profile membership or exposure classes;
- change default budgets;
- add an async runtime;
- add per-call threads;
- change the fixed worker-count architecture;
- make MCP call the synchronous pool;
- raise MSRV;
- publish to crates.io through CI;
- mark Release 4 or Release 5 closed before workflow evidence exists.

---

# Required execution order

1. Correct cancellation identity across all bounded APIs.
2. Correct synchronous worker preflight and reply classification.
3. Move MCP lifecycle start into the blocking closure.
4. Add real transition gates and replace misleading tests.
5. Correct mutable-context commit eligibility.
6. Pin release-gate tooling.
7. Run focused and full local gates.
8. Commit the code-under-test state.
9. Run ordinary CI, release verification, fuzz, sanitizer, parity, and latest-compatible workflows against that exact commit.
10. Record evidence in a documentation-only commit.

Do not update closure checkboxes before the corresponding verification is complete.

---

# Workstream 1 — Unify cancellation identity across bounded registry APIs

## Required invariant

For each accepted bounded call, exactly one effective cancellation flag exists:

```rust
let cancel_flag = caller_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
let handler_flag = cancel_flag.clone();
let pool_flag = cancel_flag.clone();
```

The handler and pool must receive clones of that same allocation.

## `call_json_with_budget`

### Correct pattern

```rust
let cancel_flag = Arc::new(AtomicBool::new(false));
let handler_cancel = cancel_flag.clone();

let result = sync_pool().submit_cancellable(
    move || {
        budget::with_cancel_flag(Some(handler_cancel), || {
            let mut eval_ctx = EvalContext::mcp_mode();
            budget::with_eval_context(&mut eval_ctx, || handler(&args_clone))
        })
    },
    timeout,
    cancel_flag,
);
```

### Acceptance criteria

- The handler sees `current_cancel_flag()` during execution.
- A pool timeout sets the same flag observed by the handler.
- No separate hidden flag is created after the handler closure captures its flag.

## `call_json_with_context`

This path is closest to correct. Preserve this pattern:

```rust
let cancel_flag = supplied.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
let handler_cancel = cancel_flag.clone();

submit_cancellable(
    move || with_cancel_flag(Some(handler_cancel), || handler(...)),
    timeout,
    cancel_flag,
)
```

### Acceptance criteria

- A supplied flag is not replaced.
- An absent flag creates one internal flag.
- Queue saturation does not set the flag because the job was not accepted.
- Timeout sets the supplied flag before returning.

## `call_json_with_execution_context`

### Correct pattern

```rust
let cancel_flag = ctx
    .cancellation
    .clone()
    .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
let handler_cancel = cancel_flag.clone();

let result = sync_pool().submit_cancellable(
    move || {
        budget::with_cancel_flag(Some(handler_cancel), || {
            let mut eval_ctx = eval_ctx_clone;
            budget::with_eval_context(&mut eval_ctx, || handler(&args_clone))
        })
    },
    timeout,
    cancel_flag,
);
```

### Trouble area

Do not pass `ctx.cancellation.clone()` into the handler while passing a new flag into the pool. That silently splits cancellation identity.

### Acceptance criteria

- A context-provided flag is the exact flag the pool sets.
- An absent context flag creates one internal flag used by both sides.
- A timeout is observable through `budget::current_cancel_flag()`.

## `call_json_with_execution_context_mut`

Use the same effective flag for:

- pool timeout;
- handler thread-local cancellation;
- post-return commit eligibility.

```rust
let cancel_flag = ctx
    .cancellation
    .clone()
    .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
let handler_cancel = cancel_flag.clone();
let commit_cancel = cancel_flag.clone();
```

### Acceptance criteria

- The pool and handler receive the same flag.
- Timeout sets the caller-provided flag when one exists.
- The transaction commit path checks the same flag before mutating `ctx.eval_ctx`.

## Required tests

Add tests for each API, not only for `SyncExecutionPool` directly:

1. `call_json_with_budget` handler observes cancellation after timeout.
2. `call_json_with_context` supplied flag becomes true on timeout.
3. `call_json_with_execution_context` context flag becomes true on timeout.
4. `call_json_with_execution_context_mut` context flag becomes true on timeout.
5. Fast successful calls leave flags false.
6. Queue saturation leaves flags false.

### Test implementation guidance

Avoid using production tools whose runtime cannot be controlled. Extract an internal helper that accepts a prepared handler and a pool reference, or add `#[cfg(test)]` prepared-handler entry points.

A useful internal seam is:

```rust
fn execute_prepared_with_context(
    pool: &SyncExecutionPool,
    handler: ToolHandler,
    args: Value,
    eval_ctx: EvalContext,
    cancel_flag: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<ToolResponse, SyncPoolError>;
```

Unit tests can pass a test function pointer that polls `current_cancel_flag()`.

---

# Workstream 2 — Correct synchronous worker preflight

## Required preflight order

Before invoking a dequeued job:

```rust
let now = Instant::now();

if now >= job.deadline {
    job.cancel_flag.store(true, Ordering::Release);
    send_timeout_if_possible(job.reply);
    continue;
}

if job.cancel_flag.load(Ordering::Acquire) {
    send_cancelled_if_possible(job.reply);
    continue;
}

invoke_handler(job);
```

Deadline should be checked first so a caller-facing elapsed deadline remains classified as timeout. A separately pre-cancelled job remains classified as cancellation.

## Required guarantees

- A job timed out while still queued never invokes its handler later.
- A job cancelled externally while queued never invokes its handler later.
- A running non-cooperative handler may continue, but retains its worker slot until exit.
- No replacement worker is spawned.

## Trouble area

This is insufficient:

```rust
if Instant::now() >= job.deadline {
    continue;
}

(job.handler)();
```

It ignores external cancellation and timeout-triggered cancellation if clock comparison is affected by scheduling granularity.

## Required deterministic tests

Use a one-worker pool and explicit gates.

### Test A: queued timeout skips invocation

1. Start job 1 and block it on a gate after it enters the worker.
2. Submit job 2 with a short timeout.
3. Wait until job 2 returns `Timeout` and its flag is true.
4. Release job 1.
5. Wait until the worker processes job 2.
6. Assert job 2’s handler-run atomic remains false.

### Test B: externally cancelled queued job skips invocation

1. Block job 1.
2. Queue job 2 with a long deadline.
3. Set job 2’s cancellation flag before releasing job 1.
4. Release job 1.
5. Assert job 2’s handler never ran and its response is `CANCELLED` if the reply channel remains open.

### Test C: running timeout retains occupancy

1. Start a non-cooperative job and confirm it entered the worker.
2. Let the caller timeout.
3. Submit enough work to prove the worker remains occupied and queue bounds still apply.
4. Release the job.
5. Prove the pool recovers without increasing `worker_count`.

## Acceptance criteria

- Worker preflight checks both deadline and cancellation.
- Timed-out queued handlers never execute.
- Externally cancelled queued handlers never execute.
- Tests do not contain comments allowing the handler to run after queued timeout.

---

# Workstream 3 — Distinguish timeout from reply-channel disconnection

## Extract reply waiting into a focused helper

Suggested implementation:

```rust
fn wait_for_reply(
    reply_rx: Receiver<ToolResponse>,
    timeout: Duration,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<ToolResponse, SyncPoolError> {
    match reply_rx.recv_timeout(timeout) {
        Ok(response) => Ok(response),
        Err(RecvTimeoutError::Timeout) => {
            cancel_flag.store(true, Ordering::Release);
            Err(SyncPoolError::Timeout)
        }
        Err(RecvTimeoutError::Disconnected) => Err(SyncPoolError::Shutdown),
    }
}
```

## Why extract the helper

It allows deterministic unit testing of the two error variants without constructing an artificial half-dead worker pool.

## Required tests

### Timeout test

- Keep the reply sender alive.
- Send no response.
- Use a short timeout.
- Assert `SyncPoolError::Timeout` and cancellation flag true.

### Disconnection test

- Create a reply channel.
- Drop the sender immediately.
- Call `wait_for_reply` with a long timeout.
- Assert `SyncPoolError::Shutdown` and cancellation flag unchanged.

### Queue-send disconnection test

A pool constructed with zero workers may disconnect its receiver immediately. If this path is retained for tests, assert submission maps to `Shutdown`, not `Timeout`.

## Acceptance criteria

- `RecvTimeoutError::Timeout` maps only to `SyncPoolError::Timeout`.
- `RecvTimeoutError::Disconnected` maps only to `SyncPoolError::Shutdown`.
- Disconnection does not set cancellation because there is no confirmed timed-out running job.
- Tests exercise the actual channel error variants rather than constructing enum values manually.

---

# Workstream 4 — Move MCP handler start into the blocking closure

## Required lifecycle semantics

- `Queued` means the handler has not begun executing.
- Semaphore acquisition alone does not make a handler running.
- `Running` begins inside the blocking closure immediately before handler invocation.
- `active_blocking_handlers` counts executing blocking closures, not submitted tasks.

## Required code shape

### Before `spawn_blocking`

Only:

- acquire the permit;
- construct owned inputs;
- submit the blocking closure.

Do not call `begin_running` before `spawn_blocking`.

### Inside `spawn_blocking`

```rust
spawn_blocking(move || {
    let _permit = permit;

    hooks.blocking_closure_entered.checkpoint();

    match lifecycle.begin_running(metrics) {
        BeginRunning::CancelledBeforeStart => {
            return cancelled_before_start_response(&tool_name);
        }
        BeginRunning::Run => {}
    }

    hooks.running_established.signal();

    if cancel_flag.load(Ordering::Acquire) {
        lifecycle.finish(metrics);
        return cancelled_response(&tool_name);
    }

    let result = catch_unwind(...handler...);
    lifecycle.finish(metrics);
    map_result(result)
})
```

## Important race behavior

If the outer timeout fires after permit acquisition but before the closure enters:

1. lifecycle is still `Queued`;
2. timeout records `TimedOutQueued`;
3. the blocking closure eventually starts;
4. `begin_running` sees `TimedOutQueued`;
5. the handler is never invoked;
6. active and timed-out-running gauges remain unchanged.

## Trouble areas

Do not:

- increment `active_blocking_handlers` before `spawn_blocking` starts;
- call `finish` after `CancelledBeforeStart` unless lifecycle semantics explicitly require a terminal-state conversion;
- double-finalize a closure that returned before `Running`;
- hold the lifecycle mutex while invoking the handler.

## Required acceptance tests

1. Timeout before permit: no closure submitted, handler never runs.
2. Timeout after permit but before closure lifecycle start: closure enters later but handler never runs.
3. `active_blocking_handlers` remains zero until the blocking closure crosses `begin_running`.
4. Running timeout increments timed-out-running exactly once.
5. Completion returns both gauges to zero.
6. Panic returns both gauges to zero.
7. Peak concurrency reflects actual executing closures.

---

# Workstream 5 — Replace timing-based lifecycle tests with exact gates

## Required gate types

Use synchronization mechanisms that cannot lose signals.

### Async-side gate

A `tokio::sync::Semaphore` works as a counted event:

```rust
struct AsyncGate {
    reached: Arc<Semaphore>,
    release: Arc<Semaphore>,
}

impl AsyncGate {
    fn signal_reached(&self) {
        self.reached.add_permits(1);
    }

    async fn wait_for_release(&self) {
        self.release.acquire().await.unwrap().forget();
    }
}
```

### Blocking-side gate

Use two `std::sync::Barrier`s:

```rust
struct BlockingGate {
    reached: Arc<Barrier>,
    release: Arc<Barrier>,
}

impl BlockingGate {
    fn checkpoint(&self) {
        self.reached.wait();
        self.release.wait();
    }
}
```

In async tests, wait on the blocking barrier through `tokio::task::spawn_blocking` so the runtime worker is not blocked.

## Suggested hook points

- permit acquired;
- blocking closure entered before `begin_running`;
- running/accounting established;
- timeout branch reached before lifecycle lock;
- timeout transition completed;
- handler returned before `finish`;
- lifecycle finish completed.

## Required exact interleavings

### Completion wins

1. Start running handler.
2. Pause timeout before lifecycle lock.
3. Release handler completion.
4. Wait for lifecycle `Finished`.
5. Release timeout.
6. Assert no timed-out-running increment occurred.

### Timeout wins

1. Start running handler.
2. Pause handler before `finish`.
3. Allow timeout transition to `TimedOutRunning`.
4. Assert `timed_out_handlers == 1` and `active_blocking_handlers == 1`.
5. Release finish.
6. Assert both gauges return to zero.

### Queued timeout after permit

1. Acquire permit.
2. Pause closure before `begin_running`.
3. Let outer timeout complete.
4. Assert phase recorded as queued timeout and gauges remain zero.
5. Release closure.
6. Assert handler-run flag remains false.

### 500 controlled interleavings

Run 250 completion-wins and 250 timeout-wins cycles using the gates above.

Do not generate these races with 1–10 ms deadlines and sleeps.

## Worker-bound test

The worker-bound test must call the actual coordinator with a semaphore of size N and handlers blocked on a shared gate.

Required assertions:

- exactly N handlers reach the running gate;
- the N+1 request remains queued;
- `active_blocking_handlers == N` at the synchronized snapshot;
- `peak_blocking_concurrency == N`;
- after release, all gauges return to zero.

## Acceptance criteria

- Race tests use hook points, not scheduler luck.
- Sleeps may be used only as a final bounded “did not happen” observation, never to establish order.
- The 500-iteration test explicitly forces each ordering.
- The worker-bound test exercises `execute_tool_bounded_inner` or the production coordinator path.
- Evidence no longer calls timing-based tests deterministic.

---

# Workstream 6 — Make mutable-context commits success-eligible

## Required commit predicate

Commit worker-local context only when all of the following are true:

```rust
response.ok
    && !cancel_flag.load(Ordering::Acquire)
    && commit_slot_contains_context
```

Pool timeout, queue saturation, shutdown, tool failure, cancellation, and panic must leave caller state unchanged.

## Correct pattern

```rust
match result {
    Ok(mut response) => {
        truncate_response(&mut response, &effective_budget);

        let commit_allowed = response.ok
            && !cancel_flag.load(Ordering::Acquire);

        if commit_allowed {
            if let Ok(mut slot) = commit_slot.lock() {
                if let Some(worker_ctx) = slot.take() {
                    ctx.eval_ctx = worker_ctx;
                }
            }
        }

        Ok(response)
    }
    Err(pool_error) => Ok(pool_error.to_tool_response(name)),
}
```

## Trouble area

Do not equate `Ok(ToolResponse)` with tool success. In this API:

- `Err(ToolCallError)` means pre-execution failure;
- `Ok(response)` may still be timeout, saturation, cancellation, parse failure, evaluation failure, or other tool-level failure;
- only `response.ok == true` is eligible for commit.

## Required test seam

`math_eval` cannot validate the generic commit rule because it clones its eval context internally.

Extract an internal prepared-handler transaction helper that accepts:

- a local `SyncExecutionPool`;
- a test `ToolHandler` function pointer;
- owned arguments;
- a cloned `EvalContext`;
- cancellation flag;
- timeout;
- commit slot.

Example test handler:

```rust
fn mutate_then_fail(_args: &Value) -> ToolResponse {
    let ctx = budget::current_eval_context().expect("context installed");
    let _ = evaluate_with_context("store(99, 2)", ctx);
    ToolResponse::error_with_code(
        "test_failure",
        machine_codes::INVALID_INPUT,
        "intentional failure",
        None,
        Some("test"),
    )
}
```

A successful companion handler should mutate the context and return `ToolResponse::success`.

## Required tests

1. Successful prepared handler commits expected context mutation.
2. Handler mutates then returns `ok=false`; caller context remains unchanged.
3. Handler mutates then times out; caller context remains unchanged after late completion.
4. Handler mutates then cancellation becomes true before commit; caller context remains unchanged.
5. Queue saturation leaves caller context unchanged.
6. Panic leaves caller context unchanged.
7. `math_eval` non-persistence limitation remains documented separately.

## Acceptance criteria

- Commit requires `response.ok == true`.
- Commit requires effective cancellation flag still false.
- Late writes remain isolated to detached commit storage.
- A generic context-mutating test handler proves both commit and rollback behavior.

---

# Workstream 7 — Pin release-gate tooling

## cargo-deny

Choose one version that:

- supports the project’s MSRV policy for the crate itself;
- runs on the current CI toolchain;
- passes the repository’s `deny.toml` configuration.

Use the exact same version in:

- `.github/workflows/ci.yml`;
- `.github/workflows/release-verification.yml`;
- local release documentation or scripts.

Example:

```bash
cargo install cargo-deny --version <PINNED_VERSION> --locked
```

Do not leave one workflow floating while another is pinned.

## Acceptance criteria

- The exact version appears in every installation command.
- Evidence records that version.
- A new upstream cargo-deny release cannot silently change the release gate.

---

# Workstream 8 — Evidence integrity and final closure

## Evidence fields

Update `docs/releases/2026-07-final-closure-evidence.md` to include:

```text
Code-under-test SHA: <implementation commit>
Evidence-recording SHA: <documentation commit>
```

The code-under-test SHA must contain all runtime, pool, test, CI, and tooling-pin changes.

## Local verification rules

Run from a clean checkout of the code-under-test SHA:

```bash
git status --short
git rev-parse HEAD
```

Evidence must record an empty status and exact SHA.

Do not write “against the working tree.”

## Focused gates

```bash
cargo test --locked --all-features --lib mcp::execution -- --test-threads=1
cargo test --locked --all-features --lib mcp::sync_pool -- --test-threads=1
cargo test --locked --all-features context_isolation -- --test-threads=1
cargo test --locked --all-features cancellation -- --test-threads=1
cargo test --locked --all-features mutable_context -- --test-threads=1
```

Use actual final filter names in evidence.

Run deterministic lifecycle coverage repeatedly:

```bash
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::execution -- --test-threads=1 || exit 1
done
```

This suite must be gate-driven, not sleep-generated.

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

## Required GitHub workflow evidence

Run against the exact code-under-test SHA:

- ordinary CI;
- manual Release Verification;
- extended fuzz matrix;
- sanitizer matrix;
- parity drift workflow;
- latest-compatible dependency workflow.

Record for each:

- workflow name;
- run ID;
- URL;
- trigger type;
- commit SHA;
- start/end timestamps;
- all job conclusions;
- artifact names;
- provenance/checksum values where applicable.

## Closure policy

- Do not mark Release 4 complete until release-verification, parity, and latest-compatible evidence succeeds.
- Do not mark Release 5 complete until extended fuzz and sanitizer evidence succeeds.
- Do not mark the runtime corrective pass complete until deterministic transition tests and shared cancellation tests succeed.

---

# Suggested implementation commits

## Commit 1 — Cancellation and pool classification

```text
fix(agent): unify cancellation and classify pool shutdown
```

Contents:

- shared flags across all bounded APIs;
- worker cancellation preflight;
- extracted reply-wait helper;
- timeout/disconnection tests.

## Commit 2 — MCP lifecycle start and deterministic gates

```text
fix(mcp): start lifecycle inside blocking closure
```

Contents:

- move `begin_running` into `spawn_blocking`;
- add exact gate primitives;
- replace timing-based race tests;
- coordinator-based worker-bound test.

## Commit 3 — Mutable transaction eligibility

```text
fix(agent): gate mutable context commits on success
```

Contents:

- shared cancellation flag;
- `response.ok` and cancellation commit predicate;
- prepared-handler transaction helper;
- generic mutation commit/rollback tests.

## Commit 4 — Reproducible release gates

```text
docs(ci): pin release tooling and prepare closure evidence
```

Contents:

- cargo-deny version pin;
- changelog wording corrections;
- evidence template with code-under-test fields;
- no false completion checkboxes.

## Commit 5 — Evidence record

After all required workflow runs succeed:

```text
docs(release): record final runtime closure evidence
```

Contents:

- run IDs and URLs;
- exact conclusions;
- artifact details;
- final closure status.

---

# Explicit acceptance checklist

## Shared cancellation

- [ ] `call_json_with_budget` uses one flag for pool and handler.
- [ ] `call_json_with_context` preserves and uses the supplied flag.
- [ ] `call_json_with_execution_context` uses one effective flag for pool and handler.
- [ ] `call_json_with_execution_context_mut` uses one effective flag for pool, handler, and commit decision.
- [ ] Timeout sets the effective flag before returning.
- [ ] Fast success leaves the flag false.
- [ ] Queue saturation leaves the flag false.

## Sync worker

- [ ] Worker checks deadline before invocation.
- [ ] Worker checks cancellation before invocation.
- [ ] Timed-out queued jobs never execute.
- [ ] Externally cancelled queued jobs never execute.
- [ ] Running non-cooperative jobs retain worker occupancy.
- [ ] Timeout and shutdown are classified separately.
- [ ] Disconnection tests exercise real channel disconnection.

## MCP lifecycle

- [ ] `begin_running` occurs inside the blocking closure.
- [ ] `active_blocking_handlers` increments inside the blocking closure.
- [ ] Permit acquisition alone does not publish `Running`.
- [ ] Timeout after permit but before closure start prevents handler invocation.
- [ ] Running timeout increments exactly once.
- [ ] Completion after timeout decrements exactly once.
- [ ] Panic returns gauges to zero.
- [ ] Peak concurrency reflects actual executing closures.

## Deterministic tests

- [ ] Exact hook/gate points control queued-timeout, completion-wins, and timeout-wins orderings.
- [ ] The 500-interleaving test forces 250 of each ordering.
- [ ] Sleeps are not used to create race order.
- [ ] Worker-bound coverage uses the actual coordinator.
- [ ] Metrics use isolated test instances.
- [ ] Repeated single-threaded execution passes 100 times.

## Mutable transaction

- [ ] Commit requires `response.ok == true`.
- [ ] Commit requires cancellation flag false.
- [ ] Tool failure leaves caller context unchanged.
- [ ] Timeout leaves caller context unchanged.
- [ ] Late completion leaves caller context unchanged.
- [ ] Cancellation leaves caller context unchanged.
- [ ] Queue saturation leaves caller context unchanged.
- [ ] A generic test handler proves successful mutation commit.
- [ ] A generic test handler proves mutation rollback on failure.

## Reproducibility and evidence

- [ ] cargo-deny is pinned to one exact version in all release gates.
- [ ] Evidence names the exact code-under-test SHA.
- [ ] Evidence names the evidence-recording SHA.
- [ ] Local verification ran from a clean checkout.
- [ ] Ordinary CI passed for the code-under-test SHA.
- [ ] Release Verification passed for the code-under-test SHA.
- [ ] Extended fuzz matrix passed for the code-under-test SHA.
- [ ] Sanitizer matrix passed for the code-under-test SHA.
- [ ] Parity and latest-compatible workflows passed where required.
- [ ] Workflow IDs, URLs, conclusions, and artifacts are recorded.
- [ ] Release 4 and Release 5 closure statements are evidence-backed.

# Completion definition

This pass is complete only when:

1. every bounded API uses one shared cancellation flag;
2. queued cancelled/expired jobs cannot invoke handlers;
3. timeout and shutdown are classified correctly;
4. MCP running accounting begins inside the blocking closure;
5. transition tests are gate-driven and exercise the real coordinator;
6. mutable context commits only on successful, uncancelled completion;
7. release tooling is pinned;
8. all required local and GitHub workflow evidence is recorded against the exact implementation commit.

Passing the current test suite without replacing timing-based race claims, shared-flag mismatches, or unconditional mutable-context commit is not completion.
