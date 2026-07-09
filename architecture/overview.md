# eggsact Architecture Overview

**Single-crate Rust project. No workspace.**

eggsact is a deterministic MCP (Model Context Protocol) server and in-process utility library for AI coding agents. It provides 80 tools across 20 categories: math evaluation, text processing, JSON analysis, regex validation, path operations, Unicode safety, shell command preflight, config inspection, patch analysis, dependency management, source analysis, and more. It also re-implements the Python `eggcalc` calculator as one of its tool categories.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Entry Points                             │
│                                                                   │
│  main.rs              CLI arg parsing, dispatch                   │
│    ├─ Expression args → calc::run()                               │
│    ├─ --mcp           → mcp::server::main()                      │
│    └─ --diagnostics   → runtime diagnostics                      │
│                                                                   │
│  lib.rs              Library root, public re-exports              │
│    ├─ run() / evaluate()      (calculator)                        │
│    ├─ agent::ToolRegistry     (in-process API)                    │
│    └─ preflight::*            (typed wrappers)                    │
└─────────────┬────────────────────┬───────────────────┬───────────┘
              │                    │                   │
              ▼                    ▼                   ▼
┌─────────────────────┐ ┌───────────────────┐ ┌────────────────────┐
│     calc/            │ │     mcp/           │ │    agent/           │
│   Calculator Core    │ │  MCP Server        │ │  In-Process API    │
│                      │ │                    │ │                    │
│  normalize.rs        │ │  server.rs         │ │  ToolRegistry      │
│  evaluator.rs        │ │  protocol.rs       │ │  Profile enum      │
│  units.rs            │ │  response.rs       │ │  ToolAudience      │
│  context.rs          │ │  runtime.rs        │ │  ExecutionContext   │
│                      │ │  budget.rs         │ │  ToolCallError     │
│  NL → tokens → AST   │ │  schema_valid.     │ │  ToolView          │
│  → evaluation        │ │  compat.rs         │ │                    │
│                      │ │  machine_codes.rs  │ │  call_json()       │
│  100+ math functions │ │  registry/         │ │  call_json_        │
│  30+ unit categories │ │  specs/            │ │    with_budget()   │
│  50+ constants       │ │  schemas/          │ │  call_json_        │
└─────────┬───────────┘ └────────┬──────────┘ │    with_execution_  │
          │                      │             │    context()        │
          │                      │             └─────────┬──────────┘
          │                      │                       │
          ▼                      ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                     tools/ — Tool Implementations                │
