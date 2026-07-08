# Testing

The eggsact test suite covers calculator, MCP protocol, text processing, Python/Rust parity, and context isolation.

See also: [Calculator](calculator.md), [MCP Server](mcp-server.md)

## Test Structure

```
tests/
  lib.rs                    # declares test modules
  test_context_isolation.rs # context isolation integration test
  calc/                     # calculator tests (4 files)
  mcp/                      # MCP protocol + tool tests (27 files)
  parity/                   # Python/Rust parity tests (12 files)
  text/                     # text processing tests (25 files)
  fixtures/                 # test data files
```

### `tests/lib.rs`

Single test crate root that declares modules:

```rust
mod calc;
mod mcp;
mod parity;
mod text;
```

All tests run via `cargo test --test lib`.

## Calculator Tests (`tests/calc/`)

| File | What It Covers |
|------|---------------|
| `test_normalize.rs` | Natural language tokenization, number words, operator words |
| `test_evaluator.rs` | Expression evaluation, functions, constants, edge cases |
| `test_units.rs` | Unit conversions, prefixed units, temperature offsets |
| `test_bug_regression.rs` | Regression tests for specific calculator bugs |

## MCP Tests (`tests/mcp/`)

| File | What It Covers |
|------|---------------|
| `test_mcp_tools.rs` | Core tool execution, input/output contracts |
| `test_protocol.rs` | JSON-RPC 2.0 protocol handling |
| `test_error_structure.rs` | Error response format and machine codes |
| `test_response_structure.rs` | ToolResponse shape validation |
| `test_route_contracts.rs` | Route-critical tool contracts (verdict + machine_code) |
| `test_machine_codes.rs` | Machine code enumeration completeness |
| `test_composite_tools.rs` | Composite tool orchestration |
| `test_edit_preflight_enhanced.rs` | Enhanced edit preflight scenarios |
| `test_preflight_wrappers.rs` | Typed preflight wrapper contract tests |
| `test_hardening_and_gaps.rs` | Profile snapshots, hardening, gap detection |
| `test_golden_fixtures.rs` | Golden fixture tests for deterministic output |
| `test_real_tool_use.rs` | Real-world tool usage scenarios |
| `test_boundary_conditions.rs` | Edge cases at limits |
| `test_additional_edge_cases.rs` | More edge cases |
| `test_edge_cases.rs` | General edge cases |
| `test_tool_coverage.rs` | Tool coverage verification |
| `test_tool_gaps.rs` | Gap detection across tools |
| `test_determinism_concurrency.rs` | Determinism under concurrent execution |
| `test_deterministic_real_use.rs` | Deterministic behavior in real scenarios |
| `test_cancellation.rs` | Cooperative cancellation |
| `test_diagnostics.rs` | Runtime diagnostics |
| `test_lifecycle_and_gaps.rs` | Tool lifecycle testing |
| `test_runtime_helpers.rs` | Runtime helper functions |
| `test_repo_diff_path_tools.rs` | Repository/diff/path tools |
| `test_analysis_tools.rs` | Analysis tools (import/export, code blocks, symbol diff, lockfile) |
| `test_schema_boundaries.rs` | Schema boundary enforcement (all tool schemas use only supported keywords) |
| `test_comprehensive_parity.rs` | Comprehensive parity with Python eggcalc |

## Text Tests (`tests/text/`)

One test file per text module (25 files). Each tests the public API of its corresponding `src/text/` module:

| File | Module Tested |
|------|--------------|
| `test_primitives.rs` | UTF-8 encoding, grapheme counting |
| `test_confusables.rs` | Unicode confusable detection |
| `test_diff.rs` | String diffing, Levenshtein distance |
| `test_measure.rs` | Text metrics (words, lines, bytes) |
| `test_validate.rs` | Bracket, JSON, regex validation |
| `test_transform.rs` | Text transforms, hashing, fingerprinting |
| `test_position.rs` | Byte/line/column position conversion |
| `test_regex_safety.rs` | ReDoS detection |
| `test_replace.rs` | Text replacement with preview |
| `test_path.rs` | Path analysis and normalization |
| `test_identifier.rs` | Identifier naming classification |
| `test_shell.rs` | Shell command parsing and quoting |
| `test_markdown.rs` | Markdown structure analysis |
| `test_glob.rs` | Glob pattern matching |
| `test_config.rs` | .env and INI validation |
| `test_toml.rs` | TOML validation and shape analysis |
| `test_patch.rs` | Unified diff parsing and application |
| `test_line_range.rs` | Line range extraction and comparison |
| `test_unicode_policy.rs` | Unicode safety policies |
| `test_unicode_tools.rs` | Mixed-script, invisible char detection |
| `test_inspect_prompt.rs` | Prompt injection detection |
| `test_cargo.rs` | Cargo.toml inspection |
| `test_version.rs` | Semver constraint checking |
| `test_bug_regression.rs` | Regression tests for text bugs |

## Parity Tests (`tests/parity/`)

Compare Rust output against Python `eggcalc` (requires `../eggcalc` to exist). These spawn both MCP servers and compare JSON output strictly.

| File | What It Covers |
|------|---------------|
| `test_tools_tier0.rs` | Tier 0 (essential) tools |
| `test_tools_tier1.rs` | Tier 1 (common) tools |
| `test_tools_tier2.rs` | Tier 2 (advanced) tools |
| `test_tools_tier3.rs` | Tier 3 (specialized) tools |
| `test_tools_core.rs` | Core tool functionality |
| `test_tools_list.rs` | Tool listing and filtering |
| `test_tools_phase4.rs` | Phase 4 tool tests |
| `test_tools_phase5.rs` | Phase 5 tool tests |
| `test_semantic_parity.rs` | Semantic equivalence tests |
| `test_error_handling.rs` | Error handling parity |
| `test_bug_fixes.rs` | Bug fix parity |

As of 2026-07-08, there are 33 known failures out of 418 parity tests. See `docs/parity.md` for the breakdown.

An accepted-failures fixture at `tests/fixtures/accepted_parity_failures.txt` lists all 33 names for regression detection.

## How to Run Tests

```bash
# All tests
cargo test

# Unit tests only (src/)
cargo test --lib

# Calculator tests
cargo test --test lib calc

# MCP tests only
cargo test --test lib mcp

# Text tests only
cargo test --test lib text

# Parity tests (requires ../eggcalc)
cargo test --test lib parity

# Doc tests
cargo test --doc

# Single test by name
cargo test --test lib -- test_name

# With output
cargo test --test lib -- --nocapture
```

## CI Pipeline

GitHub Actions runs on push/PR to `main`:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-features --lib` (unit tests)
4. `cargo test --all-features --bins` (binary tests)
5. `cargo test --all-features --tests -- --skip parity` (integration, parity excluded)
6. `cargo test --doc` (doc tests)
7. `cargo run --bin generate-docs -- --check` (generated docs freshness)
8. `cargo package --verbose`

Parity tests are excluded from CI because Python `eggcalc` is not available in CI. Run locally with `cargo test --test lib parity`.

## Adding New Tests

### Calculator Tests

Add to `tests/calc/test_evaluator.rs` or `tests/calc/test_normalize.rs`. Use `eggsact::calc::run()` or `eggsact::calc::evaluate()` directly.

### MCP Tool Tests

Add to `tests/mcp/test_mcp_tools.rs` or create a new file in `tests/mcp/`. Use the `mcp_request()` or `mcp_request_multi()` helpers from `tests/mcp/test_comprehensive_parity.rs`.

### Text Module Tests

Add to the corresponding `tests/text/test_<module>.rs` file. Each test file imports the public API from `eggsact::text::*`.

### Parity Tests

Add to the appropriate tier file in `tests/parity/`. The test must spawn both the Rust and Python MCP servers and compare JSON output.
