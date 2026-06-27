# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **BUG-001 / B1**: Raised `MAX_FACTORIAL` from 170 → 1000 to match
  Python's `math.factorial` upper bound.
- **BUG-002 / B2**: `factorial()` / `perm()` now use base-1e9
  big-integer arithmetic and surface exact results via a new
  `__int_result__` sentinel (MCP `type: "int"`, no f64 rounding).
- **BUG-003 / B3**: `polar()` accepts the common `polar(r, phi)` two-
  arg form and returns the `(r, phi)` tuple string. Single-arg form
  still works (Python `cmath.polar` semantics).
- **BUG-004 / B4**: `rect(r, phi)` now returns `(r·cos(phi),
  r·sin(phi))` to match Python's `cmath.rect`, which produces a
  complex number.
- **BUG-006 / B6**: `three point one four` parses as `3.14`. The
  `POINT_RE` / `MERGE_DECIMAL_RE` pass now runs before
  `combine_consecutive_number_words()` so the trailing `one` isn't
  consumed as a number word.
- **BUG-007 / B7**: Compact temperature conversions like `100c in f`
  and `100 rankine in celsius` work. `TEMP_CONVERSION_RE` accepts
  zero-width whitespace between number and unit, and a new
  `resolve_unit_canon()` does case-insensitive alias lookup in
  `handle_convert_value()`.
- **BUG-008 / B8**: `math_eval` returns `96.56 km/h` for `60 mph in
  km/h`. The `run()` pipeline is the canonical evaluation path for
  both CLI and MCP.
- **BUG-009 / B9**: Spaced and compound unit expressions now parse:
  `60 mph + 60 km/h`, `60 miles per hour`, `60 kph`, `60 kph + 30 mph`,
  `60 meter per second`, `1 mile per minute`, `60 km per hr`. New
  `BARE_COMPOUND_UNITS`, `PER_UNIT_RE`, `BARE_SIMPLE_UNIT_RE`, and
  `UNIT_INLINE_RE` patterns plus a rewritten `preprocess_units()`
  handle the spacing and `per`/`kph` variants.

### Tests
- 33 `test_bug00{1..9}_*` regression tests in
  `eggsact/tests/calc/test_bug_regression.rs`.
- Cross-binary parity assertions in
  `eggsact/tests/parity/test_bug_fixes.rs`.
- 168 edge-case tests in `eggsact/tests/mcp/test_edge_cases.rs`
  covering math eval (division by zero, overflow, nested parens,
  factorial big-int, polar, rect), unit convert (NaN/Inf rejection,
  temperature extremes, cross-category), text equal (NFC/NFD/NFKC
  normalization, casefold, trim, newline style), text fingerprint
  (casefold, NFC, empty, Unicode), text measure (emoji, combining
  chars, null bytes), text inspect (invisibles, bidi, confusables,
  BOM), JSON tools (deep nesting, special keys, trailing commas),
  shell tools (backslash escape, unterminated quotes), version tools
  (prerelease, build metadata, constraints), list tools (ordered/set/
  multiset modes, dedupe order preservation), path tools (empty,
  root, dotdot traversal), identifier tools (empty, casefold
  collision), regex tools (groups, ReDoS detection), markdown tools
  (empty, code fences), validate tools (brackets, TOML), escape/
  unescape (posix_shell, json, python), line range (out-of-bounds),
  dotenv/ini (empty, quotes, comments), patch tools (empty), text
  truncate (emoji grapheme boundary), glob match, text transform
  (NFC, casefold), text hash (SHA-256, MD5, empty), prompt input
  inspect (instruction phrases, HTML comments), security inspect
  (clean text, machine_code), unit info, structured data compare,
  cargo toml inspect, protocol (float/string IDs, notifications,
  tools/list field validation).

### Known Issues (new)
- **BUG-LRC-001 (B10)**: `line_range_compare` panics on out-of-bounds
  line indices (High).
- **BUG-VC-001 (B11)**: `version_compare` ignores prerelease segments
  (`1.0.0-alpha` == `1.0.0`) (Medium).
- **BUG-JC-001 (B12)**: `json_compare` treats `1.0` ≠ `1` (int vs
  float) — intended per JSON spec (Low).

## [0.1.0] - 2026-05-30

### Added

- **CLI binary** with expression evaluation and `--mcp` server mode
- **Library API** with `run()`, `evaluate()`, and `split_at_operators()`
- **Natural language math** parsing ("thirty plus five", "two to the power of ten")
- **Standard math** evaluation with full Python expression syntax
- **Unit conversions** across length, mass, time, volume, temperature, and more
- **Physical and mathematical constants** (pi, e, speed of light, Planck, Avogadro, etc.)
- **Statistical functions** (sum, mean, median, std, variance, min, max, product)
- **Number theory** (gcd, lcm, factorial)
- **MCP server** (stdio JSON-RPC 2.0, protocol version 2024-11-05, server identity `eggsact`)
- **59 MCP tools** across 17 categories:
  - Math & Units (4): math_eval, unit_convert, unit_info, constant_lookup
  - Text Measurement & Comparison (10): text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window
  - Text Transformation (4): text_transform, escape_text, unescape_text, text_replace_check
  - JSON (7): validate_json, json_extract, json_compare, json_canonicalize, json_query, json_shape, validate_schema_light
  - Regex (3): validate_regex, regex_safety_check, regex_finditer
  - Lists (3): list_compare, list_dedupe, list_sort
  - Paths (4): path_normalize, path_analyze, path_compare, path_scope_check
  - Identifiers (3): identifier_analyze, identifier_inspect, identifier_table_inspect
  - Shell (3): shell_split, shell_quote_join, argv_compare
  - Markdown (2): markdown_structure, code_fence_extract
  - Config Files (4): dotenv_validate, ini_validate, validate_toml, toml_shape
  - Patches (2): patch_apply_check, patch_summary
  - Line Ranges (2): line_range_extract, line_range_compare
  - Unicode (3): unicode_policy_check, canonicalize_text, prompt_input_inspect
  - Versioning (3): version_constraint_check, version_compare, cargo_toml_inspect
  - Glob (1): glob_match
  - Security (1): validate_brackets
- **Text processing library** (21 modules): primitives, confusables, diff, measure, validate, transform, position, regex_safety, replace, path, identifier, shell, markdown, glob, config, toml, patch, line_range, unicode_policy, cargo, version
- **Test suite** with 304+ tests (85 unit, 219 integration, including Python parity tests)

### Known Differences from Python eggcalc

- `text_hash`: Rust uses `algorithm` (singular), Python uses `algorithms` (plural)
- `text_position`: Rust is more lenient with invalid values, returns `valid: false` instead of error
- `text_truncate`: Rust uses `max_graphemes` parameter name
- `validate_toml`: Error message formats differ between implementations
