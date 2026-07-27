# Final Closure Evidence

This document records the exact evidence supporting closure of the runtime
correctness corrective passes:

- `plans/2026-07-23-final-cancellation-lifecycle-evidence-closure-pass.md`
- `plans/2026-07-24-final-verification-evidence-closure-pass.md`
- `plans/2026-07-24-final-proof-and-release-evidence-closure-pass.md`

## Code-under-test

- **Code-under-test SHA**: `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
- **Evidence date**: 2026-07-27 UTC
- **Branch**: `main`
- **Previous evidence baseline**: `50f9132f23c72e9a0df9475774430bdea9ac32d7`

The CODE_SHA extends the previous baseline with a fuzz target bound fix
(`calculator_normalization` output assertion relaxed from `expr.len()*100+1000`
to `expr.len()*1000+10_000` to accommodate legitimate large numeric results
like `946!`). The previous CODE_SHA (`50f9132`) contains deterministic
sync-pool test rewrites (queue-saturation signals, concurrency proof,
retained-worker proof) and a shared `enqueue_job` helper.

## Package

- **Version**: `1.2.0` (release candidate)
- **Manifest**: `Cargo.toml`

## Toolchain

- **Stable Rust (local)**: `1.97.0 (2d8144b78 2026-07-07)`
- **Stable Rust (release runner)**: `1.97.1`
- **MSRV**: `1.89.0` (declared in `Cargo.toml`, tested in CI)
- **Nightly Rust**: `nightly-2026-05-07`, `rustc 1.97.0-nightly (365c0e1d7 2026-05-06)`
- **cargo-fuzz**: `0.13.2`
- **cargo-deny**: `0.19.0` (pinned in CI workflows)

## Local Verification Commands

All commands ran against a clean checkout of `3e5b41c` during the final
verification window.

### Release gate

```
cargo fmt --all -- --check                                         PASS (0 diffs)
cargo clippy --locked --all-targets --all-features -- -D warnings  PASS (no issues)
cargo test --locked --all-features --lib                           PASS (494 passed, 1 ignored)
cargo test --locked --all-features --bins                          PASS (24 tests)
cargo test --locked --all-features --tests -- --skip parity        PASS (3423 passed, 1 ignored, 418 filtered)
cargo test --locked --doc                                          PASS (11 tests)
cargo run --locked --bin generate-docs -- --check                  PASS
cargo deny check advisories bans licenses sources                  PASS
cargo package --locked --verbose                                   PASS
cargo publish --locked --dry-run                                   PASS
```

### Deterministic lifecycle stress loop

```
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib mcp::execution::deterministic_tests || exit 1
done
  100 iterations: 9 passed each
```

All 9 deterministic gate-controlled tests pass across 100 sequential
invocations. The combined ordinary-scheduling fallback also passed 25 complete
library runs. The separate ignored exact-interleaving test passed 500/500
iterations (250 completion-wins and 250 timeout-wins).

### Focused lifecycle gates

```
cargo test --locked --all-features --lib mcp::execution::deterministic_tests
  9 passed (1 ignored: 500-iteration stress test)
cargo test --locked --all-features --lib mcp::execution::deterministic_tests -- --test-threads=1
  9 passed (1 ignored)
