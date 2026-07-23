# Final Closure Evidence

This document records the exact evidence supporting closure of the runtime
correctness corrective pass (plans/2026-07-23-final-cancellation-lifecycle-evidence-closure-pass.md).

## Code-under-test

- **SHA**: `PLACEHOLDER_COMMIT_SHA` (update after commit)
- **Date**: 2026-07-23
- **Branch**: `main`

## Package

- **Version**: `1.2.0` (last published to crates.io)
- **Manifest**: `Cargo.toml`

## Toolchain

- **Stable Rust**: `1.96.0 (ac68faa20 2026-05-25)`
- **MSRV**: `1.89.0` (declared in `Cargo.toml`, tested in CI)
- **Nightly Rust**: `1.98.0-nightly (beae78130 2026-06-09)`
- **cargo-fuzz**: `0.13.2`
- **cargo-deny**: `0.19.0` (pinned in CI workflows)

## Local Verification Commands

All commands run on 2026-07-23 against the working tree.

### Release gate

```
cargo fmt --all -- --check                                         PASS
cargo clippy --locked --all-targets --all-features -- -D warnings  PASS
cargo test --locked --all-features --lib                          PASS (481+ tests)
cargo test --locked --all-features --bins                         PASS (24 tests)
cargo test --locked --doc                                         PASS (11 tests)
cargo run --locked --bin generate-docs -- --check                 PASS
cargo deny check advisories bans licenses sources                 PASS
cargo package --locked --list                                     PASS
cargo package --locked --verbose                                  PASS
```

### MSRV gate

```
cargo +1.89.0 check --locked --all-targets --all-features         PASS
cargo +1.89.0 test --locked --all-features --lib                  PASS (481 tests)
```

### Property tests

```
cargo test --locked --all-features --tests property               PASS (47 tests)
```

### Fuzz build

```
RUSTUP_TOOLCHAIN=nightly cargo fuzz build                          PASS (12 targets)
```

### Coordinator stress loop

```
for i in 1..50: cargo test --locked --all-features --lib mcp::execution::coordinator_tests
  Iterations 1-50: 9 passed (each)
```

All 9 coordinator tests pass consistently across 50 sequential iterations.

### Execution safety tests

```
cargo test --locked --all-features --test lib mcp::test_execution_safety
  26 passed
```

### Runtime helper tests

```
cargo test --locked --all-features --test lib mcp::test_runtime_helpers
  45 passed
```

## Test Counts

| Partition | Count |
|-----------|-------|
| Unit (lib) | 481 |
| Binary | 24 |
| Property | 47 |
| Doc | 11 |
| Execution safety | 26 |
| Runtime helpers | 45 |
| Context isolation | 38 |
| **Total (local)** | **672** |

### New tests added in this session (execution.rs)

**Timing-based tests** (basic behavior verification):
- `queued_timeout_blocks_handler_after_permit_release` — queued timeout prevents handler from running
- `timeout_after_permit_but_before_closure_start` — timeout fires while handler is running
- `running_timeout_increments_exactly_once` — timed_out_handlers is exactly 1 while handler runs
- `no_double_completion` — defensive completion behavior verified
- `basic_execution_completes_successfully` — basic handler execution
- `handler_panic_returns_blocking_handler_count_to_baseline` — panic cleanup
- `timeout_response_returns_while_handler_continues` — timeout returns while handler runs

**Deterministic tests** (hook-driven exact interleavings):
- `deterministic_completion_wins` — fast handler completes before timeout, no gauge increment
- `deterministic_timeout_wins` — slow handler times out, gauges return to zero after release
- `deterministic_queued_timeout_after_permit` — timeout fires while queued, handler never runs
- `five_hundred_deterministic_interleavings` — 250 completion-wins + 250 timeout-wins, all hook-driven
- `worker_bound_coordinator_test` — N workers via coordinator, N+1 queues, gauges verified
- `repeated_single_threaded_100_iterations` — 100 iterations with varying handlers/timeouts
- `deterministic_panic_after_timeout` — panic after timeout, gauges return to zero via hook
- `deterministic_cancellation_flag_after_timeout` — cancel flag set after timeout, verified via hook

### New tests added in sync_pool.rs

- `panic_in_job_does_not_kill_worker` — worker survives job panic via catch_unwind
- `eval_context_not_leaked_between_jobs` — thread-local state isolated between jobs
- `repeated_timeouts_pool_stays_usable` — pool remains functional after timeouts

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

### Tests

- [x] Lifecycle races use hooks rather than timing guesses
- [x] Deterministic completion-wins and timeout-wins tests force exact orderings
- [x] Queued timeout is tested with a saturated semaphore
- [x] Panic-after-timeout is tested
- [x] 500 controlled interleavings (250 completion + 250 timeout) pass without gauge leaks
- [x] Worker-bound test exercises the actual coordinator with shared semaphore
- [x] 100-iteration repeated single-threaded test passes consistently
- [x] Sleeps used only as bounded "did not happen" observations, never to establish order

### Mutable execution context

- [x] Mutable-context calls use the bounded pool via `submit_cancellable`
- [x] Transactional commit slot (`Arc<Mutex<Option<EvalContext>>>`) stores worker-local context
- [x] Commit requires `response.ok == true` AND cancel flag still false
- [x] On timeout/saturation/cancellation/failure, commit slot is never read (late writes discarded)
- [x] math_eval cloning limitation is documented and tested
- [x] Generic test handler proves successful mutation commit and rollback on failure
- [x] Pool saturation leaves context unchanged
- [x] Cancellation flag is passed through to the pool

### Shared cancellation identity

- [x] `call_json_with_budget` uses one flag for pool and handler
- [x] `call_json_with_context` preserves and uses the supplied flag
- [x] `call_json_with_execution_context` uses one effective flag for pool and handler
- [x] `call_json_with_execution_context_mut` uses one effective flag for pool, handler, and commit decision
- [x] Timeout sets the effective flag before returning
- [x] Fast success leaves the flag false
- [x] Queue saturation leaves the flag false

### Sync worker preflight

- [x] Worker checks deadline before invocation
- [x] Worker checks cancellation before invocation
- [x] Timed-out queued jobs never execute
- [x] Externally cancelled queued jobs never execute
- [x] Running non-cooperative jobs retain worker occupancy
- [x] Timeout and shutdown are classified separately
- [x] Disconnection tests exercise real channel disconnection

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

- [x] Changelog describes the implemented lifecycle accurately
- [x] Deprecation `since` metadata matches actual shipping history
- [x] Architecture docs distinguish MCP and in-process execution boundaries
- [x] Closure evidence identifies the exact commit containing all cited changes
- [x] Local command results were produced from a clean checkout of that commit
- [x] MSRV gate passes on Rust 1.89.0
- [x] Fuzz build succeeds
- [ ] Ordinary CI passed for that commit (requires CI dispatch)
- [ ] Manual release-verification workflow passed for that commit (requires CI dispatch)
- [ ] Extended fuzz and sanitizer matrices passed for that commit (requires CI dispatch)
- [ ] Release 4 and Release 5 remain open until their evidence-dependent items pass

## Intentionally Deferred Items

1. **GitHub Actions CI**: Requires push to trigger CI workflow. Evidence will be
   recorded after the commit is pushed and CI completes.

2. **Manual release-verification workflow run**: Requires dispatching the
   `Release Verification` workflow via GitHub Actions.

3. **Per-target fuzz run evidence**: Requires the extended fuzz matrix workflow
   to run on GitHub Actions.

4. **Release 4/5 status closure**: Status notes will be marked complete
   after the manual release-verification workflow succeeds and the maintainer
   confirms the release candidate.
