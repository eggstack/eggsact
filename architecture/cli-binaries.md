# CLI & Binaries

eggsact provides a CLI entry point (`eggsact`) and two utility binaries (`generate-docs`, `verify-eggsact`).

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

Prints version, tool count, profile summary, budget tiers, runtime settings, generated data status, and environment variable names (no values). Useful for verifying the build and checking active configuration.

```bash
eggsact --diagnostics              # text output
eggsact --diagnostics --format json  # JSON output
```

#### Text output example

```
eggsact diagnostics (v0.3.0)

Tools: 80 total

Profiles:
  codegg_core: 42 tools
  codegg_core_min: 25 tools
  full: 80 tools
  ...

Route-critical tools:
  edit_preflight
  command_preflight
  config_preflight
  patch_apply_check
  text_security_inspect

Generated-doc command: cargo run --bin generate-docs
Verification command:  cargo run --bin verify-eggsact

Compatibility mode (default by surface):
  MCP server:       EggcalcPython
  In-process API:   StrictNative

Runtime:
  Active profile: codegg_core_min
  Active audience: Model
  Schema detail: full
  Limits: 10 req/s, 32 in-flight, 16 workers, 1000000 bytes request, 1000000 bytes output

Budget tiers:
  cheap: 1 MB in/out, 10s, 100 findings
  moderate: 1 MB in/out, 30s, 100 findings
  heavy: 1 MB in / 2 MB out, 30s, 100 findings

Known env vars (names only, no values):
  EGGCALC_NO_CONFIG
  EGGCALC_MCP_PROFILE
  EGGCALC_MCP_AUDIENCE
  EGGCALC_MCP_SCHEMA_DETAIL

confusables_generated.rs exists: yes
tool-cards.md exists:            yes
../eggcalc parity ref exists:    no
```

#### JSON output example

```json
{
  "version": "0.3.0",
  "tool_count": 80,
  "profiles": {
    "full": 80,
    "default": 25,
    "codegg_core_min": 25,
    "codegg_core": 42,
    "codegg_preflight": 28,
    "codegg_patch": 31,
    "codegg_config": 30,
    "codegg_unicode_security": 28,
    "codegg_shell": 29,
    "codegg_repo_audit": 34
  },
  "generated_doc_command": "cargo run --bin generate-docs",
  "verification_command": "cargo run --bin verify-eggsact",
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
    "active_profile": "codegg_core_min",
    "active_audience": "Model",
    "schema_detail": "full",
    "limits": {
      "max_requests_per_second": 10,
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
  ],
  "generated_data": {
    "confusables_generated_rs": true,
    "tool_cards_md": true
  },
  "parity_reference": {
    "path": "../eggcalc",
    "exists": false
  }
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

Generates documentation from the `ToolSpec` registry. The `ToolSpec` entries in `src/mcp/specs/` are the single source of truth; this binary reads them and produces three output files.

```bash
cargo run --bin generate-docs            # regenerate all docs (in-place)
cargo run --bin generate-docs -- --check  # verify docs are current (CI)
cargo run --bin generate-docs -- --output-dir /path  # write to a different directory
```

### What It Generates

| Output File | Content | Marker Pair |
|-------------|---------|-------------|
| `README.md` | Tool table with all non-hidden tools, grouped by category (20 categories) | `<!-- BEGIN GENERATED: eggsact tools -->` / `<!-- END GENERATED: eggsact tools -->` |
| `architecture/mcp-server.md` | Profile reference table — model/harness tool counts, tool names, harness-only tools | `<!-- BEGIN GENERATED: profile reference -->` / `<!-- END GENERATED: profile reference -->` |
| `generated/tool-cards.md` | Per-profile tool cards with description, tier, cost, stability, exposure, required args, and aliases | (whole file is generated) |

### How It Works

1. **Reads `ToolSpec` registry**: Calls `all_tools_vec()` and `tools_for_profile_audience()` from `src/mcp/registry/` to get the canonical tool list.

2. **Generates three content blocks**:
   - `generate_readme_tools()` — filters out `ToolExposure::Hidden` tools, groups by category (using a fixed `CATEGORY_ORDER` of 20 categories), emits markdown tables with tool name, tier, exposure, stability, cost, and profile membership.
   - `generate_profile_reference()` — iterates all available profiles, counts model vs harness tools, lists harness-only tools per profile.
   - `generate_tool_cards()` — iterates 8 codegg profiles, generates per-tool cards with required args (extracted from JSON schemas), aliases, and composite flags.

3. **Marker-based insertion**: For `README.md` and `architecture/mcp-server.md`, the generator uses HTML comment markers to delimit generated sections. It strips all existing generated blocks (including orphaned/duplicated ones from prior failed runs) and inserts a fresh single block. If markers don't exist yet, they are appended.

4. **Writes `generated/tool-cards.md`**: This file is entirely generated (no markers needed).

### `--check` Mode

Used in CI to verify generated docs are current without modifying files:

```bash
cargo run --bin generate-docs -- --check
# Exit code 0 = docs are current
# Exit code 1 = docs are stale (prints which files need updating)
```

In check mode, the generator compares generated content against existing files and reports mismatches. It does **not** write any files. The error message includes the exact command to regenerate:

```
Stale generated docs:
  README.md
  architecture/mcp-server.md
