# AGENTS.md

Deterministic MCP and in-process utility tools for coding agents. Single crate, no workspace. 80 tools across 20 categories: math, text, JSON, regex, path, shell, config, patch, dependency, analysis, and more.

## Commands

```bash
cargo build                          # debug build
cargo build --release                # release build
cargo test                           # all tests (unit + integration + parity)
cargo test --lib                     # unit tests in src/ only
cargo test --test lib mcp            # MCP tests only
cargo test --test lib parity         # parity tests only
cargo test --test lib text           # text tests only
cargo test --doc                     # doc tests
cargo fmt --all -- --check            # format check
cargo clippy --all-targets --all-features  # lint
cargo package                        # crates.io packaging dry run
cargo run --bin generate-docs        # regenerate docs from ToolSpec registry
cargo run --bin generate-docs -- --check  # verify generated docs are current (CI)
./release.sh                         # full pipeline: regenerate data, fmt, clippy, test, generate-docs check, package
```

## CI

GitHub Actions CI runs on push/PR to `main` (plus manual `workflow_dispatch`):
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features --lib` (unit tests)
- `cargo test --all-features --bins` (binary tests)
- `cargo test --all-features --tests -- --skip parity` (integration tests, parity excluded)
- `cargo run --bin generate-docs -- --check` (generated docs freshness)
- `cargo package --verbose` (after all checks pass)

Parity tests are excluded from CI because Python `eggcalc` is not available in
the CI environment. Run parity locally with `cargo test --test lib parity`.

GitHub Actions CI verifies release readiness but does **not** publish to crates.io. The maintainer publishes manually per `docs/release.md`.

## Verification order

`cargo fmt --all -- --check` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo test --all-features --lib` → `cargo test --all-features --bins` → `cargo test --all-features --tests -- --skip parity` → `cargo test --doc` → `cargo run --bin generate-docs -- --check` → `cargo package --verbose`

## Structure

```
src/
  main.rs           # CLI entry, arg parsing, dispatch
  lib.rs            # library root, re-exports run()/evaluate()
  bin/
    generate_docs.rs # generates docs from ToolSpec registry (README tables, profile refs, tool cards)
  calc/             # calculator: evaluator, normalize, units (3 modules)
  mcp/              # MCP server protocol, runtime, registry, validation
    server.rs       # protocol orchestration, stdio loop, dispatch
    compat.rs       # CompatibilityMode enum (EggcalcPython vs StrictNative)
    registry/       # tool registration (ToolSpec declarations, single source of truth)
      mod.rs        # re-exports, tests
      types.rs      # ToolDefinition, ToolSpec, enums
      all_tools.rs  # ALL_TOOLS aggregation from specs/, PROFILE_NAMES
      listing.rs    # filtering, audience, schema compaction, suggestions
    specs/          # ToolSpec declarations per tool category
      mod.rs        # re-exports all category slices
      math.rs       # MATH_TOOLS
      text.rs       # TEXT_TOOLS
      json.rs       # JSON_TOOLS
      ...           # one file per category (20 total)
    protocol.rs     # JSON-RPC types (Request, Response, Error, InitializeResult)
    response.rs     # ToolResponse, error sanitization, finding() helpers, with_verdict, preflight builders
    machine_codes.rs # machine-readable response codes, severity/disposition/verdict constants
    budget.rs       # per-tool budgets, tiers, composite sub-budgets, BudgetContext with cooperative helpers
    runtime.rs      # rate limiter, constants, profile management, schema detail validation
    schema_validation.rs # argument validation against tool schemas
    schemas/        # JSON-schema builders per tool category
      mod.rs        # module declarations + re-exports
      math.rs       # math/text/json/regex/path/shell/etc. schema builders
      ...           # one file per category (20 total)
  tools/            # MCP tool implementations (by category)
    helpers.rs      # shared constants, utilities, helper functions
    math.rs         # math_eval, unit_convert, unit_info, constant_lookup
    text.rs         # text_measure, text_equal, text_diff_explain, etc. (18 tools)
    json.rs         # json_extract, json_compare, json_canonicalize, etc. (6 tools)
    regex.rs        # validate_regex, regex_safety_check, regex_finditer
    validation.rs   # validate_json, validate_brackets, validate_toml, validate_schema_light
    path.rs         # path_normalize, path_analyze, path_compare, path_scope_check, glob_match, path_batch_scope_check
    shell.rs        # shell_split, shell_quote_join, argv_compare, command_preflight
    list.rs         # list_compare, list_dedupe, list_sort
    markdown.rs     # markdown_structure, code_fence_extract
    patch.rs        # patch_apply_check, patch_summary, edit_preflight, diff_risk_classify, patch_contract_check
    config.rs       # dotenv_validate, ini_validate, config_preflight, toml_shape_tool
    identifier.rs   # identifier_analyze, identifier_inspect, identifier_table_inspect
    unicode.rs      # unicode_policy_check, canonicalize_text
    version.rs      # version_compare, version_constraint_check
    cargo.rs        # cargo_toml_inspect
    dependency.rs   # dependency_edit_preflight
    diagnostics.rs  # runtime_diagnostics, profile_inspect, tool_availability_explain
    repo.rs         # repo_manifest_inspect, config_file_inspect, repo_tree_summarize, test_command_suggest, repo_language_detect
    analysis.rs     # import_export_inspect, code_block_map, symbol_name_diff, lockfile_inspect
  text/             # text processing library (25 modules)
    regex_engine.rs # regex backend classifier (classify_pattern, RegexEngineUsed, RegexFeature)
  agent/            # in-process agent API (ToolRegistry, Profile, call_json)
  preflight/        # typed preflight wrappers (EditPreflight, CommandPreflight, ConfigPreflight, PatchApplyCheck, TextSecurityInspect), strict finding parsing, structured RecommendedNextTool
tests/
  lib.rs            # declares test modules: calc, mcp, parity, text
  calc/             # calculator tests (4 files)
  mcp/              # MCP protocol + tool tests (28 files)
  parity/           # Python/Rust parity tests (12 files)
  text/             # text processing tests (25 files)
scripts/
  generate_confusables.py  # regenerates src/text/confusables_generated.rs from unicode.org
generated/
  tool-cards.md    # generated compact tool cards per codegg profile
```

