---
name: mcp-tools
description: Use when adding a new MCP tool, modifying an existing tool, working with ToolSpec entries, machine codes, preflight wrappers, composite tools, or typed preflight wrappers in the eggsact codebase.
---

## Checklist

1. **Implement the function** in `src/tools/<category>.rs`:
   - Take `&Value` (serde_json) as the input parameter
   - Validate arguments at the boundary
   - Call reusable library code from `src/text/`, `src/calc/`, or `src/services/` — never call another `crate::tools::*` handler for an internal result
   - Return `ToolResponse` (from `src/mcp/response.rs`)

2. **Add a `ToolSpec` entry** in `src/mcp/specs/<category>.rs` — this is the single source of truth for tool registration. It defines the handler, category, tier, tags, profiles, input schema, and output schema all in one place. Each category exports a `pub const <CATEGORY>_TOOLS: &[ToolSpec]` slice, which `all_tools.rs` aggregates into the combined `ALL_TOOLS`.

3. **Run the invariant test** to verify sync:
   ```bash
   cargo test tool_registration_tables_are_in_sync -- --nocapture
   ```

4. **Regenerate docs** from the registry:
   ```bash
   cargo run --features dev-tools --bin generate-docs
   ```
   This updates the profile reference block in `architecture/mcp-server.md` and `generated/tool-cards.md`. (It does not touch README — that file is hand-maintained.) Commit the generated files alongside your ToolSpec changes.

   Discovery facades (`tool_search`, `tool_invoke` in `src/mcp/discovery.rs`)
   are MCP-only and intentionally excluded from `ALL_TOOLS_VEC`,
   `src/tools/`, and generated tool-cards. Do not add them as `ToolSpec`
   entries.

5. **Add tests** at the right layer:
   - Unit tests: `src/tools/<category>.rs` (inline `#[cfg(test)]`)
   - MCP protocol tests: `tests/mcp/`
   - Discovery/search routing: `tests/mcp/test_discovery.rs` (pinned count,
     byte budgets, profile/audience bypass, deprecated/recursion guards)
   - Library behavior tests: `tests/text/` or `tests/calc/`
   - Python parity: `tests/parity/` using `compare_tool_parity()`

## Tool Metadata Schema

```rust
ToolSpec {
    name: "my_tool",
    description: "What the tool does",
    handler: my_tool_handler,
    input_schema: my_tool_input,
    output_schema: my_tool_output,
    category: "text",
    tier: 0,                     // 0=essential, 1=common, 2=advanced, 3=specialized
    profiles: &["full", "default"],
    tags: &["text", "measure"],
    exposure: ToolExposure::Default,  // Default, Contextual, ExpertOnly, HarnessOnly, Hidden
    harness_use: &["none"],           // or ["edit_preflight"], ["command_preflight"], etc.
    aliases: &[],
    cost: ToolCost::Cheap,            // Cheap, Moderate, Heavy
    stability: ToolStability::Stable, // Stable, Deprecated, Experimental
    composite: false,
}
```

### Exposure Levels

| Exposure | When to use |
|----------|-------------|
| `Default` | Safe, cheap, broadly useful model-visible tools |
| `Contextual` | Useful when workflow calls for the category |
| `ExpertOnly` | Specialized tools for manager/reviewer agents |
| `HarnessOnly` | Harness calls automatically; model should not see |
| `Hidden` | Internal/compatibility; debug contexts only |

New capabilities must consider discovery: every stable Model tool is
searchable by exact name and invokable through `tool_invoke` policy, so
write discriminating descriptions (what result, when to choose vs nearest
competitor, one critical limitation), avoid call-graph prose (“calls X, Y,
Z”), and add aliases only for observed model/user vocabulary — never
speculative synonym lists. `tier`/`tags`/`cost` stay server-side
discovery inputs, not model prompt tokens.

### Presentation Surface (`McpSurface`)

`Direct` advertises the profile/audience-filtered catalog; `Discovery`
(`EGGSACT_MCP_SURFACE=discovery` or `--mcp-surface discovery`) advertises
only the pinned front doors plus `tool_search`/`tool_invoke`. Profiles
remain the capability boundary — presentation never authorizes. Keep the
pinned set small and reviewable in `src/mcp/discovery.rs`; do not add a
`presentation` field to all 86 `ToolSpec` entries without evidence.

