# Final Proof and Release-Evidence Closure Pass

## Status

- **Status:** completed
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `85eccc600f42c279aff4fdec9f973960b6c23a30`
- **Final evidence baseline:** `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`
- **Completion evidence:** `docs/releases/2026-07-final-closure-evidence.md`
- **Scope:** remaining proof-quality, test-isolation, and release-evidence closure only
- **Predecessor plans:**
  - `plans/2026-07-23-final-cancellation-lifecycle-evidence-closure-pass.md`
  - `plans/2026-07-24-final-verification-evidence-closure-pass.md`
- **Current implementation assessment:** production runtime architecture is complete; remaining work is narrowly limited to proving final guarantees and recording exact current-head evidence

## Purpose

The timeout lifecycle, synchronous execution pool, cancellation identity, policy preflight, and mutable-context transaction model are now implemented in the intended architecture. The remaining work does not justify another design rewrite.

This pass closes the final gaps identified after the implementation and subsequent test-stability corrections:

1. the timeout-before-lifecycle-start test still settles detached closure completion with a sleep;
2. the `CancelledBeforeStart` return path does not publish an unconditional closure-exit signal;
3. panic-after-timeout still uses a handler sleep to establish the running-timeout ordering;
4. two queued synchronous timeout tests do not make a final post-dequeue assertion that the expired/cancelled handler never ran;
5. comments in those tests incorrectly describe late invocation as acceptable despite the production preflight guarantee;
6. lifecycle test handlers still rely on manually assigned shared static slot indices;
7. one test heading contains an incorrect 250/250 description for a 50/50 ordinary-CI test;
8. closure evidence identifies `6216d82...`, but current `main` contains later test correctness fixes at `c1d4f5c...` and `85eccc6...`;
9. ordinary CI, release verification, extended fuzzing, sanitizers, provenance artifacts, and Release 4/5 status remain unverified or open.

The goal is a small, reliable closure pass that leaves no material implementation, test-proof, or evidence ambiguity.

---

# Non-goals

This pass must not:

- redesign `HandlerLifecycle`;
- replace the mutex lifecycle with atomics;
- change the public MCP protocol;
- change tool outputs, schemas, profiles, audiences, or compatibility behavior;
- alter fixed synchronous worker-count or queue-capacity policy;
- add detached per-invocation worker threads;
- move MCP dispatch onto the synchronous pool;
- change direct crates.io publishing policy;
- raise MSRV;
- opportunistically refactor unrelated code;
- mark a release complete before exact workflow evidence exists;
- delete useful smoke tests merely because they are timing-based;
- describe a timing-based smoke test as an exact interleaving proof.

If a production defect is discovered while implementing this plan, stop and document the defect before broadening scope.

---

# Current production guarantees to preserve

The subagent must preserve these current guarantees:

- lifecycle state and gauge changes are serialized under one lifecycle mutex;
- `begin_running` occurs inside the `spawn_blocking` closure;
- permit acquisition alone does not publish `Running`;
- a timeout observed before `begin_running` transitions `Queued -> TimedOutQueued`;
- `CancelledBeforeStart` prevents handler invocation;
- running timeout increments `timed_out_handlers` exactly once;
- completion after running timeout decrements both running gauges exactly once;
- panic is caught and lifecycle cleanup still occurs;
- bounded registry APIs use one cancellation flag for pool timeout and handler observation;
- queued synchronous jobs check deadline and cancellation before handler invocation;
- reply timeout and channel disconnection are classified separately;
- mutable context commits only on `response.ok && !cancelled`;
- late worker completion cannot mutate caller state after timeout;
- policy and schema errors remain caller-thread `ToolCallError` values;
- `cargo-deny` remains pinned;
- crates.io publication remains a direct maintainer operation.

---

# Required execution order

Execute in this order:

