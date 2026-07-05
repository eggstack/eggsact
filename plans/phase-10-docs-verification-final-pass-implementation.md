# Phase 10: Final Documentation and Verification Pass — Implementation Note

**Date:** 2026-07-05
**Commit:** (this commit)

## Summary

Completed all six workstreams from the phase-10 final pass plan. Documentation now accurately distinguishes legacy global evaluator APIs from context-aware evaluator APIs, states that immutable `ExecutionContext` eval state is per-call cloned seed/template state, identifies `math_eval` as the current eval-context-aware tool, and does not imply MCP wire-protocol request-level contexts.

## Verification Results

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all-features` | 384 unit + 7 doc + 15 integration passed; 126 parity/MCP failures (pre-existing) |
| `cargo run --bin generate-docs -- --check` | PASS (generated docs fresh) |

### Test Failure Breakdown

The 126 test failures are all pre-existing and unrelated to this documentation pass:

- **Parity tests (~53 known):** Tool drift between Python `eggcalc` and Rust (tool count mismatch 64 vs 68, `shell_split` output differences, `unicode_policy_check` differences, `prompt_input_inspect` differences, `constant_lookup`/`unit_info` output differences).
- **MCP integration tests (~73):** Tests asserting `Some(Bool(true))` for tool outputs where the tool returns `None` — these are pre-existing test/tool drift issues.

None of these failures were introduced by this documentation pass. The new regression test `execution_context_eval_ctx_is_per_call_seed_for_immutable_dispatch` passed.

## Changes Made

### Rustdoc Updates

- **`src/calc/context.rs`**: Updated `EvalContext` Rustdoc to distinguish legacy vs context-aware APIs, document per-call seed/template behavior.
- **`src/agent/mod.rs`**: Updated `ExecutionContext`, `with_eval_context`, and `call_json_with_execution_context` Rustdoc with dispatch controls, eval-context bridge, clone/no-persist semantics, and MCP wire boundary.

### Architecture & Documentation Updates

- **`architecture/overview.md`**: Added context-isolation model (legacy vs context-aware paths, per-call seed/template, cooperative cancellation, in-process vs MCP wire).
- **`architecture/calculator.md`**: Added context-aware vs legacy API table, in-process tool dispatch section.
- **`architecture/mcp-server.md`**: Expanded eval-context semantics, added MCP wire protocol boundary paragraph.
- **`AGENTS.md`**: Added agent guidance on which context API to use, MCP wire vs in-process note.
- **`.skills/mcp-tools.md`**: Added context-aware APIs section with key invariant and calculator guidance.
- **`.skills/testing.md`**: Added context isolation testing section.

### Test Addition

- **`tests/test_context_isolation.rs`**: Added `execution_context_eval_ctx_is_per_call_seed_for_immutable_dispatch` regression test verifying that `call_json_with_execution_context` clones eval_ctx per call (same seed → same first random value), while `evaluate_with_context` advances PRNG persistently.

## Parity Availability

Parity tests require the Python `eggcalc` package at `../eggcalc`. Not verified in this pass — parity failures are pre-existing known gaps.
