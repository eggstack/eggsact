# Skill: Testing eggsact

Use this when writing, running, or debugging tests.

## Test Commands

```bash
cargo test                           # all tests (unit + integration + parity)
cargo test --lib                     # unit tests in src/ only
cargo test --test lib mcp            # MCP tests only
cargo test --test lib parity         # parity tests only
cargo test --test lib text           # text tests only
cargo test --doc                     # doc tests
```

## Verification Order

Always run in this order:
```bash
cargo fmt --check                    # format gate
cargo clippy --all-targets --all-features  # lint
cargo test                           # all tests
```

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
    ... (14 test files)
  parity/
    test_tools_core.rs        # core tool parity with Python
    test_tools_tier0..3.rs    # tier-specific parity
    test_bug_fixes.rs         # regression parity
    test_semantic_parity.rs   # semantic equivalence
  text/
    test_<module>.rs          # one file per text module
```

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

## Determinism & Concurrency Tests

`tests/mcp/test_determinism_concurrency.rs` verifies:
- Tool outputs are deterministic across runs
- Concurrent tool calls produce correct results
- No race conditions in shared state