1. add unconditional closure-exit observation;
2. rewrite the pre-start timeout test to use it;
3. rewrite panic-after-timeout as a fully gate-controlled interleaving;
4. strengthen queued synchronous timeout tests with final post-dequeue assertions;
5. remove contradictory comments and duplicated weak tests;
6. replace manual static slot assignment with safe lease/allocation semantics, or eliminate slots from lifecycle tests;
7. correct test names/headings and documentation;
8. run focused tests repeatedly under ordinary parallel execution;
9. commit all code/test corrections and record the exact implementation SHA;
10. verify from a clean checkout of that SHA;
11. run ordinary and manual GitHub workflows against that SHA;
12. record exact run/artifact evidence;
13. update Release 4/5 status only after evidence passes;
14. commit evidence and verify final-head CI.

No closure checkbox may be updated before its proof exists.

---

# Workstream 1 — Add unconditional closure-exit synchronization

## Problem

The current `finished` gate runs only after `HandlerLifecycle::finish`. The `CancelledBeforeStart` branch returns before `finish`, because no active-handler increment occurred and no lifecycle decrement is needed.

As a result, the exact timeout-before-`begin_running` test currently releases `before_begin_running` and then sleeps to give the detached closure time to exit. That proves the important state transition, but not closure termination deterministically.

## Required solution

Add a test-only signal that fires on every `spawn_blocking` closure exit, including:

- `CancelledBeforeStart`;
- cancellation check before handler invocation;
- normal handler return;
- tool-level failure;
- caught panic;
- any future early return.

Preferred design: an RAII exit notifier created as the first value inside the blocking closure.

### Example

```rust
#[cfg(test)]
struct ClosureExitGuard {
    gate: Option<Arc<ExitSignal>>,
}

#[cfg(test)]
impl Drop for ClosureExitGuard {
    fn drop(&mut self) {
        if let Some(gate) = &self.gate {
            gate.signal();
        }
    }
}
```

A blocking gate that waits for release is not necessary for this point. A one-way completion signal is preferable because the closure should not remain artificially occupied after all accounting is complete.

Possible signal implementation:

```rust
#[derive(Clone, Default)]
struct ExitSignal {
    exited: Arc<tokio::sync::Notify>,
}

impl ExitSignal {
    fn signal(&self) {
        self.exited.notify_one();
    }

    async fn wait(&self) {
        self.exited.notified().await;
    }
}
```

Equivalent channel, latch, or atomic-plus-notify designs are acceptable.

## Hook contract

Add an optional `closure_exited` hook/signal to `ExecutionHooks`.

It must:

- fire exactly once per blocking closure;
- fire on every return path;
- fire after any required lifecycle cleanup for paths that entered `Running`;
- fire without holding the lifecycle mutex;
- never block the closure;
- be no-op in production;
- not depend on the `finished` gate being released.

## Required test changes

Update `timeout_after_permit_before_lifecycle_start`:

1. pause at `before_begin_running`;
2. allow caller timeout to complete;
3. assert queued-timeout metrics;
4. release `before_begin_running`;
5. wait for `closure_exited` with a bounded watchdog;
6. assert handler invocation flag is still false;
7. assert final gauges;
8. remove the 50 ms settlement sleep.

The watchdog may fail the test if closure exit never arrives, but must not establish ordering.

## Acceptance criteria

- `closure_exited` fires on `CancelledBeforeStart`.
- `closure_exited` fires after normal completion.
- `closure_exited` fires after caught panic.
- no detached-closure test uses a sleep to infer exit.
- production behavior is unchanged.

---

# Workstream 2 — Make panic-after-timeout fully gate-controlled

## Problem

The current panic test sleeps inside the handler and relies on a shorter timeout to establish that the timeout happens while the handler is running. This is scheduler-sensitive on heavily loaded CI hosts.

## Required sequence

Create a panic handler controlled by per-invocation test state. Do not use a fixed sleep to decide when it panics.

Required interleaving:

1. start the coordinator with a generous watchdog timeout;
2. pause at `running_established` before handler execution proceeds, or use a handler-entry gate immediately after running state is established;
3. release `running_established` so the handler reaches a dedicated panic gate;
4. wait until the handler reports it is ready to panic;
5. allow the caller deadline to expire and pause at `before_timeout_record`;
6. release `before_timeout_record`;
7. wait for `timeout_recorded`;
8. assert `active_blocking_handlers == 1` and `timed_out_handlers == 1`;
9. release the panic gate;
10. handler panics immediately;
11. wait for `finished` and `closure_exited`;
12. assert both gauges are zero;
13. assert caller received timeout;
14. separately retain or add a non-timeout panic test proving panic conversion to `INTERNAL_ERROR`.

## Trouble-area example

Bad:

```rust
std::thread::sleep(Duration::from_millis(50));
panic!("expected");
```

Good:

```rust
TEST_PANIC_READY.signal();
TEST_PANIC_RELEASE.wait();
panic!("expected");
```

The test state must be per invocation, not a globally reused static boolean.

## Acceptance criteria

- no handler sleep establishes the panic/timeout ordering;
- timeout recording is observed before panic release;
- panic cleanup returns gauges to zero;
- caller timeout semantics and non-timeout panic conversion are both tested;
- the test passes under repeated ordinary parallel runs.

---

# Workstream 3 — Prove expired queued sync jobs never execute

## Problem

Production `worker_loop` correctly checks both deadline and cancellation before invoking a dequeued job. However, the tests named to prove this guarantee only assert before the blocking worker is released. They make no final assertion after the expired job is actually dequeued.

Some comments incorrectly say the handler may run later and that this is allowed. That description reflects an older cooperative-only model and contradicts current production behavior.

## Required canonical test

Replace duplicated weak tests with one deterministic canonical test for timeout expiry and one for external cancellation.

### Test A — caller timeout while queued

1. create a one-worker pool;
2. submit a first job that blocks on a deterministic gate;
3. wait until the first job confirms it owns the worker;
4. submit a second job with:
   - an invocation counter initialized to zero;
   - a short timeout;
   - its own cancellation flag;
5. wait for the second submit call to return `SyncPoolError::Timeout`;
6. assert its cancellation flag is true;
7. release the first job;
8. wait until the pool confirms it processed the second queue position.

The final processing confirmation must not be inferred by sleeping. Use one of:

- a third sentinel job submitted after the expired job and wait for sentinel completion;
- a test-only worker dequeue/skip signal;
- a pool drain barrier.

The simplest black-box approach is a sentinel job:

```text
queue order: blocking job -> expired job -> sentinel job
```

After releasing the blocker, wait for sentinel completion. Because the queue is FIFO, the worker necessarily examined the expired job before running the sentinel.

Then assert:

- expired job invocation count is still zero;
- sentinel ran exactly once;
- pool remains usable;
- timeout result remained distinct from shutdown and queue-full.

### Test B — externally cancelled while queued

Use the same structure, but set the second job’s cancellation flag externally before releasing the first worker.

Assert:

- second caller receives the documented response/error behavior;
- handler invocation count remains zero after sentinel completes;
- cancellation remains true;
- pool remains usable.

## Test naming

Use names that state the final guarantee, for example:

- `queued_timed_out_job_is_skipped_before_sentinel`;
- `queued_externally_cancelled_job_is_skipped_before_sentinel`.

Delete or rewrite tests/comments that say late invocation is allowed.

## Acceptance criteria

- assertions occur after the worker has necessarily dequeued the expired/cancelled job;
- handler invocation count remains exactly zero;
- no sleep is used as proof of queue processing;
- comments match the current preflight model;
- duplicate weak tests are removed or converted into clearly labeled smoke tests.

---

# Workstream 4 — Eliminate fragile manual slot assignment

## Problem

The global reset that previously caused cross-test interference has been removed. However, handler blocking still uses shared static arrays and manually assigned numeric slots.

Manual uniqueness is fragile because future tests can accidentally reuse a slot and introduce parallel flakes.

