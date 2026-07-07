# MCP Server Architecture

The `src/mcp/` module implements a JSON-RPC 2.0 server over stdio for AI coding agent integration.

## Files

| File | Purpose |
|------|---------|
| `server.rs` | Protocol orchestration: stdio read loop, request validation, JSON-RPC dispatch |
| `registry/` | Tool registration: aggregation, listing, types |
| `specs/` | `ToolSpec` declarations per tool category (single source of truth) |
| `protocol.rs` | JSON-RPC types: `JsonRpcRequest`, `JsonRpcResponse`, `InitializeResult`, error constructors |
| `response.rs` | `ToolResponse` struct, `sanitize_error`, response builders |
| `runtime.rs` | Rate limiter, constants, profile management |
| `schema_validation.rs` | MCP argument validation against tool input schemas |
| `compat.rs` | `CompatibilityMode` enum (EggcalcPython vs StrictNative) |
| `machine_codes.rs` | Machine-readable response codes, severity/disposition/verdict constants |
| `budget.rs` | Per-tool budget limits, `BudgetTier` enum, composite sub-budgets, `BudgetContext` with cooperative helpers (`check_not_cancelled`, `check_deadline`, `check_text_bytes`, `check_list_len`, `remaining_time_ms`) |
| `schemas/` | JSON-schema builders per tool category (math, text, json, regex, etc.) |
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
| `path.rs` | path_normalize, path_analyze, path_compare, path_scope_check, glob_match, path_batch_scope_check |
| `shell.rs` | shell_split, shell_quote_join, argv_compare, command_preflight |
| `list.rs` | list_compare, list_dedupe, list_sort |
| `markdown.rs` | markdown_structure, code_fence_extract |
| `patch.rs` | patch_apply_check, patch_summary, edit_preflight, diff_risk_classify |
| `config.rs` | dotenv_validate, ini_validate, config_preflight, toml_shape_tool |
| `identifier.rs` | identifier_analyze, identifier_inspect, identifier_table_inspect |
| `unicode.rs` | unicode_policy_check, canonicalize_text |
| `version.rs` | version_compare, version_constraint_check |
| `cargo.rs` | cargo_toml_inspect |
| `dependency.rs` | dependency_edit_preflight |
| `diagnostics.rs` | runtime_diagnostics |
| `repo.rs` | repo_manifest_inspect, config_file_inspect, repo_tree_summarize |

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
| `notifications/cancelled` | Looks up the request ID in the active-requests map and sets its cancel flag |
| `tools/list` | Returns registered tool definitions (filtered by profile) |
| `tools/call` | Executes a tool by name |
| `profiles/list` | Lists all profiles and their tool counts |
| `ping` | Returns empty response (health check) |

## Tool Registration (Single Registry)

All tool registration lives in `src/mcp/specs/<category>.rs` as `ToolSpec` declarations — one file per tool category. `src/mcp/registry/all_tools.rs` aggregates them into the combined `ALL_TOOLS` using `LazyLock`. Adding a new tool requires editing only the relevant category file in `specs/`.

### ToolSpec

Each tool is declared with a `ToolSpec` entry in the registry, which specifies:
- **handler**: The function to call (maps to a function in `tools/*.rs`)
- **category**: Tool grouping (math, text, validation, json, regex, etc.)
- **tier**: 0=essential, 1=common, 2=advanced, 3=specialized
- **profiles**: Feature profiles
- **tags**: Searchable tags
- **exposure**: Typed `ToolExposure` enum (see Exposure Model below)
- **cost**: Typed `ToolCost` enum (cheap, moderate, heavy)
- **stability**: Typed `ToolStability` enum (stable, deprecated, experimental)
- **composite**: Whether tool calls other tools internally
- **input_schema**: JSON Schema for the tool's input parameters
- **output_schema**: JSON Schema for the tool's output

### Exposure Model

Tools have a typed `ToolExposure` enum that controls visibility:

| Variant | Serialized | Semantics |
|---------|-----------|-----------|
| `Default` | `"default"` | Safe for ordinary model-visible use. Can appear in `default` or `codegg_core_min`. Cheap, easy to explain, unlikely to cause tool overload. |
| `Contextual` | `"contextual"` | Useful when the workflow calls for the category. Not in smallest default lists; exposed when editing, config work, shell planning, Unicode investigation, or repo audit is active. |
| `ExpertOnly` | `"expert_only"` | Specialized tools for manager/reviewer/research agents or explicit expert workflows. |
| `HarnessOnly` | `"harness_only"` | Tools the harness calls automatically but models should not generally see. Safety checks and preflight tools enforced by the harness. |
| `Hidden` | `"hidden"` | Internal or compatibility tools. Not listed except in debug/developer contexts. |

Serialized strings preserve backward compatibility with existing MCP clients.

### Audience Filtering

`ToolListAudience` controls which exposure levels appear in tool listings:

| Audience | Includes | Excludes |
|----------|----------|----------|
| `Model` | Default, Contextual, ExpertOnly | HarnessOnly, Hidden |
| `Harness` | Default, Contextual, ExpertOnly, HarnessOnly | Hidden |
| `Debug` | All non-hidden tools | Hidden only |

Use `tools_for_profile_audience(profile, audience)` to get filtered tool lists.
The in-process agent API (`src/agent/`) should use `Model` audience for ordinary
coder-agent sessions and `Harness` for automatic preflight checks.

**In-process API (`src/agent/`)**: The `ToolRegistry` exposes a
`ToolAudience` enum mirroring `ToolListAudience`. Use
`available_tools_model_safe()` (equivalent to `available_tools_for_audience(ToolAudience::Model)`)
for model-facing codegg integrations, or
`with_profile_and_audience(profile, ToolAudience::Harness)` for harness checks.
Use `available_tools_for_current_audience()` to list tools using the registry's
stored audience without passing it explicitly.

`ToolAudience::can_execute_exposure()` answers whether a given audience may
execute a tool with a specific exposure level. This is enforced at dispatch
time by `ToolRegistry::prepare_tool_call`.

**MCP `tools/list` and `tools/call`**: Both paths enforce profile membership.
`tools/call` also enforces audience/exposure compatibility via
`ToolRegistry::prepare_tool_call` — the active profile is resolved from
`get_active_profile()` and `Model` audience is used by default. This means
MCP `tools/call` rejects harness-only tools for ordinary model-facing calls.
Harness-oriented execution should use the in-process API with explicit
`Harness` audience.

**No per-call profile override**: `tools/call` intentionally does NOT accept
a `profile` parameter in its arguments. The active profile is set once at
server startup via the `EGGCALC_MCP_PROFILE` environment variable and applies
to all subsequent `tools/call` and `tools/list` requests. (`tools/list` does
accept a `profile` parameter for filtering, but that only affects which tools
appear in the listing, not which profile `tools/call` enforces.) This matches
the in-process API where each `ToolRegistry` instance is bound to one
profile at construction time via `with_profile_and_audience`.

### How tools/list and tools/call work

- `tools/list`: Validates MCP parameters in `server.rs`, builds a `ToolListOptions`, and delegates to `registry::list_tool_definitions()` in `registry/listing.rs`. The registry handles profile filtering, name/tier/tag filtering, schema compaction, and deprecated-field normalization. MCP retains parameter validation and profile resolution.
- `tools/call`: Resolves the active profile from `get_active_profile()` and creates a `ToolRegistry` with `Model` audience and `EggcalcPython` compatibility mode (Python-parity error messages). Delegates tool lookup, profile checking, audience/exposure checking, and argument validation to `ToolRegistry::prepare_tool_call` (shared with the in-process agent API in `src/agent/`). MCP retains its own async dispatch layer (timeout, semaphore, cancellation) around the core handler execution. This avoids duplicating lookup/validation logic between the MCP server and the agent API. The in-process agent API defaults to `StrictNative` mode (standard JSON Schema error messages).

## Tool Categories

