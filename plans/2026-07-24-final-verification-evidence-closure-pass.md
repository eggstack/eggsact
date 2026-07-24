# Final Verification and Evidence Closure Pass

## Status

- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `0926e7b0304139f4588cac32d17fadeeadf093a6`
- **Scope:** final verification, regression-proofing, and evidence closure only
- **Predecessor plans:**
  - `plans/2026-07-22-timeout-sync-policy-final-corrective-pass.md`
  - `plans/2026-07-23-final-cancellation-lifecycle-evidence-closure-pass.md`
- **Expected implementation shape:** one narrowly scoped implementation/test commit, followed by workflow execution and a documentation-only evidence commit

## Purpose

The production runtime corrections are now substantially in place:

- MCP handler lifecycle transitions are mutex-owned;
- `begin_running` occurs inside the blocking closure;
- bounded registry APIs share one cancellation flag between the pool and handler;
- queued synchronous jobs check both deadline and cancellation before invocation;
- timeout and reply-channel shutdown are classified separately;
- mutable execution-context commits require successful, uncancelled completion;
- `cargo-deny` is pinned in the ordinary CI workflow.

The remaining work is not another runtime redesign. It is a focused proof and closure pass addressing these remaining gaps:

1. the current lifecycle hooks notify but do not pause execution at exact transition boundaries;
2. several tests still use short deadlines and sleeps to establish or settle the ordering they claim to prove;
3. the specific permit-acquired / lifecycle-not-started timeout case is not forced directly;
4. the worker-bound test does not actually submit and observe the `N+1` coordinator invocation;
5. legacy shared blocking statics remain available to tests running in parallel;
6. mutable-context commit tests duplicate production commit logic instead of exercising the shared production implementation;
7. the disconnection test proves `std::sync::mpsc` behavior but does not call the production reply-classification helper;
8. closure evidence names an implementation SHA that predates tests cited by that same evidence;
9. ordinary CI, manual release verification, extended fuzzing, sanitizer evidence, and Release 4/5 closure remain open.

This pass must close those proof gaps without changing tool semantics, protocol behavior, public scope, or release strategy.

---

# Non-goals and constraints

This pass must not:

- redesign the mutex lifecycle;
- replace Tokio or the synchronous pool;
- add tools, MCP methods, profiles, or exposure classes;
- change parser, calculator, regex, normalization, diff, or schema semantics;
- change the public timeout/error envelope unless a verified bug is discovered;
- add detached per-call threads;
- make MCP use the synchronous execution pool;
- weaken or remove existing tests merely to make CI green;
- publish to crates.io through GitHub Actions;
- publish a crate as part of this pass;
- raise MSRV;
- mark Release 4 or Release 5 complete before exact workflow evidence exists;
- describe a timing-dependent test as deterministic or transition-controlled.

Crates.io publication remains a direct maintainer operation. This pass may run `cargo publish --dry-run`, but must not publish.

---

# Current truth that must be preserved

The implementation at the baseline already provides the following behavior and must not regress:

- one cancellation identity per bounded invocation;
- cancellation passed to both worker timeout ownership and handler thread-local state;
- deadline and cancellation preflight before queued synchronous handler invocation;
- `RecvTimeoutError::Timeout` mapped to timeout and `Disconnected` mapped to shutdown;
- caller-thread policy preparation and original `ToolCallError` preservation;
- mutable-context commit only when `response.ok` and cancellation remains false;
- `begin_running` and `active_blocking_handlers` accounting inside the blocking closure;
- mutex-linearized timeout and completion transitions;
- fixed worker count and bounded synchronous queue;
- pinned `cargo-deny` version in CI.

If implementation work changes any of those areas, the subagent must explain why and add direct regression coverage. Do not opportunistically refactor functioning runtime code.

---

# Required sequencing

Execute this plan in the following order:

