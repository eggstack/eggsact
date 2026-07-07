# Milestone 2: Runtime and Configuration Contract Hardening

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Harden MCP runtime configuration and execution contracts so invalid startup settings, long-running tools, and timeout/cancellation paths behave predictably. The milestone focuses on small correctness fixes and verification around environment variables, schema detail handling, resource budgets, and cooperative cancellation.

## Rationale

The MCP server already has a sound local-runtime model: request size caps, rate limiting, in-flight limits, worker semaphore, output caps, JSON-RPC ID validation, concurrent request handling, a serialized writer task, and cooperative cancellation. The remaining risk is uneven configuration validation and inconsistent cancellation polling inside heavy tool handlers.

One concrete gap is `EGGCALC_MCP_SCHEMA_DETAIL`. The runtime reads this environment variable directly into `ACTIVE_SCHEMA_DETAIL`, while the setter validates only `compact`, `normal`, or `full`. Startup behavior should not silently accept undocumented values.

A second risk is cancellation. The MCP server wraps tool work in `spawn_blocking` and uses `tokio::time::timeout`, but timed-out blocking work cannot be forcibly killed. The code correctly sets a cancel flag, so heavy handlers must poll budget/cancellation state during internal loops.

## Scope

In scope:

- Validate `EGGCALC_MCP_SCHEMA_DETAIL` at startup.
- Document runtime environment variables and accepted values.
- Audit heavy/moderate handlers for budget and cancellation polling.
- Add tests for invalid configuration behavior.
- Add representative cancellation/timeout tests for heavy tools.
- Ensure diagnostics accurately report runtime settings where feasible.

Out of scope:

- Changing MCP protocol version.
- Replacing `spawn_blocking`.
- Implementing hard thread cancellation.
- Large-scale async refactors.
- New tool additions.
- Changing profile semantics except where tests reveal incorrect behavior.

## Files likely to change

- `src/mcp/runtime.rs`
- `src/mcp/server.rs` only if additional integration hooks are needed
- `src/mcp/budget.rs`
- Heavy/moderate tool modules that do not poll cancellation sufficiently
- `src/main.rs` diagnostics output if additional config fields are exposed
- `tests/mcp/test_cancellation.rs`
- `tests/mcp/test_runtime_helpers.rs`
- `tests/mcp/test_diagnostics.rs`
- Documentation added or updated in milestone 1

## Implementation plan

### 1. Define schema detail parsing behavior

Add a small helper in `src/mcp/runtime.rs`, for example:

```rust
pub fn parse_schema_detail(s: &str) -> Option<&'static str>
```

or:

```rust
fn parse_schema_detail_or_default(s: &str) -> String
```

Recommended behavior:

- Accept exactly `compact`, `normal`, and `full`.
- For invalid values, warn to stderr and default to `full`.
- Treat empty string as invalid and default to `full`.
- Keep casing strict unless there is a strong compatibility reason to accept case-insensitive values.

This mirrors `EGGCALC_MCP_AUDIENCE`, which warns and defaults to `Model` for invalid values, while avoiding a process exit for a non-safety-critical verbosity setting.

### 2. Apply schema detail validation at startup

Change `ACTIVE_SCHEMA_DETAIL` initialization so it passes the environment value through the parser. Do not duplicate the validation logic in multiple places. `set_schema_detail()` should either call the same parser in strict mode or share the accepted-values constant.

Keep `set_schema_detail()` returning `Err` for invalid direct API calls. Startup env parsing can be lenient while programmatic setter behavior remains strict.

### 3. Add tests for schema detail parsing

Add unit tests in `runtime.rs` or integration tests in `tests/mcp/test_runtime_helpers.rs`.

Required cases:

- `compact` accepted.
- `normal` accepted.
- `full` accepted.
- `""` invalid.
- `"FULL"` invalid if strict casing is retained.
- `"verbose"` invalid.
- Whitespace-padded values are either rejected or trimmed; choose one and document it.

