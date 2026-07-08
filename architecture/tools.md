# Tool Implementations

The `src/tools/` module contains the actual handler functions for all 78 MCP tools. Each category has its own file, plus a shared helpers module.

See also: [MCP Server](mcp-server.md), [Text Library](text-library.md)

## Files

| File | Category | Tool Count |
|------|----------|-----------|
| `helpers.rs` | shared | constants, utilities, spawn semaphore |
| `math.rs` | math | 4 |
| `text.rs` | text | 18 |
| `json.rs` | json | 6 |
| `regex.rs` | regex | 3 |
| `validation.rs` | validation | 4 |
| `path.rs` | path | 6 |
| `shell.rs` | shell | 4 |
| `list.rs` | list | 3 |
| `markdown.rs` | markdown | 2 |
| `patch.rs` | patch | 5 |
| `config.rs` | config | 4 |
| `identifier.rs` | identifier | 3 |
| `unicode.rs` | unicode | 2 |
| `version.rs` | version | 2 |
| `cargo.rs` | cargo | 1 |
| `dependency.rs` | dependency | 1 |
| `diagnostics.rs` | diagnostics | 1 |
| `repo.rs` | repo | 5 |
| `analysis.rs` | analysis | 4 |

## Handler Signature

All tool handlers share the signature:

```rust
pub fn tool_name(args: &Value) -> ToolResponse
```

Handlers cannot receive an `ExecutionContext` directly. State isolation is achieved at the orchestration layer (`call_json_with_execution_context` clones `EvalContext` into a thread-local). High-risk handlers create a `BudgetContext` internally for cooperative budget checks.

## Shared Helpers (`helpers.rs`)

### Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_TEXT_LENGTH` | 100,000 | Max text input size |
| `MAX_EXPRESSION_LENGTH` | 10,000 | Max math expression length |
| `MAX_LIST_ITEMS` | 10,000 | Max list size |
| `MAX_REGEX_SAMPLES` | 100 | Max regex match samples |
| `MAX_PATTERN_LENGTH` | 1,000 | Max regex pattern length |
| `MAX_METADATA_FIELD_LENGTH` | 1,000 | Max metadata field length |

### Utilities

- `run_with_timeout(timeout, f)` — Execute a closure with a timeout
- `split_lines(text)` — Split text into lines (handles all Unicode line endings)
- `SpawnSemaphore` — Rate-limits concurrent spawned tasks
- `mask_secret_preview(value, show_chars)` — UTF-8-safe masking for secret values

## Composite Tools

Tools marked `composite: true` orchestrate other tools internally. All emit a `verdict` field via `.with_verdict()` and use `finding()` helpers with canonical severity/disposition constants.

| Tool | Verdict Domain | What It Does |
|------|---------------|-------------|
| `edit_preflight` | allow / review / block | Pre-checks an edit using text tools. Composes `path_scope_check`, `text_security_inspect`, `text_fingerprint` when input fields are provided. |
| `command_preflight` | allow / review / block | Pre-checks a shell command via policy engine. Classifies commands, detects wrapper programs (sh/bash/python/node with `-c`/`-e` → review) and script runners (make/just/task → review), detects behavioral features (env mutation scans all argv entries), checks destructive patterns, applies custom allow/deny lists. |
| `config_preflight` | valid / valid_with_warnings / invalid | Pre-checks a config file using validation tools. Auto-detects format (JSON, TOML, dotenv, INI, Cargo.toml). |
| `text_security_inspect` | allow / review / block | Calls multiple text inspection tools and aggregates results. Checks invisible chars, confusables, bidi, prompt injection. |
| `cargo_toml_inspect` | allow / review / block | Inspects Cargo.toml structure and naming. |
| `structured_data_compare` | — | Uses json_compare and list tools for structured data comparison. |

## Route-Critical Tools

A subset of tools are classified as **route-critical** — they produce structured verdicts and machine codes that downstream harnesses depend on for routing decisions. The `is_route_critical()` helper and `ROUTE_CRITICAL_TOOLS` constant in `registry/listing.rs` identify these:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `text_security_inspect`

Route-critical tools must always emit `machine_code` and `verdict` in their response envelope.

## Tool Categories Detail

### Math (4 tools)

| Tool | Description |
|------|-------------|
| `math_eval` | Evaluate natural language or direct math expressions |
| `unit_convert` | Convert between measurement units |
| `unit_info` | Get information about a unit |
| `constant_lookup` | Look up mathematical/physical constants |

### Text (18 tools)