1. lock the baseline and inspect current workflows/tests;
2. replace one-way lifecycle notifications with exact test gates where ordering must be controlled;
3. remove all shared mutable blocking controls from parallel lifecycle tests;
4. rewrite the exact lifecycle interleaving tests;
5. complete the real `N+1` worker-bound test;
6. extract and test the production mutable-context execution helper;
7. extract and test the production reply-wait classification helper;
8. run focused tests repeatedly and in ordinary parallel mode;
9. create the implementation commit and record its full SHA;
10. run all local gates from a clean checkout of that SHA;
11. run ordinary CI and all required manual workflows against that SHA;
12. record run IDs, URLs, job conclusions, artifacts, and checksums;
13. create a documentation-only evidence commit;
14. run ordinary CI on the final evidence-recording head;
15. close Release 4/5 status only when every required item is verified.

Do not update closure checkboxes before the corresponding evidence exists.

---

# Workstream 1 — Add exact transition gates

## Objective

Tests must be able to stop execution before and after the lifecycle transitions under review. A notification that merely reports that code passed a point is insufficient when the test needs to choose which side wins the race.

## Required model

Introduce test-only gate primitives with separate arrival and release phases.

Two forms are useful because some hook sites execute in async code and others execute inside `spawn_blocking`.

### Example async gate

```rust
#[cfg(test)]
#[derive(Clone, Default)]
struct AsyncTestGate {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[cfg(test)]
impl AsyncTestGate {
    async fn arrive_and_wait(&self) {
        self.entered.notify_one();
        self.release.notified().await;
    }

    async fn wait_until_entered(&self) {
        self.entered.notified().await;
    }

    fn release(&self) {
        self.release.notify_one();
    }
}
```

### Example blocking gate

Do not block a Tokio worker with a `Condvar`. The blocking form is only for code already running inside `spawn_blocking`.

```rust
#[cfg(test)]
#[derive(Clone)]
struct BlockingTestGate {
    entered: Arc<tokio::sync::Notify>,
    state: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
}

#[cfg(test)]
impl BlockingTestGate {
    fn arrive_and_wait(&self) {
        self.entered.notify_one();
        let (lock, cv) = &*self.state;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = cv.wait(released).unwrap();
        }
    }

    async fn wait_until_entered(&self) {
        self.entered.notified().await;
    }

    fn release(&self) {
        let (lock, cv) = &*self.state;
        *lock.lock().unwrap() = true;
        cv.notify_all();
    }
}
```

Equivalent channel- or barrier-based designs are acceptable. The essential contract is:

1. code signals that it reached the boundary;
2. code stops at that boundary;
3. the test performs assertions or releases the competing side;
4. the test explicitly releases the stopped code.

## Required hook sites

Provide exact gates for at least these points:

- `before_begin_running`: inside the blocking closure, after closure entry but before lifecycle `begin_running`;
- `running_established`: after `begin_running` and active-handler accounting are complete;
- `before_timeout_record`: after caller timeout and cancellation signaling, but before `record_timeout` takes the lifecycle lock;
- `timeout_recorded`: after `record_timeout` completes;
- `before_finish`: after handler return or caught panic, but before lifecycle `finish`;
- `finished`: after lifecycle completion and gauge correction.

A separate permit-acquired notification may remain for diagnostics, but it does not substitute for `before_begin_running`.

## Hook safety requirements

- Production behavior with no hooks must remain allocation-light and unchanged.
- No test gate may be held while the lifecycle mutex is held.
- No `.await` may occur while the lifecycle mutex is held.
- Blocking gates may only wait inside `spawn_blocking` or test-owned OS threads.
- Async gates must not use `std::thread::sleep`.
- Hook state must be per invocation, not global.

## Remove shared blocking state

Delete the legacy `TEST_HANDLER_SHOULD_BLOCK` and `TEST_HANDLER_RELEASED` controls after migrating their remaining users.

The statement “single-handler test” does not make a shared static safe: Rust tests execute concurrently unless explicitly serialized, and another test may mutate the same static.

Use one of:

- per-invocation gate state carried through a test-only handler slot;
- unique slot IDs with no slot reused by concurrently runnable tests;
- a test-only handler dispatcher whose state is keyed by invocation ID.

Do not leave a shared slot such as slot `0` used by multiple parallel tests.

## Acceptance criteria

- Tests can stop the closure before `begin_running`.
- Tests can stop the timeout branch before `record_timeout`.
- Tests can stop completion before `finish`.
- No lifecycle race test depends on scheduler luck to determine the winner.
- No lifecycle test shares mutable blocking controls with another test.
- Production calls use the no-hook path with no behavior change.