### Audience Filtering

Use `tools_for_profile_audience(profile, audience)` for filtered listings:
- `Model`: excludes HarnessOnly + Hidden
- `Harness`: excludes Hidden
- `Debug`: all non-hidden tools

## Machine Codes

Every non-OK `ToolResponse` must carry a `machine_code`. Use constants from `src/mcp/machine_codes.rs` — never string literals.

- Use `ToolResponse::error_with_code(error_type, machine_code, error, hints, tool)` for error responses.
- Use `.with_machine_code(code)` on a success response when the code conveys meaningful routing info.
- Use `finding(code, severity, message, details, disposition)` (from `src/mcp/response.rs`) to build structured findings with codes, severity, and disposition.
- Use `severity::*` (`info`, `low`, `medium`, `high`, `critical`), `disposition::*` (`informational`, `caution`, `blocking`), and `verdict::*` constants for finding metadata.
- Every UPPERCASE_SNAKE finding `code` emitted by a route-critical tool must be in `machine_codes::ALL`. Add the constant to `ALL` first, then use it via `machine_codes::FOO` (not a raw string). Enforced by `test_route_critical_finding_codes_are_enumerated` in `tests/mcp/test_route_contracts.rs`.

### Composite / Preflight Tools

- Use `.with_verdict(verdict)` to set the verdict field inside result JSON.
- Use `preflight_allow(tool)`, `preflight_review(tool, findings)`, or `preflight_block(tool, machine_code, findings)` for quick preflight response construction.
- Use `ToolResponse::next_tool(name, reason, arguments_hint)` for structured `recommended_next_tool`.
- **Cooperative budget checks**: Heavy and moderate handlers (e.g. `edit_preflight`, `command_preflight`, `config_preflight`) should create a `BudgetContext` via the free function `budget::for_handler(ToolBudget::HEAVY)` (or the tier matching the tool's `cost`) and call `should_stop()` at key pipeline stages to respect cancellation and timeout signals. `for_handler` automatically picks up the thread-local cancellation flag set by the dispatcher. This is cooperative — it does not force-stop the handler, so check at natural boundaries (before expensive I/O, after sub-tool calls).

See `architecture/machine-codes.md` for the full code table and design rationale.

## In-Process Execution Path

`ToolRegistry` (`src/agent/mod.rs`) provides the core tool execution path. Both the MCP server (`src/mcp/server.rs`) and direct Rust callers use it for tool lookup, profile filtering, argument validation, and dispatch. Tool functions themselves live in `src/tools/*.rs` (by category); `ToolRegistry` orchestrates calling them.

**Execution routing:** Budget-aware APIs (`call_json_with_budget`, `call_json_with_context`, `call_json_with_execution_context`) route through the `SyncExecutionPool` (8 workers, 32-slot queue) in `src/mcp/sync_pool.rs`, which enforces elapsed-time budgets and provides bounded concurrency. `call_json` dispatches directly without a pool. The MCP server path is unaffected — it uses Tokio `spawn_blocking`.

Tool listing and filtering lives in `src/mcp/registry/listing.rs`, including `list_tool_definitions()` (legacy `tools/list`) and `list_modern_tool_values()` (modern `2026-07-28` shape with `annotations` + namespaced `_meta`), audience-aware listing, and schema compaction.

### Dual-Era Protocol (`2026-07-28` + Legacy, Connection-Pinned)

- One stdio process is one connection pinned by its first classifiable message (`ConnectionEra::Undecided` → `Legacy` for `initialize` or any claim-less opening, → `Modern20260728` for a valid modern claim). Legacy uses `initialize` → `notifications/initialized` + `SessionState` (inside Legacy-pinned connections only). Modern validates `_meta` (`io.modelcontextprotocol/protocolVersion` + `clientCapabilities`, optional `clientInfo`) per request; the official auto-negotiation `server/discover` probe is enveloped and runs in a disposable sibling process. Claim-less `server/discover` is legacy traffic and never bypasses lifecycle state.
- Cross-era requests after pinning return `-32022`/`Unsupported protocol version` (id preserved); mismatched notifications are dropped before lifecycle or cancellation side effects. Malformed/unsupported modern claims (`-32602`/`-32022`) and failed method-level `initialize` validation do not switch the connection. The era lock is held only for inspect/select — never across tool execution. Exactly one concurrent opening wins.
- `tool_invoke` is a routing facade with target-specific results and intentionally has no facade-level output schema (removed `tool_invoke_output_schema()`); modern `structuredContent` is the target's normal result. Get target contracts via `tool_search(detail="schema")` or direct/full mode.
- Era branching lives at the envelope/serialization boundary (`src/mcp/server.rs` + `parse_modern_request_meta`). Do not duplicate validation, audience/profile checks, budget selection, or execution between eras — use `handle_tools_list_shared`, `handle_tools_call_shared`, `handle_profiles_list_shared` with a `modern` flag.
- Modern `tools/list` adds `resultType`, `ttlMs` (`MODERN_CACHE_TTL_MS`), `cacheScope` (`public`), `_meta` server identity, and standard `annotations`; registry metadata (`tier`, `tags`, `category`, `llm_exposure`, `cost`) lives under `io.github.eggstack/eggsact`, never top-level. Legacy serialization is unchanged.
- Modern `tools/call` adds `structuredContent` (= `ToolResponse.result`, conforming to `outputSchema`) + `resultType`/`_meta`; `isError` retained, no `structuredContent` on tool errors. Modern `ping` is removed (`-32601`); malformed envelopes are `-32602`, unsupported versions `-32022` with `{supported, requested}`.
- Annotations are uniform (`readOnlyHint: true`, `openWorldHint: false`, etc.) via `annotations_for_spec`; if a future tool violates the claim, branch there and update `test_modern_protocol` invariants. Modern and legacy must agree on tool semantics — add cross-era goldens for representative tools in `tests/mcp/test_modern_protocol.rs`.

### Context-Aware APIs

For new tool integrations, prefer `call_json_with_execution_context()` over legacy `call_json()`. The `ExecutionContext` bundles eval context, compatibility mode, profile, audience, budget, and cancellation into a single per-request struct. Tool handler signatures remain `fn(&Value) -> ToolResponse` for compatibility — context is applied at the orchestration layer, not passed into handlers. Calculator-backed handlers retrieve `EvalContext` from a thread-local set by `budget::with_eval_context()` via `budget::with_current_eval_context()` (closure-scoped access).

`call_json_with_execution_template()` is an explicit immutable alias for `call_json_with_execution_context`. `call_json_with_execution_context_mut()` (deprecated since 1.0.0) accepts `&mut ExecutionContext` and persists handler state mutations back to the caller's context — use for sequential calculator operations where state should accumulate. Note: it does **not** persist calculator state through `math_eval` (math_eval's evaluator runs in a `catch_unwind` closure and the MCP dispatch creates fresh `EvalContext` per call). For persistent calculator state, use `evaluate_with_context()`/`run_with_context()` directly.