| Tool | Description |
|------|-------------|
| `text_measure` | Count bytes, codepoints, graphemes, words, lines |
| `text_equal` | Compare two texts for equality |
| `text_diff_explain` | Generate human-readable diff between texts |
| `text_inspect` | Inspect text for invisible chars, confusables, bidi |
| `text_count` | Count occurrences of a pattern |
| `text_truncate` | Truncate text to a maximum length |
| `text_fingerprint` | SHA-256 content hash with newline detection |
| `text_hash` | Multi-algorithm text hashing |
| `text_position` | Byte/codepoint/line-column position conversion |
| `text_window` | Extract context window around a position |
| `text_transform` | Apply text transforms (case, normalization, etc.) |
| `text_replace_check` | Preview text replacement before applying |
| `text_security_inspect` | Security inspection (composite) |
| `escape_text` | Escape special characters |
| `unescape_text` | Unescape special characters |
| `prompt_input_inspect` | Detect prompt injection attempts |
| `line_range_extract` | Extract a line range from text |
| `line_range_compare` | Compare two line ranges |

### JSON (6 tools)

| Tool | Description |
|------|-------------|
| `json_extract` | Extract values from JSON by path |
| `json_compare` | Compare two JSON structures |
| `json_canonicalize` | Canonicalize JSON (sorted keys, normalized) |
| `json_query` | Query JSON with expressions (deprecated) |
| `json_shape` | Describe JSON structure |
| `structured_data_compare` | Compare structured data (composite) |

### Path (6 tools)

| Tool | Description |
|------|-------------|
| `path_normalize` | Normalize file paths |
| `path_analyze` | Analyze path components |
| `path_compare` | Compare two paths |
| `path_scope_check` | Check if path is within a workspace root |
| `glob_match` | Test glob pattern matching |
| `path_batch_scope_check` | Batch check multiple paths |

### Shell (4 tools)

| Tool | Description |
|------|-------------|
| `shell_split` | Split shell command into argv |
| `shell_quote_join` | Quote and join argv into a command |
| `argv_compare` | Compare two argv arrays |
| `command_preflight` | Pre-check shell commands (composite) |

### Validation (4 tools)

| Tool | Description |
|------|-------------|
| `validate_json` | Validate JSON syntax |
| `validate_brackets` | Check bracket balance |
| `validate_toml` | Validate TOML syntax |
| `validate_schema_light` | Light JSON schema validation |

### Regex (3 tools)

| Tool | Description |
|------|-------------|
| `validate_regex` | Validate regex syntax |
| `regex_safety_check` | Check regex for ReDoS vulnerabilities |
| `regex_finditer` | Find all regex matches |

### List (3 tools)

| Tool | Description |
|------|-------------|
| `list_compare` | Compare two lists |
| `list_dedupe` | Deduplicate a list |
| `list_sort` | Sort a list |

### Markdown (2 tools)

| Tool | Description |
|------|-------------|
| `markdown_structure` | Parse markdown structure (headings, links, etc.) |
| `code_fence_extract` | Extract code fences from markdown |

### Patch (5 tools)

| Tool | Description |
|------|-------------|
| `patch_apply_check` | Check if a unified diff applies cleanly |
| `patch_summary` | Summarize a unified diff |
| `edit_preflight` | Pre-check edit operations (composite) |
| `diff_risk_classify` | Classify diff risk level |
| `patch_contract_check` | Classify diff by contract-relevant categories |

### Config (4 tools)

| Tool | Description |
|------|-------------|
| `dotenv_validate` | Validate .env files |
| `ini_validate` | Validate INI files |
| `config_preflight` | Pre-check config files (composite) |
| `toml_shape_tool` | Describe TOML structure |

### Identifier (3 tools)

| Tool | Description |
|------|-------------|
| `identifier_analyze` | Analyze identifier naming conventions |
| `identifier_inspect` | Inspect identifiers for collisions |
| `identifier_table_inspect` | Inspect identifier tables |

### Unicode (2 tools)

| Tool | Description |
|------|-------------|
| `unicode_policy_check` | Check text against Unicode policies |
| `canonicalize_text` | Normalize text to a Unicode profile |

### Version (2 tools)

| Tool | Description |
|------|-------------|
| `version_compare` | Compare semver versions |
| `version_constraint_check` | Check if version satisfies a constraint |

### Cargo (1 tool)

| Tool | Description |
|------|-------------|
| `cargo_toml_inspect` | Inspect Cargo.toml structure (composite) |

### Dependency (1 tool)

| Tool | Description |
|------|-------------|
| `dependency_edit_preflight` | Pre-check dependency manifest edits |

### Repo (5 tools)

| Tool | Description |
|------|-------------|
| `repo_manifest_inspect` | Inspect repository manifest |
| `config_file_inspect` | Inspect config files with secret masking |
| `repo_tree_summarize` | Summarize repository file tree |
| `test_command_suggest` | Suggest verification commands from repo paths |
| `repo_language_detect` | Detect languages/ecosystems from repo tree |

### Analysis (4 tools)

| Tool | Description |
|------|-------------|
| `import_export_inspect` | Extract import/export statements from source |
| `code_block_map` | Return top-level block ranges from source/markdown |
| `symbol_name_diff` | Compare old/new source for symbol changes |
| `lockfile_inspect` | Inspect lockfile diffs for dependency-change signals |

### Diagnostics (1 tool)

| Tool | Description |
|------|-------------|
| `runtime_diagnostics` | Print runtime diagnostic info (harness-only) |