## Preferred solution: test slot leases

Add a test-only slot allocator returning an RAII lease.

### Example

```rust
#[cfg(test)]
struct TestSlotLease {
    index: usize,
}

#[cfg(test)]
impl Drop for TestSlotLease {
    fn drop(&mut self) {
        BLOCK_SLOTS[self.index].store(false, Ordering::SeqCst);
        RELEASE_SLOTS[self.index].store(false, Ordering::SeqCst);
        SLOT_IN_USE[self.index].store(false, Ordering::SeqCst);
    }
}
```

Allocation should atomically claim a free slot:

```rust
fn acquire_test_slot() -> TestSlotLease {
    for index in 0..TEST_BLOCK_SLOTS {
        if SLOT_IN_USE[index]
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return TestSlotLease { index };
        }
    }
    panic!("no test blocking slots available");
}
```

A mutex-protected free list is also acceptable.

## Alternative solution

Eliminate static slots entirely using a test-only invocation registry keyed by a unique ID embedded in arguments. The registry may store per-invocation gates in a mutex-protected map.

Use this only if it remains simpler than slot leases.

## Requirements

- tests must not hardcode slot numbers;
- lease drop resets all slot state;
- double allocation of one slot is impossible;
- test panic still releases the slot through RAII;
- capacity exhaustion produces an explicit failure;
- production code remains unaffected;
- slot allocator itself has a parallel stress test.

## Acceptance criteria

- no lifecycle test contains literals such as `block_slot_args(4)` to claim shared test state;
- ordinary parallel execution cannot reuse an active slot;
- all leased state is reset on drop;
- 100 repeated parallel lifecycle runs pass without slot collision.

---

# Workstream 5 — Correct test descriptions and evidence language

## Required corrections

- Change the ordinary test heading from “100 exact interleavings (250 completion + 250 timeout)” to “100 exact interleavings (50 completion + 50 timeout).”
- Keep the ignored stress test described separately as 500 total, 250/250.
- Remove every statement that an expired queued sync job may execute later.
- Make clear that timing-based smoke tests verify broad behavior only.
- Do not claim that no shared mutable statics exist if static arrays remain; instead state the actual isolation mechanism.
- If slot leases are implemented, state that static backing storage is protected by exclusive RAII leases.
- Correct lifecycle prose that says `begin_running` occurs immediately upon permit acquisition; it occurs inside the blocking closure after permit acquisition.

## Acceptance criteria

- names, headings, comments, and evidence agree with implementation;
- ordinary and ignored stress counts are not conflated;
- no obsolete cooperative-only queue language remains;
- every cited test name exists at the final implementation SHA.

---

# Workstream 6 — Establish a new exact implementation SHA

## Why a new SHA is required

The existing evidence records `6216d82...`, but current `main` contains later test corrections:

- `c1d4f5c...` synchronizes metric cleanup;
- `85eccc6...` removes unsafe global slot resets.

The final closure evidence must refer to a commit containing all final tests and corrections.

## Commit sequence

After Workstreams 1–5 pass focused tests:

1. run formatting;
2. ensure no unrelated changes exist;
3. commit code and tests;
4. record full 40-character `CODE_SHA`;
5. push `CODE_SHA` to `main` or a dedicated verification branch;
6. do not edit closure evidence until verification is complete.

Suggested message:

```text
test(runtime): close final timeout and queue proof gaps
```

## Acceptance criteria

- `CODE_SHA` contains closure-exit signal, panic gates, queue sentinel proofs, slot leases, and corrected descriptions;
- evidence does not cite an earlier commit;
- no test fix is committed after evidence without re-baselining evidence.

---

# Workstream 7 — Focused verification

Run from a clean checkout of `CODE_SHA`.

## Lifecycle tests

```bash
cargo test --locked --all-features --lib mcp::execution::deterministic_tests
cargo test --locked --all-features --lib mcp::execution::deterministic_tests -- --test-threads=1

for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::execution::deterministic_tests || exit 1
done
```