**Key invariant**: `ctx.eval_ctx` is **cloned** at dispatch for `call_json_with_execution_context` and `call_json_with_execution_template`; PRNG draws, memory mutations, and variable assignments inside the handler operate on the clone and **do not persist back** to the caller's `ExecutionContext`. Two calls with identical seeds produce the same first random value. For `call_json_with_execution_context_mut` (deprecated since 1.0.0), the `EvalContext` is shared directly and mutations persist — but it does **not** persist calculator state through `math_eval`.

For calculator operations, use `evaluate_with_context()` / `run_with_context()` when you need persistent mutable `EvalContext` behavior across multiple calls (PRNG draws accumulate, memory registers persist, user variables accumulate). These operate directly on the caller's `ctx`.

Do not mix `call_json_with_execution_context` with `evaluate_with_context`/`run_with_context` for the same `EvalContext` — the former clones the context so handler mutations are invisible to the caller's `ctx`. Do not mix `call_json_with_execution_context_mut` and `evaluate_with_context` for the same `EvalContext` — prefer `evaluate_with_context()`/`run_with_context()` for persistent calculator state.

## Typed Preflight Wrappers

`src/preflight/mod.rs` provides typed Rust wrappers over the raw JSON tool interface for common codegg workflows. Each wrapper has typed `Input`/`Output` structs, a `run()` method, and a `parse_response()` method for testing contract parsing without a full registry call.