```

### Focused synchronous gates

```
cargo test --locked --all-features --lib mcp::sync_pool             PASS (24 tests)
cargo test --locked --all-features --test lib sync_policy           PASS
cargo test --locked --all-features --test lib context_isolation     PASS
```

### MSRV gate

```
cargo +1.89.0 check --locked --all-targets --all-features           PASS
cargo +1.89.0 test --locked --all-features --lib                    PASS (494 passed, 1 ignored)
cargo +1.89.0 test --locked --all-features --bins                   PASS (24 tests)
cargo +1.89.0 test --locked --doc                                   PASS (11 tests)
```

### Fuzz build

```
RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build                PASS (12 targets)
RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build --sanitizer=address PASS
```

### Clean worktree verification

```
CODE_SHA=3e5b41c6ac5a8daaba11d5dfacb822f6da033464
git worktree add /tmp/eggsact-final-closure "$CODE_SHA"
cd /tmp/eggsact-final-closure
git rev-parse HEAD                                   # $CODE_SHA
git status --porcelain                               # (clean, no output)
test "$(git rev-parse HEAD)" = "$CODE_SHA"           # MATCH
```

Full release gate ran from clean worktree. Worktree remained clean after all
verification commands. Worktree was removed after verification completed.

### Fuzz-discovered production fixes

The final proof run found and closed two production defects: `gcd`/`lcm` now
reject `i64::MIN` before absolute-value conversion, and regex iteration now
advances after a zero-length match at end-of-input. Additional minimized inputs
closed a zero-count unified-diff range panic and a short-Unicode path indexing
panic. The corresponding regression seeds are committed in `fuzz/corpus/`.
The calculator-normalization and JSON-pointer fuzz assertions were also
corrected to test their actual deterministic/valid-JSON contracts rather than
invalid byte-for-byte assumptions for large-number formatting.

## Test Counts

| Partition | Count |
|-----------|-------|
| Unit (lib) | 494 passed, 1 ignored |
| Binary | 24 |
| Integration (non-parity) | 3423 passed, 1 ignored, 418 filtered |
| Doc | 11 |
| **Total (passing tests reported)** | **3952** |

### Deterministic lifecycle tests (execution.rs `deterministic_tests`)

**Gate-controlled exact interleavings** (winner selection controlled by gates, not timing):

- `timeout_after_permit_before_lifecycle_start` — Test A: timeout fires while closure is gated at `before_begin_running`; `begin_running` sees `CancelledBeforeStart`; handler never invoked
- `completion_wins_timeout_record_race` — Test B: `before_finish` released before `record_timeout`; `record_timeout` observes `Finished`; `timed_out_handlers` not incremented
- `timeout_wins_completion_race` — Test C: `record_timeout` released before `before_finish`; `timed_out_handlers` incremented then decremented
- `panic_after_timeout` — Test D: handler panics after timeout; `catch_unwind` catches panic; gauges return to zero
- `cooperative_cancellation_visibility` — Test E: handler polls cancel flag via `current_cancel_flag()`; observes flag set by timeout
- `one_hundred_exact_interleavings` — Test F: 50 completion-wins + 50 timeout-wins sequences; per-iteration gates; no shared state
- `five_hundred_exact_interleavings` — 500-iteration stress test (ignored in ordinary CI)
- `worker_bound_n_plus_one` — Test G: 3 workers with `running_established` gates; N+1 invocation queued and observed
- `deterministic_completion_wins` — fast handler completes before timeout; no gauge leak
- `repeated_single_threaded_100_iterations` — 100 iterations with varying handlers/timeouts

**Timing-based smoke tests** (`mod tests`):

- `queued_timeout_smoke_does_not_run_handler`
- `timeout_smoke_returns_while_handler_continues`
- `running_timeout_smoke_increments_once`
- `no_double_completion_smoke`
- `basic_execution_completes_successfully`
- `panic_cleanup_smoke`
- `timeout_smoke_handler_continues_after_return`

### Sync pool reply classification tests

- `wait_for_reply_success_returns_response` — success path, flag not set
- `wait_for_reply_timeout_sets_cancel_flag` — timeout sets cancel before returning
- `wait_for_reply_disconnected_returns_shutdown` — disconnection maps to Shutdown, flag not set
- `wait_for_reply_timeout_with_sender_retained_sets_cancel` — sender alive, timeout still sets flag

### Sync pool deterministic tests

- `queue_saturation_returns_queue_full` — 1 worker, 1 queue slot; BlockingJobGate holds worker, TestEnqueueSignal confirms queue insertion, third submit returns QueueFull
- `queue_saturation_does_not_set_cancel` — same pattern; third submit uses submit_cancellable with own flag, asserts flag remains false on QueueFull
- `two_jobs_run_concurrently` — 2 workers; two BlockingJobGates prove simultaneous handler execution via atomic peak counter
- `timed_out_running_retains_worker` — 1 worker, 1 queue slot; handler blocks on gate, caller times out, sentinel fills queue, third submit returns QueueFull while timed-out handler still owns worker

## Runtime Lifecycle Model

The implementation uses a mutex-backed lifecycle with five phases:

```
Queued → Running → Finished
         ↓           ↑
     TimedOutRunning ─┘
         ↑
