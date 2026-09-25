# eggsact Long-Term Specification

Status: canonical long-term implementation directive

Companion documents:

- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

This document defines the intended end state for eggsact. It is deliberately
broader than an implementation plan: it establishes product scope, architectural
ownership, determinism properties, compatibility requirements, and acceptance
criteria. The roadmap decomposes this specification into ordered execution
phases. The terminology document is normative whenever older code, docs, or
plans use overlapping terms such as tool, registry, profile, audience, surface,
preflight, service, or context.

The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

## 1. Product definition

eggsact is a deterministic local utility layer for coding agents: a CLI
calculator/utility binary, an MCP stdio server exposing curated tools, and an
in-process Rust library for harnesses. Keep it lightweight, bounded, local,
and exact-input/exact-output. MCP is a transport adapter over the deterministic
tool substrate, not a reason to accumulate unrelated agent features.

The same substrate MUST support three consumption forms without creating
separate products:

```text
CLI binary
    eggsact <expression> / eggsact <subcommand> -> calc/ + text/ + services

MCP stdio server
    eggsact --mcp -> mcp/server.rs stdio loop -> ToolSpec registry

In-process library
    run() / evaluate() + agent::ToolRegistry + preflight::* harnesses
```

## 2. Primary product goals

eggsact MUST provide:

1. A deterministic calculator (`calc/`) with units, constants, and bounded
   evaluation semantics (`^` is XOR, `**` is power).
2. A single-crate Rust implementation with a single-source `ToolSpec`
   registry (86 tools across 23 categories at baseline).
3. A typed composition hierarchy: `calc`/root -> typed `text` ->
   `agent::ToolRegistry`/execution contexts -> typed `preflight` -> MCP
   server. Raw `tools::*` adapters stay `pub` for 1.x compatibility but are
   adapter internals, not the import surface.
4. Profile/audience/exposure filtering (`full`, `default`, `codegg_*`,
   `human_math`; `Model` excludes `HarnessOnly`+`Hidden`).
5. Stable machine codes, structured findings/verdicts, bounded execution,
   cooperative cancellation, truncation, and concurrent MCP stdio dispatch.
6. Fixed-offset temporal utilities and Vixie/Cronie star-syntax cron with no
   clock, TZ database, environment, or network dependence.
7. Generated documentation, property tests, fuzz targets, MSRV/cargo-deny
   policy, and a manual release gate.
8. A client-owned stdio server with no daemon, no PID files, no restart
   command, and no background supervision.

## 3. Non-goals

eggsact is not:

- a general sandbox: classify risk, do not enforce it;
- a DevUtils feature-parity dump: admit specification-heavy exact
  operations, not generic utilities;
- a stateful agent runtime: calculator `ExecutionContext` isolation
  semantics stay unchanged unless a real consumer justifies a session;
- a network service: no HTTP in the library/MCP API, no retries, no
  telemetry, no persistent user tracking;
- a YAML platform: `config_file_inspect` stays heuristic-only for YAML
  (`analysis_mode: "heuristic"`); `config_preflight` excludes YAML;
- a PCRE2 engine: `compile_regex()` picks `regex` vs `fancy-regex` and
  reports `engine_used`;
- an automatic publisher: no automatic crates.io publishing or tag creation
  in GitHub Actions;
- a package-manager backend: no apt/deb/rpm, Homebrew, winget, Chocolatey,
  MSI, container distribution, code-signing/notarization infrastructure, or
  Windows ARM64 release without a separate qualification decision.

The self-update exception (`eggsact update`) is binary-only application
functionality in `src/update.rs` behind in-process `eggfetch-core`
(`http1,tls-rustls,tls-native-roots,proxy`; HTTP/1 only, strict
HTTPS-downgrade rejection, explicit env proxy, 10s connect / 120s total,
request-local decoded-body caps on small bodies, streamed binaries, no
external `curl` after install). Bootstrap `packaging/install.*` still uses
external download tooling.

## 4. Architectural principles

### 4.1 One crate, one registry

The crate stays single-crate with no workspace split. One `ToolSpec` in
`src/mcp/specs/<category>.rs` is the single source of truth. Test
`tool_registration_tables_are_in_sync` catches drift.

### 4.2 Typed-first composition

`src/services/` typed composites (`RepoFacts`, `PatchAnalysis`,
`SecurityInspection`, `FingerprintFacts`, `NewlineFacts`) sit over
`text/`/`calc` cores. `tools/*` adapters parse input, call typed
cores/services, and build the wire shape once at the boundary. No
adapter-to-adapter JSON composition except the intentional same-module
reuses documented in `architecture/tools.md`. Never call one handler from
another.

### 4.3 Determinism with explicit inputs

