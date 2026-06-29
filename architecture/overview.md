# eggsact Architecture Overview

Single-crate Rust project. No workspace. Reimplements Python `eggcalc`.

## Directory Layout

```
eggsact/
├── src/                    # All source code
│   ├── main.rs             # CLI entry, arg parsing, dispatch
│   ├── lib.rs              # Library root, re-exports run()/evaluate()
│   ├── calc/               # Calculator core (3 modules)
│   ├── mcp/                # MCP server (3 files, ~4k lines in server.rs)
│   └── text/               # Text processing library (24 modules)
├── tests/                  # Integration tests
│   ├── lib.rs              # Test module declarations
│   ├── calc/               # Calculator tests (4 files)
│   ├── mcp/                # MCP protocol + tool tests (14 files)
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
│   └── text-library.md
├── .skills/                # Agent task skills
│   ├── mcp-tools.md
│   ├── testing.md
│   ├── debugging.md
│   ├── release.md
│   └── text-processing.md
├── Cargo.toml              # Package manifest
├── release.sh              # Release pipeline
└── build.sh                # Simple build script
```

## Module Dependency Flow

```
main.rs → lib.rs → calc/normalize.rs → calc/evaluator.rs → calc/units.rs
                    mcp/server.rs → mcp/tools.rs → text/* modules
                                            ↓
                                   mcp/schemas.rs
```

## Data Flow

1. **CLI**: `main.rs` parses args, calls `run()` or starts MCP server
2. **Library**: `lib.rs` re-exports `run()`, `evaluate()`, `split_at_operators()`
3. **Natural language**: `run()` → `normalize.rs` (tokenize/normalize) → `evaluator.rs` (evaluate)
4. **Direct math**: `evaluate()` → `evaluator.rs` (parse + evaluate)
5. **MCP server**: stdio JSON-RPC 2.0 → `server.rs` dispatches → `tools.rs` → `text/*` modules

## Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| MCP_PROTOCOL_VERSION | `"2024-11-05"` | `src/mcp/server.rs` |
| MCP_SERVER_NAME | `"eggsact"` | `src/mcp/server.rs` |
| MAX_REQUEST_BYTES | 1,000,000 | `src/mcp/server.rs` |
| MAX_OUTPUT_BYTES | 1,000,000 | `src/mcp/server.rs` |
| MAX_TEXT_LENGTH | 100,000 | `src/mcp/tools.rs` |
| MAX_EXPRESSION_LENGTH | 10,000 | `src/mcp/tools.rs` |
| MAX_LIST_ITEMS | 10,000 | `src/mcp/tools.rs` |
| MAX_REGEX_SAMPLES | 100 | `src/mcp/tools.rs` |
| MAX_PATTERN_LENGTH | 1,000 | `src/mcp/tools.rs` |
| MAX_FACTORIAL | 1,000 | `src/calc/evaluator.rs` |
| MAX_PRIME | varies | `src/calc/evaluator.rs` |
| MAX_PERM_COMB | varies | `src/calc/evaluator.rs` |

## Dependencies (16 crates)

Core: `serde`, `serde_json` (preserve_order), `tokio` (full)
Math: `fancy-regex`, `regex`
Unicode: `unicode-normalization`, `unicode-segmentation`, `unicode_names2`, `unicode-general-category`, `caseless`
Crypto: `sha2`, `sha1`, `md5`, `crc32fast`
Data: `ahash`, `urlencoding`, `toml`, `toml_edit`