## Architecture docs

Detailed architecture documentation is in `architecture/`:

### Core

- `architecture/overview.md` — directory layout, dependency flow, constants, context isolation
- `architecture/calculator.md` — NL pipeline (30-step), AST evaluator, 140+ units, 50+ constants, EvalContext

### MCP Server & Tools

- `architecture/mcp-server.md` — JSON-RPC 2.0, tokio dispatch, tool registration, profiles, schemas
- `architecture/machine-codes.md` — ~100 response codes, finding helpers, severity/disposition/verdict
- `architecture/tools.md` — 80 tools across 20 categories, handler conventions, budget integration
- `architecture/compatibility.md` — `EggcalcPython` vs `StrictNative` modes, behavior differences

### Agent Integration

- `architecture/agent-api.md` — in-process ToolRegistry, call_json variants, ExecutionContext
- `architecture/preflight.md` — typed wrappers (Edit/Command/Config/Patch/TextSecurity), PreflightError taxonomy
- `architecture/coding-agent-integration.md` — MCP stdio vs in-process, profiles, audiences, concurrency

### Text & Testing

- `architecture/text-library.md` — 25 text modules, public API, regex engine auto-selection
- `architecture/testing.md` — 70+ test files, parity framework, route-contract fixtures, CI pipeline

### Assets & CLI

- `architecture/generated-assets.md` — doc generation, confusables data, diagnostics
- `architecture/cli-binaries.md` — generate-docs, verify-eggsact binaries, --diagnostics

Additional docs in `docs/`:

- `docs/compatibility-policy.md` — semver, breaking changes, tool/schema/machine-code stability
- `docs/contributing.md` — prerequisites, building, testing, adding new tools
- `docs/parity.md` — Python/Rust parity, 33 known failures (categories C1–C6)
- `docs/release.md` — canonical release checklist, manual crates.io publishing
- `docs/library-api.md` — in-process API, ToolRegistry, call_json variants
- `docs/mcp-tools.md` — MCP tool catalog with input/output schemas
- `docs/cli.md` — CLI flags, subcommands, environment variables

## Agent API