## Sync-pool tests

```bash
cargo test --locked --all-features --lib mcp::sync_pool

for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::sync_pool || exit 1
done
```

## Combined parallel stress

Run both modules under ordinary test scheduling, not serial isolation:

```bash
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib \
    mcp::execution::deterministic_tests \
    mcp::sync_pool || exit 1
done
```

If Cargo filtering cannot express both modules in one invocation, run the full lib suite repeatedly:

```bash
for i in $(seq 1 25); do
  cargo test --locked --all-features --lib || exit 1
done
```

## Ignored 500-iteration test

```bash
cargo test --locked --all-features --lib \
  mcp::execution::deterministic_tests::five_hundred_exact_interleavings \
  -- --ignored --exact
```

Record exact iteration and outcome counts.

## Focused acceptance criteria

- no retries are needed;
- no test requires `--test-threads=1` for correctness;
- closure-exit tests contain no settlement sleeps;
- queued expired/cancelled handlers remain uninvoked after sentinel completion;
- panic-after-timeout is gate-controlled;
- 500 ignored stress iterations pass once at minimum;
- slot lease allocator reports no collision or leak.

---

# Workstream 8 — Full clean-checkout release gate

Create a clean worktree at `CODE_SHA`:

```bash
CODE_SHA=$(git rev-parse HEAD)
git worktree add ../eggsact-final-closure "$CODE_SHA"
cd ../eggsact-final-closure

test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD)" = "$CODE_SHA"
```

Run:

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

MSRV:

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

Fuzz build:

```bash
rustc +nightly --version
cargo fuzz --version
RUSTUP_TOOLCHAIN=nightly cargo fuzz build
```

## Acceptance criteria

- all commands run against exact `CODE_SHA`;
- exact counts are recorded;
- nightly version is recorded rather than left unknown;
- clean worktree remains clean after verification;
- dry-run does not publish.

---

# Workstream 9 — GitHub workflow evidence

## Required ordinary CI

Push `CODE_SHA` and identify the ordinary CI run by exact SHA.

Record:

- workflow name;
- run ID;
- URL;
- event type;
- head SHA;
- every job name and conclusion;
- Windows, macOS, Linux, MSRV, package, generated-docs, and cargo-deny results.

Do not use an empty combined-status response as evidence. Use Actions run/job APIs or `gh run list --commit`.

## Manual release verification

Run the existing manual release-verification workflow against a branch pointing exactly to `CODE_SHA`.

Record:

- run ID and URL;
- exact SHA;
- all jobs and conclusions;
- package/provenance artifact IDs;
- checksums of downloaded artifacts.

## Extended fuzz

Run the repository’s extended fuzz matrix against `CODE_SHA`.

Record every target and conclusion. A successful `cargo fuzz build` is not a substitute for the required extended run.

## Sanitizers

The current evidence states that no sanitizer workflow exists. Resolve this explicitly.

Preferred action:

- add a manual-only Linux sanitizer workflow;
- use the repository’s supported nightly toolchain policy;
- include the applicable sanitizer commands/targets;
- document unsupported sanitizer/platform combinations;
- run against the final implementation SHA.

If adding the workflow changes code-under-test metadata, define the workflow-containing commit as the new `CODE_SHA` and rerun required clean verification.

Do not mark sanitizer closure complete merely because no workflow existed.

## Provenance artifacts

Download required artifacts and record SHA-256:

```bash
find artifacts -type f -print0 | sort -z | xargs -0 shasum -a 256
```

## Acceptance criteria

- ordinary CI is green on exact `CODE_SHA`;
- release verification is green on exact `CODE_SHA`;
- extended fuzz matrix is green;
- sanitizer matrix is green or each unsupported case is explicitly justified;
- artifacts are downloadable and checksummed;
- no required job is pending, cancelled, or silently skipped.

---

# Workstream 10 — Rewrite final closure evidence

