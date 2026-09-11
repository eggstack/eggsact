# AGENTS.md

Single-crate Rust (`eggsact`): deterministic MCP server + in-process tools. 86 tools across 23 categories. No workspace. `Cargo.lock` tracked — always pass `--locked`.

## Merge gate (in order)

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

Notes:
- `--test-threads=4` is required for integration tests (Tokio blocking-pool starvation), not a product budget. `--lib`/doc tests don't need it.
- Parity is excluded from CI (Python `eggcalc` not in CI). Full local gate: `scripts/release-check.sh` (requires clean tree + `cargo-deny`; never publishes/tags).

Focused runs:

```bash
cargo test --locked --test lib <filter>  # e.g. text, mcp, calc, property
cargo build && cargo test --locked --test lib parity  # requires eggcalc at ../eggcalc
```

Parity has 37 accepted failures (C1–C6) in `tests/fixtures/accepted_parity_failures.txt` / `docs/parity.md`. Only failures NOT in that list are regressions.

## Structure

- `src/main.rs` → CLI; `--mcp` → `mcp/server.rs` stdio loop; expression args → `calc/`
- `src/lib.rs` → `run()`/`evaluate()` re-exports
- `src/mcp/specs/` + `src/mcp/schemas/` → `ToolSpec` declarations + JSON schemas (23 files each); aggregated in `mcp/registry/all_tools.rs`
- `src/tools/` → JSON adapters (parse input, call core/service, build response once). Never call one handler from another.
- `src/services/` → typed composites (`RepoFacts`, `PatchAnalysis`, `SecurityInspection`, `FingerprintFacts`, `NewlineFacts`). Call these, not sibling handlers.
- `src/text/` → leaf deterministic cores; `src/calc/` → math; `src/agent/` → `ToolRegistry`; `src/preflight/` → typed wrappers; `src/temporal/` → fixed-offset datetime/cron.
- Start at `architecture/overview.md`; full API hierarchy in `docs/library-api.md`.

## Adding / changing a tool

- One `ToolSpec` in `src/mcp/specs/<category>.rs` is the single source of truth. Test `tool_registration_tables_are_in_sync` catches drift.
- After any registry/profile/exposure change: `cargo run --features dev-tools --bin generate-docs` (CI checks with `-- --check`).
- Never hand-edit: `src/text/confusables_generated.rs` (from `scripts/generate_confusables.py`, pinned Unicode 17.0.0 + SHA), `generated/tool-cards.md`, profile block in `architecture/mcp-server.md`, registry block in `architecture/overview.md`.
- New Rust code: `calc`/root → typed `text` → `agent::ToolRegistry` → typed `preflight` → MCP server. Raw `tools::*`, `services::*` internals, and `mcp` sub-modules beyond `server` are `pub` for 1.x compat, not the import surface.

## Gotchas

- Calculator: `^` is XOR, `**` is power. `g` means gram; use `gravity`/`standardgravity` for standard gravity.
- MCP responses are concurrent — correlate by JSON-RPC `id`, not arrival order.
- One stdio process = one protocol era pinned by first message (`initialize`/claim-less → legacy; enveloped claim → `2026-07-28`). Legacy needs `initialize` → `notifications/initialized` first. See `architecture/mcp-server.md`.
- No per-call profile: `tools/call` takes no `profile`; server-wide `EGGCALC_MCP_PROFILE` applies. `Profile::from_str_opt` is strict (`None` on unknown); use `Profile::custom(name)` for custom. Model-facing code uses `available_tools_model_safe()` (`Model` excludes `HarnessOnly`+`Hidden`).
- `Discovery` surface (`EGGSACT_MCP_SURFACE=discovery`) is presentation-only; search/invoke enforce the same profile/audience rules. Facades aren't `ToolSpec`s. If touching it, update `tests/mcp/test_discovery.rs`.
- Context APIs: `call_json_with_execution_context()` clones `eval_ctx` (mutations don't persist). Use `evaluate_with_context()`/`run_with_context()` for calculator state. `..._context_mut` is deprecated; `with_current_eval_context()` for closure scope. Re-entrant mutable access panics.
- Regex is NOT PCRE2: `compile_regex()` (`src/text/regex_engine.rs`) picks `regex` vs `fancy-regex`; outputs report `engine_used`.
- YAML is heuristic-only: `config_file_inspect` has no YAML parser (`parse_ok` true if non-empty, naive `key: value` scan, `analysis_mode: "heuristic"`). Never treat as syntax validity; `config_preflight` excludes YAML.
- Deterministic utils (`ip/cidr/codec/radix/datetime/cron`) use explicit inputs only — no clock, TZ db, env, net. Cron is Vixie/Cronie star syntax: if either DOM/DOW starts with `*` (incl. `*/n`) both must match, else either may; bare `*` is wildcard, explicit full ranges are not.
- `serde_json` has `preserve_order` — key order is intentional.
- Limits: text 100k, expr 10k, list 10k, regex samples 100, pattern 1k, request/output 1M each. Check `limits_applied`; truncation is automatic.
- Env: `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_AUDIENCE` (`Model` default, case-insensitive), `EGGCALC_MCP_SCHEMA_DETAIL` (`compact`/`normal`/`full`), `EGGSACT_MCP_SURFACE` (`direct`/`discovery`). `EGGCALC_NO_CONFIG=1` is for Python-`eggcalc` callers.
- `eggsact update` / `eggsact integrate list|detect|<client>` are verified/read-only; they never install a daemon or edit client config.

## Skills

`.opencode/skills/` (symlinked to `.agents/skills/`): `mcp-tools`, `testing`, `debugging`, `release`, `text-processing`. Load the matching skill before those tasks.
