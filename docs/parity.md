# Python/Rust Parity

## Overview

eggsact is a Rust reimplementation of the Python `eggcalc` MCP server. It is a
drop-in replacement: same tool names, same JSON-RPC protocol, same input schemas, same
output structures. Clients do not need to change any code to switch from Python to
Rust.

The Python reference lives in `eggcalc/mcp/` (schemas.py, tools.py, server.py) and
provides 64 tool definitions. The Rust implementation in `src/mcp/` replicates all 64
tools with matching behavior. All parity tests pass.

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

- **Tool set**: All 64 tool names, descriptions, and input schemas match
- **Tool metadata**: Tier, category, profiles, tags, cost, stability, composite flags are identical
- **Output schemas**: Per-property descriptions match Python's `TOOL_SCHEMAS`
- **`tools/list` response**: Serialized fields match Python (`category`, `cost`, `deprecated`, `description`, `inputSchema`, `llm_exposure`, `name`, `outputSchema`, `tags`, `tier`)
- **Error codes**: Same JSON-RPC error codes (-32600, -32601, -32602, -32603, -32700, -32000)
- **Error sanitization**: Same regex-based PII/path redaction
- **Rate limiting**: 10 req/s sliding window
- **Timeout handling**: 30s tool timeout
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
