# Final Polish and Exact-Evidence Closure Pass

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `980c4d23cf5560a0aac096631f413aaf03e900b5`
- **CODE_SHA:** `3e5b41c6ac5a8daaba11d5dfacb822f6da033464` (production code at `50f9132`, fuzz target fix at `3e5b41c`)
- **Scope:** test determinism, exact-SHA workflow evidence, and release-document cleanup
- **Runtime redesign:** explicitly out of scope
- **Publication:** explicitly out of scope; crates.io publication remains a direct maintainer action

## Purpose

The runtime-correctness work is substantially complete. The mutex-owned MCP lifecycle, cancellation identity, bounded synchronous pool, transactional mutable-context handling, exact transition gates, closure-exit signaling, queued-work preflight, and reply classification have all landed.

The remaining closure work is narrow but important:

1. two queue-saturation tests still use short sleeps to assume the second job has entered the bounded queue;
2. `two_jobs_run_concurrently` does not actually overlap two submissions;
3. `timed_out_running_retains_worker` accepts either success or saturation and therefore does not prove retained worker occupancy;
4. the extended fuzz/sanitizer run cited as final was executed on `fa6a6e9`, while the final code baseline includes a later production change to `src/text/glob.rs` at `a782006`;
5. Release 5 and release-readiness documents incorrectly describe that earlier run as executing on the exact final SHA;
6. the closure document stores a self-referential “final main head” field, which necessarily becomes stale whenever the field is updated;
7. the final proof should use one frozen code-and-test SHA for ordinary CI, release verification, extended fuzzing, sanitizers, latest-compatible dependencies, and Python parity.

This pass closes those issues without changing public behavior or expanding the project scope.

---

# Non-goals and constraints

This pass must not:

- redesign `HandlerLifecycle` or MCP timeout accounting;
- change tool output, machine-code, schema, profile, audience, or compatibility semantics;
- replace the synchronous worker pool;
- add detached per-request threads;
- change the direct-maintainer crates.io publication policy;
- add an automatic publish job;
- mark an earlier workflow run as exact-SHA evidence when any production source differs;
- use arbitrary sleeps as the primary mechanism for proving queue state or worker occupancy;
- weaken tests to accommodate scheduler-sensitive failures;
- continue updating a document solely to make a “current head” field point at the commit that updated it.

Timing may remain in smoke tests when elapsed-time behavior is the subject of the test. Timing must not be used to establish queue insertion, handler start, handler completion, or worker release when an explicit signal can represent that event.

---

# Current baseline to preserve

The following corrected behavior must remain unchanged:

- queued MCP timeout cannot later invoke the handler;
- running MCP timeout increments and decrements timeout gauges exactly once;
- blocking-closure exit is signalled from an RAII guard on every closure return path;
- test handler slots are acquired through exclusive RAII leases;
- bounded synchronous work checks deadline and cancellation before handler invocation;
- queued timed-out and externally cancelled jobs are skipped before a FIFO sentinel;
- timeout sets the same cancellation flag exposed to the handler;
- reply timeout and channel disconnection remain distinct;
- mutable context commits only on successful, uncancelled completion;
- all public policy checks remain on the caller thread;
- MCP does not invoke the synchronous pool from inside `spawn_blocking`.

Any change to production code beyond correcting a newly reproduced defect requires a new regression test and a new exact-SHA workflow baseline.

---

# Required execution sequence

Execute this plan in this order:

1. inspect the current sync-pool test helpers and workflow dispatch inputs;
2. add an exact queue-enqueued signal to the remaining saturation tests;
3. rewrite the concurrency and retained-worker tests so their names match their proofs;
4. run focused sync-pool tests repeatedly under ordinary parallel execution;
5. run the full local release gate;
6. commit all code and test changes as one frozen `CODE_SHA`;
7. push `CODE_SHA` and wait for ordinary CI;
8. dispatch release verification, extended fuzz/sanitizer, latest-compatible, and Python parity against a branch pointing exactly to `CODE_SHA`;
9. download and checksum required artifacts;
10. update closure and release documents once with exact run identities;
11. remove self-referential head metadata rather than chasing it with more commits;
12. commit documentation as an evidence commit;
13. require ordinary CI to pass on the evidence commit, but do not edit the document again merely to embed that evidence commit's own SHA or run ID.

Do not update closure checkboxes before the relevant run has completed successfully.

