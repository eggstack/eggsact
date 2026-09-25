# eggsact Canonical Terminology and Domain Model

Status: normative companion to `plans/000-long-term-specification.md`

This document defines the language eggsact implementation plans, protocol
types, storage schemas, architecture documents, tests, user labels, and
operator documentation MUST use. When current code or docs use a term
differently, the compatibility mapping below describes the migration target.

## 1. Naming rules

1. A durable capability MUST have one `ToolSpec` declaration, not several
   overlapping names.
2. A presentation facade is not a tool identity.
3. A typed core, a JSON adapter, a composite service, a registry entry, and
   a preflight wrapper are distinct objects.
4. Terms MUST NOT be used as interchangeable shorthand when they cross the
   adapter/service/core, profile/audience, or direct/discovery boundaries.
5. Compatibility shims MAY remain during 1.x but MUST be labeled as
   compatibility projections.

## 2. Top-level relationships

```text
ToolSpec (src/mcp/specs/<category>.rs)
  -> JSON schema (src/mcp/schemas/<category>.rs)
  -> registry aggregation (mcp/registry/all_tools.rs)
  -> ToolRegistry entry (agent/)
  -> typed preflight wrapper (preflight/)
  -> MCP server exposure (mcp/server.rs)

text/ + calc/ leaf cores
  -> services/ typed composites
  -> tools/ JSON adapters (parse input, call core/service, build response once)
  -> ToolRegistry / ExecutionContext / preflight -> MCP transport
```

The principal runtime relationship is:

```text
ToolSpec
  -> ToolRegistry
  -> ExecutionContext
  -> preflight wrapper
  -> MCP server (direct | discovery surface)

Evaluation
  -> normalize -> tokens -> AST -> evaluation (calc/)
```

## 3. Capability terms

### Tool

One deterministic capability with a canonical name, `ToolSpec`
declaration, JSON schema, profile exposure, audience, machine code, and
bounded execution semantics. Baseline: 86 tools across 23 categories.

### ToolSpec

The single source of truth for one tool in
`src/mcp/specs/<category>.rs`. Aggregated in `mcp/registry/all_tools.rs`.
Test `tool_registration_tables_are_in_sync` catches drift.

### Facade

An MCP-only deterministic presentation helper (`tool_search` /
`tool_invoke`) plus five pinned front doors under the discovery surface.
Facades are not `ToolSpec`s and MUST NOT be counted as tools.

### Category

One of the 23 registry groupings (`analysis`, `cargo`, `config`,
`dependency`, `diagnostics`, `encoding`, `identifier`, `json`, `list`,
`markdown`, `math`, `network`, `patch`, `path`, `regex`, `repo`, `shell`,
`temporal`, `text`, `toml`, `unicode`, `validation`, `version`).

### Machine code

A stable structured identifier for a finding/verdict class. Used by
typed preflight wrappers and structured responses.

## 4. Composition terms

### Leaf core

Deterministic logic in `src/text/` or `src/calc/`. No JSON transport
concerns. The preferred dependency target for new Rust code.

### Composite service

Typed logic in `src/services/` over cores: `RepoFacts`,
`PatchAnalysis`, `SecurityInspection`, `FingerprintFacts`,
`NewlineFacts`. Call these, not sibling handlers.

### JSON adapter

Code in `src/tools/` that parses MCP/library JSON input, calls one
core/service, and builds the wire response once. Never call one handler
from another (except the three intentional same-module reuses documented
in `architecture/tools.md`).

### ToolRegistry

The in-process API in `src/agent/` mapping canonical tool names to
typed execution with profile/audience filtering and bounded budgets.

### Preflight wrapper

Typed harness wrapper in `src/preflight/` (e.g.
`DependencyPreflight`). The recommended harness import surface alongside
`calc`/root, `text`, `ToolRegistry`, and execution contexts.

### ExecutionContext

Immutable-by-default evaluation scope for calculator state and tool
budgets. `call_json_with_execution_context()` clones `eval_ctx`
(mutations do not persist). Use `evaluate_with_context()` /
`run_with_context()` for calculator state. `..._context_mut` is
deprecated. Re-entrant mutable access panics.

## 5. Exposure terms

### Profile

A server-wide named tool subset (`full`, `default`, `codegg_core_min`,
`codegg_core`, `codegg_preflight`, `codegg_patch`, `codegg_config`,
`codegg_unicode_security`, `codegg_shell`, `codegg_repo_audit`,
`human_math`). Selected once per server via `EGGCALC_MCP_PROFILE`.
`tools/call` takes no per-call profile. `Profile::from_str_opt` is
strict (`None` on unknown); use `Profile::custom(name)` for custom.

### Audience

`Model` (default, case-insensitive via `EGGCALC_MCP_AUDIENCE`) vs
`Harness`. Model-facing code uses `available_tools_model_safe()`
(`Model` excludes `HarnessOnly`+`Hidden`).

### Surface