---

# Workstream 2 — Rewrite lifecycle tests as exact interleavings

## Test structure rule

Race tests must run the coordinator in a spawned task so the test can interact with gates while the invocation is in progress.

Example pattern:

```rust
let call = tokio::spawn(execute_tool_bounded_with_hooks(...));
gate.wait_until_entered().await;
// Assert state or release competing path.
gate.release();
let outcome = call.await.unwrap();
```

Do not call `execute_tool_bounded_with_hooks(...).await` first and then attempt to control a boundary that has already passed.

## Test A — Timeout after permit but before lifecycle start

Required sequence:

1. use a semaphore with one permit;
2. install a blocking `before_begin_running` gate;
3. spawn the coordinator with a short but nonzero budget;
4. wait until the blocking closure reaches `before_begin_running`;
5. do not release the gate until the caller-facing timeout has returned;
6. assert the timeout response was returned;
7. assert `total_timeouts == 1`;
8. assert `active_blocking_handlers == 0` and `timed_out_handlers == 0` while the closure remains gated;
9. release `before_begin_running`;
10. prove the actual handler body was never invoked;
11. wait for the detached closure to finish via the `finished` hook;
12. assert all gauges remain zero except cumulative timeout count.

This is the exact proof missing from the current zero-permit test.

## Test B — Completion wins the timeout-record race

Required sequence:

1. start a handler that pauses at `before_finish`;
2. allow it to establish `Running`;
3. allow the deadline to expire;
4. pause the timeout branch at `before_timeout_record`;
5. release `before_finish` so the lifecycle transitions to `Finished`;
6. wait for the `finished` hook;
7. release `before_timeout_record`;
8. `record_timeout` must observe `Finished` and must not increment `timed_out_handlers`;
9. the caller still receives a timeout because its deadline expired;
10. final gauges must be zero.

This test proves that completion cannot be overwritten by late timeout accounting.

## Test C — Timeout wins the completion race

Required sequence:

1. start a handler and pause it at `before_finish`;
2. wait for `running_established`;
3. allow timeout recording to complete;
4. wait for `timeout_recorded`;
5. assert `active_blocking_handlers == 1` and `timed_out_handlers == 1`;
6. release `before_finish`;
7. wait for `finished`;
8. assert both gauges return exactly to zero.

No sleep may be used to decide when timeout recording has completed.

## Test D — Panic after timeout

Use a test handler that pauses, then panics when released.

Required sequence:

1. handler reaches `Running`;
2. timeout transition completes;
3. assert both running gauges equal one;
4. release the handler to panic;
5. wait for lifecycle completion;
6. assert both gauges return to zero;
7. verify panic is converted to the documented internal-error response when the caller has not already returned a timeout, and does not kill the blocking pool thread.

## Test E — Cooperative cancellation visibility

Use a handler that waits until the timeout flag becomes true and then exits. Do not implement this with a fixed 200 ms sleep.

Example:

```rust
while !current_cancel_flag().unwrap().load(Ordering::Acquire) {
    std::hint::spin_loop();
}
```

A bounded watchdog is acceptable to prevent a hung test, but the watchdog must not establish the expected order.

Assert:

- timeout sets the exact flag passed to the handler;
- handler observes it;
- lifecycle gauges return to zero;
- no replacement thread is created.

## Test F — 500 exact interleavings

Run 500 iterations, alternating:

- 250 completion-wins sequences using the exact gates from Test B;
- 250 timeout-wins sequences using the exact gates from Test C.

Requirements:

- no sleep to release or settle a handler;
- unique per-iteration gate state;
- no global/shared slot;
- exact expected outcome count: 250 timeout responses and 250 selected completion outcomes according to the test design;
- gauges asserted at quiescence after every iteration;
- peak worker count never exceeds the configured semaphore size.

If 500 full coordinator invocations make ordinary debug CI unreasonably slow, use 100 in ordinary unit CI and a separate ignored/manual stress test for 500, but the closure evidence must accurately state which count ran in which gate. Do not claim 500 when only 100 ran.

## Test G — Real `N+1` worker bound

