# eggsact Architecture Overview

**Single-crate Rust project. No workspace. 86 tools across 23 categories.**

eggsact is a deterministic MCP (Model Context Protocol) server and in-process utility
library for AI coding agents. It exposes 86 tools across 23 categories covering math
evaluation, text processing, JSON analysis, regex validation, path operations, Unicode
safety, shell command preflight, config inspection, patch analysis, dependency
management, source analysis, network literals, encodings, and fixed-offset time. It also
re-implements the Python `eggcalc` calculator as its `math` category.

Every tool is a **local deterministic computation**: no clock, no network, no timezone
database, no environment reads in the tool cores, and no mutation of the external
environment. That single property is what makes the whole system testable, diffable, and
safe to hand to a model.

This document is the **master index** for `architecture/`. It gives the bird's-eye view
and one overview paragraph per discrete component; each component links to its own
deep-dive document. Read this file to understand how the pieces fit together, then
follow a link to review a component in depth.

## How to read this directory

Three entry points, depending on what you need:

| If you want to… | Read |
|---|---|
| Understand the system as a whole | This file, top to bottom |
| Review one discrete component | The [Deep Dive Index](#deep-dive-index), then that document |
| Change or extend the tool surface | [Tool Registration Pattern](#tool-registration-pattern), then [tools.md](tools.md) and [registry-profiles.md](registry-profiles.md) |

Every deep-dive doc is standalone: it states its own contracts, names its own key files,
and lists its own review checklist. Read any one of them without reading the others.

## Generated Registry Facts

<!-- BEGIN GENERATED: registry facts -->
| Registry fact | Value |
|---|---:|
| Underlying tools | 86 |
| Full/Model direct tools | 77 |
| Full/Model discovery tools | 7 |

| Category | Count |
|---|---:|
| `analysis` | 4 |
| `cargo` | 1 |
| `config` | 3 |
| `dependency` | 1 |
| `diagnostics` | 3 |
| `encoding` | 2 |
| `identifier` | 3 |
| `json` | 6 |
| `list` | 3 |
| `markdown` | 2 |
| `math` | 4 |
| `network` | 2 |
| `patch` | 5 |
| `path` | 6 |
| `regex` | 3 |
| `repo` | 5 |
| `shell` | 4 |
| `temporal` | 2 |
| `text` | 18 |
| `toml` | 1 |
| `unicode` | 2 |
| `validation` | 4 |
| `version` | 2 |

| Profile | Model | Harness |
|---|---:|---:|
| `full` | 77 | 86 |
| `default` | 25 | 25 |
| `codegg_core_min` | 6 | 6 |
| `codegg_core` | 19 | 19 |
| `codegg_preflight` | 7 | 13 |
| `codegg_patch` | 10 | 12 |
| `codegg_config` | 14 | 14 |
| `codegg_unicode_security` | 6 | 8 |
| `codegg_shell` | 5 | 6 |
| `codegg_repo_audit` | 18 | 18 |
| `human_math` | 4 | 4 |

<!-- END GENERATED: registry facts -->


The tables below are generated from the `ToolSpec` declarations in
`src/mcp/specs/*.rs`. Never hand-edit anything between the `BEGIN/END GENERATED`
markers. After changing registry membership, profile exposure, or a discovery front
door, regenerate with:

```bash
cargo run --locked --features dev-tools --bin generate-docs
```

CI enforces freshness with `-- --check`. The same run also refreshes the profile
reference block in [mcp-server.md](mcp-server.md) and the tool cards in
`generated/tool-cards.md`.

---

## Bird's-Eye View

The system is a **six-layer stack** with three entry points. Everything funnels down
into pure deterministic cores, and the only place that builds a wire-shaped response is
the outermost tool-adapter layer.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ L6  ENTRY POINTS                                                          │
│     main.rs ─ CLI arg parsing, dispatch                                   │
│       ├─ expression args ─────────────► calc (L2)                         │
│       ├─ --mcp ───────────────────────► mcp::server (L4, stdio JSON-RPC)  │
│       ├─ --diagnostics │ integrate │ update                               │
│     lib.rs ─ library root + public re-exports                            │
│       ├─ agent::ToolRegistry (L5, in-process, no IPC)                     │
│       └─ preflight::*       (L6, typed wrappers)                         │
│     src/bin/generate_docs.rs ─ build-time doc generator (dev-tools only)  │
└──────────────────────────────────────────────────────────────────────────┘
                                     │
┌──────────────────────────────────────────────────────────────────────────┐
│ L5  IN-PROCESS DISPATCH                                                  │
│     agent/     ToolRegistry, Profile, ToolAudience, ExecutionContext,    │
│                prepare_tool_call() — the shared lookup/validate core     │
│     preflight/ 6 typed workflow wrappers (edit, command, config, patch,   │
│                text-security, dependency) with fail-closed errors        │
└──────────────────────────────────────────────────────────────────────────┘
                                     │
┌──────────────────────────────────────────────────────────────────────────┐
│ L4  MCP PRESENTATION (src/mcp/)                                          │
│     server.rs, protocol.rs, response.rs, runtime.rs, budget.rs,          │
│     execution.rs, sync_pool.rs, schema_validation.rs, compat.rs,         │
│     machine_codes.rs                                                     │
│     registry/ (types, all_tools, listing) + specs/ (23) + schemas/ (23)  │
│     discovery.rs, discovery_eval.rs                                      │
│     → JSON-RPC 2.0 over stdio, dual-era pinning, concurrent dispatch     │
└──────────────────────────────────────────────────────────────────────────┘
                                     │
┌──────────────────────────────────────────────────────────────────────────┐
│ L3  TOOL ADAPTERS (src/tools/)  — the ONLY JSON boundary                 │
│     22 handler modules (23 categories; the `toml` handler lives in       │
│     config.rs) + helpers.rs (shared limits and input helpers)            │
│     86 handlers · parse input → call a core/service → build ToolResponse  │
└──────────────────────────────────────────────────────────────────────────┘
                                     │
┌──────────────────────────────────────────────────────────────────────────┐
│ L2  TYPED SERVICES (src/services/)  — composite logic, no wire types      │
│     RepoFacts, PatchAnalysis, SecurityInspection, FingerprintFacts,      │
│     NewlineFacts                                                         │
└──────────────────────────────────────────────────────────────────────────┘
                                     │
┌──────────────────────────────────────────────────────────────────────────┐
│ L1  DETERMINISTIC CORES — pure, leaf, no agent/mcp/tools dependency      │
│     text/       27 modules (measure, diff, validate, transform, path,    │
│                 identifier, shell, markdown, patch, glob, toml, config,  │
│                 cargo, version, script, regex engine/safety, Unicode     │
│                 policy/tools/properties, confusables, prompt inspection)  │
│     calc/       normalize → evaluator → units, with EvalContext          │
│     temporal/   pub(crate) fixed-offset datetime + bounded cron search   │
└──────────────────────────────────────────────────────────────────────────┘
```

**The load-bearing rule:** the wire shape (`ToolResponse`, JSON envelopes, machine
codes, verdicts) is constructed exactly once, at L3. L1 and L2 know nothing about JSON,
profiles, or the registry. That is what lets the same logic serve both the MCP server
and the in-process Rust API without duplicating behavior.

---

## Discrete Components at a Glance

One paragraph per component: what it owns, how it connects, and where to read more.

### 1. Entry points and CLI — `src/main.rs`, `src/lib.rs`

The user-facing surface. `main.rs` parses args into a `CliCommand`: bare expression
arguments go to the calculator, `--mcp` starts the stdio server (with optional
`--mcp-surface direct|discovery`), `--diagnostics` renders runtime diagnostics (with
optional `--format json|text`), `update` performs the verified self-update, and
`integrate <client|list|detect>` renders MCP setup text. `lib.rs` is the library root
and the recommended downstream import surface: it re-exports `run`, `evaluate`,
`EvalContext`, and the four context-aware variants.

→ Deep dive: [cli-binaries.md](cli-binaries.md) (CLI and binaries),
[self-update.md](self-update.md) (`update`/`integrate` in depth),
[generated-assets.md](generated-assets.md) (`generate-docs`).

### 2. Self-update and integration — `src/update.rs`, `src/integrate.rs`

The only part of the crate that touches the network, and the only part that can
replace the running binary. `src/update.rs` (3,029 lines, the second-largest file in
the crate) implements a verified update: select a release via crates.io, fetch the
Eggpack release manifest, project the asset for the host triple, download, verify
SHA-256, validate the candidate by running it, and commit under a mutation lock with
backup and rollback. Transport is the Eggup stack (`eggup-core`/`eggup-eggfetch`/
`eggup-acquisition`/`eggup-eggpack` 0.1.2 over `eggfetch-core`): in-process HTTP/1 +
TLS, strict HTTPS-downgrade rejection, explicit env proxy, no retries, no external
`curl` after install.

`src/integrate.rs` is the read-only counterpart: it *renders* per-client MCP setup
text and never writes anything. Neither command installs a daemon or edits client
config, and neither adds HTTP to the library or MCP API.

→ Deep dive: [self-update.md](self-update.md).

### 3. Calculator core — `src/calc/`

Natural-language math. `normalize.rs` transforms English into math ("thirty miles per
hour in meters per sec"), `evaluator.rs` parses and evaluates it via recursive descent
with ~90 functions and 8 precedence levels, `units.rs` holds 150+ units with 500+
aliases plus physical constants, and `context.rs` carries the mutable per-call
`EvalContext` (PRNG, memory registers, user variables). Surfaced as the 4 `math` tools
and directly through `run()`/`evaluate()`.

Two traps worth knowing before you touch this: `^` is bitwise XOR and `**` is power;
and `g` means *gram*, not standard gravity (use `gravity`/`standardgravity`).

→ Deep dive: [calculator.md](calculator.md).

### 4. Text library — `src/text/`

The largest module by volume: 27 leaf modules, ~31k lines including two auto-generated
data files. Pure deterministic cores with no dependency on `agent`, `mcp`, `tools`, or
`preflight` — the one narrow exception is `text::diff`, which reads the cancellation
flag from `mcp::budget`. Contains the grapheme-aware primitives, diff/Levenshtein
routines, the validators (JSON/brackets/TOML/config), the transforms, the shell
tokenizer, path and glob handling, the Unicode policy engine, confusables and
prompt-injection detection, and the regex engine selector that chooses `regex` vs
`fancy-regex` per pattern.

→ Deep dive: [text-library.md](text-library.md).

### 5. Generated Unicode data — `src/text/confusables_generated.rs`, `src/text/unicode_properties_generated.rs`

Two large `include!`d data files, currently **Unicode 18.0.0**, with SHA-256 checksums
pinned in their headers and asserted by unit tests. `confusables_generated.rs` holds
6,712 confusable mappings. Both are produced by checked-in generators and must never be
hand-edited. (Note: `scripts/generate_confusables.py` still writes a hardcoded
`# Version: 17.0.0` header line while pinning 18.0.0 data — a cosmetic generator bug
tracked as drift, not a data problem.)

→ Deep dive: [generated-assets.md](generated-assets.md).

### 6. Temporal core — `src/temporal/` (`pub(crate)`)

Fixed-offset datetime conversion and a Vixie/Cronie-style cron parser with bounded
next-match search. Explicit inputs only: no clock, no timezone database, no env, no
network. Surfaced through exactly 2 tools. Its cron day-of-month/day-of-week rule is
subtle enough to be worth reading before you touch matching logic.

→ Deep dive: [temporal.md](temporal.md).

### 7. Typed services — `src/services/`

The composite layer that keeps reusable multi-step logic out of JSON adapters:
`RepoFacts` (canonical ecosystem/path/language facts), `PatchAnalysis` (single-parse
neutral diff facts), `SecurityInspection`, `FingerprintFacts`, `NewlineFacts`. These
never touch `ToolResponse`, the registry, profile/audience policy, or schema validation,
and they take a lightweight `should_stop` closure for cooperative cancellation instead
of an MCP `BudgetContext`.

→ Deep dive: [services.md](services.md).

### 8. Tool adapters — `src/tools/`

The JSON boundary for all 86 tools, spread across 22 handler modules. Each handler
parses and validates its input, calls typed cores/services (never a sibling handler),
and builds the `ToolResponse` once. `helpers.rs` holds the shared input limits and
parsing utilities. A machine-enforced layering test
(`adapter_layering_has_no_handler_to_handler_composition`, in
`tests/mcp/test_substrate_007.rs`) keeps composition inside the typed cores.

→ Deep dive: [tools.md](tools.md).

### 9. MCP server — `src/mcp/`

The presentation layer: JSON-RPC 2.0 over stdio, dual-era protocol pinning (legacy
`2025-11-25`/`2024-11-05` vs modern `2026-07-28`), concurrent dispatch through a
`JoinSet` with an `mpsc` writer, schema validation against a strict JSON Schema subset,
and Python-compatible JSON serialization. One stdio process pins exactly one protocol
era, decided by the first message.

→ Deep dive: [mcp-server.md](mcp-server.md).

### 10. Registry, profiles and discovery — `src/mcp/registry/`, `specs/`, `schemas/`, `discovery.rs`

One `ToolSpec` per tool is the single source of truth: 23 spec files and 23 schema
files aggregated into `ALL_TOOLS_VEC`, with a sync test that catches drift. Eleven named
profiles plus `ToolAudience` and `ToolExposure` control what `tools/list` returns and
what dispatch will accept. The optional `Discovery` surface (`tool_search`/`tool_invoke`
over pinned front doors) is presentation-only — search and invoke enforce the same
profile and audience rules as direct calls.

→ Deep dive: [registry-profiles.md](registry-profiles.md).

### 11. Budget and concurrency — `src/mcp/budget.rs`, `execution.rs`, `sync_pool.rs`, `runtime.rs`

Three enforceable budget tiers (`CHEAP`/`MODERATE`/`HEAVY`) with cooperative
cancellation, a 5-phase handler lifecycle executed on blocking threads with timeouts,
and a bounded synchronous worker pool (8 workers, 32-slot queue) for the in-process API.
Cancellation is cooperative: an `Arc<AtomicBool>` is set on timeout and long-running
handlers poll it at pipeline stages.

→ Deep dive: [budget-concurrency.md](budget-concurrency.md).

### 12. Machine codes — `src/mcp/machine_codes.rs`

~145 machine-readable response codes in `UPPER_SNAKE_CASE`, plus severity, disposition,
and verdict constants and `finding()` constructors. This is the vocabulary harnesses
route on instead of parsing prose.

→ Deep dive: [machine-codes.md](machine-codes.md).

### 13. Compatibility modes — `src/mcp/compat.rs`

Two validation vocabularies: `EggcalcPython` (Python-`eggcalc` error wording; the MCP
server default) and `StrictNative` (strict JSON Schema type names; the in-process
default). The mode affects type-name formatting in validation errors only; dispatch and
serialization are identical. It is orthogonal to the protocol era.

→ Deep dive: [compatibility.md](compatibility.md).

### 14. Agent API — `src/agent/`

The synchronous in-process `ToolRegistry`: no IPC, no stdio. Four dispatch levels from
`call_json` up to `call_json_with_execution_context`, an `ExecutionContext` builder, and
`prepare_tool_call()` as the shared lookup/validate/dispatch core.

→ Deep dive: [agent-api.md](agent-api.md).

### 15. Preflight wrappers — `src/preflight/`

Six typed workflow wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`,
`PatchApplyCheck`, `TextSecurityInspect`, `DependencyPreflight`) that dispatch through
`ToolRegistry` and parse responses into verdict enums, with a fail-closed
`PreflightError` taxonomy (`ToolCall`/`ToolRejected`/`ContractViolation`). This is the
highest layer; it depends on `agent/`.

→ Deep dive: [preflight.md](preflight.md).

### 16. Testing — `tests/`

Five suites (`calc`, `mcp`, `text`, `parity`, `property`) plus doc tests and a
standalone context-isolation test. Parity compares against the Python `eggcalc` and is
excluded from CI; 37 failures (C1–C6) are accepted and tracked in
`tests/fixtures/accepted_parity_failures.txt`.

→ Deep dive: [testing.md](testing.md).

### 17. Performance — `benches/performance.rs`

A dependency-free, release-mode, maintainer-run harness that prints stable `key=value`
records for the source label, toolchain, target, and scenario measurements. Set
`EGGSACT_BENCH_SHA` to label a candidate. Timing here is **evidence, not a merge
threshold** — never add host-specific timing thresholds to ordinary tests. It also
states the boundary invariants that must hold (Python-style serialization, single-pass
patch application, one-pass replace indexes) and the boundary serialization contracts.

→ Deep dive: [performance.md](performance.md).

### 18. Generated assets — `src/bin/generate_docs.rs`, `scripts/`

The build-time doc generator (dev-tools feature) owns every generated block in this
directory: the registry facts here, the profile reference in
[mcp-server.md](mcp-server.md), and `generated/tool-cards.md`. `scripts/` also holds
the Unicode data generators and the release gate. See also component 4 for the data
files themselves.

→ Deep dive: [generated-assets.md](generated-assets.md).

### 19. Coding-agent integration

Choosing stdio vs in-process transport, selecting a profile and audience, tuning
budgets, and per-client setup examples. Kept separate from the module docs because it
is about *deployment choices*, not internals.

→ Deep dive: [coding-agent-integration.md](coding-agent-integration.md).

---

## Deep Dive Index

Every component above has a dedicated document. This table is the navigation index.

| # | Component | Deep dive | Key source files | Lines |
|---|-----------|-----------|------------------|------:|
| 1 | Entry points & CLI | [cli-binaries.md](cli-binaries.md) | `src/main.rs`, `src/lib.rs` | 602 |
| 2 | Self-update & integration | [self-update.md](self-update.md) | `src/update.rs`, `src/integrate.rs` | 3,287 |
| 3 | Calculator core | [calculator.md](calculator.md) | `src/calc/*.rs` | 8,497 |
| 4 | Text library | [text-library.md](text-library.md) | `src/text/*.rs` | 31,104 |
| 5 | Generated Unicode data | [generated-assets.md](generated-assets.md) | `src/text/*_generated.rs`, `src/bin/generate_docs.rs` | 13,200 |
| 6 | Temporal core | [temporal.md](temporal.md) | `src/temporal/*.rs` | 483 |
| 7 | Typed services | [services.md](services.md) | `src/services/*.rs` | 1,998 |
| 8 | Tool adapters | [tools.md](tools.md) | `src/tools/*.rs` | 19,998 |
| 9 | MCP server | [mcp-server.md](mcp-server.md) | `src/mcp/{server,protocol,response,runtime,schema_validation}.rs` | 5,980 |
| 10 | Registry, profiles, discovery | [registry-profiles.md](registry-profiles.md) | `src/mcp/registry/*.rs`, `src/mcp/specs/`, `src/mcp/schemas/`, `src/mcp/discovery*.rs` | 6,356 |
| 11 | Budget & concurrency | [budget-concurrency.md](budget-concurrency.md) | `src/mcp/{budget,execution,sync_pool,runtime}.rs` | 5,917 |
| 12 | Machine codes | [machine-codes.md](machine-codes.md) | `src/mcp/machine_codes.rs` | 603 |
| 13 | Compatibility modes | [compatibility.md](compatibility.md) | `src/mcp/compat.rs` | 36 |
| 14 | Agent API | [agent-api.md](agent-api.md) | `src/agent/mod.rs` | 1,858 |
| 15 | Preflight wrappers | [preflight.md](preflight.md) | `src/preflight/mod.rs` | 3,543 |
| 16 | Testing | [testing.md](testing.md) | `tests/` (95 files) | 59,839 |
| 17 | Performance | [performance.md](performance.md) | `benches/performance.rs` | — |
| 18 | Generated assets | [generated-assets.md](generated-assets.md) | `src/bin/generate_docs.rs`, `scripts/` | 2,590 |
| 19 | Coding-agent integration | [coding-agent-integration.md](coding-agent-integration.md) | `src/agent/`, `src/mcp/server.rs` | — |

---

## Layering and Dependency Rules

Dependencies point strictly downward, with three deliberate, documented exceptions.

```
main.rs
  └─ lib.rs
       ├─ calc/         normalize → evaluator → units (+ context)
       ├─ mcp/          server, protocol, runtime, budget, schema_validation
       │    └─ registry/  types → all_tools → specs/* → listing
       ├─ tools/        category modules → services/ → text/
       ├─ services/     → text/
       ├─ agent/        → mcp/registry, mcp/budget, mcp/schema_validation
       ├─ preflight/    → agent/ → tools/
       └─ temporal/     → (leaf; budget/cron types only)
  └─ bin/generate_docs.rs
```

| Layer | May depend on | Must never depend on |
|-------|---------------|-----------------------|
| `text/` | nothing internal | `agent`, `mcp`, `tools`, `services`, `preflight` |
| `temporal/` | nothing internal | `tools`, `services`, `agent`, `preflight` |
| `calc/` | nothing internal | `agent`, `mcp`, `tools`, `services`, `preflight` |
| `services/` | `text/`, `temporal/` | `tools` (sibling handlers), registry, profiles, schema validation |
| `tools/` | `text/`, `services/`, `calc/`, `mcp/response`, `mcp/budget` | sibling handlers |
| `mcp/` | `tools/`, `text/` | — |
| `agent/` | `mcp/registry`, `mcp/budget`, `mcp/schema_validation` | — |
| `preflight/` | `agent/` | — |

**Documented exceptions** (all intentional, all narrow):

1. `text::diff` reads the cancellation flag via `crate::mcp::budget::current_cancel_flag()`.
2. `temporal::cron` uses handler budget types and `ToolResponse` for cron matching/search.
3. `tools/*` adapters use `mcp::budget` for cooperative cancellation, and `mcp::response`
   to build the wire shape — that is the boundary, not a violation of it.

The handler-to-handler prohibition is machine-enforced, not just documented. The primary
guard is `substrate_008_generic_handler_graph_has_no_handler_to_handler_edges` in
`tests/mcp/test_substrate_008.rs`, which walks the discovered handler graph
repository-wide across all 86 handlers. `adapter_layering_has_no_handler_to_handler_composition`
in `tests/mcp/test_substrate_007.rs` is the older four-site guard, retained as
supplemental diagnostics. Both fail the build if an adapter in `src/tools/` calls
another adapter in `src/tools/`.

### Recommended Rust import surface

Downstream Rust consumers should integrate in this order:

`calc`/root re-exports → typed `text` primitives → `agent::ToolRegistry` and execution
contexts → typed `preflight` workflow APIs → the MCP server entry surface.

Raw `tools::*` handlers, `services::*` internals, and `mcp` sub-modules beyond `server`
are `pub` for 1.x compatibility but are **not** the recommended import surface. See
[../docs/library-api.md](../docs/library-api.md) for the full hierarchy.

---

## Context Isolation Model

Two context structs carry mutable per-request state, and the boundary between them is
the subtlest part of the API.

| Struct | Location | Purpose |
|--------|----------|---------|
| `EvalContext` | `src/calc/context.rs` | Calculator state: PRNG, memory registers, user variables, side-effect gates |
| `ExecutionContext` | `src/agent/mod.rs` | Dispatch state: `eval_ctx`, profile, audience, budget, cancellation, compat mode, source |

| Path | API | State behavior |
|------|-----|----------------|
| Legacy calculator | `evaluate()`, `run()` | Uses process-global mutable statics |
| Context-aware calculator | `evaluate_with_context(expr, ctx)`, `run_with_context(...)` | Mutations **persist** in the caller's `ctx` across calls |
| Context-aware dispatch | `call_json_with_execution_context(name, args, ctx)` | **Clones** `ctx.eval_ctx` into a thread-local; handler mutations do **not** persist back |

That asymmetry is deliberate: calculator state belongs to the caller's session, while
tool dispatch is a pure function of its inputs.

`..._context_mut` variants are deprecated; use `evaluate_with_context()` /
`run_with_context()` for calculator state and `with_current_eval_context()` for
closure scope. Re-entrant mutable access panics.

### Cooperative cancellation

Cancellation is cooperative, never forceful. On timeout an `Arc<AtomicBool>` is set;
handlers that build a `BudgetContext` internally poll `BudgetContext::should_stop()` at
pipeline stages. Roughly 20 handlers across ten tool files do this — the composite and
route-critical tools (`edit_preflight`, `command_preflight`, `config_preflight`,
`patch_apply_check`, `patch_summary`, `patch_contract_check`,
`dependency_edit_preflight`) plus the heavy analysis tools (`config_file_inspect`,
`text_security_inspect`, `text_diff_explain`, `structured_data_compare`,
`regex_finditer`, `identifier_table_inspect`, `import_export_inspect`,
`code_block_map`, `symbol_name_diff`, `lockfile_inspect`, `repo_tree_summarize`,
`test_command_suggest`, `repo_language_detect`).

---

## Concurrency Model

The MCP stdio server reads requests serially but dispatches each as a tokio task via
`JoinSet`. Responses are serialized through an `mpsc` channel to a single writer task,
so output lines never interleave.

| Constant | Value | Purpose |
|----------|------:|---------|
| `MAX_IN_FLIGHT_REQUESTS` | 32 | Maximum concurrent request tasks |
| `MAX_TOOL_WORKERS` | 16 | Semaphore for concurrent blocking tool executions |
| `MAX_REQUEST_BYTES` | 1,000,000 | Maximum request size |
| `MAX_OUTPUT_BYTES` | 1,000,000 | Maximum response size |
| `MAX_REQUEST_ID_LENGTH` | 1,024 | Maximum JSON-RPC id length |
| `DEFAULT_SYNC_WORKERS` | 8 | In-process sync pool workers |
| `DEFAULT_SYNC_QUEUE` | 32 | In-process sync pool queue slots |

**Clients must correlate responses by JSON-RPC `id`, not by arrival order.** Concurrent
dispatch means responses routinely come back out of order.

The in-process agent API (`src/agent/`) is synchronous and avoids IPC entirely.

---

## Data Flow

```
CLI args
  │
  ├─ Expression ──► calc::run() ──► normalize() ──► evaluator ──► result
  │
  ├─ --mcp ───────► MCP stdio loop
  │                  └─ JSON-RPC dispatch (concurrent, id-correlated)
  │                     └─ server.rs: validate + route
  │                        └─ registry lookup + profile/audience check
  │                           └─ schema validation
  │                              └─ tools/* handler
  │                                 └─ services/ or text/ core
  │                                    └─ ToolResponse built once
  │                                       └─ budget truncation
  │                                          └─ JSON-RPC response
  │
  └─ In-process ──► agent::ToolRegistry::call_json()
                     └─ prepare_tool_call()  (lookup, profile, audience, validation)
                        └─ handler execution
                           └─ ToolResponse with budget enforcement
```

---

## Tool Registration Pattern

Adding a tool requires **one `ToolSpec` entry** in `src/mcp/specs/<category>.rs`:

```rust
pub const MATH_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "math_eval",
        description: "Evaluate arithmetic...",
        handler: math_eval,              // fn from src/tools/math.rs
        input_schema: math_eval_input,    // fn() -> Value from src/mcp/schemas/math.rs
        output_schema: math_eval_output,
        category: "math",
        tier: 0,                          // 0=essential, 1=common, 2=advanced, 3=specialized
        profiles: &["full", "default", "human_math"],
        tags: &["math", "evaluation", "arithmetic", "units", "constants"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
];
```

Aggregation happens once in `ALL_TOOLS_VEC` (`src/mcp/registry/all_tools.rs`), which
collects all 23 category slices. The test `tool_registration_tables_are_in_sync` fails
if a declared tool and the registry disagree.

After any registry, profile, exposure, or discovery change, regenerate the docs (§
[Generated Registry Facts](#generated-registry-facts)) and run the full gate.

---

## Tool Categories (86 tools)

| Category | Count | Description | Deep dive |
|----------|------:|-------------|-----------|
| **text** | 18 | Measure, compare, diff, inspect, transform, hash, fingerprint, escape, prompt detection | [text-library.md](text-library.md) |
| **json** | 6 | Extract, compare, canonicalize, query, shape, structured compare | [tools.md](tools.md) |
| **path** | 6 | Normalize, analyze, compare, scope check, glob match, batch scope check | [tools.md](tools.md) |
| **patch** | 5 | Apply check, summary, edit preflight (route-critical), diff risk, contract check | [tools.md](tools.md) |
| **repo** | 5 | Manifest inspect, config file inspect, tree summarize, test suggest, language detect | [tools.md](tools.md) |
| **math** | 4 | Expression evaluation, unit conversion, unit info, constant lookup | [calculator.md](calculator.md) |
| **validation** | 4 | JSON, brackets, TOML, light schema validation | [tools.md](tools.md) |
| **shell** | 4 | Split, quote/join, argv compare, command preflight (route-critical) | [tools.md](tools.md) |
| **analysis** | 4 | Import/export inspect, code block map, symbol name diff, lockfile inspect | [tools.md](tools.md) |
| **config** | 3 | dotenv validate, INI validate, config preflight (route-critical) | [tools.md](tools.md) |
| **diagnostics** | 3 | Runtime diagnostics, profile inspect, tool availability explain (harness-only) | [tools.md](tools.md) |
| **identifier** | 3 | Analyze, inspect, table inspect (collision detection) | [tools.md](tools.md) |
| **list** | 3 | Compare (ordered/set/multiset), dedupe, sort | [tools.md](tools.md) |
| **regex** | 3 | Validate, safety check, finditer (auto-selects `regex` vs `fancy-regex`) | [text-library.md](text-library.md) |
| **encoding** | 2 | Strict byte codecs and checked radix conversion | [tools.md](tools.md) |
| **markdown** | 2 | Structure parse, code fence extract | [tools.md](tools.md) |
| **network** | 2 | IPv4/IPv6 classification and CIDR arithmetic | [tools.md](tools.md) |
| **temporal** | 2 | Fixed-offset datetime conversion and bounded cron search | [temporal.md](temporal.md) |
| **unicode** | 2 | Policy check, canonicalize | [text-library.md](text-library.md) |
| **version** | 2 | Compare, constraint check (semver/cargo) | [tools.md](tools.md) |
| **cargo** | 1 | Cargo.toml inspect (emits a verdict) | [tools.md](tools.md) |
| **dependency** | 1 | Dependency edit preflight (Rust/Python/Node ecosystem detection) | [tools.md](tools.md) |
| **toml** | 1 | TOML structure analysis (handler lives in `tools/config.rs`) | [tools.md](tools.md) |

---

## Profile System

Eleven named profiles control which tools are exposed. Per-audience counts are in the
[generated registry facts](#generated-registry-facts) table above.

| Profile | Purpose |
|---------|---------|
| `full` | All non-hidden tools |
| `default` | Essential + common tools |
| `codegg_core_min` | Minimal coder-agent set |
| `codegg_core` | Standard coder-agent set |
| `codegg_preflight` | Preflight-focused set |
| `codegg_patch` | Patch editing set |
| `codegg_config` | Config inspection set |
| `codegg_unicode_security` | Unicode/security set |
| `codegg_shell` | Shell command set |
| `codegg_repo_audit` | Repository audit set |
| `human_math` | Human-readable math |

The Model/Harness count gap comes from audience filtering, not different profile
membership. **Audience levels**: `Model` (excludes `HarnessOnly` + `Hidden`), `Harness`
(excludes `Hidden`), `Debug` (all non-hidden). Model-facing code should use
`available_tools_model_safe()`.

There is **no per-call profile**: `tools/call` takes no `profile` argument. The
server-wide `EGGCALC_MCP_PROFILE` env var applies. `Profile::from_str_opt` is strict
(`None` on unknown input); use `Profile::custom(name)` for custom names.

→ Deep dive: [registry-profiles.md](registry-profiles.md).

---

## Key Files Reference

### Source

| File | Lines | Purpose |
|------|------:|---------|
| `src/main.rs` | 518 | CLI arg parsing and dispatch |
| `src/lib.rs` | 84 | Library root, public re-exports |
| `src/calc/normalize.rs` | 2,272 | Natural-language tokenization pipeline |
| `src/calc/evaluator.rs` | 3,797 | AST expression evaluator (~90 functions) |
| `src/calc/units.rs` | 2,310 | 150+ units, 500+ aliases, physical constants |
| `src/calc/context.rs` | 82 | `EvalContext` (mutable per-call calculator state) |
| `src/mcp/server.rs` | 2,268 | Protocol orchestration, stdio loop, concurrent dispatch |
| `src/mcp/execution.rs` | 2,348 | Handler phase state machine, `execute_tool_handler` |
| `src/mcp/runtime.rs` | 1,323 | Rate limiter, limits, profile/audience state, metrics |
| `src/mcp/sync_pool.rs` | 1,263 | `SyncExecutionPool` (bounded worker pool) |
| `src/mcp/response.rs` | 1,162 | `ToolResponse`, `python_json_dumps`, truncation |
| `src/mcp/budget.rs` | 983 | `ToolBudget` tiers, `BudgetContext`, thread-local bridges |
| `src/mcp/discovery.rs` | 895 | `McpSurface`, pinned front doors, search/invoke facades |
| `src/mcp/schema_validation.rs` | 751 | Argument validation against tool schemas |
| `src/mcp/machine_codes.rs` | 603 | ~145 machine-readable response codes |
| `src/mcp/registry/listing.rs` | 594 | Filtering, audience, schema compaction, suggestions |
| `src/mcp/registry/mod.rs` | 522 | Registry aggregation and lookups |
| `src/mcp/protocol.rs` | 476 | JSON-RPC types and era classification |
| `src/mcp/discovery_eval.rs` | 166 | Direct-vs-discovery catalog byte metrics |
| `src/mcp/compat.rs` | 36 | `CompatibilityMode` (`EggcalcPython` vs `StrictNative`) |
| `src/mcp/specs/*.rs` | 23 files | `ToolSpec` declarations, one file per category |
| `src/mcp/schemas/*.rs` | 23 files | JSON schema builders, one file per category |
| `src/tools/text.rs` | 3,270 | Largest handler module (18 text tools) |
| `src/tools/patch.rs` | 1,816 | Patch handlers incl. `edit_preflight` |
| `src/tools/analysis.rs` | 1,599 | Import/export, code block map, lockfile |
| `src/tools/dependency.rs` | 1,425 | Dependency edit preflight |
| `src/tools/helpers.rs` | 1,404 | Shared limits and input helpers |
| `src/tools/repo.rs` | 1,394 | Repo handlers projecting `RepoFacts` |
| `src/tools/shell.rs` | 1,374 | Shell handlers incl. `command_preflight` |
| `src/tools/json.rs` | 1,334 | JSON handlers |
| `src/tools/config.rs` | 758 | Config handlers (also hosts the `toml` handler) |
| `src/tools/validation.rs` | 761 | Validation handlers |
| `src/text/validate.rs` | 3,579 | Largest hand-written text core |
| `src/text/confusables_generated.rs` | 6,715 | Generated confusables data (Unicode 18.0.0) — **do not edit** |
| `src/text/unicode_properties_generated.rs` | 5,737 | Generated security property tables — **do not edit** |
| `src/services/repo.rs` | 943 | `RepoFacts` |
| `src/services/security.rs` | 533 | `SecurityInspection` |
| `src/services/patch_analysis.rs` | 335 | `PatchAnalysis` |
| `src/temporal/cron.rs` | 408 | Cron parser and bounded search |
| `src/agent/mod.rs` | 1,858 | `ToolRegistry`, `Profile`, `ExecutionContext` |
| `src/preflight/mod.rs` | 3,543 | Six typed preflight wrappers |
| `src/update.rs` | 3,029 | Verified binary self-update (Eggup/eggfetch stack) |
| `src/integrate.rs` | 258 | Read-only per-client MCP setup renderers |
| `src/bin/generate_docs.rs` | 748 | Doc generator (dev-tools feature) |

Runtime dependency count is 25, plus 3 dev-dependencies. `Cargo.lock` is tracked —
always pass `--locked`.

### Tests

| Directory | Files | Lines | What they cover |
|-----------|------:|------:|-----------------|
| `tests/mcp/` | 36 | 42,559 | Protocol, tool contracts, route contracts, concurrency, hardening, layering guards |
| `tests/text/` | 28 | 6,523 | Text processing modules plus regression |
| `tests/calc/` | 5 | 3,392 | Calculator units (normalize, evaluator, units, regression) |
| `tests/parity/` | 12 | 4,112 | Python/Rust parity (**requires `eggcalc` at `../eggcalc`; excluded from CI**) |
| `tests/property/` | 12 | 1,105 | Property-based round-trip, idempotence, determinism, symmetry |
| `tests/lib.rs`, `tests/test_context_isolation.rs` | 2 | 2,251 | Suite root and standalone context isolation |

### Generated, config and data

| File | Purpose |
|------|---------|
| `generated/tool-cards.md` | Per-profile tool cards (generated) |
| `scripts/generate_confusables.py` | Regenerates `confusables_generated.rs` (pinned version + SHA) |
| `scripts/release-check.sh` | Canonical local release gate |
| `data/confusables.rs` | Confusables source data for the generator |
| `deny.toml` | `cargo-deny` license/advisory policy |
| `.cargo/config.toml` | The only release link flags (`/BREPRO`, `/DEBUG:NONE`) — load-bearing, never set `RUSTFLAGS` |
| `Cargo.toml` | 22 runtime dependencies; `Cargo.lock` is tracked — always pass `--locked` |

---

## Dependencies

| Category | Crates |
|----------|--------|
| Core | `serde`, `serde_json` (`preserve_order`), `tokio` (rt, macros, io-std, io-util, sync, time), `base64`, `time` |
| Regex | `regex`, `fancy-regex` |
| Unicode | `unicode-normalization`, `unicode-segmentation`, `unicode_names2`, `unicode-general-category`, `caseless` |
| Crypto | `sha2`, `sha1`, `md5`, `crc32fast` |
| Data | `urlencoding`, `toml`, `toml_edit` |
| Self-update (binary-only, not the library/MCP API) | `eggup-core` / `eggup-eggfetch` / `eggup-acquisition` / `eggup-eggpack` **0.1.2** (single git rev) over `eggfetch-core` 0.2.0; `eggfetch-core` and `futures-util` are **dev**-dependencies, pulled in by the test fixture |

`serde_json` has `preserve_order` enabled, so **key order in outputs is intentional** —
do not "fix" it with a `BTreeMap` or a sort.

---

## Key Constants and Limits

| Constant | Value | Location |
|----------|------:|----------|
| `MAX_TEXT_LENGTH` | 100,000 | `src/tools/helpers.rs` |
| `MAX_EXPRESSION_LENGTH` | 10,000 | `src/tools/helpers.rs` |
| `MAX_LIST_ITEMS` | 10,000 | `src/tools/helpers.rs` |
| `MAX_PATTERN_LENGTH` | 1,000 | `src/tools/helpers.rs` |
| `MAX_REGEX_SAMPLES` | 100 | `src/tools/helpers.rs` |
| `MAX_METADATA_FIELD_LENGTH` | 1,000 | `src/tools/helpers.rs` |
| `MAX_FACTORIAL` | 1,000 | `src/calc/evaluator.rs` |
| `MAX_IN_FLIGHT_REQUESTS` | 32 | `src/mcp/runtime.rs` |
| `MAX_TOOL_WORKERS` | 16 | `src/mcp/runtime.rs` |
| `MAX_REQUEST_BYTES` | 1,000,000 | `src/mcp/runtime.rs` |
| `MAX_OUTPUT_BYTES` | 1,000,000 | `src/mcp/runtime.rs` |
| `MAX_REQUEST_ID_LENGTH` | 1,024 | `src/mcp/runtime.rs` |
| `DEFAULT_SYNC_WORKERS` | 8 | `src/mcp/sync_pool.rs` |
| `DEFAULT_SYNC_QUEUE` | 32 | `src/mcp/sync_pool.rs` |
| `MCP_SERVER_NAME` | `"eggsact"` | `src/mcp/runtime.rs` |
| `PREFERRED_PROTOCOL_VERSION` | `"2026-07-28"` | `src/mcp/runtime.rs` |
| `MCP_PROTOCOL_VERSION` | `"2025-11-25"` (legacy preferred) | `src/mcp/runtime.rs` |
| `LEGACY_SUPPORTED_VERSIONS` | `["2025-11-25", "2024-11-05"]` | `src/mcp/runtime.rs` |

Budget tiers (`src/mcp/budget.rs`) — `CHEAP` (10 s), `MODERATE` (30 s), `HEAVY`
(30 s, 2 MB output); all three default to 1 MB input, 100 KB text, 10k list items,
1k pattern chars, 100 regex samples, 16 workers, 100 findings.

Truncation is automatic. Check `limits_applied` in a response to detect it.

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `EGGCALC_MCP_PROFILE` | Server-wide active profile. There is no per-call profile. |
| `EGGCALC_MCP_AUDIENCE` | `Model` (default) / `Harness` / `Debug`, case-insensitive |
| `EGGCALC_MCP_SCHEMA_DETAIL` | `compact` / `normal` / `full` (default `full`) |
| `EGGSACT_MCP_SURFACE` | `direct` / `discovery`; `--mcp-surface` on the CLI overrides |
| `EGGCALC_NO_CONFIG` | Disables config file loading (set for Python `eggcalc` callers) |
| `EGGSACT_BENCH_SHA` | Labels a performance candidate (maintainer-run, non-gating) |

---

## Build and Test Gate

Run in this order before merging:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

`--test-threads=4` is required for integration tests because of Tokio blocking-pool
starvation; it is not a product budget. Parity is excluded from CI because the Python
`eggcalc` reference is not available there — run it locally with
`cargo test --locked --test lib parity` when you have `../eggcalc` checked out.

The full local gate is `scripts/release-check.sh` (requires a clean tree and
`cargo-deny`; it never publishes or tags). Performance evidence is maintainer-run and
non-gating: `cargo bench --locked --bench performance`.

MSRV is **1.89.0** (`rust-version` in `Cargo.toml`).

---

## Gotchas Worth Knowing Before You Edit

- Calculator: `^` is XOR, `**` is power. `g` means gram; use `gravity` /
  `standardgravity` for standard gravity.
- MCP responses are concurrent — correlate by JSON-RPC `id`, never by arrival order.
- One stdio process pins exactly one protocol era, decided by the first message.
  Legacy requires `initialize` → `notifications/initialized` first.
- Regex is **not** PCRE2. `compile_regex()` in `src/text/regex_engine.rs` chooses
  `regex` vs `fancy-regex` per pattern; responses report `engine_used`.
- YAML support is **heuristic only**: `config_file_inspect` has no YAML parser and sets
  `analysis_mode: "heuristic"`. `parse_ok` true only means non-empty. Never treat it as
  syntax validity, and note `config_preflight` excludes YAML entirely.
- Deterministic utils (`ip`/`cidr`/`codec`/`radix`/`datetime`/`cron`) take explicit
  inputs only — no clock, no TZ database, no env, no network.
- `serde_json` has `preserve_order`; key order in output is intentional.
- `update` and `integrate` are verified/read-only: they never install a daemon and
  never edit client config.
- Do not add HTTP to the library/MCP API, retries, or extra fetch features without
  measurement.

---

## Related Documentation

### In this directory

| Doc | Covers |
|-----|--------|
| [calculator.md](calculator.md) | NL math pipeline, AST evaluator, units, constants |
| [text-library.md](text-library.md) | The 27 text processing modules |
| [services.md](services.md) | Typed composite services |
| [temporal.md](temporal.md) | Fixed-offset datetime and cron |
| [tools.md](tools.md) | The 86 tool adapters |
| [mcp-server.md](mcp-server.md) | JSON-RPC transport, protocol eras, dispatch |
| [registry-profiles.md](registry-profiles.md) | `ToolSpec`, profiles, audience, discovery |
| [budget-concurrency.md](budget-concurrency.md) | Budget tiers, cancellation, worker pools |
| [machine-codes.md](machine-codes.md) | Response-code vocabulary |
| [compatibility.md](compatibility.md) | `EggcalcPython` vs `StrictNative` |
| [agent-api.md](agent-api.md) | In-process `ToolRegistry` |
| [preflight.md](preflight.md) | Typed preflight wrappers |
| [cli-binaries.md](cli-binaries.md) | CLI modes and `generate-docs` |
| [self-update.md](self-update.md) | Verified self-update and `integrate` |
| [testing.md](testing.md) | Test structure, parity framework, CI |
| [performance.md](performance.md) | Bench harness and hot-path contracts |
| [generated-assets.md](generated-assets.md) | Generated docs and Unicode data pipeline |
| [coding-agent-integration.md](coding-agent-integration.md) | Transport and profile choices |

### Elsewhere in the repository

| Doc | Location |
|-----|----------|
| CLI usage | `docs/cli.md` |
| Library API hierarchy | `docs/library-api.md` |
| MCP tool catalog | `docs/mcp-tools.md` |
| Contributing | `docs/contributing.md` |
| Parity status | `docs/parity.md` |
| Compatibility policy | `docs/compatibility-policy.md` |
| Release process | `docs/release.md` |
| Installation | `docs/installation.md` |
| Fuzzing | `docs/fuzzing.md` |
| Milestone status | `plans/registry.md` |
| Agent skills | `.opencode/skills/*/SKILL.md` |

---

## Maintaining This Directory

- **Never hand-edit generated content.** The registry-facts block in this file, the
  profile-reference block in [mcp-server.md](mcp-server.md), and
  `generated/tool-cards.md` are all produced by `generate-docs`. Edit the source of
  truth (`src/mcp/specs/`, registry policy) and regenerate.
- **Never hand-edit the Unicode data files.** `confusables_generated.rs` and
  `unicode_properties_generated.rs` come from checked-in generators with pinned
  versions and checksums.
- When you add a component, add it to the [Discrete Components](#discrete-components-at-a-glance)
  section, the [Deep Dive Index](#deep-dive-index) table, and the
  [Related Documentation](#related-documentation) list in the same change.
- Keep the bird's-eye level honest: this file explains *what a component is and how it
  connects*. Design rationale, tables of values, and edge-case semantics belong in the
  component's own deep dive.