Queued ──┘ (timeout before spawn → TimedOutQueued, handler never runs)
```

### Invariants enforced

- `timed_out_handlers <= active_blocking_handlers` at all stable snapshots
- No decrement runs without a preceding matching increment
- Queued timeout never changes `timed_out_handlers`
- Every running-timeout increment has exactly one decrement
- All gauges return to zero after controlled workers finish
- Handler lifecycle completion runs under the same lock as timeout transition

## Closure Checklist Items

### Runtime lifecycle

- [x] Initial handler state is queued, not running
- [x] A queued timeout never starts blocking work later
- [x] Queued timeout increments `total_timeouts` only
- [x] Running timeout increments `timed_out_handlers` before publishing a decrementable state
- [x] No handler path can decrement before the matching increment
- [x] Handler completion decrements the timed-out-running gauge exactly once
- [x] Panic and cancellation use the same completion accounting
- [x] Stable snapshots never show unsigned underflow
- [x] `timed_out_handlers <= active_blocking_handlers` at synchronized snapshots
- [x] All gauges return to zero after controlled workers exit

### Active requests

- [x] Active-request identity uses an explicit generation/token
- [x] Completion cleanup uses awaited locking
- [x] No correctness path relies on `try_lock` in `Drop`
- [x] Normal return removes the request
- [x] Timeout removes the request
- [x] Cancellation removes the request
- [x] Handler panic removes the request
- [x] Response serialization failure removes the request
- [x] A stale generation cannot remove a replacement request
- [x] Request IDs are reusable immediately after cleanup

### Synchronous execution

- [x] A fixed worker-count executor exists (8 workers, 32-slot queue)
- [x] The submission queue is bounded
- [x] Budget-aware synchronous calls enforce `max_elapsed_ms`
- [x] Queue saturation returns a structured `RESOURCE_EXHAUSTED` error
- [x] Timed-out work retains worker occupancy until it exits
- [x] Repeated timeouts do not create unbounded threads
- [x] Cancellation and eval-context state are installed and restored per job
- [x] MCP does not call the synchronous executor from inside `spawn_blocking`
- [x] Raw `call_json` timeout semantics are documented accurately
- [x] Worker survives job panic via `catch_unwind`

### Tests — exact transition gates

- [x] Exact async/blocking test gates exist (`BlockingTestGate`, `AsyncTestGate`)
- [x] Test gates pause (not merely notify) via `arrive_and_wait`
- [x] All 7 lifecycle hook sites have gate support
- [x] Timeout-before-`begin_running` is directly forced (Test A)
- [x] Completion-wins is forced at the timeout lifecycle lock (Test B)
- [x] Timeout-wins is forced before completion lifecycle lock (Test C)
- [x] Panic-after-timeout uses exact gates (Test D)
- [x] Cooperative cancellation exits without a fixed sleep (Test E)
- [x] 100 interleavings use per-invocation gates, no shared state (Test F)
- [x] Worker-bound test submits and observes N+1 (Test G)
- [x] Parallel lifecycle tests use exclusive RAII slot leases over cfg(test) backing storage; no manual slot assignment is exposed to tests
- [x] Legacy `TEST_HANDLER_SHOULD_BLOCK` / `TEST_HANDLER_RELEASED` removed
- [x] Timing-based tests renamed with `_smoke_` suffix

### Tests — mutable commit

- [x] Mutable transaction tests call shared production code (`execute_handler_with_commit_slot`)
- [x] No test helper duplicates the commit-slot algorithm
- [x] Success commits, failure/cancel/timeout/saturation/panic roll back

### Tests — reply classification

- [x] Production `wait_for_reply` helper extracted and called by `submit_cancellable`
- [x] Tests exercise the helper directly (success, timeout, disconnect, flag visibility)
- [x] `std::mpsc`-only test replaced with production helper tests

### Mutable execution context

- [x] `call_json_with_execution_context_mut` delegates to shared `execute_handler_with_commit_slot`
- [x] Commit requires `response.ok == true` AND cancel flag still false
- [x] On timeout/saturation/cancellation/failure, commit slot is never read (late writes discarded)

### MCP lifecycle

- [x] `begin_running` occurs inside the blocking closure
- [x] `active_blocking_handlers` increments inside the blocking closure
- [x] Permit acquisition alone does not publish `Running`
- [x] Timeout after permit but before closure start prevents handler invocation
- [x] Running timeout increments exactly once
- [x] Completion after timeout decrements exactly once
- [x] Panic returns gauges to zero
- [x] Peak concurrency reflects actual executing closures

### Documentation and evidence

- [x] Test names and comments match what each test actually proves
- [x] Evidence distinguishes smoke tests from exact transition tests
- [x] Every cited test exists in the code-under-test SHA `3e5b41c`
- [x] No approximate test counts remain (exact: 494 lib passed, 24 bin, 3423 integration passed, 11 doc)
- [x] Closure evidence identifies the exact code-and-workflow evidence baseline
- [x] All local verification gates pass from the code-under-test SHA
- [x] MSRV gate passes on Rust 1.89.0
- [x] Fuzz build succeeds (12 targets)
- [x] `cargo publish --dry-run` passes
- [x] Clean worktree at CODE_SHA verified
- [x] Ordinary CI passed on CODE_SHA `50f9132` (Run `30185819114`, 12/12 jobs)
- [x] Release-verification passed on CODE_SHA `50f9132` (Run `30285308354`, Full Release Gate)
- [x] Extended fuzz and sanitizer matrices passed on CODE_SHA `3e5b41c` (Run `30287151564`, 19/19 jobs)
- [x] Latest-compatible passed on CODE_SHA `50f9132` (Run `30285309780`)
- [x] Python parity passed on CODE_SHA `50f9132` (Run `30285310359`)
- [x] Final evidence head CI passed on CODE_SHA `3e5b41c` (Run `30288582349` on `7174c09`, 12/12 jobs)

## GitHub Actions Evidence

### Ordinary CI — CODE_SHA `50f9132`

- **Run ID**: `30185819114`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30185819114>
- **Head SHA**: `50f9132f23c72e9a0df9475774430bdea9ac32d7` (via evidence commit `bb022d7`)
- **Conclusion**: success; all 12 jobs passed (Check, Clippy, Generated Docs, MSRV, Test lib/bins/integration/doc, cargo-deny, macOS, Windows)

### Release Verification — CODE_SHA `50f9132`

- **Run ID**: `30285308354`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30285308354>
- **Head SHA**: `50f9132f23c72e9a0df9475774430bdea9ac32d7`
- **Conclusion**: success; Full Release Gate passed

### Extended Fuzz — CODE_SHA `3e5b41c`

- **Run ID**: `30287151564`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30287151564>
- **Head SHA**: `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
- **Conclusion**: success; 19/19 jobs passed (12 fuzz-matrix + 7 fuzz-sanitizers)
- **Fuzz matrix targets**: calculator_expression, calculator_normalization, calculator_normalization, glob_matching, json_pointer, markdown_fences, regex_classification, regex_execution, shell_quoting, shell_tokenization, toml_config, unicode_inspection, unified_diff
- **Sanitizer targets**: calculator_expression, glob_matching, json_pointer, regex_classification, shell_tokenization, unicode_inspection, unified_diff

