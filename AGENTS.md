# AGENTS.md

Deterministic MCP and in-process utility tools for coding agents. Single crate, no workspace. 80 tools across 20 categories: math, text, JSON, regex, path, shell, config, patch, dependency, analysis, and more.

## Commands

```bash
cargo build                          # debug build
cargo build --release                # release build
cargo test --locked                  # all tests (unit + integration + parity)
cargo test --locked --lib            # unit tests in src/ only
cargo test --locked --test lib mcp   # MCP tests only
cargo test --locked --test lib parity # parity tests only
cargo test --locked --test lib text  # text tests only
cargo test --locked --doc            # doc tests
cargo fmt --all -- --check            # format check
cargo clippy --locked --all-targets --all-features  # lint
cargo package --locked                # crates.io packaging dry run
cargo deny check advisories bans licenses sources  # supply-chain audit
cargo run --features dev-tools --bin generate-docs        # regenerate docs from ToolSpec registry
cargo run --features dev-tools --bin generate-docs -- --check  # verify generated docs are current (CI)
scripts/release-check.sh               # full local release gate (no publish, no tag); requires clean tree + cargo-deny
```

## Verification order

`cargo fmt --all -- --check` → `cargo clippy --locked --all-targets --all-features -- -D warnings` → `cargo test --locked --all-features --lib` → `cargo test --locked --all-features --bins` → `cargo test --locked --all-features -- --skip parity --test-threads=4` → `cargo test --locked --doc` → `cargo run --locked --features dev-tools --bin generate-docs -- --check` → `cargo deny check advisories bans licenses sources` → `cargo package --locked --list` → `cargo package --locked --verbose` → `cargo publish --locked --dry-run`

## CI

GitHub Actions CI runs on push/PR to `main` (plus manual `workflow_dispatch`):

**Linux correctness** (single job, one cache):
- `cargo fmt --all -- --check`
- `cargo run --locked --features dev-tools --bin generate-docs -- --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features -- --skip parity --test-threads=4`
- `cargo test --locked --doc`

**Supported-platform compilation** (matrix, scheduled/manual only):
- Windows: `cargo check --locked --all-targets --all-features`
- macOS: `cargo check --locked --all-targets --all-features`

MSRV, cargo-deny, parity, latest-compatible, and fuzz/sanitizer checks are scheduled/manual (not merge-blocking). See `docs/verification.md`.

Parity tests are excluded from CI because Python `eggcalc` is not available in the CI environment. Run parity locally with `cargo test --test lib parity`.

GitHub CI verifies merge correctness but does **not** publish to crates.io. The maintainer publishes manually per `docs/release.md`.

## Structure

```
src/
  main.rs           # CLI entry, arg parsing, dispatch
  lib.rs            # library root, re-exports run()/evaluate()
  calc/             # calculator: evaluator, normalize, units, context (4 modules)
  mcp/              # MCP server protocol, runtime, registry, validation
    server.rs       # protocol orchestration, stdio loop, dispatch
    registry/       # tool registration (ToolSpec declarations, single source of truth)
      types.rs      # ToolDefinition, ToolSpec, enums
      all_tools.rs  # ALL_TOOLS aggregation from specs/
    specs/          # ToolSpec declarations per tool category (20 files)
    schemas/        # JSON-schema builders per tool category (20 files)
  tools/            # MCP tool implementations (by category, 20 files)
    helpers.rs      # shared constants, utilities, helper functions
  text/             # text processing library (25 modules + generated confusables data file)
    regex_engine.rs # regex backend classifier
    confusables_generated.rs  # AUTO-GENERATED — never edit
  agent/            # in-process agent API (ToolRegistry, Profile, call_json)
  preflight/        # typed preflight wrappers (EditPreflight, CommandPreflight, etc.)
tests/
  lib.rs            # declares test modules: calc, mcp, parity, text, property
  parity/           # Python/Rust parity tests (requires ../eggcalc)
  property/         # property-based tests (10 modules, 55 tests)
architecture/       # detailed design docs (15 files) — see index below
plans/
  roadmap.md        # the single living plan; completed phase records were pruned (git history keeps them)
docs/releases/      # archived v1.2.0 evidence ledgers (historical only)
```

## Architecture docs index

Detailed design documentation lives in `architecture/`. Use these as the deep-reference for the gotchas below:

