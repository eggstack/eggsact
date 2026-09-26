---
name: text-processing
description: Use when working with text processing functionality in src/text/, adding new text modules, or understanding the text module catalog and conventions in the eggsact codebase.
---

## Module List (27 modules, 30 files including `mod.rs` and generated data files)

| Module | File | Purpose |
|--------|------|---------|
| `cargo` | `cargo.rs` | Cargo.toml inspection |
| `config` | `config.rs` | .env and .ini validation |
| `confusables` | `confusables.rs` | Unicode confusable skeleton (UTS #39 internal/bidi/public stages over sorted static table, binary search) |
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
| `regex_engine` | `regex_engine.rs` | Regex backend classifier (rust-regex vs fancy-regex) |
| `regex_safety` | `regex_safety.rs` | ReDoS detection |
| `replace` | `replace.rs` | Text replacement with preview |
| `script` | `script.rs` | Script diagnostics + UTS #39 resolved-script verdicts (over `unicode_properties`) |
| `shell` | `shell.rs` | Shell command parsing and quoting |
| `synthesis` | `synthesis.rs` | Composite tool orchestration |
| `toml` | `toml.rs` | TOML validation and shape analysis |
| `transform` | `transform.rs` | Text transforms, hashing, fingerprinting |
| `unicode_policy` | `unicode_policy.rs` | Unicode safety policies |
| `unicode_properties` | `unicode_properties.rs` | Unicode 18.0.0 security property tables (Default_Ignorable, Script/Extensions, Bidi) + typed layer |
| `unicode_tools` | `unicode_tools.rs` | Mixed-script, invisible char detection (typed hazards) |
| `validate` | `validate.rs` | JSON/regex/bracket validation, list ops |
| `version` | `version.rs` | Semver comparison and constraint checking |

Plus `confusables_generated.rs` and `unicode_properties_generated.rs` — auto-generated data files (never edit directly).
`confusables_generated.rs` is generated from the pinned Unicode Security 18.0.0 source by
`scripts/generate_confusables.py`; `unicode_properties_generated.rs` from pinned UCD 18.0.0 inputs by
`scripts/generate_unicode_security_properties.py`; the checked-in tables are used at build time,
so ordinary CI does not need network access.

## Code Conventions

- Public functions return result structs with `#[derive(Serialize)]`
- Error types use snake_case strings: `"input_too_large"`, `"invalid_arguments"`, etc.
- Re-export key functions from `src/text/mod.rs`
- Unit tests go in `#[cfg(test)]` modules at the bottom of each file
- Integration tests go in `tests/text/test_<module>.rs` (not all modules have a dedicated test file; `regex_engine` is tested via property tests in `tests/property/test_regex_properties.rs`)

## Adding a New Text Module

1. Create `src/text/<module>.rs`
2. Add `pub mod <module>;` to `src/text/mod.rs`
3. Re-export public functions from `src/text/mod.rs`
4. Add MCP tool wrapper in `src/tools/<category>.rs`
5. Add a `ToolSpec` entry in `src/mcp/specs/<category>.rs` (single source of truth for registration)
6. Add tests in `tests/text/test_<module>.rs`
7. Run `cargo run --locked --features dev-tools --bin generate-docs` to regenerate docs
8. Run `cargo test` to verify

## Reusable Library Pattern

Business logic goes in `src/text/` or `src/calc/`. Shared composite logic used by more than one tool handler goes in `src/services/` (`FingerprintFacts`, `NewlineFacts`, `SecurityInspection`, `RepoFacts`, `PatchAnalysis` — no `ToolResponse`, registry, or schema deps). Repo/path classification lives in `services::repo` (canonical); `tools::helpers` path shims delegate there. MCP tool wrappers in `src/tools/*.rs`
should be thin — they parse input, call the library/service function, and return `ToolResponse`.
This keeps logic testable without JSON-RPC overhead. Never call one `crate::tools::*` handler from another to obtain an internal result — call the typed core/service instead.

## Key Dependencies

- `std::collections::HashMap` for hash maps (no external hash-map dependency)
- `serde` for JSON serialization
- `unicode-normalization`, `unicode-segmentation` for Unicode
- `fancy-regex` for regex with lookahead
- `sha2`, `sha1`, `md5`, `crc32fast` for hashing