`direct` (default) vs `discovery` (via `EGGSACT_MCP_SURFACE`).
Discovery is presentation-only; search/invoke enforce the same
profile/audience rules.

### Schema detail

`compact` / `normal` / `full` via `EGGCALC_MCP_SCHEMA_DETAIL`.

## 6. Protocol terms

### Protocol era

One-era-per-stdio-connection, pinned by the first message:
`initialize`/claim-less -> legacy; enveloped claim -> `2026-07-28`.
Legacy needs `initialize` -> `notifications/initialized` first. See
`architecture/mcp-server.md`.

### Structured content

Schema-conforming `structuredContent` reusing the existing registry and
bounded execution paths, plus standard Tool annotations and modern
cache/result metadata on the modern era path.

### Discovery evaluation

Deterministic measurement (`src/mcp/discovery_eval.rs`) of serialized
Tool definitions: byte ratio, registry-derived semantic coverage,
retrieval top-1/top-3/top-5, leak count, scenario corpus, and offline
trace scoring (`scripts/score-discovery-traces.py`). The scorer never
substitutes for recorded model/client traces.

## 7. Distribution terms

### Qualified target

One of the five release build targets: `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

### Self-update transport

Binary-only updater in `src/update.rs` behind `eggfetch-core` with the
pinned `http1,tls-rustls,tls-native-roots,proxy` profile. Strict
HTTPS-downgrade rejection, explicit env proxy, 10s connect / 120s total
through body EOF, request-local decoded-body caps on small bodies,
streamed binaries. No external `curl` after install. Bootstrap
`packaging/install.*` still uses external download tooling.

### Release contract

The invariant set guarded by `scripts/check-release-contract.py`,
including Zig bootstrap layout, `eggfetch-core` presence, and the
`src/update.rs` no-`curl` rule.

## 8. Verification terms

### Merge gate

The ordered local gate in `AGENTS.md`: fmt check, generate-docs check,
clippy `-D warnings`, non-parity tests with `--test-threads=4`, doc
tests. Parity excluded from CI; full local gate is
`scripts/release-check.sh` (requires clean tree + `cargo-deny`; never
publishes/tags).

### Parity baseline

37 accepted failures (C1-C6) in
`tests/fixtures/accepted_parity_failures.txt` / `docs/parity.md`. Only
failures NOT in that list are regressions. Requires `eggcalc` at
`../eggcalc`.

### Performance evidence

Maintainer-run, non-gating benchmark evidence
(`cargo bench --locked --bench performance`, `EGGSACT_BENCH_SHA`
labeling, same host/toolchain comparison, stable text output, never
host-specific timing thresholds in ordinary tests). See
`architecture/performance.md`.

## 9. Compatibility mapping from current code

### Current `tools::*` public items

Retain `pub` for 1.x compatibility but document as adapter internals.
New harness code MUST use `calc`/root -> typed `text` ->
`ToolRegistry`/execution contexts -> typed `preflight` -> MCP server.
See `docs/library-api.md` and `architecture/overview.md`.

### Current `services::*` and `mcp` sub-modules

`pub` for 1.x compatibility, not the import surface.

### Current path-derived or process-global state

Daemon-owned cwd assumptions MUST NOT appear; deterministic utils take
explicit inputs. `EGGCALC_NO_CONFIG=1` is for Python-`eggcalc` callers.

### Current `json_query`

Deprecated-but-compatible in 1.x; do not promote in default docs.

## 10. Prohibited ambiguous usage

The following SHOULD be removed from new design documents unless qualified:

- "the tool" when several `ToolSpec`s, facades, or preflight wrappers
  may exist;
- "registry" when the intended object is `ToolRegistry` vs
  `mcp/registry/all_tools.rs` vs the `ToolSpec` declarations;
- "profile" when the intended object is profile vs audience vs surface
  vs schema detail;
- "discovery" when the intended object is the presentation surface vs
  the deterministic evaluation harness vs recorded model traces;
- "context" when the intended object is calculator `ExecutionContext`
  vs MCP request context vs closure scope;
- "update" when the intended object is self-update transport vs
  bootstrap installer vs dependency bump;
- "parity" when the intended object is accepted-failure baseline vs a
  new regression;
- "performance" when the intended object is non-gating benchmark
  evidence vs merge-gate correctness.

## 11. Review checklist

Any implementation plan or architectural change SHOULD answer:

1. Which `ToolSpec`(s), facades, or preflight wrappers are involved?
2. Which layer owns the mutable state (core / service / adapter /
   registry / context / transport)?
3. Is the change profile-, audience-, or surface-scoped?
4. Is a facade being mistaken for capability?
5. Which limits/truncation apply and is `limits_applied` reported?
6. Which protocol era(s) are affected?
7. Which execution context semantics apply?
8. What happens on truncation, cancellation, oversize input, malformed
   regex, heuristic YAML, or cross-era requests?
9. What is generated (registry facts, tool cards, profile blocks) and
   was `generate-docs` re-run?
10. Which compatibility prefix, deprecation, or isolation semantic
    remains, and how is it retired?