Deterministic utils (`ip/cidr/codec/radix/datetime/cron`) use explicit
inputs only -- no clock, TZ db, env, or net. Cron follows Vixie/Cronie
star semantics: if either DOM/DOW starts with `*` (incl. `*/n`) both must
match, else either may; bare `*` is wildcard, explicit full ranges are not.

### 4.4 Bounded execution

Limits are contractual: text 100k, expr 10k, list 10k, regex samples 100,
pattern 1k, request/output 1M each. Check `limits_applied`; truncation is
automatic. `serde_json` uses `preserve_order` -- key order is intentional.

### 4.5 Presentation is not capability

`Discovery` surface (`EGGSACT_MCP_SURFACE=discovery`) is presentation-only;
search/invoke enforce the same profile/audience rules. Facades are not
`ToolSpec`s. Direct remains the 1.x default unless evidence-backed rollout
gates pass.

## 5. Compatibility requirements

- The original 80-tool registration order remains an exact prefix; later
  utilities are full-profile only.
- `json_query` stays deprecated-but-compatible in 1.x with no promotion in
  default docs.
- Calculator context isolation semantics are unchanged; `..._context_mut`
  is deprecated; re-entrant mutable access panics.
- One stdio process pins one protocol era by first message
  (`initialize`/claim-less -> legacy; enveloped claim -> `2026-07-28`).
  Legacy needs `initialize` -> `notifications/initialized` first.
- No per-call profile: `tools/call` takes no `profile`; server-wide
  `EGGCALC_MCP_PROFILE` applies. `Profile::from_str_opt` is strict (`None`
  on unknown); use `Profile::custom(name)` for custom. Model-facing code
  uses `available_tools_model_safe()`.
- `serde_json` key order, machine codes, profile/audience policy,
  legacy/modern protocol behavior, discovery ranking, deterministic
  outputs, and bounded execution semantics are preserved across
  performance work. Generated docs stay unchanged when no `ToolSpec`
  metadata changes.

## 6. Distribution and release requirements

- Five qualified release targets: Linux x86-64 / AArch64, macOS Intel /
  Apple Silicon, Windows x86-64. Stripped standalone binaries only.
- Exact-tag Unix installer verification; `shasum -a 256` fallback where
  `sha256sum` is unavailable.
- `scripts/check-release-contract.py` guards the release invariant,
  including the `src/update.rs` no-`curl` rule and `eggfetch-core`
  presence.
- `eggsact update` / `eggsact integrate list|detect|<client>` are
  verified/read-only; they never install a daemon or edit client config.
- Performance evidence is maintainer-run and non-gating
  (`cargo bench --locked --bench performance`, `EGGSACT_BENCH_SHA`
  labeling, same host/toolchain comparison, never host-specific timing
  thresholds in ordinary tests). See `architecture/performance.md`.

## 7. System invariants

The implementation MUST preserve:

1. Single crate; `Cargo.lock` tracked; `--locked` always.
2. One `ToolSpec` per tool; registry sync test green.
3. Typed hierarchy (`calc`/root -> `text` -> `ToolRegistry`/contexts ->
   `preflight` -> MCP server); no adapter-to-adapter calls outside the
   three documented same-module reuses.
4. Profile/audience as the authorization boundary;
   `available_tools_model_safe()` for model-facing paths.
5. Deterministic exact-input/exact-output behavior; no clock/TZ/env/net in
   deterministic utils.
6. Bounded limits with `limits_applied` reporting; automatic truncation.
7. One-era-per-stdio-connection; cross-era requests use `-32022`;
   mismatched notifications dropped before side effects.
8. Context isolation: `call_json_with_execution_context()` clones
   `eval_ctx`; `evaluate_with_context()`/`run_with_context()` for
   calculator state.
9. No HTTP in library/MCP API; updater transport confined to
   `src/update.rs` with the pinned eggfetch feature/trust profile.
10. Generated docs in sync (`generate-docs --check` green); never hand-edit
    `src/text/confusables_generated.rs`, `generated/tool-cards.md`, the
    profile block in `architecture/mcp-server.md`, or the registry block
    in `architecture/overview.md`.

## 8. Completion criteria

This directive is complete when:

- a coding agent can use the CLI, MCP stdio server, and in-process
  library for the same deterministic capabilities with identical outputs;
- all 86 tools honor profile/audience/exposure filtering with stable
  machine codes and bounded execution;
- discovery remains a smaller, searchable, protocol-current presentation
  with evidence-backed rollout gates satisfied before any default change;
- binary distribution covers all five qualified targets with installer
  verification and a self-contained updater (bootstrap installers excepted);
- the manual release gate (fmt, generate-docs check, clippy, tests,
  doc tests, release contract, cargo-deny, MSRV) is reproducible.
