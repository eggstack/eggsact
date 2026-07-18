# Tool Implementations

The `src/tools/` module contains the actual handler functions for all 80 MCP tools. Each category has its own file, plus a shared helpers module.

See also: [MCP Server](mcp-server.md), [Text Library](text-library.md), [Machine Codes](machine-codes.md), [Preflight](preflight.md)

## Module Overview

20 source files implement 80 tools across 20 categories. Every handler follows the same signature:

```rust
pub fn tool_name(args: &Value) -> ToolResponse
```

Handlers cannot receive an `ExecutionContext` directly. State isolation is applied at the orchestration layer (`call_json_with_execution_context` clones `EvalContext` into a thread-local). High-risk handlers create a `BudgetContext` internally for cooperative budget checks by calling `crate::mcp::budget::for_handler(ToolBudget::HEAVY)` at the top of the function.

### File Listing

| File | Category | Tool Count | Notes |
|------|----------|-----------|-------|
| `helpers.rs` | shared | — | Constants, utilities, spawn semaphore, path classification |
| `math.rs` | math | 4 | Calculator-backed eval, unit conversion, constants |
| `text.rs` | text | 18 | Unicode-aware text processing, security inspection |
| `json.rs` | json | 6 | JSON extract, compare, canonicalize, shape, structured comparison |
| `regex.rs` | regex | 3 | Regex validation, safety check, finditer |
| `validation.rs` | validation | 4 | JSON/TOML/bracket validation, light schema validation |
| `path.rs` | path | 6 | Path normalization, analysis, scope checking, glob matching |
| `shell.rs` | shell | 4 | Shell parsing, quoting, argv comparison, command preflight |
| `list.rs` | list | 3 | List compare, deduplicate, sort |
| `markdown.rs` | markdown | 2 | Markdown structure extraction, code fence extraction |
| `patch.rs` | patch | 5 | Patch apply check, summary, edit preflight, diff risk, contract check |
| `config.rs` | config | 3 | dotenv/INI validation, config preflight (toml_shape_tool handler lives here but belongs to `toml` category) |
| `identifier.rs` | identifier | 3 | Identifier analyze, inspect, table inspect with collision detection |
| `unicode.rs` | unicode | 2 | Unicode policy check, text canonicalization |
| `version.rs` | version | 2 | Semver comparison, constraint checking |
| `cargo.rs` | cargo | 1 | Cargo.toml inspection and verdict logic |
| `dependency.rs` | dependency | 1 | Dependency edit preflight with ecosystem detection |
| `diagnostics.rs` | diagnostics | 3 | Runtime diagnostics, profile inspection, tool availability |
| `repo.rs` | repo | 5 | Manifest inspection, config file inspection, tree summary, language detect |
| `analysis.rs` | analysis | 4 | Import/export, code block map, symbol name diff, lockfile inspect |

## Shared Helpers (`helpers.rs`)

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_TEXT_LENGTH` | 100,000 | Max text input size (codepoints) |
| `MAX_INSPECT_ITEMS` | 100 | Max items returned in inspection results |
| `MAX_LIST_ITEMS` | 10,000 | Max list size |
| `MAX_REGEX_SAMPLES` | 100 | Max regex match samples |
| `MAX_REGEX_SAMPLE_LENGTH` | 10,000 | Max single regex sample length |
| `MAX_MATCHES_REGEX` | 100 | Default max matches for regex_finditer |
| `MAX_MATCHES_HARD_CAP` | 1,000 | Hard cap on regex_finditer matches |
| `MAX_PATTERN_LENGTH` | 1,000 | Max regex pattern length |
| `MAX_SCHEMA_DEPTH` | 32 | Max schema nesting depth |
| `MAX_SCHEMA_ELEMENTS` | 10,000 | Max schema elements traversed |
| `MAX_METADATA_FIELD_LENGTH` | 1,000 | Max metadata field length for edit_preflight |
| `REGEX_TIMEOUT_SECONDS` | 5 | Timeout for regex execution |
| `MAX_CONCURRENT_SPAWNED` | 16 | Max concurrent spawned threads |
| `SPAWN_ACQUIRE_TIMEOUT` | 10 | Seconds to wait for a spawn permit |
| `MAX_EXPRESSION_LENGTH` | 10,000 | Max math expression length |

### Utility Functions

| Function | Description |
|----------|-------------|
| `split_lines(text)` | Split text into lines handling `\n`, `\r\n`, `\r`, `\v`, `\f`, `\x1c`–`\x1e`, NEL (`\u{0085}`), LS (`\u{2028}`), PS (`\u{2029}`). |
| `json_type_name(value)` | Returns Python-style type name: `"NoneType"`, `"bool"`, `"int"`, `"float"`, `"str"`, `"list"`, `"dict"`. |
| `unicode_casefold(s)` | Caseless comparison via `caseless::default_case_fold_str`. |
| `normalize_text_count_input(text, normalization)` | Apply NFC or NFKC normalization for text_count. |
| `text_count_matches(work_text, work_target, count_mode)` | Count matches by byte/codepoint/grapheme/substring mode. Returns `(count, positions)`. |
| `contains_true_division(expr)` | Check if expression contains `/` (not `//`), used to force float output in math_eval. |
| `common_prefix_len(a, b)` | Length of common prefix between two strings (char-level). |
| `common_suffix_len(a, b)` | Length of common suffix between two strings (char-level). |
| `classify_difference(raw_equal, nfc_equal, casefold_equal, byte_equal, length_diff, invisibles_detected)` | Classify why two strings differ: `exact_match`, `unicode_normalization_only`, `case_only`, `accent_or_diacritic_difference`, `length_only`, `invisible_character`, `ordinary_text_difference`. |
| `generate_agent_instruction(classification, raw_equal, nfc_equal, byte_equal)` | Generate human-readable guidance based on string difference classification. |
| `is_invisible_char(c)` | Detect zero-width spaces, joiners, separators, BOM, NBSP, line/paragraph separators, word joiners, variation selectors, etc. |
| `invisible_display_name(c)` | Short display name for invisible characters: `ZWSP`, `ZWNJ`, `ZWJ`, `LRM`, `RLM`, `BOM`, `NBSP`, etc. |
| `bidi_display_name(c)` | Short display name for bidirectional control characters: `LRE`, `RLE`, `PDF`, `LRO`, `RLO`, `LRI`, `RLI`, `FSI`, `PDI`. |
| `unicode_name_char(c)` | Full Unicode character name via `unicode_names2` with fallbacks for special characters. |
| `is_combining_mark(c)` | Detect combining marks in ranges U+0300–U+036F, U+1AB0–U+1AFF, U+1DC0–U+1DFF, U+20D0–U+20FF, U+FE20–U+FE2F. |
| `build_safe_repr(text)` | Build a visible representation of text where spaces, tabs, newlines, invisibles, bidi controls, combining marks, and variation selectors are replaced with bracketed display names. |
| `apply_detail_limit(arr, max_items)` | Truncate a slice to `max_items` entries. |
| `inspect_max_items(detail)` | Return 10 for `"summary"`, `MAX_INSPECT_ITEMS` otherwise. |
| `build_extract_summary(v)` | Build a human-readable summary for JSON extract results. |
| `json_value_preview(v)` | Short preview string for a JSON value. |
| `get_json_type(v)` | JSON type name: `"object"`, `"array"`, `"string"`, `"number"`, `"boolean"`, `"null"`. |
| `get_json_type_detail(v)` | Like `get_json_type` but distinguishes `"integer"` from `"float"`. |
| `get_python_json_type(v)` | Python-style type names for JSON: `"str"`, `"int"`, `"float"`, `"bool"`, `"NoneType"`, `"object"`, `"array"`. |
| `compare_json_values(a, b, options)` | Recursive deep JSON comparison with configurable options (ignore order, numeric string equivalence, casefold keys, missing-as-null, max diffs). Returns `(equal, type_match, diffs)`. |
| `json_canonicalize_invalid_response(error)` | Build a structured error response for invalid JSON in canonicalize. |
| `detect_duplicates_in_json(text, duplicates)` | Low-level byte scan to detect duplicate JSON keys at any nesting depth. |
| `sort_json_keys(v)` | Recursively sort object keys using `BTreeMap`. |
| `mask_secret_preview(value)` | UTF-8-safe masking: short values → `"***"`, longer → `"ab***yz"` (2 char prefix/suffix). Never returns the full value. |
| `escape_ascii(s)` | Escape non-ASCII characters as `\uXXXX` for `ensure_ascii` mode in json_canonicalize. |
| `classify_path(path)` | Classify a repo-relative path into a bucket: `manifests`, `lockfiles`, `ci`, `configs`, `tests`, `generated`, `vendor`, `assets`, `scripts`, `docs`, `source`. Returns `(bucket, is_hidden, is_dotfile)`. |
| `classify_paths(paths)` | Batch classify paths into buckets. Returns `(buckets, entrypoint_candidates, high_leverage_paths, tool_hints, findings)`. |
| `classify_diff_path(path)` | Wrapper around `classify_path` for diff risk classification. |

