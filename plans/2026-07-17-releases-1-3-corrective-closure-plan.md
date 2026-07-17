# Releases 1–3 Corrective Closure Plan

## Purpose

This plan closes the remaining correctness gaps identified after implementation of:

- Release 1 — Execution Safety
- Release 2 — MCP Protocol Maturity
- Release 3 — State and Policy Correctness

The broad architecture is now sound. This pass is intentionally narrow. It should not add new tools, expand protocol scope, redesign the registry, or begin the later CI/fuzzing/feature-decomposition roadmap. Its purpose is to make the first three releases internally consistent, operationally bounded, and accurately documented.

## Current state

The following work has landed correctly:

- MCP blocking handlers hold owned semaphore permits until the outer `spawn_blocking` closure exits.
- Cancellation notifications bypass the ordinary request rate limiter and use awaited active-request locking.
- Duplicate request IDs and in-flight capacity checks are performed atomically.
- Request cleanup and thread-local context restoration use RAII guards.
- MCP initialization, lifecycle enforcement, protocol-version negotiation, and extension advertisement are implemented.
- MCP dispatch no longer mutates process-global calculator mode.
- Profile and audience policy enforcement is shared and metadata lookup is audience-aware.
- Immutable execution-template semantics are explicit.

Four issues remain:

1. Some handlers still use inner timeout threads that can outlive the outer handler and escape `MAX_TOOL_WORKERS` accounting.
2. `timed_out_handlers` metrics can become permanently inflated in a near-deadline completion race.
3. `call_json_with_execution_context_mut` advertises persistent calculator state, but `math_eval` clones the evaluator context internally and does not persist state.
4. MCP client capabilities are parsed at initialization but are not retained in negotiated session state.

A smaller documentation/API cleanup is also required around lifecycle helpers and the mutable-context contract.

---

# Goals

1. Ensure every CPU-consuming timeout worker is covered by an explicit concurrency bound for its full lifetime.
2. Make timeout metrics exact under all completion/timeout interleavings.
3. Either make mutable execution context genuinely persistent for calculator-backed tool calls or remove the misleading contract before release.
4. Retain negotiated client capabilities in session state for future capability-dependent behavior.
5. Reconcile changelog, README, architecture, and rustdoc with actual behavior.
6. Add focused race, stress, and state-persistence tests that prevent regression.
7. Produce explicit verification evidence sufficient to close Releases 1–3.

# Non-goals

This pass must not:

- Add additional MCP protocol revisions beyond those already supported.
- Add sampling, roots, elicitation, or server-initiated capability-dependent features.
- Add new deterministic tools.
- Begin Cargo feature decomposition or crate splitting.
- Add broad fuzzing or benchmark infrastructure except for narrowly targeted regression tests.
- Replace the entire evaluator architecture unless a smaller safe change is impossible.
- Weaken timeouts, cancellation, input limits, output limits, or profile/audience enforcement.

---

# Workstream 1 — Eliminate or contain nested timeout workers

## Problem

The outer MCP runtime now correctly holds an owned semaphore permit until the `spawn_blocking` handler exits. However, some handler paths call a helper such as `run_with_timeout` that creates an inner `std::thread::spawn` worker.

If the inner helper returns a timeout while the inner thread continues running, the outer handler can exit and release the MCP worker permit before the inner CPU-consuming thread terminates. Repeated calls can therefore exceed `MAX_TOOL_WORKERS` despite the outer fix.

This is a real resource-containment defect, not a documentation-only limitation.

## Required audit

Search all production code for:

- `std::thread::spawn`
- `thread::spawn`
- `spawn_blocking`
- `run_with_timeout`
- channel-based timeout helpers
- timeout wrappers that abandon owned work
- regex or parser helpers that use detached workers

Document every result in the implementation commit or closure note with:

- Caller/tool name.
- Worker type.
- Maximum lifetime.
- Whether it can outlive its caller.
- Concurrency bound.
- Cancellation behavior.

## Preferred design

The preferred result is to remove nested detached timeout workers from MCP-dispatched handlers.

Where possible:

1. Execute the synchronous operation directly inside the already bounded MCP `spawn_blocking` closure.
2. Use `BudgetContext::should_stop()` or equivalent cooperative checks within loops.
3. Let the outer MCP timeout determine when the client receives a timeout response.
4. Keep the outer semaphore permit until the synchronous work actually exits.

This produces one clear resource boundary and avoids stacked timeout systems.

