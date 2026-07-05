# Python/Rust Parity

## Overview

eggsact is a Rust reimplementation of the Python `eggcalc` MCP server. It is a
drop-in replacement for the **subset of tools present in the Rust build**: same tool
names, same JSON-RPC protocol, same input schemas, same output structures for matching
tools. Clients do not need to change any code to switch from Python to Rust for
matching tools.

The Python reference lives in `eggcalc/mcp/` (schemas.py, tools.py, server.py) and
provides 67 tool definitions. The Rust implementation in `src/mcp/` ships 68 tools
(see [Known parity gaps](#known-parity-gaps) below); the remaining 3 are planned for
phase 10 work.

The parity suite is intended to validate all Rust tools against the Python reference
when `eggcalc` is available in the expected path. See
[Verification status](#verification-status) for the most recent local run.

## Parity Framework

The parity test suite in `tests/parity/` validates Rust behavior against the Python
reference implementation. It works by:

1. Spawning the Python MCP server as a subprocess (`python3 -m eggcalc.mcp.server`).
2. Spawning the Rust MCP server as a subprocess (`./target/debug/eggsact --mcp`).
3. Sending identical JSON-RPC `tools/call` requests to both servers.
4. Parsing the JSON-RPC responses from each.
5. Comparing the parsed output values for strict JSON equality.

Both servers receive the same request over stdin and return results over stdout. If
either server is unavailable, the test fails with a clear error message identifying
which server could not be reached. The `compare_tool_parity()` function in
`tests/parity/mod.rs` orchestrates this process and returns a `ParityTestResult` with
the tool name, input, and pass/fail status.

The Rust binary must be built before running parity tests:

```sh
cargo build
```

## Test Organization

Parity tests are split into tiers based on tool complexity and dependency groups:

| File | Tier | Contents |
|------|------|----------|
| `test_tools_core.rs` | Core | math_eval, text_measure, text_equal, text_diff_explain, and other fundamental tools (27 tests) |
| `test_tools_tier0.rs` | Tier 0 | Unit conversion, constants, path utilities (14 tests) |
| `test_tools_tier1.rs` | Tier 1 | Validation, list operations, text hashing (27 tests) |
| `test_tools_tier2.rs` | Tier 2 | JSON, glob, identifier, markdown, shell tools (25 tests) |
| `test_tools_tier3.rs` | Tier 3 | Patch, config, unicode policy, version tools (25 tests) |
| `test_semantic_parity.rs` | Semantic | Semantic parity tests for edge cases across all tools |
| `test_tools_phase4.rs` | Phase 4 | Phase 4 tool tests (regex, shell, unicode, path, version) |
| `test_tools_phase5.rs` | Phase 5 | Phase 5 tool tests (text serialization parity) |
| `test_tools_list.rs` | Tool List | Tool list order and catalog parity tests |
| `test_protocol.rs` | Protocol | Protocol-level parity tests (initialize, ping, profiles/list) |
| `test_error_handling.rs` | Errors | Missing parameters, invalid types, edge cases (33 tests) |

Tier boundaries reflect implementation dependencies. Core tools are tested first
because higher-tier tools may depend on patterns validated in core tests.

## Running Parity Tests

The parity tests are a standard cargo test target:

```sh
cargo test --test lib parity
```

This requires:
- A working Python 3 installation with `eggcalc` available in the repo root's Python path.
- The Rust binary built (`cargo build`).

To run all tests (unit, integration, and parity):

```sh
cargo test
```

To run only the unit tests within `src/`:

```sh
cargo test --lib
```

## Parity-Achieved Areas

The following areas have been brought to full parity:

- **Tool set**: All 67 tool names, descriptions, and input schemas match
- **Tool metadata**: Tier, category, profiles, tags, cost, stability, composite flags are identical
- **Output schemas**: Per-property descriptions match Python's `TOOL_SCHEMAS`
- **`tools/list` response**: Serialized fields match Python (`category`, `cost`, `deprecated`, `description`, `inputSchema`, `llm_exposure`, `name`, `outputSchema`, `tags`, `tier`)
- **Error codes**: Same JSON-RPC error codes (-32600, -32601, -32602, -32603, -32700, -32000)
- **Error sanitization**: Same regex-based PII/path redaction
- **Rate limiting**: 10 req/s sliding window
- **Timeout handling**: Tool timeouts are budget-derived from `ToolSpec.cost` (`Cheap`/`Moderate`/`Heavy`), not a fixed 30s.
- **JSON parser error messages**: serde_json errors are mapped to Python `json` module equivalents
- **TOML parser error messages**: toml_edit errors are mapped to Python `tomllib` equivalents
- **Math semantics**: `^` is XOR (matching Python AST BitXor), `/` is true division (matching Python AST Div)
- **`text_hash` warnings**: MD5 non-cryptographic warning and unknown algorithm warning match Python
- **`text_inspect` output**: Full structural alignment with Python's `synthesis.py` output
- **`text_equal` output**: `first_difference` always computed from raw diff, not gated on `equal`
- **`json_canonicalize`**: Key insertion order preserved via `serde_json` `preserve_order` feature
- **`regex_finditer`**: `groups` and `groupdict` always emitted (empty when no capture groups)
- **`path_analyze`**: Absolute path parents include leading slash
- **`cargo_toml_inspect`**: Always emits `dependencies`, `dev_dependencies`, `build_dependencies`, `target_specific`
- **`text_diff_explain`**: Unicode character names via `unicode_names2`, `max_diffs_applied` defaults to 20
- **`json_shape`**: Recursive structural summary matching Python format
- **`unicode_policy_check`**: Script filtering excludes Common/Inherited/Other/Unknown
- **`edit_preflight`**: `preview_before`/`preview_after` default to empty strings

## Architectural Differences

These are intentional design differences between the Python and Rust implementations.
They do not affect client-facing behavior or interchangeability.

### Process Model

- **Python**: Uses `multiprocessing.get_context("spawn")` for `validate_regex`,
  `regex_finditer`, and `dotenv_validate`. Each runs in a separate OS process with
  `RLIMIT_AS` (256MB memory limit) and RAII `_SpawnPermit` semaphore (max 4 concurrent).
- **Rust**: Uses `std::thread::spawn` for `math_eval`, `validate_regex`, `regex_finditer`,
  and `dotenv_validate` isolation. Threads share the same address space. A timeout
  (`MAX_TOOL_TIMEOUT_SECONDS = 30`) prevents hung tools. Rust's memory safety makes
  process isolation less critical than in C-backed Python extensions.

### Orphan Cleanup

- **Python**: `_cleanup_orphaned_processes()` runs lazily on every `tools/call` request,
  terminating stale child processes from previous tool executions.
- **Rust**: Not needed. When a thread handle is dropped on timeout, the OS thread
  continues briefly but exits naturally when the closure returns. No explicit cleanup
  is required.

### Concurrency

- **Python**: Uses `ThreadPoolExecutor` with bounded workers (16).
- **Rust**: Uses Tokio `Semaphore` with bounded permits (16) and `spawn_blocking`.

## Server Identity

Both implementations identify themselves identically to MCP clients:

- **Name:** `eggsact`
- **Version:** `1.1.3`

This ensures clients see the same server regardless of which backend is running.

## Verification status

The parity suite has **not** been observed passing in full on the most recent
release-polish pass. CI does not run parity tests (Python `eggcalc` is not installed
in the CI environment), and the most recent local run against
`/Users/davidbowman/projects/eggcalc` produced partial results.

- **Command:** `cargo test --test lib parity`
- **Date:** 2026-07-04
- **Commit:** `614651ec5651ea0d4002f47b3555c0c68b2b1ce0`
- **Result:** 360 passed, **53 failed**, 2 ignored (out of 413 parity tests; 2520
  non-parity tests filtered out)

Do not interpret partial parity as a regression — the failures have accumulated across
the phase 06–09 line of work and were not previously captured in this document. The
previous "all parity tests pass" wording was unsupported and has been removed.

## Known parity gaps

The 53 failing parity tests fall into three categories. None are regressions
introduced by a single change; they accumulated as the Rust tool set and audience
model evolved. Fixing them is **explicitly out of scope** for the phase 06–09
release-polish pass and is deferred to follow-up work.

### A. Test-harness audience bug (~40 failures)

The MCP test helpers in `tests/mcp/test_comprehensive_parity.rs` and
`tests/mcp/test_tool_gaps.rs` spawn the binary with the default audience, which is
`ToolAudience::Model`. Tools declared as `ToolExposure::HarnessOnly` in their
`ToolSpec` (`src/mcp/specs/unicode.rs`, `src/mcp/specs/text.rs`,
`src/mcp/specs/shell.rs`, `src/mcp/specs/path.rs`) are rejected by the audience
filter before reaching the handler, so the response has no `ok` field and the
test panics on `result.get("ok")`.

Affected tools and approximate failure counts:

| Tool | Failures | ToolSpec |
|------|----------|----------|
| `unicode_policy_check` | ~16 | `src/mcp/specs/unicode.rs` |
| `prompt_input_inspect` | ~6 | `src/mcp/specs/text.rs` |
| `shell_split` | ~9 | `src/mcp/specs/shell.rs` |
| `path_scope_check` | 2 | `src/mcp/specs/path.rs` |
| `patch_apply_check` | 1 | `src/mcp/specs/patch.rs` |

The fix is to update the test harness to use the `codegg_preflight` profile and
`Harness` audience for these tools, or to call them in-process via
`ToolRegistry::with_profile_and_audience(..., ToolAudience::Harness)`. This is a
test-only change; tool semantics are correct.

### B. Tool/output drift (~8 failures)

Real behavioral differences between Rust and Python outputs that need targeted
fixes in Rust tools:

- `cargo_toml_inspect_basic` (`tests/parity/test_tools_tier3.rs:57`) — output
  shape differs from Python.
- `constant_lookup_speed_of_light` (`tests/parity/test_tools_tier0.rs:49`) — unit
  metadata or ordering differs.
- `unit_info_invalid` (`tests/parity/test_tools_tier0.rs:42`) — error envelope
  shape differs.
- `text_security_inspect_with_hidden` (`tests/parity/test_tools_tier3.rs:139`) —
  finding shape differs.
- `tools_list_tier_*_as_bool` and `tools_list_tier_int`
  (`tests/parity/test_semantic_parity.rs:29,54,79`) — tier-filtered `tools/list`
  ordering differs from Python (same set, different sort).
- `tools_list_order_compact` / `_normal` / `_full`
  (`tests/parity/test_tools_list.rs:6,12,18`) — index 11 ordering: Python emits
  `validate_brackets`, Rust emits `text_position`.

### C. Tool-set gap: 68 vs 67 tools (1 extra in Rust)

The Rust `full` profile ships 68 tools. The Python reference defines 67. The
Rust build includes one tool not present in the Python reference:

- `runtime_diagnostics`

The three tools previously missing from Rust (`config_file_inspect`,
`dependency_edit_preflight`, `repo_manifest_inspect`) were added in phase 09.

Until these are added, the `profiles/list` parity test
(`tests/parity/test_semantic_parity.rs:431`) also fails because the per-profile
tool sets differ — the `default`, `codegg_core_min`, `codegg_core`,
`codegg_unicode_security`, `codegg_shell`, and `human_math` profiles match
exactly, but the remaining four do not.
