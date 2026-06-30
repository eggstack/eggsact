# eggsact MCP Tool Reference

Complete reference for all 64 tools exposed by the `eggsact` MCP server.

## Overview

| Property | Value |
|----------|-------|
| Protocol version | 2024-11-05 |
| Server name | `eggsact` |
| Server version | 1.1.3 |
| Transport | stdio JSON-RPC 2.0 |
| Total tools | 64 |

The server communicates over stdin/stdout using newline-delimited JSON-RPC 2.0 messages. `tools/call` responses follow MCP shape: JSON-RPC `result.content[0].text` contains a JSON-encoded `ToolResponse` envelope with an `ok` boolean field.

## Transport Protocol

### Request Format

```json
{"jsonrpc": "2.0", "method": "tools/call", "id": 1, "params": {"name": "math_eval", "arguments": {"expression": "2+3"}}}
```

### Success Response Format

```json
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"ok\": true, \"tool\": \"math_eval\", \"result\": {\"value\": \"5\", \"type\": \"int\"}}"}]}}
```

### Error Response Format

```json
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"ok\": false, \"tool\": \"math_eval\", \"error_type\": \"input_too_large\", \"machine_code\": \"INPUT_TOO_LARGE\", \"error\": \"Expression length 10001 exceeds MAX_EXPRESSION_LENGTH 10000\", \"hints\": [\"Reduce expression length\"]}"}],"isError":true}}
```

### Machine Codes

Every non-OK tool response includes a `machine_code` field with a stable, machine-readable code from `src/mcp/machine_codes.rs`. Codes enable programmatic routing (retry, skip, escalate) without parsing human-readable error messages. Structured findings use `code`, `severity`, and `message` fields — see `architecture/machine-codes.md` for the full code table, finding helpers, and design rationale.

### Server Error Format (JSON-RPC level)

```json
{"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "Method not found: unknown_method"}}
```

## Input Limits

| Constant | Value | Applies to |
|----------|-------|------------|
| `MAX_TEXT_LENGTH` | 100,000 characters | All text/string inputs across all tools |
| `MAX_EXPRESSION_LENGTH` | 10,000 characters | `math_eval` expression parameter |
| `MAX_LIST_ITEMS` | 10,000 items | Array parameters in `list_compare`, `list_dedupe`, `list_sort`, `identifier_inspect`, `identifier_table_inspect`, `shell_quote_join` |
| `MAX_REGEX_SAMPLES` | 100 | `validate_regex` samples array |
| `MAX_PATTERN_LENGTH` | 1,000 characters | `regex_safety_check` pattern parameter |

Limits are enforced at the tool level. Exceeding a limit returns an `input_too_large` error with a descriptive message.

## Error Handling

### Tool-Level Error Types

All tool errors use snake_case error types:

| Error Type | Description |
|------------|-------------|
| `input_too_large` | Input exceeds the applicable size limit |
| `invalid_arguments` | Missing or malformed required parameters |
| `validation_error` | Enum value out of range, invalid input combination |
| `evaluation_error` | Math expression evaluation failed |
| `conversion_error` | Unit conversion is not possible between the specified units |
| `parse_error` | JSON or TOML parsing failed |
| `unknown_tool` | Requested tool name not found in handler table |

### JSON-RPC Level Errors

| Code | Meaning |
|------|---------|
| -32601 | Method not found (unknown JSON-RPC method) |

### Validation Error Hints

Many tools return a `hints` array with suggestions when validation fails:

```json
{
  "ok": false,
  "error_type": "validation_error",
  "error": "Unsupported normalization form: XYZ",
  "hints": ["Use one of: raw, NFC, NFD, NFKC, NFKD"]
}
```

## Tool Categories

Tools are grouped into 16 metadata categories covering math, text, JSON, validation, regex, lists, paths, identifiers, shell, markdown, configuration, patches, TOML, Unicode, versioning, and Cargo metadata.

---

## Math & Units

### math_eval

Evaluate arithmetic, unit conversions, constants, and scientific expressions.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `expression` | string | yes | -- | Math expression to evaluate |

**Return:** `{"value": <string>, "type": <string>, "unit": <string|null>, "display": <string|null>}`. `unit` and `display` are present only for unit-bearing results.

**Limits:** `MAX_EXPRESSION_LENGTH` (10,000 chars).

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 1, "params": {"name": "math_eval", "arguments": {"expression": "sqrt(144) + 2**10"}}}

// Response
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"ok\": true, \"tool\": \"math_eval\", \"result\": {\"value\": \"1036\", \"type\": \"int\"}}"}]}}
```

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 2, "params": {"name": "math_eval", "arguments": {"expression": "30m in ft"}}}

// Response
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"ok\": true, \"tool\": \"math_eval\", \"result\": {\"value\": \"98.42519685039369\", \"type\": \"float\", \"unit\": \"ft\", \"display\": \"98.42519685039369 ft\"}}"}]}}
```

---

### unit_convert

Convert a numeric value between two units.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `value` | number | yes | -- | Numeric value to convert |
| `from_unit` | string | yes | -- | Source unit (e.g. `"m"`, `"km"`, `"ft"`) |
| `to_unit` | string | yes | -- | Target unit |

**Return:** `{"value": <number>, "from_unit": <string>, "to_unit": <string>, "factor": <number>}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 3, "params": {"name": "unit_convert", "arguments": {"value": 100, "from_unit": "km", "to_unit": "mi"}}}

// Response
{"jsonrpc": "2.0", "id": 3, "result": {"ok": true, "result": {"value": 62.1371192237334, "from_unit": "km", "to_unit": "mi", "factor": 0.621371192237334}}}
```

---

### unit_info

Look up information about a unit (canonical name, category).

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `unit` | string | yes | -- | Unit identifier to look up |

**Return:** `{"unit": <string>, "canonical": <string|null>, "category": <string|null>, "is_valid": <boolean>}`

---

### constant_lookup

