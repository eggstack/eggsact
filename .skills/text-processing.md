# Skill: Text Processing Modules

Use this when working with text processing functionality in `src/text/`.

## Module List (24 modules)

| Module | File | Purpose |
|--------|------|---------|
| `cargo` | `cargo.rs` | Cargo.toml inspection |
| `config` | `config.rs` | .env and .ini validation |
| `confusables` | `confusables.rs` | Unicode confusable character detection |
| `diff` | `diff.rs` | Text diffing, Levenshtein distance |
| `glob` | `glob.rs` | Glob pattern matching |
| `identifier` | `identifier.rs` | Identifier naming analysis |
| `inspect_prompt` | `inspect_prompt.rs` | Prompt injection detection |
| `line_range` | `line_range.rs` | Line range extraction/comparison |
| `markdown` | `markdown.rs` | Markdown structure parsing |
| `measure` | `measure.rs` | Text metrics (words, lines, bytes) |
| `patch` | `patch.rs` | Unified diff parsing |
| `path` | `path.rs` | Path analysis and normalization |
| `position` | `position.rs` | Byte/line/column position conversion |
| `primitives` | `primitives.rs` | UTF-8 encoding, grapheme counting |
| `regex_safety` | `regex_safety.rs` | ReDoS detection |
| `replace` | `replace.rs` | Text replacement with preview |
| `shell` | `shell.rs` | Shell command parsing and quoting |
| `synthesis` | `synthesis.rs` | Composite tool orchestration |
| `toml` | `toml.rs` | TOML validation and shape analysis |
| `transform` | `transform.rs` | Text transforms, hashing, fingerprinting |
| `unicode_policy` | `unicode_policy.rs` | Unicode safety policies |
| `unicode_tools` | `unicode_tools.rs` | Mixed-script, invisible char detection |
| `validate` | `validate.rs` | JSON/regex/bracket validation, list ops |
| `version` | `version.rs` | Semver comparison and constraint checking |

Plus `confusables_generated.rs` — auto-generated data file (never edit directly).

## Code Conventions

- Public functions return result structs with `#[derive(Serialize)]`
- Error types use snake_case strings: `"input_too_large"`, `"invalid_arguments"`, etc.
- Re-export key functions from `src/text/mod.rs`
- Unit tests go in `#[cfg(test)]` modules at the bottom of each file
- Integration tests go in `tests/text/test_<module>.rs`

## Adding a New Text Module

1. Create `src/text/<module>.rs`
2. Add `pub mod <module>;` to `src/text/mod.rs`
3. Re-export public functions from `src/text/mod.rs`
4. Add MCP tool wrapper in `src/tools/<category>.rs`
5. Add a `ToolSpec` entry in `src/mcp/specs/<category>.rs` (single source of truth for registration)
6. Add tests in `tests/text/test_<module>.rs`
7. Run `cargo run --bin generate-docs` to regenerate docs
8. Run `cargo test` to verify

## Reusable Library Pattern

Business logic goes in `src/text/` or `src/calc/`. MCP tool wrappers in `src/tools/*.rs`
should be thin — they parse input, call the library function, and return `ToolResponse`.
This keeps logic testable without JSON-RPC overhead.

## Key Dependencies

- `ahash` for hash maps (faster than std HashMap)
- `serde` for JSON serialization
- `unicode-normalization`, `unicode-segmentation` for Unicode
- `fancy-regex` for regex with lookahead
- `sha2`, `sha1`, `md5`, `crc32fast` for hashing