Update `docs/releases/2026-07-final-closure-evidence.md` only after implementation and workflow verification.

## Required identity fields

```text
Final code-under-test SHA: <40-character CODE_SHA>
Evidence-recording commit: <documentation commit identity>
Final main head: <40-character final SHA>
```

The final code-under-test must contain every cited test.

## Required corrections

- replace `6216d82...` with the new final implementation SHA;
- record exact current test counts;
- record stable, MSRV, nightly, cargo-fuzz, and cargo-deny versions;
- record the 100-run lifecycle loop result;
- record the 100-run sync-pool loop result;
- record the 500-iteration ignored stress result;
- record clean-checkout proof;
- record ordinary CI run and all jobs;
- record release-verification run;
- record fuzz and sanitizer runs;
- record artifact IDs and SHA-256 checksums;
- update the slot-isolation claim to match actual implementation;
- remove all pending placeholders that are required for closure.

## Evidence accuracy rules

- no approximate counts;
- no `PASS` claim without command or immutable workflow identity;
- no test cited from a later commit than `CODE_SHA`;
- no claim that all tests are deterministic when smoke tests remain timing-based;
- no closure claim while sanitizer or release verification is absent;
- no self-referential infinite SHA editing.

## Documentation commit sequence

1. create evidence document after all runs complete;
2. commit evidence;
3. record evidence commit SHA externally or in a non-self-referential finalization line;
4. run ordinary CI on the evidence commit;
5. record final-head CI in one final metadata commit if necessary;
6. do not make further code/test changes after evidence without re-running verification.

---

# Workstream 11 — Reconcile Release 4 and Release 5

Locate the exact release status files and criteria. Do not close based only on this plan.

For each release, produce a criterion-to-evidence table:

| Criterion | Evidence | Result |
|---|---|---|
| Final code SHA | commit URL/SHA | pass/open |
| Clean local release gate | evidence section | pass/open |
| MSRV | command output | pass/open |
| Ordinary CI | run ID/URL | pass/open |
| Release verification | run ID/URL | pass/open |
| Extended fuzz | run ID/matrix | pass/open |
| Sanitizers | run ID/matrix | pass/open |
| Provenance | artifact ID/SHA-256 | pass/open |
| Publish dry-run | command output | pass/open |
| Direct maintainer publish readiness | explicit maintainer decision | ready/open |

## Acceptance criteria

- Release 4 and Release 5 status match actual evidence;
- no evidence-dependent item remains checked without evidence;
- one unambiguous release-readiness statement exists;
- crates.io publication remains direct and is not performed by CI.

---

# Required test inventory after completion

At minimum, final tests must directly prove:

## MCP lifecycle

- timeout after permit but before lifecycle start;
- unconditional closure exit after `CancelledBeforeStart`;
- completion wins timeout record;
- timeout wins completion;
- panic after recorded timeout;
- cooperative cancellation visibility;
- exact 100-interleaving ordinary test;
- exact 500-interleaving manual stress test;
- real `N+1` worker bound;
- no slot collision under parallel execution.

## Sync pool

- reply success;
- reply timeout sets cancellation;
- reply disconnection maps to shutdown;
- queued timeout job skipped after worker reaches its queue position;
- externally cancelled queued job skipped after worker reaches its queue position;
- queue-full does not set cancellation;
- running non-cooperative work retains worker occupancy;
- worker survives panic;
- pool remains usable after repeated timeouts.

## Mutable context

- success commits;
- tool failure rolls back;
- pre-cancel rolls back;
- running cancellation rolls back;
- timeout rolls back;
- saturation rolls back;
- panic rolls back;
- public wrapper preserves policy preflight.

---

# Stop conditions

Stop and report rather than marking closure if:

- any exact lifecycle test still needs a settlement sleep;
- `closure_exited` fails to fire on an early return;
- panic ordering cannot be forced without timing;
- an expired or cancelled queued handler runs after sentinel completion;
- slot allocation can collide or leak;
- ordinary parallel stress flakes;
- current-head local verification differs from recorded evidence;
- workflow head SHA differs from `CODE_SHA`;
- release-verification, fuzz, or sanitizer runs are unavailable or failing;
- provenance artifacts cannot be downloaded or checksummed;
- a code/test commit lands after evidence without re-verification;
- closing Release 4/5 would require CI-based crates.io publication.

---

# Explicit closure checklist

## Code and tests

- [ ] unconditional `closure_exited` signal implemented;
- [ ] `closure_exited` fires on every blocking-closure return path;
- [ ] pre-start timeout test has no settlement sleep;
- [ ] panic-after-timeout ordering is fully gate-controlled;
- [ ] queued timeout test asserts non-invocation after sentinel completion;
- [ ] queued external-cancel test asserts non-invocation after sentinel completion;
- [ ] obsolete “handler may run later” comments removed;
- [ ] hardcoded slot numbers removed from tests;
- [ ] slot allocator/lease has parallel tests;
- [ ] 100-test heading corrected to 50/50;
- [ ] all focused tests pass under ordinary parallel scheduling.

## Local verification

- [ ] final `CODE_SHA` recorded;
- [ ] clean worktree at `CODE_SHA` verified;
- [ ] lifecycle suite passes normally and single-threaded;
- [ ] lifecycle suite passes 100 repeated ordinary runs;
- [ ] sync-pool suite passes 100 repeated ordinary runs;
- [ ] ignored 500-interleaving stress test passes;
- [ ] full release gate passes;
- [ ] MSRV gate passes;
- [ ] nightly and cargo-fuzz versions recorded;
- [ ] fuzz build passes;
- [ ] package and publish dry-run pass;
- [ ] final worktree remains clean.

## GitHub evidence

- [ ] ordinary CI passes on `CODE_SHA`;
- [ ] release-verification workflow passes on `CODE_SHA`;
- [ ] extended fuzz matrix passes;
- [ ] sanitizer matrix passes;
- [ ] required compatibility/parity jobs pass;
- [ ] provenance artifacts downloaded;
- [ ] SHA-256 checksums recorded;
- [ ] final evidence head passes ordinary CI.

## Documentation and release status

- [ ] evidence names final implementation SHA, not `6216d82...`;
- [ ] evidence includes later test fixes;
- [ ] exact counts and versions recorded;
- [ ] no pending required placeholders remain;
- [ ] Release 4 status reconciled;
- [ ] Release 5 status reconciled;
- [ ] direct crates.io publishing policy preserved.

---

# Suggested commit sequence

1. `test(runtime): add unconditional closure exit and gated panic proof`
2. `test(sync): prove queued timeout and cancellation skip execution`
3. `test(runtime): lease blocking slots for parallel isolation`
4. Optional workflow commit: `ci: add manual sanitizer verification`
5. `docs(release): record final current-head closure evidence`
6. Optional metadata-only finalization: `docs(release): record final evidence head`

Combining the first three is acceptable if the diff remains reviewable. Do not mix unrelated cleanup.

---

# Definition of done

This line of work is complete only when:

1. all production runtime guarantees remain intact;
2. every blocking closure return path is deterministically observable;
3. panic-after-timeout is proven without scheduler-dependent sleeps;
4. expired and externally cancelled queued sync jobs are proven uninvoked after dequeue processing;
5. test-state isolation cannot collide under parallel execution;
6. all focused and full local gates pass from a clean exact-SHA checkout;
7. ordinary CI, release verification, extended fuzz, sanitizers, and required compatibility workflows pass against the exact implementation SHA;
8. provenance artifacts are downloaded and checksummed;
9. final evidence cites only tests contained in the final code-under-test SHA;
10. final evidence head passes ordinary CI;
11. Release 4 and Release 5 status files match actual evidence;
12. crates.io publication remains a direct maintainer operation.

Until all twelve conditions are met, describe the repository as production-implementation complete but final proof/release-evidence open.