### Input Validation Helpers

| Function | Description |
|----------|-------------|
| `_require_str(args, field, tool)` | Extract a string field from args, checking type and length against `MAX_TEXT_LENGTH`. Returns `Err(Box<ToolResponse>)` on failure. |
| `require_non_negative_int_arg(args, field, tool)` | Extract a non-negative integer field, rejecting booleans, floats, and negative values. |
| `require_array_arg(args, field, tool)` | Extract an array field, returning `Err(Box<ToolResponse>)` if not an array. |
| `validate_line_range_order(start_line, end_line, tool)` | Ensure `start_line <= end_line`. |

## Input Validation Pattern

Every handler validates arguments in this order:

1. **Extract required parameters** — using `_require_str`, `args.get().and_then(|v| v.as_str())`, or `require_array_arg`. Missing/wrong-type → `INVALID_ARGUMENTS` error with machine code.
2. **Check text length** — `text.chars().count() > MAX_TEXT_LENGTH` → `INPUT_TOO_LARGE` error. This is a codepoint count, not byte count.
3. **Validate enum parameters** — check against allowed values (e.g., `["posix", "windows"]`). Invalid → `INVALID_ARGUMENTS`.
4. **Validate numeric bounds** — negative, zero-where-positive-expected, overflow → `INVALID_ARGUMENTS`.
5. **Mode-specific argument contracts** — composite tools validate that the correct combination of arguments is present for each mode (e.g., `edit_preflight` validates `old`/`new` for literal mode, `patch` for patch mode, `start_line`/`end_line`/`new` for line_range mode).
6. **Conflicting argument detection** — reject arguments that belong to a different mode (e.g., `patch` in literal mode).

## Response Pattern

### Success Responses

```rust
ToolResponse::success(result_json, Some("tool_name"))
    .with_tool("tool_name")
    .with_machine_code("MACHINE_CODE")   // optional
    .with_verdict("allow")               // optional
    .with_findings(findings_vec)         // optional
    .with_recommended_next_tool(next_tool) // optional
    .with_warnings(warnings_vec)         // optional
```

### Error Responses

```rust
ToolResponse::error_with_code(
    "error_type",                    // error type string
    machine_codes::SOME_CODE,       // machine-readable code
    "Human-readable message",       // message
    Some(vec!["suggestion"]),       // optional suggestions
    Some("tool_name"),              // tool name
)
```

### Verdict Constants

| Constant | Value | Meaning |
|----------|-------|---------|
| `verdict::ALLOW` | `"allow"` | Action is permitted |
| `verdict::REVIEW` | `"review"` | Needs human review |
| `verdict::BLOCK` | `"block"` | Action should be blocked |
| `verdict::VALID` | `"valid"` | Config is valid |
| `verdict::INVALID` | `"invalid"` | Config is invalid |
| `verdict::VALID_WITH_WARNINGS` | `"valid_with_warnings"` | Valid but with warnings |

### Severity/Disposition Constants

