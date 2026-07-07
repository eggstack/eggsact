# Skill: Debugging eggsact

Use this when diagnosing issues in the codebase.

## Common Issues

### Tool Registration Drift

If `tool_registration_tables_are_in_sync` fails, you've modified the ToolSpec registry (`src/mcp/specs/<category>.rs`) without running `cargo run --bin generate-docs`. See `.skills/mcp-tools.md` for the complete list.

### Parity Test Failures

Parity tests compare Rust vs Python output. The parity suite has known gaps as of
2026-07-04 — see `docs/parity.md` `Verification status` and `Known parity gaps`
for the current 53-failure breakdown (categorized as test-harness audience bug,
tool/output drift, and a 3-tool gap). When investigating a new parity failure:

1. Check if the Python server is running at `../eggcalc`
2. Compare the specific tool call and arguments
3. Check `docs/parity.md` for known differences
4. If the difference is intentional, update the parity test

### Build Failures

```bash
cargo fmt --all -- --check           # check formatting first (CI-equivalent)
cargo clippy --all-targets --all-features -- -D warnings  # check lint
```

### Confusables Data Stale

If confusable character detection seems wrong:
```bash
python3 scripts/generate_confusables.py  # regenerate from Unicode.org
```
Never edit `src/text/confusables_generated.rs` directly.

### MCP Server Issues

Test the MCP server interactively:
```bash
# Initialize
echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | cargo run -- --mcp

# List tools
echo '{"jsonrpc":"2.0","method":"tools/list","id":2}' | cargo run -- --mcp

# Call a tool
echo '{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"math_eval","arguments":{"expression":"2+3"}}}' | cargo run -- --mcp
```

### Unit Conversion Issues

- `g` means gram in unit expressions, not gravity
- Use `gravity` or `standardgravity` for standard gravity
- Temperature conversions use offset math, not multiplicative factors
- Prefixed units (`kN`, `mV`, `mA`) are supported

### Expression Evaluation Gotchas

- `^` is XOR, not exponentiation — use `**` for power
- Complex numbers use `j` suffix (e.g., `3+4j`)
- `evaluate()` does NOT parse natural language — use `run()` for that

## Input Limits

| Limit | Value | Applies to |
|-------|-------|------------|
| MAX_TEXT_LENGTH | 100,000 | All text inputs |
| MAX_EXPRESSION_LENGTH | 10,000 | math_eval expression |
| MAX_LIST_ITEMS | 10,000 | Array parameters |
| MAX_REGEX_SAMPLES | 100 | validate_regex samples |
| MAX_PATTERN_LENGTH | 1,000 | regex_safety_check pattern |
| MAX_REQUEST_BYTES | 1,000,000 | MCP request body |
| MAX_OUTPUT_BYTES | 1,000,000 | MCP response body |

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `EGGCALC_NO_CONFIG=1` | Disable config file loading (set in main.rs) |
| `EGGCALC_MCP_PROFILE` | Select MCP tool profile |
| `EGGCALC_MCP_AUDIENCE` | Select MCP audience (Model, Harness, Debug) |
| `EGGCALC_MCP_SCHEMA_DETAIL` | Control schema detail level |
