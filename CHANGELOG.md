# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **ToolAudience enum** in `src/agent/` with `Model`, `Harness`, `Debug` variants.
  `ToolRegistry` gains `with_profile_and_audience()`, `available_tools_for_audience()`,
  and `available_tools_model_safe()` for audience-aware tool listings.
- **Profile snapshot tests** for all 11 named profiles verifying tool counts
  and composition (`tests/mcp/test_hardening_and_gaps.rs`).
- **Strict profile parsing**: `Profile::from_str_opt` returns `None` for unknown
  names; `Profile::custom(name)` constructs explicit custom profiles.
- **Deprecated `ToolResponse::error`**: renamed to
  `error_without_code_for_legacy_tests_only` (hidden). All new code must use
  `error_with_code()`.
- **Concurrency Model docs**: documented serial stdio read-loop semantics and
  `MAX_TOOL_WORKERS` scope in `architecture/mcp-server.md` and `architecture/overview.md`.

### Changed
- **Phase 3: Stable Response Contracts and Machine Codes**. Every non-OK tool
  response now carries a machine-readable `machine_code` for programmatic
  routing.
  - New `src/mcp/machine_codes.rs` module: single source of truth for all
    57 machine code constants (UPPER_SNAKE_CASE, parity-compatible with
    Python `eggcalc`).
  - New `ToolResponse::error_with_code()` constructor that requires a
    machine code, ensuring error responses are always machine-routable.
  - New `ToolResponse::success_with_machine_code()` convenience.
  - New finding helpers in `src/mcp/response.rs`: `finding()`,
    `finding_with_location()`, `prompt_finding()` for structured findings
    with `code`, `severity`, `message`, and `location`/`span`.
  - New `severity`, `disposition`, and `verdict` constant modules for
    finding metadata.
  - All tool files migrated from string-literal machine codes to
    `machine_codes::*` constants (zero scattered string literals remain).
  - MCP server-level errors (cancelled, timeout, output_too_large,
    serialization_error) now carry machine codes.
  - `helpers.rs` validation error paths now use `error_with_code` with
    `INVALID_ARGUMENTS` or `INPUT_TOO_LARGE`.
  - 22 new tests in `tests/mcp/test_machine_codes.rs` covering constants
    validity, constructor behavior, finding shape, and machine code
    presence on composite tools.
  - New `architecture/machine-codes.md` reference doc with full code table,
    finding helpers, severity/disposition/verdict constants, and composite
    tool verdict patterns.
  - Updated `architecture/mcp-server.md`, `.skills/mcp-tools.md`,
    `.skills/testing.md`, `README.md`, `docs/mcp-tools.md`, and `AGENTS.md`
    to document the new response contract.

### Fixed
- **BUG-001 / B1**: Raised `MAX_FACTORIAL` from 170 → 1000 to match
  Python's `math.factorial` upper bound.
- **BUG-002 / B2**: `factorial()` / `perm()` now use base-1e9
  big-integer arithmetic and surface exact results via a new
  `__int_result__` sentinel (MCP `type: "int"`, no f64 rounding).
- **BUG-003 / B3**: `polar()` accepts the common `polar(r, phi)` two-
  arg form and returns the `(r, phi)` tuple string. Single-arg form
  still works (Python `cmath.polar` semantics).
- **BUG-004 / B4**: `rect(r, phi)` now returns `(r·cos(phi),
  r·sin(phi))` to match Python's `cmath.rect`, which produces a
  complex number.
- **BUG-006 / B6**: `three point one four` parses as `3.14`. The
  `POINT_RE` / `MERGE_DECIMAL_RE` pass now runs before
  `combine_consecutive_number_words()` so the trailing `one` isn't
  consumed as a number word.
- **BUG-007 / B7**: Compact temperature conversions like `100c in f`
  and `100 rankine in celsius` work. `TEMP_CONVERSION_RE` accepts
  zero-width whitespace between number and unit, and a new
  `resolve_unit_canon()` does case-insensitive alias lookup in
  `handle_convert_value()`.