| Category | Count | Tools |
|----------|-------|-------|
| text | 18 | text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window, text_transform, text_replace_check, text_security_inspect, escape_text, unescape_text, prompt_input_inspect, line_range_extract, line_range_compare |
| json | 6 | json_extract, json_compare, json_canonicalize, json_query, json_shape, structured_data_compare |
| math | 4 | math_eval, unit_convert, unit_info, constant_lookup |
| validation | 4 | validate_json, validate_brackets, validate_toml, validate_schema_light |
| path | 6 | path_normalize, path_analyze, path_compare, path_scope_check, glob_match, path_batch_scope_check |
| shell | 4 | shell_split, shell_quote_join, argv_compare, command_preflight |
| regex | 3 | validate_regex, regex_safety_check, regex_finditer |
| list | 3 | list_compare, list_dedupe, list_sort |
| markdown | 2 | markdown_structure, code_fence_extract |
| patch | 4 | patch_apply_check, patch_summary, edit_preflight, diff_risk_classify |
| config | 3 | dotenv_validate, ini_validate, config_preflight |
| identifier | 3 | identifier_analyze, identifier_inspect, identifier_table_inspect |
| unicode | 2 | unicode_policy_check, canonicalize_text |
| version | 2 | version_compare, version_constraint_check |
| toml | 1 | toml_shape |
| cargo | 1 | cargo_toml_inspect |
| dependency | 1 | dependency_edit_preflight |
| repo | 3 | repo_manifest_inspect, config_file_inspect, repo_tree_summarize |
| diagnostics | 1 | runtime_diagnostics |

## Composite Tools

Tools marked `composite: true` orchestrate other tools internally. All emit a `verdict` field in their result JSON via the `.with_verdict()` builder, and use `finding()` helpers with canonical `severity::*` and `disposition::*` constants.

| Tool | Verdict domain | What it does |
|------|---------------|-------------|
| `edit_preflight` | allow / review / block | Pre-checks an edit operation using text tools. Optionally composes `path_scope_check`, `text_security_inspect`, and `text_fingerprint` (newline detection) when the corresponding input fields are provided. |
| `command_preflight` | allow / review / block | Pre-checks a shell command using a policy engine. Classifies commands via per-policy allow/review/block matrices (`default`, `strict`, `permissive`), detects behavioral features (network, filesystem, process, env, shell features), checks destructive patterns, applies custom `policy_config` allow/deny lists, and runs regex safety on regex-like args. |
| `config_preflight` | valid / valid_with_warnings / invalid | Pre-checks a config file using validation tools |
| `text_security_inspect` | allow / review / block | Calls multiple text inspection tools and aggregates results |
| `cargo_toml_inspect` | allow / review / block | Inspects Cargo.toml structure and naming |
| `structured_data_compare` | — | Uses json_compare and list tools for structured data |

## Route-Critical Tools

A subset of tools are classified as **route-critical** — they produce structured verdicts and machine codes that downstream harnesses depend on for routing decisions. The `is_route_critical()` helper and `ROUTE_CRITICAL_TOOLS` constant in `registry/listing.rs` identify these tools:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `text_security_inspect`

Route-critical tools must always emit a `machine_code` and `verdict` in their response envelope. The `patch_apply_check` tool is `HarnessOnly` exposure and does not appear in model-facing listings.

## Concurrency Model

The MCP stdio server now supports **concurrent request handling**. The read loop
in `server.rs` reads requests from stdin as lines arrive and spawns each request
as an independent tokio task via a `JoinSet`. Responses are sent through an
`mpsc` channel to a dedicated writer task that serializes stdout writes,
preventing interleaved output from concurrent handlers.

**Concurrency limits:**

