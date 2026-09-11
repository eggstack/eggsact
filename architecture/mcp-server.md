# MCP Server Architecture

The `src/mcp/` module implements a JSON-RPC 2.0 server over stdio for AI coding agent integration.

## Files

| File | Purpose |
|------|---------|
| `server.rs` | Protocol orchestration: stdio read loop, request validation, JSON-RPC dispatch |
| `execution.rs` | Execution coordinator: mutex-backed handler lifecycle, request timeout accounting |
| `runtime.rs` | Rate limiter, constants, profile management, generation-aware request tracking |
| `sync_pool.rs` | Bounded synchronous execution pool (8 workers, 32-slot queue) for budget-aware APIs |
| `registry/` | Tool registration: aggregation, listing, types |
| `specs/` | `ToolSpec` declarations per tool category (single source of truth) |
| `protocol.rs` | JSON-RPC types: `JsonRpcRequest`, `JsonRpcResponse`, `InitializeResult`, error constructors |
| `response.rs` | `ToolResponse` struct, `sanitize_error`, response builders, `CallMetrics` |
| `schema_validation.rs` | MCP argument validation against tool input schemas |
| `compat.rs` | `CompatibilityMode` enum (EggcalcPython vs StrictNative) |
| `machine_codes.rs` | Machine-readable response codes, severity/disposition/verdict constants |
| `budget.rs` | Per-tool budget limits, `BudgetTier` enum, composite sub-budgets, `BudgetContext` with cooperative helpers |
| `schemas/` | JSON-schema builders per tool category (math, text, json, regex, etc.) |
| `mod.rs` | Module declarations |

### Schema Validation Subset

The schema validator in `schema_validation.rs` implements a **strict subset** of JSON Schema (draft 2020-12). Tool schemas in `src/mcp/schemas/` must only use keywords from the supported list. The boundary invariant is enforced by `test_schema_boundaries` in `tests/mcp/test_schema_boundaries.rs`, which walks all registered tool schemas and rejects any that use unsupported keywords.

#### Supported validation keywords

| Keyword | Behavior |
|---------|----------|
| `type` | String or array of strings. Checked against JSON value type. |
| `properties` | Object sub-property schemas. Recursively validated. |
| `required` | Required fields in objects. Returns "Missing required argument" on absence. |
| `additionalProperties` | Boolean (default `false`). When false, rejects extra fields not in `properties`. |
| `items` | Array element schema. Each element recursively validated. |
| `minItems`, `maxItems` | Array length bounds. |
| `uniqueItems` | When `true`, rejects arrays with duplicate serialized elements. |
| `minLength`, `maxLength` | String char-count bounds (`.chars().count()`, not byte length). |
| `pattern` | Regex match on strings. Compiled via `regex::Regex`. |
| `minimum`, `maximum` | Inclusive numeric bounds (f64). |
| `exclusiveMinimum`, `exclusiveMaximum` | Strict numeric bounds (`>` / `<`). |
| `multipleOf` | Numeric divisibility with epsilon tolerance (abs < 1e-12 or rel < 1e-9). |
| `enum` | Array membership check. Value must match one of the listed values. |
| `const` | Exact value match. |

#### Annotation-only keywords (allowed, not enforced)

`description`, `title`, `default`, `examples`, `$schema` — present in schemas for documentation/tooling but not validated at runtime.

#### Deterministic Output