Look up a physical constant by name.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `name` | string | yes | -- | Constant name (e.g. `"speed_of_light"`, `"avogadro"`) |

**Return:** `{"name": <string>, "value": <number>, "symbol": <string>, "display_name": <string>}`

---

## Text Measurement

### text_measure

Measure text properties with detailed breakdowns.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Input string to measure |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return:** `{"bytes_utf8": <int>, "codepoints": <int>, "graphemes": <int>, "words": <int>, "lines": <int>, "nonempty_lines": <int>, "blank_lines": <int>, "warnings": [<string>]}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 4, "params": {"name": "text_measure", "arguments": {"text": "Hello, world!\nSecond line.\n"}}}

// Response
{"jsonrpc": "2.0", "id": 4, "result": {"ok": true, "result": {"bytes_utf8": 27, "codepoints": 27, "graphemes": 27, "words": 3, "lines": 3, "nonempty_lines": 2, "blank_lines": 1, "warnings": []}}}
```

---

### text_equal

Compare two strings with configurable normalization and comparison options.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `a` | string | yes | -- | First string |
| `b` | string | yes | -- | Second string |
| `normalization` | string | no | `"raw"` | Unicode normalization: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `casefold` | boolean | no | `false` | Case-insensitive comparison |
| `trim` | boolean | no | `false` | Trim whitespace before comparing |
| `ignore_newline_style` | boolean | no | `false` | Normalize `\r\n`/`\r` to `\n` |
| `ignore_trailing_whitespace` | boolean | no | `false` | Strip trailing whitespace per line |
| `ignore_final_newline` | boolean | no | `false` | Strip trailing newlines |

**Return:** `{"equal": <boolean>, "classification": <string>}` where `classification` is one of `"exact_match"`, `"case_only"`, `"length_only"`, `"ordinary_text_difference"`.

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 5, "params": {"name": "text_equal", "arguments": {"a": "Hello", "b": "hello", "casefold": true}}}

// Response
{"jsonrpc": "2.0", "id": 5, "result": {"ok": true, "result": {"equal": true, "classification": "case_only"}}}
```

---

### text_diff_explain

Explain string differences with span-level detail.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `a` | string | yes | -- | First string |
| `b` | string | yes | -- | Second string |
| `max_diffs` | integer | no | 20 | Maximum number of diff spans to report |
| `include_codepoints` | boolean | no | `true` | Include Unicode codepoint arrays |
| `include_context` | boolean | no | `true` | Include surrounding context |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return:** `{"classification": <string>, "truncated": <boolean>, "spans": [<{start, end, kind}>], "a_codepoints": [<string>], "b_codepoints": [<string>]}`

The `classification` field indicates the dominant diff type: `"equal"`, `"insertions_only"`, `"deletions_only"`, `"substitutions"`, or `"mixed"`.

---

### text_inspect

Inspect string for hidden characters and confusables.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to inspect |
| `include_codepoints` | boolean | no | `true` | Include Unicode codepoint list |
| `include_confusables` | boolean | no | `true` | Check for confusable characters |

**Return:** `{"hidden_char_count": <int>, "codepoints": [<string>], "confusables": [<string>], "has_confusables": <boolean>, "has_hidden_chars": <boolean>}`

---

### text_count

Count occurrences of a target substring or character with configurable mode and normalization.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to search in |
| `target` | string | no | -- | Non-empty substring or character to count. If omitted, returns a frequency table |
| `count_mode` | string | no | `"codepoint"` | Counting mode: `"codepoint"`, `"grapheme"`, `"byte"`, `"substring"` |
| `normalization` | string | no | `"raw"` | Normalization: `"raw"`, `"NFC"`, `"NFKC"` |

For `codepoint`, `grapheme`, and `byte` modes, `target` is validated after the requested normalization. Use `substring` mode when an NFKC target can expand to multiple characters.

**Return (with target):** `{"count": <int>, "positions": [<int>], "target": <string>, "normalization": <string>, "text_length_codepoints": <int>}`
**Return (without target):** `{<char>: <count>}` frequency table.

---

### text_truncate

Truncate text to a maximum number of grapheme clusters.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to truncate |
| `max_graphemes` | integer | yes | -- | Maximum grapheme count (>= 0) |

**Return:** `{"original_graphemes": <int>, "truncated_graphemes": <int>, "truncated": <boolean>, "text": <string>}`

---

### text_fingerprint

Compute a content fingerprint with configurable normalization.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to fingerprint |
| `unicode` | string | no | `"raw"` | Unicode normalization: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `newline` | string | no | `"raw"` | Newline normalization: `"raw"`, `"LF"` |
| `trim_final_newline` | boolean | no | `false` | Remove trailing newline before hashing |
| `casefold` | boolean | no | `false` | Case-fold before hashing |

**Return:** `{"sha256": <string>, "bytes_utf8": <int>, "codepoints": <int>, "graphemes": <int>, "newline_style": <string>, "normalization": <string>, "summary": <string>}`

---

### text_hash

Compute hashes of text using multiple algorithms.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to hash |
| `algorithms` | array of strings | no | `["sha256"]` | Hash algorithms to use |
| `encoding` | string | no | `"utf-8"` | Text encoding to hash bytes of |

**Return:** `{"encoding": <string>, "bytes": <int>, "codepoints": <int>, "hashes": {<algo>: <hex_string>}, "warnings": [<string>], "summary": <string>}`

---

### text_position

Convert between text position representations (byte offset, codepoint index, line/column, UTF-16 offset).

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to analyze |
| `byte_offset` | integer | no | -- | UTF-8 byte offset to convert from |
| `codepoint_index` | integer | no | -- | Codepoint index to convert from |
| `line` | integer | no | -- | Line number to convert from |
| `column` | integer | no | -- | Column number to convert from |
| `utf16_offset` | integer | no | -- | UTF-16 code unit offset to convert from |
| `line_base` | integer | no | 1 | Base for line numbering (0 or 1) |
| `column_base` | integer | no | 1 | Base for column numbering (0 or 1) |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return (normal/full):** `{"valid": <boolean>, "byte_offset": <int>, "codepoint_index": <int>, "utf16_offset": <int>, "line": <int>, "column": <int>, "line_base": <int>, "column_base": <int>, "char": <string>, "codepoint": <string>, "name": <string>, "line_text_preview": <string>, "error": <string|null>, "summary": <string>}`

---

### text_window

Extract a context window around a position in text.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to extract window from |
| `position` | object | yes | -- | Position specification (see below) |
| `context_lines` | integer | no | 2 | Number of context lines before and after |
| `include_visible_repr` | boolean | no | `true` | Include visible representation of the line |

The `position` object accepts: `kind`, `value`, `byte_offset`, `codepoint_index`, `grapheme_index`, `line`, `column`, `line_base`, `column_base`.

**Return:** `{"position": <object>, "line_text": <string>, "line_visible_repr": <string>, "before": [<string>], "after": [<string>], "newline_style": <string>, "at_codepoint": <string>, "warnings": [<string>]}`

---

## Text Transformation

### text_transform

Apply a sequence of text transformation operations.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to transform |
| `operations` | array of strings | yes | -- | List of operation names to apply |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return (normal/full):** `{"changed": <boolean>, "text": <string>, "operations_applied": [<string>], "removed": <int>, "warnings": [<string>], "summary": <string>}`

---

### escape_text

Escape special characters in text using the specified mode.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to escape |
| `mode` | string | yes | -- | Escape mode (e.g. `"json"`, `"shell"`, `"regex"`) |

**Return:** `{"mode": <string>, "escaped": <string>, "changed": <boolean>, "summary": <string>}`

---

### unescape_text

Unescape escape sequences in text using the specified mode.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to unescape |
| `mode` | string | yes | -- | Unescape mode |

**Return:** `{"mode": <string>, "unescaped": <string>, "changed": <boolean>, "error": <string|null>, "summary": <string>}`

---

### text_replace_check

Check the effect of a text replacement before applying it.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Source text |
| `old` | string | yes | -- | Substring to find |
| `new` | string | yes | -- | Replacement string |
| `mode` | string | no | `"exact"` | Match mode: `"exact"`, `"nfc"`, `"nfkc"`, `"casefold"`, `"whitespace_collapse"` |
| `expected_count` | integer | no | -- | Expected number of replacements (returns error if mismatch) |
| `allow_multiple` | boolean | no | `false` | Allow multiple replacements |
| `newline_policy` | string | no | `"preserve"` | Newline handling: `"preserve"`, `"normalize_lf"`, `"normalize_crlf"` |
| `return_preview` | boolean | no | `false` | Include before/after preview |
| `max_preview_chars` | integer | no | 2000 | Maximum preview characters |

**Return:** `{"match_count": <int>, "unique_match": <boolean>, "expected_count_met": <boolean>, "would_change": <boolean>, "positions": [<int>], "changed_text_fingerprint": <string>, "newline_style_before": <string>, "newline_style_after": <string>, "preview_before": <string|null>, "preview_after": <string|null>, "findings": [<object>]}`

---

## JSON

### validate_json

Validate a JSON string and report structure information.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | JSON text to validate |

**Return:** `{"valid": <boolean>, "error": <string|null>, "line": <int|null>, "column": <int|null>, "position": <int|null>, "type": <string|null>, "top_level_keys": [<string>|null]}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 6, "params": {"name": "validate_json", "arguments": {"text": "{\"name\": \"test\", \"value\": 42}"}}}