---

# Workstream 1 — Deterministic queue-fill observation

## Problem

The pool's single worker is deterministically occupied in the saturation tests, but the tests still wait 20–50 ms and assume the second submitter has completed `try_send` into the one-slot queue. Under load, the third submission can race ahead of the second.

The repository already has a test-only enqueue signal used by queued external-cancellation coverage. Reuse one common mechanism rather than introducing another timing workaround.

## Required helper contract

Provide or retain a `cfg(test)` submission path that signals only after `try_send` succeeds:

```rust
#[cfg(test)]
fn submit_cancellable_with_enqueue_signal(
    &self,
    handler: impl FnOnce() -> ToolResponse + Send + 'static,
    timeout: Duration,
    cancel_flag: Arc<AtomicBool>,
    enqueued: Arc<TestEnqueueSignal>,
) -> Result<ToolResponse, SyncPoolError> {
    // Build job.
    // Call try_send.
    // Return QueueFull/Shutdown immediately on failure.
    // Signal `enqueued` only after successful try_send.
    // Then wait for reply through the production wait_for_reply helper.
}
```

The signal must not fire:

- before `try_send`;
- when the queue is full;
- when the pool channel is disconnected.

The production `submit_cancellable` path should continue to use the same underlying submit implementation without a test signal.

### Preferred factoring

Use one private implementation to avoid duplicating queue submission:

```rust
fn submit_cancellable_inner(
    &self,
    handler: impl FnOnce() -> ToolResponse + Send + 'static,
    timeout: Duration,
    cancel_flag: Arc<AtomicBool>,
    #[cfg(test)] enqueued: Option<Arc<TestEnqueueSignal>>,
) -> Result<ToolResponse, SyncPoolError>;
```

Equivalent factoring is acceptable. Do not copy the entire production method into a test-only method.

## Rewrite `queue_saturation_returns_queue_full`

Required sequence:

1. construct a pool with one worker and queue capacity one;
2. submit a blocking first job from another thread;
3. wait on `BlockingJobGate::wait_until_started` so the worker is known to be occupied;
4. submit the second job through the enqueue-signal path;
5. wait until the second job signals successful queue insertion;
6. submit the third job;
7. assert exactly `Err(SyncPoolError::QueueFull { worker_count: 1 })`;
8. release the first job;
9. join all submitter threads;
10. assert first and second jobs succeeded and the pool remains usable.

Prohibited pattern:

```rust
std::thread::sleep(Duration::from_millis(50));
// assume job 2 is queued
```

Required pattern:

```rust
enqueued.wait_until_entered();
let third = pool.submit(...);
assert!(matches!(third, Err(SyncPoolError::QueueFull { worker_count: 1 })));
```

## Rewrite `queue_saturation_does_not_set_cancel`

Use the same exact sequence, but submit the third job through `submit_cancellable` with a caller-owned flag.

Assert:

- third submission returns `QueueFull`;
- the third job's cancellation flag remains false;
- the third handler is never invoked;
- first and second jobs drain normally;
- the pool accepts a subsequent sentinel job.

## Acceptance criteria

- Neither saturation test sleeps to infer queue insertion.
- The queue-enqueued signal fires only after successful `try_send`.
- Both tests deterministically produce `QueueFull` in 100 repeated runs.
- Queue saturation does not set the rejected job's cancellation flag.
- No test-only submit path duplicates reply classification or production queue policy.

---

# Workstream 2 — Make concurrency and occupancy tests prove their names

## Rewrite `two_jobs_run_concurrently`

### Current defect

The current test calls the first blocking `submit` synchronously and waits for its response before issuing the second submission. Both jobs run, but not concurrently.

### Required test

Use a two-worker pool and two per-job gates.

Required sequence:

1. create `SyncExecutionPool::with_limits(2, 2)`;
2. create two independent `BlockingJobGate` instances;
3. spawn one submitter thread per job;
4. each handler signals its gate and waits for release;
5. wait until both gates report started before releasing either;
6. assert both handlers are simultaneously inside their handler bodies;
7. release both gates;
8. join both submitter threads;
9. assert both responses succeed;
10. submit a sentinel and assert the pool remains usable.

A shared atomic active counter can provide an additional assertion:

