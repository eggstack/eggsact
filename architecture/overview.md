# eggsact Architecture Overview

Single-crate Rust project. No workspace. Reimplements Python `eggcalc`.

## Directory Layout

```
eggsact/
├── src/                    # All source code
│   ├── main.rs             # CLI entry, arg parsing, dispatch
│   ├── lib.rs              # Library root, re-exports run()/evaluate()
│   ├── calc/               # Calculator core (3 modules)
│   ├── mcp/                # MCP server protocol, runtime, registry, validation
│   │   ├── server.rs       # Protocol orchestration, stdio loop, dispatch
│   │   ├── compat.rs       # CompatibilityMode enum (EggcalcPython vs StrictNative)
│   │   ├── registry/         # Tool registration: aggregation, listing, types
│   │   │   ├── mod.rs        # Re-exports, tests
│   │   │   ├── types.rs      # ToolDefinition, ToolSpec, enums
│   │   │   ├── all_tools.rs  # ALL_TOOLS aggregation from specs/, PROFILE_NAMES
│   │   │   └── listing.rs    # Filtering, audience, schema compaction, suggestions
│   │   ├── specs/            # ToolSpec declarations per tool category
│   │   │   ├── mod.rs        # Re-exports all category slices
│   │   │   ├── math.rs       # MATH_TOOLS
│   │   │   ├── text.rs       # TEXT_TOOLS
│   │   │   └── ... (one file per category)
│   │   ├── protocol.rs     # JSON-RPC types
│   │   ├── response.rs     # ToolResponse, error sanitization, finding() helpers, with_verdict, preflight builders
│   │   ├── runtime.rs      # Rate limiter, constants, profile management
│   │   ├── schema_validation.rs # Argument validation
│   │   ├── machine_codes.rs # Machine-readable response codes
│   │   ├── budget.rs       # Per-tool budgets, tiers, composite sub-budgets, BudgetContext
│   │   └── schemas/        # JSON-schema builders per tool category
│   │       ├── mod.rs      # Module declarations + re-exports
│   │       ├── math.rs
│   │       ├── text.rs
│   │       ├── json.rs
│   │       └── ... (one submodule per category)
│   ├── tools/              # MCP tool implementations (by category)
│   │   ├── helpers.rs      # Shared constants, utilities
│   │   ├── math.rs         # Math & unit tools
│   │   ├── text.rs         # Text processing tools (18)
│   │   ├── json.rs         # JSON tools (6)
│   │   ├── regex.rs        # Regex tools (3)
│   │   ├── validation.rs   # Validation tools (4)
│   │   ├── path.rs         # Path tools (5)
│   │   ├── shell.rs        # Shell tools (4)
│   │   ├── list.rs         # List tools (3)
│   │   ├── markdown.rs     # Markdown tools (2)
│   │   ├── patch.rs        # Patch tools (3)
│   │   ├── config.rs       # Config tools (3)
│   │   ├── identifier.rs   # Identifier tools (3)
│   │   ├── unicode.rs      # Unicode tools (2)
│   │   ├── version.rs      # Version tools (2)
│   │   └── cargo.rs        # Cargo tool (1)
│   ├── agent/              # In-process agent API (ToolRegistry, Profile, call_json)
│   ├── preflight/          # Typed preflight wrappers with fail-closed contract enforcement (PreflightError), strict finding parsing, structured RecommendedNextTool, preflight_allow/review/block builders
│   └── text/               # Text processing library (24 modules)
├── tests/                  # Integration tests
│   ├── lib.rs              # Test module declarations
│   ├── calc/               # Calculator tests (4 files)
│   ├── mcp/                # MCP protocol + tool tests (22 files)
│   ├── parity/             # Python/Rust parity tests (12 files)
│   └── text/               # Text processing tests (24 files)
├── scripts/
│   └── generate_confusables.py   # Regenerates confusables data
├── data/
│   └── confusables.rs      # Confusable character data
├── docs/                   # Detailed documentation
│   ├── cli.md
│   ├── contributing.md
│   ├── library-api.md
│   ├── mcp-tools.md
│   └── parity.md
├── architecture/           # Architecture documentation
│   ├── overview.md
│   ├── calculator.md
│   ├── mcp-server.md
│   ├── machine-codes.md
│   ├── text-library.md
│   └── compatibility.md
├── .skills/                # Agent task skills
│   ├── mcp-tools.md
│   ├── testing.md
│   ├── debugging.md
│   ├── release.md
│   └── text-processing.md
├── Cargo.toml              # Package manifest
├── release.sh              # Release pipeline and crate packaging check
└── build.sh                # Simple build script
```

## Context Isolation Model