If the helper writes warnings to stderr, avoid brittle assertions on stderr text unless the project already has a stable pattern for this.

### 4. Audit heavy and moderate handlers for cooperative checks

Review all tools marked `ToolCost::Heavy` and all composite tools. Verify they create or inherit a `BudgetContext` and poll at meaningful internal boundaries.

Priority handlers to inspect:

- `text_security_inspect`
- `text_diff_explain`
- `structured_data_compare`
- `patch_apply_check`
- `patch_summary`
- `edit_preflight`
- `config_preflight`
- `command_preflight`
- `repo_tree_summarize`
- `identifier_table_inspect`
- Regex tools that iterate over samples or text windows

For each loop over lines, findings, paths, samples, characters, hunks, or JSON nodes, add checks such as:

```rust
if budget_ctx.should_stop() { ... }
budget_ctx.check_deadline()?;
budget_ctx.check_text_bytes(...)?;
budget_ctx.check_list_len(...)?;
```

Use existing response conventions for timeout/cancellation machine codes. Do not introduce divergent error shapes.

### 5. Add representative cancellation tests

Extend existing cancellation tests rather than inventing a new harness. The tests should verify behavior, not exact timing.

Useful patterns:

- Create a cancellation flag already set before calling an in-process heavy tool through `call_json_with_context`.
- Trigger a tool path that iterates over many items, and assert a cancellation/timeout-style machine code or controlled error.
- For MCP stdio, send a long-running request and a `notifications/cancelled` notification with the same request ID, then assert the request eventually returns a structured cancelled/timeout response where feasible.

Avoid flaky tests dependent on wall-clock timing. Prefer deterministic pre-cancelled flags or artificially small budgets.

### 6. Add budget override tests

For `ToolRegistry::call_json_with_budget` and `call_json_with_context`, add or verify tests for:

- Oversized serialized input rejected before handler execution.
- Output truncation marks limits consistently.
- Explicit budget overrides default tool budgets.
- Cancellation flag is visible to handlers that create their own `BudgetContext` through thread-local context.

### 7. Improve diagnostics if needed

If diagnostics do not currently report active schema detail or active audience, add those fields to JSON diagnostics and text diagnostics. Keep the output stable and machine-readable.

Recommended JSON additions:

```json
"runtime": {
  "active_profile": "...",
  "active_audience": "...",
  "schema_detail": "...",
  "limits": {
    "max_requests_per_second": 10,
    "max_in_flight_requests": 32,
    "max_tool_workers": 16,
    "max_request_bytes": 1000000,
    "max_output_bytes": 1000000
  }
}
```

If exposing runtime state from CLI diagnostics forces global initialization side effects, document that and keep the change minimal.

## Testing requirements

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If new diagnostics fields alter expected diagnostics tests, update tests and docs together.

## Acceptance criteria

- `EGGCALC_MCP_SCHEMA_DETAIL` cannot silently install an undocumented value.
- Accepted schema detail values are documented and tested.
- Programmatic `set_schema_detail()` remains strict.
- Heavy and composite tools have cancellation/budget checks at meaningful loop boundaries.
- Cancellation tests cover in-process execution and at least one MCP path where feasible.
- Budget override tests cover pre-execution input rejection and output truncation.
- Diagnostics accurately report runtime settings if fields are added.
- Existing CI-equivalent commands pass.

## Review checklist

Before closing the milestone, verify:

- Invalid schema detail env handling does not panic.
- Invalid schema detail env handling does not produce malformed tool lists.
- Tests do not rely on fragile timing assumptions.
- Cancellation errors use existing machine-code conventions.
- Added budget checks do not accidentally change successful small-input behavior.
- No public stable API is added unless it is intentionally documented.

## Handoff notes

This milestone should be implemented before route-critical fixture tightening because exact route behavior is easier to assert once runtime configuration and cancellation semantics are stable. Keep changes conservative: the goal is predictable runtime behavior, not a new scheduler or execution model.
