# Skill: Adding or Updating MCP Tools

Use this when adding a new MCP tool or modifying an existing one.

## Checklist

1. **Implement the function** in `src/mcp/tools.rs`:
   - Take `&Value` (serde_json) as the input parameter
   - Validate arguments at the boundary
   - Call reusable library code from `src/text/` or `src/calc/`
   - Return `ToolResponse` (from `src/mcp/schemas.rs`)

2. **Register in 4 places** in `src/mcp/server.rs`:
   - `TOOL_HANDLERS` (line ~30) — dispatch table mapping name → handler fn
   - `TOOL_METADATA` (line ~97) — category, tier, tags, profiles
   - `list_tools_raw()` (line ~1379) — tool definition with input schema
   - `OUTPUT_SCHEMAS` (line ~1310) — output JSON schema

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
ToolMetadata {
    category: "text",        // category name (matches existing)
    tier: 0,                 // 0=essential, 1=common, 2=advanced, 3=specialized
    profiles: &["default"],  // feature profiles
    tags: &["measure"],      // searchable tags
    llm_exposure: "full",    // "full", "indirect", "internal"
    harness_use: true,       // whether to show in tool summaries
    aliases: &[],            // alternative tool names
    cost: "cheap",           // "cheap", "moderate", "expensive"
    stability: "stable",     // "stable", "experimental", "deprecated"
    composite: false,        // true if this tool calls other tools internally
}
```

## Composite Tools

Tools marked `composite: true` orchestrate calls to other tools internally.
Examples: `text_security_inspect`, `edit_preflight`, `command_preflight`, `config_preflight`, `structured_data_compare`.
These are implemented in `src/text/synthesis.rs` and wrapped in `src/mcp/tools.rs`.

## Adding a Text Processing Module

1. Create `src/text/<module>.rs` with the implementation
2. Add `pub mod <module>;` to `src/text/mod.rs` and re-export key functions
3. Add MCP tool wrapper in `src/mcp/tools.rs`
4. Register in all 4 places in `server.rs`
5. Add tests in `tests/text/test_<module>.rs`
6. Update `architecture/text-library.md` if significant
