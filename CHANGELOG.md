# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.3.0] - 2026-07-17

### Added
- **`call_json_with_execution_template`**: explicit immutable alias for
  `call_json_with_execution_context`. Identical behavior (clones `eval_ctx`);
  use when you want to make the immutability intent explicit at the call site.
- **`call_json_with_execution_context_mut`**: mutable persistent-context
  variant. Accepts `&mut ExecutionContext` and persists handler state
  mutations (PRNG draws, memory registers, user variables) back to the
  caller's `EvalContext`. Use for sequential calculator operations where
  state should accumulate.
- **`prepare_tool_call_with_policy`**: shared policy preparation method
  accepting explicit effective profile, audience, and compatibility mode.
  Used internally by `call_json_with_execution_context` to resolve
  `ExecutionContext` overrides before dispatch.
- **`get_tool_unfiltered` / `has_registered_tool`**: administrative
  tool-lookup methods that bypass audience/exposure checks. `get_tool` and
  `has_tool` now enforce audience/exposure in addition to profile membership.
- **RAII guards in `budget.rs`**: `CancelFlagGuard` and `EvalContextGuard`
  provide panic-safe thread-local restoration for `CURRENT_CANCEL_FLAG` and
  `CURRENT_EVAL_CONTEXT`.

### Changed
- **MCP dispatch no longer calls `ensure_mcp_defaults()` / `set_mcp_mode()`**.
  Instead, MCP dispatch creates `EvalContext::mcp_mode()` and sets it via
  `budget::with_eval_context()` thread-local bridge before handler dispatch.
  This provides state isolation without global side effects. The global
  `MCP_MODE`, `ALLOW_RANDOM`, `ALLOW_SIDE_EFFECTS` flags remain for legacy
  library callers but are no longer set by MCP dispatch.
- **`with_eval_context` now takes `&EvalContext`** (shared reference) instead
  of `&mut EvalContext`. This aligns with the clone-on-dispatch semantics of
  `call_json_with_execution_context`.
- `set_mcp_mode()` is now deprecated for new code. Use
  `EvalContext::mcp_mode()` instead.

### Deprecated
- **`ensure_mcp_defaults()`** — MCP dispatch now creates
  `EvalContext::mcp_mode()` directly. The function is retained for backward
  compatibility but should not be called in new code.
- **`set_mcp_mode()`** — use `EvalContext::mcp_mode()` instead. The global
  `AtomicBool` flags remain for legacy `evaluate()`/`run()` callers.

## [1.2.0] - 2026-07-17

### Added
- **MCP lifecycle enforcement**: The server now requires a proper `initialize` →
  `notifications/initialized` handshake before accepting `tools/list`,
  `tools/call`, `profiles/list`, and other extension methods. Methods called
  before initialization return a structured `-32600` error with
  `NOT_INITIALIZED` machine code.
- **Typed initialize parameters**: `InitializeParams` struct validates
  `protocolVersion`, `capabilities`, and `clientInfo` fields with proper
  error messages for missing or malformed fields.
- **Protocol version negotiation**: The server supports multiple MCP protocol
  revisions (`2025-11-25` preferred, `2024-11-05` legacy). Negotiation
  returns the requested version if supported, otherwise falls back to the
  preferred version.
- **Session state machine**: Per-connection `SessionState` enum tracks
  `Uninitialized` → `AwaitingInitialized` → `Ready` transitions. Duplicate
  `initialize` requests are rejected with `ALREADY_INITIALIZED`.
- **Server capabilities advertisement**: The initialize response now includes
  `experimental.eggsact` capabilities for `profiles`, `schemaDetail`, and
  `audienceFiltering`.
- **Lifecycle-aware request dispatch**: The server handles `initialize`
  requests inline in the read loop to avoid race conditions with
  `notifications/initialized`.

### Changed
- **Breaking**: Clients must now send `initialize` and `notifications/initialized`
  before calling tools. Previous behavior where tools were available immediately
  is no longer supported. The `EGGSACT_MCP_LEGACY_NO_INIT` escape hatch is
  NOT implemented in this release — fix consumers instead.
- `MCP_PROTOCOL_VERSION` constant now resolves to the preferred version
  (`2025-11-25`) instead of the legacy `2024-11-05`.

## [1.1.5] - 2026-07-16

### Added
- **Runtime metrics**: `RUNTIME_METRICS` global provides live atomic counters
  for `active_requests`, `active_blocking_handlers`, `timed_out_handlers`,
  `total_timeouts`, and `peak_blocking_concurrency`. RAII `MetricGuard`
  ensures counters decrement correctly on panic/unwind. Exposed via
  `snapshot_metrics()` for diagnostics.
- **`register_request()` API**: atomic check+insert under one lock
  acquisition. Returns `Result<RequestGuard, RegisterRequestError>` with
  `DuplicateId` and `CapacityExceeded` error variants. Replaces the previous
  two-lock-window approach for in-flight and duplicate checks.