```rust
let now = active.fetch_add(1, Ordering::SeqCst) + 1;
peak.fetch_max(now, Ordering::SeqCst);
gate.arrive_and_wait();
active.fetch_sub(1, Ordering::SeqCst);
```

Assert `peak == 2`. The gate arrival proof is still required; the counter alone must not rely on sleep-based overlap.

Rename the test only if the implementation cannot prove overlap. Do not retain a concurrency name for a sequential test.

## Rewrite `timed_out_running_retains_worker`

### Current defect

The current test allows the second request to either succeed or return `QueueFull`, making the occupancy assertion non-falsifiable.

### Required test

Use one worker and zero queue capacity if supported by `sync_channel(0)`, or one worker plus an exactly filled queue if zero capacity is unsuitable.

Preferred sequence with zero queue capacity:

1. create `SyncExecutionPool::with_limits(1, 0)`;
2. start job one with a `BlockingJobGate`;
3. wait until job one is running;
4. allow the caller-facing timeout to occur while job one remains gated;
5. assert job one's submitter received `Timeout`;
6. while the handler remains gated and still owns the worker, submit job two;
7. assert job two returns exactly `QueueFull` and its handler did not run;
8. release job one;
9. wait for a closure-exit or handler-exit signal from job one;
10. submit job three and assert success.

Alternative with queue capacity one:

- enqueue a sentinel as job two and wait for its enqueue signal;
- job three must return `QueueFull` while timed-out job one still occupies the worker;
- release job one;
- sentinel runs;
- a final recovery job succeeds.

Do not accept multiple outcomes for the key occupancy assertion.

## Review adjacent timing tests

Review these tests while editing the file:

- `repeated_timeouts_pool_stays_usable`;
- `running_cooperative_handler_exits_on_flag`;
- `repeated_timeouts_do_not_increase_worker_count`;
- `timeout_smoke_handler_continues_after_return`;
- `no_double_completion_smoke`.

Timing is acceptable where these are explicitly smoke/elapsed-time tests. Replace settlement sleeps with exit signals where one already exists and doing so is narrow. Do not expand this pass into a wholesale test-suite rewrite.

## Acceptance criteria

- `two_jobs_run_concurrently` proves two simultaneous running handlers.
- `timed_out_running_retains_worker` has one exact expected result while the timed-out handler remains active.
- Both tests use explicit start/release/exit synchronization.
- No key assertion accepts mutually contradictory outcomes.
- Focused sync-pool tests pass 100 consecutive runs under normal test-thread scheduling.

---

# Workstream 3 — Freeze one exact code-and-test SHA

## Create `CODE_SHA`

After Workstreams 1 and 2:

```bash
cargo fmt --all -- --check
cargo test --locked --all-features --lib mcp::sync_pool

for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::sync_pool || exit 1
done
```

Then run the full local gate before committing:

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

Commit code and tests with no evidence edits in the same commit.

Suggested commit message:

```text
test(sync): close final deterministic pool proof gaps
```

Record the full 40-character commit as `CODE_SHA`.

## Clean-checkout local verification

Create a fresh worktree at `CODE_SHA` and rerun the canonical local gate:

```bash
CODE_SHA=$(git rev-parse HEAD)
git worktree add /tmp/eggsact-polish-closure "$CODE_SHA"
cd /tmp/eggsact-polish-closure

test "$(git rev-parse HEAD)" = "$CODE_SHA"
test -z "$(git status --porcelain)"
```

Also run:

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc

RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build
RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build --sanitizer=address
```

The worktree must remain clean after all commands.

## Acceptance criteria

- `CODE_SHA` contains every production file, test, corpus seed, workflow, and helper being claimed as release-ready.
- No documentation-only evidence commit is used as the code-under-test baseline.
- All local gates run from a clean checkout of `CODE_SHA`.
- Exact test counts and toolchain versions are recorded for later evidence updates.

---

# Workstream 4 — Run every required workflow on exact `CODE_SHA`

## Why reruns are required

The prior extended fuzz/sanitizer run was on `fa6a6e9`. The final baseline later changed production `src/text/glob.rs` to remove a Unicode indexing panic. Because `glob_matching` is an extended fuzz and sanitizer target, the old run cannot be described as exact-final-SHA evidence.

Do not reuse an earlier run across a production-code delta.

## Verification branch

Create a temporary branch pointing exactly to `CODE_SHA`:

```bash
git branch verification/final-polish "$CODE_SHA"
git push origin verification/final-polish
```

Confirm:

```bash
git rev-parse verification/final-polish
# must equal CODE_SHA
```

Use the branch only to dispatch workflows. Do not add commits to it.

## Required workflows

Discover the exact current workflow names and supported inputs with `gh workflow list` and the workflow YAML files. Dispatch or observe these against the exact verification branch:

1. ordinary CI;
2. release verification;
3. extended fuzz plus sanitizer matrix;
4. latest-compatible dependencies;
5. Python parity.

Use the repository's actual workflow filenames. Example only:

```bash
gh workflow run fuzz-scheduled.yml --ref verification/final-polish
gh workflow run release-verification.yml --ref verification/final-polish
```

## Required exact-SHA assertions

For every run, verify:

- `head_sha == CODE_SHA`;
- branch/ref is the exact verification branch or `main` at `CODE_SHA`;
- run status is completed;
- conclusion is success;
- no required matrix job is missing;
- no required job is cancelled or skipped without an explicit approved reason.

### Extended fuzz/sanitizer acceptance

- all 12 extended fuzz target jobs pass;
- all 7 AddressSanitizer jobs pass;
- `glob_matching` is included in both the applicable extended and sanitizer evidence;
- the run uses the pinned nightly toolchain defined by the workflow;
- the run's head SHA equals `CODE_SHA`.

### Release-verification acceptance

- format, generated docs, clippy, unit, binary, integration, doc, cargo-deny, package, publish dry run, provenance generation, and artifact upload all pass;
- the provenance artifact records `CODE_SHA`;
- package version, MSRV, Rust version, lockfile checksum, and package-file count are present.

### Parity acceptance

- run completes against `CODE_SHA`;
- accepted differences remain explicitly ignored/documented;
- zero unaccepted parity failures remain.

## Artifact handling

Download required artifacts and compute SHA-256:

```bash
gh run download "$RELEASE_RUN_ID" --dir /tmp/eggsact-release-artifacts
find /tmp/eggsact-release-artifacts -type f -print0 \
  | sort -z \
  | xargs -0 shasum -a 256
