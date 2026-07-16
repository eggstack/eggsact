# Release 1 — Execution Safety and Concurrent Request Integrity

## Purpose

This release closes the highest-risk operational defects in the current MCP server: worker permits released before timed-out blocking handlers terminate, cancellation notifications subjected to ordinary request handling, cancellation loss under lock contention, and duplicate active request IDs overwriting request state.

The result must be a server whose configured concurrency limits describe actual execution, not merely the number of request futures still awaiting results.

## Goals

1. Make `MAX_TOOL_WORKERS` a hard upper bound on running blocking tool handlers.
2. Preserve worker occupancy accounting after client-facing timeouts.
3. Make cancellation response-free, quota-independent, and race-resistant.
4. Reject duplicate in-flight request IDs.
5. Guarantee active-request cleanup across normal return, error, panic, timeout, writer failure, and shutdown.
6. Add stress tests that reproduce the failure modes rather than only unit-testing helpers.

## Non-goals

- Do not implement MCP version negotiation in this release.
- Do not redesign the complete tool-handler ABI.
- Do not introduce process isolation or killable subprocess workers.
- Do not increase concurrency limits to mask accounting defects.
- Do not weaken timeout or cancellation behavior for Python parity.

# Workstream 1 — Blocking worker containment

## Current defect

`server.rs` acquires a semaphore permit in an async future and then awaits a `spawn_blocking` join handle under `tokio::time::timeout`. When the timeout drops that async future, the permit can be released while the blocking closure continues. Repeated timed-out calls can therefore create more live blocking computations than `MAX_TOOL_WORKERS`.

## Implementation

1. Change the worker semaphore to use owned permits for tool execution.
2. Acquire the permit before spawning blocking work.
3. Move the owned permit into the `spawn_blocking` closure.
4. Retain the permit until the handler closure exits, including after the response timeout.
5. Keep the client-facing timeout outside the join wait so a timeout response can still be returned promptly.
6. Set the cooperative cancellation flag when the deadline expires.
7. Ensure the detached join handle is observed or accounted for so panics are not silently lost.

Suggested shape:

```rust
let permit = semaphore.clone().acquire_owned().await?;
let handle = tokio::task::spawn_blocking(move || {
    let _permit = permit;
    with_cancel_flag(Some(cancel_flag), || handler(&args))
});

match tokio::time::timeout(deadline, handle).await {
    Ok(joined) => { /* normal mapping */ }
    Err(_) => { /* set cancellation; return timeout; permit remains in closure */ }
}
```

The final implementation may wrap this in a helper, but permit lifetime must remain obvious in code review.

## Runtime accounting

Add internal atomic counters or a small runtime metrics structure for:

- active request tasks;
- active blocking handlers;
- timed-out handlers still running;
- total timeout responses;
- peak blocking-handler concurrency.

Counters must use RAII guards so decrement occurs on panic/unwind. Expose them only through diagnostics/debug surfaces. Avoid adding model-facing tool noise.

## Nested timeout audit

Review:

- `src/tools/helpers.rs::run_with_timeout`;
- regex tool execution;
- any explicit `std::thread::spawn` or nested `spawn_blocking` use;
- composite tools that invoke heavy sub-operations.

Document any thread that can outlive the tool handler. Where practical, remove nested detached timeout mechanisms in favor of the outer budget/cancellation system. If a helper must remain, give it bounded occupancy and cooperative cancellation.

# Workstream 2 — Notification and cancellation routing

## Required ordering

Refactor the read loop into clear stages:

1. Frame-size check.
2. JSON parse and top-level validation.
3. Extract method and determine request versus notification.
4. Handle control notifications.
5. Apply ordinary request rate limiting only to requests.
6. Validate ID and in-flight limits.
7. Register and dispatch the request.

## Notification rules

- Never emit a JSON-RPC response for a notification.
- `notifications/cancelled` must bypass the ordinary request rate limiter.
- Unknown notifications should be ignored after bounded validation.
- Malformed cancellation notifications should be ignored and optionally logged to stderr; they should not receive error responses.
- `notifications/initialized` behavior remains compatible with the current lifecycle until Release 2.

## Cancellation locking

Replace `apply_cancellation` using `try_lock()` with an async helper:

```rust
pub async fn apply_cancellation(active: &ActiveRequests, id: &Value) -> bool
```

Implementation requirements:

1. Validate ID type and size before locking when possible.
2. Await the active map lock.
3. Clone the target `Arc<AtomicBool>`.
4. Release the lock.
5. Set the flag outside the critical section.
6. Return whether a live request was found.

No valid cancellation may be dropped because the map was briefly locked.

## Cancellation state coverage

Test cancellation while a request is:

- registered but waiting for a worker permit;
- actively running;
- timed out but still running;
- finishing response serialization;
- already complete;
- unknown to the server.

For waiting requests, the request must check cancellation before spawning blocking work and return `CANCELLED` without consuming a worker if cancellation already occurred.

