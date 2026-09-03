# CLI & Binaries

eggsact provides a CLI entry point (`eggsact`) and one utility binary (`generate-docs`).

See also: [Overview](overview.md), [CLI & Usage](../docs/cli.md)

## CLI (`src/main.rs`)

The CLI supports five modes via a single `CliCommand` enum:

```
Usage: eggsact [--mcp | --diagnostics [--format json|text] | expression]
  --mcp              Start MCP server mode
  --diagnostics      Print diagnostic information
  --format json|text Output format for --diagnostics (default: text)
  -h, --help         Print this help message
  -V, --version      Print version information
  expression         Evaluate math expression
```

### Modes

| Mode | Flag | Description |
|------|------|-------------|
| Help | `-h`, `--help`, or no args | Print usage information |
| Version | `-V`, `--version` | Print `eggsact {version}` |
| Evaluate | `expression` (any other args) | Evaluate a math expression via `eggsact::calc::run()` |
| MCP Server | `--mcp` | Start MCP stdio JSON-RPC 2.0 server |
| Diagnostics | `--diagnostics [--format json\|text]` | Print runtime diagnostics (default: text) |

### Expression Mode

When args don't match a recognized flag, they are joined with spaces and passed to `eggsact::calc::run()`. The result is printed to stdout; errors exit with code 1.

```bash
eggsact "thirty plus five"           # 35
eggsact "3 + 4 * 2"                  # 11
eggsact "30m to ft"                   # 98.4251968503937
eggsact "2 ** 10"                     # 1024
eggsact "sqrt(144)"                   # 12
eggsact "1 gallon to liter"           # 3.785411784
```

Note: use `**` for exponentiation (not `^`, which is XOR).

### MCP Server Mode

Starts the MCP stdio server. Reads JSON-RPC 2.0 messages from stdin and writes responses to stdout. The active profile is resolved from `EGGCALC_MCP_PROFILE` at startup.

```bash
eggsact --mcp
# Protocol: JSON-RPC 2.0 over stdio
# See architecture/mcp-server.md for full reference
```

The server sets `EGGCALC_NO_CONFIG=1` unconditionally before dispatching any mode (including MCP), preventing config file loading.

### Diagnostics Mode

Prints version, tool count, profile summary, budget tiers, runtime settings, and environment variable names (no values). Useful for verifying the build and checking active configuration.

```bash
eggsact --diagnostics              # text output
eggsact --diagnostics --format json  # JSON output
```

#### Text output example

```
eggsact diagnostics (v1.2.3)

Tools: 86 total

Profiles:
  full: 86 tools
  default: 25 tools
  codegg_core_min: 6 tools
  ...

Route-critical tools:
  edit_preflight
  command_preflight
  config_preflight
  patch_apply_check
  text_security_inspect

Compatibility mode (default by surface):
  MCP server:       EggcalcPython
  In-process API:   StrictNative

Runtime:
  Active profile: full
  Active audience: model
  Schema detail: full
  Limits: 32 in-flight, 16 workers, 1000000 bytes request, 1000000 bytes output

Budget tiers:
  cheap: 1 MB in/out, 10s, 100 findings
  moderate: 1 MB in/out, 30s, 100 findings
  heavy: 1 MB in / 2 MB out, 30s, 100 findings

Known env vars (names only, no values):
  EGGCALC_NO_CONFIG
  EGGCALC_MCP_PROFILE
  EGGCALC_MCP_AUDIENCE
  EGGCALC_MCP_SCHEMA_DETAIL
```

#### JSON output example

```json
{
  "version": "1.2.3",
  "tool_count": 86,
  "profiles": {
    "full": 86,
    "default": 25,
    "codegg_core_min": 6,
    "codegg_core": 19,
    "codegg_preflight": 13,
    "codegg_patch": 12,
    "codegg_config": 14,
    "codegg_unicode_security": 8,
    "codegg_shell": 6,
    "codegg_repo_audit": 18,
    "human_math": 4
  },
  "compatibility_mode": {
    "mcp_server": "EggcalcPython",
    "in_process_api": "StrictNative"
  },
  "route_critical_tools": [
    "edit_preflight",
    "command_preflight",
    "config_preflight",
    "patch_apply_check",
    "text_security_inspect"
  ],
  "budget_tiers": {
    "cheap": "1 MB in/out, 10s, 100 findings",
    "moderate": "1 MB in/out, 30s, 100 findings",
    "heavy": "1 MB in / 2 MB out, 30s, 100 findings"
  },
  "runtime": {
    "active_profile": "full",
    "active_audience": "model",
    "schema_detail": "full",
    "limits": {
      "max_in_flight_requests": 32,
      "max_tool_workers": 16,
      "max_request_bytes": 1000000,
      "max_output_bytes": 1000000
    }
  },
  "env_var_names": [
    "EGGCALC_NO_CONFIG",
    "EGGCALC_MCP_PROFILE",
    "EGGCALC_MCP_AUDIENCE",
    "EGGCALC_MCP_SCHEMA_DETAIL"
  ]
}
```