- 10 new execution safety integration tests (`tests/mcp/test_execution_safety.rs`):
  integer ID duplicates, string ID duplicates, ID reuse after completion,
  cancellation targeting, cancellation of unknown/completed requests,
  worker containment under concurrency, timeout permit leak detection,
  graceful shutdown drain, malformed cancellation notification handling.
- 12 new runtime helper tests (`tests/mcp/test_runtime_helpers.rs`):
  `register_request` success/duplicate/capacity/guard-cleanup/reuse,
  `MetricGuard` increment/decrement/nesting, `snapshot_metrics`,
  runtime peak tracking, `apply_cancellation` flag-outside-lock.

### Changed
- **Two-lock-window violation fixed**: in-flight limit check, duplicate ID
  check, and active-request insertion now occur under a single lock
  acquisition via `register_request()`. Previously these were three separate
  lock windows, creating a race where two requests with the same ID could
  both pass the duplicate check before either was inserted.
- **Cancellation flag set outside critical section**: `apply_cancellation`
  now clones the `Arc<AtomicBool>`, releases the active-map lock, then sets
  the flag outside the critical section. Previously the flag was set while
  holding the lock.
- **Malformed cancellation notifications logged to stderr**: missing
  `requestId` parameter or missing `params` now emit a diagnostic warning
  on stderr instead of being silently ignored.
- **Worker permit tracking**: `spawn_blocking` closures now include
  `MetricGuard` for active-blocking-handler counting and peak concurrency
  watermark updates. Timeout path increments `total_timeouts` and
  `timed_out_handlers` counters; handler exit decrements
  `timed_out_handlers`.
- `architecture/mcp-server.md`: documented nested thread lifetimes
  (`run_with_timeout` inner `std::thread::spawn` that can outlive the
  handler by 5–30 seconds), `register_request` API, runtime metrics
  counters, and `MetricGuard` RAII pattern.

### Fixed
- Duplicate ID check and insertion are now atomic under one lock, closing
  a race where two concurrent requests with the same ID could both pass
  validation.

## [1.1.4] - 2026-07-09

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
- **`EGGCALC_MCP_AUDIENCE` env var**: controls `ToolAudience` for MCP subprocess
  spawns (`Model`, `Harness`, `Debug`). Case-insensitive. Defaults to `Model`
  on invalid values with a diagnostic warning. Used by test helpers to access
  HarnessOnly tools.
- **Accepted parity failures fixture**: `tests/fixtures/accepted_parity_failures.txt`
  lists all 31 accepted parity failure test names for regression detection.
- **`docs/release-readiness.md`**: new file documenting the release candidate state (commit SHA, CI result, local gate, package status, known deferred items, publish checklist).
- **CI policy statement**: AGENTS.md, `.skills/release.md`, and `docs/contributing.md` all now explicitly state that GitHub CI does not publish to crates.io.
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
- **Canonical release doc**: `docs/release.md` is now the single source of truth for release procedure. Added explicit "Release policy" section stating GitHub CI verifies release readiness but does NOT publish to crates.io; the maintainer publishes manually with `cargo publish` from a local authenticated environment.
- **Tagging policy**: documented tag-after-publish policy in `docs/release.md` (crates.io releases are immutable; tag after `cargo publish` succeeds).
- **Crates.io publishing section**: new "Manual crates.io publishing" section in `docs/release.md` with prerequisites, pre-publish (`cargo publish --dry-run`), publish command, and tagging order.
- **Package excludes tightened**: `Cargo.toml` `exclude` list now also excludes `.skills/`, `release.sh`, `AGENTS.md`, and `deny.toml`. Internal agent skill docs and CI-only config no longer ship in the published crate.
- **CI workflow** (`ci.yml`): Split single `test` job into `test-lib`,
  `test-bins`, and `test-integration` (with `--skip parity`). Added
  `workflow_dispatch` trigger. Package job depends on all five check jobs.
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

### Fixed
- **Stale `architecture/release.md` reference**: AGENTS.md listed a file at `architecture/release.md` that does not exist. Replaced with a pointer to the canonical `docs/release.md`.
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
- **Parity Category A (23 tests)**: Fixed test-harness audience bug.
  HarnessOnly tools were rejected by `ToolAudience::Model` in MCP
  subprocess spawns. Added `EGGCALC_MCP_AUDIENCE` env var and updated
  all 9 MCP test helper files.
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

### Deprecated
- `BudgetContext::check_text_len(...)` is retained as a `#[deprecated]`
  alias for `check_text_bytes(...)`. The method was renamed in 1.1.4 because
  enforcement is byte-based (`str::len()`), not character-based. The shim
  forwards to the canonical method and emits a deprecation note. Direct
  struct literals of `ToolBudget` remain valid; prefer builders
  (`ToolBudget::with_max_text_bytes(n)` etc.) to avoid ABI breaks when
  fields are renamed.

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