- **BUG-008 / B8**: `math_eval` returns `96.56 km/h` for `60 mph in
  km/h`. The `run()` pipeline is the canonical evaluation path for
  both CLI and MCP.
- **BUG-009 / B9**: Spaced and compound unit expressions now parse:
  `60 mph + 60 km/h`, `60 miles per hour`, `60 kph`, `60 kph + 30 mph`,
  `60 meter per second`, `1 mile per minute`, `60 km per hr`. New
  `BARE_COMPOUND_UNITS`, `PER_UNIT_RE`, `BARE_SIMPLE_UNIT_RE`, and
  `UNIT_INLINE_RE` patterns plus a rewritten `preprocess_units()`
  handle the spacing and `per`/`kph` variants.
- **BUG-LRC-001 / B10**: `line_range_compare` now rejects out-of-range
  line indices with an error instead of panicking.
- **BUG-201**: `path_normalize` no longer duplicates the drive letter
  on Windows paths like `C:\foo\bar`; the joined component is stripped
  of the leading `C:` before being re-prepended.
- **BUG-202**: `json_extract` recognises RFC 6901's `-` reference
  token for arrays (the after-last sentinel) and reports
  `index_out_of_range` instead of `invalid_pointer_syntax`.
- **BUG-203**: `json_compare` now reports mismatched object key counts
  as `object_key_count_changed` (not `array_length_changed`) and keeps
  `same_type` true when both sides are objects.
- **BUG-204**: Removed the dead `MAX_RESULT_DIGITS` branch in
  `check_result_value` (the saturating `as i64` cast made the digit
  cap unreachable); `MAX_RESULT_VALUE` already gates overflow.
- **BUG-205**: `perm(n, r)` and `comb(n, r)` now use base-1e9
  big-integer arithmetic so results up to `MAX_PERM_COMB` are exact.
  Values within the 53-bit f64 mantissa are returned as float; larger
  values surface via the `__int_result__` sentinel.
- **BUG-206**: `nextprime` and `prevprime` now enforce the
  `MAX_PRIME` upper-bound guard that `isprime` already had, closing
  a denial-of-service surface in the `math_eval` MCP tool.
- **BUG-207**: `is_unit("b")` correctly resolves to `bit`; the
  lowercase SI bit symbol is now an explicit alias in `UNIT_ALIASES`,
  so the uppercase fallback no longer aliases it to byte `B`.
- **BUG-208**: `glob_match` no longer panics when a malformed glob
  bracket range translates into an invalid regex; invalid translated
  segments are treated as non-matches.

### Changed
- Centralized MCP server identity and protocol constants in
  `src/mcp/server.rs`.
- Added a registration invariant test so MCP tool definitions, handlers,
  metadata, and the exported tool count cannot drift silently.
- Added conventional `-h`/`--help` and `-V`/`--version` CLI handling with
  parser tests, and documented the flag behavior in the CLI guide.
- Expanded `release.sh` and contributing docs so release builds run formatting,
  clippy, and the full test suite before `cargo build --release`.
- Added `cargo package` to the release script and GitHub Actions so crates.io
  packaging is verified before publishing.
- GitHub Actions now mirrors the documented release gates: formatting, clippy
  with warnings denied, build, tests, and package verification.
- Centralized list-argument validation for `list_compare`, `list_dedupe`, and
  `list_sort` tool handlers to reduce duplicated MCP boundary checks.
- Refreshed README and MCP reference examples to match current unit output
  and MCP `content` response shape.
- Aligned README, MCP reference, and architecture category counts with the
  server's `TOOL_METADATA` taxonomy.

### Tests
- 33 `test_bug00{1..9}_*` regression tests in
  `eggsact/tests/calc/test_bug_regression.rs`.
