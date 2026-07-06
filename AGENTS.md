# AGENTS.md

Rust reimplementation of Python `eggcalc`. Natural language math calculator + MCP server with registered tools. Single crate, no workspace.

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

GitHub Actions CI runs on push/PR to `main`:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo run --bin generate-docs -- --check` (generated docs freshness)
- `cargo package --verbose` (after all checks pass)

## Verification order

`cargo fmt --all -- --check` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo test --all-features` → `cargo run --bin generate-docs -- --check` → `cargo package --verbose`

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
      ...           # one file per category (19 total)
    protocol.rs     # JSON-RPC types (Request, Response, Error, InitializeResult)
    response.rs     # ToolResponse, error sanitization, finding() helpers, with_verdict, preflight builders
    machine_codes.rs # machine-readable response codes, severity/disposition/verdict constants
    budget.rs       # per-tool budgets, tiers, composite sub-budgets, BudgetContext with cooperative helpers
    runtime.rs      # rate limiter, constants, profile management
    schema_validation.rs # argument validation against tool schemas
    schemas/        # JSON-schema builders per tool category
      mod.rs        # module declarations + re-exports
      math.rs       # math/text/json/regex/path/shell/etc. schema builders
  tools/            # MCP tool implementations (by category)
    helpers.rs      # shared constants, utilities, helper functions
    math.rs         # math_eval, unit_convert, unit_info, constant_lookup
    text.rs         # text_measure, text_equal, text_diff_explain, etc. (18 tools)
    json.rs         # json_extract, json_compare, json_canonicalize, etc. (6 tools)
    regex.rs        # validate_regex, regex_safety_check, regex_finditer
    validation.rs   # validate_json, validate_brackets, validate_toml, validate_schema_light
    path.rs         # path_normalize, path_analyze, path_compare, glob_match, etc.
    shell.rs        # shell_split, shell_quote_join, argv_compare, command_preflight
    list.rs         # list_compare, list_dedupe, list_sort
    markdown.rs     # markdown_structure, code_fence_extract
    patch.rs        # patch_apply_check, patch_summary, edit_preflight
    config.rs       # dotenv_validate, ini_validate, config_preflight
    identifier.rs   # identifier_analyze, identifier_inspect, identifier_table_inspect
    unicode.rs      # unicode_policy_check, canonicalize_text
    version.rs      # version_compare, version_constraint_check
    cargo.rs        # cargo_toml_inspect
  text/             # text processing library (24 modules)
  agent/            # in-process agent API (ToolRegistry, Profile, call_json)
  preflight/        # typed preflight wrappers (EditPreflight, CommandPreflight, ConfigPreflight, PatchApplyCheck, TextSecurityInspect), strict finding parsing, structured RecommendedNextTool
tests/
  lib.rs            # declares test modules: calc, mcp, parity, text
  calc/             # calculator tests (4 files)
  mcp/              # MCP protocol + tool tests (22 files)
  parity/           # Python/Rust parity tests (12 files)
  text/             # text processing tests (24 files)
scripts/
  generate_confusables.py  # regenerates src/text/confusables_generated.rs from unicode.org
generated/
  tool-cards.md    # generated compact tool cards per codegg profile
```

## Architecture docs

Detailed architecture documentation is in `architecture/`:

- `architecture/overview.md` — directory layout, dependency flow, constants, context isolation model
- `architecture/calculator.md` — calculator core, NL pipeline, units, constants, EvalContext
- `architecture/mcp-server.md` — MCP protocol, tool registration, categories, error handling, ExecutionContext
- `architecture/machine-codes.md` — machine-readable response codes, finding helpers, severity/disposition/verdict constants, composite tool verdicts
- `architecture/text-library.md` — all 24 text modules, public API, code patterns
- `architecture/compatibility.md` — compatibility mode (EggcalcPython vs StrictNative), behavior differences

Additional policy docs in `docs/`:

- `docs/compatibility-policy.md` — semantic versioning, breaking changes, tool/schema/machine-code stability, deprecation timelines

## Agent API

`src/agent/` provides an in-process API for calling tools without MCP. `ToolRegistry` wraps the tool registry with profile filtering and `call_json()` dispatch. `call_json_with_budget()` accepts a custom `ToolBudget` to override default per-tool limits. `call_json_with_execution_context()` accepts an `ExecutionContext` for full per-request state isolation — recommended for new code. `src/preflight/` adds typed wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`) that parse tool responses into structured Rust types with fail-closed contract enforcement.