// Response
{"jsonrpc": "2.0", "id": 6, "result": {"ok": true, "result": {"valid": true, "error": null, "line": null, "column": null, "position": null, "type": "object", "top_level_keys": ["name", "value"]}}}
```

---

### json_extract

Extract a value from JSON using a JSON Pointer.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | JSON text |
| `pointer` | string | no | `""` | JSON Pointer (RFC 6901) path |
| `max_output_chars` | integer | no | 4000 | Maximum preview characters |

**Return:** `{"valid_json": <boolean>, "found": <boolean>, "pointer": <string>, "value": <any>, "value_type": <string>, "preview": <string>, "truncated": <boolean>}`

When `found` is false, additional fields explain the failure: `missing_at`, `reason` (`"key_not_found"`, `"index_out_of_range"`, `"invalid_pointer_syntax"`), and `available_keys`.

---

### json_compare

Compare two JSON documents with configurable comparison semantics.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `a` | string | yes | -- | First JSON document |
| `b` | string | yes | -- | Second JSON document |
| `ignore_object_order` | boolean | no | `true` | Ignore key ordering in objects |
| `ignore_array_order` | boolean | no | `false` | Ignore element ordering in arrays |
| `numeric_string_equivalence` | boolean | no | `false` | Treat `"42"` and `42` as equal |
| `casefold_keys` | boolean | no | `false` | Case-insensitive key comparison |
| `max_diffs` | integer | no | 50 | Maximum differences to report |

**Return:** `{"equal": <boolean>, "valid_json_a": <boolean>, "valid_json_b": <boolean>, "same_type": <boolean>, "differences": [<{path, kind, a_preview, b_preview}>], "findings": [<{kind, message}>]}`

---

### json_canonicalize

Canonicalize JSON with sorted keys.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | JSON text to canonicalize |
| `sort_keys` | boolean | no | `true` | Sort object keys alphabetically |
| `trailing_newline` | boolean | no | `false` | Append trailing newline |

**Return:** `{"canonical": <string>, "parse_ok": <boolean>, "valid": <boolean>, "warnings": [<string>]}`

---

### json_query

Query a JSON value using a JSON Pointer. Lighter-weight alternative to `json_extract` that returns the full value without preview truncation.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | JSON text |
| `pointer` | string | no | `""` | JSON Pointer path |

**Return:** `{"found": <boolean>, "pointer": <string>, "value": <any>, "type": <string>}`

---

### json_shape

Analyze the structure of a JSON document.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | JSON text |
| `max_depth` | integer | no | 4 | Maximum nesting depth to analyze |
| `max_keys` | integer | no | 100 | Maximum object keys to report |
| `max_array_items` | integer | no | 5 | Maximum array items to sample |

**Return:** `{"valid": <boolean>, "shape": <object>, "truncated": <boolean>, "summary": <string>}`

---

### validate_schema_light

Validate JSON data against a simple inline schema.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | JSON data to validate |
| `schema` | object | yes | -- | Schema definition object |

Supported schema properties: `type`, `required`, `properties`, `additional_properties`, `items`, `min_items`, `max_items`, `min_length`, `max_length`, `pattern`, `enum`.

**Return:** `{"valid": <boolean>, "violations": [<{path, message, value_type, expected_type}>], "truncated": <boolean>, "summary": <string>}`

---

## Regex

### validate_regex

Test a regex pattern against sample strings.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pattern` | string | yes | -- | Regex pattern |
| `samples` | array of strings | yes | -- | Strings to test (max 100) |
| `flags` | array of strings | no | -- | Regex flags |
| `ignore_case` | boolean | no | `false` | Case-insensitive matching |
| `multiline` | boolean | no | `false` | Multiline mode |
| `dotall` | boolean | no | `false` | Dot matches newlines |
| `ascii` | boolean | no | `false` | ASCII-only character classes |

