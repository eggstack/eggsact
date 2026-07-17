# Testing

The eggsact test suite spans 70+ test files across 4 integration suites, plus unit tests in `src/` and doc tests. All integration tests compile into a single test crate via `tests/lib.rs`.

See also: [Calculator](calculator.md), [MCP Server](mcp-server.md), [Agent API](agent-api.md), [Preflight](preflight.md)

## Test Structure Overview

```
tests/
  lib.rs                          # single test crate root, declares 4 modules
  test_context_isolation.rs       # context isolation integration tests (819 lines)
  calc/                           # calculator tests (4 files)
  mcp/                            # MCP protocol + tool tests (27 files)
  text/                           # text processing tests (24 files)
  parity/                         # Python/Rust parity tests (11 files)
  fixtures/
    accepted_parity_failures.txt  # 33 known parity failures for regression detection
```

The test crate root (`tests/lib.rs`) declares exactly four modules:

```rust
mod calc;
mod mcp;
mod parity;
mod text;
```

All integration tests run via `cargo test --test lib`. Context isolation tests are a standalone integration test (`tests/test_context_isolation.rs`) and run via the default `cargo test`.

## Calculator Tests (`tests/calc/`)

Four files covering the calculator core (`src/calc/`):

| File | Coverage |
|------|----------|
| `test_normalize.rs` | Natural language tokenization, number words (`one` → `1`), operator words (`plus` → `+`), case folding |
| `test_evaluator.rs` | Expression evaluation, arithmetic, functions (`sin`, `sqrt`), constants (`pi`, `e`), operator precedence, nested expressions |
| `test_units.rs` | Unit conversions (`km → m`), prefixed units, temperature offsets (`°C ↔ °F`), compound units, dimension checking |
| `test_bug_regression.rs` | Regression tests for specific calculator bugs (overflow, parsing edge cases, division by zero) |

Tests call `eggsact::calc::run()` or `eggsact::calc::evaluate()` directly — no MCP overhead.

## MCP Tests (`tests/mcp/`)

27 test files covering the MCP server, protocol, tool execution, and contract enforcement.

### Important Test Files

#### `test_route_contracts.rs` (~1600 lines)

The largest MCP test file. Uses a table-driven `RouteFixture` pattern to verify route-critical tool contracts:

```rust
struct RouteFixture {
    tool: &'static str,
    label: &'static str,
    args: Value,
    expect_ok: bool,
    expect_machine_code: &'static str,
    expect_verdict: Option<&'static str>,
    expect_findings: Vec<ExpectedFinding>,
}
```

Key tests:
- **`all_fixtures()`** — single source of truth for all route-critical contract scenarios. Each fixture specifies expected `machine_code`, `verdict`, and required findings (subset check).
- **Registry invariant tests** — verify `ROUTE_CRITICAL_TOOLS` list is consistent with `is_route_critical()` and that all listed tools exist in the full registry.
- **MCP stdio coverage** — some tests exercise tools through the MCP stdio protocol (spawning the binary) to verify wire-level contracts.
- **Non-empty string contracts** — verifies `machine_code` and `verdict` are non-empty strings, not just present.

#### `test_comprehensive_parity.rs` (~2066 lines)

Thin-coverage tool tests, deterministic output verification, sequential multi-tool sessions, and cross-tool interaction patterns.

Key helper — `mcp_request_multi()`:

```rust
fn mcp_request_multi(requests: &[&str]) -> Vec<Value>
```

- Spawns a single MCP process and writes all requests to stdin.
- Responses may arrive in completion order (concurrent dispatch via `JoinSet`).
- Correlates responses by JSON-RPC `id`, then reorders to match request slice order.
- Panics on duplicate `id`s, missing `id`s, or unexpected `id`s.
- Notifications (no `id`) are excluded from the returned vector.

#### `test_hardening_and_gaps.rs` (~2380 lines)