- **`PreflightError`** has three variants: `ToolCall` (registry rejected), `ToolRejected` (tool returned `ok: false`), `ContractViolation` (missing mandatory field in `ok: true` response). Missing fields are hard failures, not silent defaults.
- **Typed verdict enums**: `EditVerdict`, `CommandVerdict`, `ConfigVerdict` with `Other(String)` variant for forward compatibility. `FindingSeverity` and `FindingDisposition` follow the same pattern.
- **`RecommendedNextTool`** struct: `{ name: String, reason: Option<String>, arguments_hint: Option<Value> }`. Parsed from both string and object shapes; fails closed on malformed values.
- **Strict finding parsing**: `Finding::try_from_value_strict()` and `Finding::from_array_strict()` require `code`, `severity`, and `message` strings. Used by all typed preflight wrappers. Permissive `Finding::from_value()` / `Finding::from_array()` preserved for backward compatibility.
- **`parse_response()`** is public on each wrapper for testing contract parsing without a full registry call.
- **`EditPreflightInput`** accepts optional `file_path`/`workspace_root` (triggers `path_scope_check`), `newline_policy` (triggers `text_fingerprint` newline detection), `unicode_policy` (triggers `text_security_inspect`), `expected_fingerprint` (triggers `text_fingerprint` SHA-256 comparison), and `edit_metadata` (passthrough). All sub-tool results appear in `subresults` and structured output fields.
- **`available_tools()` is deprecated** since 0.3.0 — it only filters `Hidden` and is not model-safe. Use `available_tools_model_safe()`, `available_tools_for_audience(audience)`, or `available_tools_for_current_audience()` instead.

- **`ToolDefinition`** lives in `src/mcp/registry/types.rs` (not `server.rs`).
- **`ToolAudience`** enum (`Model`, `Harness`, `Debug`) controls which exposure levels appear in tool listings and which tools may be executed. Use `available_tools_model_safe()` for model-facing integrations. `ToolAudience::can_execute_exposure()` enforces audience at dispatch time.
- **Route-critical tools** (`is_route_critical()` in `registry/listing.rs`): `edit_preflight`, `command_preflight`, `config_preflight`, `patch_apply_check`, `text_security_inspect`. Must always emit `machine_code` and `verdict` in their response envelope. Verified by route-contract tests in `tests/mcp/test_route_contracts.rs`.
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

Agent task skills in `.skills/`:

- `.skills/mcp-tools.md` — how to add or update MCP tools
- `.skills/testing.md` — testing patterns, commands, test structure
- `.skills/debugging.md` — common issues, debugging workflows
- `.skills/release.md` — release process and checklist
- `.skills/text-processing.md` — text module conventions and patterns

## Key gotchas

- **`server.rs` was cleaned up**: schema compaction, response wrapping, and profile resolution moved to their respective modules (`registry/listing.rs`, `response.rs`, `runtime.rs`). The stdio read loop remains in `server.rs` but delegates most work. The read loop now spawns each request as a tokio task (via `JoinSet`) for concurrent handling, with responses serialized through an `mpsc` channel and writer task.

- **Tool timeouts are budget-derived**: `tools/call` uses `budget.max_elapsed_ms` from `ToolBudget` (resolved via `budget_for_tool()` from `ToolSpec.cost`), not a fixed 30s constant. Composite tools get sub-budgets via `SubBudget`/`CompositeBudgetAllocator`.

- **Cooperative budget checks in high-risk handlers**: `edit_preflight`, `command_preflight`, `config_preflight`, `config_file_inspect`, and `dependency_edit_preflight` create a `BudgetContext` internally (since `ToolHandler` is `fn(&Value) -> ToolResponse` and cannot receive context). They call `should_stop()` at key pipeline stages. The MCP server creates an `Arc<AtomicBool>` cancel flag and attaches it via `with_cancellation()` before dispatch; on timeout, the flag is set but blocking work may continue (cooperative, not forceful).

- **Handler signatures remain `fn(&Value) -> ToolResponse`**: Tool functions do not accept an `ExecutionContext`. Context isolation is applied at the orchestration layer (`call_json_with_execution_context`), not passed into handlers. Calculator-backed handlers (e.g., `math_eval`) retrieve `EvalContext` from a thread-local set by `budget::with_eval_context()`. This preserves compatibility with existing handler code while enabling per-request state isolation.

- **Context-aware APIs vs legacy**: Use `call_json_with_execution_context()` (agent) or `evaluate_with_context()`/`run_with_context()` (calculator) when you need per-call state isolation (e.g., reproducible PRNG, user variables, memory registers). Legacy wrappers (`call_json`, `evaluate`, `run`) are fine for simple cases where default state is acceptable.

- **Agent guidance on which context API to use**:
  - **`call_json_with_execution_context(&ctx)`** — for per-call profile/audience/compat/budget/cancellation overrides. `ctx.eval_ctx` is **cloned** at dispatch; mutations inside the handler do **not** persist back. Do not assume `ctx.eval_ctx` is mutated persistently.
  - **`evaluate_with_context(expr, ctx)` / `run_with_context(expr, ctx)`** — for persistent mutable `EvalContext` behavior across multiple calculator calls (PRNG draws accumulate, memory registers persist, user variables accumulate). These operate directly on the caller's `ctx`.
  - Do not mix the two for the same `EvalContext`: `call_json_with_execution_context` clones `ctx.eval_ctx` so handler mutations are invisible to the caller's `ctx`. Use `evaluate_with_context`/`run_with_context` when you need state to accumulate across calls.

