# AGENTS.md

Rust reimplementation of Python `eggcalc`. Natural language math calculator + MCP server (64 tools). Single crate, no workspace.

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
cargo fmt --check                    # format check
cargo clippy --all-targets --all-features  # lint
cargo package                        # crates.io packaging dry run
cargo run --bin generate-docs        # regenerate docs from ToolSpec registry
cargo run --bin generate-docs -- --check  # verify generated docs are current (CI)
./release.sh                         # full pipeline: regenerate data, fmt, clippy, test, release build, package
```

## CI

GitHub Actions CI runs on push/PR to `main`:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo run --bin generate-docs -- --check` (generated docs freshness)
- `cargo package --verbose` (after all checks pass)

## Verification order

`cargo fmt --check` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo test --verbose` → `cargo package --verbose`

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
      ...           # one file per category (16 total)
    protocol.rs     # JSON-RPC types (Request, Response, Error, InitializeResult)
    response.rs     # ToolResponse, error sanitization, finding() helpers, with_verdict, preflight builders
    machine_codes.rs # machine-readable response codes, severity/disposition/verdict constants
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
  preflight/        # typed preflight wrappers (ConfigPreflight, CommandPreflight, EditPreflight), strict finding parsing, structured RecommendedNextTool
tests/
  lib.rs            # declares test modules: calc, mcp, parity, text
  calc/             # calculator tests (4 files)
  mcp/              # MCP protocol + tool tests (18 files)
  parity/           # Python/Rust parity tests (12 files)
  text/             # text processing tests (24 files)
scripts/
  generate_confusables.py  # regenerates src/text/confusables_generated.rs from unicode.org
generated/
  tool-cards.md    # generated compact tool cards per codegg profile
```

## Architecture docs

Detailed architecture documentation is in `architecture/`:

- `architecture/overview.md` — directory layout, dependency flow, constants
- `architecture/calculator.md` — calculator core, NL pipeline, units, constants
- `architecture/mcp-server.md` — MCP protocol, tool registration, categories, error handling
- `architecture/machine-codes.md` — machine-readable response codes, finding helpers, severity/disposition/verdict constants, composite tool verdicts
- `architecture/text-library.md` — all 24 text modules, public API, code patterns
- `architecture/compatibility.md` — compatibility mode (EggcalcPython vs StrictNative), behavior differences

## Agent API

`src/agent/` provides an in-process API for calling tools without MCP. `ToolRegistry` wraps the tool registry with profile filtering and `call_json()` dispatch. `src/preflight/` adds typed wrappers (`ConfigPreflight`, `CommandPreflight`, `EditPreflight`) that parse tool responses into structured Rust types with fail-closed contract enforcement.

- **`PreflightError`** has three variants: `ToolCall` (registry rejected), `ToolRejected` (tool returned `ok: false`), `ContractViolation` (missing mandatory field in `ok: true` response). Missing fields are hard failures, not silent defaults.
- **Typed verdict enums**: `EditVerdict`, `CommandVerdict`, `ConfigVerdict` with `Other(String)` variant for forward compatibility. `FindingSeverity` and `FindingDisposition` follow the same pattern.
- **`RecommendedNextTool`** struct: `{ name: String, reason: Option<String>, arguments_hint: Option<Value> }`. Parsed from both string and object shapes; fails closed on malformed values.
- **Strict finding parsing**: `Finding::try_from_value_strict()` and `Finding::from_array_strict()` require `code`, `severity`, and `message` strings. Used by all typed preflight wrappers. Permissive `Finding::from_value()` / `Finding::from_array()` preserved for backward compatibility.
- **`parse_response()`** is public on each wrapper for testing contract parsing without a full registry call.
- **`EditPreflightInput`** accepts optional `file_path`/`workspace_root` (triggers `path_scope_check`), `newline_policy` (triggers `text_fingerprint` newline detection), `unicode_policy` (triggers `text_security_inspect`), `expected_fingerprint` (triggers `text_fingerprint` SHA-256 comparison), and `edit_metadata` (passthrough). All sub-tool results appear in `subresults` and structured output fields.

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

- **`server.rs` was cleaned up**: schema compaction, response wrapping, and profile resolution moved to their respective modules (`registry/listing.rs`, `response.rs`, `runtime.rs`). The stdio read loop remains in `server.rs` but delegates most work.

- **Never edit `src/text/confusables_generated.rs`** — auto-generated by `scripts/generate_confusables.py`. Edit the script, not the output.
- **Never hand-edit generated docs** — README tool tables, architecture profile reference, and `generated/tool-cards.md` are generated by `cargo run --bin generate-docs`. Edit `ToolSpec` entries in `src/mcp/specs/` instead, then re-run the generator.
- **Adding an MCP tool requires one `ToolSpec` entry** in `src/mcp/specs/<category>.rs`. This is the single source of truth for tool registration. A test (`tool_registration_tables_are_in_sync`) will catch drift.
- **`^` is XOR, not exponentiation.** Use `**` for power. This matches Python behavior.
- **`g` means gram** in unit expressions. Use `gravity` or `standardgravity` for standard gravity.
- **Parity tests require `eggcalc`** Python package at `../eggcalc`. They spawn both MCP servers and compare JSON output strictly. They won't pass without the Python project present.
- **CI mirrors release gates.** GitHub Actions runs fmt, clippy, build, tests, and `cargo package`.
- **`Cargo.lock` is gitignored** but present. This is unusual for a binary crate — don't commit it.
- **`serde_json` uses `preserve_order`** feature — key order is intentional in serialized JSON.
- **Env vars:** `EGGCALC_NO_CONFIG=1` (set in main.rs), `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_SCHEMA_DETAIL`.
- **Input limits:** MAX_TEXT_LENGTH=100k, MAX_EXPRESSION_LENGTH=10k, MAX_LIST_ITEMS=10k, MAX_REGEX_SAMPLES=100, MAX_PATTERN_LENGTH=1k, MAX_REQUEST_BYTES=1M, MAX_OUTPUT_BYTES=1M.