```

Record:

- run ID;
- run URL;
- exact head SHA;
- artifact ID;
- artifact filename;
- SHA-256;
- relevant package metadata.

## Acceptance criteria

- Every required workflow passes on the same exact `CODE_SHA`.
- No production-code difference exists between the fuzz/sanitizer run and the claimed release baseline.
- All matrix jobs and artifact identities are recorded.
- The temporary verification branch is deleted after evidence is committed, unless repository policy requires retaining it.

---

# Workstream 5 — Normalize closure evidence without a SHA loop

## Remove self-referential metadata

Delete fields such as:

```text
Final main head: <sha>
```

A document-changing commit necessarily makes that field stale. Do not replace it with another “current head” value.

Use durable identities instead:

```text
Code-under-test SHA: <CODE_SHA>
Evidence document: docs/releases/2026-07-final-closure-evidence.md
Evidence commit: resolve with `git log -1 --format=%H -- <path>`
Workflow runs: explicit immutable run IDs and head SHAs below
```

It is acceptable that the evidence document cannot contain the SHA of the commit that first contains itself. Git history is the authoritative identity for that commit.

## Update exact workflow claims

Correct all statements that currently claim run `30138546987` executed on the final SHA.

After the rerun, replace the old extended fuzz/sanitizer evidence with the new run tied to `CODE_SHA`.

Do not say:

```text
The older run remains valid because later changes were non-functional.
```

when a later commit modified production source.

The old run may remain in a historical section labelled as an earlier baseline, but it must not satisfy final closure criteria.

## Files to reconcile

At minimum inspect and update:

- `docs/releases/2026-07-final-closure-evidence.md`;
- `docs/release-4-status.md`;
- `docs/release-5-status.md`;
- `docs/release-readiness.md`;
- `plans/2026-07-24-final-proof-and-release-evidence-closure-pass.md` only if its status marker is maintained in-place.

## Required evidence content

The final evidence must state:

- exact `CODE_SHA`;
- clean-checkout proof;
- local stable/MSRV/nightly versions;
- exact test counts;
- exact sync-pool repeated-run count;
- exact lifecycle stress results already retained from the same code baseline;
- ordinary CI run ID and all required job conclusions;
- release-verification run ID;
- exact-SHA fuzz/sanitizer run ID and matrix conclusions;
- latest-compatible run ID;
- Python parity run ID and counts;
- provenance artifact ID, filename, and SHA-256;
- direct-maintainer publication status.

## Evidence-commit sequencing

1. complete all runs against `CODE_SHA`;
2. update documents with immutable run IDs and head SHAs;
3. commit documents once;
4. push and require ordinary CI to pass on that evidence commit;
5. do not edit the documents again solely to record their own commit SHA or CI run ID.

The evidence commit's CI result remains visible in GitHub's immutable check history and does not need to be recursively embedded in the document.

## Acceptance criteria

- No self-referential “final head” field remains.
- Every final workflow claim names a run whose `head_sha` equals `CODE_SHA`.
- Earlier runs are clearly labelled historical and do not satisfy final criteria.
- No document calls production-different SHAs “code identical.”
- Release 4, Release 5, and release-readiness use the same final baseline and run identities.
- Documentation is committed once after workflow completion, avoiding a SHA-update loop.

---

# Workstream 6 — Release-status reconciliation

## Release 4

Release 4 may remain complete if all infrastructure and release-verification criteria pass on `CODE_SHA`.

Verify its status table references:

- ordinary CI on `CODE_SHA`;
- exact-SHA release verification;
- current provenance artifact;
- latest-compatible and parity runs on `CODE_SHA`;
- clean-checkout package and publish-dry-run results.

## Release 5

Release 5 cannot be finally closed until extended fuzz and sanitizer matrices pass on `CODE_SHA`.

Required Release 5 wording:

- 12 fuzz targets build and execute;
- 7 sanitizer matrix jobs execute;
- all tracked regression seeds are present;
- no untriaged crash, hang, loop, overflow, or OOM remains from the final run;
- fuzz-discovered production fixes are included in `CODE_SHA` and exercised by the exact-SHA run;
- workflow run ID and exact SHA are stated accurately.

## Publication status

Keep these unchecked/direct-maintainer actions separate from correctness closure:

- `cargo publish --locked`;
- annotated `v1.2.0` tag creation and push.

Release readiness may be complete while publication remains pending.

## Acceptance criteria

- Release 4 and Release 5 status notes agree with the closure evidence.
- Release 5 no longer cites an earlier production-different SHA as exact-final evidence.
- Publication is not performed or automated by this pass.

---

# Verification checklist

## Sync-pool polish

- [ ] Queue insertion has an exact post-`try_send` signal.
- [ ] `queue_saturation_returns_queue_full` contains no queue-fill sleep.
- [ ] `queue_saturation_does_not_set_cancel` contains no queue-fill sleep.
- [ ] Rejected saturated handler is never invoked.
- [ ] Saturation does not set the rejected job's cancellation flag.
- [ ] `two_jobs_run_concurrently` proves simultaneous handler execution.
- [ ] `timed_out_running_retains_worker` has one exact occupancy outcome.
- [ ] Focused sync-pool suite passes 100 consecutive normal-scheduling runs.

## Exact code baseline

- [ ] One frozen `CODE_SHA` contains all code, tests, workflows, and corpus seeds.
- [ ] Clean worktree at `CODE_SHA` is verified.
- [ ] Full local release gate passes.
- [ ] MSRV gate passes.
- [ ] Normal and ASan fuzz builds pass.
- [ ] Worktree remains clean.

## Workflow evidence

- [ ] Ordinary CI passes on `CODE_SHA`.
- [ ] Release verification passes on `CODE_SHA`.
- [ ] Extended fuzz 12/12 passes on `CODE_SHA`.
- [ ] Sanitizer 7/7 passes on `CODE_SHA`.
- [ ] Latest-compatible passes on `CODE_SHA`.
- [ ] Python parity passes on `CODE_SHA` with zero unaccepted failures.
- [ ] Provenance artifact is downloaded and checksummed.

## Documentation

- [ ] Self-referential final-head fields are removed.
- [ ] No older production-different run is called exact-final evidence.
- [ ] Closure evidence names immutable run IDs and head SHAs.
- [ ] Release 4 status matches evidence.
- [ ] Release 5 status matches evidence.
- [ ] Release readiness matches evidence.
- [ ] Evidence documents are updated once after workflow completion.
- [ ] Ordinary CI passes on the evidence commit.

---

# Required repeated tests

Run at minimum:

```bash
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib \
    mcp::sync_pool::tests::queue_saturation_returns_queue_full || exit 1
  cargo test --locked --all-features --lib \
    mcp::sync_pool::tests::queue_saturation_does_not_set_cancel || exit 1
  cargo test --locked --all-features --lib \
    mcp::sync_pool::tests::two_jobs_run_concurrently || exit 1
  cargo test --locked --all-features --lib \
    mcp::sync_pool::tests::timed_out_running_retains_worker || exit 1