### Arg Parsing

`parse_args()` is a pure function that pattern-matches against a `Vec<String>` slice:

```rust
#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Help,
    Version,
    Mcp,
    Diagnostics { format: String },
    Evaluate(String),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> CliCommand
```

Matching rules (in order):

| Input | Result |
|-------|--------|
| `[]` (empty) | `Help` |
| `["-h"]` or `["--help"]` | `Help` |
| `["-V"]` or `["--version"]` | `Version` |
| `["--mcp"]` | `Mcp` |
| `["--diagnostics"]` | `Diagnostics { format: "text" }` |
| `["--diagnostics", "--format", "json"]` | `Diagnostics { format: "json" }` |
| Everything else | `Evaluate(args.join(" "))` |

Key design: the fallback case joins all remaining args with spaces, so `eggsact thirty plus five` works identically to `eggsact "thirty plus five"`.

### Unit Tests

`src/main.rs` contains 7 unit tests for arg parsing:

```bash
cargo test --lib main
```

| Test | Covers |
|------|--------|
| `parse_no_args_as_help` | Empty args → `Help` |
| `parse_help_flags` | `-h` and `--help` → `Help` |
| `parse_version_flags` | `-V` and `--version` → `Version` |
| `parse_mcp_flag` | `--mcp` → `Mcp` |
| `parse_expression_joins_all_remaining_args` | Multiple words → `Evaluate("thirty plus five")` |
| `parse_diagnostics_flag` | `--diagnostics` → `Diagnostics { format: "text" }` |
| `parse_diagnostics_format_json` | `--diagnostics --format json` → `Diagnostics { format: "json" }` |

---

## `generate-docs` Binary (`src/bin/generate_docs.rs`)

Generates documentation from the `ToolSpec` registry. The `ToolSpec` entries in `src/mcp/specs/` are the single source of truth; this binary reads them and produces two generated outputs.

```bash
cargo run --features dev-tools --bin generate-docs            # regenerate all docs (in-place)
cargo run --features dev-tools --bin generate-docs -- --check  # verify docs are current (CI)
cargo run --features dev-tools --bin generate-docs -- --output-dir /path  # write to a different directory
```

### What It Generates

| Output File | Content | Marker Pair |
|-------------|---------|-------------|
| `architecture/mcp-server.md` | Profile reference table — model/harness tool counts, tool names, harness-only tools | `<!-- BEGIN GENERATED: profile reference -->` / `<!-- END GENERATED: profile reference -->` |
| `generated/tool-cards.md` | Per-profile tool cards with description, tier, cost, stability, exposure, required args, and aliases | (whole file is generated) |

### How It Works

1. **Reads `ToolSpec` registry**: Calls `all_tools_vec()` and `tools_for_profile_audience()` from `src/mcp/registry/` to get the canonical tool list.

2. **Generates two content blocks**:
   - `generate_profile_reference()` — iterates all available profiles, counts model vs harness tools, lists harness-only tools per profile.
   - `generate_tool_cards()` — iterates 8 codegg profiles, generates per-tool cards with required args (extracted from JSON schemas), aliases, and composite flags.

3. **Marker-based insertion**: For `architecture/mcp-server.md`, the generator uses HTML comment markers to delimit generated sections. It strips all existing generated blocks (including orphaned/duplicated ones from prior failed runs) and inserts a fresh single block. If markers don't exist yet, they are appended.

4. **Writes `generated/tool-cards.md`**: This file is entirely generated (no markers needed).

### `--check` Mode

Used in CI to verify generated docs are current without modifying files:

```bash
cargo run --features dev-tools --bin generate-docs -- --check
# Exit code 0 = docs are current
# Exit code 1 = docs are stale (prints which files need updating)
```

In check mode, the generator compares generated content against existing files and reports mismatches. It does **not** write any files. The error message includes the exact command to regenerate:

```
Stale generated docs:
  architecture/mcp-server.md
Run `cargo run --features dev-tools --bin generate-docs` to regenerate.
```

### Orphan and Triplication Handling

The generator is resilient to malformed marker blocks. `find_all_generated_spans()` detects orphaned BEGIN markers (e.g., from a crashed prior run) and `strip_all_generated_blocks()` removes all generated content — well-formed and orphaned — before inserting a fresh block. Unit tests verify this behavior:

- `find_all_spans_handles_well_formed_block` — single BEGIN/END pair
- `find_all_spans_detects_orphans` — two BEGIN, one END → first well-formed, second orphan
- `find_all_spans_handles_triplication` — three BEGIN, one END → 1 well-formed, 2 orphans
- `strip_all_removes_triplicated_blocks` — cleans all blocks, preserves surrounding headings

### Internal Tests

The binary includes 14 unit tests covering generation invariants, marker spans, and orphan-block handling:

| Test | Purpose |
|------|---------|
| `generated_tool_cards_exclude_hidden_tools` | No hidden tool appears in tool cards |
| `profile_counts_match_registry` | Profile reference table counts match live registry |
| `profile_reference_includes_harness_only_tools` | Harness-only tools listed per profile |
| `tool_cards_reference_only_known_tools` | No unknown tool names in tool cards |
| `tool_card_required_args_match_schema` | Required args in cards match JSON schemas |
| `stale_docs_message_uses_cargo_bin_name` | Error message uses `generate-docs` (dash, not underscore) |
| `mcp_server_doc_markers_are_well_formed` | mcp-server.md has exactly one well-ordered marker pair |

### When to Regenerate

Run `cargo run --features dev-tools --bin generate-docs` whenever you:

- Add, remove, or rename an MCP tool in `src/mcp/specs/`
- Change a `ToolSpec`'s `profiles`, `exposure`, `cost`, `stability`, `description`, or `input_schema`
- Add a new profile in `src/mcp/registry/all_tools.rs`
- Change tool schema definitions (required args, types)

After regenerating, commit the updated files. CI will fail if generated docs are stale.

---

## Environment Variables

| Variable | Set In | Purpose | Values |
|----------|--------|---------|--------|
| `EGGCALC_NO_CONFIG` | `main.rs` (unconditionally) | Disables config file loading | `1` |
| `EGGCALC_MCP_PROFILE` | User / harness | Active profile for MCP server and in-process API | Profile name (e.g., `codegg_core_min`, `full`) |
| `EGGCALC_MCP_AUDIENCE` | User / harness | Active audience for tool exposure filtering | `Model` (default), `Harness`, `Debug` |
| `EGGCALC_MCP_SCHEMA_DETAIL` | User / harness | Controls schema compaction in tool listings | `compact`, `normal`, `full` (default) |

Notes:

- `EGGCALC_NO_CONFIG` is set to `"1"` by `main.rs` before any mode dispatch. It cannot be overridden from the environment.
- `EGGCALC_MCP_AUDIENCE` is case-insensitive; invalid values fall back to `Model`.
- `EGGCALC_MCP_SCHEMA_DETAIL` is case-insensitive; invalid values fall back to `full` with a stderr warning.
- `EGGCALC_MCP_PROFILE` is resolved once at server startup. There is no per-call profile override over MCP wire protocol.

---

## Build and Run Commands

### Building

```bash
cargo build                          # debug build
cargo build --release                # release build
```

### Running the CLI

```bash
# Expression mode
cargo run -- "3 + 4 * 2"             # 11
cargo run -- "30m to ft"             # 98.425...
cargo run -- "sqrt(144)"             # 12

# MCP server mode
cargo run -- --mcp

# Diagnostics
cargo run -- --diagnostics
cargo run -- --diagnostics --format json

# Help / version
cargo run -- --help
cargo run -- --version
```

### Running Utility Binaries

```bash
# Generate docs (regenerate)
cargo run --features dev-tools --bin generate-docs

# Generate docs (check mode, for CI)
cargo run --features dev-tools --bin generate-docs -- --check

# Run main.rs unit tests
cargo test --locked --lib main
```

### Full Verification Pipeline (Manual)

The recommended verification order before release:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked --verbose
```

Or equivalently, the canonical release gate:

```bash
scripts/release-check.sh
```

---

## Design Notes

- **No clap/structopt**: Arg parsing is hand-rolled via `parse_args()` pattern matching. This keeps the dependency tree minimal and makes the CLI behavior fully deterministic.
- **`EGGCALC_NO_CONFIG` is hardcoded**: The main binary always sets this env var, preventing any config file from affecting CLI behavior. This ensures reproducible output.
- **Diagnostics exposes names, not values**: The `--diagnostics` mode lists environment variable names but never reads or prints their values, avoiding secret leakage.
- **`generate-docs` uses marker-based insertion**: HTML comments delimit generated sections in markdown files. This allows hand-editing around generated content while keeping the generated parts reproducible.