│                                                                   │
│  80 tools across 20 categories:                                   │
│  math(4) text(18) json(6) regex(3) validation(4) path(6)        │
│  shell(4) list(3) markdown(2) patch(5) config(3) toml(1)        │
│  identifier(3) unicode(2) version(2) cargo(1) dependency(1)     │
│  repo(5) diagnostics(3) analysis(4)                              │
│                                                                   │
│  helpers.rs — shared constants, utilities, spawn semaphore        │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     text/ — Text Processing Library               │
│                                                                   │
│  25 modules providing the core text operations:                   │
│  primitives, confusables, diff, measure, validate, transform,   │
│  position, regex_safety, regex_engine, replace, path,           │
│  identifier, shell, markdown, glob, config, toml, patch,        │
│  line_range, unicode_policy, unicode_tools, inspect_prompt,     │
│  synthesis, cargo, version                                       │
│                                                                   │
│  confusables_generated.rs — auto-generated Unicode data           │
└─────────────────────────────────────────────────────────────────┘
```

---

## Module Deep Dives

Each major component has a dedicated architecture doc. The table below serves as an index — read the overview here, then follow the link for the deep dive.

| Component | Doc | What It Covers | Key Files |
|-----------|-----|----------------|-----------|
| **Calculator Core** | [calculator.md](calculator.md) | NL normalization pipeline (30-step), AST evaluator (recursive descent, 8 precedence levels), 150+ unit definitions, 50+ physical/math constants, EvalContext for mutable per-call state, big-integer factorial/perm/comb, sentinel-based return protocol | `src/calc/{normalize,evaluator,units,context}.rs` |
| **MCP Server** | [mcp-server.md](mcp-server.md) | JSON-RPC 2.0 over stdio, tokio concurrent dispatch via JoinSet, tool registration via ToolSpec (single source of truth), profile/audience filtering, schema validation (JSON Schema subset), rate limiting, cancellation model, python-compatible JSON serialization | `src/mcp/{server,protocol,response,runtime,budget,schema_validation,compat,machine_codes}.rs`, `src/mcp/registry/`, `src/mcp/specs/`, `src/mcp/schemas/` |
| **Machine Codes** | [machine-codes.md](machine-codes.md) | ~125 machine-readable response code constants (UPPER_SNAKE_CASE), severity/disposition/verdict constants, `finding()` helper functions for constructing structured findings, route-critical tool contract | `src/mcp/machine_codes.rs` |
| **Text Library** | [text-library.md](text-library.md) | 25 text processing modules: primitives (grapheme-aware), diff/similarity (Levenshtein, LCS), validation (JSON/brackets/regex/TOML), transforms (case/normalize/escape), shell tokenizer, regex engine auto-selection (rust-regex vs fancy-regex), Unicode policy engine, confusables detection, prompt injection detection, composite tool orchestration | `src/text/*.rs` (25 files) |
| **Compatibility** | [compatibility.md](compatibility.md) | `EggcalcPython` vs `StrictNative` validation modes — Python-parity error messages vs strict JSON Schema enforcement, how compat mode propagates through MCP server and agent API | `src/mcp/compat.rs` |
| **Agent API** | [agent-api.md](agent-api.md) | In-process `ToolRegistry` (synchronous dispatch), 11 named `Profile` variants + Custom, `ToolAudience` (Model/Harness/Debug), `ExecutionContext` with builder pattern, 4 dispatch levels (`call_json` → `call_json_with_execution_context`), tool listing methods, `prepare_tool_call()` shared core | `src/agent/mod.rs` |
| **Preflight Wrappers** | [preflight.md](preflight.md) | 5 typed wrappers (`EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`), `PreflightError` taxonomy (ToolCall/ToolRejected/ContractViolation), typed verdict enums with `Other(String)` forward-compat, strict vs permissive `Finding` parsing, `RecommendedNextTool` | `src/preflight/mod.rs` |
| **Tool Implementations** | [tools.md](tools.md) | Per-category tool handler details, composite tool orchestration pattern (edit/command/config preflight), route-critical tools, command policy engine, dependency ecosystem detection, repo analysis, source analysis | `src/tools/*.rs` (20 files) |
| **Testing** | [testing.md](testing.md) | Test structure (70+ files across 4 suites), parity test framework (Python/Rust comparison), CI pipeline, how to add tests, fixture-backed route contract tests | `tests/` |
| **CLI & Binaries** | [cli-binaries.md](cli-binaries.md) | `main.rs` CLI modes, `generate-docs` binary (README/profile/tool-cards generation), `verify-eggsact` binary (9-step verification pipeline), `--diagnostics` flag | `src/main.rs`, `src/bin/{generate_docs,verify_eggsact}.rs` |

---

## Module Dependency Flow

```
main.rs
  ├→ lib.rs
  │    ├→ calc/         (normalize → evaluator → units)
  │    ├→ mcp/server.rs (protocol, runtime, budget, schema_validation)
  │    ├→ mcp/registry/ (types → all_tools → specs/* → listing)
  │    ├→ mcp/response.rs, machine_codes.rs, compat.rs
  │    ├→ tools/*       (category modules → text/* modules)
  │    ├→ agent/        (ToolRegistry, ExecutionContext)
  │    └→ preflight/    (typed wrappers over agent/ + tools/)
  └→ bin/generate_docs.rs, bin/verify_eggsact.rs
```

### Dependency Rules

- **`text/`** is the leaf layer — pure utility, no dependency on agent/mcp/tools
- **`tools/`** depends on `text/` for core operations, `calc/` for math_eval, `mcp/response.rs` for `ToolResponse`
- **`mcp/`** depends on `tools/` (handler dispatch), `text/` (schema validation uses text utilities)
- **`agent/`** depends on `mcp/registry/` (tool lookup), `mcp/budget.rs` (budget enforcement), `mcp/schema_validation.rs` (argument validation)
- **`preflight/`** depends on `agent/` (ToolRegistry dispatch) — the highest layer

---

## Context Isolation Model

Two context structs carry mutable per-request state:

| Struct | Location | Purpose |
|--------|----------|---------|
| `EvalContext` | `src/calc/context.rs` | Calculator state: PRNG, memory registers, user variables, random/side-effect gates |
| `ExecutionContext` | `src/agent/mod.rs` | Dispatch state: eval_ctx, profile, audience, budget, cancellation, compat mode, source |

### Legacy vs Context-Aware APIs

| Path | API | State Behavior |
|------|-----|----------------|
| Legacy calculator | `evaluate()`, `run()` | Uses process-global mutable statics (`MEMORY_REGISTERS`, `PRNG_STATE`, etc.) |
| Context-aware calculator | `evaluate_with_context(expr, ctx)` | Accepts mutable `EvalContext`; mutations persist in caller's `ctx` across calls |
| Context-aware dispatch | `call_json_with_execution_context(name, args, ctx)` | Clones `ctx.eval_ctx` into thread-local; handler mutations **do not persist** back |

### Cooperative Cancellation

Cancellation is cooperative, not forceful. An `Arc<AtomicBool>` flag is set on timeout. High-risk handlers (`edit_preflight`, `command_preflight`, `config_preflight`, `config_file_inspect`, `dependency_edit_preflight`, `text_security_inspect`) check the flag at pipeline stages via `BudgetContext::should_stop()`.

---

## Concurrency Model

The MCP stdio server reads requests serially but dispatches each as a tokio task via `JoinSet`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_IN_FLIGHT_REQUESTS` | 32 | Maximum concurrent request tasks |
| `MAX_TOOL_WORKERS` | 16 | Semaphore for concurrent blocking tool executions |
| `MAX_REQUESTS_PER_SECOND` | 10 | Rate limiter on incoming requests |
| `MAX_REQUEST_BYTES` | 1,000,000 | Maximum request size |
| `MAX_OUTPUT_BYTES` | 1,000,000 | Maximum response size |

Responses are serialized through an `mpsc` channel to a dedicated writer task, preventing interleaved output. **Clients must correlate responses by JSON-RPC `id`**, not by arrival position.

The in-process agent API (`src/agent/`) is synchronous and avoids IPC overhead.

---

## Data Flow

```
CLI args
  │
  ├─ Expression → calc::run() → normalize() → evaluator → result
  │
  ├─ --mcp → MCP stdio loop
  │         → JSON-RPC 2.0 dispatch
  │         → server.rs validates + routes
  │         → registry lookup + profile/audience check
  │         → schema validation
  │         → tools/* handler execution
  │         → text/* core operations
  │         → ToolResponse construction
  │         → budget truncation
  │         → JSON-RPC response (may be out of order)
  │
  └─ In-process → agent::ToolRegistry::call_json()
                  → prepare_tool_call() (lookup, profile, audience, validation)
                  → handler execution
                  → ToolResponse with budget enforcement
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

**Aggregation**: `ALL_TOOLS_VEC` in `src/mcp/registry/all_tools.rs` collects all 20 category slices. A test (`tool_registration_tables_are_in_sync`) catches drift.

---

## Tool Categories (80 tools)

| Category | Count | Description | Deep Dive |
|----------|-------|-------------|-----------|
| **math** | 4 | Expression evaluation, unit conversion, unit info, constant lookup | [calculator.md](calculator.md) |
| **text** | 18 | Measure, compare, diff, inspect, transform, hash, fingerprint, escape, prompt detection | [text-library.md](text-library.md) |
| **json** | 6 | Extract, compare, canonicalize, query, shape, structured data compare | [tools.md](tools.md) |
| **regex** | 3 | Validate, safety check, finditer (auto-selects rust-regex vs fancy-regex) | [text-library.md](text-library.md) |
| **validation** | 4 | JSON, brackets, TOML, light schema validation | [tools.md](tools.md) |
| **path** | 6 | Normalize, analyze, compare, scope check, glob match, batch scope check | [tools.md](tools.md) |
| **shell** | 4 | Split, quote/join, argv compare, command preflight (composite, route-critical) | [tools.md](tools.md) |
| **list** | 3 | Compare (ordered/set/multiset), dedupe, sort | [tools.md](tools.md) |
| **markdown** | 2 | Structure parse, code fence extract | [tools.md](tools.md) |
| **patch** | 5 | Apply check, summary, edit preflight (composite, route-critical), diff risk, contract check | [tools.md](tools.md) |
| **config** | 3 | dotenv validate, INI validate, config preflight (composite, route-critical) | [tools.md](tools.md) |
| **toml** | 1 | TOML structure analysis | [tools.md](tools.md) |
| **identifier** | 3 | Analyze, inspect, table inspect (collision detection) | [tools.md](tools.md) |
| **unicode** | 2 | Policy check, canonicalize | [text-library.md](text-library.md) |
| **version** | 2 | Compare, constraint check (semver/cargo) | [tools.md](tools.md) |
| **cargo** | 1 | Cargo.toml inspect (composite, emits verdict) | [tools.md](tools.md) |
| **dependency** | 1 | Dependency edit preflight (Rust/Python/Node ecosystem detection) | [tools.md](tools.md) |
| **repo** | 5 | Manifest inspect, config file inspect, tree summarize, test suggest, language detect | [tools.md](tools.md) |
| **diagnostics** | 3 | Runtime diagnostics, profile inspect, tool availability explain (harness-only) | [tools.md](tools.md) |
| **analysis** | 4 | Import/export inspect, code block map, symbol name diff, lockfile inspect | [tools.md](tools.md) |

---

## Profile System

11 named profiles control which tools are exposed:

| Profile | Purpose | Tool Count |
|---------|---------|------------|
| `full` | All non-hidden tools | 80 |
| `default` | Essential + common tools | ~50 |
| `codegg_core_min` | Minimal coder-agent set | ~20 |
| `codegg_core` | Standard coder-agent set | ~35 |
| `codegg_preflight` | Preflight-focused set | ~15 |
| `codegg_patch` | Patch editing set | ~12 |
| `codegg_config` | Config inspection set | ~10 |
| `codegg_unicode_security` | Unicode/security set | ~8 |
| `codegg_shell` | Shell command set | ~10 |
| `codegg_repo_audit` | Repository audit set | ~12 |
| `human_math` | Human-readable math | ~10 |

**Audience levels**: `Model` (excludes HarnessOnly+Hidden), `Harness` (excludes Hidden), `Debug` (all non-hidden).

---

## Key Files Reference

### Source

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | 268 | CLI entry point, arg parsing, dispatch |
| `src/lib.rs` | 82 | Library root, re-exports |
| `src/calc/mod.rs` | — | Calculator module re-exports |
| `src/calc/normalize.rs` | ~2100 | Natural language tokenization (30-step pipeline) |
| `src/calc/evaluator.rs` | ~3700 | AST-based expression evaluator (100+ functions) |
| `src/calc/units.rs` | ~2350 | Unit definitions (150+), aliases (500+), conversions |
| `src/calc/context.rs` | 77 | EvalContext (mutable per-call state) |
| `src/mcp/server.rs` | — | Protocol orchestration, stdio loop, concurrent dispatch |
| `src/mcp/protocol.rs` | — | JSON-RPC types |
| `src/mcp/response.rs` | — | ToolResponse, python_json_dumps, finding helpers, truncation |
| `src/mcp/runtime.rs` | — | Rate limiter, constants, profile/audience management |
| `src/mcp/budget.rs` | — | ToolBudget (3 tiers), BudgetContext, composite sub-budgets |
| `src/mcp/schema_validation.rs` | — | Argument validation against tool schemas |
| `src/mcp/compat.rs` | — | CompatibilityMode (EggcalcPython vs StrictNative) |
| `src/mcp/machine_codes.rs` | — | ~125 machine-readable response code constants |
| `src/mcp/registry/types.rs` | — | ToolDefinition, ToolSpec, enums |
| `src/mcp/registry/all_tools.rs` | — | ALL_TOOLS aggregation, PROFILE_NAMES |
| `src/mcp/registry/listing.rs` | — | Filtering, audience, schema compaction, suggestions |
| `src/mcp/specs/*.rs` | — | ToolSpec declarations (20 files, one per category) |
| `src/mcp/schemas/*.rs` | — | JSON-schema builders (20 files, one per category) |
| `src/tools/helpers.rs` | 1778 | Shared constants, utilities, spawn semaphore |
| `src/tools/*.rs` | — | Tool implementations (19 files) |
| `src/text/*.rs` | — | Text processing library (25 files) |
| `src/agent/mod.rs` | ~1400 | ToolRegistry, Profile, ExecutionContext |
| `src/preflight/mod.rs` | ~3000 | Typed preflight wrappers |

### Tests

| Directory | Files | What They Cover |
|-----------|-------|----------------|
| `tests/calc/` | 4 | Calculator unit tests (normalize, evaluator, units, regression) |
| `tests/mcp/` | 28 | MCP protocol, tool tests, route contracts, concurrency, hardening |
| `tests/text/` | 25 | Text processing module tests (one per module + regression) |
| `tests/parity/` | 12 | Python/Rust parity tests (requires `eggcalc` at `../eggcalc`) |
| `tests/test_context_isolation.rs` | 1 | Context isolation integration test |

### Generated & Config

| File | Purpose |
|------|---------|
| `generated/tool-cards.md` | Per-profile tool cards (generated by `cargo run --bin generate-docs`) |
| `scripts/generate_confusables.py` | Regenerates `src/text/confusables_generated.rs` |
| `release.sh` | Full release pipeline |
| `deny.toml` | cargo-deny license/advisory checks |
| `Cargo.toml` | Package manifest (18 dependencies) |

---

## Dependencies

| Category | Crates |
|----------|--------|
| Core | `serde`, `serde_json` (preserve_order), `tokio` (full) |
| Regex | `fancy-regex`, `regex` |
| Unicode | `unicode-normalization`, `unicode-segmentation`, `unicode_names2`, `unicode-general-category`, `caseless` |
| Crypto | `sha2`, `sha1`, `md5`, `crc32fast` |
| Data | `ahash`, `urlencoding`, `toml`, `toml_edit` |

---

## Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| `MAX_TEXT_LENGTH` | 100,000 | `src/tools/helpers.rs` |
| `MAX_EXPRESSION_LENGTH` | 10,000 | `src/tools/helpers.rs` |
| `MAX_LIST_ITEMS` | 10,000 | `src/tools/helpers.rs` |
| `MAX_REGEX_SAMPLES` | 100 | `src/tools/helpers.rs` |
| `MAX_PATTERN_LENGTH` | 1,000 | `src/tools/helpers.rs` |
| `MAX_METADATA_FIELD_LENGTH` | 1,000 | `src/tools/helpers.rs` |
| `MAX_FACTORIAL` | 1,000 | `src/calc/evaluator.rs` |
| `MCP_PROTOCOL_VERSION` | `"2024-11-05"` | `src/mcp/runtime.rs` |
| `MCP_SERVER_NAME` | `"eggsact"` | `src/mcp/runtime.rs` |

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `EGGCALC_NO_CONFIG` | Disables config file loading (set in main.rs) |
| `EGGCALC_MCP_PROFILE` | Active profile for MCP server (set at startup) |
| `EGGCALC_MCP_AUDIENCE` | Active audience for MCP server (`Model`/`Harness`/`Debug`) |
| `EGGCALC_MCP_SCHEMA_DETAIL` | Schema compaction control (`compact`, `normal`, `full`; default: `full`). Invalid values warn to stderr and default to `full` |

---

## Build & Test

```bash
cargo build                          # debug build
cargo build --release                # release build
cargo test                           # all tests
cargo test --lib                     # unit tests only
cargo test --test lib mcp            # MCP tests only
cargo test --test lib parity         # parity tests only
cargo test --test lib text           # text tests only
cargo fmt --all -- --check           # format check
cargo clippy --all-targets --all-features  # lint
cargo run --bin generate-docs        # regenerate docs
cargo run --bin generate-docs -- --check  # verify docs are current
./release.sh                         # full release pipeline
```

---

## Related Documentation

| Doc | Location |
|-----|----------|
| CLI usage | `docs/cli.md` |
| Library API | `docs/library-api.md` |
| MCP tool catalog | `docs/mcp-tools.md` |
| Contributing | `docs/contributing.md` |
| Parity status | `docs/parity.md` |
| Compatibility policy | `docs/compatibility-policy.md` |
| Release process | `docs/release.md` |
| Agent skills | `.skills/*.md` |
