# CLI & Binaries

eggsact provides a CLI entry point and two utility binaries.

See also: [Overview](overview.md)

## CLI (`src/main.rs`)

The CLI supports four modes:

```
Usage: eggsact [--mcp | --diagnostics [--format json|text] | expression]
```

### Modes

| Mode | Flag | Description |
|------|------|-------------|
| Help | `-h`, `--help`, or no args | Print usage |
| Version | `-V`, `--version` | Print version |
| Evaluate | `expression` (any other args) | Evaluate a math expression |
| MCP Server | `--mcp` | Start MCP server (stdio JSON-RPC 2.0) |
| Diagnostics | `--diagnostics` | Print runtime diagnostics |

### Expression Mode

When args don't match a flag, they are joined with spaces and passed to `eggsact::calc::run()`:

```bash
eggsact "thirty plus five"           # 35
eggsact "3 + 4 * 2"                  # 11
eggsact "30m to ft"                   # 98.425...
eggsact "2 ** 10"                     # 1024
```

### MCP Server Mode

Starts the MCP stdio server:

```bash
eggsact --mcp
# Reads JSON-RPC 2.0 from stdin, writes to stdout
```

See [MCP Server](mcp-server.md) for protocol details.

### Diagnostics Mode

Prints version, tool count, profile summary, budget tiers, and environment variable names (no values):

```bash
eggsact --diagnostics           # text output
eggsact --diagnostics --format json  # JSON output
```

JSON output example:

```json
{
  "version": "1.1.3",
  "tool_count": 71,
  "profiles": { "full": 71, "default": 25, ... },
  "compatibility_mode": { "mcp_server": "EggcalcPython", "in_process_api": "StrictNative" },
  "budget_tiers": { "cheap": "...", "moderate": "...", "heavy": "..." },
  "env_var_names": ["EGGCALC_NO_CONFIG", "EGGCALC_MCP_PROFILE", ...],
  "generated_data": { "confusables_generated_rs": true },
  "parity_reference": { "path": "../eggcalc", "exists": false }
}
```

### Arg Parsing

`parse_args()` is a pure function that matches against `Vec<String>`:

```rust
fn parse_args(args: impl IntoIterator<Item = String>) -> CliCommand
```

Returns `CliCommand` enum: `Help`, `Version`, `Mcp`, `Diagnostics { format }`, `Evaluate(String)`.

## `generate-docs` Binary (`src/bin/generate_docs.rs`)

Generates documentation from the ToolSpec registry:

```bash
cargo run --bin generate-docs          # regenerate all docs
cargo run --bin generate-docs -- --check  # verify docs are current (CI)
```

### Generated Files

| File | What It Generates |
|------|------------------|
| `README.md` | Tool table with all non-hidden tools by category |
| `architecture/mcp-server.md` | Profile reference table (sections between `BEGIN GENERATED`/`END GENERATED`) |
| `generated/tool-cards.md` | Per-profile tool cards with required arguments |

The generator reads `ToolSpec` entries directly from `src/mcp/specs/` (the single source of truth) and filters out tools with `ToolExposure::Hidden`.

### `--check` Mode

Used in CI to verify generated docs are current. Exits with error if any generated file would change.

## `verify-eggsact` Binary (`src/bin/verify_eggsact.rs`)

Runs a 5-step verification pipeline:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-features`
4. `cargo build --release`
5. `cargo package --verbose`

Optional parity check if `../eggcalc` exists.

Reports results as markdown.

## Environment Variables

| Variable | Set In | Purpose |
|----------|--------|---------|
| `EGGCALC_NO_CONFIG` | `main.rs` | Disables config file loading |
| `EGGCALC_MCP_PROFILE` | User | Active profile for MCP server |
| `EGGCALC_MCP_AUDIENCE` | User | Active audience (`Model`/`Harness`/`Debug`) |
| `EGGCALC_MCP_SCHEMA_DETAIL` | User | Schema compaction control |

## Unit Tests

`src/main.rs` contains unit tests for arg parsing:

```bash
cargo test --lib main
```

Tests cover: no args, help flags, version flags, MCP flag, expression joining, diagnostics flags, diagnostics format.