Available wrappers: `EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`, `DependencyPreflight`.

All wrappers return `Result<Output, PreflightError>`. `PreflightError` distinguishes `ToolCall` (registry rejected), `ToolRejected` (tool returned `ok: false`), and `ContractViolation` (missing mandatory field — hard failure, no silent defaults).

Do not create a typed wrapper per tool: add typed APIs only for stable agent workflows where route-critical fields need compile-time contracts. Patch-review and repo-audit facades were evaluated and declined — `PatchAnalysis`/`RepoFacts` projections already answer those workflows through `ToolRegistry`. See `architecture/preflight.md`.

Add `#[allow(deprecated)]` to test code that calls `ToolRegistry::available_tools()`.

## Composite Tools

Tools marked `composite: true` orchestrate typed cores/services internally — they do not call sibling tool handlers.
Examples: `text_security_inspect` (`services::inspect_text_security`), `edit_preflight` (replace/line-range/patch cores + `services::fingerprint_facts`/`newline_facts`/`inspect_text_security` + `text::path_scope_check`), `command_preflight` (`text::regex_safety_check` core), `config_preflight` (`text::validate_json`/`validate_schema_light`/`json_canonicalize`/`toml`/`dotenv`/`ini`/`cargo` cores), `structured_data_compare` (`text::validate_json` core).
These are implemented in `src/tools/` (category modules) with ToolSpec declarations in `src/mcp/specs/`.

`edit_preflight` optionally composes additional typed checks when the corresponding input fields are provided: `text::path_scope_check` (via `file_path` + `workspace_root` fields), `services::inspect_text_security` (via `unicode_policy` field), and `services::fingerprint_facts`/`newline_facts` (via `newline_policy`/`expected_fingerprint` fields for newline style detection and SHA-256). Each sub-result is included in the `subresults` map when computed.

Shared composite logic that more than one caller needs lives in `src/services/` (`FingerprintFacts`, `NewlineFacts`, `SecurityInspection`, `RepoFacts`, `PatchAnalysis`). Services never touch `ToolResponse`, the registry, profiles/audiences, or schema validation; cancellation uses a lightweight `should_stop` view. Repo tools (`repo_manifest_inspect`, `repo_tree_summarize`, `repo_language_detect`, `test_command_suggest`) project `RepoFacts`; patch tools (`patch_summary`, `patch_contract_check`, `diff_risk_classify`) project/apply policy over `PatchAnalysis` (single parse, canonical repo buckets). The three remaining same-module JSON reuses (`json_compare`/`json_shape_tool` in `structured_data_compare`, `toml_shape_tool` in `config_preflight`) are intentional, commented, and deferred to follow-up typed extraction.

## Adding a Text Processing Module

1. Create `src/text/<module>.rs` with the implementation
2. Add `pub mod <module>;` to `src/text/mod.rs` and re-export key functions
3. Add tool function in `src/tools/<category>.rs`
4. Add a `ToolSpec` entry in `src/mcp/specs/<category>.rs`
5. Add tests in `tests/text/test_<module>.rs`
6. Update `architecture/text-library.md` if significant

### Deterministic utility categories

The network, encoding, and temporal categories are `full`-profile-only
contextual tools. Keep them exact-input/exact-output: do not add system clock,
IANA timezone, locale, filesystem, network, environment, or random-state
lookups. `datetime_convert` uses fixed offsets and decimal timestamp strings;
`cron_inspect` is a bounded five-field parser with Vixie/Cronie star-syntax
DOM/DOW matching: when neither field starts with `*`, either parsed field
may match; when either field starts with `*`, including supported `*/n`
step forms, both parsed predicates must match. Bare `*` behaves as the
familiar wildcard because its value set already contains every value;
explicit full ranges/lists are not equivalent to star syntax.
`codec_convert` must validate before canonicalizing, and
`radix_convert` is signed-magnitude `u128` only.