Run `cargo run --bin generate-docs` to regenerate.
```

### Orphan and Triplication Handling

The generator is resilient to malformed marker blocks. `find_all_generated_spans()` detects orphaned BEGIN markers (e.g., from a crashed prior run) and `strip_all_generated_blocks()` removes all generated content — well-formed and orphaned — before inserting a fresh block. Unit tests verify this behavior:

- `find_all_spans_handles_well_formed_block` — single BEGIN/END pair
- `find_all_spans_detects_orphans` — two BEGIN, one END → first well-formed, second orphan
- `find_all_spans_handles_triplication` — three BEGIN, one END → 1 well-formed, 2 orphans
- `strip_all_removes_triplicated_blocks` — cleans all blocks, preserves surrounding headings

### Internal Tests

The binary includes 11 unit tests (`tests` module) plus 4 marker-integrity tests (`generated_marker_integrity` module):

| Test | Purpose |
|------|---------|
| `tool_table_contains_all_non_hidden_tools` | Every non-hidden tool appears in README table |
| `generated_readme_excludes_hidden_tools` | No hidden tool appears in README table |
| `generated_tool_cards_exclude_hidden_tools` | No hidden tool appears in tool cards |
| `profile_counts_match_registry` | Profile reference table counts match live registry |
| `profile_reference_includes_harness_only_tools` | Harness-only tools listed per profile |
| `tool_cards_reference_only_known_tools` | No unknown tool names in tool cards |
| `tool_card_required_args_match_schema` | Required args in cards match JSON schemas |
| `stale_docs_message_uses_cargo_bin_name` | Error message uses `generate-docs` (dash, not underscore) |
| `regenerate_command_uses_dash_form` | `REGENERATE_COMMAND` constant uses dash form |
| `readme_markers_are_well_formed` | README.md has exactly one well-ordered marker pair |
| `mcp_server_doc_markers_are_well_formed` | mcp-server.md has exactly one well-ordered marker pair |

### When to Regenerate

Run `cargo run --bin generate-docs` whenever you:

- Add, remove, or rename an MCP tool in `src/mcp/specs/`
- Change a `ToolSpec`'s `profiles`, `exposure`, `cost`, `stability`, `description`, or `input_schema`
- Add a new profile in `src/mcp/registry/all_tools.rs`
- Change tool schema definitions (required args, types)

After regenerating, commit the updated files. CI will fail if generated docs are stale.

---

## `verify-eggsact` Binary (`src/bin/verify_eggsact.rs`)

Runs a 9-step verification pipeline and emits a markdown report. Exits with code 0 (all pass) or 1 (any failure).

```bash
cargo run --bin verify-eggsact                     # full pipeline
cargo run --bin verify-eggsact -- --report markdown  # explicit format (only markdown supported)
```

### Pipeline Steps

| # | Step | Command | Description |
|---|------|---------|-------------|
| 1 | `cargo fmt` | `cargo fmt --all -- --check` | Format check (no modifications) |
| 2 | `cargo clippy` | `cargo clippy --all-targets --all-features -- -D warnings` | Lint with warnings as errors |
| 3 | `cargo test --lib` | `cargo test --all-features --lib` | Unit tests in `src/` only |
| 4 | `cargo test --bins` | `cargo test --all-features --bins` | Binary tests (generate-docs tests, etc.) |
| 5 | `cargo test --tests` | `cargo test --all-features --tests -- --skip parity` | Integration tests (parity excluded) |
| 6 | `cargo test --doc` | `cargo test --doc` | Doc tests |
| 7 | `generate-docs --check` | `cargo run --bin generate-docs -- --check` | Verify generated docs are fresh |
| 8 | `cargo package` | `cargo package --verbose` | Crates.io packaging dry run |
| 9 | parity tests | `cargo test --test lib parity --all-features` | Python/Rust parity (skipped if `../eggcalc` missing) |

Each step is timed independently. The parity step is automatically skipped when `../eggcalc` does not exist (detected via `fs::metadata()`).

### Output Format

The report is printed as markdown with a header, commit hash, and a results table:

```markdown
# Eggsact Verification Report