**Return:** `{"valid_pattern": <boolean>, "results": [<{sample, matched, groups}>]}`

---

### regex_safety_check

Check a regex pattern for catastrophic backtracking risks.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pattern` | string | yes | -- | Regex pattern to analyze (max 1,000 chars) |

**Return:** `{"valid_pattern": <boolean>, "risk": <string>, "findings": [<object>]}`

---

### regex_finditer

Find all regex matches in text with position and group information.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pattern` | string | yes | -- | Regex pattern |
| `text` | string | yes | -- | Text to search |
| `flags` | array of strings | no | -- | Regex flags |
| `max_matches` | integer | no | 100 | Maximum matches to return |
| `include_line_column` | boolean | no | `true` | Include line/column positions |
| `include_groups` | boolean | no | `true` | Include capture groups |

**Return:** `{"valid_pattern": <boolean>, "matches": [<{match, span, line?, column?, groups?, groupdict?}>], "truncated": <boolean>, "match_count": <int>, "error": <string|null>}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 7, "params": {"name": "regex_finditer", "arguments": {"pattern": "\\b\\w+@\\w+\\.\\w+\\b", "text": "Contact alice@example.com or bob@test.org"}}}

// Response
{"jsonrpc": "2.0", "id": 7, "result": {"ok": true, "result": {"valid_pattern": true, "matches": [{"match": "alice@example.com", "span": [8, 24], "line": 1, "column": 9, "groups": [], "groupdict": {}}], "truncated": false, "match_count": 1, "error": null}}}
```

---

## Lists

### list_compare

Compare two lists with alignment, near-matches, and detailed difference reporting.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `a` | array of strings | yes | -- | First list |
| `b` | array of strings | yes | -- | Second list |
| `mode` | string | no | `"set"` | Comparison mode: `"ordered"`, `"set"`, `"multiset"` |
| `casefold` | boolean | no | `false` | Case-insensitive comparison |
| `normalization` | string | no | `"NFC"` | Unicode normalization: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `trim` | boolean | no | `false` | Trim whitespace before comparing |
| `include_near_matches` | boolean | no | `false` | Find fuzzy near-matches |
| `near_match_threshold` | integer | no | 2 | Levenshtein distance threshold for near-matches |
| `ignore_order` | boolean | no | `false` | Treat ordered comparisons as unordered |
| `treat_as_multiset` | boolean | no | `false` | Treat set comparisons as multiset (count-aware) |

**Return (set mode):** `{"equal": <boolean>, "only_in_a": [<any>], "only_in_b": [<any>], "missing_in_a": [<any>], "missing_in_b": [<any>], "near_matches": [<{a, b, distance, classification}>], "a_count": <int>, "b_count": <int>, "mode": "set"}`

**Return (ordered mode):** Adds `first_diff_index`, `equal_prefix_length`, `aligned` (array of `{op, a, b, a_index, b_index}`), `duplicates_in_a`, and `duplicates_in_b`.

**Return (multiset mode):** Adds `count_deltas` (map of item to delta count), `common`, `duplicates_in_a`, and `duplicates_in_b`.

---

### list_dedupe

Remove duplicate items from a list.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `items` | array | yes | -- | List of items to deduplicate |
| `normalization` | string | no | `"NFC"` | Unicode normalization: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `casefold` | boolean | no | `false` | Case-insensitive deduplication |
| `stable` | boolean | no | `true` | Preserve first-occurrence order |

**Return:** `{"items": [<any>], "original_count": <int>, "deduped_count": <int>, "duplicates_removed": <int>}`

---

### list_sort

Sort a list of items with configurable normalization.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `items` | array | yes | -- | List of items to sort |
| `normalization` | string | no | `"NFC"` | Unicode normalization: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `casefold` | boolean | no | `false` | Case-insensitive sort |
| `reverse` | boolean | no | `false` | Sort in descending order |

**Return:** `{"items": [<any>], "original_count": <int>, "sorted_count": <int>}`

---

## Paths

### path_normalize

Normalize a file path for the specified platform.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `path` | string | yes | -- | Path to normalize |
| `platform` | string | no | `"posix"` | Target platform: `"posix"`, `"windows"` |
| `collapse_dot_segments` | boolean | no | `true` | Resolve `.` and `..` segments |
| `preserve_trailing_separator` | boolean | no | `false` | Keep trailing separator |

**Return:** `{"normalized": <string>, "is_absolute": <boolean>, "components": [<string>], "warnings": [<string>]}`

---

### path_analyze