`src/agent/` provides an in-process API for calling tools without MCP. `ToolRegistry` wraps the tool registry with profile filtering and `call_json()` dispatch. `call_json_with_budget()` accepts a custom `ToolBudget` to override default per-tool limits. `call_json_with_execution_context()` accepts an `ExecutionContext` for full per-request state isolation — recommended for new code (immutable, clones `eval_ctx`). `call_json_with_execution_template()` is an explicit immutable alias for `call_json_with_execution_context()`. `call_json_with_execution_context_mut()` accepts a `&mut ExecutionContext` and persists handler state mutations back to the caller's context. `src/preflight/` adds typed wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`) that parse tool responses into structured Rust types with fail-closed contract enforcement.

- **`PreflightError`** has three variants: `ToolCall` (registry rejected), `ToolRejected` (tool returned `ok: false`), `ContractViolation` (missing mandatory field in `ok: true` response). Missing fields are hard failures, not silent defaults.
- **Typed verdict enums**: `EditVerdict`, `CommandVerdict`, `ConfigVerdict` with `Other(String)` variant for forward compatibility. `FindingSeverity` and `FindingDisposition` follow the same pattern.
- **`RecommendedNextTool`** struct: `{ name: String, reason: Option<String>, arguments_hint: Option<Value> }`. Parsed from both string and object shapes; fails closed on malformed values.
- **Strict finding parsing**: `Finding::try_from_value_strict()` and `Finding::from_array_strict()` require `code`, `severity`, and `message` strings. Used by all typed preflight wrappers. Permissive `Finding::from_value()` / `Finding::from_array()` preserved for backward compatibility.
- **`parse_response()`** is public on each wrapper for testing contract parsing without a full registry call.
- **`EditPreflightInput`** accepts optional `file_path`/`workspace_root` (triggers `path_scope_check`), `newline_policy` (triggers `text_fingerprint` newline detection), `unicode_policy` (triggers `text_security_inspect`), `expected_fingerprint` (triggers `text_fingerprint` SHA-256 comparison), and `edit_metadata` (passthrough). All sub-tool results appear in `subresults` and structured output fields.
- **`available_tools()` is deprecated** since 0.3.0 — it only filters `Hidden` and is not model-safe. Use `available_tools_model_safe()`, `available_tools_for_audience(audience)`, or `available_tools_for_current_audience()` instead.
- **`get_tool`/`has_tool` now check audience/exposure** in addition to profile membership. Use `get_tool_unfiltered` and `has_registered_tool` for administrative use that bypasses audience/exposure checks.
- **`with_eval_context`** takes `&EvalContext` (shared reference). `CancelFlagGuard` and `EvalContextGuard` RAII guards in `budget.rs` provide panic-safe thread-local restoration.
- **`prepare_tool_call_with_policy`** accepts explicit effective profile, audience, and compatibility mode. Shared policy preparation function used by `call_json_with_execution_context`.

- **`ToolDefinition`** lives in `src/mcp/registry/types.rs` (not `server.rs`).
- **`ToolAudience`** enum (`Model`, `Harness`, `Debug`) controls which exposure levels appear in tool listings and which tools may be executed. Use `available_tools_model_safe()` for model-facing integrations. `ToolAudience::can_execute_exposure()` enforces audience at dispatch time.
- **Route-critical tools** (`is_route_critical()` in `registry/listing.rs`): `edit_preflight`, `command_preflight`, `config_preflight`, `patch_apply_check`, `text_security_inspect`. Must always emit `machine_code` and `verdict` in their response envelope. Verified by fixture-backed route-contract tests (`RouteFixture` struct, `all_fixtures()`) in `tests/mcp/test_route_contracts.rs`, including registry invariant tests, MCP stdio coverage, and audience enforcement.
- **`Profile::from_str_opt`** is strict — returns `None` for unknown names. Use `Profile::custom(name)` to construct a custom profile explicitly.
- **`ToolResponse::error`** has been renamed to `error_without_code_for_legacy_tests_only` (deprecated/hidden). Use `error_with_code()` instead.
- **`CompatibilityMode`** enum (`EggcalcPython`, `StrictNative`) controls validation behavior. `StrictNative` is the default for in-process API; MCP server uses `EggcalcPython`. See `architecture/compatibility.md`.

## Exposure & Audience Model

Tools have typed `ToolExposure` and `ToolListAudience` enums in `src/mcp/registry/types.rs` and `src/mcp/registry/listing.rs`:

- **Exposure**: `Default`, `Contextual`, `ExpertOnly`, `HarnessOnly`, `Hidden` — controls which contexts a tool appears in.
- **Audience**: `Model` (excludes HarnessOnly+Hidden), `Harness` (excludes Hidden), `Debug` (all non-hidden).

Use `tools_for_profile_audience(profile, audience)` for filtered listings. Both `tools/list` and `tools/call` enforce profile membership. `tools/call` also enforces audience/exposure compatibility via `ToolRegistry::prepare_tool_call` — the active profile is resolved from `get_active_profile()` and Model audience is used by default. MCP `tools/call` rejects harness-only tools for model audience.

**No per-call profile override**: `tools/call` intentionally does NOT accept a `profile` parameter in its arguments. The active profile is set once at server startup via the `EGGCALC_MCP_PROFILE` environment variable and applies to all subsequent `tools/call` requests. (`tools/list` accepts a `profile` parameter for filtering the listing, but that does not change which profile `tools/call` enforces.) This matches the in-process API where each `ToolRegistry` is bound to one profile at construction time.

**Codegg guidance**: Use `codegg_core_min` + `Model` audience for ordinary coder-agent sessions. Use `codegg_preflight`/`codegg_shell` + `Harness` audience for automatic preflight checks via the in-process API (`ToolRegistry::with_profile_and_audience`).

Profile snapshot tests (`tests/mcp/test_hardening_and_gaps.rs`) verify that all 11 named profiles exist and their tool lists match expected tool counts.

## Skills

Agent task skills in `.opencode/skills/` (symlinked from `.agents/skills/` for Codex compatibility):

- `.opencode/skills/mcp-tools/SKILL.md` — how to add or update MCP tools
- `.opencode/skills/testing/SKILL.md` — testing patterns, commands, test structure
- `.opencode/skills/debugging/SKILL.md` — common issues, debugging workflows
- `.opencode/skills/release/SKILL.md` — release process and checklist
- `.opencode/skills/text-processing/SKILL.md` — text module conventions and patterns

## Key gotchas