**commit:** `abc1234`
**generated-docs freshness:** `PASS`
**parity availability:** available

## Results

| Step | Status | Duration |
|------|--------|----------|
| cargo fmt | `PASS` | 120ms |
| cargo clippy | `PASS` | 8.2s |
| cargo test --lib | `PASS` | 4.1s |
| cargo test --bins | `PASS` | 1.3s |
| cargo test --tests (skip parity) | `PASS` | 12.5s |
| cargo test --doc | `PASS` | 2.0s |
| generate-docs --check | `PASS` | 1.5s |
| cargo package | `PASS` | 3.2s |
| parity tests | `SKIP` | - |

**Overall: PASS**
```

Status badges: `` `PASS` ``, `` `FAIL` ``, `` `SKIP` `` (parity only, when unavailable).

Duration formatting: `<1s` → `{ms}ms`, `<60s` → `{s}s`, `≥60s` → `{m}m {s}s`.

### Parity Check

Parity tests compare eggsact MCP output against the Python `eggcalc` package. They require `../eggcalc` to exist (the Python reference implementation). When the directory is absent, step 9 is marked `SKIP` and the report notes:

```
parity availability: unavailable (`../eggcalc` not found) — parity tests skipped
```

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
cargo run --bin generate-docs

# Generate docs (check mode, for CI)
cargo run --bin generate-docs -- --check

# Verify eggsact (full pipeline)
cargo run --bin verify-eggsact

# Run main.rs unit tests
cargo test --lib main
```

### Full Verification Pipeline (Manual)

The recommended verification order before release:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Or equivalently:

```bash
cargo run --bin verify-eggsact
```

---

## Design Notes

- **No clap/structopt**: Arg parsing is hand-rolled via `parse_args()` pattern matching. This keeps the dependency tree minimal and makes the CLI behavior fully deterministic.
- **`EGGCALC_NO_CONFIG` is hardcoded**: The main binary always sets this env var, preventing any config file from affecting CLI behavior. This ensures reproducible output.
- **Diagnostics exposes names, not values**: The `--diagnostics` mode lists environment variable names but never reads or prints their values, avoiding secret leakage.
- **`generate-docs` uses marker-based insertion**: HTML comments delimit generated sections in markdown files. This allows hand-editing around generated content while keeping the generated parts reproducible.
- **`verify-eggsact` is a self-contained pipeline**: It spawns each step as a child process with `EGGCALC_NO_CONFIG=1`, captures exit status, and builds a markdown report. No external test framework required.
