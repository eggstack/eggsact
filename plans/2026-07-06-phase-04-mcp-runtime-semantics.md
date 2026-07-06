# Phase 04: MCP runtime semantics and optional concurrency upgrade

## Purpose

Clarify or improve MCP stdio runtime behavior around cancellation, timeout, request handling, and concurrency. The current server is adequate for many codegg use cases, especially when codegg uses the in-process API for hot paths, but its cancellation semantics are easy to overstate. This phase either documents the current serial model precisely or upgrades the MCP server to support true in-flight cancellation and concurrent request handling.

## Current observations

The server read loop reads one line, validates it, spawns a task for request handling, immediately awaits that task, writes the response, and then reads the next line. This means MCP request handling is effectively serial at the read-loop level. Although tool execution uses `spawn_blocking` and a semaphore, the server does not continue reading stdin while the current request is in progress.

`notifications/cancelled` inserts a request ID into a cancellation set. `tools/call` checks that set before execution starts. Because the read loop is serial, a cancellation notification sent while a tool is already running will not be read until after the running request completes or times out. Therefore, current JSON-RPC cancellation is mostly pre-start cancellation.

Timeout handling wraps a blocking tool task in `tokio::time::timeout`. When timeout fires, the server sets a cooperative cancellation flag and returns a timeout response, but the blocking worker may continue because Rust cannot kill a running thread. This is acceptable if documented honestly, but it is not a hard CPU stop.

## Decision point

Before implementation, decide which path the project wants.

### Option A: preserve serial MCP and document it precisely

Choose this if codegg will use the in-process API for hot paths and MCP is primarily for simple external integrations. This option is simpler and lower risk.

Required work:

- Update architecture docs to state that MCP cancellation notifications only affect queued/pre-start requests in the current serial read-loop model.
- State that timeout responses set a cooperative cancellation flag, but blocking worker termination is best-effort.
- Avoid wording that implies in-flight cancellation works over stdio in the current server.
- Add tests that document current behavior where feasible.

### Option B: implement concurrent MCP request handling and true in-flight cancellation

Choose this if external MCP clients need long-running tool cancellation, concurrent request processing, or out-of-order JSON-RPC responses.

Required work:

- Refactor the read loop to continue reading stdin while prior requests execute.
- Maintain active request state keyed by request ID.
- Store a cancellation flag per in-flight request.
- Process `notifications/cancelled` by setting the active request flag immediately.
- Serialize stdout writes through a dedicated writer task or an async mutex.
- Bound in-flight requests to prevent unbounded memory growth.
- Clean active request entries on completion, timeout, parse rejection, and errors.

## Option A implementation plan: documentation and tests

1. Update `architecture/mcp-server.md` concurrency section.

   Explicitly define:

   - Serial read-loop behavior.
   - Tool semaphore scope.
   - Pre-start cancellation behavior.
   - Timeout-triggered cooperative cancellation behavior.
   - Recommendation to use in-process API for codegg hot paths.

2. Update server comments.

   In `server.rs`, clarify the comments around `notifications/cancelled`, timeout, and cancellation flags. Avoid implying that a cancellation notification can interrupt an already-running request under the serial loop.

3. Add runtime behavior tests if practical.

   The tests do not need to simulate full stdio if that is difficult. At minimum, unit-test `CancelledRequests` semantics and request-ID validation. If there are existing async server tests, add a serial-cancellation documentation test.

4. Add diagnostics note.

   If `runtime_diagnostics` or CLI diagnostics can expose concurrency mode, add a field such as `mcp_request_mode: "serial"` and `cancellation_mode: "pre_start_plus_timeout_cooperative"`. If adding fields is too much for this phase, document only.

## Option B implementation plan: concurrent request handling

1. Introduce request state.

   Add a structure like:

   ```rust
   struct ActiveRequest {
       cancel_flag: Arc<AtomicBool>,
       started_at: Instant,
       method: String,
   }
   ```

   Store active requests in `Arc<Mutex<HashMap<ValueOrNormalizedId, ActiveRequest>>>`. Normalize request IDs carefully because JSON-RPC IDs can be strings, integers, or null for requests where cancellation is meaningful only if ID exists.

2. Split read and write paths.

   Use an `mpsc` channel for responses. The read loop validates input and spawns request tasks without awaiting them. A single writer task receives serialized JSON values and writes one line per response to stdout. This prevents interleaved stdout writes.

3. Preserve notification semantics.

   Notifications without IDs should not produce responses. `notifications/cancelled` should set the cancellation flag if the request is active; if not active, it may record a short-lived pre-start cancellation marker for a bounded time or bounded count.

4. Bound concurrency.

   Keep `MAX_TOOL_WORKERS` for blocking tool execution. Add a separate `MAX_IN_FLIGHT_REQUESTS` if needed so many cheap requests do not accumulate unbounded tasks. Return a JSON-RPC error when overloaded.

5. Integrate cancellation flags.

   Pass the per-request flag through `budget::with_cancel_flag` for handlers. Ensure timeout also sets the same flag. Ensure cooperative checks in tools can observe it.

6. Preserve output ordering expectations.

   JSON-RPC allows responses to be returned as requests complete, but some clients may implicitly expect order. Document that concurrent mode may return out-of-order responses unless the project chooses an ordered-response queue. Ordered responses are simpler for clients but can reintroduce head-of-line blocking.

7. Test concurrent behavior.

   Add tests for:

   - Two requests can be in flight.
   - A cancellation notification marks an active request cancelled.
   - Timeout marks the same cancel flag.
   - Responses are valid one-line JSON and never interleave.
   - Active request map is cleaned after completion.
   - Overload behavior is bounded and structured.

## Acceptance criteria for Option A

- Architecture docs accurately describe serial MCP behavior.
- Server comments no longer overstate in-flight cancellation.
- Diagnostics/docs recommend in-process API for codegg hot paths.
- Tests cover request-ID/cancellation-set behavior as feasible.
- Existing CI passes.

## Acceptance criteria for Option B

- MCP server can continue reading stdin while a tool request runs.
- `notifications/cancelled` can cancel an active request cooperatively.
- Stdout responses are serialized safely.
- In-flight request state is bounded and cleaned.
- Timeout and explicit cancellation share the same cancellation flag path.
- Tests cover concurrency, cancellation, timeout, cleanup, and output validity.
- Existing behavior for ordinary single-request clients remains compatible.

## Risks and constraints

Option B is materially more invasive. It changes response ordering and can expose client assumptions. Do not choose it unless external MCP behavior requires it. For codegg, prefer the in-process API for performance and deterministic harness control. If implementing Option B, keep a feature flag or documented mode only if the complexity is justified.

## Handoff notes

Start this phase with a decision. If uncertain, choose Option A now and file Option B as future work. The current architecture document already acknowledges serial behavior, so a precise documentation pass may be sufficient until a real MCP client requires in-flight cancellation.