The current test starts only `N` handlers. Extend it to prove the extra invocation remains queued.

Required sequence:

1. configure a shared semaphore with `N = 3`;
2. start three handlers, each gated after `Running`;
3. wait until all three are running;
4. assert active and peak concurrency equal three;
5. start a fourth coordinator invocation with a generous timeout;
6. prove the fourth invocation has not reached `running_established` while all three permits remain occupied;
7. use a short bounded non-event observation only for this negative assertion;
8. release exactly one running handler;
9. wait until the fourth invocation reaches `running_established`;
10. assert active concurrency remains three, not four;
11. release remaining handlers;
12. join every coordinator task;
13. assert final gauges are zero and peak concurrency is exactly three.

## Test H — Parallel-suite stability

Run the lifecycle tests under ordinary parallel execution. They must not require `--test-threads=1` for correctness.

Also retain a single-threaded repeated gate as supplementary evidence.

Required commands:

```bash
cargo test --locked --all-features --lib mcp::execution::deterministic_tests
cargo test --locked --all-features --lib mcp::execution::deterministic_tests -- --test-threads=1

for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::execution::deterministic_tests || exit 1
done
```

## Acceptance criteria

- Exact winner selection is controlled by gates, not milliseconds.
- Timeout-before-lifecycle-start is directly forced.
- Completion-wins and timeout-wins both cross the same lifecycle lock boundary.
- `N+1` is submitted and observed queued.
- No mutable test control is shared across concurrently runnable tests.
- The suite passes 100 repeated ordinary-parallel runs.
- Evidence labels any remaining timing-based tests as smoke tests only.

---

# Workstream 3 — Test mutable commit semantics through shared production code

## Current problem

The existing generic commit/rollback tests contain a test helper that copies the commit-slot algorithm from `call_json_with_execution_context_mut`.

A copied implementation can continue passing after the production method regresses. Tests must call the same production helper used by the public method.

## Required refactor

Extract the post-policy execution portion of the mutable API into one private helper, for example:

```rust
fn execute_prepared_handler_with_execution_context_mut(
    handler: registry::ToolHandler,
    args: Value,
    ctx: &mut ExecutionContext,
    effective_budget: ToolBudget,
    tool_name: &str,
) -> Result<ToolResponse, ToolCallError>;
```

The public method must perform:

1. effective policy resolution;
2. caller-thread preflight;
3. tool/budget resolution;
4. input-size validation;
5. one call to the shared production helper.

The shared helper must own the actual:

- cancellation identity;
- worker-pool submission;
- thread-local cancel/eval-context installation;
- commit slot;
- `response.ok && !cancelled` predicate;
- late-completion discard behavior;
- runtime error conversion.

Tests may call this private helper with a test handler. They must not reproduce its body.

## Required tests

Use a generic test handler that mutates a known register through `current_eval_context()` and then returns behavior controlled by arguments.

Required cases:

1. **Success commits:** handler mutates register and returns `ok=true`; caller context contains the mutation.
2. **Tool failure rolls back:** handler mutates then returns `ok=false`; caller context is unchanged.
3. **Pre-cancelled rolls back:** effective flag starts true; handler is never invoked and context is unchanged.
4. **Running cancellation rolls back:** handler mutates its local context, waits for cancellation, returns; caller context is unchanged.
5. **Timeout rolls back:** caller receives timeout; late worker completion cannot mutate caller context.
6. **Queue saturation rolls back:** no worker execution and no caller mutation.
7. **Panic rolls back:** caught panic does not commit worker context.
8. **Public wrapper preserves policy:** at least one direct test still calls `call_json_with_execution_context_mut` and verifies profile/audience/schema preflight behavior.

## Trouble-area example

Do not write this in the test:

```rust
// Bad: duplicates production logic.
let commit_allowed = response.ok && !cancel.load(Ordering::Acquire);
if commit_allowed { /* copy slot into ctx */ }
```

Instead:

```rust
// Good: test invokes the shared helper used by the public API.
let response = execute_prepared_handler_with_execution_context_mut(
    test_mutating_handler,
    args,
    &mut ctx,
    budget,
    "test_mutating_handler",
)?;
```

## Acceptance criteria

