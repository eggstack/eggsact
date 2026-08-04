# Budget, Concurrency & Execution

How eggsact enforces resource limits, manages concurrent tool execution, and handles timeouts.

See also: [MCP Server](mcp-server.md), [Agent API](agent-api.md), [Tool Implementations](tools.md)

## Files

| File | Purpose |
|------|---------|
| `src/mcp/budget.rs` | `ToolBudget`, `BudgetTier`, `BudgetContext`, thread-local eval-context bridge, cooperative cancellation |
| `src/mcp/execution.rs` | `HandlerPhase` state machine, `execute_tool_handler()`, `execute_tool_bounded()`, runtime metrics, test hooks (`#[cfg(test)]`) |
| `src/mcp/sync_pool.rs` | `SyncExecutionPool` — bounded synchronous worker pool for in-process budget-aware APIs |
| `src/mcp/runtime.rs` | `RuntimeMetrics`, `RUNTIME_METRICS` global, `RateLimiter`, request tracking, protocol negotiation |

---

## Budget System

### ToolBudget

`ToolBudget` (`src/mcp/budget.rs`) is an enforceable resource budget with 9 numeric fields:

| Field | CHEAP | MODERATE | HEAVY | Purpose |
|-------|-------|----------|-------|---------|
| `max_input_bytes` | 1M | 1M | 1M | Max serialized input size |
| `max_output_bytes` | 1M | 1M | 2M | Max serialized output size |
| `max_text_bytes` | 100K | 100K | 100K | Max text input (byte-based, not codepoint) |
| `max_list_items` | 10K | 10K | 10K | Max list size |
| `max_regex_pattern_chars` | 1K | 1K | 1K | Max regex pattern length |
| `max_regex_samples` | 100 | 100 | 100 | Max regex match samples |
| `max_elapsed_ms` | 10K | 30K | 30K | Deadline for tool execution |
| `max_spawned_workers` | 16 | 16 | 16 | Max concurrent workers |
| `max_findings` | 100 | 100 | 100 | Max findings in response |

Builder methods (`with_max_elapsed_ms`, `with_max_findings`, `with_max_output_bytes`, `with_max_input_bytes`, `with_max_text_bytes`) allow per-tool overrides from base tiers.

### ToolCost vs ToolBudget

- **`ToolCost`** (on `ToolSpec`): descriptive metadata — `Cheap`, `Moderate`, `Heavy`
- **`ToolBudget`**: enforceable numeric limits resolved at dispatch time

`budget_for_tool(tool_name, cost)` maps cost to budget, with explicit overrides for composite tools (`edit_preflight`, `command_preflight`, `config_preflight`, `text_security_inspect`, `patch_apply_check`) that always get `HEAVY`.

### BudgetContext

`BudgetContext` combines a `ToolBudget` with a deadline (`Instant`) and optional cancellation flag (`Arc<AtomicBool>`):

```rust
let budget_ctx = crate::mcp::budget::for_handler(ToolBudget::HEAVY);
// Inside handler loops:
if budget_ctx.should_stop() {
    return budget_ctx.check_should_stop("tool_name").unwrap_err();
}
```

`should_stop()` returns `true` when either the deadline has passed or the cancellation flag is set. The `check_should_stop()` helper constructs a `ToolResponse` error with the appropriate machine code (`TIMEOUT` or `CANCELLED`).

### Thread-Local Bridges

Two thread-local bridges allow handler functions (which have signature `fn(&Value) -> ToolResponse`) to access context they cannot receive directly:

| Bridge | Purpose | Safety |
|--------|---------|--------|
| `with_cancel_flag(flag, f)` | Sets the thread's cancellation flag for the duration of `f` | Guard-owned previous value; RAII `CancelFlagGuard` restores on drop |
| `with_eval_context(ctx, f)` | Sets the thread's `EvalContext` for the duration of `f` | Guard-owned previous pointer; RAII `EvalContextGuard` restores on drop |
| `with_current_eval_context(f)` | Closure-scoped access to the current `EvalContext` | Mutable borrow exists only inside `f`; no escaping references |

`for_handler(budget)` is the recommended entry point — it creates a `BudgetContext` and automatically picks up the thread-local cancellation flag if one is set.

### Cooperative Cancellation

Cancellation is cooperative, not forceful. On timeout:
1. The MCP server sets an `Arc<AtomicBool>` flag
2. High-risk handlers check `budget_ctx.should_stop()` at pipeline stages
3. When detected, the handler returns a timeout error response
4. The handler may continue running on the blocking thread — the pool does not kill threads

Handlers that check cancellation: `edit_preflight`, `command_preflight`, `config_preflight`, `config_file_inspect`, `dependency_edit_preflight`, `text_security_inspect`.

---

## Handler Lifecycle (execution.rs)

`HandlerPhase` is a mutex-backed state machine that serializes all transitions through a single lock:

```
          ┌──────────┐
          │  Queued   │ ← initial state
          └────┬─────┘
               │ begin_running()
               ▼
          ┌──────────┐
          │  Running  │ ← handler executing
          └────┬─────┘
               │ finish()
               ▼
          ┌──────────┐
          │ Finished  │ ← terminal state
          └──────────┘
```

### Timeout Transitions

```
          ┌──────────┐         record_timeout()
          │  Queued   │ ──────────────────────→ ┌──────────────┐
          └──────────┘                          │ TimedOutQueued │
               │                                └───────┬──────┘
               │ begin_running()                         │
               ▼                                         │
          ┌──────────┐         record_timeout()          │
          │  Running  │ ──────────────────────→ ┌───────────────┐
          └────┬─────┘                          │ TimedOutRunning│
               │                                └───────┬───────┘
               │ finish()                               │ finish()
               ▼                                        ▼
          ┌──────────┐                           ┌──────────┐
          │ Finished  │ ←─────────────────────── │ Finished  │
          └──────────┘                           └──────────┘
```