Security hardening, profile invariants, sanitization, cancellation, schema detail, and production review tests.

Profile snapshot tests:
- **`test_profile_snapshots_all_11_profiles_exist`** — asserts all 11 named profiles are defined.
- **`test_profile_snapshots_full_equals_all_non_hidden`** — Full profile tool list equals all non-hidden tools.
- **`test_profile_snapshots_human_math_only_math_category`** — HumanMath profile contains only math-category tools.

#### `test_schema_boundaries.rs` (136 lines)

Enforces that all registered tool schemas use only supported JSON Schema keywords:

- **Supported**: `type`, `properties`, `required`, `additionalProperties`, `items`, `enum`, `const`, numeric/string/array constraints.
- **Annotation-only** (allowed, not enforced): `description`, `title`, `default`, `examples`, `$schema`.
- **Unsupported** (must never appear): `$ref`, `$defs`, `definitions`, `oneOf`, `anyOf`, `allOf`, `not`, `if/then/else`, `format`, `patternProperties`, etc.

Walks every tool's input schema recursively and collects violations. This prevents schema drift — adding a new keyword to a tool spec that the validator can't handle is caught immediately.

### All MCP Test Files

| File | Coverage |
|------|----------|
| `test_mcp_tools.rs` | Core tool execution, input/output contracts |
| `test_protocol.rs` | JSON-RPC 2.0 protocol handling, initialize, tools/list |
| `test_error_structure.rs` | Error response format, machine codes on errors |
| `test_response_structure.rs` | ToolResponse shape validation |
| `test_route_contracts.rs` | Route-critical tool contracts (verdict + machine_code) |
| `test_machine_codes.rs` | Machine code enumeration completeness |
| `test_composite_tools.rs` | Composite tool orchestration, sub-budget allocation |
| `test_edit_preflight_enhanced.rs` | Enhanced edit preflight scenarios |
| `test_preflight_wrappers.rs` | Typed preflight wrapper contract tests |
| `test_hardening_and_gaps.rs` | Profile snapshots, hardening, gap detection |
| `test_golden_fixtures.rs` | Golden fixture tests for deterministic output |
| `test_real_tool_use.rs` | Real-world tool usage scenarios |
| `test_boundary_conditions.rs` | Edge cases at input limits |
| `test_additional_edge_cases.rs` | Additional edge cases |
| `test_edge_cases.rs` | General edge cases |
| `test_tool_coverage.rs` | Tool coverage verification |
| `test_tool_gaps.rs` | Gap detection across tools |
| `test_determinism_concurrency.rs` | Determinism under concurrent execution |
| `test_deterministic_real_use.rs` | Deterministic behavior in real scenarios |
| `test_cancellation.rs` | Cooperative cancellation |
| `test_diagnostics.rs` | Runtime diagnostics, profile inspect |
| `test_lifecycle_and_gaps.rs` | Tool lifecycle testing |
| `test_runtime_helpers.rs` | Runtime helper functions |
| `test_repo_diff_path_tools.rs` | Repository/diff/path tools |
| `test_analysis_tools.rs` | Import/export, code blocks, symbol diff, lockfile |
| `test_schema_boundaries.rs` | Schema keyword enforcement |
| `test_comprehensive_parity.rs` | Comprehensive parity with Python eggcalc |

## Text Tests (`tests/text/`)

24 test files — one per `src/text/` module. Each tests the public API of its corresponding module.

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

11 test files comparing Rust output against Python `eggcalc`. Requires `../eggcalc` to exist locally.

### Parity Infrastructure

The `tests/parity/mod.rs` file provides shared helpers:

- **`compare_tool_parity(tool, args)`** — spawns both Rust and Python MCP servers, calls the same tool with identical arguments, and asserts byte-identical JSON output.
- **`compare_tool_parity_superset(tool, args)`** — asserts Rust output is a superset of Python output (Rust may emit extra fields).
- **`compare_tool_text_parity(tool, args)`** — compares raw `content[0].text` strings before JSON parsing.
- **`compare_tools_list_parity(schema_detail)`** — compares tool names from `tools/list` between Rust and Python.