Tool responses are semantically deterministic for identical inputs. Public JSON object fields use `BTreeMap` for stable key ordering. See [text-library.md](text-library.md#deterministic-output) for the full contract. MCP clients must still correlate concurrent responses by JSON-RPC ID.

#### Explicitly unsupported keywords

The following keywords must **never** appear in registered tool schemas:

- **Composition**: `$ref`, `$defs`, `definitions`, `oneOf`, `anyOf`, `allOf`, `not`
- **Conditionals**: `if`/`then`/`else`
- **Pattern properties**: `patternProperties`, `propertyNames`
- **Dependencies**: `dependentRequired`, `dependentSchemas`
- **Array extras**: `contains`, `prefixItems`, tuple validation
- **Format/content**: `format`, `contentEncoding`, `contentMediaType`
- **Property counts**: `minProperties`, `maxProperties`
- **Unevaluated**: unevaluated properties/items

#### Recursive validation

Validation recurses into nested schemas via `validate_property_inner()` with a `max_depth` limit of 10. This supports `properties` sub-schemas, `items` element schemas, and `additionalProperties` object schemas. Deeply nested schemas beyond 10 levels produce a "Schema nesting too deep" error.

Tool implementations live in `src/tools/` (category modules):

| Module | Tools |
|--------|-------|
| `helpers.rs` | Shared constants, utility functions |
| `math.rs` | math_eval, unit_convert, unit_info, constant_lookup |
| `text.rs` | text_measure, text_equal, text_diff_explain, text_inspect, text_count, text_truncate, text_fingerprint, text_hash, text_position, text_window, text_transform, text_replace_check, text_security_inspect, escape_text, unescape_text, prompt_input_inspect, line_range_extract, line_range_compare |
| `json.rs` | json_extract, json_compare, json_canonicalize, json_query, json_shape, structured_data_compare |
| `regex.rs` | validate_regex, regex_safety_check, regex_finditer |
| `validation.rs` | validate_json, validate_brackets, validate_toml, validate_schema_light |
| `path.rs` | path_normalize, path_analyze, path_compare, path_scope_check, glob_match, path_batch_scope_check |
| `shell.rs` | shell_split, shell_quote_join, argv_compare, command_preflight |
| `list.rs` | list_compare, list_dedupe, list_sort |
| `markdown.rs` | markdown_structure, code_fence_extract |
| `patch.rs` | patch_apply_check, patch_summary, edit_preflight, diff_risk_classify, patch_contract_check |
| `config.rs` | dotenv_validate, ini_validate, config_preflight, toml_shape_tool |
| `identifier.rs` | identifier_analyze, identifier_inspect, identifier_table_inspect |
| `unicode.rs` | unicode_policy_check, canonicalize_text |
| `version.rs` | version_compare, version_constraint_check |
| `cargo.rs` | cargo_toml_inspect |
| `dependency.rs` | dependency_edit_preflight |
| `diagnostics.rs` | runtime_diagnostics, profile_inspect, tool_availability_explain |
| `repo.rs` | repo_manifest_inspect, config_file_inspect, repo_tree_summarize, test_command_suggest, repo_language_detect |
| `analysis.rs` | import_export_inspect, code_block_map, symbol_name_diff, lockfile_inspect |

## Protocol

- Transport: stdio (stdin/stdout)
- Protocol: JSON-RPC 2.0
- MCP versions: `2026-07-28` (preferred, modern stateless), `2025-11-25` (legacy preferred), `2024-11-05` (legacy)
- Server identity: `eggsact` (legacy `serverInfo`; modern `_meta.io.modelcontextprotocol/serverInfo` on every result)
- Server instructions: concise cross-tool guidance in legacy `initialize` (`instructions`) and modern `server/discover` (`instructions`); preflight tools inspect rather than execute

### Supported Methods

| Method | Eras | Description |
|--------|------|-------------|
| `initialize` | Legacy only | Returns server info, capabilities, and instructions |
| `server/discover` | Modern with a valid `_meta` claim | Returns supported versions, capabilities, instructions, cache hints, and `_meta` identity; claim-less traffic follows legacy lifecycle/method semantics |
| `notifications/initialized` | Legacy notification (modern claims are admitted but no-op) | Client acknowledgment; modern envelopes never mutate session state |
| `notifications/cancelled` | Both | Looks up the request ID in the active-requests map and sets its cancel flag |
| `tools/list` | Both | Returns registered tool definitions (filtered by profile); modern adds `resultType`, `ttlMs`, `cacheScope`, `_meta`, standard annotations |
| `tools/call` | Both | Executes a tool by name; modern adds `resultType`, `structuredContent`, `_meta` |
| `profiles/list` | Both (eggsact extension) | Lists all profiles and their tool counts; modern adds `resultType`, `_meta` |
| `ping` | Legacy only | Returns empty response (health check); removed in `2026-07-28` (`-32601`) |

### Connection Lifecycle (Dual-Era, Connection-Pinned)

One stdio process is one connection. The first classifiable inbound message
pins the era exactly once (`ConnectionEra::Undecided` → `Legacy` for
`initialize` or any claim-less message, → `Modern20260728` for a valid modern
claim); later requests cannot switch eras. This mirrors the official
TypeScript SDK `serveStdio` model (opening exchange pins; its enveloped
`server/discover` probe runs in a disposable sibling process and never appears
on the session child's wire). The spec allows dual-era servers to serve both
eras concurrently on one endpoint, but eggsact chooses pinning on stdio so a
long-lived process cannot mix eras. Modern request `_meta` remains validated
per request even though the era is per-connection.

- A successful legacy `initialize` pins `Legacy`; legacy clients must then
  complete the handshake before calling tools:

1. Client sends `initialize` with `protocolVersion`, `capabilities`, and `clientInfo`
2. Server responds with negotiated legacy `protocolVersion`, `capabilities`, `serverInfo`, and concise `instructions`
3. Client sends `notifications/initialized` (no response)
4. Server transitions to `Ready` state — all legacy methods now available

- A valid modern envelope (normally enveloped `server/discover`, or a direct
  modern `tools/list`/`tools/call`) pins `Modern20260728` with no handshake
  and no `SessionState` access. Every modern call still validates its own
  `_meta`.
- Claim-less opening traffic (`ping`, `tools/list` / `tools/call`, unknown
  methods, notifications, and `server/discover`) pins Legacy. Claim-less
  `server/discover` is then handled by legacy lifecycle/method semantics and
  does not return modern discovery.
- Malformed modern envelopes (`-32602`) and unsupported claims (`-32022`) are
  answered without pinning; the client can retry. Failed legacy `initialize`
  validation is method-level behavior after its claim-less opening has already
  pinned Legacy.
- After pinning, a request classified for the other era receives
  `-32022 Unsupported protocol version` with the original id preserved; no
  `ERA_MISMATCH` wire code exists. Mismatched notifications are dropped
  without a response or lifecycle/cancellation side effect. The era lock
  (`Arc<Mutex<ConnectionEra>>`, same pattern as `SessionState`) is held only
  for inspect/select; tool execution stays concurrent. Exactly one concurrent
  opening wins; the loser sees the standard unsupported-version response.

An enveloped modern `server/discover` works from a fresh process before (or without) `initialize` and leaves legacy state untouched, so subsequent modern requests need no `notifications/initialized`.

#### Session States (Legacy Only)

| State | Allowed Methods | Notes |
|-------|----------------|-------|
| `Uninitialized` | `initialize`, `ping` | Default state at connection start; claim-less `server/discover` is not a modern probe |
| `AwaitingInitialized` | `notifications/initialized`, `ping` | After `initialize` response sent |
| `Ready` | All legacy methods | After `notifications/initialized` received; `server/discover` is not a legacy method |

Modern requests never consult or mutate `SessionState`.

#### Version Negotiation (Era-Aware)

| Version | Era | Status | Notes |
|---------|-----|--------|-------|
| `2026-07-28` | Modern | Preferred | Stateless per-request `_meta`; `server/discover` advertises first |
| `2025-11-25` | Legacy | Legacy preferred | `initialize` fallback target |
| `2024-11-05` | Legacy | Supported | Backward compatibility |

- Legacy `initialize` negotiates only among `2025-11-25` / `2024-11-05`; unsupported (including modern) requests fall back to `2025-11-25`, never into modern state.
- `server/discover` advertises all three revisions in deterministic preference order (`2026-07-28`, `2025-11-25`, `2024-11-05`).
- A modern request naming an unsupported revision returns `-32022 UnsupportedProtocolVersion` with `{supported, requested}` instead of entering legacy state. Malformed modern envelopes (missing/incorrect `_meta`) return `-32602`.

`NegotiatedProtocol` retains `client_capabilities: ClientCapabilities` for the legacy session lifetime. Modern capabilities are request-scoped (`ModernRequestContext`) and never stored globally. Client identity (`clientInfo` / envelope `clientInfo`, `serverInfo`) is advisory for display/logging/debugging only.

#### Server Capabilities

The legacy initialize response advertises (plus concise `instructions`):

```json
{
  "capabilities": {
    "tools": { "listChanged": false },
    "experimental": {
      "eggsact": {
        "profiles": true,
        "schemaDetail": true,
        "audienceFiltering": true
      }
    }
  }
}
```

The modern `server/discover` result advertises the same capabilities plus:

```json
{
  "resultType": "complete",
  "supportedVersions": ["2026-07-28", "2025-11-25", "2024-11-05"],
  "capabilities": { "tools": { "listChanged": false }, "experimental": { "eggsact": { "...": true } } },
  "instructions": "eggsact is a local deterministic utility server. ...",
  "ttlMs": 3600000,
  "cacheScope": "public",
  "_meta": { "io.modelcontextprotocol/serverInfo": { "name": "eggsact", "version": "1.2.4" } }
}
```

`listChanged` remains `false`; the catalog is static. The progressive-discovery line must use a stable presentation surface plus search/invoke routing, not per-connection list mutation.

#### Modern Tool Catalog (Cacheable, Deterministic)

- Modern `tools/list` returns `resultType: complete`, `tools` in registry order (deterministic, not alphabetized), `ttlMs: 3600000`, `cacheScope: public`, and `_meta` server identity. Legacy `tools/list` is unchanged (`{"tools": [...]}`).
- Registry order is compatibility-sensitive; `test_modern_protocol::modern_list_deterministic_order_and_bytes` guards stable ordering and bytes.
- Modern Tool shape uses standard fields (`name`, `description`, `inputSchema`, `outputSchema`, `annotations`) plus namespaced `_meta` (`io.github.eggstack/eggsact` with `tier`, `tags`, `category`, `llm_exposure`, `cost`, `deprecated` when true). Nonstandard top-level keys are legacy-only.
- Annotations are uniform (`readOnlyHint: true`, `destructiveHint: false`, `idempotentHint: true`, `openWorldHint: false`) because every tool is a local deterministic computation. Hints, not enforcement.

#### Modern Tool Results (`structuredContent`)

- Successful modern `tools/call` returns `resultType: complete`, `content[0].text` (Python-style JSON fallback, unchanged), `structuredContent` (= `ToolResponse.result`, conforming to the tool's `outputSchema`), and `_meta` identity. `verdict`/`machine_code` stay inside `result` and the text envelope.
- Tool-level errors return `resultType: complete`, `content`, `isError: true`, `_meta`, with no `structuredContent`. JSON-RPC errors (`-32601`, `-32602`, `-32022`, etc.) are unchanged apart from modern codes.
- `ping` is absent in the modern era; `resultType` is required on all modern results (`complete`; eggsact emits no `input_required` — no MRTR workflow).

#### Lifecycle Errors

| Error | Code | Data Code | When |
|-------|------|-----------|------|
| Not initialized | -32600 | `NOT_INITIALIZED` | Legacy method called before `initialize` on an Undecided/Legacy connection |
| Already initialized | -32600 | `ALREADY_INITIALIZED` | Duplicate `initialize` request on a Legacy connection |
| Era-classification mismatch | -32022 | — | Cross-era request after pinning (modern claim on Legacy-pinned, or claim-less legacy message on Modern-pinned); id preserved, other era untouched |
| Initialized before initialize | — | — | `notifications/initialized` received before `initialize` — **silently ignored** (no response), per JSON-RPC notification semantics; Modern-pinned legacy notifications also ignored without touching `SessionState` |
| Invalid params (modern envelope) | -32602 | — | Malformed claimed modern envelope; never pins |
| Unsupported version | -32022 | — | Modern `_meta` names an unsupported revision; `data: {supported, requested}`; never pins |
| Modern initialize / ping | -32601 | — | Valid modern envelope naming `initialize`/`ping` on a Modern connection (`initialize` legacy-only; `ping` removed). A valid modern claim arriving on a Legacy-pinned connection is a `-32022` routing mismatch |

The `initialized_before_initialize` helper (`INITIALIZED_BEFORE_INITIALIZE` data code) has been removed. Wrong-state `notifications/initialized` notifications are silently discarded.

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
- `tools/call`: Resolves the active profile from `get_active_profile()` and creates a `ToolRegistry` with `Model` audience and `EggcalcPython` compatibility mode (Python-parity error messages). Delegates tool lookup, profile checking, audience/exposure checking, and argument validation to `ToolRegistry::prepare_tool_call` (shared with the in-process agent API in `src/agent/`). MCP retains its own async dispatch layer (timeout, semaphore, cancellation) around the core handler execution. This avoids duplicating lookup/validation logic between the MCP server and the agent API. The in-process agent API defaults to `StrictNative` mode (standard JSON Schema error messages). For budget-aware in-process APIs (`call_json_with_budget`, `call_json_with_context`, `call_json_with_execution_context`), the handler is dispatched through the `SyncExecutionPool` rather than directly.

### Direct vs Discovery Surface (`src/mcp/discovery.rs`)

`McpSurface` separates capability policy from presentation. Startup selects
via `EGGSACT_MCP_SURFACE=direct|discovery` (new `EGGSACT_` namespace for
eggsact-native config) or `--mcp-surface direct|discovery` (CLI overrides
env); the default remains `direct` for 1.x compatibility, while discovery is
explicitly opt-in.

- **Direct**: current behavior. `tools/list` advertises the
  profile/audience-filtered canonical set; `tool_search` / `tool_invoke`
  are unknown tools.
- **Discovery**: `tools/list` advertises at most the pinned front doors
  allowed by the active profile/audience plus the two facades (7 entries
  for `full`/Model; fewer for narrow profiles such as `codegg_patch`).
  Pinned entries use compact descriptions/schemas with no output schemas
  or tier/tags/cost bookkeeping; modern entries carry only
  `name`/`description`/`inputSchema`/`annotations` (no `_meta`). `names`
  narrows the discovery set; `tier`/`tags` are ignored in discovery mode.
- **`tool_search`**: deterministic weighted lexical search over the
  active `ToolSpec` slice after profile/audience filtering (exact name >
  exact alias > name token/prefix > tag/category > description >
  Levenshtein recovery; ties break by relevance then registry order).
  Input: `query` (required), `limit` 1–10 (default 5), `detail`
  `summary|schema` (default `summary`), `include_deprecated` (default
  false; deprecated tools such as `json_query` appear only by exact name
  or opt-in, with `replacement` hint where documented).
- **`tool_invoke`**: MCP-layer generic router (`name` + `arguments:{}`).
  It reuses `ToolRegistry::prepare_tool_call` + target
  `budget_for_tool` + `execution::execute_tool_bounded` and the same
  era-specific response adapter as a direct call, so the target name stays
  visible in `ToolResponse`/metadata. It rejects `tool_search` /
  `tool_invoke` targets (no recursion) and enforces the same
  profile/audience rules as direct calls — omitted-from-list never means
  unauthorized, and HarnessOnly/Hidden stay unreachable to Model callers.
  It is intentionally a routing facade with target-specific results and has
  no facade-level output schema: modern `structuredContent` is the target's
  normal `ToolResponse.result`. Obtain the target input contract via
  `tool_search(detail="schema")` or direct/full mode; do not add an 86-way
  union or duplicate target schemas into the facade.

The two facades are MCP-only orchestration (`src/mcp/discovery.rs`); they
are not in `ALL_TOOLS_VEC`, `src/tools/`, or generated tool-cards. Eggsact
still has 86 underlying deterministic tools.

The rollout baseline is measured by `src/mcp/discovery_eval.rs` and guarded by
`tests/mcp/test_discovery.rs`: full/Model direct advertises 77 tools in 111,911
serialized UTF-8 bytes, while discovery advertises 7 in 6,088 bytes (5.44%,
gate <=25%). Semantic coverage is registry-derived, not hard-coded: all 76
stable full/Model tools have task-oriented fixtures (88 positive intents with
second phrasings for overlap-prone tools, 9 containment; no bare-name
fixtures, no HarnessOnly counted as positive, deprecated stays
migration/negative). Retrieval gates stay top-1 >=90%, top-3 >=98%, top-5
100% with zero Model-audience leaks. The portable corpus holds 48 Model task
scenarios plus 4 `must_not_expose` containment cases (`audience`/`kind`,
plural `expected_tools`); Model tasks must stay 40–60 and Harness tasks never
join the Model denominator. These are byte/lexical regression gates;
paired provider traces are scored separately with
`scripts/score-discovery-traces.py --pair` (strict validation, 2pp
success/selection noninferiority, invalid/retry and workflow-exercised
gates). External OpenAI/Anthropic direct/discovery pairs plus the
instructions A/B are still pending under `mcp-surface-03c`; generated
integrations stay direct until that evidence lands (see
`tests/fixtures/discovery_traces/README.md` and `docs/verification.md`).

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
| patch | 5 | patch_apply_check, patch_contract_check, patch_summary, edit_preflight, diff_risk_classify |
| config | 3 | dotenv_validate, ini_validate, config_preflight |
| identifier | 3 | identifier_analyze, identifier_inspect, identifier_table_inspect |
| unicode | 2 | unicode_policy_check, canonicalize_text |
| version | 2 | version_compare, version_constraint_check |
| toml | 1 | toml_shape |
| cargo | 1 | cargo_toml_inspect |
| dependency | 1 | dependency_edit_preflight |
| repo | 5 | repo_manifest_inspect, config_file_inspect, repo_tree_summarize, test_command_suggest, repo_language_detect |
| analysis | 4 | import_export_inspect, code_block_map, symbol_name_diff, lockfile_inspect |
| diagnostics | 3 | runtime_diagnostics, profile_inspect, tool_availability_explain |

### Regex Backend Contract

The `validate_regex`, `regex_safety_check`, and `regex_finditer` tools auto-select between two regex backends via the `classify_pattern()` function in `src/text/regex_engine.rs`. The classification is based on which constructs the pattern uses:

- **Rust `regex`** (linear-time, safe): used for patterns with no lookaround, no backreferences, and no PCRE-only constructs. This is the default and preferred backend.
- **`fancy-regex`** (backtracking-based): used when the pattern contains lookaround (`(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`) or backreferences (`\1`, `(?P=name)`). Supports named captures `(?P<name>...)` with both engines.

Regex tool outputs include these fields reflecting the backend decision:

| Field | Type | Description |
|-------|------|-------------|
| `engine_used` | string | `"rust-regex"` or `"fancy-regex"` — which backend compiled the pattern |
| `dialect` | string | `"eggsact-regex"` — the eggsact regex dialect identifier |
| `unsupported_features` | string[] | PCRE-only constructs detected (branch reset, recursion, `\K`, control verbs, atomic groups). Empty when all constructs are supported. |

When `unsupported_features` is non-empty, the tool returns machine code `REGEX_UNSUPPORTED_FEATURE`. This is distinct from `REGEX_UNSAFE` (ReDoS safety) — a pattern can be fully supported by the engine but unsafe, or safe but unsupported. See `architecture/machine-codes.md` for the full machine code table.

## Composite Tools

Tools marked `composite: true` orchestrate other tools internally. All emit a `verdict` field in their result JSON via the `.with_verdict()` builder, and use `finding()` helpers with canonical `severity::*` and `disposition::*` constants.

| Tool | Verdict domain | What it does |
|------|---------------|-------------|
| `edit_preflight` | allow / review / block | Pre-checks an edit operation using text tools. Optionally composes `path_scope_check`, `text_security_inspect`, and `text_fingerprint` (newline detection) when the corresponding input fields are provided. |
| `command_preflight` | allow / review / block | Pre-checks a shell command using a policy engine. Classifies commands via per-policy allow/review/block matrices (`default`, `strict`, `permissive`), detects behavioral features (network, filesystem, process, env, shell features), detects wrapper programs (sh/bash/python/node with `-c`/`-e` flags → review) and script runners (make/just/task → review), checks destructive patterns, applies custom `policy_config` allow/deny lists, scans all argv entries for env mutation (`FOO=bar ...`), and runs regex safety on regex-like args. |
| `config_preflight` | valid / valid_with_warnings / invalid | Pre-checks a config file using validation tools |
| `text_security_inspect` | allow / review / block | Calls multiple text inspection tools and aggregates results |
| `structured_data_compare` | — | Uses json_compare and list tools for structured data |

## Route-Critical Tools

A subset of tools are classified as **route-critical** — they produce structured verdicts and machine codes that downstream harnesses depend on for routing decisions. The `is_route_critical()` helper and `ROUTE_CRITICAL_TOOLS` constant in `registry/listing.rs` identify these tools:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `text_security_inspect`

Route-critical tools must always emit a `machine_code` and `verdict` in their response envelope. The `patch_apply_check` tool is `HarnessOnly` exposure and does not appear in model-facing listings.

These contracts are verified by fixture-backed tests in `tests/mcp/test_route_contracts.rs`: a `RouteFixture` struct drives table-driven assertions for each tool's happy-path and error-path responses, with registry invariant tests and MCP stdio coverage. The tests assert `machine_code`, `verdict`, and a subset of expected findings for each fixture, and verify audience enforcement for HarnessOnly tools.

## Concurrency Model

The MCP stdio server supports **concurrent request handling**. The read loop
in `server.rs` reads requests from stdin as lines arrive and spawns each request
as an independent tokio task via a `JoinSet`. Responses are sent through an
`mpsc` channel to a dedicated writer task that serializes stdout writes,
preventing interleaved output from concurrent handlers.

**Concurrency limits:**

- `MAX_IN_FLIGHT_REQUESTS` (32): maximum simultaneously active request tasks.
  Exceeding this limit returns a JSON-RPC error (-32000 with
  `RESOURCE_EXHAUSTED` data code).
- `MAX_TOOL_WORKERS` (16): semaphore permits for concurrent blocking tool
  executions *within* tasks. This is a back-pressure mechanism for CPU-bound
  tools, not a concurrency driver.

**Worker containment:** Semaphore permits are acquired via `acquire_owned()`
and moved into `spawn_blocking` closures. The owned permit is held until the
handler closure exits, ensuring `MAX_TOOL_WORKERS` is a hard upper bound even
after client-facing timeouts. All handlers execute directly inside the
`spawn_blocking` closure with `catch_unwind` safety — there are no nested OS
threads. The outer tokio semaphore provides the sole concurrency bound.

**Read loop stages:** The read loop processes each incoming line through seven
stages in order:

1. Bounded frame-size check (rejects lines exceeding `MAX_REQUEST_BYTES`
   during incremental reading — the server never allocates the full line
   before checking the limit). The reader counts every byte before LF,
   discounts only a final CR in CRLF, retains at most the configured cap, and
   consumes through exactly the terminating LF so the next JSONL frame is
   preserved.
2. JSON parse
3. Top-level validation (must be an object)
4. Extract method
5. Validate ID type (must be string, integer, or absent — null is rejected)
6. Construct request (or handle notifications)
7. For notifications: dispatch immediately
8. For requests: reject null IDs → in-flight check → duplicate ID check → register + dispatch

**Cancellation model:** Each request gets an `Arc<AtomicBool>` cancel flag at
dispatch time. A correctly classified `notifications/cancelled` notification
in either supported stdio era sets the flag for an active request ID. A
cross-era or malformed notification is dropped before lookup, with no stdout
response and no cancellation side effect. The flag is shared between the
timeout path (`tokio::time::timeout` in `handle_request_async`) and the handler
(via `budget::with_cancel_flag()` thread-local), so external cancellation and
timeout share the same signal. The handler can check
`BudgetContext::should_stop()` at any pipeline stage to detect cancellation.

`apply_cancellation` is `async` — it uses `.lock().await` on the
active-request map instead of `.try_lock()`, preventing cancellation loss under
lock contention. Cancellation notifications are dispatched immediately without
any rate or capacity checks.

**Duplicate-ID policy:** Non-null duplicate request IDs are rejected atomically
by `register_request()` under a single lock acquisition — in-flight limit check,
duplicate check, and insertion happen in one lock window. Rejection returns a
JSON-RPC error (-32600) using the `DUPLICATE_REQUEST_ID` machine code. Null
IDs (`id: null`) are rejected for requests because concurrent tracking and
error correlation become ambiguous. Notifications use an absent `id` field,
not `null`.

**Generation-aware request cleanup:** `ActiveRequest` in `runtime.rs` carries a
`generation: u64` field. A global `NEXT_GENERATION` `AtomicU64` counter assigns
monotonically increasing generations at registration time. `register_request()`
returns `(RequestGuard, RequestRegistration)`. `complete_request()` is async
and removes entries only when the generation matches, preventing stale completions
from evicting newer requests that reused the same ID. `RequestGuard::Drop` is a
panic-proof fallback: it removes its own entry (generation-checked, via
`try_lock`) if it is somehow still registered, so a task panicking before the
awaited cleanup cannot leak the in-flight slot; in the normal flow it is a
no-op. The server uses an
outer/inner task pattern: `tokio::spawn` for the inner handler, with an awaited
`complete_request` after the join completes.

**Diagnostics counters:** `RequestGuard` and owned semaphore permits are
reflected in the diagnostics counter infrastructure — `runtime_diagnostics`
reports in-flight and active-worker counts. Global `RUNTIME_METRICS` provides
live atomic counters: `active_requests`, `active_blocking_handlers`,
`timed_out_handlers`, `total_timeouts`, and `peak_blocking_concurrency`.
RAII `MetricGuard` ensures counters decrement correctly on panic/unwind.
At synchronized snapshots, `timed_out_handlers <= active_blocking_handlers`.

**Timeout metrics lifecycle:** `timed_out_handlers` uses a
`Mutex<HandlerPhase>` lifecycle in `execution.rs`. Each handler transitions
through five phases:

| Phase | Meaning |
|-------|---------|
| `Queued` | Request received, awaiting spawn |
| `Running` | Handler is actively executing |
| `TimedOutQueued` | Timeout while still queued (never ran) |
| `TimedOutRunning` | Timeout fired while handler was running |
| `Finished` | Handler has completed |

Queued timeouts transition `Queued → TimedOutQueued` and increment only
`total_timeouts`, NOT `timed_out_handlers`. Running timeouts transition
`Running → TimedOutRunning` and increment `timed_out_handlers`. Completion
after timeout transitions `TimedOutRunning → Finished` and decrements
`timed_out_handlers`. All transitions are linearizable under a single lock
acquisition.

**Graceful shutdown:** When stdin reaches EOF, the read loop breaks and the
server drains in-flight requests via `JoinSet::join_next()` before dropping the
channel sender and waiting for the writer task to flush remaining responses.

### Worker containment — single-layer execution

All tool handlers now execute directly inside the bounded `spawn_blocking`
closure. Each handler wraps its evaluator call in `std::panic::catch_unwind`
for safety. The execution stack is:

```
tokio::time::timeout (outer, budget-derived)
  └─ spawn_blocking (holds owned permit)
       └─ handler work (math_eval / regex / ini parse / etc.)
            wrapped in std::panic::catch_unwind
```

The outer tokio semaphore (`MAX_TOOL_WORKERS=16`) is the **sole concurrency
bound**. There are no nested OS threads — `SpawnSemaphore`, `SpawnPermit`,
and related infrastructure have been removed from `src/tools/helpers.rs`.

Handlers (`math_eval`, `validate_regex`, `regex_finditer`, `dotenv_validate`)
execute directly within the `spawn_blocking` closure. The `catch_unwind`
wrapper prevents panics from unwinding across the FFI boundary into tokio's
thread pool.

### Execution Coordinator (`src/mcp/execution.rs`)

The execution coordinator manages the mutex-backed handler lifecycle and
timeout accounting. It owns a `HandlerLifecycle` (wrapping
`Mutex<HandlerPhase>`) per handler and provides the `SyncExecutionPool` for
budget-aware in-process APIs.

#### Mutex-Backed Handler Lifecycle

Each request handler progresses through phases in `execution.rs`:

```
Queued → Running → Finished
         ↓           ↑
    TimedOutRunning ─┘
         ↑
Queued ──┘ (timeout before spawn → TimedOutQueued, handler never runs)
```

### Bounded Synchronous Execution Pool (`src/mcp/sync_pool.rs`)

`SyncExecutionPool` provides bounded synchronous execution for budget-aware
in-process APIs. It has 8 worker threads and a 32-slot queue.

| API | Routes through pool? |
|-----|---------------------|
| `call_json_with_budget` | Yes |
| `call_json_with_context` | Yes |
| `call_json_with_execution_context` | Yes |
| `call_json` | No (direct dispatch, no pool) |

Queue saturation returns `RESOURCE_EXHAUSTED`. The MCP server path is **not**
affected — it uses Tokio `spawn_blocking` with the existing semaphore
infrastructure.

The pool enforces elapsed-time budgets from `ToolBudget.max_elapsed_ms` by
running handlers with a timeout, matching the MCP server's cooperative
cancellation model.

**Response ordering contract (JSON-RPC):** Because requests are dispatched
concurrently and may complete out of request order, **clients must correlate
responses to requests by JSON-RPC `id`**, not by arrival position. The `id`
field on each response echoes the `id` of the originating request (string or
integer only — null is rejected for requests). JSON-RPC clients that implicitly
assume ordered responses will observe races on multi-request sessions. The
server intentionally does not serialize dispatch behind a head-of-line lock,
because doing so would re-introduce the latency bottleneck that the concurrent
runtime removed.

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

- **Profile/Audience**: Falls back to registry defaults when `None`. When `Some`, uses the context's values for tool filtering and exposure checks. Resolved via `prepare_tool_call_with_policy`.
- **Compatibility mode**: Used for argument schema validation.
- **EvalContext**: **Cloned** and set as thread-local via `budget::with_eval_context()`, making it available to calculator-backed tools (e.g., `math_eval` uses `run_with_context()` when a thread-local context is present). Mutations inside the handler do not persist back to the caller's `ExecutionContext`. Two calls with identical seeds produce the same first random value. Use `call_json_with_execution_context_mut()` for the mutable variant where handler state persists (deprecated since 1.0.0 — does not persist calculator state through `math_eval`).
- **Budget/Cancellation**: Resource limits and cooperative cancellation flag. The cancellation flag is set as a thread-local during dispatch so that high-risk handlers that create their own `BudgetContext` inherit cancellation.

**MCP wire protocol boundary**: `call_json_with_execution_context` is an **in-process** API. It does not change the MCP JSON-RPC wire protocol. The MCP server still resolves its active profile from `EGGCALC_MCP_PROFILE` at init time. Per-request context overrides over the wire would require a future MCP request-level context API.

Legacy APIs remain as backward-compatible wrappers:

| Method | What it wraps |
|--------|---------------|
| `call_json(name, args)` | Creates a default context internally |
| `call_json_with_budget(name, args, budget)` | Context with custom budget |
| `call_json_with_context(name, args, budget, cancel_flag)` | Context with budget and cancellation |
| `call_json_with_execution_context(name, args, ctx)` | Full context (immutable, clones eval_ctx) — **recommended for new code** |
| `call_json_with_execution_template(name, args, ctx)` | Explicit immutable alias for `call_json_with_execution_context` |
| `call_json_with_execution_context_mut(name, args, ctx)` | Mutable persistent context — **deprecated since 1.0.0**, does not persist calculator state through `math_eval` |

### MCP startup env vars → runtime context

The MCP server reads `EGGCALC_MCP_PROFILE`, `EGGCALC_MCP_AUDIENCE`, and `EGGCALC_MCP_SCHEMA_DETAIL` at startup. These become the `profile`, `audience`, and `compatibility_mode` fields in `ExecutionContext::mcp_default()`. Once set, they apply to all subsequent tool calls — there is no per-call profile override in `tools/call`. `EGGCALC_MCP_SCHEMA_DETAIL` accepts `compact`, `normal`, or `full` (default: `full`). Invalid values warn to stderr and default to `full`.

### Handler signatures unchanged

Tool handler functions retain the signature `fn(&Value) -> ToolResponse` for compatibility. This means handlers cannot receive an `ExecutionContext` directly — state isolation is achieved by the caller passing context into `call_json_with_execution_context()`, which applies it at the orchestration layer. Calculator-backed handlers (e.g., `math_eval`) retrieve the `EvalContext` from a thread-local set by `budget::with_eval_context()`. High-risk handlers (`edit_preflight`, `command_preflight`, etc.) create a `BudgetContext` internally for cooperative budget checks.

### What remains global / thread-local

| State | Why global |
|-------|------------|
| `MCP_MODE`, `ALLOW_RANDOM`, `ALLOW_SIDE_EFFECTS` AtomicBool | One-shot startup flags, race-safe. Context-aware APIs bypass these via `EvalContext` fields. MCP dispatch no longer sets these directly — it uses `EvalContext::mcp_mode()` through the thread-local bridge. These remain for legacy library callers. |
| `ACTIVE_PROFILE`, `ACTIVE_AUDIENCE`, `ACTIVE_SCHEMA_DETAIL` RwLock | Set once at startup, read-only after init |
| `CURRENT_CANCEL_FLAG` thread-local | Properly scoped per-dispatch via guard-owned restoration |
| `CURRENT_EVAL_CONTEXT` thread-local | Set by `with_eval_context()` during calculator-backed tool dispatch; accessed via closure-scoped `with_current_eval_context()` |
| 36+ LazyLock immutable caches | Regex, tables, tool definitions — immutable after init |
| Request-scoped Arc objects | Cloned per-request, not shared |
| `MEMORY_REGISTERS`, `USER_VARIABLES`, `PRNG_STATE`, `GAUSS_SPARE` LazyLock | Legacy mutable state. Context-aware APIs use `EvalContext` fields instead; globals remain for legacy `evaluate()` path. |

These are intentionally global because they represent immutable configuration or startup-time state, not per-request mutable state. The legacy mutable globals (`MEMORY_REGISTERS`, `USER_VARIABLES`, `PRNG_STATE`, `GAUSS_SPARE`) are retained for backward compatibility but are bypassed by context-aware APIs.

## Concurrency Limits

Defined in `src/mcp/runtime.rs`:
- `MAX_IN_FLIGHT_REQUESTS`: 32
- `MAX_TOOL_WORKERS`: 16
- `MAX_REQUEST_ID_LENGTH`: 1024
- `MAX_REQUEST_BYTES`: 1,000,000
- `MAX_OUTPUT_BYTES`: 1,000,000

No fixed request-rate limiter is applied. The in-flight limit, tool semaphore,
and byte limits are sufficient for the local stdio server. Capacity pressure
returns a JSON-RPC error in the -32000 range with `RESOURCE_EXHAUSTED` data,
not -32600.

Tool timeouts are now **budget-derived** rather than using a fixed `MAX_TOOL_TIMEOUT_SECONDS`. Each `ToolSpec` declares a `cost` field (`ToolCost::Cheap`, `Moderate`, `Heavy`), which maps to a `ToolBudget` with per-tool limits including `max_elapsed_ms`. The `budget_for_tool()` function in `src/mcp/budget.rs` resolves the effective budget, and `tools/call` uses `budget.max_elapsed_ms` as the timeout instead of the previous fixed 30s constant.

## Budget-Aware Dispatch

The MCP server applies per-tool resource budgets during `tools/call` dispatch:

### Budget Module (`src/mcp/budget.rs`)

- **`BudgetTier`** enum: `Cheap`, `Moderate`, `Heavy` — maps from `ToolCost` in `ToolSpec`.
- **`ToolBudget`** struct: per-tool resource limits — `max_input_bytes`, `max_output_bytes`, `max_elapsed_ms`, `max_text_bytes`, `max_findings`, `max_list_items`, `max_regex_pattern_chars`, `max_regex_samples`, `max_spawned_workers`.
- **`budget_for_tool(tool_name)`**: resolves the effective `ToolBudget` for a tool. Composite tools (`edit_preflight`, `command_preflight`, `config_preflight`, `text_security_inspect`, `patch_apply_check`) always receive `HEAVY` budgets regardless of their declared `ToolCost`. Other tools map from their `ToolCost` via `BudgetTier`.
- **Builders**: `ToolBudget::with_max_findings(n)`, `with_max_output_bytes(n)`, `with_max_input_bytes(n)`, `with_max_text_bytes(n)`, `with_max_elapsed_ms(n)` — used by callers (especially tests) to override a single budget field without rebuilding the whole struct. Direct struct literals remain valid but break ABI when fields are renamed; prefer builders.
- **`BudgetContext`**: runtime context passed into tool handlers — holds a deadline (`Instant`), a `cancelled` flag, and `should_stop()` which checks both deadline expiry and cancellation. Helper methods: `check_not_cancelled(tool_name)`, `check_deadline(tool_name)`, `check_text_bytes(field, text, tool_name)`, `check_list_len(field, len, tool_name)`, `remaining_time_ms()`. `check_text_len` is retained as a deprecated alias for `check_text_bytes` (renamed in 1.1.4 because enforcement is byte-based, not character-based).
- **`for_handler(budget)`**: convenience constructor that picks up the thread-local cancellation flag. Recommended for handler functions with signature `fn(&Value) -> ToolResponse` that cannot receive context directly.
- **Composite sub-budgets**: `SubBudget` and `CompositeBudgetAllocator` allow composite tools (e.g., `edit_preflight`, `command_preflight`) to split their parent budget across child tool calls via `sub_budget_context()`. The allocator divides input/output/text/findings limits evenly across `N` sub-tools, sharing the parent deadline.

### Response Truncation (`src/mcp/response.rs`)

`truncate_response()` enforces budget limits on completed tool responses. When a tool produces more findings, output bytes, or text characters than its budget allows, the response is truncated and `limits_applied` is populated with descriptions of what was capped.

- **Findings cap** (`max_findings`): when findings exceed the cap, excess entries are dropped (sorted by severity, highest kept) and a synthetic `OUTPUT_TOO_LARGE` notice is appended as the final entry. One slot is always reserved for the notice, so the total findings length is ≤ `max_findings` — real findings are capped at `max_findings - 1` plus the notice.
- **Result truncation** (`max_output_bytes`): when the serialized `result` object exceeds the cap, it is **replaced** with a summary object containing only `machine_code`, `verdict`, `ok`, and any caller-supplied `summary` key, plus `truncated: true`, `original_size_bytes`, `max_output_bytes`. This guarantees the wire payload fits the budget while preserving route-critical fields. Caller-supplied `summary` strings are preserved rather than overwritten.
- `limits_applied` in the response envelope reports what was truncated.

### Input Pre-Check (`src/agent/mod.rs`)

`call_json_with_budget()` checks the serialized input against `budget.max_input_bytes` **before** dispatching to the handler. Oversized input fails with `INPUT_TOO_LARGE` (high severity, blocking). The MCP server's `tools/call` handler runs `truncate_response` *before* the `MAX_OUTPUT_BYTES` hard cap so response shaping happens once.

### Runtime Metrics (`src/mcp/response.rs`)

`CallMetrics` struct captures per-call resource usage (elapsed time, input bytes, output bytes before/after truncation, budget tier, limits applied count, sub-tool count). `CallMetricsBuilder` (via `.with_metrics()`) collects these during execution, attaching them as `_metrics` inside `result`. Fields with `None` values are skipped during serialization.

### Integration

1. `tools/call` resolves `ToolBudget` from `ToolSpec.cost` via `budget_for_tool()`
2. A `BudgetContext` is constructed with a deadline derived from `budget.max_elapsed_ms`
3. An `Arc<AtomicBool>` cancel flag is created and attached via `with_cancellation()`
4. The context is passed to the tool handler; `should_stop()` allows cooperative cancellation at key pipeline stages
5. On timeout, the cancel flag is set before returning a TIMEOUT error (cooperative — blocking work may continue)
6. After the handler returns, `truncate_response()` caps findings/output if the budget was exceeded
7. `limits_applied` in the response envelope reports what was truncated

Heavy and moderate tool handlers also create a `BudgetContext` via the free function `budget::for_handler(ToolBudget)` (which auto-attaches the thread-local cancel flag) and poll `should_stop()` at meaningful pipeline stages (after format detection, before sub-tool dispatches, in iteration loops). High-risk handlers (`edit_preflight`, `command_preflight`, `config_preflight`, `config_file_inspect`, `dependency_edit_preflight`, `text_security_inspect`) do the same at additional stages. Since `ToolHandler` signatures are `fn(&Value) -> ToolResponse`, the MCP server attaches the cancel flag to a shared context that handlers access via internal initialization.

For the in-process agent API, `call_json_with_budget()` on `ToolRegistry` accepts a custom `ToolBudget` to override the default per-tool limits. Input is pre-checked against `max_input_bytes` and rejected with `INPUT_TOO_LARGE` before handler dispatch.

## Response Contract

Every tool call returns a `ToolResponse` (defined in `src/mcp/response.rs`) with 11 fields:

| Field | Type | When present |
|-------|------|-------------|
| `ok` | bool | always |
| `tool` | string | usually present (set by handlers; technically optional) |
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
- `-32601`: Method not found (also modern `initialize` / removed `ping`)
- `-32600`: Invalid request (legacy lifecycle violations)
- `-32602`: Invalid params (tool validation; malformed modern `_meta`)
- `-32022`: Unsupported protocol version (modern `_meta` names an unsupported revision; `data: {supported, requested}`)
- `-32000`: Implementation-defined (capacity, handler panic; `RESOURCE_EXHAUSTED` uses `-32000` with data code)

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
| `full` | 77 | 86 | `argv_compare`, `canonicalize_text`, `cargo_toml_inspect`, `cidr_inspect`, `code_block_map`, `code_fence_extract`, `codec_convert`, `command_preflight`, `config_file_inspect`, `config_preflight`, `constant_lookup`, `cron_inspect`, `datetime_convert`, `dependency_edit_preflight`, `diff_risk_classify`, `dotenv_validate`, `edit_preflight`, `escape_text`, `glob_match`, `identifier_analyze`, `identifier_inspect`, `identifier_table_inspect`, `import_export_inspect`, `ini_validate`, `ip_inspect`, `json_canonicalize`, `json_compare`, `json_extract`, `json_query`, `json_shape`, `line_range_compare`, `line_range_extract`, `list_compare`, `list_dedupe`, `list_sort`, `lockfile_inspect`, `markdown_structure`, `math_eval`, `patch_contract_check`, `patch_summary`, `path_analyze`, `path_compare`, `path_normalize`, `radix_convert`, `regex_finditer`, `regex_safety_check`, `repo_language_detect`, `repo_manifest_inspect`, `repo_tree_summarize`, `shell_quote_join`, `structured_data_compare`, `symbol_name_diff`, `test_command_suggest`, `text_count`, `text_diff_explain`, `text_equal`, `text_fingerprint`, `text_hash`, `text_inspect`, `text_measure`, `text_position`, `text_replace_check`, `text_security_inspect`, `text_transform`, `text_truncate`, `text_window`, `toml_shape`, `unescape_text`, `unit_convert`, `unit_info`, `validate_brackets`, `validate_json`, `validate_regex`, `validate_schema_light`, `validate_toml`, `version_compare`, `version_constraint_check` | `patch_apply_check`, `path_batch_scope_check`, `path_scope_check`, `profile_inspect`, `prompt_input_inspect`, `runtime_diagnostics`, `shell_split`, `tool_availability_explain`, `unicode_policy_check` |
| `default` | 25 | 25 | `escape_text`, `glob_match`, `identifier_inspect`, `json_canonicalize`, `json_compare`, `line_range_extract`, `list_dedupe`, `list_sort`, `math_eval`, `path_normalize`, `regex_finditer`, `regex_safety_check`, `text_count`, `text_diff_explain`, `text_equal`, `text_fingerprint`, `text_inspect`, `text_measure`, `text_replace_check`, `text_window`, `unescape_text`, `validate_brackets`, `validate_json`, `validate_regex`, `validate_toml` |  |
| `codegg_core_min` | 6 | 6 | `command_preflight`, `config_preflight`, `edit_preflight`, `text_replace_check`, `text_security_inspect`, `validate_json` |  |
| `codegg_core` | 19 | 19 | `cargo_toml_inspect`, `code_block_map`, `command_preflight`, `config_preflight`, `edit_preflight`, `identifier_inspect`, `import_export_inspect`, `path_normalize`, `repo_language_detect`, `structured_data_compare`, `test_command_suggest`, `text_diff_explain`, `text_equal`, `text_fingerprint`, `text_inspect`, `text_replace_check`, `text_security_inspect`, `validate_json`, `validate_toml` |  |
| `codegg_preflight` | 7 | 13 | `command_preflight`, `config_preflight`, `dependency_edit_preflight`, `edit_preflight`, `lockfile_inspect`, `patch_contract_check`, `text_security_inspect` | `patch_apply_check`, `path_batch_scope_check`, `path_scope_check`, `prompt_input_inspect`, `shell_split`, `unicode_policy_check` |
| `codegg_patch` | 10 | 12 | `diff_risk_classify`, `edit_preflight`, `line_range_compare`, `line_range_extract`, `lockfile_inspect`, `patch_contract_check`, `patch_summary`, `symbol_name_diff`, `text_diff_explain`, `text_replace_check` | `patch_apply_check`, `path_batch_scope_check` |
| `codegg_config` | 14 | 14 | `config_file_inspect`, `config_preflight`, `dependency_edit_preflight`, `dotenv_validate`, `ini_validate`, `json_canonicalize`, `json_compare`, `json_extract`, `structured_data_compare`, `toml_shape`, `validate_json`, `validate_schema_light`, `validate_toml`, `version_compare` |  |
| `codegg_unicode_security` | 6 | 8 | `canonicalize_text`, `identifier_inspect`, `text_inspect`, `text_position`, `text_security_inspect`, `text_transform` | `prompt_input_inspect`, `unicode_policy_check` |
| `codegg_shell` | 5 | 6 | `argv_compare`, `command_preflight`, `regex_safety_check`, `shell_quote_join`, `test_command_suggest` | `shell_split` |
| `codegg_repo_audit` | 18 | 18 | `cargo_toml_inspect`, `code_block_map`, `code_fence_extract`, `config_file_inspect`, `dependency_edit_preflight`, `diff_risk_classify`, `identifier_table_inspect`, `import_export_inspect`, `json_shape`, `lockfile_inspect`, `markdown_structure`, `patch_contract_check`, `repo_language_detect`, `repo_manifest_inspect`, `repo_tree_summarize`, `symbol_name_diff`, `test_command_suggest`, `text_fingerprint` |  |
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

Two files are generated from the ToolSpec registry by `cargo run --features dev-tools --bin generate-docs`:

- **architecture/mcp-server.md** profile reference — per-profile tool counts and names (sections between `BEGIN GENERATED`/`END GENERATED` markers)
- **generated/tool-cards.md** — per-profile tool cards with required arguments

The generator reads `ToolSpec` entries directly from `src/mcp/specs/` (the single source of truth) and filters out tools with `ToolExposure::Hidden`. Run `cargo run --features dev-tools --bin generate-docs -- --check` to verify generated docs are current. The CI pipeline enforces this check.

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

The `--diagnostics` flag prints version, tool count, per-profile tool counts, budget tiers, runtime settings, known environment variable names (no values), generated data status, and parity-reference availability. Supports `--format json` for structured output. The `runtime_diagnostics` MCP tool provides similar information to harness-only audiences. Two additional companion tools — `profile_inspect` (per-profile metadata) and `tool_availability_explain` (why a tool is hidden) — are available as MCP-only tools for harness-side introspection.
