# MCP Server Architecture

The `src/mcp/` module implements a JSON-RPC 2.0 server over stdio for AI coding agent integration.

## Files

| File | Purpose |
|------|---------|
| `server.rs` | Protocol orchestration: stdio read loop, request validation, JSON-RPC dispatch |
| `registry.rs` | Tool registration: `ToolSpec` declarations (single source of truth) |
| `protocol.rs` | JSON-RPC types: `JsonRpcRequest`, `JsonRpcResponse`, `InitializeResult`, error constructors |
| `response.rs` | `ToolResponse` struct, `sanitize_error`, response builders |
| `runtime.rs` | Rate limiter, cancelled requests, timeout constants, profile management |
| `schema_validation.rs` | MCP argument validation against tool input schemas |
| `schemas.rs` | Re-exports from `protocol.rs` and `response.rs` (backward compatibility) |
| `mod.rs` | Module declarations |

Tool implementations live in `src/tools/` (category modules):

| Module | Tools |
|--------|-------|
| `helpers.rs` | Shared constants, utility functions, spawn semaphore |
| `math.rs` | math_eval, unit_convert, unit_info, constant_lookup |
| `text.rs` | text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window, text_transform, text_replace_check, text_security_inspect, escape_text, unescape_text, prompt_input_inspect, line_range_extract, line_range_compare |
| `json.rs` | json_extract, json_compare, json_canonicalize, json_query, json_shape, structured_data_compare |
| `regex.rs` | validate_regex, regex_safety_check, regex_finditer |
| `validation.rs` | validate_json, validate_brackets, validate_toml, validate_schema_light |
| `path.rs` | path_normalize, path_analyze, path_compare, path_scope_check, glob_match |
| `shell.rs` | shell_split, shell_quote_join, argv_compare, command_preflight |
| `list.rs` | list_compare, list_dedupe, list_sort |
| `markdown.rs` | markdown_structure, code_fence_extract |
| `patch.rs` | patch_apply_check, patch_summary, edit_preflight |
| `config.rs` | dotenv_validate, ini_validate, config_preflight |
| `identifier.rs` | identifier_analyze, identifier_inspect, identifier_table_inspect |
| `unicode.rs` | unicode_policy_check, canonicalize_text |
| `version.rs` | version_compare, version_constraint_check |
| `cargo.rs` | cargo_toml_inspect |

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

## Tool Registration (Single Registry)

All tool registration lives in `src/mcp/registry.rs` as `ToolSpec` declarations. This is the single source of truth. A test (`tool_registration_tables_are_in_sync`) catches drift.

### ToolSpec

Each tool is declared with a `ToolSpec` entry in the registry, which specifies:
- **handler**: The function to call (maps to a function in `tools/*.rs`)
- **category**: Tool grouping (math, text, validation, json, regex, etc.)
- **tier**: 0=essential, 1=common, 2=advanced, 3=specialized
- **profiles**: Feature profiles
- **tags**: Searchable tags
- **exposure**: "default", "contextual", "expert_only", "harness_only", "hidden"
- **composite**: Whether tool calls other tools internally
- **input_schema**: JSON Schema for the tool's input parameters
- **output_schema**: JSON Schema for the tool's output

### How tools/list and tools/call work

- `tools/list`: Looks up all `ToolSpec` entries in the registry and returns `Vec<ToolDefinition>` with full input schemas.
- `tools/call`: Delegates tool lookup, profile checking, and argument validation to `ToolRegistry::prepare_tool_call` (shared with the in-process agent API in `src/agent/`). MCP retains its own async dispatch layer (timeout, semaphore, cancellation) around the core handler execution. This avoids duplicating lookup/validation logic between the MCP server and the agent API.

## Tool Categories (64 tools)

| Category | Count | Tools |
|----------|-------|-------|
| text | 18 | text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window, text_transform, text_replace_check, text_security_inspect, escape_text, unescape_text, prompt_input_inspect, line_range_extract, line_range_compare |
| json | 6 | json_extract, json_compare, json_canonicalize, json_query, json_shape, structured_data_compare |
| math | 4 | math_eval, unit_convert, unit_info, constant_lookup |
| validation | 4 | validate_json, validate_brackets, validate_toml, validate_schema_light |
| path | 5 | path_normalize, path_analyze, path_compare, path_scope_check, glob_match |
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

Defined in `src/mcp/runtime.rs`:
- `MAX_REQUESTS_PER_SECOND`: 10
- `MAX_CANCELLED_REQUESTS`: 10,000
- `MAX_TOOL_TIMEOUT_SECONDS`: 30
- `MAX_TOOL_WORKERS`: 16

## Response Contract

Every tool call returns a `ToolResponse` (defined in `src/mcp/response.rs`) with 11 fields:

| Field | Type | When present |
|-------|------|-------------|
| `ok` | bool | always |
| `tool` | string | always |
| `result` | object | `ok=true` |
| `error_type` | string | `ok=false` |
| `error` | string | `ok=false` |
| `hints` | string[] | `ok=false` |
| `warnings` | string[] | optional |
| `limits_applied` | string[] | optional |
| `findings` | object[] | optional |
| `machine_code` | string | when set |
| `recommended_next_tool` | object | optional |

### Error Responses

Non-OK responses use `ToolResponse::error_with_code()` to include a machine-readable code from `src/mcp/machine_codes.rs`:
```json
{
  "ok": false,
  "tool": "math_eval",
  "error_type": "evaluation_error",
  "machine_code": "EVALUATION_ERROR",
  "error": "Division by zero",
  "hints": ["Check for zero denominators"]
}
```

### Machine Codes

All machine code constants live in `src/mcp/machine_codes.rs`. See `architecture/machine-codes.md` for the full code table, finding helpers, severity/disposition/verdict constants, and composite tool verdict patterns.

### JSON-RPC Level Errors

JSON-RPC level errors use standard codes (constructed in `src/mcp/protocol.rs`):
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