Each parity helper spawns a fresh MCP process per call (single-request sessions), avoiding state leakage between tests.

### Parity Test Files

| File | Coverage |
|------|----------|
| `test_tools_tier0.rs` | Tier 0 (essential) tools — math_eval, text_measure, etc. |
| `test_tools_tier1.rs` | Tier 1 (common) tools |
| `test_tools_tier2.rs` | Tier 2 (advanced) tools |
| `test_tools_tier3.rs` | Tier 3 (specialized) tools |
| `test_tools_core.rs` | Core tool functionality |
| `test_tools_list.rs` | `tools/list` ordering, schema detail levels, profile filtering |
| `test_tools_phase4.rs` | Phase 4 tool tests |
| `test_tools_phase5.rs` | Phase 5 tool tests |
| `test_semantic_parity.rs` | Semantic equivalence tests |
| `test_error_handling.rs` | Error handling parity |
| `test_bug_fixes.rs` | Bug fix parity |

### Known Failures

As of 2026-07-08, there are **33 known failures** out of 418 parity tests. These are accepted behavioral differences, not regressions. See `docs/parity.md` for the full breakdown.

The fixture file `tests/fixtures/accepted_parity_failures.txt` lists all 33 test names:

```
# Accepted parity failures (categories C1–C6 from docs/parity.md decision table).
test_shell_split_comment_handling
test_shell_split_quoted_hash
test_prompt_input_inspect_clean
test_unicode_policy_check_identifier_strict
test_edit_preflight_basic
test_tools_list_order_full
test_math_eval_power
...
```

This file is used for regression detection: any parity failure NOT in this list is flagged as an unexpected regression.

## Context Isolation Tests

`tests/test_context_isolation.rs` (819 lines) verifies per-request state isolation across multiple dimensions:

| Test | What It Verifies |
|------|-----------------|
| `test_profile_isolation` | Two registries with different profiles enforce different tool availability in the same process |
| `test_audience_isolation` | Model audience rejects HarnessOnly tools; Harness audience allows them |
| `test_compatibility_mode_isolation` | StrictNative and EggcalcPython modes do not leak between calls |
| `test_cancellation_isolation` | A cancelled context does not poison later uncancelled calls |
| `test_budget_isolation` | Per-call budget overrides do not leak between registries |
| `test_eval_context_isolation` | `call_json_with_execution_context` clones `eval_ctx` — handler mutations do not persist back |
| `test_concurrent_isolation` | Multiple registries on separate threads maintain independent state |
| `test_execution_context_with_custom_budget` | Custom `ToolBudget` overrides default limits per-call |

These tests use the in-process agent API (`ToolRegistry`, `call_json_with_execution_context`, `ExecutionContext`) — no MCP subprocess overhead.

## How to Run Tests

### All Tests

```bash
cargo test                          # unit + integration + doc tests
```

### By Suite

```bash
cargo test --lib                     # unit tests in src/ only
cargo test --test lib                # all integration tests
cargo test --test lib calc           # calculator tests only
cargo test --test lib mcp            # MCP tests only
cargo test --test lib text           # text tests only
cargo test --test lib parity         # parity tests (requires ../eggcalc)
cargo test --doc                     # doc tests only
```

### Filtering

```bash
cargo test --test lib -- test_name           # single test by name
cargo test --test lib -- --list              # list all tests
cargo test --test lib -- --nocapture         # show println! output
cargo test --test lib mcp -- --skip parity   # MCP tests, skip parity
```

### Binary Tests

```bash
cargo test --bins                            # tests for bin/ targets
```

## CI Pipeline

GitHub Actions runs on push/PR to `main` (plus manual `workflow_dispatch`):

