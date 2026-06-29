# MCP Server Architecture

The `src/mcp/` module implements a JSON-RPC 2.0 server over stdio for AI coding agent integration.

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `server.rs` | ~4,000 | Protocol handling, tool dispatch, registration tables |
| `tools.rs` | varies | Tool implementation functions (thin wrappers) |
| `schemas.rs` | varies | JSON-RPC types, ToolResponse, error sanitization |
| `mod.rs` | small | Module re-exports |

## Protocol

- Transport: stdio (stdin/stdout)
- Protocol: JSON-RPC 2.0
- MCP version: `2024-11-05`
- Server identity: `eggsact`

### Supported Methods

| Method | Description |
|--------|-------------|
| `initialize` | Returns server info and capabilities |
| `notifications/initialized` | Client acknowledgment (no response) |
| `tools/list` | Returns all 64 tool definitions |
| `tools/call` | Executes a tool by name |

## Tool Registration (4 Tables)

All 4 must be kept in sync. A test (`tool_registration_tables_are_in_sync`) catches drift.

### 1. TOOL_HANDLERS (line ~30)

Static dispatch table: `&[(&str, ToolHandlerFn)]` mapping tool names to handler functions.

### 2. TOOL_METADATA (line ~97)

`LazyLock<HashMap<&'static str, ToolMetadata>>` with rich metadata:
- `category`: Tool grouping (math, text, validation, json, regex, etc.)
- `tier`: 0=essential, 1=common, 2=advanced, 3=specialized
- `profiles`: Feature profiles
- `tags`: Searchable tags
- `llm_exposure`: "full", "indirect", "internal"
- `composite`: Whether tool calls other tools internally

### 3. list_tools_raw() (line ~1379)

Returns `Vec<ToolDefinition>` with full input schemas for each tool.

### 4. OUTPUT_SCHEMAS (line ~1310)

`LazyLock<HashMap<&'static str, Value>>` with JSON Schema for tool output.

## Tool Categories (64 tools)

| Category | Count | Tools |
|----------|-------|-------|
| text | 17 | text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window, text_transform, text_replace_check, text_security_inspect, escape_text, unescape_text, prompt_input_inspect, line_range_extract, line_range_compare |
| json | 7 | validate_json, json_extract, json_compare, json_canonicalize, json_query, json_shape, structured_data_compare |
| math | 4 | math_eval, unit_convert, unit_info, constant_lookup |
| validation | 5 | validate_json, validate_brackets, validate_regex, validate_toml, validate_schema_light |
| path | 4 | path_normalize, path_analyze, path_compare, path_scope_check |
| shell | 4 | shell_split, shell_quote_join, argv_compare, command_preflight |
| regex | 3 | validate_regex, regex_safety_check, regex_finditer |
| list | 3 | list_compare, list_dedupe, list_sort |
| markdown | 2 | markdown_structure, code_fence_extract |
| patch | 3 | patch_apply_check, patch_summary, edit_preflight |
| config | 3 | dotenv_validate, ini_validate, config_preflight |
| identifier | 3 | identifier_analyze, identifier_inspect, identifier_table_inspect |
| unicode | 2 | unicode_policy_check, canonicalize_text |
| version | 2 | version_compare, version_constraint_check |
| toml | 1 | toml_shape |
| cargo | 1 | cargo_toml_inspect |

## Composite Tools

Tools marked `composite: true` orchestrate other tools internally:

| Tool | What it does |
|------|-------------|
| `text_security_inspect` | Calls multiple text inspection tools and aggregates results |
| `edit_preflight` | Pre-checks an edit operation using text tools |
| `command_preflight` | Pre-checks a shell command using shell/identifier tools |
| `config_preflight` | Pre-checks a config file using validation tools |
| `structured_data_compare` | Uses json_compare and list tools for structured data |

## Rate Limiting

- `MAX_REQUESTS_PER_SECOND`: 10
- `MAX_CANCELLED_REQUESTS`: 10,000
- `MAX_TOOL_TIMEOUT_SECONDS`: 30
- `MAX_TOOL_WORKERS`: 16

## Error Handling

Tool errors return `ToolResponse` with `ok: false`:
```json
{
  "ok": false,
  "tool": "math_eval",
  "error_type": "evaluation_error",
  "error": "Division by zero",
  "hints": ["Check for zero denominators"]
}
```

JSON-RPC level errors use standard codes:
- `-32601`: Method not found
- `-32600`: Invalid request
- `-32602`: Invalid params

## Error Types

| Error Type | Description |
|------------|-------------|
| `input_too_large` | Input exceeds size limit |
| `invalid_arguments` | Missing or malformed parameters |
| `validation_error` | Enum out of range, invalid input |
| `evaluation_error` | Math evaluation failed |
| `conversion_error` | Unit conversion impossible |
| `parse_error` | JSON/TOML parsing failed |
| `unknown_tool` | Tool name not found |