- 10 `test_bug2{01..07}_*` regression tests (Windows drive-letter
  path normalization, RFC 6901 `/-` array pointer, object key-count
  diff kind, dead digit-cap removal, perm/comb big-int precision,
  prime upper-bound guard, lowercase `b` bit alias).
- Added `glob_match` regression coverage for invalid bracket ranges that
  previously panicked during regex compilation.
- Added direct list-tool handler coverage for malformed list arguments that
  bypass JSON schema preflight.
- Cross-binary parity assertions in
  `eggsact/tests/parity/test_bug_fixes.rs`.
- 168 edge-case tests in `eggsact/tests/mcp/test_edge_cases.rs`
  covering math eval (division by zero, overflow, nested parens,
  factorial big-int, polar, rect), unit convert (NaN/Inf rejection,
  temperature extremes, cross-category), text equal (NFC/NFD/NFKC
  normalization, casefold, trim, newline style), text fingerprint
  (casefold, NFC, empty, Unicode), text measure (emoji, combining
  chars, null bytes), text inspect (invisibles, bidi, confusables,
  BOM), JSON tools (deep nesting, special keys, trailing commas),
  shell tools (backslash escape, unterminated quotes), version tools
  (prerelease, build metadata, constraints), list tools (ordered/set/
  multiset modes, dedupe order preservation), path tools (empty,
  root, dotdot traversal), identifier tools (empty, casefold
  collision), regex tools (groups, ReDoS detection), markdown tools
  (empty, code fences), validate tools (brackets, TOML), escape/
  unescape (posix_shell, json, python), line range (out-of-bounds),
  dotenv/ini (empty, quotes, comments), patch tools (empty), text
  truncate (emoji grapheme boundary), glob match, text transform
  (NFC, casefold), text hash (SHA-256, MD5, empty), prompt input
  inspect (instruction phrases, HTML comments), security inspect
  (clean text, machine_code), unit info, structured data compare,
  cargo toml inspect, protocol (float/string IDs, notifications,
  tools/list field validation).

### Known Differences
- `version_compare` in `semver` mode preserves Python parity by comparing
  only major/minor/patch. Pre-release ordering is enforced by
  `version_constraint_check`.
- `json_compare` treats `1.0` and `1` as different JSON values, matching
  JSON type-sensitive comparison.

## [Unreleased — corrective verification pass] - 2026-07-06

### Added
- `run_with_registry(&ToolRegistry, &Input)` variants on all five typed
  preflight wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`,
  `PatchApplyCheck`, `TextSecurityInspect`). Lets callers override the
  default profile/audience per call. Existing `run()` API is unchanged.
- `apply_cancellation(&ActiveRequests, &Value)` testable helper extracted
  from the MCP server's `notifications/cancelled` handler.
  `#[doc(hidden)] pub` for tests.
- `mcp::runtime::test_support::make_active_request(cancel_flag)` helper
  for tests. `#[doc(hidden)] pub`.
- `platform` property on `path_batch_scope_check` input schema
  (`posix` / `windows` / `auto`).
- 16 new integration tests for `repo_tree_summarize`,
  `diff_risk_classify`, and `path_batch_scope_check` covering bounds,
  verdicts, and audience routing (`tests/mcp/test_repo_diff_path_tools.rs`).
- 15 new tests for typed wrapper correctness including regression test
  for canonical argument names (`tests/mcp/test_preflight_wrappers.rs`).
- 14 new tests for `mcp::runtime` cancellation and active-request helpers
  (`tests/mcp/test_runtime_helpers.rs`).

### Changed
- **PatchApplyCheckInput**: renamed fields `patch` → `patch_text`,
  `original` → `original_text` to match the canonical MCP tool schema
  in `src/mcp/schemas/patch.rs`. The old names were wrong (didn't match
  the wire schema) — this is a bug fix, not a breaking change.
- `PatchApplyCheck::run()` now uses
  `ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)`
  (previously `ToolRegistry::default()` = Model audience). This was a
  silent failure: `patch_apply_check` is `HarnessOnly` exposure and
  could never execute under Model audience. New default actually works.
