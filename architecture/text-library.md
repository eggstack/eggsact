# Text Processing Library

The `src/text/` module provides 24 text processing modules used by MCP tools and available as a library API.

## Module Index

| Module | File | MCP Tools | Public API |
|--------|------|-----------|------------|
| `cargo` | `cargo.rs` | `cargo_toml_inspect` | `cargo_toml_inspect()` |
| `config` | `config.rs` | `dotenv_validate`, `ini_validate` | `dotenv_validate()`, `ini_validate()` |
| `confusables` | `confusables.rs` | (used by identifier_inspect) | `has_confusables()`, `find_confusables()`, `CONFUSABLES` |
| `diff` | `diff.rs` | `text_diff_explain` | `levenshtein_distance()`, `diff_spans()` |
| `glob` | `glob.rs` | `glob_match` | `glob_match()` |
| `identifier` | `identifier.rs` | `identifier_analyze`, `identifier_inspect`, `identifier_table_inspect` | `identifier_analyze()`, `identifier_inspect()` |
| `inspect_prompt` | `inspect_prompt.rs` | `prompt_input_inspect` | `prompt_input_inspect()` |
| `line_range` | `line_range.rs` | `line_range_extract`, `line_range_compare` | `line_range_extract()`, `line_range_compare()` |
| `markdown` | `markdown.rs` | `markdown_structure`, `code_fence_extract` | `markdown_structure()`, `code_fence_extract()` |
| `measure` | `measure.rs` | `text_measure` | `text_length()`, `word_count()`, `line_count()` |
| `patch` | `patch.rs` | `patch_apply_check`, `patch_summary` | `patch_apply_check()`, `patch_summary()` |
| `path` | `path.rs` | `path_normalize`, `path_analyze`, `path_compare`, `path_scope_check` | `path_analyze()`, `path_compare()`, `path_scope_check()` |
| `position` | `position.rs` | `text_position`, `text_window` | `text_position()`, `text_window()` |
| `primitives` | `primitives.rs` | (internal) | `count_graphemes()`, `truncate_to_grapheme()` |
| `regex_safety` | `regex_safety.rs` | `regex_safety_check` | `regex_safety_check()` |
| `regex_engine` | `regex_engine.rs` | (used by `validate_regex`, `regex_safety_check`, `regex_finditer`) | `classify_pattern()` |
| `replace` | `replace.rs` | `text_replace_check` | `text_replace_check()` |
| `shell` | `shell.rs` | `shell_split`, `shell_quote_join`, `argv_compare` | `shell_split()`, `shell_quote_join()`, `argv_compare()` |
| `synthesis` | `synthesis.rs` | composite tools | `text_security_inspect()`, `edit_preflight()`, `command_preflight()`, `config_preflight()` |
| `toml` | `toml.rs` | `validate_toml`, `toml_shape` | `validate_toml()`, `toml_shape()` |
| `transform` | `transform.rs` | `text_transform`, `escape_text`, `unescape_text`, `text_fingerprint`, `text_hash` | `text_transform()`, `escape_text()`, `text_fingerprint()`, `text_hash()` |
| `unicode_policy` | `unicode_policy.rs` | `unicode_policy_check`, `canonicalize_text` | `unicode_policy_check()`, `canonicalize_text()` |
| `unicode_tools` | `unicode_tools.rs` | (internal) | Mixed-script detection, invisible char detection |
| `validate` | `validate.rs` | `validate_json`, `validate_regex`, `validate_brackets`, `validate_schema_light` | `validate_json()`, `validate_regex()`, `validate_brackets()`, `json_shape()` |
| `version` | `version.rs` | `version_compare`, `version_constraint_check` | `check_version_constraint()`, `version_compare()` |

Plus `confusables_generated.rs` — auto-generated data file.

## Code Patterns

### Result Structs

Each module defines result structs with `#[derive(Serialize)]`:
```rust
#[derive(Serialize)]
pub struct TextMeasureResult {
    pub bytes_utf8: usize,
    pub codepoints: usize,
    pub graphemes: usize,
    pub words: usize,
    pub lines: usize,
    // ...
}
```

### Error Handling

Library functions return `Result<T, String>` or specific result types.
MCP tool wrappers in `tools.rs` convert to `ToolResponse`.

### Testing

- Unit tests: `#[cfg(test)]` modules at bottom of each file
- Integration tests: `tests/text/test_<module>.rs`
- Each text module has a corresponding test file

## Key Functions

### Text Measurement
- `text_length(text)` → character count
- `word_count(text)` → whitespace-delimited word count
- `line_count(text)` → newline-separated line count

### Diff & Similarity
- `levenshtein_distance(a, b)` → edit distance
- `diff_spans(a, b, max_diffs)` → semantic diff

### Validation
- `validate_json(text)` → JSON syntax check
- `validate_brackets(input)` → bracket balance check
- `validate_regex(pattern, text)` → regex syntax check

### Transforms
- `escape_text(text, mode)` → escape characters
- `text_transform(text, ops)` → apply transforms
- `text_fingerprint(text)` → content hash
- `text_hash(text, algos, encoding)` → multi-algorithm hash

### Unicode
- `has_confusables(text)` → check for homoglyphs
- `find_confusables(text)` → find with mappings
- `unicode_policy_check(text)` → policy validation
- `canonicalize_text(text, profile)` → normalize

### Position
- `text_position(text, ...)` → convert between byte/codepoint/line-col
- `text_window(text, position, context)` → extract context window

## Regex Backend Classifier

`regex_engine.rs` determines which regex backend compiles a given pattern. It exports:

### `classify_pattern(pattern) -> RegexClassification`

Scans a pattern string and returns a classification containing:

- **`preferred_engine`**: `RegexEngineUsed` — either `RustRegex` or `FancyRegex`. Driven by whether the pattern uses lookaround or backreferences (which require `fancy-regex`) or PCRE-only constructs (which are unsupported).
- **`features`**: `Vec<RegexFeature>` — detected features (`LookAhead`, `LookBehind`, `Backreference`, `NamedCapture`, `InlineFlags`, `UnsupportedPcreConstruct`).
- **`unsupported_features`**: `Vec<String>` — human-readable descriptions of PCRE-only constructs (branch reset, recursion/subroutines, `\K`, control verbs, atomic groups).

### `RegexEngineUsed` enum

| Variant | Backend | When selected |
|---------|---------|---------------|
| `RustRegex` | `regex` crate | No lookaround, no backreferences, no PCRE-only constructs |
| `FancyRegex` | `fancy-regex` crate | Pattern uses lookaround or backreferences |

### `RegexFeature` enum

| Variant | Meaning | Forces backend |
|---------|---------|----------------|
| `LookAhead` | `(?=...)` or `(?!...)` | FancyRegex |
| `LookBehind` | `(?<=...)` or `(?<!...)` | FancyRegex |
| `Backreference` | `\1`–`\9` or `(?P=name)` | FancyRegex |
| `NamedCapture` | `(?P<name>...)` | Neither (both support) |
| `InlineFlags` | `(?i)`, `(?m)`, etc. | Neither (both support) |
| `UnsupportedPcreConstruct(String)` | branch reset, recursion, `\K`, control verbs, atomic groups | — (unsupported by either backend) |

The classifier is a conservative scanner that handles escapes and character classes correctly to avoid false positives on lookaround-like text inside literals or character classes. It is used by `validate_regex`, `regex_safety_check`, and `regex_finditer` to route patterns and report `engine_used`, `dialect`, and `unsupported_features` in their output.
