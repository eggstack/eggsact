# Final closure plan: parity correlation, CI gates, and public API compatibility

## Purpose

This plan covers the remaining closure work after the corrective verification pass. The repo is materially improved: the `PatchApplyCheck` wrapper/schema/audience issue was fixed, concurrent MCP runtime documentation was corrected, runtime helper tests were added, and the new repo/diff/path tools now have broader test coverage. The remaining work should stay narrow and release-oriented.

This pass has three goals:

1. Fix or adapt the MCP multi-request parity harness so concurrent JSON-RPC responses are correlated by `id` rather than list position.
2. Run and document the full verification gate matrix.
3. Audit and resolve public API compatibility concerns introduced by `ToolBudget.max_text_chars` -> `max_text_bytes` and related `SubBudget` renames.

Do not add new tools or start the phase 06 dependency/lockfile expansion in this pass.

## Current known state

The corrective verification pass documented current parity status as:

- `357 passed`
- `56 failed`
- `2 ignored`
- `413` parity tests considered, with many non-parity tests filtered out

The new delta includes three concurrent-ordering failures in `tests/mcp/test_comprehensive_parity.rs`:

- `test_sequential_session_multiple_tools`
- `test_sequential_session_same_tool_repeatedly`
- `test_sequential_session_tool_then_error_then_tool`

Root cause: phase 04 changed the MCP runtime to dispatch requests concurrently using `JoinSet` and serialize responses through an `mpsc` writer. JSON-RPC clients must correlate responses by `id`. The current parity helper appears to correlate multi-response sessions by response order, which is no longer a valid assumption under concurrent handling.

## Non-goals

- Do not revert the concurrent runtime unless a concrete protocol violation is found.
- Do not force ordered responses just to satisfy a positional test helper unless the project intentionally wants stronger-than-JSON-RPC ordering semantics.
- Do not broaden parity scope to make Rust-superset tools match Python tools that do not exist.
- Do not add new public APIs unrelated to compatibility repair.
- Do not change machine-code strings or verdict strings unless fixing a concrete test-proven bug.

## Task 1: fix MCP multi-request parity correlation

### Problem

The concurrent MCP server may return responses in completion order rather than request submission order. This is expected for concurrent JSON-RPC request handling as long as each response includes the correct `id`. A test helper that correlates responses positionally is incorrect under the new runtime.

### Implementation steps

1. Inspect `tests/mcp/test_comprehensive_parity.rs` and any shared MCP parity helper functions, especially a helper named or behaving like `mcp_request_multi()`.
2. Identify all places where a `Vec<Response>` is compared positionally against a `Vec<Request>` or expected output list.
3. Replace positional correlation with `id`-based correlation:
   - parse each response as JSON;
   - extract `id`;
   - build `HashMap<IdKey, Response>`;
   - for each request with an `id`, look up the matching response;
   - preserve notification semantics: requests without IDs should not expect responses;
   - detect duplicate response IDs as a test failure;
   - detect missing response IDs as a test failure;
   - detect unexpected extra response IDs as a test failure.
4. Support both string and integer JSON-RPC IDs. Do not treat bool IDs as valid if the server rejects them elsewhere.
5. Keep a helper to return ordered results by request order after `id` correlation, so existing assertions need minimal changes.
6. Add a regression test specifically demonstrating out-of-order response input still maps to the correct request outputs.

### Acceptance criteria

- The three concurrent-ordering parity failures no longer fail because of positional correlation.
- Tests still catch real mismatches in response payloads, error codes, and response IDs.
- Notification-only requests do not create false missing-response failures.
- Duplicate or missing response IDs are hard test failures.
- The helper remains readable and reusable for future MCP parity tests.

## Task 2: decide whether response ordering needs an explicit compatibility mode

### Problem

JSON-RPC clients should correlate by ID, but some simple stdio clients may implicitly assume ordered responses. The repo should be explicit about its contract.

### Steps

1. Review `architecture/mcp-server.md` and README MCP docs.
2. Confirm docs state that concurrent request handling may return responses out of request order and clients must use `id` for correlation.
3. Decide if an optional ordered-response compatibility mode is needed.

Recommended decision: do not add ordered mode now. Keep the runtime concurrent and standards-aligned. Add an ordered mode only if a real downstream client requires it.

If an ordered mode is added later, it should be explicitly configured and tested; do not silently make concurrent dispatch head-of-line blocked again.

### Acceptance criteria

- Docs clearly state the concurrent response-order contract.
- No code changes are made unless a concrete client compatibility requirement exists.

## Task 3: run focused parity after helper fix

### Steps

Run the failing tests directly first:

```bash
cargo test --test lib test_sequential_session_multiple_tools -- --nocapture
cargo test --test lib test_sequential_session_same_tool_repeatedly -- --nocapture
cargo test --test lib test_sequential_session_tool_then_error_then_tool -- --nocapture
```