- `diff_risk_classify`: `max_patch_chars` default lowered from 200,000
  to 100,000 to match `ToolBudget::MODERATE.max_text_bytes`. Aligns
  advertised budget bound with the actual enforced one.

### Fixed
- **MCP server panic path**: `server.rs` no longer panics on
  `expect("tool semaphore unexpectedly closed")`. Replaced with a
  graceful `INTERNAL_ERROR` tool response when the semaphore is dropped
  (server shutting down).
- **Documentation drift**:
  - `architecture/mcp-server.md`: removed nonexistent `MAX_CANCELLED_REQUESTS`
    constant; replaced "cancellation set" with the actual `ActiveRequests`
    map model.
  - `architecture/overview.md`: corrected "serial at the read-loop level"
    claim — the read loop is concurrent via `JoinSet` + `mpsc` writer.
  - `README.md`: corrected `runtime.rs` description from "cancelled
    requests" to "active request tracking".
  - `tests/mcp/test_hardening_and_gaps.rs`: updated stale comment header
    referencing `TestCancelledRequests`.
  - `docs/parity.md`: removed nonexistent `MAX_TOOL_TIMEOUT_SECONDS`
    constant; updated verification status (357 passed, 56 failed as of
    2026-07-06); documented 3 new concurrent-ordering parity failures.
  - `AGENTS.md`: parity known-failures count updated 53 → 56.

## [Unreleased — final closure pass] - 2026-07-07

### Added
- `ToolBudget::with_max_text_bytes(n)` builder for customising the per-call
  text-length cap. Matches the existing builder pattern for `max_input_bytes`,
  `max_output_bytes`, `max_findings`, and `max_elapsed_ms`.
- 3 regression tests for the multi-request MCP correlation helper
  (`tests/mcp/test_comprehensive_parity.rs`):
  - `test_correlation_helper_uses_string_ids`
  - `test_correlation_helper_preserves_request_order_under_concurrency`
  - `test_correlation_helper_handles_notification_alongside_requests`
- 1 budget unit test: `with_max_text_bytes_overrides_limit`
  (`src/mcp/budget.rs`).
- 1 budget unit test: `check_text_len_shim_forwards_to_check_text_bytes`
  (`src/mcp/budget.rs`).

### Changed
- `mcp_request_multi()` (`tests/mcp/test_comprehensive_parity.rs`) now
  correlates responses by JSON-RPC `id` field rather than positional index.
  The MCP stdio server dispatches requests concurrently via `tokio::JoinSet`,
  so responses may arrive in completion order. The helper:
  - parses each request's `id` up front;
  - indexes responses into a `HashMap<Value, Value>` keyed by id;
  - returns responses in request-slice order so existing positional
    assertions remain stable;
  - skips notifications (no id) silently;
  - hard-fails on duplicate, missing, or unexpected response ids.
- `docs/parity.md`: Category D (concurrent ordering) marked Resolved;
  verification status updated to 362 passed / 54 failed / 2 ignored (out of
  416 parity tests, post id-correlation fix).
- `architecture/mcp-server.md`: added explicit "Response ordering contract"
  section to the concurrency model — clients must correlate responses to
  requests by JSON-RPC `id`, not by arrival position. Notifications produce
  no response by JSON-RPC contract.
- `README.md`: MCP server section now notes concurrent dispatch and the
  id-correlation requirement.
- `AGENTS.md`: parity known-failures count updated 56 → 54; added Key
  gotcha entry on MCP concurrent response ordering.

### Deprecated
- `BudgetContext::check_text_len(...)` is retained as a `#[deprecated]`
  alias for `check_text_bytes(...)`. The method was renamed in 1.1.4 because
  enforcement is byte-based (`str::len()`), not character-based. The shim
  forwards to the canonical method and emits a deprecation note. Direct
  struct literals of `ToolBudget` remain valid; prefer builders
  (`ToolBudget::with_max_text_bytes(n)` etc.) to avoid ABI breaks when
  fields are renamed.

