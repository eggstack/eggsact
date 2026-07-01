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
│   │   ├── registry.rs     # Tool registration (ToolSpec declarations)
│   │   ├── protocol.rs     # JSON-RPC types
│   │   ├── response.rs     # ToolResponse, error sanitization
│   │   ├── runtime.rs      # Rate limiter, constants, profile management
│   │   ├── schema_validation.rs # Argument validation
│   │   ├── machine_codes.rs # Machine-readable response codes
│   │   └── schemas.rs      # Re-exports for backward compatibility
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
│   ├── preflight/          # Typed preflight wrappers (ConfigPreflight, CommandPreflight, EditPreflight)
│   └── text/               # Text processing library (24 modules)
├── tests/                  # Integration tests
│   ├── lib.rs              # Test module declarations
│   ├── calc/               # Calculator tests (4 files)
│   ├── mcp/                # MCP protocol + tool tests (17 files)
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
│   └── text-library.md
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

## Module Dependency Flow

```
main.rs → lib.rs → calc/normalize.rs → calc/evaluator.rs → calc/units.rs
                    mcp/server.rs → mcp/protocol.rs, mcp/response.rs, mcp/runtime.rs
                                 → mcp/schema_validation.rs → mcp/registry.rs
                                 → tools/* → text/* modules
```

## Data Flow

1. **CLI**: `main.rs` parses args, calls `run()` or starts MCP server
2. **Library**: `lib.rs` re-exports `run()`, `evaluate()`, `split_at_operators()`
3. **Natural language**: `run()` → `normalize.rs` (tokenize/normalize) → `evaluator.rs` (evaluate)
4. **Direct math**: `evaluate()` → `evaluator.rs` (parse + evaluate)
5. **MCP server**: stdio JSON-RPC 2.0 → `server.rs` (protocol orchestration) → `tools/*` (category modules) → `text/*` modules
6. **In-process agent API**: `agent/ToolRegistry::call_json()` → lookup, profile check, validation (via `prepare_tool_call`) → `tools/*` handlers. No async dispatch; MCP retains timeout/semaphore, agent is synchronous.

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
