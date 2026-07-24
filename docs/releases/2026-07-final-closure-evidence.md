# Final Closure Evidence

This document records the exact evidence supporting closure of the runtime
correctness corrective passes:

- `plans/2026-07-23-final-cancellation-lifecycle-evidence-closure-pass.md`
- `plans/2026-07-24-final-verification-evidence-closure-pass.md`

## Code-under-test

- **SHA**: `6216d82f355e7acacf05484355c5d1252010327b`
- **Date**: 2026-07-24
- **Branch**: `main`

## Package

- **Version**: `1.2.0` (last published to crates.io)
- **Manifest**: `Cargo.toml`

## Toolchain

- **Stable Rust**: `1.97.0 (2d8144b78 2026-07-07)`
- **MSRV**: `1.89.0` (declared in `Cargo.toml`, tested in CI)
- **Nightly Rust**: not recorded for this pass
- **cargo-fuzz**: `0.13.2`
- **cargo-deny**: `0.19.0` (pinned in CI workflows)

## Local Verification Commands

All commands run on 2026-07-24 against the working tree at `6216d82`.

### Release gate

```
cargo fmt --all -- --check                                         PASS (0 diffs)
cargo clippy --locked --all-targets --all-features -- -D warnings  PASS (no issues)
cargo test --locked --all-features --lib                           PASS (492 tests)
cargo test --locked --all-features --bins                          PASS (24 tests)
cargo test --locked --all-features --tests -- --skip parity        PASS (3418 tests)
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

All 9 deterministic gate-controlled tests pass consistently across 100 sequential iterations.
No flakiness observed.

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
cargo +1.89.0 test --locked --all-features --lib                    PASS (492 tests)
cargo +1.89.0 test --locked --all-features --bins                   PASS (24 tests)
cargo +1.89.0 test --locked --doc                                   PASS (11 tests)
```

### Fuzz build

```
RUSTUP_TOOLCHAIN=nightly cargo fuzz build                           PASS (12 targets)
```

## Test Counts

| Partition | Count |
|-----------|-------|
| Unit (lib) | 492 |
| Binary | 24 |
| Integration (non-parity) | 3418 |
| Doc | 11 |
| **Total (local)** | **3945** |

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
- [x] No parallel lifecycle test uses shared mutable statics (per-slot BLOCK_SLOTS with unique slots)
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
- [x] Every cited test exists in the code-under-test SHA `6216d82`
- [x] No approximate test counts remain (exact: 492 lib, 24 bin, 3418 integration, 11 doc)
- [x] Closure evidence identifies the exact commit containing all cited changes
- [x] All local verification gates pass from the code-under-test SHA
- [x] MSRV gate passes on Rust 1.89.0
- [x] Fuzz build succeeds (12 targets)
- [x] `cargo publish --dry-run` passes
- [ ] Ordinary CI passed for `6216d82` (run pending)
- [ ] Manual release-verification workflow passed for `6216d82` (pending)
- [ ] Extended fuzz and sanitizer matrices passed (pending)

## GitHub Actions Evidence (to be filled after CI completes)

### Ordinary CI

- **Run ID**: (pending)
- **URL**: (pending)
- **Head SHA**: `6216d82f355e7acacf05484355c5d1252010327b`
- **Conclusion**: (pending)

### Release Verification

- **Run ID**: (pending)
- **URL**: (pending)
- **Head SHA**: `6216d82f355e7acacf05484355c5d1252010327b`

### Extended Fuzz

- **Run ID**: (pending)
- **URL**: (pending)

### Provenance Artifacts

- **Artifact ID**: (pending)
- **SHA-256**: (pending)

## Intentionally Deferred Items

1. **Per-target fuzz run evidence**: Requires the extended fuzz matrix workflow
   to run on GitHub Actions.

2. **Release 4/5 status closure**: Status notes will be marked complete
   after the release-verification workflow succeeds and the maintainer
   confirms the release candidate.

3. **Sanitizer matrix**: The repository does not currently have a sanitizer
   CI workflow. One should be added as a manual-only workflow per the plan.
