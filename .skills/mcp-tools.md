# Skill: Adding or Updating MCP Tools

Use this when adding a new MCP tool or modifying an existing one.

## Checklist

1. **Implement the function** in `src/mcp/tools.rs`:
   - Take `&Value` (serde_json) as the input parameter
   - Validate arguments at the boundary
   - Call reusable library code from `src/text/` or `src/calc/`
   - Return `ToolResponse` (from `src/mcp/schemas.rs`)

2. **Add a `ToolSpec` entry** in `src/mcp/specs/<category>.rs` — this is the single source of truth for tool registration. It defines the handler, category, tier, tags, profiles, input schema, and output schema all in one place. Each category exports a `pub const <CATEGORY>_TOOLS: &[ToolSpec]` slice, which `all_tools.rs` aggregates into the combined `ALL_TOOLS`.

3. **Run the invariant test** to verify sync:
   ```bash
   cargo test tool_registration_tables_are_in_sync -- --nocapture
   ```

4. **Regenerate docs** from the registry:
   ```bash
   cargo run --bin generate-docs
   ```
   This updates README tool tables, architecture profile references, and `generated/tool-cards.md`. Commit the generated files alongside your ToolSpec changes.

5. **Add tests** at the right layer:
   - Unit tests: `src/mcp/tools.rs` (inline `#[cfg(test)]`)
   - MCP protocol tests: `tests/mcp/`
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

See `architecture/machine-codes.md` for the full code table and design rationale.

## In-Process Execution Path

`ToolRegistry` (`src/agent/mod.rs`) provides the core tool execution path. Both the MCP server (`src/mcp/server.rs`) and direct Rust callers use it for tool lookup, profile filtering, argument validation, and dispatch. Tool functions themselves live in `src/tools/*.rs` (by category); `ToolRegistry` orchestrates calling them.

Tool listing and filtering lives in `src/mcp/registry/listing.rs`, including `list_tool_definitions()` (used by the MCP `tools/list` handler), audience-aware listing, and schema compaction.

### Context-Aware APIs

For new tool integrations, prefer `call_json_with_execution_context()` over legacy `call_json()`. The `ExecutionContext` bundles eval context, compatibility mode, profile, audience, budget, and cancellation into a single per-request struct. Tool handler signatures remain `fn(&Value) -> ToolResponse` for compatibility — context is applied at the orchestration layer, not passed into handlers. Calculator-backed handlers retrieve `EvalContext` from a thread-local set by `budget::with_eval_context()`.

**Key invariant**: `ctx.eval_ctx` is **cloned** at dispatch; PRNG draws, memory mutations, and variable assignments inside the handler operate on the clone and **do not persist back** to the caller's `ExecutionContext`. Two calls with identical seeds produce the same first random value.

For calculator operations, use `evaluate_with_context()` / `run_with_context()` when you need persistent mutable `EvalContext` behavior across multiple calls (PRNG draws accumulate, memory registers persist, user variables accumulate). These operate directly on the caller's `ctx`.

Do not mix `call_json_with_execution_context` with `evaluate_with_context`/`run_with_context` for the same `EvalContext` — the former clones the context so handler mutations are invisible to the caller's `ctx`.

## Composite Tools

Tools marked `composite: true` orchestrate calls to other tools internally.
Examples: `text_security_inspect`, `edit_preflight`, `command_preflight`, `config_preflight`, `structured_data_compare`.
These are implemented in `src/text/synthesis.rs` and wrapped in `src/mcp/tools.rs`.

`edit_preflight` optionally composes additional tools when the corresponding input fields are provided: `path_scope_check` (via `file_path` + `workspace_root` fields), `text_security_inspect` (via `unicode_policy` field), and `text_fingerprint` (via `newline_policy` field for newline style detection). Each sub-tool call is included in the `subresults` map when invoked.

## Adding a Text Processing Module

1. Create `src/text/<module>.rs` with the implementation
2. Add `pub mod <module>;` to `src/text/mod.rs` and re-export key functions
3. Add MCP tool wrapper in `src/mcp/tools.rs`
4. Add a `ToolSpec` entry in `src/mcp/specs/<category>.rs`
5. Add tests in `tests/text/test_<module>.rs`
6. Update `architecture/text-library.md` if significant