| Doc | Covers |
|-----|--------|
| `architecture/overview.md` | Start here: crate layout, module map, request flow |
| `architecture/machine-codes.md` | Full machine code table, finding helpers, verdict constants |
| `architecture/budget-concurrency.md` | SyncExecutionPool, truncation, budget checks |
| `architecture/mcp-server.md` | MCP lifecycle, concurrency, response ordering, generated profile reference block |
| `architecture/registry-profiles.md` | Profile definitions, audience model, exposure levels |
| `architecture/calculator.md` | Evaluator, normalization, units, context |
| `architecture/text-library.md` | Text module catalog and conventions |
| `architecture/preflight.md` | Typed preflight wrappers, composite tools |
| `architecture/agent-api.md` | ToolRegistry, in-process execution path |
| `architecture/testing.md` | Test structure, parity, property tests, fuzzing |
| `architecture/generated-assets.md` | What `generate-docs` writes, confusables generation, parity harness, diagnostics |
| `architecture/tools.md` | The 20 tool categories and their handlers |
| `architecture/cli-binaries.md` | CLI flags and binary behavior |
| `architecture/coding-agent-integration.md` | How coding agents should integrate eggsact |
| `architecture/compatibility.md` | CompatibilityMode (EggcalcPython vs StrictNative) semantics |

## Docs index

Hand-maintained user-facing docs in `docs/`:

| Doc | Covers |
|-----|--------|
| `docs/mcp-tools.md` | Full MCP tool reference and wire protocol |
| `docs/library-api.md` | Rust library API guide |
| `docs/math-features.md` | Math functions, constants, units |
| `docs/cli.md` | CLI usage |
| `docs/parity.md` | Python parity status and accepted differences |
| `docs/compatibility-policy.md` | Semver policy, machine-code/profile stability rules |
| `docs/release.md` | Canonical release checklist (manual publish) |
| `docs/verification.md` | Verification tiers and gates |
| `docs/contributing.md` | Contribution workflow |
| `docs/msrv.md`, `docs/fuzzing.md` | MSRV policy; fuzz corpus/triage policy |

## Key gotchas