Then run the parity suite if Python `eggcalc` is available:

```bash
cargo test --test lib parity
```

### Acceptance criteria

- The three concurrent-ordering failures are fixed or reclassified with a precise reason.
- `docs/parity.md` is updated with new pass/fail counts after the run.
- If parity cannot be run because Python `eggcalc` is unavailable, document that explicitly and do not claim parity status changed.

## Task 4: run full non-parity verification gates

Run the full release gate matrix:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If `cargo test --all-features` includes parity tests that require local Python state, split the command into always-available tests and parity tests, but document the split precisely.

### Acceptance criteria

- Every command either passes or has a documented, reproducible, pre-existing reason for failure.
- No generated docs drift remains.
- No clippy warnings remain under `-D warnings`.
- Package build succeeds.

## Task 5: audit public API compatibility for budget field renames

### Problem

Phase 02 renamed `ToolBudget.max_text_chars` to `max_text_bytes`, and similarly adjusted `SubBudget`. This is semantically correct because enforcement uses `str::len()` bytes, but it may be a public API break if downstream code constructs these structs directly.

### Steps

1. Inspect visibility of the affected types and fields:
   - `ToolBudget`
   - `SubBudget`
   - `BudgetContext::check_text_bytes`
   - any removed `check_text_len` helper
2. Search docs/examples/tests for old names:

```bash
rg "max_text_chars|check_text_len|max text chars|max_text" .
```

3. Decide the compatibility strategy.

Preferred approach if public break risk is unacceptable:

- Keep canonical field `max_text_bytes`.
- Reintroduce a deprecated method-level compatibility shim where feasible:

```rust
#[deprecated(note = "use check_text_bytes")]
pub fn check_text_len(...) -> Result<(), ToolResponse> {
    self.check_text_bytes(...)
}
```

- For struct fields, direct aliasing is not possible in Rust without duplicating fields or changing constructors. If `ToolBudget` fields are public and direct struct literals are an intended API, document the break and consider a version bump. If direct struct literals are not intended, add builder constructors and update docs to discourage direct struct literals.

4. Add builder methods if missing:

- `ToolBudget::with_max_text_bytes(n)`
- `SubBudget` construction helper if appropriate

5. Update crate-level docs and `docs/library-api.md` to show builder-style customization rather than full struct literals.

### Acceptance criteria

- The public API impact is explicitly understood.
- Either compatibility shims are added or the changelog states the breaking impact and versioning decision.
- Docs use `max_text_bytes` consistently.
- Old method name, if retained as deprecated shim, has tests.

## Task 6: verify new tests are wired into the test module tree

### Problem

The corrective pass added:

- `tests/mcp/test_preflight_wrappers.rs`
- `tests/mcp/test_repo_diff_path_tools.rs`
- `tests/mcp/test_runtime_helpers.rs`

These files must be included by `tests/mcp/mod.rs` and run under the normal test commands.

### Steps

1. Inspect `tests/mcp/mod.rs` for module declarations.
2. Run targeted tests:

```bash
cargo test --all-features test_preflight_wrappers
cargo test --all-features test_repo_diff_path_tools
cargo test --all-features test_runtime_helpers
```

If module names differ, use the actual test function/module names.

3. Confirm tests are not accidentally hidden behind missing feature flags.

### Acceptance criteria

All newly added test files are compiled and executed in the standard all-features test run.

## Task 7: close docs/changelog after verification

### Steps

1. Update `CHANGELOG.md` under the corrective verification section with final results:
   - wrapper bug fixed;
   - runtime helper tests added;
   - repo/diff/path tool tests added;
   - parity helper fixed or deferred;
   - verification command outcomes.
2. Update `docs/parity.md` with the latest parity counts and root-cause classifications.
3. Ensure no stale wording remains:

```bash
rg "53 known failures|MAX_CANCELLED_REQUESTS|cancellation set|serial at the read-loop|MAX_TOOL_TIMEOUT_SECONDS|max_text_chars|check_text_len" .
```

Some matches may be legitimate if they are compatibility notes or deprecated APIs; every remaining match should be intentional.

### Acceptance criteria

Docs describe the actual runtime, actual parity state, and actual public API compatibility story.

## Suggested commit structure

1. `test(parity): correlate concurrent MCP responses by JSON-RPC id`
2. `docs(mcp): clarify concurrent response ordering contract`
3. `chore: run verification gates and update parity status`
4. `fix(api): add budget compatibility shims or document field rename`
5. `docs: close corrective verification notes`

## Done criteria

This closure pass is done when:

- the three concurrent-ordering parity failures are fixed or explicitly deferred with a standards-based rationale;
- full non-parity gates pass locally;
- generated docs are current;
- package build succeeds;
- public API impact of budget field renames is resolved or versioned;
- the changelog and parity docs reflect the final verified state;
- no new feature work was introduced.