- No test-only helper duplicates the commit-slot algorithm.
- Public mutable dispatch and transaction tests use the same production execution helper.
- Failure, cancellation, timeout, saturation, and panic cannot commit state.
- Late worker writes remain detached after timeout.
- `math_eval` clone semantics remain documented as a separate limitation.

---

# Workstream 4 — Test production reply classification

## Current problem

The present disconnection test drops a sender on a standalone channel and proves that the standard library returns `RecvTimeoutError::Disconnected`. It does not invoke the production mapping used by `SyncExecutionPool::submit_cancellable`.

## Required refactor

Extract reply waiting and classification into a small production helper used directly by `submit_cancellable`:

```rust
fn wait_for_reply(
    reply_rx: &std::sync::mpsc::Receiver<ToolResponse>,
    timeout: Duration,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<ToolResponse, SyncPoolError> {
    match reply_rx.recv_timeout(timeout) {
        Ok(response) => Ok(response),
        Err(RecvTimeoutError::Timeout) => {
            cancel_flag.store(true, Ordering::SeqCst);
            Err(SyncPoolError::Timeout)
        }
        Err(RecvTimeoutError::Disconnected) => Err(SyncPoolError::Shutdown),
    }
}
```

Equivalent ownership signatures are acceptable.

`submit_cancellable` must call this helper. Unit tests must call this helper with real receivers representing:

- successful reply;
- timeout with sender retained and no message;
- disconnected sender;
- timeout sets cancellation;
- disconnection does not falsely set cancellation unless separately required by shutdown policy.

Do not retain a test whose only assertion is that `std::sync::mpsc` has a `Disconnected` variant.

## Optional stronger pool test

A test-only worker mode may deliberately drop a job reply without sending, allowing the public pool submission path to return `Shutdown`. Add this only if it remains narrow and does not complicate production construction.

The extracted production helper is sufficient if it is the sole classification path.

## Acceptance criteria

- Production submission uses the tested reply-classification helper.
- Timeout and disconnection tests exercise that helper.
- Timeout sets cancellation before returning.
- Disconnection returns `Shutdown`, not `Timeout`.
- Queue-full behavior remains distinct and does not set cancellation.

---

# Workstream 5 — Reconcile tests and documentation claims

## Test naming

Retain timing-based tests only as smoke or behavior tests. Rename or describe them accordingly.

Examples:

- `timeout_smoke_returns_while_handler_continues`;
- `queued_timeout_smoke_does_not_run_handler`;
- `panic_cleanup_smoke`.

Do not use names such as `deterministic_*`, `exact_*`, or `controlled_interleavings` unless gate control actually selects the transition ordering.

## Evidence claims

The closure document must not claim:

- all races use hooks if some race ordering still relies on timeout duration;
- `N+1` queueing if the fourth invocation is not submitted;
- exact clean-checkout evidence when commands ran in an uncommitted working tree;
- a test count written as `481+`;
- 500 iterations when the executed gate ran fewer;
- current-head evidence using a SHA that predates cited tests.

## Exact test inventory

Generate the final test names from the implementation rather than copying an older list.

Record exact counts from command output. No approximate count markers are allowed.

## Acceptance criteria

- Test names and comments match what each test actually proves.
- Evidence distinguishes smoke tests from exact transition tests.
- Every cited test exists in the code-under-test SHA.
- No approximate test counts remain.

---

# Workstream 6 — Establish an exact code-under-test commit

## Implementation commit

After Workstreams 1–5 pass focused testing:

1. ensure the tree contains all code and test changes;
2. run formatting;
3. commit the implementation and tests;
4. record the full 40-character SHA as `CODE_SHA`;
5. do not edit evidence yet.

Suggested commit message:

```text
test(runtime): close exact lifecycle and transaction proof gaps
```

## Clean-checkout requirement

Create a fresh checkout or worktree at `CODE_SHA`:

```bash
CODE_SHA=$(git rev-parse HEAD)
git status --short

git worktree add ../eggsact-verification "$CODE_SHA"
cd ../eggsact-verification

test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$CODE_SHA"
```

All local evidence commands must run in this clean worktree.

Do not claim clean-checkout verification if:

- generated files were modified before commands ran;
- the worktree had untracked files affecting tests;
- commands ran against a later documentation commit;
- `Cargo.lock` or generated docs changed during verification.

---

# Workstream 7 — Local verification gates

## Focused lifecycle gates

```bash
cargo test --locked --all-features --lib mcp::execution::deterministic_tests
cargo test --locked --all-features --lib mcp::execution::deterministic_tests -- --test-threads=1

for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::execution::deterministic_tests || exit 1
done
```

Record:

- exact number of tests per run;
- total loop iterations;
- whether any retry was needed;
- elapsed time if useful.

## Focused synchronous gates

```bash
cargo test --locked --all-features --lib mcp::sync_pool
cargo test --locked --all-features sync_policy
cargo test --locked --all-features context_isolation
```

## Canonical release gate

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

Do not omit `cargo publish --dry-run` from evidence. This does not publish the crate.

## MSRV gate

Use the declared MSRV without raising it:

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

## Fuzz build

```bash
RUSTUP_TOOLCHAIN=nightly cargo fuzz build
```

Record exact nightly and `cargo-fuzz` versions.

## Local gate acceptance criteria

- Every command runs against `CODE_SHA` in a clean checkout.
- Exact test counts are recorded.
- No command is reported as passing based solely on a previous working tree.
- `git status --porcelain` remains empty after verification.
- `cargo publish --dry-run` passes without publishing.

---

# Workstream 8 — GitHub Actions and manual evidence

## Discover actual workflow names

Do not invent workflow names. Inspect the repository:

```bash
gh workflow list
ls .github/workflows
```

At minimum, identify:

- ordinary CI;
- release verification;
- extended fuzz matrix;
- sanitizer matrix or the repository's documented equivalent;
- parity/latest-compatible workflows required by Release 4 or Release 5.

## Ordinary CI on `CODE_SHA`

The implementation commit must be pushed so ordinary CI runs against `CODE_SHA`.

Record using commands equivalent to:

```bash
gh run list \
  --commit "$CODE_SHA" \
  --limit 100 \
  --json databaseId,workflowName,event,status,conclusion,headSha,url
```

For each required CI job, record:

- workflow run ID;
- URL;
- exact head SHA;
- event type;
- final conclusion;
- every job name and conclusion.

Do not treat an empty status API response as proof that CI did not run. Use GitHub Actions run and job APIs or `gh` directly.

## Manual workflows against exact code

Run manual workflows before creating the evidence commit, while `main` still points to `CODE_SHA`, or create a temporary verification branch pointing exactly to `CODE_SHA`.

Example:

```bash
git branch verification/runtime-closure "$CODE_SHA"
git push origin verification/runtime-closure

gh workflow run release-verification.yml \
  --ref verification/runtime-closure
```

Use the actual workflow filenames and supported inputs.

Verify every run reports `head_sha == CODE_SHA` or the exact temporary branch SHA.

Delete the temporary verification branch after evidence is safely recorded, unless repository policy retains evidence branches.

## Required workflow evidence

Record actual values for:

- ordinary CI run;
- manual release-verification run;
- provenance artifact name, artifact ID, and checksum;
- extended fuzz matrix run and every target conclusion;
- sanitizer matrix run and every configuration conclusion;
- parity/latest-compatible conclusions required by release status plans;
- Windows, macOS, Linux, and MSRV jobs where applicable.

## Artifact checksum example

```bash
gh run download "$RUN_ID" --dir /tmp/eggsact-artifacts
find /tmp/eggsact-artifacts -type f -print0 | sort -z | xargs -0 shasum -a 256
```

Record the checksum algorithm and exact artifact filename.

## Missing sanitizer workflow

If prior release plans require sanitizer evidence but no workflow exists:

1. confirm that absence from repository history and plans;
2. add one narrowly scoped manual-only sanitizer workflow;
3. use the existing nightly policy and supported Linux target;
4. do not make sanitizer execution part of every ordinary push unless already intended;
5. document exact commands and known platform limitations;
6. run it against `CODE_SHA` or a new implementation SHA containing the workflow.

Adding a required workflow changes `CODE_SHA`; repeat clean verification against the new SHA.

## Workflow acceptance criteria

