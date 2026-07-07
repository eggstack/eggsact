# eggsact Architecture Overview

**Single-crate Rust project. No workspace. Reimplements Python `eggcalc`.**

eggsact is a natural language math calculator and MCP (Model Context Protocol) server for AI coding agents. It provides 71 tools across 19 categories for deterministic math, text processing, structured data, paths, Unicode safety, shell commands, and more.

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
│  16+ math functions  │ │  registry/         │ │  call_json_        │
│  30+ unit categories │ │  specs/            │ │    with_budget()   │
│  20+ constants       │ │  schemas/          │ │  call_json_        │
└─────────┬───────────┘ └────────┬──────────┘ │    with_execution_  │
          │                      │             │    context()        │
          │                      │             └─────────┬──────────┘
          │                      │                       │
          ▼                      ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                     tools/ — Tool Implementations                │
│                                                                   │
│  71 tools across 19 categories:                                   │
│  math(4) text(18) json(6) regex(3) validation(4) path(6)        │
│  shell(4) list(3) markdown(2) patch(4) config(4) identifier(3)  │
│  unicode(2) version(2) toml(1) cargo(1) dependency(1)           │
│  repo(3) diagnostics(1)                                          │
│                                                                   │
│  helpers.rs — shared constants, utilities, spawn semaphore        │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     text/ — Text Processing Library               │
│                                                                   │
│  24 modules providing the core text operations:                   │
│  primitives, confusables, diff, measure, validate, transform,   │
│  position, regex_safety, replace, path, identifier, shell,       │
│  markdown, glob, config, toml, patch, line_range, unicode_policy,│
│  unicode_tools, inspect_prompt, synthesis, cargo, version        │
│                                                                   │
│  confusables_generated.rs — auto-generated Unicode data           │
└─────────────────────────────────────────────────────────────────┘
```

---

## Module Deep Dives

Each major component has a dedicated architecture doc:

| Component | Doc | What It Covers |
|-----------|-----|----------------|
| **Calculator Core** | [calculator.md](calculator.md) | Natural language normalization, AST evaluator, units, constants, EvalContext |
| **MCP Server** | [mcp-server.md](mcp-server.md) | Protocol, tool registration, profiles, concurrency, budget, response contract |
| **Machine Codes** | [machine-codes.md](machine-codes.md) | Response codes, severity/disposition/verdict constants, finding helpers |
| **Text Library** | [text-library.md](text-library.md) | 24 text modules, public API, code patterns |
| **Compatibility** | [compatibility.md](compatibility.md) | EggcalcPython vs StrictNative validation modes |
| **Agent API** | [agent-api.md](agent-api.md) | ToolRegistry, Profile, ExecutionContext, in-process dispatch |
| **Preflight Wrappers** | [preflight.md](preflight.md) | Typed wrappers, PreflightError, verdict enums, finding parsing |
| **Tool Implementations** | [tools.md](tools.md) | Per-category tool details, composite tools, route-critical tools |
| **Testing** | [testing.md](testing.md) | Test structure, parity tests, how to run tests |
| **CLI & Binaries** | [cli-binaries.md](cli-binaries.md) | main.rs, generate-docs, verify-eggsact, diagnostics |

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

Cancellation is cooperative, not forceful. An `Arc<AtomicBool>` flag is set on timeout. High-risk handlers (`edit_preflight`, `command_preflight`, `config_preflight`, `config_file_inspect`, `dependency_edit_preflight`) check the flag at pipeline stages via `BudgetContext::should_stop()`.

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

## Key Files Reference

### Source

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | 220 | CLI entry point, arg parsing, dispatch |
| `src/lib.rs` | 81 | Library root, re-exports |
| `src/calc/mod.rs` | — | Calculator module re-exports |
| `src/calc/normalize.rs` | — | Natural language tokenization |
| `src/calc/evaluator.rs` | — | AST-based expression evaluator |
| `src/calc/units.rs` | — | Unit definitions and conversions |
| `src/calc/context.rs` | — | EvalContext (mutable per-call state) |
| `src/mcp/server.rs` | — | Protocol orchestration, stdio loop |
| `src/mcp/protocol.rs` | — | JSON-RPC types |
| `src/mcp/response.rs` | — | ToolResponse, error sanitization, finding helpers |
| `src/mcp/runtime.rs` | — | Rate limiter, constants, profile management |
| `src/mcp/budget.rs` | — | Per-tool budgets, BudgetContext, composite sub-budgets |
| `src/mcp/schema_validation.rs` | — | Argument validation against tool schemas |
| `src/mcp/compat.rs` | — | CompatibilityMode (EggcalcPython vs StrictNative) |
| `src/mcp/machine_codes.rs` | — | Machine-readable response codes |
| `src/mcp/registry/types.rs` | — | ToolDefinition, ToolSpec, enums |
| `src/mcp/registry/all_tools.rs` | — | ALL_TOOLS aggregation, PROFILE_NAMES |
| `src/mcp/registry/listing.rs` | — | Filtering, audience, schema compaction |
| `src/mcp/specs/*.rs` | — | ToolSpec declarations (20 files, one per category) |
| `src/mcp/schemas/*.rs` | — | JSON-schema builders (20 files, one per category) |
| `src/tools/helpers.rs` | 1766 | Shared constants, utilities, spawn semaphore |
| `src/tools/*.rs` | — | Tool implementations (19 files) |
| `src/text/*.rs` | — | Text processing library (26 files) |
| `src/agent/mod.rs` | — | ToolRegistry, Profile, ExecutionContext |
| `src/preflight/mod.rs` | — | Typed preflight wrappers |

### Tests

| Directory | Files | What They Cover |
|-----------|-------|----------------|
| `tests/calc/` | 4 | Calculator unit tests (normalize, evaluator, units, regression) |
| `tests/mcp/` | 26 | MCP protocol, tool tests, route contracts, concurrency, hardening |
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
| `EGGCALC_MCP_SCHEMA_DETAIL` | Schema compaction control |

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
| Agent skills | `.skills/*.md` |