| Step | Command | What It Checks |
|------|---------|---------------|
| 1 | `cargo fmt --all -- --check` | Code formatting |
| 2 | `cargo clippy --all-targets --all-features -- -D warnings` | Lint warnings as errors |
| 3 | `cargo test --all-features --lib` | Unit tests in `src/` |
| 4 | `cargo test --all-features --bins` | Binary target tests |
| 5 | `cargo test --all-features --tests -- --skip parity` | Integration tests (parity excluded) |
| 6 | `cargo test --doc` | Doc tests |
| 7 | `cargo run --bin generate-docs -- --check` | Generated docs freshness |
| 8 | `cargo package --verbose` | Crates.io packaging dry run |

Parity tests are excluded from CI because Python `eggcalc` is not available in the CI environment. Run locally with `cargo test --test lib parity`.

CI mirrors the release verification gates but does **not** publish to crates.io. The maintainer publishes manually per `docs/release.md`.

## Adding New Tests

### Multi-Message Session Testing

Tests that need to verify lifecycle behavior or multi-request sessions use
raw process spawning with explicit initialization handshakes. The standard
`mcp_request()` helper in each test file automatically includes the
initialization handshake (initialize + notifications/initialized) before
the actual request, and returns only the last response line.

For tests that need multiple requests in one session, use
`mcp_request_multi()` which also includes the initialization handshake and
correlates responses by JSON-RPC `id`.

### Calculator Tests

Add to `tests/calc/test_evaluator.rs` or `tests/calc/test_normalize.rs`. Call `eggsact::calc::run()` or `eggsact::calc::evaluate()` directly. No MCP subprocess needed.

```rust
#[test]
fn test_new_feature() {
    let result = eggsact::calc::run("expression", None);
    assert!(result.is_ok());
}
```

### MCP Tool Tests

Add to `tests/mcp/test_mcp_tools.rs` or create a new file in `tests/mcp/`. Register new files in `tests/mcp/mod.rs`.

For in-process testing, use the agent API:

```rust
use eggsact::agent::{Profile, ToolRegistry};

let registry = ToolRegistry::with_profile(Profile::Full);
let resp = registry.call_json("tool_name", json!({...}));
assert!(resp.is_ok());
```

For MCP stdio testing, use helpers from `test_comprehensive_parity.rs`:

```rust
fn mcp_request(request: &str) -> String { ... }
fn mcp_request_multi(requests: &[&str]) -> Vec<Value> { ... }
```

For route-critical tools, add fixtures to `all_fixtures()` in `test_route_contracts.rs`.

### Text Module Tests

Add to the corresponding `tests/text/test_<module>.rs` file. Import the public API from `eggsact::text::*`.

### Parity Tests

Add to the appropriate tier file in `tests/parity/`. Use the shared helpers:

```rust
use crate::parity::{compare_tool_parity, compare_tool_parity_superset};

#[test]
fn test_new_tool_parity() {
    let result = compare_tool_parity("tool_name", json!({...}));
    assert!(result.passed, "{}", result.error.unwrap_or_default());
}
```

### Context Isolation Tests

Add to `tests/test_context_isolation.rs`. Use the in-process agent API with `ExecutionContext`:

```rust
let registry = ToolRegistry::with_profile(Profile::Full);
let ctx = ExecutionContext::test_default();
let resp = registry.call_json_with_execution_context("tool", json!({...}), &ctx);
```

## Key Test Patterns

### RouteFixture Table-Driven Tests

`test_route_contracts.rs` defines a `RouteFixture` struct and an `all_fixtures()` function that returns all test cases. The `run_fixture()` function asserts `ok`, `machine_code`, `verdict`, and required findings (subset check) for each fixture. This pattern:

- Keeps all route-critical contract expectations in one place.
- Makes it trivial to add new scenarios — just push a new `RouteFixture`.
- Enables both direct API tests and MCP stdio tests using the same fixture data.

### mcp_request_multi with Id-Based Correlation