| Severity | Disposition | Typical Use |
|----------|-------------|-------------|
| `severity::CRITICAL` | `disposition::BLOCKING` | Parse errors, security blocks |
| `severity::HIGH` | `disposition::BLOCKING` | Blocking findings |
| `severity::MEDIUM` | `disposition::CAUTION` | Review-required findings |
| `severity::LOW` | `disposition::INFORMATIONAL` | Informational findings |
| `severity::INFO` | `disposition::INFORMATIONAL` | Notices |

## Composite Tools

Tools marked `composite: true` orchestrate other tools internally. All emit a `verdict` field via `.with_verdict()` and use `finding()` helpers with canonical severity/disposition constants. They collect sub-results in a `subresults` map and findings in a vector.

### edit_preflight

**File:** `patch.rs` | **Budget:** `HEAVY` | **Route-critical:** yes

Pre-checks an edit operation before applying it. Supports three replacement modes:

#### Mode Dispatch

| Mode | Required Args | Conflicting Args | Sub-tool Called |
|------|---------------|-------------------|-----------------|
| `literal` | `old`, `new` | `patch`, `start_line`, `end_line` | `text_replace_check` (exact match mode) |
| `patch` | `patch` | `old`, `new`, `start_line`, `end_line` | `patch_apply_check` |
| `line_range` | `start_line`, `end_line`, `new` | `old`, `patch` | `line_range_extract` |

#### Pipeline (all modes)

1. **Input validation** — mode-specific argument contract (required/forbidden args), metadata bounds checking on `edit_metadata.*` fields (max 1,000 chars).
2. **Mode-specific sub-tool call** — dispatches to `text_replace_check`, `patch_apply_check`, or `line_range_extract`. Collects `NO_MATCH`, `MULTIPLE_MATCHES`, `PATCH_FAILED`, `INVALID_RANGE` findings.
3. **Fingerprint check** — if `expected_fingerprint` is provided, computes SHA-256 of the relevant text (original for literal, result_fingerprint from patch_apply_check for patch, fingerprint from line_range_extract for line_range). Emits `FINGERPRINT_MISMATCH` finding if mismatch.
4. **Path scope check** — if `file_path` + `workspace_root` are provided, calls `path_scope_check`. Emits `PATH_SCOPE_ESCAPE` finding if target is outside workspace root.
5. **Newline style detection** — if `newline_policy != "skip"`, detects newline style on original and replacement text using `text_fingerprint`. Emits `NEWLINE_INCONSISTENCY` finding if mixed styles.
6. **Unicode security check** — if `unicode_policy != "skip"`, calls `text_security_inspect` on the replacement text. Emits `UNICODE_RISK` finding if verdict is `block` or `review`.
7. **Verdict derivation** — `derive_primary_machine_code()` selects the highest-priority code from findings (PATH_SCOPE_ESCAPE > LINE_RANGE_INVALID > PATCH_FAILED > AMBIGUOUS_REPLACEMENT > FINGERPRINT_MISMATCH > UNICODE_RISK > NEWLINE_INCONSISTENCY > EDIT_OK). `derive_verdict()` maps to allow/review/block.

### command_preflight

**File:** `shell.rs` | **Budget:** `HEAVY` | **Route-critical:** yes

Pre-checks a shell command through a multi-stage pipeline:

#### Pipeline