- **MCP wire vs in-process**: `call_json_with_execution_context` is an in-process API. It does not change the MCP JSON-RPC wire protocol. The MCP server resolves its active profile from `EGGCALC_MCP_PROFILE` at startup. Per-request context over the wire would require a future MCP request-level context API.

- **Response truncation is automatic**: `truncate_response()` caps findings/output when a tool exceeds its budget limits. Check `limits_applied` in the response envelope to detect truncation. Findings cap reserves one slot for a synthetic `OUTPUT_TOO_LARGE` notice (so total ≤ `max_findings`); result over-cap is replaced with a summary object preserving `machine_code`/`verdict`/`ok`/caller-`summary`, plus `truncated: true`, `original_size_bytes`, `max_output_bytes`.
- **Input pre-check**: `call_json_with_budget()` (in-process) and `tools/call` (MCP) check serialized input against `budget.max_input_bytes` *before* dispatch. Oversized input fails with `INPUT_TOO_LARGE` (high, blocking) instead of wasting compute.

- **Never edit `src/text/confusables_generated.rs`** — auto-generated by `scripts/generate_confusables.py`. Edit the script, not the output.
- **Never hand-edit generated docs** — README tool tables, architecture profile reference, and `generated/tool-cards.md` are generated by `cargo run --bin generate-docs`. Edit `ToolSpec` entries in `src/mcp/specs/` instead, then re-run the generator.
- **Adding an MCP tool requires one `ToolSpec` entry** in `src/mcp/specs/<category>.rs`. This is the single source of truth for tool registration. A test (`tool_registration_tables_are_in_sync`) will catch drift.
- **`^` is XOR, not exponentiation.** Use `**` for power. This matches Python behavior.
- **`g` means gram** in unit expressions. Use `gravity` or `standardgravity` for standard gravity.
- **Parity tests require `eggcalc`** Python package at `../eggcalc`. They spawn both MCP servers and compare JSON output strictly. As of 2026-07-06, the parity suite has 56 known failures (out of 413 tests) — see `docs/parity.md` `Verification status` and `Known parity gaps` for the breakdown (test-harness audience bug, tool/output drift, a 3-tool gap: `config_file_inspect`, `dependency_edit_preflight`, `repo_manifest_inspect`, and 3 concurrent-ordering failures in multi-request sessions). The Rust `full` profile ships 71 tools; Python defines 67. Do not treat these as regressions — they accumulated across the phase 06–09 line of work and are tracked for follow-up.
- **`mask_secret_preview()` in `src/tools/helpers.rs`** is a UTF-8-safe masking helper that operates on `.chars()` boundaries, never splitting multibyte sequences. Used by `config_file_inspect` and other tools that display secret values in findings. The old byte-slicing code was replaced with this helper to avoid panics on multi-byte Unicode input.
- **`deny.toml` configures `cargo-deny`** for license/advisory/ban/source checks. Run `cargo deny check` locally. Allowed licenses: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, Unlicense, Unicode-DFS-2016, Unicode-3.0, Zlib.
- **CI mirrors release gates.** GitHub Actions runs fmt, clippy, tests, generated-docs check, and `cargo package`.
- **`Cargo.lock` is gitignored** but present. This is unusual for a binary crate — don't commit it.
- **`serde_json` uses `preserve_order`** feature — key order is intentional in serialized JSON.
- **Env vars:** `EGGCALC_NO_CONFIG=1` (set in main.rs), `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_SCHEMA_DETAIL`.
- **Platform support**: `command_preflight` recognizes `platform` values `posix`, `windows`, and `auto`. Only `posix` is implemented; `windows` returns `UNSUPPORTED_FEATURE` and `auto` resolves to `posix`.
- **Input limits:** MAX_TEXT_LENGTH=100k, MAX_EXPRESSION_LENGTH=10k, MAX_LIST_ITEMS=10k, MAX_REGEX_SAMPLES=100, MAX_PATTERN_LENGTH=1k, MAX_REQUEST_BYTES=1M, MAX_OUTPUT_BYTES=1M.
- **`--diagnostics` CLI flag** prints version, tool count, profile summary, budget tiers, and env var names (no values). Supports `--format json`. `runtime_diagnostics` MCP tool exposes similar info to harness-only audiences.
- **`cargo run --bin verify-eggsact`** runs a 5-step verification pipeline (fmt, clippy, test, build, package) with optional parity check, and reports results as markdown.