`mcp_request_multi()` sends multiple JSON-RPC requests over a single MCP session. Since the server dispatches requests concurrently via `JoinSet`, responses may arrive out of order. The helper:

1. Parses request `id`s up front.
2. Collects all response lines from stdout.
3. Indexes responses by `id` in a `HashMap`.
4. Reorders to match the original request slice.

This ensures positional assertions remain stable regardless of server-side concurrency ordering.

### Profile Snapshot Tests

`test_hardening_and_gaps.rs` includes profile snapshot tests that assert:

- All 11 named profiles exist in the registry.
- Full profile tool list equals all non-hidden tools.
- HumanMath profile contains only math-category tools.

These catch drift when tools are added/removed or profile definitions change.

### Schema Boundary Enforcement

`test_schema_boundaries.rs` walks every registered tool's input schema and asserts no unsupported JSON Schema keywords are present. This prevents adding keywords (like `$ref`, `oneOf`, `format`) that the custom schema validator in `src/mcp/schema_validation.rs` cannot handle.

### Deterministic Concurrency Harness

Tests for `apply_cancellation` and `RequestGuard` use `#[tokio::test]` and
`.await` — they exercise async cancellation paths and RAII drop behavior
deterministically without spawning a full MCP server:

- `apply_cancellation` tests are async (`#[tokio::test]`) and use `.await`
  on the lock, verifying that cancellation notifications are applied correctly
  even under contention. The flag is set **outside** the critical section
  (after the lock is released), tested by `apply_cancellation_sets_flag_outside_lock`.
- `RequestGuard` drop behavior can be tested by creating a guard and dropping
  it, verifying that the active-request entry is cleaned up and that stale
  cancel flags do not remove newer entries with the same ID.
- `register_request` tests verify atomic check+insert under one lock
  acquisition, capacity limits, duplicate rejection, and ID reuse after
  completion.
- `MetricGuard` tests verify RAII increment/decrement on static atomic
  counters, including nested guard scoping.

### Execution Safety Stress Tests

`tests/mcp/test_execution_safety.rs` contains integration tests that exercise
the MCP server's execution safety invariants:

- **Worker containment**: 20 concurrent `math_eval` calls via in-process API
  verify no deadlock under `MAX_TOOL_WORKERS=16` concurrency.
- **Timeout permit leak detection**: a short-budget call followed by a
  normal call verifies permits are released after timeout.
- **Duplicate ID rejection**: integer and string ID duplicates are rejected
  with JSON-RPC errors; ID reuse after completion succeeds.
- **Cancellation targeting**: cancelling one request does not affect another
  concurrent request.
- **Shutdown drain**: closing stdin with in-flight requests verifies all
  responses are drained before exit.
- **Malformed cancellation**: notifications with missing params or
  requestId produce no response and are logged to stderr.

### Running Focused Safety Tests

```bash
# Runtime helper tests (register_request, MetricGuard, apply_cancellation)
cargo test --test lib mcp::test_runtime_helpers

# Execution safety integration tests (worker containment, duplicate IDs, shutdown)
cargo test --test lib mcp::test_execution_safety

# Cancellation propagation (flag plumbing, budget context, tool-specific)
cargo test --test lib mcp::test_cancellation

# Determinism and concurrency (concurrent tool calls, determinism)
cargo test --test lib mcp::test_determinism_concurrency

# Protocol tests (null ID, notification, duplicate ID)
cargo test --test lib mcp::test_protocol

# Full MCP test suite (excluding parity)
cargo test --test lib mcp -- --skip parity
```

### Context Isolation via ExecutionContext

`test_context_isolation.rs` tests that `call_json_with_execution_context()` properly isolates per-request state:

- `eval_ctx` is **cloned** at dispatch — handler mutations do not persist back to the caller's context.
- Budget overrides are per-call and do not leak.
- Cancellation flags are per-context and do not poison other registries.
- Profile and audience enforcement is per-registry, not per-call.