- **`^` is XOR, not exponentiation.** Use `**` for power. This matches Python behavior.
- **`g` means gram** in unit expressions. Use `gravity` or `standardgravity` for standard gravity.
- **Regex backend auto-selection**: `regex_finditer` and `validate_regex` auto-select between Rust `regex` (fast, linear-time) and `fancy-regex` (lookaround/backreference support) via `classify_pattern()` in `src/text/regex_engine.rs`. Outputs report `engine_used` (`"rust-regex"` or `"fancy-regex"`), `dialect` (`"eggsact-regex"`), and `unsupported_features` for PCRE-only constructs. Unsupported constructs return `REGEX_UNSUPPORTED_FEATURE` machine code. This is NOT PCRE2.
- **Parity tests require `eggcalc`** Python package at `../eggcalc`. See `docs/parity.md` for the 33 known failures (categories C1–C6, accepted behavioral differences). Do not treat these as regressions.
- **Never edit `src/text/confusables_generated.rs`** — auto-generated by `scripts/generate_confusables.py`. Edit the script, not the output.
- **Never hand-edit generated docs** — README tool tables, architecture profile reference, and `generated/tool-cards.md` are generated by `cargo run --bin generate-docs`. Edit `ToolSpec` entries in `src/mcp/specs/` instead.
- **Adding an MCP tool requires one `ToolSpec` entry** in `src/mcp/specs/<category>.rs`. This is the single source of truth. A test (`tool_registration_tables_are_in_sync`) catches drift.
- **`deny.toml`** configures `cargo-deny` for license/advisory checks. Allowed licenses: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, Unlicense, Unicode-DFS-2016, Unicode-3.0, Zlib.
- **`Cargo.lock` is gitignored** but present. Don't commit it.
- **`serde_json` uses `preserve_order`** feature — key order is intentional in serialized JSON.
- **Env vars:** `EGGCALC_NO_CONFIG=1` (set in main.rs), `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_AUDIENCE` (case-insensitive, defaults to `Model`), `EGGCALC_MCP_SCHEMA_DETAIL` (`compact`/`normal`/`full`; defaults to `full`).
- **Input limits:** MAX_TEXT_LENGTH=100k, MAX_EXPRESSION_LENGTH=10k, MAX_LIST_ITEMS=10k, MAX_REGEX_SAMPLES=100, MAX_PATTERN_LENGTH=1k, MAX_REQUEST_BYTES=1M, MAX_OUTPUT_BYTES=1M.
- **Context-aware vs legacy APIs**: `call_json_with_execution_context()` clones `eval_ctx` — handler mutations do **not** persist back. Use `call_json_with_execution_context_mut()` when you need handler state changes to persist. Use `evaluate_with_context()`/`run_with_context()` when you need persistent mutable `EvalContext` across calculator calls. Do not mix for the same `EvalContext`. **`call_json_with_execution_context_mut` is `#[deprecated(since = "0.4.0")]`** — it does NOT persist calculator state through `math_eval` (math_eval's evaluator runs in a `catch_unwind` closure and the MCP dispatch creates fresh `EvalContext` per call). For persistent calculator state, use `evaluate_with_context()`/`run_with_context()` directly.
- **`ensure_mcp_defaults()` is deprecated** — MCP dispatch now creates `EvalContext::mcp_mode()` and sets it via `budget::with_eval_context()` thread-local bridge instead of calling `ensure_mcp_defaults()`/`set_mcp_mode()` globally.
- **MCP wire vs in-process**: `call_json_with_execution_context` is in-process only. The MCP server resolves its profile from `EGGCALC_MCP_PROFILE` at startup.
- **Response truncation is automatic**: `truncate_response()` caps findings/output when a tool exceeds its budget. Check `limits_applied` in the response envelope.
- **MCP response ordering is concurrent**: Responses may arrive out of request order. **Correlate by JSON-RPC `id`**, not arrival position.
- **Input pre-check**: Both `call_json_with_budget()` (in-process) and `tools/call` (MCP) check serialized input against `budget.max_input_bytes` before dispatch. Oversized input fails with `INPUT_TOO_LARGE`.
- **`ToolResponse::error`** has been renamed to `error_without_code_for_legacy_tests_only` (deprecated). Use `error_with_code()` instead.
- **`apply_cancellation` is async** — it uses `.lock().await` instead of `.try_lock()`, so callers must `.await` it. This prevents cancellation loss under active-map lock contention. The cancel flag is cloned from the Arc and set **outside** the critical section (after the lock is released).
- **Duplicate non-null request IDs are rejected** atomically via `register_request()` under a single lock acquisition — in-flight limit check, duplicate check, and insertion happen in one lock window. Returns `RegisterRequestError::DuplicateId` or `RegisterRequestError::CapacityExceeded`. Null IDs (`id: null`) are also rejected for requests because concurrent tracking and error correlation become ambiguous. Notifications use absent `id`, not `null`.
- **Runtime metrics**: `RUNTIME_METRICS` provides live atomic counters (`active_requests`, `active_blocking_handlers`, `timed_out_handlers`, `total_timeouts`, `peak_blocking_concurrency`). RAII `MetricGuard` ensures correct decrement on panic/unwind. `snapshot_metrics()` returns a point-in-time snapshot. `timed_out_handlers` is race-free: an `AtomicU8` lifecycle state machine (`HANDLER_RUNNING → HANDLER_TIMED_OUT → HANDLER_FINISHED`) with `compare_exchange` on timeout and `swap` on handler exit ensures exactly one increment and one decrement per timeout.
- **Panic safety net**: `math_eval`, `validate_regex`, `regex_finditer`, and `dotenv_validate` use `std::panic::catch_unwind` to convert panics to error responses instead of letting them propagate as JoinErrors.
- **Timeout lifecycle state machine**: `HANDLER_RUNNING (0) → HANDLER_TIMED_OUT (1) → HANDLER_FINISHED (2)`. Timeout path uses `compare_exchange(HANDLER_RUNNING, HANDLER_TIMED_OUT)` — only increments `timed_out_handlers` on success. Handler exit uses `swap(HANDLER_FINISHED)` — only decrements `timed_out_handlers` if previous state was `HANDLER_TIMED_OUT`. This guarantees exactly one increment and one decrement per timeout.
- **Client capabilities retained**: `NegotiatedProtocol` includes `client_capabilities: ClientCapabilities` field. Client capabilities are stored for the entire session lifetime. Not yet used for capability-dependent behavior — just retained.
- **MCP lifecycle required**: The server requires `initialize` → `notifications/initialized` before `tools/list`, `tools/call`, `profiles/list`. Methods before initialization return `-32600` with `NOT_INITIALIZED` data code. Ping is always allowed.