### Fixed
- 3 parity failures in multi-request MCP sessions resolved by switching
  `mcp_request_multi()` to id-based correlation. The concurrent server
  behaviour is correct and intentional; the previous helper was
  positionally correlating responses, which is not a valid assumption under
  concurrent dispatch. Affected tests:
  - `test_sequential_session_multiple_tools`
  - `test_sequential_session_tool_then_error_then_tool`
  - `test_correlation_helper_handles_notification_alongside_requests` (new
    test caught a race condition where the test used
    `notifications/cancelled` against a live request id, causing that
    request to actually be cancelled; corrected to target an unused id 999).
- Verification gates: `cargo fmt --check`, `cargo clippy
  --all-targets --all-features -- -D warnings`,
  `cargo run --bin generate-docs -- --check`, and
  `cargo package --verbose` all pass locally. Full non-parity test suite:
  2885 passed, 130 failed (130 pre-existing parity/MCP harness failures,
  unchanged from baseline). Parity test suite: 362 passed, 54 failed
  (improved from 356 passed / 57 failed baseline by 6 passing and 3 fewer
  failing).

## [0.1.0] - 2026-05-30

### Added

- **CLI binary** with expression evaluation and `--mcp` server mode
- **Library API** with `run()`, `evaluate()`, and `split_at_operators()`
- **Natural language math** parsing ("thirty plus five", "two to the power of ten")
- **Standard math** evaluation with full Python expression syntax
- **Unit conversions** across length, mass, time, volume, temperature, and more
- **Physical and mathematical constants** (pi, e, speed of light, Planck, Avogadro, etc.)
- **Statistical functions** (sum, mean, median, std, variance, min, max, product)
- **Number theory** (gcd, lcm, factorial)
- **MCP server** (stdio JSON-RPC 2.0, protocol version 2024-11-05, server identity `eggsact`)
- **64 MCP tools** across 16 metadata categories:
  - Math (4): math_eval, unit_convert, unit_info, constant_lookup
  - Text (18): text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window, text_transform, text_replace_check, text_security_inspect, escape_text, unescape_text, prompt_input_inspect, line_range_extract, line_range_compare
  - JSON (6): json_extract, json_compare, json_canonicalize, json_query, json_shape, structured_data_compare
  - Validation (4): validate_json, validate_brackets, validate_toml, validate_schema_light
  - Path (5): path_normalize, path_analyze, path_compare, path_scope_check, glob_match
  - Shell (4): shell_split, shell_quote_join, argv_compare, command_preflight
  - Regex (3): validate_regex, regex_safety_check, regex_finditer
  - List (3): list_compare, list_dedupe, list_sort
  - Markdown (2): markdown_structure, code_fence_extract
  - Patch (3): patch_apply_check, patch_summary, edit_preflight
  - Config (3): dotenv_validate, ini_validate, config_preflight
  - Identifier (3): identifier_analyze, identifier_inspect, identifier_table_inspect
  - Unicode (2): unicode_policy_check, canonicalize_text
  - Version (2): version_constraint_check, version_compare
  - TOML (1): toml_shape
  - Cargo (1): cargo_toml_inspect
- **Text processing library** (24 modules): primitives, confusables, diff, measure, validate, transform, position, regex_safety, replace, path, identifier, shell, markdown, glob, config, toml, patch, line_range, unicode_policy, unicode_tools, inspect_prompt, synthesis, cargo, version
- **Test suite** with unit, integration, MCP protocol, and Python parity tests

### Known Differences from Python eggcalc

- `text_hash`: Rust uses `algorithm` (singular), Python uses `algorithms` (plural)
- `text_position`: Rust is more lenient with invalid values, returns `valid: false` instead of error
- `text_truncate`: Rust uses `max_graphemes` parameter name
- `validate_toml`: Error message formats differ between implementations