Analyze path structure, components, and properties.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `path` | string | yes | -- | Path to analyze |
| `style` | string | no | `"auto"` | Path style detection: `"auto"`, `"posix"`, `"windows"` |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return (normal/full):** `{"input": <string>, "style": <string>, "absolute": <boolean>, "has_traversal": <boolean>, "components": [<string>], "parent": <string>, "name": <string>, "stem": <string>, "suffix": <string>, "suffixes": [<string>], "hidden": <boolean>, "normalized_lexical": <string>, "warnings": [<string>], "summary": <string>}`

---

### path_compare

Compare two paths with configurable normalization.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `left` | string | yes | -- | First path |
| `right` | string | yes | -- | Second path |
| `platform` | string | no | `"posix"` | Platform: `"posix"`, `"windows"` |
| `case_sensitive` | boolean | no | `true` | Case-sensitive comparison |
| `normalize_separators` | boolean | no | `true` | Normalize path separators |
| `collapse_dot_segments` | boolean | no | `true` | Resolve `.` and `..` segments |

**Return:** `{"equal": <boolean>, "left_normalized": <string>, "right_normalized": <string>, "differences": [<object>], "findings": [<object>]}`

---

### path_scope_check

Check whether a target path is inside a root directory.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `root` | string | yes | -- | Root directory path |
| `target` | string | yes | -- | Target path to check |
| `platform` | string | no | `"posix"` | Platform: `"posix"`, `"windows"` |
| `case_sensitive` | boolean | no | `true` | Case-sensitive path comparison |

**Return:** `{"inside_root": <boolean>, "root_normalized": <string>, "target_normalized": <string>, "relative_path": <string>, "escapes_via_dotdot": <boolean>, "absolute_target": <string>, "findings": [<object>]}`

---

## Identifiers

### identifier_analyze

Analyze a single identifier for naming convention, language validity, and suggestions.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Identifier text to analyze |
| `languages` | array of strings | no | -- | Languages to check validity for |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return (normal/full):** `{"text": <string>, "classification": <string>, "python_valid": <boolean>, "python_keyword": <boolean>, "rust_valid": <boolean>, "javascript_valid": <boolean>, "env_valid": <boolean>, "suggestions": [<string>], "warnings": [<string>], "summary": <string>}`

---

### identifier_inspect

Inspect a list of identifiers for naming collisions and confusable characters.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `identifiers` | array of strings | yes | -- | Identifiers to inspect |
| `language` | string | no | `"generic"` | Language: `"generic"`, `"python"`, `"rust"`, `"javascript"`, `"typescript"`, `"json_key"` |
| `normalization` | string | no | `"NFC"` | Unicode normalization: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `casefold` | boolean | no | `false` | Case-insensitive collision detection |
| `check_confusables` | boolean | no | `true` | Check for confusable characters |

**Return:** `{"identifiers": [<{name, normalized, collisions, confusables}>], "collisions": [<{group, identifiers}>]}`

---

### identifier_table_inspect

Inspect a structured table of identifiers for collisions, reserved keyword hits, and mixed style groups.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `identifiers` | array of objects | yes | -- | Identifier entries (each with `name`, optional `kind`, `file`, `line`) |
| `language` | string | no | `"python"` | Language: `"generic"`, `"python"`, `"rust"`, `"javascript"`, `"typescript"`, `"json_key"` |
| `checks` | array of strings | no | -- | Specific checks to run |

**Return:** `{"count": <int>, "collisions": [<object>], "reserved_keyword_hits": [<object>], "mixed_style_groups": [<object>], "findings": [<object>]}`

---

## Shell

### shell_split

Split a shell command string into an argv array.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `command` | string | yes | -- | Shell command string |
| `shell` | string | no | `"posix"` | Shell dialect: `"posix"` |
| `detect_risky_features` | boolean | no | `true` | Detect pipes, redirections, glob patterns, etc. |

**Return:** `{"parse_ok": <boolean>, "argv": [<string>], "argc": <int>, "features": {"has_pipe": <boolean>, "has_redirection": <boolean>, "has_command_substitution": <boolean>, "has_variable_expansion": <boolean>, "has_glob_pattern": <boolean>, "has_control_operator": <boolean>, "has_unbalanced_quotes": <boolean>}, "findings": [<object>]}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 8, "params": {"name": "shell_split", "arguments": {"command": "echo 'hello world' > /tmp/out.txt"}}}

// Response
{"jsonrpc": "2.0", "id": 8, "result": {"ok": true, "result": {"parse_ok": true, "argv": ["echo", "hello world", ">", "/tmp/out.txt"], "argc": 4, "features": {"has_pipe": false, "has_redirection": true, "has_command_substitution": false, "has_variable_expansion": false, "has_glob_pattern": false, "has_control_operator": false, "has_unbalanced_quotes": false}, "findings": []}}}
```

---

### shell_quote_join

Quote and join an argv array into a shell command string.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `argv` | array of strings | yes | -- | Arguments to quote and join |
| `shell` | string | no | `"posix"` | Shell dialect: `"posix"` |

**Return:** `{"command": <string>, "roundtrip_ok": <boolean>, "findings": [<object>]}`

---

### argv_compare

Compare two shell commands or argv arrays for equivalence.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `left_command` | string | no | -- | Left command string (parsed into argv) |
| `right_command` | string | no | -- | Right command string (parsed into argv) |
| `left_argv` | array of strings | no | -- | Left argv array (use instead of command) |
| `right_argv` | array of strings | no | -- | Right argv array (use instead of command) |
| `shell` | string | no | `"posix"` | Shell dialect: `"posix"` |

You must provide either `left_command`/`right_command` or `left_argv`/`right_argv` (or a mix).

**Return:** `{"argv_equal": <boolean>, "left_argv": [<string>], "right_argv": [<string>], "first_difference": <int|null>, "findings": [<object>]}`

---

## Markdown

### markdown_structure

Analyze Markdown structure: headings, links, code fences, HTML comments, frontmatter, tables.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Markdown text to analyze |
| `include_sections` | boolean | no | `true` | Include heading/section info |
| `include_links` | boolean | no | `true` | Include link info |
| `include_code_fences` | boolean | no | `true` | Include code fence info |
| `include_html_comments` | boolean | no | `true` | Include HTML comment info |