Mutable per-request state is isolated via two context structs:

- **`EvalContext`** (`src/calc/context.rs`) — per-evaluation calculator state (PRNG, memory registers, user variables, random/side-effect gates).
- **`ExecutionContext`** (`src/agent/mod.rs`) — per-request dispatch state (eval context, compatibility mode, profile, audience, budget, cancellation, request ID, source).

Context-aware APIs (`evaluate_with_context`, `run_with_context`, `call_json_with_execution_context`) thread these through the call chain. `call_json_with_execution_context` additionally sets the `EvalContext` as a thread-local, making it available to calculator-backed tool handlers without changing their `fn(&Value) -> ToolResponse` signature. Legacy wrappers (`evaluate`, `run`, `call_json`) remain for backward compatibility but do not isolate per-call state.

Global statics (AtomicBool flags, RwLock profiles, LazyLock caches, thread-local cancel flags) represent startup-time immutable configuration and are intentionally shared across all requests. Legacy mutable globals (`MEMORY_REGISTERS`, `USER_VARIABLES`, `PRNG_STATE`, `GAUSS_SPARE`) remain for backward compatibility but are bypassed by context-aware APIs.

## Concurrency Model

The MCP stdio server is serial at the read-loop level (one request at a time).
`MAX_TOOL_WORKERS` limits concurrent tool executions within a single dispatch
but does not imply concurrent request reads. The in-process agent API
(`src/agent/`) is synchronous and avoids IPC overhead.

## Module Dependency Flow

```
main.rs → lib.rs → calc/normalize.rs → calc/evaluator.rs → calc/units.rs
                    mcp/server.rs → mcp/protocol.rs, mcp/response.rs, mcp/runtime.rs
                                 → mcp/schema_validation.rs, mcp/budget.rs
                    mcp/registry/ → registry/types.rs, registry/all_tools.rs, registry/listing.rs
                                 → specs/* (category ToolSpec declarations)
                                 → tools/* → text/* modules
```

`ToolDefinition` lives in `registry/types.rs` (not `server.rs`). `ToolResponse::error`
is hidden/deprecated; use `error_without_code_for_legacy_tests_only` only in
legacy test code — all new code must use `error_with_code()`.

## Data Flow

1. **CLI**: `main.rs` parses args, calls `run()` or starts MCP server
2. **Library**: `lib.rs` re-exports `run()`, `evaluate()`, `split_at_operators()`
3. **Natural language**: `run()` → `normalize.rs` (tokenize/normalize) → `evaluator.rs` (evaluate)
4. **Direct math**: `evaluate()` → `evaluator.rs` (parse + evaluate)
5. **MCP server**: stdio JSON-RPC 2.0 → `server.rs` (protocol orchestration) → `tools/*` (category modules) → `text/*` modules
6. **In-process agent API**: `agent/ToolRegistry::call_json()` → lookup, profile check, audience/exposure check, validation (via `prepare_tool_call`) → `tools/*` handlers. No async dispatch; MCP retains timeout/semaphore, agent is synchronous. Uses `StrictNative` validation by default; MCP server uses `EggcalcPython`.

## Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| MCP_PROTOCOL_VERSION | `"2024-11-05"` | `src/mcp/runtime.rs` |
| MCP_SERVER_NAME | `"eggsact"` | `src/mcp/runtime.rs` |
| MAX_REQUEST_BYTES | 1,000,000 | `src/mcp/runtime.rs` |
| MAX_OUTPUT_BYTES | 1,000,000 | `src/mcp/runtime.rs` |
| MAX_TEXT_LENGTH | 100,000 | `src/tools/helpers.rs` |
| MAX_EXPRESSION_LENGTH | 10,000 | `src/tools/helpers.rs` |
| MAX_LIST_ITEMS | 10,000 | `src/tools/helpers.rs` |
| MAX_REGEX_SAMPLES | 100 | `src/tools/helpers.rs` |
| MAX_PATTERN_LENGTH | 1,000 | `src/tools/helpers.rs` |
| MAX_FACTORIAL | 1,000 | `src/calc/evaluator.rs` |
| MAX_PRIME | varies | `src/calc/evaluator.rs` |
| MAX_PERM_COMB | varies | `src/calc/evaluator.rs` |

## Dependencies (18 crates)

Core: `serde`, `serde_json` (preserve_order), `tokio` (full)
Math: `fancy-regex`, `regex`
Unicode: `unicode-normalization`, `unicode-segmentation`, `unicode_names2`, `unicode-general-category`, `caseless`
Crypto: `sha2`, `sha1`, `md5`, `crc32fast`
Data: `ahash`, `urlencoding`, `toml`, `toml_edit`