- `MAX_IN_FLIGHT_REQUESTS` (32): maximum simultaneously active request tasks.
  Exceeding this limit returns a JSON-RPC error (-32600 "Too many in-flight
  requests").
- `MAX_TOOL_WORKERS` (16): semaphore permits for concurrent blocking tool
  executions *within* tasks. This is a back-pressure mechanism for CPU-bound
  tools, not a concurrency driver.
- `MAX_REQUESTS_PER_SECOND` (10): rate limiter on incoming requests.

**Cancellation model:** Each request gets an `Arc<AtomicBool>` cancel flag at
dispatch time. When a `notifications/cancelled` notification arrives for an
active request ID, the flag is set to `true` cooperatively. The flag is shared
between the timeout path (`tokio::time::timeout` in `handle_request_async`) and
the handler (via `budget::with_cancel_flag()` thread-local), so external
cancellation and timeout share the same signal. The handler can check
`BudgetContext::should_stop()` at any pipeline stage to detect cancellation.

**Graceful shutdown:** When stdin reaches EOF, the read loop breaks and the
server drains in-flight requests via `JoinSet::join_next()` before dropping the
channel sender and waiting for the writer task to flush remaining responses.

**Response ordering contract (JSON-RPC):** Because requests are dispatched
concurrently and may complete out of request order, **clients must correlate
responses to requests by JSON-RPC `id`**, not by arrival position. The `id`
field on each response echoes the `id` of the originating request (string,
integer, or null). JSON-RPC clients that implicitly assume ordered responses
will observe races on multi-request sessions. The server intentionally does
not serialize dispatch behind a head-of-line lock, because doing so would
re-introduce the latency bottleneck that the concurrent runtime removed.

Notifications (requests without an `id`) produce no response by JSON-RPC
contract; clients must not expect them in the output stream.

Test helpers in `tests/mcp/test_comprehensive_parity.rs`
(`mcp_request_multi()`) implement id-based correlation explicitly and reorder
responses to match request slice order so existing positional assertions
remain stable. Any future helper that sends multiple requests in one session
must do the same.

For high-throughput preflight calls, codegg should use the **in-process agent
API** (`src/agent/`) rather than the MCP stdio server. The agent API
(`ToolRegistry::call_json()`) is synchronous and avoids the serialization and
IPC overhead of the stdio transport.

## ExecutionContext (`src/agent/mod.rs`)

`ExecutionContext` bundles per-request mutable state into a single struct:

```rust
pub struct ExecutionContext {
    pub eval_ctx: EvalContext,
    pub compatibility_mode: CompatibilityMode,
    pub profile: Option<Profile>,
    pub audience: Option<ToolAudience>,
    pub budget: Option<ToolBudget>,
    pub cancellation: Option<Arc<AtomicBool>>,
    pub request_id: Option<String>,
    pub source: ExecutionSource,
}
```

`ExecutionSource` distinguishes callers: `Cli`, `Library`, `Mcp`, `Agent`, `Test`.

### Constructors

| Method | Profile | Audience | Compatibility | Description |
|--------|---------|----------|---------------|-------------|
| `ExecutionContext::cli_default()` | `None` | `None` | `StrictNative` | CLI invocations |
| `ExecutionContext::library_default()` | `None` | `None` | `StrictNative` | Library consumers |
| `ExecutionContext::mcp_default(profile, audience)` | `Some(profile)` | `Some(audience)` | `EggcalcPython` | MCP server (Python-parity errors) |
| `ExecutionContext::agent_default(profile, audience)` | `Some(profile)` | `Some(audience)` | `StrictNative` | In-process agent calls |
| `ExecutionContext::test_default()` | `None` | `None` | `StrictNative` | Test harness |

Builder methods: `with_eval_context()`, `with_budget()`, `with_cancellation()`, `with_request_id()`. Named-field builder: `ExecutionContext::builder()`.

### Context-aware dispatch

`ToolRegistry::call_json_with_execution_context()` accepts an `ExecutionContext` and honors all its fields:

- **Profile/Audience**: Falls back to registry defaults when `None`. When `Some`, uses the context's values for tool filtering and exposure checks.
- **Compatibility mode**: Used for argument schema validation.
- **EvalContext**: **Cloned** and set as thread-local via `budget::with_eval_context()`, making it available to calculator-backed tools (e.g., `math_eval` uses `run_with_context()` when a thread-local context is present). Mutations inside the handler do not persist back to the caller's `ExecutionContext`. Two calls with identical seeds produce the same first random value.
- **Budget/Cancellation**: Resource limits and cooperative cancellation flag. The cancellation flag is set as a thread-local during dispatch so that high-risk handlers that create their own `BudgetContext` inherit cancellation.

**MCP wire protocol boundary**: `call_json_with_execution_context` is an **in-process** API. It does not change the MCP JSON-RPC wire protocol. The MCP server still resolves its active profile from `EGGCALC_MCP_PROFILE` at init time. Per-request context overrides over the wire would require a future MCP request-level context API.

Legacy APIs remain as backward-compatible wrappers:

| Method | What it wraps |
|--------|---------------|
| `call_json(name, args)` | Creates a default context internally |
| `call_json_with_budget(name, args, budget)` | Context with custom budget |
| `call_json_with_context(name, args, budget, cancel_flag)` | Context with budget and cancellation |
| `call_json_with_execution_context(name, args, ctx)` | Full context — **recommended for new code** |

### MCP startup env vars → runtime context

The MCP server reads `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_AUDIENCE`, and `EGGCALC_MCP_SCHEMA_DETAIL` at startup. These become the `profile`, `audience`, and `compatibility_mode` fields in `ExecutionContext::mcp_default()`. Once set, they apply to all subsequent tool calls — there is no per-call profile override in `tools/call`.

### Handler signatures unchanged

Tool handler functions retain the signature `fn(&Value) -> ToolResponse` for compatibility. This means handlers cannot receive an `ExecutionContext` directly — state isolation is achieved by the caller passing context into `call_json_with_execution_context()`, which applies it at the orchestration layer. Calculator-backed handlers (e.g., `math_eval`) retrieve the `EvalContext` from a thread-local set by `budget::with_eval_context()`. High-risk handlers (`edit_preflight`, `command_preflight`, etc.) create a `BudgetContext` internally for cooperative budget checks.

### What remains global / thread-local

| State | Why global |
|-------|------------|
| `MCP_MODE`, `ALLOW_RANDOM`, `ALLOW_SIDE_EFFECTS` AtomicBool | One-shot startup flags, race-safe. Context-aware APIs bypass these via `EvalContext` fields. |
| `ACTIVE_PROFILE`, `ACTIVE_AUDIENCE`, `ACTIVE_SCHEMA_DETAIL` RwLock | Set once at startup, read-only after init |
| `SPAWN_SEMAPHORE` | Intentional global concurrency limiter |
| `CURRENT_CANCEL_FLAG` thread-local | Properly scoped per-dispatch |
| `CURRENT_EVAL_CONTEXT` thread-local | Set by `with_eval_context()` during calculator-backed tool dispatch; bypasses legacy globals |
| 36+ LazyLock immutable caches | Regex, tables, tool definitions — immutable after init |
| Request-scoped Arc objects | Cloned per-request, not shared |
| `MEMORY_REGISTERS`, `USER_VARIABLES`, `PRNG_STATE`, `GAUSS_SPARE` LazyLock | Legacy mutable state. Context-aware APIs use `EvalContext` fields instead; globals remain for legacy `evaluate()` path. |

These are intentionally global because they represent immutable configuration or startup-time state, not per-request mutable state. The legacy mutable globals (`MEMORY_REGISTERS`, `USER_VARIABLES`, `PRNG_STATE`, `GAUSS_SPARE`) are retained for backward compatibility but are bypassed by context-aware APIs.

## Rate Limiting

Defined in `src/mcp/runtime.rs`:
- `MAX_REQUESTS_PER_SECOND`: 10
- `MAX_IN_FLIGHT_REQUESTS`: 32
- `MAX_TOOL_WORKERS`: 16
- `MAX_REQUEST_ID_LENGTH`: 1024
- `MAX_REQUEST_BYTES`: 1,000,000
- `MAX_OUTPUT_BYTES`: 1,000,000

Tool timeouts are now **budget-derived** rather than using a fixed `MAX_TOOL_TIMEOUT_SECONDS`. Each `ToolSpec` declares a `cost` field (`ToolCost::Cheap`, `Moderate`, `Heavy`), which maps to a `ToolBudget` with per-tool limits including `max_elapsed_ms`. The `budget_for_tool()` function in `src/mcp/budget.rs` resolves the effective budget, and `tools/call` uses `budget.max_elapsed_ms` as the timeout instead of the previous fixed 30s constant.

## Budget-Aware Dispatch

The MCP server applies per-tool resource budgets during `tools/call` dispatch:

### Budget Module (`src/mcp/budget.rs`)

- **`BudgetTier`** enum: `Cheap`, `Moderate`, `Heavy` — maps from `ToolCost` in `ToolSpec`.
- **`ToolBudget`** struct: per-tool resource limits — `max_input_bytes`, `max_output_bytes`, `max_elapsed_ms`, `max_text_bytes`, `max_findings`, `max_list_items`, `max_pattern_length`, `max_regex_pattern_chars`, `max_regex_samples`, `max_spawned_workers`.
- **`budget_for_tool(tool_name)`**: resolves the effective `ToolBudget` for a tool, applying composite overrides when a tool orchestrates other tools internally.
- **Builders**: `ToolBudget::with_max_findings(n)`, `with_max_output_bytes(n)`, `with_max_input_bytes(n)`, `with_max_text_bytes(n)` — used by callers (especially tests) to override a single budget field without rebuilding the whole struct. Direct struct literals remain valid but break ABI when fields are renamed; prefer builders.
- **`BudgetContext`**: runtime context passed into tool handlers — holds a deadline (`Instant`), a `cancelled` flag, and `should_stop()` which checks both deadline expiry and cancellation. Helper methods: `check_not_cancelled(tool_name)`, `check_deadline(tool_name)`, `check_text_bytes(field, text, tool_name)`, `check_list_len(field, len, tool_name)`, `remaining_time_ms()`. `check_text_len` is retained as a deprecated alias for `check_text_bytes` (renamed in 1.1.4 because enforcement is byte-based, not character-based).
- **Composite sub-budgets**: `SubBudget` and `CompositeBudgetAllocator` allow composite tools (e.g., `edit_preflight`, `command_preflight`) to split their parent budget across child tool calls via `sub_budget_context()`.

### Response Truncation (`src/mcp/response.rs`)

`truncate_response()` enforces budget limits on completed tool responses. When a tool produces more findings, output bytes, or text characters than its budget allows, the response is truncated and `limits_applied` is populated with descriptions of what was capped.

- **Findings cap** (`max_findings`): when findings exceed the cap, excess entries are dropped and a synthetic `OUTPUT_TOO_LARGE` notice is appended. The total findings length is always ≤ `max_findings` (the synthetic notice itself reserves one slot, so N findings + 1 synthetic ≤ N+1 entries).
- **Result truncation** (`max_output_bytes`): when the serialized `result` object exceeds the cap, it is **replaced** with a summary object containing only `machine_code`, `verdict`, `ok`, and any caller-supplied `summary` key, plus `truncated: true`, `original_size_bytes`, `max_output_bytes`. This guarantees the wire payload fits the budget while preserving route-critical fields.
- `limits_applied` in the response envelope reports what was truncated.

### Input Pre-Check (`src/agent/mod.rs`)

`call_json_with_budget()` checks the serialized input against `budget.max_input_bytes` **before** dispatching to the handler. Oversized input fails with `INPUT_TOO_LARGE` (high severity, blocking). The MCP server's `tools/call` handler runs `truncate_response` *before* the `MAX_OUTPUT_BYTES` hard cap so response shaping happens once.

### Runtime Metrics (`src/mcp/response.rs`)

`CallMetrics` struct captures per-call resource usage. `CallMetricsBuilder` (via `.with_metrics()`) collects elapsed time, output size, and other metrics during execution, feeding back into budget enforcement.

### Integration

1. `tools/call` resolves `ToolBudget` from `ToolSpec.cost` via `budget_for_tool()`
2. A `BudgetContext` is constructed with a deadline derived from `budget.max_elapsed_ms`
3. An `Arc<AtomicBool>` cancel flag is created and attached via `with_cancellation()`
4. The context is passed to the tool handler; `should_stop()` allows cooperative cancellation at key pipeline stages
5. On timeout, the cancel flag is set before returning a TIMEOUT error (cooperative — blocking work may continue)
6. After the handler returns, `truncate_response()` caps findings/output if the budget was exceeded
7. `limits_applied` in the response envelope reports what was truncated

High-risk handlers (`edit_preflight`, `command_preflight`, `config_preflight`, `config_file_inspect`, `dependency_edit_preflight`) create a `BudgetContext` internally and call `should_stop()` at key pipeline stages (after format detection, before sub-tool dispatches, in iteration loops). Since `ToolHandler` signatures are `fn(&Value) -> ToolResponse`, the MCP server attaches the cancel flag to a shared context that handlers access via internal initialization.

For the in-process agent API, `call_json_with_budget()` on `ToolRegistry` accepts a custom `ToolBudget` to override the default per-tool limits. Input is pre-checked against `max_input_bytes` and rejected with `INPUT_TOO_LARGE` before handler dispatch.

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
| `recommended_next_tool` | `{name, reason, arguments_hint}` | optional | Structured next-tool suggestion |

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

## Profiles

Profiles control which tools are available. The `full` profile includes all non-hidden tools. Named profiles include specific tool subsets.

### Profile Reference

<!-- BEGIN GENERATED: profile reference -->
| Profile | Model Tools | Harness Tools | Model Tool Names | Harness-Only Tools |
|---------|-------------|---------------|------------------|--------------------|
| `full` | 64 | 71 | `argv_compare`, `canonicalize_text`, `cargo_toml_inspect`, `code_fence_extract`, `command_preflight`, `config_file_inspect`, `config_preflight`, `constant_lookup`, `dependency_edit_preflight`, `diff_risk_classify`, `dotenv_validate`, `edit_preflight`, `escape_text`, `glob_match`, `identifier_analyze`, `identifier_inspect`, `identifier_table_inspect`, `ini_validate`, `json_canonicalize`, `json_compare`, `json_extract`, `json_query`, `json_shape`, `line_range_compare`, `line_range_extract`, `list_compare`, `list_dedupe`, `list_sort`, `markdown_structure`, `math_eval`, `patch_summary`, `path_analyze`, `path_compare`, `path_normalize`, `regex_finditer`, `regex_safety_check`, `repo_manifest_inspect`, `repo_tree_summarize`, `shell_quote_join`, `structured_data_compare`, `text_count`, `text_diff_explain`, `text_equal`, `text_fingerprint`, `text_hash`, `text_inspect`, `text_measure`, `text_position`, `text_replace_check`, `text_security_inspect`, `text_transform`, `text_truncate`, `text_window`, `toml_shape`, `unescape_text`, `unit_convert`, `unit_info`, `validate_brackets`, `validate_json`, `validate_regex`, `validate_schema_light`, `validate_toml`, `version_compare`, `version_constraint_check` | `patch_apply_check`, `path_batch_scope_check`, `path_scope_check`, `prompt_input_inspect`, `runtime_diagnostics`, `shell_split`, `unicode_policy_check` |
| `default` | 25 | 25 | `escape_text`, `glob_match`, `identifier_inspect`, `json_canonicalize`, `json_compare`, `line_range_extract`, `list_dedupe`, `list_sort`, `math_eval`, `path_normalize`, `regex_finditer`, `regex_safety_check`, `text_count`, `text_diff_explain`, `text_equal`, `text_fingerprint`, `text_inspect`, `text_measure`, `text_replace_check`, `text_window`, `unescape_text`, `validate_brackets`, `validate_json`, `validate_regex`, `validate_toml` |  |
| `codegg_core_min` | 6 | 6 | `command_preflight`, `config_preflight`, `edit_preflight`, `text_replace_check`, `text_security_inspect`, `validate_json` |  |
| `codegg_core` | 15 | 15 | `cargo_toml_inspect`, `command_preflight`, `config_preflight`, `edit_preflight`, `identifier_inspect`, `path_normalize`, `structured_data_compare`, `text_diff_explain`, `text_equal`, `text_fingerprint`, `text_inspect`, `text_replace_check`, `text_security_inspect`, `validate_json`, `validate_toml` |  |
| `codegg_preflight` | 5 | 11 | `command_preflight`, `config_preflight`, `dependency_edit_preflight`, `edit_preflight`, `text_security_inspect` | `patch_apply_check`, `path_batch_scope_check`, `path_scope_check`, `prompt_input_inspect`, `shell_split`, `unicode_policy_check` |
| `codegg_patch` | 7 | 9 | `diff_risk_classify`, `edit_preflight`, `line_range_compare`, `line_range_extract`, `patch_summary`, `text_diff_explain`, `text_replace_check` | `patch_apply_check`, `path_batch_scope_check` |
| `codegg_config` | 14 | 14 | `config_file_inspect`, `config_preflight`, `dependency_edit_preflight`, `dotenv_validate`, `ini_validate`, `json_canonicalize`, `json_compare`, `json_extract`, `structured_data_compare`, `toml_shape`, `validate_json`, `validate_schema_light`, `validate_toml`, `version_compare` |  |
| `codegg_unicode_security` | 6 | 8 | `canonicalize_text`, `identifier_inspect`, `text_inspect`, `text_position`, `text_security_inspect`, `text_transform` | `prompt_input_inspect`, `unicode_policy_check` |
| `codegg_shell` | 4 | 5 | `argv_compare`, `command_preflight`, `regex_safety_check`, `shell_quote_join` | `shell_split` |
| `codegg_repo_audit` | 11 | 11 | `cargo_toml_inspect`, `code_fence_extract`, `config_file_inspect`, `dependency_edit_preflight`, `diff_risk_classify`, `identifier_table_inspect`, `json_shape`, `markdown_structure`, `repo_manifest_inspect`, `repo_tree_summarize`, `text_fingerprint` |  |
| `human_math` | 4 | 4 | `constant_lookup`, `math_eval`, `unit_convert`, `unit_info` |  |

<!-- END GENERATED: profile reference -->


| Profile | Intended Consumer | Description |
|---------|------------------|-------------|
| `full` | Debug, legacy MCP clients | All non-hidden tools. Broadest access. |
| `default` | General MCP clients | Model-default + some contextual tools. May grow slowly. |
| `codegg_core_min` | Ordinary coder-agent sessions | Smallest model-visible profile. Reduces hallucination without choice overload. |
| `codegg_core` | Manager/reviewer agents | Broader model-safe profile for deterministic utility use. |
| `codegg_preflight` | Harness (automatic checks) | Harness-oriented. Includes harness-only tools. Not for direct model exposure. |
| `codegg_patch` | Edit harness | Patch/edit-focused. Splits model-visible inspection from harness-only preflight. |
| `codegg_config` | Config editing workflows | JSON/TOML/config validation and inspection. |
| `codegg_unicode_security` | Suspicious input ingress | Unicode, hidden-character, confusable, and identifier security checks. |
| `codegg_shell` | Shell harness | Shell argv and command preflight. Harness use is automatic. |
| `codegg_repo_audit` | Manager/reviewer/research | Specialized repo inspection. Not default coder-agent exposure. |
| `human_math` | Direct human utility | Calculator, unit, and constant tools. |

### Codegg Integration Guide

Recommended profile + audience combinations for codegg:

| Workflow | Profile | Audience | Notes |
|----------|---------|----------|-------|
| Ordinary coder-agent | `codegg_core_min` | Model | Smallest safe tool list |
| Edit harness | `codegg_preflight` or `codegg_patch` | Harness | Automatic preflight checks |
| Shell harness | `codegg_shell` | Harness | Automatic before command execution |
| Config edits | `codegg_config` | Model or Harness | Depends on whether model calls tools directly |
| Suspicious input | `codegg_unicode_security` | Model or Harness | Security checks on ingress |
| Repo audit | `codegg_repo_audit` | Model | Manager/reviewer workflows |
| Math tasks | `human_math` | Model | Direct calculator use |

### Generated Documentation

Three files are generated from the ToolSpec registry by `cargo run --bin generate-docs`:

- **README.md** tool table — all non-hidden tools listed by category
- **architecture/mcp-server.md** profile reference — per-profile tool counts and names (sections between `BEGIN GENERATED`/`END GENERATED` markers)
- **generated/tool-cards.md** — per-profile tool cards with required arguments

The generator reads `ToolSpec` entries directly from `src/mcp/specs/` (the single source of truth) and filters out tools with `ToolExposure::Hidden`. Run `cargo run --bin generate-docs -- --check` to verify generated docs are current. The CI pipeline enforces this check.

### Typed Preflight Wrappers

`src/preflight/mod.rs` provides typed Rust wrappers over the raw JSON tool interface for common codegg workflows. Each wrapper has typed `Input`/`Output` structs, a `run()` method, and a `parse_response()` method for testing contract parsing without a full registry call.

Available wrappers:

| Wrapper | Tool | Verdict Enum |
|---------|------|-------------|
| `EditPreflight` | `edit_preflight` | `EditVerdict` |
| `CommandPreflight` | `command_preflight` | `CommandVerdict` |
| `ConfigPreflight` | `config_preflight` | `ConfigVerdict` |
| `PatchApplyCheck` | `patch_apply_check` | `EditVerdict` |
| `TextSecurityInspect` | `text_security_inspect` | (string verdict) |

All wrappers return `Result<Output, PreflightError>`. `PreflightError` distinguishes `ToolCall` (registry rejected), `ToolRejected` (tool returned `ok: false`), and `ContractViolation` (missing mandatory field — hard failure, no silent defaults).

## CLI Diagnostics

The `--diagnostics` flag prints version, tool count, per-profile tool counts, budget tiers, known environment variable names (no values), and parity-reference availability. Supports `--format json` for structured output. The `runtime_diagnostics` MCP tool provides similar information to harness-only audiences.