## Acceptable alternative

If a nested worker cannot be removed because an underlying library call is non-cooperative, introduce a separate process-wide bounded executor or semaphore specifically for abandonable inner work.

Requirements for this alternative:

- The inner worker must acquire an owned permit before starting.
- The permit must live inside the inner thread until it exits.
- The bound must be explicit and documented.
- The inner worker count must be included in diagnostics.
- Repeated timeouts must not create more active inner workers than the configured bound.
- Queueing behavior must be deterministic when the inner-worker pool is saturated.

Do not solve this with an unbounded global thread pool.

## Calculator-specific decision

`math_eval` currently appears to use a timeout helper that requires a `'static + Send` closure and clones evaluator state.

Refactor this path so it does not require abandoning an unbounded inner thread. Candidate designs:

### Option A — Outer-runtime timeout only

For MCP dispatch, call the evaluator directly inside the bounded outer `spawn_blocking` closure and rely on evaluator cancellation checkpoints plus the outer timeout response.

The in-process calculator API may retain a separate timeout helper if it is clearly documented and independently bounded.

### Option B — Bounded evaluator worker

Create a bounded evaluator worker abstraction that owns both the evaluator context and its concurrency permit. Return the resulting context with the evaluation result.

Option A is preferable if evaluator loops already support practical cooperative cancellation.

## Tests

Add deterministic tests that use an injected test handler or test-only delay hook.

Required assertions:

- Repeated timed-out calls never cause actual CPU worker concurrency to exceed the configured outer plus explicitly documented inner bounds.
- A timed-out inner operation retains its permit until it exits.
- Requests submitted after saturation wait or fail according to the documented policy.
- Cancellation remains processable while all worker slots are occupied.
- Shutdown behavior is explicit when inner workers are still running.

Avoid wall-clock-fragile tests. Use barriers, channels, latches, or test-only hooks to control worker progress.

## Acceptance criteria

- No production timeout path can create an unbounded worker that outlives its accounting permit.
- All remaining thread-spawn sites have documented bounds and lifetimes.
- A stress test proves the configured concurrency bound under repeated timeouts.
- `architecture/mcp-server.md` no longer describes uncontained nested threads as an accepted limitation.

---

# Workstream 2 — Make timeout metrics race-free

## Problem

The current timeout path uses an atomic boolean plus separate increments and decrements:

- The outer timeout branch marks the handler timed out and increments `timed_out_handlers`.
- The blocking closure checks the boolean before exit and decrements if it observes the timeout.

A near-deadline race can allow the blocking closure to finish before seeing the timeout flag while the outer timeout branch still increments afterward. The counter then remains permanently nonzero.

## Required state model

Replace the boolean protocol with one atomic lifecycle state.

Suggested states:

```rust
const HANDLER_RUNNING: u8 = 0;
const HANDLER_TIMED_OUT: u8 = 1;
const HANDLER_FINISHED: u8 = 2;
```

Suggested transitions:

### Timeout path

Use `compare_exchange(HANDLER_RUNNING, HANDLER_TIMED_OUT, ...)`.

- On success: increment `total_timeouts` and `timed_out_handlers`.
- If current state is `HANDLER_FINISHED`: increment `total_timeouts` only if the client actually received a timeout response, but do not increment `timed_out_handlers`.
- No other path may increment `timed_out_handlers`.

### Handler exit path

Use `swap(HANDLER_FINISHED, ...)`.

- If previous state was `HANDLER_TIMED_OUT`, decrement `timed_out_handlers`.
- If previous state was `HANDLER_RUNNING`, do not touch the timed-out counter.

The exact encoding may differ, but the implementation must guarantee that each increment has exactly one matching decrement.

## Metric semantics

Document precise meanings:

- `active_requests`: registered request tasks currently in the active map or task lifecycle, whichever is chosen. Make implementation and docs agree.
- `active_blocking_handlers`: currently executing outer blocking closures.
- `timed_out_handlers`: handlers for which a timeout response has been returned and whose underlying work is still running.
- `total_timeouts`: cumulative timeout responses returned.
- `peak_blocking_concurrency`: peak outer blocking-handler concurrency.

If nested workers remain, add separate metrics rather than overloading `active_blocking_handlers`.

## Tests

Add unit tests for every state transition and an integration stress test around the deadline boundary.

Required cases:

- Handler exits before timeout.
- Timeout occurs before handler exit.
- Handler exit and timeout race repeatedly.
- Handler panics after timeout.
- Cancellation causes exit after timeout.
- Metrics return to zero after all handlers terminate.
- `total_timeouts` remains monotonic and equals the number of timeout responses generated by the test.

Use at least several hundred controlled race iterations if the test remains deterministic and fast.

## Acceptance criteria

- `timed_out_handlers` cannot leak or underflow.
- Every possible state transition is unit-tested.
- After all test workers finish, active and timed-out gauges are zero.
- Metric documentation exactly matches implementation.

---

# Workstream 3 — Correct the mutable execution-context contract

## Problem

`call_json_with_execution_context_mut` is described as persisting PRNG draws, registers, and variables. However, `math_eval`, currently the sole evaluator-context consumer, clones the context internally for timeout execution. As a result, the new API does not provide the persistent state it advertises.

The changelog and top-level API wording therefore overstate actual behavior.

## Required decision

Choose and implement one of the following. Do not leave the current hybrid contract.

## Preferred option — Make persistence real

Refactor the calculator-backed handler path so mutable dispatch returns updated evaluator state.

A viable design is:

1. Add an internal calculator execution function that accepts an owned `EvalContext`.
2. Execute the expression against that owned context.
3. Return a structure containing:
   - `ToolResponse`
   - updated `EvalContext`
   - terminal status sufficient to determine commit/rollback
4. In `call_json_with_execution_context_mut`, replace `ctx.eval_ctx` only when the transaction policy says mutations should commit.

Possible internal result:

```rust
struct StatefulHandlerResult {
    response: ToolResponse,
    eval_ctx: EvalContext,
    commit: bool,
}
```

Do not expose this type publicly unless there is a clear API reason.

### Transaction policy

Define exact behavior:

- Unknown tool: no mutation.
- Profile or audience rejection: no mutation.
- Invalid arguments: no mutation.
- Input budget rejection: no mutation.
- Parse failure: no mutation.
- Evaluation failure: no mutation unless the evaluator already defines partial side effects as intentional.
- Successful evaluation: commit all evaluator state changes.
- Cancellation before execution: no mutation.
- Cancellation during execution: no mutation unless a clearly documented transactional checkpoint model is implemented.
- Timeout: no mutation, even if abandoned underlying work later finishes.
- Panic/internal error: no mutation.

The simplest safe model is copy-on-execute and commit-on-success.

### Handler signature constraint

The generic registry handler type currently appears to be stateless (`fn(&Value) -> ToolResponse`). Do not force all tools into a mutable handler signature merely for one calculator tool unless the broader design benefits are clear.

Preferred approaches:

- Add an internal stateful dispatch path only for tools that declare evaluator-context support.
- Add a stateful handler table parallel to the ordinary handler table.
- Special-case `math_eval` in one well-documented internal adapter while preserving shared profile/audience/schema policy.

Avoid duplicating policy checks between ordinary and stateful dispatch.

## Fallback option — Withdraw the persistence claim

If making stateful tool dispatch correct would require disproportionate redesign:

- Remove or deprecate `call_json_with_execution_context_mut` before publishing the claimed contract.
- Rename it if it remains useful only for temporary direct borrowing.
- Remove changelog claims of persisted PRNG/register/variable state.
- Direct callers to `evaluate_with_context` and `run_with_context` for persistent calculator sessions.
- Add an explicit future-roadmap note rather than a misleading partial API.

Because version `1.3.0` is already documented in the changelog, determine whether it has actually been published. If unpublished, correct the changelog directly. If published, add an unreleased correction and follow semver policy.

## Tests for preferred option

Required tests using `call_json_with_execution_context_mut`:

- Variable assignment persists into the next call.
- Memory register changes persist.
- Seeded PRNG advances across sequential calls rather than restarting.
- Two separate mutable contexts remain isolated.
- Immutable template dispatch remains reproducible and does not mutate its template.
- Parse failure does not mutate state.
- Invalid argument failure does not mutate state.
- Cancellation does not mutate state.
- Timeout does not commit late-arriving state.
- Concurrent use of distinct contexts is safe.

If the API requires exclusive `&mut` access, document that one context cannot be used concurrently without external synchronization.

## Acceptance criteria

Preferred option:

- The mutable API demonstrably persists calculator state across tool calls.
- Failure, cancellation, timeout, and panic paths follow documented rollback semantics.
- Immutable and mutable APIs have clearly distinct tests and documentation.