- Every required workflow has a successful run tied to the exact implementation SHA.
- Every matrix entry is successful or explicitly documented as an approved non-applicable case.
- No required evidence is represented by a pending, skipped-without-justification, cancelled, or missing run.
- Artifacts are downloaded and checksummed.
- Ordinary CI also passes on the final evidence-recording commit.

---

# Workstream 9 — Rewrite closure evidence truthfully

## Required evidence identity fields

The final evidence document must contain separate identities:

```text
Code-under-test SHA: <full implementation SHA>
Evidence-recording SHA: <full documentation commit SHA, added after commit when known>
Final-head CI SHA: <full final main SHA>
```

The code-under-test SHA must contain every source file and every test cited by the document.

## Required local evidence fields

Record:

- clean-checkout command and result;
- stable Rust version;
- MSRV toolchain version;
- nightly version;
- `cargo-fuzz` version;
- `cargo-deny` version;
- every command exactly as executed;
- exact test count for each partition;
- exact deterministic stress iteration count;
- clean `git status` after commands.

## Required GitHub evidence fields

Record:

- workflow name;
- run ID;
- run URL;
- event;
- head SHA;
- run conclusion;
- job/matrix conclusions;
- artifact IDs/names;
- artifact SHA-256 checksums.

## Closure checkbox policy

A checkbox may be marked complete only when the evidence is in the same document or directly referenced by immutable run/artifact identity.

Leave these open if missing:

- ordinary CI against code-under-test;
- ordinary CI against final evidence head;
- release verification;
- extended fuzz matrix;
- sanitizer matrix;
- provenance artifact checksum;
- Release 4 closure;
- Release 5 closure.

## Evidence commit sequence

1. create evidence content with `CODE_SHA` and all completed run details;
2. commit as documentation-only;
3. record `EVIDENCE_SHA`;
4. update the document once, if necessary, to include the evidence-recording SHA using a second documentation-only commit;
5. clearly distinguish the commit that contains evidence from the code-under-test;
6. run ordinary CI on final `main`;
7. record final-head CI without implying manual workflows reran against the docs-only head unless they did.

Avoid an infinite self-referential SHA update. It is acceptable to state:

```text
Evidence-recording commit: the commit containing this document; see repository history.
Final evidence-finalization SHA: <full SHA of the one subsequent metadata commit>
```

Alternatively place immutable run data in a separate evidence artifact whose checksum is recorded before the final documentation commit.

## Acceptance criteria

- Evidence no longer names `54bd7b9...` if later cited tests are required for closure.
- No test count contains `+`, `approximately`, or similar ambiguity.
- Every test named in evidence exists at `CODE_SHA`.
- Local evidence explicitly says clean checkout and proves it.
- Workflow run IDs and artifact checksums are present.
- Final-head ordinary CI is green.

---

# Workstream 10 — Close Release 4 and Release 5 status

## Required process

Locate the status/plan files defining Release 4 and Release 5 closure. Do not infer their criteria from memory.

For each release:

1. list every remaining criterion;
2. link it to exact evidence in the final closure document;
3. mark it complete only after the relevant workflow or local gate succeeds;
4. preserve any intentionally deferred non-blocking item as deferred, not completed;
5. keep direct crates.io publication outside CI.

## Release closure decision table

Use a table similar to:

| Criterion | Evidence identity | Result |
|---|---|---|
| Ordinary CI | run ID / URL / SHA | pass/fail |
| Release verification | run ID / URL / SHA | pass/fail |
| Provenance artifact | artifact ID / SHA-256 | pass/fail |
| Extended fuzz | run ID / target matrix | pass/fail |
| Sanitizers | run ID / matrix | pass/fail |
| Package and publish dry-run | local command / SHA | pass/fail |
| Maintainer direct-publish readiness | explicit maintainer decision | ready/open |

## Acceptance criteria

- Release 4 and Release 5 status files match the evidence document.
- No release is marked complete with an unchecked evidence dependency.
- Crates.io publishing remains a direct maintainer action.
- The repository has one unambiguous final release-readiness statement.

---

# Verification checklist

## Implementation and tests