Key invariant: all transitions happen under a single mutex lock, eliminating load-then-CAS gaps. The `finish()` method always runs (via `catch_unwind`) so gauges are always corrected.

### Runtime Metrics

`RUNTIME_METRICS` (`src/mcp/runtime.rs`) provides live atomic counters:

| Counter | Type | Description |
|---------|------|-------------|
| `active_requests` | AtomicUsize | Requests currently being processed |
| `active_blocking_handlers` | AtomicUsize | Blocking handlers currently executing |
| `timed_out_handlers` | AtomicUsize | Handlers that timed out while Running |
| `total_timeouts` | AtomicU64 | Total timeout attempts (including Queued) |
| `peak_blocking_concurrency` | AtomicUsize | Max concurrent blocking handlers observed |

Invariant: at synchronized snapshots, `timed_out_handlers <= active_blocking_handlers`.

RAII `MetricGuard` ensures correct decrement on panic/unwind. `snapshot_metrics()` returns a point-in-time snapshot.

---

## SyncExecutionPool (sync_pool.rs)

A bounded synchronous worker pool for in-process budget-aware APIs. **Not used by the MCP server** (which uses Tokio `spawn_blocking`).

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_SYNC_WORKERS` | 8 | Worker thread count |
| `DEFAULT_SYNC_QUEUE` | 32 | Bounded channel capacity |

### Architecture

```
call_json_with_budget()
  └→ SyncExecutionPool::submit(handler)
       ├→ Queue full? → SyncPoolError::QueueFull → RESOURCE_EXHAUSTED
       └→ Enqueued → worker picks up job
            ├→ Runs handler closure on worker thread
            ├→ Cancel flag checked via BudgetContext
            └→ Result sent back via SyncSender<ToolResponse>
                 ├→ Caller receives result within timeout → ToolResponse
                 └→ Caller timeout expires → SyncPoolError::Timeout
                      (handler may still be running on worker thread)
```

### API Routing

| Method | Pool? | Elapsed enforcement? |
|--------|-------|---------------------|
| `call_json()` | No | No |
| `call_json_with_budget()` | Yes | Yes |
| `call_json_with_context()` | Yes | Yes |
| `call_json_with_execution_context()` | Yes | Yes |

---

## MCP Server Concurrency (server.rs + runtime.rs)

The MCP stdio server reads requests serially but dispatches each as a tokio task via `JoinSet`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_IN_FLIGHT_REQUESTS` | 32 | Maximum concurrent request tasks |
| `MAX_TOOL_WORKERS` | 16 | Semaphore for concurrent blocking tool executions |
| `MAX_REQUESTS_PER_SECOND` | 10 | Token-bucket rate limiter on incoming requests |
| `MAX_REQUEST_BYTES` | 1,000,000 | Maximum request size (checked pre-dispatch) |
| `MAX_OUTPUT_BYTES` | 1,000,000 | Maximum response size (post-truncation) |

### Request Flow

```
stdin → JSON-RPC parse → rate limit check → in-flight limit check
  → request_id registration (duplicate detection)
  → spawn tokio task
    → schema validation
    → tool lookup + profile/audience check
    → Semaphore::acquire (MAX_TOOL_WORKERS)
    → spawn_blocking (handler execution)
      → BudgetContext creation
      → handler invocation
      → ToolResponse construction
    → truncate_response (MAX_OUTPUT_BYTES)
    → send to mpsc channel
  → complete_request (generation-aware cleanup)
stdout ← JSON-RPC response (may be out of order)
```

### Request Tracking

- `register_request()` atomically checks in-flight limit, duplicate ID, and inserts under a single lock
- Null IDs (`id: null`) are rejected — concurrent tracking and error correlation become ambiguous
- `complete_request()` is async and removes entries only when generation matches (prevents stale cleanup)

### Response Ordering

Responses may arrive out of request order. **Clients must correlate by JSON-RPC `id`**, not by arrival position.

---

## Timeout Model

### MCP Server Path

1. `execute_tool_bounded()` spawns the handler via `tokio::spawn_blocking`
2. A timeout task (`tokio::time::sleep`) races against handler completion
3. On timeout: the `Arc<AtomicBool>` cancel flag is set (outside the lifecycle lock)
4. Handler lifecycle transitions via mutex: `Running → TimedOutRunning`
5. `timed_out_handlers` gauge incremented
6. When handler completes: `TimedOutRunning → Finished`, gauge decremented

### In-Process Path

1. `SyncExecutionPool::submit()` sends the handler to a worker thread
2. Caller waits on `Receiver::recv_timeout(deadline)`
3. On timeout: `SyncPoolError::Timeout` returned to caller
4. Handler may continue running on the worker thread

### Load-Tolerant Budgets

Tools frequently exercised under heavy parallel test load (`math_eval`, `text_diff_explain`, `regex_finditer`) get 120s budgets instead of the standard 30s moderate budget to prevent spurious timeouts from worker starvation.

---

## Request Size Limits

| Check | Where | Limit |
|-------|-------|-------|
| Request body size | `server.rs` (pre-parse) | `MAX_REQUEST_BYTES` (1 MB) |
| Request ID length | `runtime.rs` | `MAX_REQUEST_ID_LENGTH` (1024 chars) |
| Input size | `schema_validation.rs` (pre-dispatch) | `budget.max_input_bytes` (default 1 MB) |
| Output size | `response.rs` (post-execution) | `budget.max_output_bytes` (default 1–2 MB) |
| Text size | Handler validation | `budget.max_text_bytes` (default 100 KB) |