Fallback option:

- No public API or changelog text claims persistence that does not occur.
- Persistent calculator state remains available through the direct calculator context APIs.
- A future implementation path is documented without presenting it as current behavior.

---

# Workstream 4 — Retain negotiated client capabilities

## Problem

Initialization parses `ClientCapabilities`, but the negotiated session state retains only protocol version and client implementation metadata.

This prevents future dispatch or server behavior from consulting what the client actually advertised.

## Implementation

Extend `NegotiatedProtocol` or replace it with a more complete session structure containing:

```rust
pub struct NegotiatedProtocol {
    pub version: String,
    pub client_name: String,
    pub client_version: Option<String>,
    pub client_capabilities: ClientCapabilities,
}
```

If `ClientCapabilities` is mutable or large, use an owned immutable representation appropriate for session lifetime.

Add accessors such as:

```rust
pub fn client_capabilities(&self) -> Option<&ClientCapabilities>
```

Do not add behavior for roots, sampling, or elicitation in this pass. Retention is sufficient.

## Forward compatibility

Unknown client capability fields are currently tolerated through value-based fields. Preserve that behavior.

Consider retaining the full raw capability object in addition to typed known fields if doing so materially improves forward compatibility. If not, document that known capability groups are retained and unknown top-level fields are ignored.

## Tests

Required cases:

- Empty capability object survives initialization and state transitions.
- Known roots/sampling/elicitation/experimental values survive initialization.
- Capability data remains available in `Ready` state.
- Duplicate initialization does not overwrite existing negotiated capabilities.
- Unsupported protocol negotiation still retains the capabilities from the accepted initialize request.
- Malformed known capability fields are rejected consistently with current typed parsing policy.

## Acceptance criteria

- Parsed client capabilities are stored for the entire session.
- Lifecycle transitions preserve capability data.
- No new optional protocol behavior is falsely advertised or invoked.

---

# Workstream 5 — Lifecycle and dead-helper cleanup

## Initialized notification helper

Review `initialized_before_initialize(id)` and similar lifecycle response helpers.

A valid JSON-RPC notification has no response. Therefore:

- Do not send a JSON-RPC error for a notification-form `notifications/initialized` received in the wrong state.
- Log or record the protocol violation internally.
- If a client incorrectly sends the notification method with an ID, treat it as a malformed request according to an explicit policy and test it separately.

Remove dead helpers that cannot be reached under the final policy.

## State documentation

Ensure documentation distinguishes:

- Request methods that receive errors.
- Notifications that are ignored or logged without response.
- Lifecycle state transitions.
- Duplicate initialize request behavior.
- Initialized notification before initialize.
- Initialized notification after readiness.

## Acceptance criteria

- No notification path writes a JSON-RPC response.
- Lifecycle helper functions correspond to reachable behavior.
- Tests cover wrong-state request and notification forms separately.

---

# Workstream 6 — Documentation and release reconciliation

Update all sources of truth together:

- `CHANGELOG.md`
- `README.md`
- `architecture/mcp-server.md`
- `architecture/agent-api.md`
- `architecture/calculator.md`
- `docs/library-api.md`
- `docs/compatibility-policy.md` if semver behavior changes
- relevant rustdoc in `src/agent/mod.rs`
- diagnostics documentation if metrics change

## Required documentation corrections

### Resource containment

State the actual worker model after the corrective change. Do not claim a hard concurrency bound if any unaccounted worker can outlive its permit.

### Timeout metrics

Define each metric precisely and state whether gauges include queued, running, timed-out, or nested work.

### Mutable context

Ensure all documentation says one consistent thing:

- Either mutable tool dispatch persists calculator state with explicit transaction semantics.
- Or it does not, and the API is removed/renamed/deprecated accordingly.

Do not retain contradictory sections where the API claims persistence and then disclaims it for the only relevant tool.

### Client capabilities

Document that client capabilities are retained but not yet used to initiate optional server behaviors.

### Versioning

Confirm whether `1.1.5`, `1.2.0`, and `1.3.0` were published to crates.io.

- If unpublished, keep changes under `[Unreleased]` or reconcile version sections with the intended next release.
- If published, add corrections under a new unreleased section and follow semver compatibility constraints.

Do not rewrite published release history inaccurately.

## Acceptance criteria

- Changelog and rustdoc match behavior.
- No contradictory mutable-context claims remain.
- Release history accurately reflects published artifacts.
- Architecture documentation describes only bounded worker behavior.