1. **Parse** — calls `shell_split` to tokenize the command. On parse error, emits `SHELL_PARSE_ERROR` (HIGH/BLOCKING).
2. **policy_config rules** — if `policy_config` is provided, applies custom `deny_commands`/`allow_commands`/`deny_subcommands`. Deny beats allow.
3. **Built-in policy classification** — calls `classify(program, subcommand, policy)` which dispatches to `classify_default`, `classify_strict`, or `classify_permissive`. See [Command Policy Engine](#command-policy-engine) below.
4. **Destructive pattern detection** — `check_destructive_patterns()` detects:
   - Pipe-to-shell: `curl/wget ... | sh/bash/zsh/python/perl/ruby`
   - `rm -rf /` or `rm -rf .`
   - `git reset --hard`, `git clean -fdx`, `git push --force`
   - `chmod -R 777`, recursive `chown`
5. **Behavioral feature detection** — `detect_behavioral_features()` scans argv and shell features for:
   - Network access (curl, wget, ssh, scp, etc.)
   - Filesystem write (rm, dd, mkfs, etc.)
   - Privilege escalation (sudo, su, doas, pkexec)
   - Process control (kill, pkill, screen, tmux)
   - Environment mutation (FOO=bar pattern in argv)
   - Shell features (command substitution, redirection, pipe, background)
6. **policy_config allow_* overrides** — `allow_network`, `allow_filesystem_write`, `allow_process_control`, `allow_env_mutation` can suppress behavioral findings.
7. **Risky shell feature findings** — emits `RISKY_SHELL_FEATURE` for each enabled shell feature (pipe, redirect, etc.).
8. **Regex safety check** — if the command looks like it contains regex (grep/sed/awk/regex), runs `regex_safety_check` on regex-like args. Emits `REGEX_RISK` findings.
9. **Verdict** — block if any critical/high finding; review if medium; allow otherwise.
10. **Primary code selection** — `select_primary_code()` uses priority order: PARSE_ERROR > DESTRUCTIVE_COMMAND > PRIVILEGE_ESCALATION > NETWORK_ACCESS > FILESYSTEM_WRITE > PROCESS_CONTROL > COMMAND_SUBSTITUTION > REDIRECTION > PIPELINE > BACKGROUND_EXECUTION > UNAPPROVED_COMMAND > ENV_MUTATION > POLICY_REVIEW > RISK > REGEX_RISK.
11. **Recommended next tool** — suggests `shell_split` for unbalanced quotes/parse errors, `text_security_inspect` for non-ASCII commands.

### config_preflight

**File:** `config.rs` | **Budget:** `HEAVY` | **Route-critical:** yes

Pre-checks a configuration file using format-specific validation:

#### Pipeline

1. **Format detection** — auto-detects from `format` param or heuristics:
   - Starts with `{` or `[` → try JSON first, fallback TOML
   - Contains `=` and not JSON → dotenv
   - Default → JSON
   - Explicit: `json`, `toml`, `dotenv`, `ini`, `cargo_toml`
2. **Format-specific validation:**
   - **JSON** → `validate_json` → if valid and schema provided → `validate_schema_light` → if valid → `json_canonicalize` (canonicalization check)
   - **TOML** → `validate_toml` → if valid → `toml_shape` (structure analysis)
   - **dotenv** → `dotenv_validate` (custom validator with key pattern, duplicate policy, export support)
   - **INI** → `ini_validate` (section parsing, duplicate detection)
   - **cargo_toml** → `cargo_toml_inspect` (Cargo-specific validation)
3. **Verdict** — `invalid` if parse fails; `valid_with_warnings` if schema violations; `valid` otherwise.
4. **Machine code** — `CONFIG_PARSE_FAILED` > `CONFIG_SCHEMA_MISMATCH` > `CONFIG_HAS_WARNINGS` > `CONFIG_OK`.

### text_security_inspect

**File:** `text.rs` | **Budget:** `HEAVY` | **Route-critical:** yes

Composite security inspection that aggregates multiple sub-tools:

#### Pipeline

1. **text_inspect** — always called. Checks invisible characters, confusables, bidi controls, mixed scripts, normalization differences. Emits `HIDDEN_CHARS`, `CONFUSABLES` findings.
2. **unicode_policy_check** — maps `policy` param: `"source_code"` → `source_code` policy, everything else → `human_text` policy. Iterates individual findings from the sub-tool.
3. **Normalization check** — if `normalize != "none"`, applies NFC/NFD/NFKC/NFKD and checks if text changed. Emits `NORMALIZATION_DIFF` machine code.
4. **prompt_input_inspect** — called when policy is `"prompt"`, `"markdown"`, or `"default"`. Checks for hidden Unicode, bidi controls, HTML comments, markdown links, ANSI escapes, terminal controls, base64 blobs, instruction phrases, long minified lines. Emits `PROMPT_INJECTION_RISK`.
5. **identifier_inspect** — called when policy is `"identifier"` or `"default"`. Extracts words matching Python's `str.isidentifier()` pattern, runs collision/confusable detection. Emits `IDENTIFIER_COLLISION_RISK`.
6. **Verdict** — block if any HIGH severity finding; review if MEDIUM; allow otherwise.
7. **Machine code priority** — `TEXT_SECURITY_OK` (no issues) or first of: `UNICODE_RISK`, `NORMALIZATION_DIFF`, `PROMPT_INJECTION_RISK`, `IDENTIFIER_COLLISION_RISK`.

### structured_data_compare

**File:** `json.rs` | **Budget:** `HEAVY`

Compares two structured data inputs (currently JSON only):

#### Pipeline

1. **Validate both inputs** — calls `validate_json` on `a` and `b`. Emits `INVALID_JSON_A`/`INVALID_JSON_B` findings if invalid.
2. **JSON comparison** — calls `json_compare` with configurable options (ignore_object_order, ignore_array_order, max_diffs). Collects `VALUE_DIFF` findings.
3. **Shape comparison** — calls `json_shape_tool` on both inputs. Emits `TYPE_MISMATCH` finding if top-level types differ.
4. **Machine code** — `INVALID_INPUT` > `DATA_EQUAL` > `DATA_DIFF`.
5. **Sub-results** — includes `validate_a`, `validate_b`, `json_compare`, `shape_a`, `shape_b`.

## Route-Critical Tools

A subset of tools are classified as **route-critical** — they produce structured verdicts and machine codes that downstream harnesses depend on for routing decisions. The `is_route_critical()` helper and `ROUTE_CRITICAL_TOOLS` constant in `registry/listing.rs` identify these:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `text_security_inspect`

Route-critical tools **must** always emit `machine_code` and `verdict` in their response envelope. This is verified by fixture-backed route-contract tests (`RouteFixture` struct, `all_fixtures()`) in `tests/mcp/test_route_contracts.rs`.

## Tool Categories Detail

### Math (4 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `math_eval` | Evaluate natural language or direct math expressions | Executes directly in bounded `spawn_blocking` with `catch_unwind`. Detects true division (`/`) to force float output. Returns `{value, type, unit?, display?}`. |
| `unit_convert` | Convert between measurement units | Validates unit existence and category compatibility. Special-cases temperature conversions (non-linear). Returns `{value, from_unit, to_unit, factor}`. |
| `unit_info` | Get information about a unit | Returns canonical name and category. Fails on unknown units. |
| `constant_lookup` | Look up physical/mathematical constants | Case-insensitive lookup in `PHYSICAL_CONSTANTS` map. Returns `{name, value, symbol, display_name}`. |

### Text (18 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `text_measure` | Count bytes, codepoints, graphemes, words, lines | Detects combining marks, ZWJ sequences, variation selectors, regional indicators, emoji modifiers. Reports normalization status (NFC/NFD/NFKC/NFKD). |
| `text_equal` | Compare two texts for equality | Supports casefold, normalization (NFC/NFD/NFKC/NFKD), trim, ignore_newline_style, ignore_trailing_whitespace, ignore_final_newline. Reports first difference with codepoints. |
| `text_diff_explain` | Generate human-readable diff between texts | Uses `levenshtein_distance` and `diff_spans`. Reports classification (case_only, unicode_normalization_only, etc.), security findings (invisibles, confusables), agent instruction. |
| `text_inspect` | Inspect text for invisible chars, confusables, bidi | Reports metrics, normalization status, invisibles, bidi controls, confusables, mixed scripts. Machine code priority: CONFUSABLES > BIDI > INVISIBLES. |
| `text_count` | Count occurrences of a pattern | Modes: `codepoint`, `grapheme`, `byte`, `substring`. Supports normalization. Without target, returns character frequency table. |
| `text_truncate` | Truncate text to a maximum length | Grapheme-aware truncation via `truncate_to_grapheme`. |
| `text_fingerprint` | SHA-256 content hash with newline detection | Configurable unicode normalization, newline normalization (raw/LF), trim_final_newline, casefold. Reports newline style. |
| `text_hash` | Multi-algorithm text hashing | Supports 40+ encodings (UTF-8, ASCII, Latin-1, Shift_JIS, GB2312, etc.). Multiple algorithms per call. |
| `text_position` | Byte/codepoint/line-column position conversion | Bidirectional conversion between byte_offset, codepoint_index, line/column, utf16_offset. Configurable line_base (0/1) and column_base (0/1). |
| `text_window` | Extract context window around a position | Supports position kinds: byte_offset, codepoint_index, grapheme_index, line_column. Returns before/after context with visible repr. |
| `text_transform` | Apply text transforms (case, normalization, etc.) | 13 operations: normalize_nfc/nfd/nfkc/nfkd, casefold, trim, trim_trailing_whitespace, normalize_newlines_lf, ensure_final_newline, strip_final_newline, remove_zero_width, remove_bidi_controls, visible_repr. |
| `text_replace_check` | Preview text replacement before applying | Modes: exact, nfc, nfkc, casefold, whitespace_collapse. Reports match count, positions, preview, newline style change. |
| `text_security_inspect` | Security inspection (composite) | See [Composite Tools](#composite-tools) section above. |
| `escape_text` | Escape special characters | 9 modes: html_text, json_string, markdown_code_block, markdown_inline_code, posix_shell_single, python_string, regex_literal, rust_string, url_component. |
| `unescape_text` | Unescape special characters | 4 modes: json_string, python_string, unicode_escape, url_component. |
| `prompt_input_inspect` | Detect prompt injection attempts | 9 detection categories (see below). Computes risk score (error=5, warn=3, info=1). |
| `line_range_extract` | Extract a line range from text | Configurable line_base. Returns extracted text, byte/char offsets, newline style, SHA-256 fingerprint. |
| `line_range_compare` | Compare two line ranges | Modes: exact, ignore_trailing_whitespace, normalize_newlines. Returns fingerprints and first difference. |

#### prompt_input_inspect Detection Categories

| Check | Description | Finding Codes |
|-------|-------------|---------------|
| `unicode_hidden` | Zero-width spaces, joiners, BOM, NBSP, variation selectors, line/paragraph separators | `HIDDEN_CHAR` (error for ZWSP/ZWNJ/ZWJ/WORD JOINER; warn for others) |
| `bidi` | Bidirectional control characters (LRE, RLE, PDF, LRO, RLO, LRI, RLI, FSI, PDI, LRM, RLM) | `BIDI_CONTROL` (warn) |
| `html_comments` | HTML comments `<!--...-->` | `HTML_COMMENT` (warn if non-empty, info if empty) |
| `markdown_links` | Markdown links `[text](url)` — flags data: URIs and URL/text mismatches | `MARKDOWN_LINK` (warn for suspicious, info otherwise) |
| `ansi_escapes` | ANSI escape sequences `\x1b[...` | `ANSI_ESCAPE` (warn) |
| `terminal_controls` | C0/C1 control characters, terminal escape sequences | `TERMINAL_CONTROL` (info) |
| `base64_like_blobs` | Base64-encoded strings ≥64 chars with mixed case/digits | `BASE64_BLOB` (warn) |
| `instruction_phrases` | Prompt injection patterns: "ignore previous", "system prompt", "you are now", "jailbreak", etc. (22 default phrases, customizable) | `INSTRUCTION_PHRASE` (warn) |
| `long_minified_lines` | Lines >1,000 characters | `LONG_LINE` (info) |

### JSON (6 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `json_extract` | Extract values from JSON by JSON Pointer path | Supports `~0`/`~1` escaping. Reports missing keys with available_keys. Truncates preview to `max_output_chars`. |
| `json_compare` | Compare two JSON structures | Options: ignore_object_order, ignore_array_order, numeric_string_equivalence, casefold_keys, treat_missing_null_as_equal, max_diffs. Reports path-based diffs. |
| `json_canonicalize` | Canonicalize JSON (sorted keys, normalized) | Options: sort_keys, indent, ensure_ascii, detect_duplicate_keys, trailing_newline. Python-style compact formatter for minified output. SHA-256 hash of canonical form. |
| `json_query` | Query JSON with expressions (deprecated) | Deprecated in favor of `json_extract`. Emits deprecation warning and `recommended_next_tool`. |
| `json_shape` | Describe JSON structure | Configurable max_depth, max_keys, max_array_items. Reports type, shape, truncation status. |
| `structured_data_compare` | Compare structured data (composite) | See [Composite Tools](#composite-tools) section above. |

### Path (6 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `path_normalize` | Normalize file paths | Supports posix/windows platforms. collapse_dot_segments, preserve_trailing_separator options. |
| `path_analyze` | Analyze path components | Reports: is_absolute, components, parent, name, stem, suffix, suffixes, hidden, has_traversal, normalized_lexical. |
| `path_compare` | Compare two paths | Options: platform, case_sensitive, normalize_separators, collapse_dot_segments. |
| `path_scope_check` | Check if path is within a workspace root | Reports inside_root, escapes_via_dotdot, relative_path, absolute_target. |
| `glob_match` | Test glob pattern matching | Platform-aware (posix/windows). Reports matched/unmatched segments. |
| `path_batch_scope_check` | Batch check multiple paths | Checks each target against root. Detects escaping, absolute, dotdot, and duplicate normalized paths. Emits per-target findings. |

### Shell (4 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `shell_split` | Split shell command into argv | POSIX shell parsing. Reports features: has_pipe, has_redirection, has_command_substitution, has_variable_expansion, has_glob_pattern, has_control_operator, has_background, has_unbalanced_quotes. |
| `shell_quote_join` | Quote and join argv into a command | Validates all elements are strings. Reports roundtrip_ok. |
| `argv_compare` | Compare two argv arrays | XOR validation: each side must be command OR argv, not both. Reports first_difference. |
| `command_preflight` | Pre-check shell commands (composite) | See [Composite Tools](#composite-tools) section above. |

### Validation (4 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `validate_json` | Validate JSON syntax | Reports valid, error, line/column/position, top_level_keys, json_type. |
| `validate_brackets` | Check bracket balance | Customizable pairs (max 64 entries). Default: `()[]{}` `<>`. Reports unmatched_openers, unmatched_closers. |
| `validate_toml` | Validate TOML syntax | Reports valid, error, line/column/position, toml_type, tables, top_level_keys. |
| `validate_schema_light` | Light JSON schema validation | Validates type, required keys, additional_properties, min/max_items, min/max_length, pattern (regex), enum. Max depth 32, max elements 10,000, max violations 100. |

### Regex (3 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `validate_regex` | Validate regex syntax | Safety check first (blocks medium/high risk). Runs on dedicated thread with 5s timeout. Reports engine_used (`"rust-regex"` or `"fancy-regex"`), dialect (`"eggsact-regex"`), unsupported_features. |
| `regex_safety_check` | Check regex for ReDoS vulnerabilities | Reports risk level (none/medium/high) and findings. Used internally by command_preflight. |
| `regex_finditer` | Find all regex matches | Safety check, 5s timeout, configurable max_matches (hard cap 1,000). Reports match, span, groups, groupdict, optional line/column. Engine auto-selection via `classify_pattern()`. |

### List (3 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `list_compare` | Compare two lists | Three modes: `ordered` (positional alignment with equal_prefix_length), `set` (set membership), `multiset` (count comparison). Supports casefold, normalization, trim, near_match (Levenshtein distance threshold). |
| `list_dedupe` | Deduplicate a list | Stable (preserves order) or unstable. Supports normalization and casefold. Reports original/deduped counts. |
| `list_sort` | Sort a list | Stable or unstable sort. Supports normalization, casefold, reverse. Key-based sorting preserves original values. |

### Markdown (2 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `markdown_structure` | Parse markdown structure (headings, links, etc.) | Configurable section/link/code_fence/html_comment inclusion. Reports frontmatter, tables_detected. |
| `code_fence_extract` | Extract code fences from markdown | Optional language filter. Reports unclosed_fences. |

### Patch (5 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `patch_apply_check` | Check if a unified diff applies cleanly | Reports hunks_total/applied/failed, affected_line_ranges, newline_style_before/after, result_fingerprint. |
| `patch_summary` | Summarize a unified diff | Reports files_changed, additions, deletions, renames_detected, binary_patch_detected. |
| `edit_preflight` | Pre-check edit operations (composite) | See [Composite Tools](#composite-tools) section above. |
| `diff_risk_classify` | Classify diff risk level | Uses `classify_diff_path` to bucket files. |
| `patch_contract_check` | Classify diff by contract-relevant categories | Detects scope_escape, lockfile_change, manifest_change, ci_change, config_change, generated_change, vendor_change, source_change. Checks for large deletions (>200 lines). |

### Config (4 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `dotenv_validate` | Validate .env files | Custom key_pattern regex (safety-checked), allow_export, duplicate_policy (warn/error/allow). Runs on dedicated thread with 5s timeout (ReDoS protection). |
| `ini_validate` | Validate INI files | Reports sections, keys_by_section, duplicates, invalid_lines. |
| `config_preflight` | Pre-check config files (composite) | See [Composite Tools](#composite-tools) section above. |
| `toml_shape_tool` | Describe TOML structure | Reports top_level_keys, tables, truncated, summary. |

### Identifier (3 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `identifier_analyze` | Analyze identifier naming conventions | Validates against Python/Rust/JavaScript/env rules. Reports classification, suggestions, warnings. |
| `identifier_inspect` | Inspect identifiers for collisions | Detects casefold collisions, normalization collisions, confusable characters. Reports per-identifier warnings. |
| `identifier_table_inspect` | Inspect identifier tables | Batch collision detection across multiple identifiers with file/line metadata. Checks: casefold, normalization, confusable, style, reserved, mixed_style. Reports collision groups, reserved_keyword_hits, mixed_style_groups. |

### Unicode (2 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `unicode_policy_check` | Check text against Unicode policies | Policies: `identifier_strict`, `filename_safe`, `source_code`, `human_text`, `json_key`, `domain_like`. Optional normalization pre-check. |
| `canonicalize_text` | Normalize text to a Unicode profile | Profiles: `source_file_identity`, `identifier_compare`, `human_label_compare`, `json_key_compare`, `path_segment_compare`. Reports operations_applied, fingerprint_before/after. |

### Version (2 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `version_compare` | Compare semver versions | Schemes: `semver`, `pep440`, `loose`. Reports comparison result, validity. |
| `version_constraint_check` | Check if version satisfies a constraint | Schemes: `semver`, `cargo`. Reports satisfies, parsed_version, parsed_constraint, explanation. |

### Cargo (1 tool)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `cargo_toml_inspect` | Inspect Cargo.toml structure | Checks workspace, dependencies, path dependencies, suspicious/confusable names. |

### Dependency (1 tool)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `dependency_edit_preflight` | Pre-check dependency manifest edits (composite) | See [Dependency Ecosystem Detection](#dependency-ecosystem-detection) below. |

### Diagnostics (3 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `runtime_diagnostics` | Print runtime diagnostic info (harness-only) | Reports active_profile, tool_count, budget_tier_summary, runtime limits, generated data status. |
| `profile_inspect` | Inspect active profile and tool counts (harness-only) | Reports tool_count, model/harness_visible counts, route_critical status, warnings. |
| `tool_availability_explain` | Explain tool availability per profile/audience (harness-only) | Reports exists, available_in_profile, callable_by_audience, exposure, suggested_tool/profile/audience. |

### Repo (5 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `repo_manifest_inspect` | Inspect repository manifest | Detects Rust/Python/Node/Go/mixed project types. Classifies manifests, configs, lockfiles. Generates tool_hints per ecosystem. |
| `config_file_inspect` | Inspect config files with secret masking (composite) | Detects format (json, toml, yaml, dotenv, ini, cargo_toml, package_json, pyproject). Scans for: secret-like keys (masked via `mask_secret_preview`), insecure URLs (http:// non-localhost), debug flags, command hooks, TLS disabled, wildcard hosts. Policy overrides: allow_debug_flags, allow_insecure_urls, allow_command_hooks. |
| `repo_tree_summarize` | Summarize repository file tree | Uses `classify_paths` to bucket all paths. Reports project_types, entrypoint_candidates, high_leverage_paths, tool_hints. Flags missing lockfiles, high generated/vendor percentages. |
| `test_command_suggest` | Suggest verification commands from repo paths | Generates commands per ecosystem (cargo check/test/fmt/clippy, pytest/ruff, npm test/lint, go build/test/vet) with confidence scores. |
| `repo_language_detect` | Detect languages/ecosystems from repo tree | Extension-based detection for 26 language categories. Reports file_count, extensions, confidence per language. Detects ecosystems from manifest files. |

### Analysis (4 tools)

| Tool | Description | Notable Details |
|------|-------------|-----------------|
| `import_export_inspect` | Extract import/export statements from source | Auto-detects language (Rust/Python/JS/Go). Extracts use/pub_use/mod/extern_crate, from_import/import/import_alias, import_from/export/require, import/import_blank. Reports limitations per language. |
| `code_block_map` | Return top-level block ranges from source/markdown | Brace-based for Rust/JS/TS/Go (fn, struct, enum, impl, trait, mod, function, class, arrow_function, func, type). Indentation-based for Python (def, async_def, class). Fenced code blocks and headings for markdown. |
| `symbol_name_diff` | Compare old/new source for symbol changes | Detects added/removed/unchanged symbols. Rename detection via LCS-based name similarity (configurable threshold, default 0.6). |
| `lockfile_inspect` | Inspect lockfile diffs for dependency-change signals | Ecosystem detection: cargo, npm, pnpm, yarn, poetry, go, uv. Extracts packages from before/after/diff. Reports added/removed/updated with version and source info. Flags git/path dependencies. |

## Budget Integration

High-risk handlers create a `BudgetContext` at the start:

```rust
let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::HEAVY);
```

Budget tiers:

| Tier | Used By | Max Elapsed |
|------|---------|-------------|
| `CHEAP` | repo_language_detect, test_command_suggest, import_export_inspect, code_block_map, symbol_name_diff | Short |
| `MODERATE` | patch_apply_check, patch_summary, patch_contract_check, dependency_edit_preflight, identifier_table_inspect, text_diff_explain, lockfile_inspect, regex_finditer, repo_tree_summarize | Medium |
| `HEAVY` | edit_preflight, command_preflight, config_preflight, text_security_inspect, structured_data_compare, config_file_inspect | Long |

Handlers call `budget_ctx.should_stop()` at key pipeline stages. If it returns true, they return `budget_ctx.check_should_stop("tool_name").unwrap_err()` which produces a timeout error response with the appropriate machine code.

The MCP server creates an `Arc<AtomicBool>` cancel flag and attaches it via `with_cancellation()` before dispatch. On timeout, the flag is set but blocking work may continue (cooperative, not forceful).

## Command Policy Engine

The command policy engine classifies shell commands into three dispositions:

| Disposition | Meaning |
|-------------|---------|
| `Allow` | Command is permitted under this policy |
| `Review` | Needs human review |
| `Block` | Always blocked |

### classify_default

| Program | Allow Subcommands | Review Subcommands | Block |
|---------|-------------------|-------------------|-------|
| `rg`, `grep`, `find`, `ls`, `cat`, `head`, `tail`, `wc`, `file`, `which`, `where`, `type`, `echo`, `printf`, `realpath`, `pwd`, `readlink`, `stat`, `du`, `df`, `id`, `whoami`, `uname`, `date` | All | — | — |
| `cargo`, `rustc`, `rustup` | check, test, clippy, doc, search, version, list, tree, loc | fmt, fix, clean, publish, build, bench | — |
| `git` | status, diff, log, show, describe, rev-parse, remote, tag, blame, shortlog | add, checkout, restore, merge, rebase, cherry-pick, commit, fetch, pull, push, reset, revert, clean, config, branch, stash | — |
| `npm`, `yarn`, `pnpm`, `bun` | list, ls, outdated, version | install, ci, update, add, remove, run | — |
| `pip`, `pip3` | — | install, uninstall, upgrade | — |
| `python`, `python3` | — | -c, -m, install, uninstall, upgrade | — |
| `curl`, `wget`, `http`, `https` | — | All | — |
| `rm`, `rmdir`, `shred`, `wipefs`, `chmod`, `chown`, `chgrp`, `dd`, `mkfs`, `fdisk`, `parted`, `sudo`, `su`, `doas`, `pkexec` | — | — | All |
| `sh`, `bash`, `zsh`, `ksh`, `dash`, `fish`, `ash`, `busybox`, `node`, `deno`, `ruby`, `perl`, `lua`, `php`, `Rscript` | — | All | — |
| `make`, `just`, `task` | — | All | — |
| `kill`, `pkill`, `killall`, `xkill`, `nohup`, `screen`, `tmux`, `setsid` | — | All | — |
| _unknown_ | — | All | — |

### classify_strict

More restrictive. Only explicitly safe commands are allowed:

| Program | Allow | Review | Block |
|---------|-------|--------|-------|
| `cargo`, `rustc`, `rustup` | check, test, clippy, fmt, doc, search, version, list, tree, loc | Everything else | — |
| `git` | status, diff, log, show, describe, rev-parse | Everything else | — |
| `rg`, `grep`, `find`, `ls`, `cat`, `head`, `tail`, `wc`, `which`, `echo`, `pwd`, `uname`, `date` | All | — | — |
| `rm`, `rmdir`, `shred`, `chmod`, `chown`, `chgrp`, `dd`, `mkfs`, `sudo`, `su`, `doas` | — | — | All |
| `sh`, `bash`, `zsh`, `ksh`, `dash`, `fish`, `node`, `deno`, `ruby`, `perl`, `lua`, `php`, `Rscript` | — | All | — |
| `make`, `just`, `task` | — | All | — |
| _unknown_ | — | All | — |

### classify_permissive

Only blocks clearly destructive patterns. Wrapper programs with code-execution flags are reviewed:

| Program | Allow | Review | Block |
|---------|-------|--------|-------|
| `rm`, `rmdir`, `shred`, `wipefs`, `dd`, `mkfs`, `fdisk`, `parted`, `chmod`, `chown`, `chgrp`, `sudo`, `su`, `doas`, `pkexec` | — | — | All |
| `sh`, `bash`, `zsh`, `ksh`, `dash`, `fish`, `node`, `deno`, `bun`, `ruby`, `perl`, `lua`, `php`, `Rscript`, `python`, `python3` | Non-code flags | `-c`, `-e`, `-x`, `-l`, `-eval` | — |
| _everything else_ | All | — | — |

### Behavioral Features

`detect_behavioral_features(argv, features)` scans for:

| Feature | Programs/Patterns | Machine Code |
|---------|-------------------|--------------|
| Network access | curl, wget, http, https, nc, ncat, socat, ssh, scp, sftp, rsync, telnet, ftp, nmap, ping, traceroute, dig, nslookup, host | `SHELL_NETWORK_ACCESS` |
| Filesystem write | rm, rmdir, shred, wipefs, dd, mkfs, fdisk, parted; subcommands: write, create, delete, remove, unlink | `SHELL_FILESYSTEM_WRITE` |
| Privilege escalation | sudo, su, doas, pkexec, runas | `SHELL_PRIVILEGE_ESCALATION` |
| Process control | kill, pkill, killall, xkill, nohup, screen, tmux, setsid | `SHELL_PROCESS_CONTROL` |
| Environment mutation | FOO=bar pattern in argv (arg contains `=` and doesn't start with `-` or `/`) | `SHELL_ENV_MUTATION` |
| Command substitution | `has_command_substitution` from shell_split features | `SHELL_COMMAND_SUBSTITUTION` |
| Redirection | `has_redirection` from shell_split features | `SHELL_REDIRECTION` |
| Pipeline | `has_pipe` from shell_split features | `SHELL_PIPELINE` |
| Background execution | `has_background` from shell_split features | `SHELL_BACKGROUND_EXECUTION` |

### Destructive Patterns

`check_destructive_patterns(command, argv)` detects:

| Pattern | Example | Finding |
|---------|---------|---------|
| Pipe-to-shell | `curl ... \| sh` | `PipeToShell` |
| Destructive remove | `rm -rf /` or `rm -rf .` | `DestructiveRemove` |
| Git reset hard | `git reset --hard` | `DestructiveGitReset` |
| Git clean force | `git clean -fdx` | `DestructiveGitClean` |
| Force push | `git push --force` | `ForceGitPush` |
| Permissive chmod | `chmod -R 777` | `PermissiveChmod` |
| Recursive chown | `chown -R` | `RecursiveChown` |

### policy_config Override

The `policy_config` JSON object supports:

| Field | Type | Effect |
|-------|------|--------|
| `deny_commands` | `string[]` | Commands in this list → `SHELL_UNAPPROVED_COMMAND` (BLOCKING) |
| `allow_commands` | `string[]` | If non-empty, only these commands are allowed (others → CAUTION) |
| `deny_subcommands` | `{program: string[]}` | Subcommand-level deny rules |
| `max_command_length` | `int` | Override MAX_TEXT_LENGTH for command length |
| `allow_network` | `bool` | Suppress NetworkAccess findings |
| `allow_filesystem_write` | `bool` | Suppress FilesystemWrite findings |
| `allow_process_control` | `bool` | Suppress ProcessControl findings |
| `allow_env_mutation` | `bool` | Suppress EnvMutation findings |

## Dependency Ecosystem Detection

`dependency_edit_preflight` detects ecosystem from file path and content:

### Detection

| File Path Pattern | Ecosystem |
|-------------------|-----------|
| `*Cargo.toml` | `rust` |
| `*pyproject.toml`, `*requirements.txt`, `*setup.cfg`, `*setup.py` | `python` |
| `*package.json`, `*package-lock.json` | `node` |
| Content contains `[package]` or `[dependencies]` | `rust` |
| Content contains `[project]` or `[build-system]` | `python` |
| Content starts with `{` and contains `"dependencies"` | `node` |

### Rust-Specific Detection

- Parses `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` via TOML parser with line-based fallback
- Detects `[target.'cfg(...)'.dependencies]` and `[workspace.dependencies]`
- Identifies dependency sources: `registry`, `git`, `path`, `workspace`
- Detects new build scripts (`[package].build`)
- Detects `[patch]` and `[replace]` sections
- Policy: `allow_path_deps` (default true), `allow_git_deps` (default false), `allow_patch_sections` (default false)

### Python-Specific Detection

- Parses `[project].dependencies`, `[project].optional-dependencies`, `[build-system].requires`
- Detects requirements.txt entries: standard deps, editable installs (`-e`), URL deps, local paths, unconstrained specs, constraint/include flags (`-c`, `-r`, `-f`)
- Detects pyproject.toml build-backend changes
- Reports direct URL dependencies and editable installs

### Node-Specific Detection

- Parses `dependencies`, `devDependencies`, `peerDependencies`, `optionalDependencies`
- Detects URL/tarball/git specifiers (`http://`, `git+`, `github:`, `gitlab:`, `bitbucket:`, `.tgz`)
- Detects risky npm scripts: `install`, `postinstall`, `preinstall`, `prepare`, `uninstall`
- Detects `packageManager` field changes

### Findings

| Machine Code | Severity | Meaning |
|--------------|----------|---------|
| `DEPENDENCY_ADDED` | MEDIUM | New dependency added |
| `DEPENDENCY_REMOVED` | MEDIUM | Dependency removed |
| `DEPENDENCY_VERSION_WIDENED` | LOW | Version constraint changed (exact→range, ^→range) |
| `DEPENDENCY_GIT_SOURCE` | MEDIUM | Git/URL/tarball dependency detected |
| `DEPENDENCY_PATH_SOURCE` | MEDIUM | Path dependency detected |
| `DEPENDENCY_BUILD_SCRIPT` | MEDIUM | Build script or hook changed |
| `DEPENDENCY_PATCH_OVERRIDE` | HIGH (policy-blocked) | Patch/replace section added against policy |