- **`^` is XOR, not exponentiation.** Use `**` for power. Matches Python.
- **`g` means gram** in unit expressions. Use `gravity` or `standardgravity` for standard gravity.
- **Never edit `src/text/confusables_generated.rs`** — auto-generated sorted static table by `scripts/generate_confusables.py`. Edit the script, not the output. The generator pins the expected Unicode Security version and SHA-256 checksum; regenerating with different source data fails loudly. Use `lookup()` for single-character lookups.
- **Confusables source pin:** regeneration uses the official version-specific Unicode 17.0.0 source at `https://www.unicode.org/Public/17.0.0/security/confusables.txt`, verifies the pinned SHA-256 and header before writing, and is not part of ordinary CI or `scripts/release-check.sh`.
- **Bounded JSONL reader:** `read_bounded_line()` counts every byte before LF, discounts only a final CR for CRLF, retains at most `MAX_REQUEST_BYTES`, and drains through exactly LF with `fill_buf()`/`consume()` so following frames remain intact.
- **Windows drive-relative diagnostics:** `path_scope_check()` conservatively rejects `D:foo`-style targets and reports the actual target text/drive; it does not model per-drive current directories.
- **Never hand-edit generated assets** — the profile reference block in `architecture/mcp-server.md` and `generated/tool-cards.md` are produced by `cargo run --features dev-tools --bin generate-docs`; `src/text/confusables_generated.rs` is produced by `scripts/generate_confusables.py`. Edit `ToolSpec` entries in `src/mcp/specs/` (or the generator script) instead. README and all `docs/*.md` are hand-maintained. See `architecture/generated-assets.md`.
- **Adding an MCP tool requires one `ToolSpec` entry** in `src/mcp/specs/<category>.rs`. This is the single source of truth. A test (`tool_registration_tables_are_in_sync`) catches drift.
- **Parity tests require `eggcalc`** Python package at `../eggcalc`. See `docs/parity.md` for 37 known failures (C1–C6), enumerated in `tests/fixtures/accepted_parity_failures.txt`. Any parity failure NOT in that list is an unexpected regression. Do not treat listed failures as regressions.
- **`Cargo.lock` is tracked** because eggsact ships binaries. CI uses `--locked` for reproducible builds.
- **`serde_json` uses `preserve_order`** — key order is intentional in serialized JSON.
- **Regex backend auto-selection**: `regex_finditer` and `validate_regex` use `compile_regex()` in `src/text/regex_engine.rs` to pick between Rust `regex` (fast, linear-time) and `fancy-regex` (lookaround/backreferences). Outputs report `engine_used` and `unsupported_features`. This is NOT PCRE2.
- **Context-aware vs legacy APIs**: `call_json_with_execution_context()` clones `eval_ctx` — mutations do **not** persist back. Use `evaluate_with_context()`/`run_with_context()` for calculator state. **`call_json_with_execution_context_mut` is `#[deprecated(since = "1.0.0")]`**. Use `with_current_eval_context()` for closure-scoped thread-local access. Re-entrant mutable access panics via an exclusive-access guard.
- **Response truncation is automatic**: `truncate_response()` caps findings/output when a tool exceeds its budget. Check `limits_applied` in the response envelope. See `architecture/budget-concurrency.md`.
- **MCP response ordering is concurrent**: Responses may arrive out of request order. **Correlate by JSON-RPC `id`**, not arrival position. See `architecture/mcp-server.md`.
- **Sync execution pool for budget-aware APIs**: `call_json_with_budget`, `call_json_with_context`, and `call_json_with_execution_context` route through `SyncExecutionPool` (8 workers, 32-slot queue). Queue saturation returns `RESOURCE_EXHAUSTED`. `call_json` remains direct (no pool). The MCP server path uses Tokio `spawn_blocking`. See `architecture/budget-concurrency.md`.
- **MCP lifecycle required**: The server requires `initialize` → `notifications/initialized` before `tools/list`, `tools/call`, `profiles/list`. Methods before initialization return `-32600` with `NOT_INITIALIZED` data code. Ping is always allowed. See `architecture/mcp-server.md`.
- **`ToolDefinition`** lives in `src/mcp/registry/types.rs` (not `server.rs`).
- **`ToolAudience`** enum (`Model`, `Harness`, `Debug`) controls exposure. Use `available_tools_model_safe()` for model-facing integrations.
- **`Profile::from_str_opt`** is strict — returns `None` for unknown names. Use `Profile::custom(name)` for custom profiles.
- **Env vars:** `EGGCALC_NO_CONFIG=1` (set in main.rs), `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_AUDIENCE` (case-insensitive, defaults to `Model`), `EGGCALC_MCP_SCHEMA_DETAIL` (`compact`/`normal`/`full`; defaults to `full`).
- **Input limits:** MAX_TEXT_LENGTH=100k, MAX_EXPRESSION_LENGTH=10k, MAX_LIST_ITEMS=10k, MAX_REGEX_SAMPLES=100, MAX_PATTERN_LENGTH=1k, MAX_REQUEST_BYTES=1M, MAX_OUTPUT_BYTES=1M.
- **Test-thread bound:** `--test-threads=4` is used in CI and the release gate to prevent Tokio blocking-pool starvation when many MCP subprocess tests run in parallel. This is a test-runner containment measure, not a product budget. Unit tests (`--lib`) and doc tests do not need it.

## Exposure & Audience Model

Tools have typed `ToolExposure` and `ToolListAudience` enums in `src/mcp/registry/types.rs` and `src/mcp/registry/listing.rs`:

- **Exposure**: `Default`, `Contextual`, `ExpertOnly`, `HarnessOnly`, `Hidden`
- **Audience**: `Model` (excludes HarnessOnly+Hidden), `Harness` (excludes Hidden), `Debug` (all non-hidden)

**No per-call profile override**: `tools/call` intentionally does NOT accept a `profile` parameter. The active profile is set once at server startup via `EGGCALC_MCP_PROFILE` and applies to all `tools/call` requests.

See `architecture/registry-profiles.md` for profile definitions and the audience model.

## Skills

Agent task skills in `.opencode/skills/` (symlinked from `.agents/skills/` for Codex compatibility):

- `.opencode/skills/mcp-tools/SKILL.md` — how to add or update MCP tools
- `.opencode/skills/testing/SKILL.md` — testing patterns, commands, test structure
- `.opencode/skills/debugging/SKILL.md` — common issues, debugging workflows
- `.opencode/skills/release/SKILL.md` — release process and checklist
- `.opencode/skills/text-processing/SKILL.md` — text module conventions and patterns

## Fuzzing

12 fuzz targets via `cargo-fuzz` + libFuzzer in `fuzz/`. Requires nightly Rust.

```bash
cargo install cargo-fuzz --locked
cargo fuzz build
cargo fuzz run calculator_expression -- -max_total_time=60 -timeout=5
```

Property tests run in ordinary CI: `cargo test --locked --all-features property`

See `docs/fuzzing.md` for corpus policy, crash triage, and regression promotion workflow.