done
```

Then run the complete sync-pool module 100 times:

```bash
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::sync_pool || exit 1
done
```

Do not retry a failed iteration and report the later retry as the original pass. Investigate and correct any flake.

---

# Trouble-area examples

## Incorrect queue proof

```rust
spawn_second_submitter();
std::thread::sleep(Duration::from_millis(20));
assert_queue_full_on_third_submit();
```

This proves only that 20 ms elapsed.

## Correct queue proof

```rust
spawn_second_submitter_with_signal(enqueued.clone());
enqueued.wait_until_entered();
assert_queue_full_on_third_submit();
```

This proves the queue insertion operation completed.

## Incorrect concurrency proof

```rust
let first = pool.submit(first_job, timeout)?;
let second = pool.submit(second_job, timeout)?;
```

The second call begins after the first response returns.

## Correct concurrency proof

```rust
let h1 = std::thread::spawn(move || pool1.submit(job1, timeout));
let h2 = std::thread::spawn(move || pool2.submit(job2, timeout));
gate1.wait_until_started();
gate2.wait_until_started();
// Both handlers are now blocked inside their bodies.
gate1.release();
gate2.release();
```

## Incorrect evidence inheritance

```text
Run A on SHA X remains exact evidence for SHA Y because the changes were minor.
```

This is invalid when `X..Y` contains any production-code change affecting a covered fuzz target.

## Correct evidence rule

```text
Final extended fuzz run R has head SHA equal to CODE_SHA C.
`git diff C R.head_sha` is empty because they are the same commit.
```

---

# Suggested commit sequence

Keep the sequence compact:

1. `test(sync): close final deterministic pool proof gaps`
2. `docs(release): record exact-sha final closure evidence`

A third commit should not be needed merely to update a “current head” field, because that field must be removed.

---

# Subagent stop conditions

Stop and report rather than claiming closure when:

- a saturation test still needs a sleep to produce `QueueFull`;
- `two_jobs_run_concurrently` cannot prove simultaneous handler entry;
- retained-worker testing accepts both success and saturation;
- any focused test flakes during 100 repeated runs;
- a required workflow's head SHA differs from `CODE_SHA`;
- any extended fuzz or sanitizer matrix job is missing, skipped, cancelled, or failed;
- `glob_matching` is not exercised in the final fuzz evidence;
- a provenance artifact cannot be downloaded or checksummed;
- parity has any unaccepted failure;
- local verification modifies tracked files;
- documentation still calls an earlier production-different run exact-final evidence;
- closure requires automating crates.io publication.

Do not convert a failed exact-SHA requirement into an “equivalent SHA” argument. Create and verify a new exact baseline instead.

---

# Definition of done

This final polish line is complete only when:

1. all queue-state and worker-occupancy assertions use explicit synchronization;
2. the concurrency test proves two handlers overlap;
3. the retained-worker test has one exact expected result;
4. focused sync-pool tests pass 100 repeated normal-scheduling runs;
5. one frozen `CODE_SHA` contains all final code, tests, workflows, and corpus seeds;
6. clean-checkout local stable, MSRV, package, publish-dry-run, fuzz-build, and ASan-build gates pass at `CODE_SHA`;
7. ordinary CI, release verification, extended fuzzing, sanitizers, latest-compatible dependencies, and Python parity all pass with `head_sha == CODE_SHA`;
8. provenance artifacts are downloaded and checksummed;
9. closure evidence contains no self-referential head field;
10. Release 4, Release 5, release-readiness, and final closure evidence all cite the same exact baseline and immutable runs;
11. the evidence commit passes ordinary CI without triggering another documentation-update loop;
12. actual crates.io publication and tag creation remain direct maintainer actions.

After these conditions are met, the runtime-correctness, verification, fuzzing, and release-readiness line may be described as fully closed, with only direct publication remaining.
