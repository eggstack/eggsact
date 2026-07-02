# Finish Priority 2: Source-Scanning Guard Test

## Goal

Add a source-level guard test that scans all `src/` files and rejects any call to `error_without_code_for_legacy_tests_only` outside `src/mcp/response.rs` and test code. This enforces the machine-code enforcement policy at the test level, complementing the existing `#[cfg(test)]` compile-time gate.

## Context

- `error_without_code_for_legacy_tests_only` is `#[cfg(test)]`-gated at `src/mcp/response.rs:327`
- Currently only called from a unit test inside `response.rs` itself
- No production call sites exist today, but the guard test prevents future drift
- Established pattern: `include_str!` is used in parity tests, but for whole-directory scanning we need `std::fs::read_dir` or `glob`

## Implementation

### Step 1: Add guard test to `tests/mcp/test_machine_codes.rs`

Add a test that:
1. Walks all `.rs` files under `src/`
2. Reads each file's content
3. Searches for `error_without_code_for_legacy_tests_only(`
4. Tracks which files contain the call
5. Allows `src/mcp/response.rs` (the definition)
6. Allows files in test modules (paths containing `/tests/` or inline `#[cfg(test)]` modules)
7. Fails if any non-allowed file contains the call

Use `std::fs::read_dir` recursively or `glob` to find all `.rs` files under `src/`.

### Step 2: Run verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib
cargo test --test lib mcp
cargo test --test lib text
cargo test --test lib parity
cargo test profile_snapshot
cargo test machine_code
cargo test tool_registry
cargo test preflight
cargo package --verbose
```

## Files Modified

- `tests/mcp/test_machine_codes.rs` — add `source_guard_rejects_legacy_error_in_production` test

## Acceptance Criteria

- Guard test passes when no production code calls the legacy constructor
- Guard test would fail if someone added `error_without_code_for_legacy_tests_only(` to a non-test `src/` file
- All existing tests remain green
- CI gates pass (fmt, clippy, test, package)