**Return:** `{"headings": [<{level, text, line}>], "code_fences": [<{language, line, content}>], "links": [<{text, url, line}>], "html_comments": [<{content, line}>], "frontmatter": <object|null>, "tables_detected": <boolean>, "findings": [<object>]}`

---

### code_fence_extract

Extract code fence blocks from Markdown text, optionally filtered by language.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Markdown text |
| `language` | string | no | -- | Filter to specific language (e.g. `"rust"`, `"python"`) |
| `include_content` | boolean | no | `true` | Include fence body content |

**Return:** `{"blocks": [<{language, line_start, line_end, content}>], "unclosed_fences": [<{line_start}>], "findings": [<object>]}`

---

## Config

### dotenv_validate

Validate a `.env` file format.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | .env file content |
| `allow_export` | boolean | no | `true` | Allow `export KEY=VALUE` syntax |
| `key_pattern` | string | no | `"^[A-Za-z_][A-Za-z0-9_]*$"` | Regex pattern for valid key names (max 1,000 chars) |
| `duplicate_policy` | string | no | `"warn"` | Duplicate key handling: `"warn"`, `"error"`, `"allow"` |

**Return:** `{"parse_ok": <boolean>, "entries": [<{key, value, line}>], "duplicates": [<{key, lines}>], "invalid_lines": [<{line, reason}>], "requires_quoting": [<string>], "contains_expansion_syntax": <boolean>, "findings": [<object>]}`

---

### ini_validate

Validate an INI file format.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | INI file content |
| `duplicate_policy` | string | no | `"warn"` | Duplicate key handling: `"warn"`, `"error"`, `"allow"` |

**Return:** `{"parse_ok": <boolean>, "sections": [<string>], "keys_by_section": {<section>: [<string>]}, "duplicates": [<{section, key, lines}>], "invalid_lines": [<{line, reason}>], "findings": [<object>]}`

---

### validate_toml

Validate a TOML document.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | TOML text |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return:** `{"parse_ok": <boolean>, "valid": <boolean>, "error": <string|null>, "line": <int|null>, "column": <int|null>, "tables": [<string>], "top_level_keys": [<string>]}`

---

### toml_shape

Analyze the structure of a TOML document.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | TOML text |
| `max_tables` | integer | no | 100 | Maximum tables to report |
| `detail` | string | no | `"normal"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return (normal/full):** `{"valid": <boolean>, "top_level_keys": [<string>], "tables": [<string>], "truncated": <boolean>, "summary": <string>}`

---

## Patches

### patch_apply_check

Check whether a unified diff patch can be applied to original text.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `original_text` | string | yes | -- | Original file content (max 200,000 chars) |
| `patch_text` | string | yes | -- | Unified diff patch text (max 100,000 chars) |
| `strict` | boolean | no | `true` | Require exact context matches |
| `return_result_fingerprint` | boolean | no | `true` | Include SHA-256 fingerprint of result |
| `return_result_text` | boolean | no | `false` | Include full result text |

**Return:** `{"patch_parse_ok": <boolean>, "applies": <boolean>, "hunks_total": <int>, "hunks_applied": <int>, "hunks_failed": <int>, "failed_hunks": [<int>], "affected_line_ranges": [<{start, end}>], "newline_style_before": <string>, "newline_style_after": <string>, "result_fingerprint": <string|null>, "result_text": <string|null>, "findings": [<object>]}`

---

### patch_summary

Summarize a unified diff patch: files changed, additions, deletions, renames.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `patch_text` | string | yes | -- | Unified diff patch text (max 100,000 chars) |

**Return:** `{"files_changed": <int>, "hunks_total": <int>, "additions": <int>, "deletions": <int>, "renames_detected": <boolean>, "binary_patch_detected": <boolean>, "line_ranges_by_file": {<file>: [{start, end}]}, "findings": [<object>]}`

---

## Line Ranges

### line_range_extract

Extract a line range from text with metadata.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Source text |
| `start_line` | integer | yes | -- | Start line (inclusive) |
| `end_line` | integer | yes | -- | End line (inclusive) |
| `line_base` | integer | no | 1 | Line numbering base (0 or 1) |
| `include_line_numbers` | boolean | no | `false` | Prefix output lines with line numbers |
| `include_fingerprint` | boolean | no | `true` | Include SHA-256 fingerprint of extracted range |

**Return:** `{"line_count_total": <int>, "start_line": <int>, "end_line": <int>, "valid_range": <boolean>, "text": <string>, "lines": [<string>], "byte_start": <int>, "byte_end": <int>, "char_start": <int>, "char_end": <int>, "newline_style": <string>, "ends_with_newline": <boolean>, "fingerprint": <string>, "findings": [<object>]}`

---

### line_range_compare

Compare the same line range from two different texts.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `left_text` | string | yes | -- | First source text |
| `right_text` | string | yes | -- | Second source text |
| `start_line` | integer | yes | -- | Start line (inclusive) |
| `end_line` | integer | yes | -- | End line (inclusive) |
| `line_base` | integer | no | 1 | Line numbering base (0 or 1) |
| `comparison_mode` | string | no | `"exact"` | Comparison mode: `"exact"`, `"ignore_trailing_whitespace"`, `"normalize_newlines"` |

**Return:** `{"equal": <boolean>, "left_fingerprint": <string>, "right_fingerprint": <string>, "diff_summary": <string>, "first_difference": <object|null>}`

---

## Unicode

### unicode_policy_check

Check text against a named Unicode safety policy.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to check |
| `policy` | string | yes | -- | Policy name: `"identifier_strict"`, `"filename_safe"`, `"source_code"`, `"human_text"`, `"json_key"`, `"domain_like"` |
| `normalization` | string | no | -- | Apply normalization before checking: `"raw"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |

**Return:** `{"pass": <boolean>, "policy": <string>, "normalized_form": <string>, "findings": [<object>], "summary": <string>}`

