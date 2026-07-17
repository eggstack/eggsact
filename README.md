# eggsact

[![Crates.io](https://img.shields.io/crates/v/eggsact)](https://crates.io/crates/eggsact)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Deterministic MCP and in-process utility tools for coding agents. 80 tools across 20 categories: math, text, JSON, regex, path, shell, config, patch, dependency, analysis, and more. Includes a natural language math evaluator that parses expressions like "thirty plus five" or "30m + 100ft".

## Key Features

- MCP server with 80 registered tools for AI agents to reduce hallucinations
- In-process agent API for direct tool calls without MCP overhead
- Typed preflight wrappers for edit, command, config, and patch workflows
- Natural language math: "two to the power of ten" evaluates to 1024
- Unit conversions: "30m to ft", "100C in F"
- Physical and mathematical constants: `pi`, `c`, `planck`, `avogadro`, `gravity`
- High-performance Rust implementation with zero required external services

## Installation

### From crates.io

```bash
cargo install eggsact
```

### From source

```bash
git clone https://github.com/eggstack/eggsact
cd eggsact
cargo install --path .
```

## Quick Start

### CLI

```bash
# Natural language
eggsact "thirty plus five"
# 35

# Standard math
eggsact "3 + 4 * 2"
# 11

# Unit conversions
eggsact "30m to ft"
# 98.4251968503937

# Power
eggsact "2 ** 10"
# 1024

# Help and version
eggsact --help
eggsact --version

# Diagnostics (text output)
eggsact --diagnostics

# Diagnostics (JSON output)
eggsact --diagnostics --format json

# MCP server mode (stdio JSON-RPC)
eggsact --mcp
```

### Library

```rust
use eggsact::{run, evaluate};

// Natural language — returns (result, type)
let (result, _typ) = run("thirty plus five").unwrap();
assert_eq!(result, "35");

// Direct math evaluation
let (result, _typ) = evaluate("2 ** 10").unwrap();
assert_eq!(result, "1024");

// Unit conversion
let (result, _typ) = run("30m to ft").unwrap();
// result ≈ "98.42519685039369 ft"
```

### MCP Server

Start the server and connect via JSON-RPC 2.0 over stdio. The server supports MCP protocol versions `2025-11-25` (preferred) and `2024-11-05` (legacy).

```bash
eggsact --mcp
```

**Lifecycle**: Clients must complete the initialization handshake before calling tools:

```json
→ {"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"my-client"}}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{...},"serverInfo":{"name":"eggsact","version":"..."}}}
→ {"jsonrpc":"2.0","method":"notifications/initialized"}
→ {"jsonrpc":"2.0","method":"tools/list","id":2}
← {"jsonrpc":"2.0","id":2,"result":{"tools":[...]}}
```

The server dispatches requests **concurrently**. Responses may arrive out of request order. **Clients must correlate responses to requests by JSON-RPC `id`**, not by arrival position. See `architecture/mcp-server.md` for the full concurrency and response-ordering contract.

## Response Contract

Every MCP tool response includes a `machine_code` field (when non-OK) for programmatic routing and classification. Machine codes are defined in `src/mcp/machine_codes.rs`. See `architecture/machine-codes.md` for the full code table and finding helpers.

## MCP Tools

<!-- BEGIN GENERATED: eggsact tools -->
80 tools across 20 categories. See `architecture/mcp-server.md` for the full reference.

### Math (4)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `math_eval` | 0 | default | stable | mod | default, full, human_math |
| `unit_convert` | 2 | contextual | stable | cheap | full, human_math |
| `unit_info` | 2 | contextual | stable | cheap | full, human_math |
| `constant_lookup` | 2 | contextual | stable | cheap | full, human_math |

### Text (18)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `text_measure` | 0 | default | stable | cheap | default, full |
| `text_equal` | 0 | default | stable | cheap | codegg_core, default, full |
| `text_diff_explain` | 1 | default | stable | mod | codegg_core, codegg_patch, default, full |
| `text_inspect` | 1 | default | stable | mod | codegg_core, codegg_unicode_security, default, full |
| `text_count` | 0 | default | stable | cheap | default, full |
| `text_truncate` | 3 | expert | stable | cheap | full |
| `text_transform` | 2 | contextual | stable | mod | codegg_unicode_security, full |
| `text_position` | 2 | contextual | stable | cheap | codegg_unicode_security, full |
| `text_hash` | 2 | contextual | stable | mod | full |
| `escape_text` | 1 | default | stable | cheap | default, full |
| `unescape_text` | 1 | default | stable | cheap | default, full |
| `text_window` | 1 | default | stable | cheap | default, full |
| `text_fingerprint` | 0 | default | stable | cheap | codegg_core, codegg_repo_audit, default, full |
| `text_replace_check` | 1 | default | stable | cheap | codegg_core, codegg_core_min, codegg_patch, default, full |
| `line_range_extract` | 1 | default | stable | cheap | codegg_patch, default, full |
| `line_range_compare` | 2 | contextual | stable | mod | codegg_patch, full |
| `prompt_input_inspect` | 2 | harness | stable | mod | codegg_preflight, codegg_unicode_security, full |
| `text_security_inspect` | 1 | default | stable | heavy | codegg_core, codegg_core_min, codegg_preflight, codegg_unicode_security, full |

### Json (6)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `json_extract` | 2 | contextual | stable | mod | codegg_config, full |
| `json_compare` | 1 | default | stable | mod | codegg_config, default, full |
| `json_shape` | 3 | expert | stable | mod | codegg_repo_audit, full |
| `json_canonicalize` | 1 | default | stable | mod | codegg_config, default, full |
| `json_query` | 1 | contextual | deprecated | mod | full |
| `structured_data_compare` | 2 | contextual | stable | heavy | codegg_config, codegg_core, full |

### Regex (3)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `validate_regex` | 1 | default | stable | mod | default, full |
| `regex_finditer` | 1 | default | stable | mod | default, full |
| `regex_safety_check` | 1 | default | stable | cheap | codegg_shell, default, full |

### Validation (4)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `validate_brackets` | 1 | default | stable | cheap | default, full |
| `validate_json` | 0 | default | stable | cheap | codegg_config, codegg_core, codegg_core_min, default, full |
| `validate_toml` | 1 | default | stable | cheap | codegg_config, codegg_core, default, full |
| `validate_schema_light` | 3 | contextual | stable | mod | codegg_config, full |

### List (3)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `list_compare` | 2 | contextual | stable | mod | full |
| `list_dedupe` | 1 | default | stable | cheap | default, full |
| `list_sort` | 1 | default | stable | cheap | default, full |

### Path (6)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `path_normalize` | 0 | default | stable | cheap | codegg_core, default, full |
| `path_analyze` | 2 | contextual | stable | cheap | full |
| `path_compare` | 2 | contextual | stable | cheap | full |
| `path_scope_check` | 2 | harness | stable | cheap | codegg_preflight, full |
| `glob_match` | 1 | default | stable | cheap | default, full |
| `path_batch_scope_check` | 2 | harness | stable | cheap | codegg_patch, codegg_preflight, full |

### Shell (4)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `shell_split` | 2 | harness | stable | cheap | codegg_preflight, codegg_shell, full |
| `shell_quote_join` | 2 | contextual | stable | cheap | codegg_shell, full |
| `argv_compare` | 2 | contextual | stable | cheap | codegg_shell, full |
| `command_preflight` | 1 | default | stable | heavy | codegg_core, codegg_core_min, codegg_preflight, codegg_shell, full |

### Markdown (2)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `markdown_structure` | 2 | contextual | stable | mod | codegg_repo_audit, full |
| `code_fence_extract` | 2 | contextual | stable | mod | codegg_repo_audit, full |

### Config (3)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `dotenv_validate` | 2 | contextual | stable | cheap | codegg_config, full |
| `ini_validate` | 2 | contextual | stable | cheap | codegg_config, full |
| `config_preflight` | 1 | default | stable | heavy | codegg_config, codegg_core, codegg_core_min, codegg_preflight, full |

### Identifier (3)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `identifier_analyze` | 3 | expert | stable | mod | full |
| `identifier_inspect` | 1 | default | stable | mod | codegg_core, codegg_unicode_security, default, full |
| `identifier_table_inspect` | 3 | expert | stable | mod | codegg_repo_audit, full |

### Unicode (2)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `unicode_policy_check` | 2 | harness | stable | mod | codegg_preflight, codegg_unicode_security, full |
| `canonicalize_text` | 2 | contextual | stable | mod | codegg_unicode_security, full |

### Version (2)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `version_compare` | 2 | contextual | stable | cheap | codegg_config, full |
| `version_constraint_check` | 3 | expert | stable | cheap | full |

### Toml (1)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `toml_shape` | 2 | contextual | stable | mod | codegg_config, full |

### Patch (5)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `patch_apply_check` | 2 | harness | stable | mod | codegg_patch, codegg_preflight, full |
| `patch_summary` | 2 | contextual | stable | mod | codegg_patch, full |
| `edit_preflight` | 1 | default | stable | heavy | codegg_core, codegg_core_min, codegg_patch, codegg_preflight, full |
| `patch_contract_check` | 2 | contextual | stable | mod | codegg_patch, codegg_preflight, codegg_repo_audit, full |
| `diff_risk_classify` | 2 | contextual | stable | mod | codegg_patch, codegg_repo_audit, full |

### Cargo (1)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `cargo_toml_inspect` | 3 | expert | stable | mod | codegg_core, codegg_repo_audit, full |

### Dependency (1)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `dependency_edit_preflight` | 2 | contextual | stable | mod | codegg_config, codegg_preflight, codegg_repo_audit, full |

### Repo (5)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `repo_manifest_inspect` | 2 | contextual | stable | cheap | codegg_repo_audit, full |
| `config_file_inspect` | 2 | contextual | stable | mod | codegg_config, codegg_repo_audit, full |
| `test_command_suggest` | 2 | contextual | stable | cheap | codegg_core, codegg_repo_audit, codegg_shell, full |
| `repo_language_detect` | 2 | contextual | stable | cheap | codegg_core, codegg_repo_audit, full |
| `repo_tree_summarize` | 2 | contextual | stable | mod | codegg_repo_audit, full |

### Analysis (4)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `import_export_inspect` | 2 | contextual | stable | cheap | codegg_core, codegg_repo_audit, full |
| `code_block_map` | 2 | contextual | stable | cheap | codegg_core, codegg_repo_audit, full |
| `symbol_name_diff` | 2 | contextual | stable | cheap | codegg_patch, codegg_repo_audit, full |
| `lockfile_inspect` | 2 | contextual | stable | mod | codegg_patch, codegg_preflight, codegg_repo_audit, full |

### Diagnostics (3)

| Tool | Tier | Exposure | Stability | Cost | Profiles |
|------|------|----------|-----------|------|----------|
| `runtime_diagnostics` | 3 | harness | stable | cheap | full |
| `profile_inspect` | 3 | harness | stable | cheap | full |
| `tool_availability_explain` | 3 | harness | stable | cheap | full |


<!-- END GENERATED: eggsact tools -->


## Math Features

### Operations

- Basic arithmetic: `+`, `-`, `*`, `/`, `%`
- Power: `**`, `^` (e.g., `2 ** 10` = 1024)
- Parentheses for grouping

### Functions

- **Trigonometric**: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`
- **Logarithmic**: `log`, `ln`, `log10`, `log2`, `exp`
- **Other**: `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `round`, `trunc`, `sign`, `factorial`

### Constants

- **Mathematical**: `pi`, `e`, `tau`, `phi`
- **Physical**: `c`, `h`, `hbar`, `k`, `G`, `na`, `R`, `qe`, `me`, `mp`, `mn`, `epsilon0`, `mu0`, `gravity`, `atm`

`g` is parsed as the gram unit in unit expressions. Use `gravity` or `standardgravity` for standard gravity.

### Statistical

- `sum`, `mean`/`average`, `median`, `std`/`stddev`, `variance`, `min`, `max`, `product`

### Number Theory

- `gcd`, `lcm`, `factorial`

### Units

| Category | Units |
|----------|-------|
| Length | `m`, `km`, `cm`, `mm`, `in`, `ft`, `yd`, `mi`, `ly`, `au`, `pc` |
| Mass | `kg`, `g`, `mg`, `ug`, `ng`, `lb`, `oz`, `ton`, `stone` |
| Time | `s`, `ms`, `us`, `ns`, `min`, `h`, `d`, `wk`, `yr` |
| Volume | `L`, `mL`, `gal`, `qt`, `pt`, `cup`, `floz`, `tbsp`, `tsp` |
| Temperature | `C`, `F`, `K` |
| Data | `B`, `KB`, `MB`, `GB`, `TB` |
| Pressure | `Pa`, `kPa`, `MPa`, `GPa`, `bar`, `atm`, `psi` |
| Energy | `J`, `kJ`, `cal`, `kcal`, `Wh`, `kWh`, `BTU`, `eV` |
| Power | `W`, `kW`, `MW`, `GW`, `hp` |
| Force | `N`, `kN`, `dyne`, `lbf` |
| Voltage | `V`, `kV`, `mV` |
| Current | `A`, `mA` |
| Angle | `rad`, `deg` |
| Speed | `m/s`, `km/h`, `mph`, `kn`, `mach` |
| Frequency | `Hz`, `kHz`, `MHz`, `GHz`, `THz` |

Temperature conversions use offset math, not multiplicative factors. Prefixed units like `kN`, `mV`, `mA` are supported.

## Library API

### `run`

Evaluate a natural language or unit-expression string. Handles NL parsing, normalization, and unit conversion.

```rust
pub fn run(expr: &str) -> Result<(String, String), eggsact::calc::RunError>
```

```rust
use eggsact::run;

let (result, typ) = run("thirty plus five").unwrap();
assert_eq!(result, "35");
assert_eq!(typ, "int");

let (result, typ) = run("30m + 100ft").unwrap();
// result ≈ "60.480000000000004 m", typ = "float"

let (result, typ) = run("sqrt(144)").unwrap();
assert_eq!(result, "12");
```

### `evaluate`

Evaluate a direct math expression. Expects valid Python/Rust syntax (no natural language).

```rust
pub fn evaluate(expr: &str) -> Result<(String, String), String>
```

```rust
use eggsact::evaluate;

let (result, typ) = evaluate("5 + 3").unwrap();
assert_eq!(result, "8");

let (result, typ) = evaluate("2 ** 10").unwrap();
assert_eq!(result, "1024");
```

### `split_at_operators`

Split a math expression string at operator boundaries.

```rust
pub fn split_at_operators(expr: &str) -> Vec<String>
```

```rust
use eggsact::split_at_operators;

let tokens = split_at_operators("5+3*2");
assert_eq!(tokens, vec!["5", "+", "3", "*", "2"]);
```

### When to use which

| Input type | Use |
|------------|-----|
| `"five plus three"` | `run()` |
| `"30m + 100ft"` | `run()` |
| `"5 + 3"` | `evaluate()` or `run()` |
| `"sqrt(144)"` | `evaluate()` or `run()` |
| `"1km in m"` | `run()` only |

## In-Process Agent API

Call eggsact tools directly from Rust without starting an MCP server. The `ToolRegistry` provides a typed, synchronous API with profile filtering, argument validation, and tool execution.

### Calling tools by name

```rust
use eggsact::agent::{ToolRegistry, ExecutionContext, Profile, ToolAudience};

let registry = ToolRegistry::default();

// Simple call
let response = registry.call_json("text_equal", serde_json::json!({
    "a": "hello",
    "b": "hello",
})).unwrap();
assert!(response.ok);

// Context-aware call (recommended for new code — immutable, clones eval_ctx)
let ctx = ExecutionContext::agent_default(Profile::Full, ToolAudience::Model);
let response = registry.call_json_with_execution_context(
    "math_eval",
    serde_json::json!({"expression": "2 + 3"}),
    &ctx,
).unwrap();
assert!(response.ok);
```

### Profile selection

Profiles control which subset of tools is available. `Profile::from_str_opt` is
strict — it returns `None` for unknown names. Use `Profile::custom(name)` for
custom profiles.

```rust
use eggsact::agent::{ToolRegistry, Profile, ToolAudience};

// Model-facing codegg session
let registry = ToolRegistry::with_profile_and_audience(
    Profile::CodeggCoreMin, ToolAudience::Model,
);
let tools = registry.available_tools_model_safe();
assert!(tools.iter().any(|t| t.name == "text_equal"));

// Harness preflight checks
let harness_registry = ToolRegistry::with_profile_and_audience(
    Profile::CodeggPreflight,
    ToolAudience::Harness,
);
```

Audience is enforced at dispatch time: `call_json` rejects harness-only tools
when the registry uses `Model` audience, and rejects hidden tools for all audiences.

### Typed preflight wrappers

For common workflows, use the typed wrappers in `eggsact::preflight`:

```rust
use eggsact::preflight::{ConfigPreflight, ConfigPreflightInput, ConfigFormat};

let input = ConfigPreflightInput {
    text: r#"{"key": "value"}"#.to_string(),
    format: ConfigFormat::Json,
    schema: None,
    strict: false,
};
let output = ConfigPreflight::run(&input).unwrap();
assert!(output.valid);
assert!(!output.machine_code.is_empty());
```

Available preflight wrappers: `EditPreflight`, `CommandPreflight`, `ConfigPreflight`,
`PatchApplyCheck`, `TextSecurityInspect`.

All wrappers return `Result<Output, PreflightError>` where `PreflightError`
distinguishes three failure modes:

- **`ToolCall`** — registry rejected the call before execution
- **`ToolRejected`** — tool executed but returned `ok: false`
- **`ContractViolation`** — tool returned `ok: true` but response shape
  violated the typed contract (missing mandatory field)

Missing mandatory fields are **hard failures** — wrappers will not silently
default `machine_code`, `verdict`, or other route-critical fields.

## Architecture

```
eggsact/
├── src/
│   ├── main.rs              # CLI entry point, argument parsing
│   ├── lib.rs               # Public API exports
│   ├── calc/                # Calculator core
│   │   ├── mod.rs           # Module re-exports
│   │   ├── normalize.rs     # Natural language tokenization, number words
│   │   ├── evaluator.rs     # AST-based expression evaluation
│   │   └── units.rs         # Unit definitions and conversion factors
│   ├── mcp/                 # MCP server
│   │   ├── mod.rs           # Module re-exports
│   │   ├── server.rs        # stdio JSON-RPC 2.0 server, protocol orchestration
│   │   ├── registry/          # Tool registration (ToolSpec declarations, single source of truth)
│   │   │   ├── mod.rs         # Re-exports, tests
│   │   │   ├── types.rs       # ToolDefinition, ToolSpec, enums
│   │   │   ├── all_tools.rs   # ALL_TOOLS constant, PROFILE_NAMES
│   │   │   └── listing.rs     # Filtering, audience, schema compaction, suggestions
│   │   ├── protocol.rs      # JSON-RPC types (Request, Response, Error, InitializeResult)
│   │   ├── response.rs      # ToolResponse, error sanitization, finding() helpers, with_verdict, preflight builders
│   │   ├── runtime.rs       # Rate limiter, active request tracking, constants, profile management
│   │   ├── schema_validation.rs # MCP argument validation against tool schemas
│   │   └── schemas/         # JSON-schema builders per tool category
│   │       ├── mod.rs       # Module declarations + re-exports
│   │       └── ...          # One submodule per tool category
│   ├── tools/               # MCP tool implementations (by category)
│   │   ├── mod.rs           # Module re-exports
│   │   ├── helpers.rs       # Shared constants, utilities, helper functions
│   │   ├── math.rs          # math_eval, unit_convert, unit_info, constant_lookup
│   │   ├── text.rs          # text_measure, text_equal, text_diff_explain, etc. (18 tools)
│   │   ├── json.rs          # json_extract, json_compare, json_canonicalize, etc. (6 tools)
│   │   ├── regex.rs         # validate_regex, regex_safety_check, regex_finditer
│   │   ├── validation.rs    # validate_json, validate_brackets, validate_toml, validate_schema_light
│   │   ├── path.rs          # path_normalize, path_analyze, path_compare, glob_match, path_batch_scope_check
│   │   ├── shell.rs         # shell_split, shell_quote_join, argv_compare, command_preflight
│   │   ├── list.rs          # list_compare, list_dedupe, list_sort
│   │   ├── markdown.rs      # markdown_structure, code_fence_extract
│   │   ├── patch.rs         # patch_apply_check, patch_summary, edit_preflight, diff_risk_classify, patch_contract_check
│   │   ├── config.rs        # dotenv_validate, ini_validate, config_preflight, toml_shape_tool
│   │   ├── identifier.rs    # identifier_analyze, identifier_inspect, identifier_table_inspect
│   │   ├── unicode.rs       # unicode_policy_check, canonicalize_text
│   │   ├── version.rs       # version_compare, version_constraint_check
│   │   ├── cargo.rs         # cargo_toml_inspect
│   │   ├── dependency.rs    # dependency_edit_preflight
│   │   ├── diagnostics.rs   # runtime_diagnostics, profile_inspect, tool_availability_explain
│   │   └── repo.rs          # repo_manifest_inspect, config_file_inspect, repo_tree_summarize, test_command_suggest, repo_language_detect
│   └── text/                # Text processing library (25 modules)
│       ├── mod.rs           # Module re-exports
│       ├── primitives.rs    # UTF-8 encoding, codepoint iteration
│       ├── confusables.rs   # Unicode confusable character lookup
│       ├── diff.rs          # String diffing, Levenshtein distance
│       ├── measure.rs       # Text metrics (words, lines, bytes)
│       ├── validate.rs      # Bracket, JSON, regex validation
│       ├── transform.rs     # Text transforms, hashing, fingerprinting
│       ├── position.rs      # Byte/line/column position conversion
│       ├── regex_safety.rs  # ReDoS detection
│       ├── replace.rs       # Text replacement with preview
│       ├── path.rs          # Path analysis and normalization
│       ├── identifier.rs    # Identifier naming classification
│       ├── shell.rs         # Shell command parsing and quoting
│       ├── markdown.rs      # Markdown structure analysis
│       ├── glob.rs          # Glob pattern matching
│       ├── config.rs        # .env and INI validation
│       ├── toml.rs          # TOML validation and shape analysis
│       ├── patch.rs         # Unified diff parsing and application
│       ├── line_range.rs    # Line range extraction and comparison
│       ├── unicode_policy.rs # Unicode safety policies
│       ├── unicode_tools.rs # Mixed-script, invisible char detection
│       ├── inspect_prompt.rs # Prompt injection detection
│       ├── synthesis.rs     # Composite tool orchestration
│       ├── cargo.rs         # Cargo.toml inspection
│       ├── version.rs       # Semver constraint checking
│       └── confusables_generated.rs # Generated confusables data (data file)
├── tests/                   # Integration and unit tests
├── docs/                    # Detailed documentation
├── architecture/            # Architecture documentation
├── .opencode/skills/        # Agent task skills (opencode format)
├── .agents/skills/          # Agent task skills (symlinks to .opencode/skills/)
├── Cargo.toml
└── README.md
```

## Development

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Run

```bash
# Single expression
cargo run -- "thirty plus five"

# MCP server mode
cargo run -- --mcp
```

## Relationship to Python eggcalc

`eggsact` is a Rust reimplementation of the Python `eggcalc` project. The Python version uses AST parsing of expressions and a plugin-based MCP server. `eggsact` reimplements the same normalization pipeline, evaluation engine, and all MCP tools in Rust for higher performance and easier distribution as a single binary.

The two projects are functionally equivalent for core math, unit conversion, and text processing operations.

## License

MIT -- see [LICENSE](LICENSE) for details.
