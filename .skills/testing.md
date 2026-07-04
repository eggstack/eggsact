# Skill: Testing eggsact

Use this when writing, running, or debugging tests.

## Test Commands

```bash
cargo test                           # all tests (unit + integration + parity)
cargo test --lib                     # unit tests in src/ (including agent module)
cargo test --lib agent               # agent module tests only
cargo test --test lib mcp            # MCP tests only
cargo test --test lib parity         # parity tests only
cargo test --test lib text           # text tests only
cargo test --doc                     # doc tests
cargo package                        # release/package verification
```

## Verification Order

Always run in this order (CI mirrors this exactly):
```bash
cargo fmt --all -- --check           # format gate
cargo clippy --all-targets --all-features -- -D warnings  # lint (warnings denied)
cargo test --verbose                 # all tests
cargo run --bin generate-docs -- --check  # generated docs freshness
cargo package --verbose              # crates.io package verification
```

CI runs these checks on push/PR to `main` via `.github/workflows/ci.yml`.

## Test Structure

```
tests/
  lib.rs                     # declares test modules
  calc/
    test_evaluator.rs         # calculator evaluator tests
    test_normalize.rs         # NL normalization tests
    test_units.rs             # unit conversion tests
    test_bug_regression.rs    # regression tests for bugs
  mcp/
    test_protocol.rs          # JSON-RPC protocol tests
    test_mcp_tools.rs         # tool behavior tests
    test_edge_cases.rs        # edge case coverage
    test_response_structure.rs
    test_golden_fixtures.rs
    test_determinism_concurrency.rs
    ... (22 test files)
  parity/
    test_tools_core.rs        # core tool parity with Python
    test_tools_tier0..3.rs    # tier-specific parity
    test_bug_fixes.rs         # regression parity
    test_semantic_parity.rs   # semantic equivalence
  text/
    test_<module>.rs          # one file per text module
```

Agent module unit tests (`src/agent/mod.rs` inline `#[cfg(test)]`) cover `ToolRegistry` profile filtering, unknown tool errors, argument validation, and `call_json` success paths.

## Parity Tests

Parity tests compare Rust MCP tool output against the Python `eggcalc` package.
They require:
- Python 3.x installed
- `eggcalc` package at `../eggcalc` (sibling directory)
- Both binaries built: `cargo build` and Python server at `../eggcalc`

Run parity tests:
```bash
cargo test --test lib parity
```

## Machine Code Enforcement

The test `test_all_tool_responses_have_machine_code` verifies that every non-OK `ToolResponse` includes a `machine_code` field. If you add a new error path, ensure it uses `error_with_code()` or `.with_machine_code()` — the test will catch missing codes.

`test_route_critical_finding_codes_are_enumerated` (in `tests/mcp/test_route_contracts.rs`) verifies that every UPPERCASE_SNAKE finding `code` emitted by a route-critical tool is present in `machine_codes::ALL`. Add new codes to `ALL` and reference them as constants (`machine_codes::FOO`), never as raw strings.

See `architecture/machine-codes.md` for the full list of machine codes.

## Budget / Truncation Testing

Tests that need to exercise truncation or input-overflow behavior can override single budget fields via the `ToolBudget` builders:
- `ToolBudget::with_max_findings(n)` — exercise findings cap (reserves 1 slot for synthetic `OUTPUT_TOO_LARGE` notice).
- `ToolBudget::with_max_output_bytes(n)` — exercise result truncation (oversized result is replaced with summary object preserving `machine_code`/`verdict`/`ok`/caller-`summary`).
- `ToolBudget::with_max_input_bytes(n)` — exercise input pre-check (`INPUT_TOO_LARGE` rejection before handler dispatch).

Existing truncation tests live in `src/mcp/response.rs` (`truncate_*` tests) and in-process tests live in `src/agent/mod.rs` (`call_json_with_budget_*` tests).

## Edge Case Test Coverage

`tests/mcp/test_edge_cases.rs` (168 tests) covers:
- Math: division by zero, overflow, nested parens, factorial big-int, polar, rect
- Units: NaN/Inf rejection, temperature extremes, cross-category
- Text: NFC/NFD/NFKC normalization, casefold, trim, emoji, combining chars
- JSON: deep nesting, special keys, trailing commas
- Shell: backslash escape, unterminated quotes
- And more — see the file for full list

## Golden Fixture Tests

`tests/mcp/test_golden_fixtures.rs` verifies tool outputs match expected JSON.
Update fixtures when tool output intentionally changes:
```bash
UPDATE_GOLDEN=1 cargo test test_golden
```

## Context Isolation Testing

When testing tools that depend on per-evaluation state (PRNG, memory registers, user variables), use the context-aware APIs:

- `evaluate_with_context(expr, ctx)` / `run_with_context(expr, ctx)` for calculator operations
- `call_json_with_execution_context(name, args, ctx)` for full tool dispatch

Verify that:
- Two `EvalContext` instances with the same PRNG seed produce identical results
- `EvalContext::mcp_mode()` disables random/side-effect functions
- Legacy wrappers (`evaluate`, `run`, `call_json`) remain backward-compatible (default context)

## Determinism & Concurrency Tests

`tests/mcp/test_determinism_concurrency.rs` verifies:
- Tool outputs are deterministic across runs
- Concurrent tool calls produce correct results
- No race conditions in shared state