---

### canonicalize_text

Apply a named canonicalization profile to text.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to canonicalize |
| `profile` | string | yes | -- | Profile: `"source_file_identity"`, `"identifier_compare"`, `"human_label_compare"`, `"json_key_compare"`, `"path_segment_compare"` |
| `return_mapping` | boolean | no | `false` | Include character-level transformation mapping |

**Return:** `{"text": <string>, "changed": <boolean>, "operations_applied": [<string>], "fingerprint_before": <string>, "fingerprint_after": <string>, "findings": [<object>], "mapping": <object|null>}`

---

### prompt_input_inspect

Inspect text for prompt injection risks: hidden characters, instruction phrases, ANSI escapes.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to inspect |

**Return:** `{"has_hidden_chars": <boolean>, "hidden_chars": [<string>], "length": <int>, "findings": [<object>]}`

---

## Versioning

### version_constraint_check

Check whether a version string satisfies a constraint.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `version` | string | yes | -- | Version string (e.g. `"1.2.3"`) |
| `constraint` | string | yes | -- | Constraint string (e.g. `">=1.0.0"`, `"^1.2"`) |
| `scheme` | string | no | `"semver"` | Versioning scheme: `"semver"`, `"cargo"` |

**Return:** `{"satisfies": <boolean>, "parsed_version": <string>, "parsed_constraint": <string>, "scheme": <string>, "explanation": <string>, "findings": [<object>]}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 9, "params": {"name": "version_constraint_check", "arguments": {"version": "2.1.0", "constraint": ">=2.0.0"}}}

// Response
{"jsonrpc": "2.0", "id": 9, "result": {"ok": true, "result": {"satisfies": true, "parsed_version": "2.1.0", "parsed_constraint": ">=2.0.0", "scheme": "semver", "explanation": "2.1.0 >= 2.0.0", "findings": []}}}
```

---

### version_compare

Compare two version strings.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `a` | string | yes | -- | First version string |
| `b` | string | yes | -- | Second version string |
| `scheme` | string | no | `"semver"` | Versioning scheme: `"semver"`, `"pep440"`, `"loose"` |

**Return:** `{"comparison": <string>, "valid": <boolean>, "scheme": <string>, "summary": <string>}`

The `comparison` field is `"less"`, `"equal"`, or `"greater"`.

---

### cargo_toml_inspect

Inspect a `Cargo.toml` file for package metadata, workspace configuration, and dependency analysis.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Cargo.toml content |
| `check_workspace` | boolean | no | `true` | Analyze workspace configuration |
| `check_dependencies` | boolean | no | `true` | Analyze dependencies for issues |

**Return:** `{"parse_ok": <boolean>, "package": <object|null>, "workspace": <object|null>, "dependencies": <object|null>, "path_dependencies": [<object>], "suspicious_dependency_names": [<string>], "duplicate_or_confusable_dependency_names": [<object>], "findings": [<object>]}`

---

## Other

### glob_match

Test whether a path matches a glob pattern.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pattern` | string | yes | -- | Glob pattern (e.g. `"src/**/*.rs"`) |
| `path` | string | yes | -- | Path to test against the pattern |
| `platform` | string | no | `"posix"` | Platform: `"posix"`, `"windows"` |
| `case_sensitive` | boolean | no | `true` | Case-sensitive matching |

**Return:** `{"matched": <boolean>}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 10, "params": {"name": "glob_match", "arguments": {"pattern": "src/**/*.rs", "path": "src/main.rs"}}}

// Response
{"jsonrpc": "2.0", "id": 10, "result": {"ok": true, "result": {"matched": true}}}
```

---

### validate_brackets

Check delimiter balance in text (parentheses, brackets, braces, angle brackets).

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to check for balanced delimiters |

**Return:** `{"balanced": <boolean>, "unmatched_openers": [<string>], "unmatched_closers": [<string>]}`

```json
// Request
{"jsonrpc": "2.0", "method": "tools/call", "id": 11, "params": {"name": "validate_brackets", "arguments": {"text": "fn foo() { let x = (a + b); }"}}}