- [ ] Exact async/blocking test gates exist.
- [ ] Test gates pause, not merely notify.
- [ ] Timeout-before-`begin_running` is directly forced.
- [ ] Completion-wins is forced at the timeout lifecycle lock.
- [ ] Timeout-wins is forced before completion lifecycle lock.
- [ ] Panic-after-timeout uses exact gates.
- [ ] Cooperative cancellation exits without a fixed sleep.
- [ ] 500 interleavings use per-invocation gates, or evidence accurately records a smaller ordinary-CI count plus separate stress count.
- [ ] Worker-bound test submits and observes `N+1`.
- [ ] No parallel lifecycle test uses shared mutable statics or shared slots.
- [ ] Mutable transaction tests call shared production code.
- [ ] Reply-disconnection tests call the production classification helper.

## Local verification

- [ ] Implementation SHA recorded.
- [ ] Clean worktree created at implementation SHA.
- [ ] Focused lifecycle suite passes normally and single-threaded.
- [ ] Focused lifecycle suite passes 100 repeated normal-parallel runs.
- [ ] Sync-pool and policy suites pass.
- [ ] Full release gate passes.
- [ ] MSRV gate passes.
- [ ] Fuzz build passes.
- [ ] Package and publish dry-run pass.
- [ ] Exact counts and versions recorded.
- [ ] Verification worktree remains clean.

## GitHub evidence

- [ ] Ordinary CI passes against implementation SHA.
- [ ] Release-verification workflow passes against implementation SHA.
- [ ] Extended fuzz matrix passes.
- [ ] Sanitizer matrix passes.
- [ ] Required parity/latest-compatible workflows pass.
- [ ] Provenance artifacts downloaded and checksummed.
- [ ] Final documentation head passes ordinary CI.

## Documentation and release status

- [ ] Code-under-test SHA contains every cited test.
- [ ] Evidence-recording identity is explicit.
- [ ] No approximate counts remain.
- [ ] Timing smoke tests are not called deterministic.
- [ ] Release 4 criteria are reconciled.
- [ ] Release 5 criteria are reconciled.
- [ ] No crates.io publication was performed by CI.

---

# Suggested commit sequence

Use a compact sequence so evidence provenance remains understandable.

1. `test(runtime): add exact lifecycle transition gates`
2. `test(agent): exercise shared mutable transaction and reply helpers`
3. `docs(release): record exact runtime closure evidence`
4. Optional metadata-only finalization: `docs(release): add final evidence commit identity`

Combining the first two commits is acceptable if all changes remain narrowly scoped. Do not interleave unrelated cleanup.

---

# Subagent stop conditions

Stop and report rather than marking closure when any of these occurs:

- an exact gate test flakes under ordinary parallel execution;
- the `N+1` invocation reaches `Running` while all permits are occupied;
- a timeout, failed response, cancellation, saturation, or panic commits mutable context;
- a disconnected reply maps to timeout;
- local verification changes tracked files;
- CI runs against a SHA other than the claimed code-under-test;
- an expected fuzz or sanitizer matrix entry is missing;
- an artifact cannot be downloaded or checksummed;
- Release 4/5 criteria conflict with the closure document;
- closing the work would require publishing to crates.io through CI.

A partial successful implementation is not sufficient to mark this plan complete. Leave exact unchecked criteria and record the blocking evidence gap.

---

# Definition of done

This line of work is complete only when all of the following are true:

1. production runtime behavior remains correct and unchanged from the corrected baseline;
2. lifecycle winner selection is proven with gates at the exact transition boundaries;
3. the timeout-before-lifecycle-start case is directly tested;
4. worker-bound testing includes a real queued `N+1` invocation;
5. transaction and reply classification tests exercise shared production helpers;
6. all focused and canonical local gates pass from a clean exact-SHA checkout;
7. ordinary CI, release verification, extended fuzzing, sanitizers, and required compatibility workflows pass against the exact implementation SHA;
8. provenance artifacts are identified and checksummed;
9. the final evidence document cites only tests present at its code-under-test SHA;
10. final-head ordinary CI passes after evidence is committed;
11. Release 4 and Release 5 status files are reconciled with actual evidence;
12. crates.io publication remains an explicit direct maintainer operation.

Until every item above is satisfied, describe the repository as implementation-complete but verification/evidence-open rather than fully closed.