### Latest-compatible Dependencies — CODE_SHA `50f9132`

- **Run ID**: `30285309780`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30285309780>
- **Head SHA**: `50f9132f23c72e9a0df9475774430bdea9ac32d7`
- **Conclusion**: success; Latest Compatible passed

### Python Parity — CODE_SHA `50f9132`

- **Run ID**: `30285310359`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30285310359>
- **Head SHA**: `50f9132f23c72e9a0df9475774430bdea9ac32d7`
- **Conclusion**: success; Parity (latest eggcalc) passed

### Provenance Artifact

- **Release provenance SHA-256**: `2311088008417aeeccf4168bb9d6341e77d2856db7db304e114cc8e971c88c80`
- **Package version**: `1.2.0`
- **Rust stable (release runner)**: `1.97.1`
- **MSRV**: `1.89.0`
- **Lockfile SHA-256**: `5dd9396665d264fb406c4e9295f6caae2696916650db33a25e7dd2c31d04cec7`
- **Package files**: 236

### Historical runs (earlier baselines, not final evidence)

#### Ordinary CI (baseline `06f7a0b`)

- **Run ID**: `30162970273`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30162970273>
- **Head SHA**: `06f7a0bd7c1005439e9de229c37cb34d988b42e4`
- **Conclusion**: success; all 12 jobs passed

#### Release Verification (baseline `06f7a0b`)

- **Run ID**: `30177462182`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30177462182>
- **Head SHA**: `06f7a0bd7c1005439e9de229c37cb34d988b42e4`
- **Conclusion**: success; all 18 jobs passed

#### Extended Fuzz (baseline `fa6a6e9`, historical only)

- **Run ID**: `30138546987`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30138546987>
- **Head SHA**: `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`
- **Conclusion**: success; 19/19 jobs passed, including 7/7 sanitizer jobs

#### Latest-compatible dependencies (baseline `fa6a6e9`, historical only)

- **Run ID**: `30138547661`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30138547661>
- **Head SHA**: `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`
- **Conclusion**: success

#### Python parity (baseline `fa6a6e9`, historical only)

- **Run ID**: `30138548267`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30138548267>
- **Head SHA**: `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`
- **Conclusion**: success; 381 passed, 0 failed, 37 ignored
- **Report**: eggcalc `1.1.6`, Python `3.12.13`

#### Provenance Artifacts (from baseline `06f7a0b`, historical only)

- **Release provenance artifact ID**: `8624794842`
- **Release provenance SHA-256**: `7f977abfbfc94eb9c66e7894622ba0a41e1116892ece458a3e4f9bacbb51a30f`

## Intentionally Deferred Items

1. **Actual crates.io publication and tag creation** remain direct maintainer
   actions. The release gate proves the package and publish dry run but does
   not publish.

2. **Accepted Python parity differences** remain documented in `docs/parity.md`;
   the final parity workflow passed with those accepted cases ignored.

3. **Sanitizers** are covered by the existing `fuzz-scheduled.yml` sanitizer
   matrix: all seven sanitizer jobs passed in the final run.