// Response
{"jsonrpc": "2.0", "id": 11, "result": {"ok": true, "result": {"balanced": true, "unmatched_openers": [], "unmatched_closers": []}}}
```

---

## Security

### text_security_inspect

Inspect text for security concerns: hidden characters, mixed scripts, confusables, and injection patterns.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Text to inspect |
| `policy` | string | no | `"default"` | Policy: `"default"`, `"source_code"`, `"prompt"`, `"markdown"`, `"identifier"` |
| `normalize` | string | no | `"none"` | Normalize before checking: `"none"`, `"NFC"`, `"NFD"`, `"NFKC"`, `"NFKD"` |
| `compare_normalized` | boolean | no | `false` | Compare normalized vs raw form |
| `detail` | string | no | `"summary"` | Detail level: `"summary"`, `"normal"`, `"full"` |

**Return:** `{"pass": <boolean>, "policy": <string>, "findings": [<object>], "summary": <string>}`

---

## Preflight

### edit_preflight

Preview the effect of a text edit before applying it. Supports literal find-replace, patch, and line-range modes.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `original` | string | yes | -- | Original text content |
| `old` | string | no | -- | Text to find (literal mode) |
| `new` | string | no | -- | Replacement text (literal mode) |
| `replacement_mode` | string | no | `"literal"` | Mode: `"literal"`, `"patch"`, `"line_range"` |
| `strict` | boolean | no | `true` | Require exact match |
| `expected_fingerprint` | string | no | -- | SHA-256 fingerprint to verify original |
| `patch_text` | string | no | -- | Unified diff patch (patch mode) |
| `start_line` | integer | no | -- | Start line (line_range mode) |
| `end_line` | integer | no | -- | End line (line_range mode) |

**Return:** `{"ok_to_apply": <boolean>, "match_count": <int>, "unique_match": <boolean>, "preview_before": <string>, "preview_after": <string>, "fingerprint_before": <string>, "fingerprint_after": <string>, "findings": [<object>]}`

---

### command_preflight

Analyze a shell command for safety before execution: detect dangerous patterns, pipes, redirections, and risky features.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `command` | string | yes | -- | Shell command to analyze |
| `platform` | string | no | `"posix"` | Platform: `"posix"`, `"windows"`, `"auto"` |
| `policy` | string | no | `"default"` | Policy: `"default"`, `"strict"`, `"permissive"` |
| `working_directory` | string | no | -- | Working directory context |

**Return:** `{"verdict": <string>, "argv": [<string>], "features": <object>, "risk_level": <string>, "findings": [<object>]}`

---

### config_preflight

Validate a configuration file before writing: detect syntax errors, schema violations, and structural issues.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | yes | -- | Configuration file content |
| `format` | string | no | `"auto"` | Format: `"auto"`, `"json"`, `"toml"`, `"dotenv"`, `"ini"`, `"cargo_toml"` |
| `schema` | object | no | -- | Optional schema for validation |
| `strict` | boolean | no | `false` | Strict validation mode |

**Return:** `{"valid": <boolean>, "format": <string>, "verdict": <string>, "findings": [<object>]}`

---

## Comparison

### structured_data_compare

Compare two structured data strings (JSON or TOML) with configurable comparison semantics.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `a` | string | yes | -- | First data string |
| `b` | string | yes | -- | Second data string |
| `format` | string | no | `"json"` | Format: `"json"`, `"toml"` |
| `ignore_object_order` | boolean | no | `true` | Ignore key ordering |
| `ignore_array_order` | boolean | no | `false` | Ignore element ordering |
| `max_diffs` | integer | no | 50 | Maximum differences to report |

**Return:** `{"equal": <boolean>, "valid_a": <boolean>, "valid_b": <boolean>, "differences": [<object>], "findings": [<object>]}`

---

## Quick Reference Table

| # | Tool | Category | Required Params |
|---|------|----------|-----------------|
| 1 | `math_eval` | Math | `expression` |
| 2 | `unit_convert` | Math | `value`, `from_unit`, `to_unit` |
| 3 | `unit_info` | Math | `unit` |
| 4 | `constant_lookup` | Math | `name` |
| 5 | `text_measure` | Text | `text` |
| 6 | `text_equal` | Text | `a`, `b` |
| 7 | `text_diff_explain` | Text | `a`, `b` |
| 8 | `text_inspect` | Text | `text` |
| 9 | `text_count` | Text | `text` |
| 10 | `text_truncate` | Text | `text`, `max_graphemes` |
| 11 | `text_fingerprint` | Text | `text` |
| 12 | `text_hash` | Text | `text` |
| 13 | `text_position` | Text | `text` |
| 14 | `text_window` | Text | `text`, `position` |
| 15 | `text_transform` | Text | `text`, `operations` |
| 16 | `escape_text` | Text | `text`, `mode` |
| 17 | `unescape_text` | Text | `text`, `mode` |
| 18 | `text_replace_check` | Text | `text`, `old`, `new` |
| 19 | `validate_json` | Validation | `text` |
| 20 | `json_extract` | JSON | `text` |
| 21 | `json_compare` | JSON | `a`, `b` |
| 22 | `json_canonicalize` | JSON | `text` |
| 23 | `json_query` | JSON | `text` |
| 24 | `json_shape` | JSON | `text` |
| 25 | `validate_schema_light` | Validation | `text`, `schema` |
| 26 | `validate_regex` | Regex | `pattern`, `samples` |
| 27 | `regex_safety_check` | Regex | `pattern` |
| 28 | `regex_finditer` | Regex | `pattern`, `text` |
| 29 | `list_compare` | List | `a`, `b` |
| 30 | `list_dedupe` | List | `items` |
| 31 | `list_sort` | List | `items` |
| 32 | `path_normalize` | Path | `path` |
| 33 | `path_analyze` | Path | `path` |
| 34 | `path_compare` | Path | `left`, `right` |
| 35 | `path_scope_check` | Path | `root`, `target` |
| 36 | `identifier_analyze` | Identifier | `text` |
| 37 | `identifier_inspect` | Identifier | `identifiers` |
| 38 | `identifier_table_inspect` | Identifier | `identifiers` |
| 39 | `shell_split` | Shell | `command` |
| 40 | `shell_quote_join` | Shell | `argv` |
| 41 | `argv_compare` | Shell | _(none required)_ |
| 42 | `markdown_structure` | Markdown | `text` |
| 43 | `code_fence_extract` | Markdown | `text` |
| 44 | `dotenv_validate` | Config | `text` |
| 45 | `ini_validate` | Config | `text` |
| 46 | `validate_toml` | Validation | `text` |
| 47 | `toml_shape` | TOML | `text` |
| 48 | `patch_apply_check` | Patch | `original_text`, `patch_text` |
| 49 | `patch_summary` | Patch | `patch_text` |
| 50 | `line_range_extract` | Text | `text`, `start_line`, `end_line` |
| 51 | `line_range_compare` | Text | `left_text`, `right_text`, `start_line`, `end_line` |
| 52 | `unicode_policy_check` | Unicode | `text`, `policy` |
| 53 | `canonicalize_text` | Unicode | `text`, `profile` |
| 54 | `prompt_input_inspect` | Text | `text` |
| 55 | `version_constraint_check` | Version | `version`, `constraint` |
| 56 | `version_compare` | Version | `a`, `b` |
| 57 | `cargo_toml_inspect` | Cargo | `text` |
| 58 | `glob_match` | Path | `pattern`, `path` |
| 59 | `validate_brackets` | Validation | `text` |
| 60 | `text_security_inspect` | Text | `text` |
| 61 | `edit_preflight` | Patch | `file_path`, `old`, `new` |
| 62 | `command_preflight` | Shell | `command` |
| 63 | `config_preflight` | Config | `file_path`, `text` |
| 64 | `structured_data_compare` | JSON | `left`, `right` |
