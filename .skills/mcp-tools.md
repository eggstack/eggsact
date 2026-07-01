# Skill: Adding or Updating MCP Tools

Use this when adding a new MCP tool or modifying an existing one.

## Checklist

1. **Implement the function** in `src/mcp/tools.rs`:
   - Take `&Value` (serde_json) as the input parameter
   - Validate arguments at the boundary
   - Call reusable library code from `src/text/` or `src/calc/`
   - Return `ToolResponse` (from `src/mcp/schemas.rs`)

2. **Add a `ToolSpec` entry** in `src/mcp/registry.rs` — this is the single source of truth for tool registration. It defines the handler, category, tier, tags, profiles, input schema, and output schema all in one place.

3. **Run the invariant test** to verify sync:
   ```bash
   cargo test tool_registration_tables_are_in_sync -- --nocapture
   ```

4. **Add tests** at the right layer:
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
- Use `finding()`, `finding_with_location()`, or `prompt_finding()` (from `src/mcp/response.rs`) to build structured findings with codes and severity.
- Use `severity::*`, `disposition::*`, and `verdict::*` constants for finding metadata.

See `architecture/machine-codes.md` for the full code table and design rationale.

## In-Process Execution Path

`ToolRegistry` (`src/agent/mod.rs`) provides the core tool execution path. Both the MCP server (`src/mcp/server.rs`) and direct Rust callers use it for tool lookup, profile filtering, argument validation, and dispatch. Tool functions themselves live in `src/mcp/tools.rs`; `ToolRegistry` orchestrates calling them.

## Composite Tools

Tools marked `composite: true` orchestrate calls to other tools internally.
Examples: `text_security_inspect`, `edit_preflight`, `command_preflight`, `config_preflight`, `structured_data_compare`.
These are implemented in `src/text/synthesis.rs` and wrapped in `src/mcp/tools.rs`.

## Adding a Text Processing Module

1. Create `src/text/<module>.rs` with the implementation
2. Add `pub mod <module>;` to `src/text/mod.rs` and re-export key functions
3. Add MCP tool wrapper in `src/mcp/tools.rs`
4. Add a `ToolSpec` entry in `src/mcp/registry.rs`
5. Add tests in `tests/text/test_<module>.rs`
6. Update `architecture/text-library.md` if significant