---

# Suggested implementation sequence

1. Audit all worker-spawning and timeout sites.
2. Choose and document the nested-worker containment strategy.
3. Implement nested-worker containment or eliminate inner workers.
4. Replace timeout boolean accounting with atomic lifecycle state.
5. Add metric transition unit tests and near-deadline race tests.
6. Decide the mutable-context contract.
7. Implement real state commit/rollback, or withdraw the API claim.
8. Add mutable/immutable context regression tests.
9. Store client capabilities in negotiated session state.
10. Remove or reconcile dead lifecycle helpers.
11. Update all documentation and changelog entries.
12. Run focused tests, full local verification, and package checks.

Keep commits reviewable by workstream. A reasonable commit series is:

1. `fix(runtime): contain nested timeout workers`
2. `fix(metrics): make timeout accounting race-free`
3. `fix(agent): correct mutable execution context semantics`
4. `fix(mcp): retain negotiated client capabilities`
5. `docs: reconcile releases 1-3 closure contracts`

---

# Required test matrix

## Runtime containment

- Outer worker saturation.
- Repeated client-facing timeouts.
- Nested-worker saturation if nested workers remain.
- Cancellation while saturated.
- Shutdown while timed-out work remains.
- No concurrency above configured bounds.

## Metrics

- Complete-before-timeout.
- Timeout-before-complete.
- Near-simultaneous completion and timeout.
- Panic after timeout.
- Cancellation after timeout.
- Gauge cleanup to zero.

## Mutable context

- Variable persistence.
- Register persistence.
- PRNG advancement.
- Context isolation.
- Immutable reproducibility.
- Parse-error rollback.
- Policy-error rollback.
- Cancellation rollback.
- Timeout rollback.

## Lifecycle/capabilities

- Capabilities retained through both lifecycle transitions.
- Duplicate initialize preserves original state.
- Wrong-state notifications produce no response.
- Wrong-state request-form lifecycle methods receive the documented error.

---

# Focused verification commands

Adapt exact test names to the implementation, but the handoff should run at minimum:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features runtime
cargo test --all-features execution_safety
cargo test --all-features context_isolation
cargo test --all-features protocol
cargo test --all-features lifecycle
cargo test --all-features
```

If the repository uses separate binary/integration jobs, also run the canonical commands documented in `docs/testing.md` and `docs/release.md`.

Run package verification:

```bash
cargo package --allow-dirty
cargo publish --dry-run --allow-dirty
```

Use the repository's documented release script only if it does not publish automatically and its behavior has been reviewed.

---

# Explicit closure criteria

Releases 1–3 may be marked closed only when all of the following are true:

## Release 1 closure

- Every worker that can outlive its caller is covered by an explicit concurrency permit for its full lifetime.
- Repeated timeout stress tests cannot exceed configured concurrency bounds.
- Cancellation remains available while workers are saturated.
- Timeout gauges return to zero after work terminates.
- No metric race can leak or underflow counters.

## Release 2 closure

- Lifecycle and version negotiation tests remain green.
- Client capabilities are retained in session state.
- Wrong-state notifications never receive responses.
- Extension capability advertisement remains accurate.

## Release 3 closure

- MCP dispatch has no process-global calculator-mode side effects.
- Registry metadata and execution policy remain audience-consistent.
- Mutable context either genuinely persists calculator state with rollback semantics or is no longer advertised as doing so.
- Immutable template dispatch remains deterministic and isolated.
- Documentation and changelog accurately describe the final contract.

## Repository gate

- Formatting passes.
- Clippy passes with warnings denied.
- Full test suite passes.
- Package and publish dry-runs pass.
- Current GitHub CI is green, or any unavailable external evidence is recorded explicitly without claiming success.
- No known correctness issue from this plan remains deferred without a written release-blocking decision.

---

# Handoff result

The implementing agent should leave a concise closure record in the final implementation commit or a status note containing:

- Worker-spawn audit results.
- Chosen nested-timeout design.
- Final metric state machine.
- Mutable-context decision and transaction semantics.
- Capability-retention representation.
- Focused test results.
- Full local gate results.
- GitHub CI result.
- Package and publish dry-run result.
- Any deliberately deferred item and why it is not release-blocking.

The expected end state is that Releases 1–3 can be treated as genuinely closed, after which work can proceed to the verification-infrastructure portion of the broader roadmap.