# Workstream 3 — Duplicate request IDs and active-map integrity

## Duplicate-ID policy

Reject a request whose non-null ID already exists in the active map. The check and insertion must occur under one lock acquisition.

Do not use separate `contains_key` and `insert` lock windows.

Suggested API:

```rust
enum RegisterRequestError {
    DuplicateId,
    CapacityExceeded,
}

async fn register_request(...) -> Result<RequestGuard, RegisterRequestError>
```

A guard should own cleanup responsibility. On drop, it removes only the entry corresponding to its own request generation/token, preventing an old task from removing a later request if ID reuse occurs after completion.

## Null IDs

Make an explicit decision in this release. Preferred policy: reject `id: null` for requests because concurrent tracking and error correlation become ambiguous. Notifications are represented by an absent ID, not null.

If compatibility requires accepting null, prevent more than one null-ID request from being active and document the limitation. Add a decision note in `architecture/mcp-server.md`.

## Cleanup guard

Introduce an RAII active-request guard or equivalent finally-style cleanup. It must handle:

- normal completion;
- handler error;
- join error;
- panic unwind;
- timeout response;
- output serialization failure;
- writer channel closure;
- server shutdown.

For timed-out handlers, distinguish removal from the client-visible active request map from the separate running-blocking-handler metric. Cancellation lookup after timeout may remain available until the handler exits if that improves cooperative termination; make the chosen behavior explicit and tested.

# Workstream 4 — Tests

## Deterministic test hooks

Avoid timing-only tests that depend on machine speed. Add test-only handlers or synchronization hooks using barriers/notifies so tests can control:

- handler start;
- handler release;
- permit acquisition;
- timeout occurrence;
- cancellation observation;
- handler termination.

Keep test hooks behind `#[cfg(test)]` or a non-published test-support feature.

## Required tests

### Worker containment

- Start more tasks than `MAX_TOOL_WORKERS` whose handlers wait on a barrier.
- Confirm only `MAX_TOOL_WORKERS` enter the blocking section.
- Force their response deadlines to expire.
- Submit additional tasks.
- Confirm no additional handler enters until an original blocking handler actually exits.
- Confirm peak concurrency never exceeds the configured maximum.

### Cancellation

- Saturate the ordinary request rate and then send cancellation; verify cancellation is applied.
- Hold the active-map lock and send cancellation; verify it waits rather than disappearing.
- Send malformed cancellation; verify no stdout response.
- Cancel a request waiting for a worker; verify it never enters the handler.
- Cancel a running cooperative handler; verify bounded termination.
- Cancel an already-completed or unknown ID; verify no response and no state corruption.

### Duplicate IDs

- Submit two concurrent string IDs with the same value; second is rejected.
- Repeat with integer IDs.
- Reuse the ID after the first request completes; reuse succeeds.
- Verify cancellation targets the correct request.
- Verify the first request cleanup cannot remove the second request's entry.

### Shutdown

- Close stdin with active requests.
- Verify graceful waiting behavior is bounded and documented.
- Verify writer channel drains valid responses.
- Verify metrics and active maps return to zero after all handlers exit.

# Documentation updates

Update `architecture/mcp-server.md` with:

- response deadline versus execution lifetime;
- worker permit lifetime;
- cancellation routing and notification semantics;
- duplicate-ID policy;
- null-ID policy;
- active-request cleanup behavior;
- shutdown behavior;
- diagnostics counters.

Update `architecture/testing.md` with the deterministic concurrency harness and stress-test commands.

Add a changelog entry describing the corrected worker-bound and cancellation semantics. Treat duplicate-ID rejection as a protocol correctness fix.

# Validation sequence

Run focused tests first:

```bash
cargo test --all-features --test lib mcp::test_cancellation -- --nocapture
cargo test --all-features --test lib mcp::test_protocol -- --nocapture
cargo test --all-features --test lib mcp::test_determinism_concurrency -- --nocapture
cargo test --all-features --test lib mcp::test_hardening_and_gaps -- --nocapture
```

Then run the full repository gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Run the Python parity suite when available. Any changed parity result must be classified; do not simply extend the accepted-failure list.

# Acceptance criteria

Release 1 is complete only when:

- actual running blocking handlers never exceed `MAX_TOOL_WORKERS`;
- timed-out handlers retain occupancy until termination;
- cancellation bypasses ordinary request rate limits;
- notifications never produce responses;
- cancellation cannot be lost due to active-map lock contention;
- duplicate active IDs are rejected atomically;
- cleanup is guard-based and correct across all terminal paths;
- stress tests reproduce and close each identified race;
- diagnostics report timeout and occupancy state without exposing internal tools to model audiences;
- full verification passes.

# Handoff notes

Implement the worker-containment fix before refactoring cancellation, because cancellation tests need accurate worker-state behavior. Implement request registration and cleanup guards next, then reorder notification/rate-limit handling around those primitives. Keep commits small enough that each invariant can be reviewed independently.