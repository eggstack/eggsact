# eggsact

[![Crates.io](https://img.shields.io/crates/v/eggsact)](https://crates.io/crates/eggsact)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A natural language math calculator with an MCP (Model Context Protocol) server for AI coding agents. Parses expressions like "thirty plus five" or "30m + 100ft" and evaluates them to results. Ships with 64 MCP tools covering math, text processing, JSON, regex, paths, Unicode safety, and more.

## Key Features

- Natural language math: "two to the power of ten" evaluates to 1024
- Unit conversions: "30m to ft", "100C in F"
- Physical and mathematical constants: `pi`, `c`, `planck`, `avogadro`, `gravity`
- MCP server with 64 tools for AI agents to reduce hallucinations
- High-performance Rust implementation with zero required external services
- Rust reimplementation of the Python `eggcalc` project

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

Start the server and connect via JSON-RPC 2.0 over stdio. The server identifies as `eggsact` with MCP protocol version `2024-11-05`.

```bash
eggsact --mcp
```

## Response Contract

Every MCP tool response includes a `machine_code` field (when non-OK) for programmatic routing and classification. Machine codes are defined in `src/mcp/machine_codes.rs`. See `architecture/machine-codes.md` for the full code table and finding helpers.

## MCP Tools

64 tools across 16 categories. See `architecture/mcp-server.md` for the full reference.

### Math & Units (4)

| Tool | Description |
|------|-------------|
| `math_eval` | Evaluate arithmetic, unit conversions, constants, and scientific expressions |
| `unit_convert` | Convert a quantity between compatible units |
| `unit_info` | Get metadata about a unit (category, base unit, aliases) |
| `constant_lookup` | Look up physical or mathematical constants by name |

### Text (18)

| Tool | Description |
|------|-------------|
| `text_measure` | Measure text properties: bytes, characters, codepoints, words, lines |
| `text_equal` | Compare two strings with casefold and trim options |
| `text_diff_explain` | Levenshtein distance and character-level diff between strings |
| `text_inspect` | Inspect text for hidden characters, codepoints, and confusables |
| `text_count` | Count character occurrences or build a frequency table |
| `text_truncate` | Truncate text to a given length with ellipsis options |
| `text_fingerprint` | Generate a stable text fingerprint for deduplication |
| `text_hash` | Hash text with SHA-256, SHA-1, MD5, or CRC32 |
| `text_position` | Convert between byte offsets, line/column, and UTF-16 positions |
| `text_window` | Extract a window of text around a position |
| `text_transform` | Casefold, normalize (NFC/NFD/NFKC/NFKD), and transform text |
| `text_replace_check` | Preview a text replacement before applying it |
| `escape_text` | Escape special characters (JSON, shell, regex, URL) |
| `unescape_text` | Unescape escaped strings back to their original form |
| `text_security_inspect` | Composite: aggregate security checks across multiple tools |
| `prompt_input_inspect` | Detect hidden characters and instruction injection in prompts |
| `line_range_extract` | Extract a line range from text |
| `line_range_compare` | Compare line ranges between two texts |

### JSON (6)

| Tool | Description |
|------|-------------|
| `json_extract` | Extract values from JSON by path |
| `json_compare` | Compare two JSON structures for equality or diff |
| `json_canonicalize` | Produce canonical JSON for deterministic serialization |
| `json_query` | Query JSON with a simple dot-path language |
| `json_shape` | Describe the structure of a JSON document |
| `structured_data_compare` | Composite: compare structured data using JSON tools |

### Regex (3)

| Tool | Description |
|------|-------------|
| `validate_regex` | Test regex patterns with lookahead/lookbehind, groups, and flags |
| `regex_safety_check` | Detect ReDoS vulnerabilities and catastrophic backtracking |
| `regex_finditer` | Find all regex matches in a string with capture groups |

### Lists (3)

| Tool | Description |
|------|-------------|
| `list_compare` | Compare two lists, find common and unique items |
| `list_dedupe` | Remove duplicate items from a list |
| `list_sort` | Sort a list with configurable order and key extraction |

### Validation (4)

| Tool | Description |
|------|-------------|
| `validate_json` | Validate JSON syntax with error position reporting |
| `validate_toml` | Validate TOML syntax |
| `validate_brackets` | Check bracket balance in text (parens, braces, brackets, angle) |
| `validate_schema_light` | Lightweight JSON schema validation |

### Paths (5)

| Tool | Description |
|------|-------------|
| `path_normalize` | Normalize and canonicalize file paths |
| `path_analyze` | Analyze path components (parent, stem, extension, etc.) |
| `path_compare` | Compare two paths for equivalence |
| `path_scope_check` | Check whether a path is within a given directory scope |
| `glob_match` | Match a file path against a glob pattern |

### Identifiers (3)

| Tool | Description |
|------|-------------|
| `identifier_analyze` | Classify identifier naming conventions (snake_case, camelCase, etc.) |
| `identifier_inspect` | Inspect identifiers for confusables and collisions |
| `identifier_table_inspect` | Analyze a table of identifiers for naming issues |

### Shell (4)

| Tool | Description |
|------|-------------|
| `shell_split` | Split a shell command string into argv tokens |
| `shell_quote_join` | Quote and join tokens into a safe shell command string |
| `argv_compare` | Compare two argument lists for equivalence |
| `command_preflight` | Composite: pre-check a shell command using shell/identifier tools |

### Markdown (2)

| Tool | Description |
|------|-------------|
| `markdown_structure` | Parse markdown headings, lists, and code blocks |
| `code_fence_extract` | Extract fenced code blocks with language tags |

### Config (3)

| Tool | Description |
|------|-------------|
| `dotenv_validate` | Validate `.env` file syntax |
| `ini_validate` | Validate INI file syntax |
| `config_preflight` | Composite: pre-check a config file using validation tools |

### Patches (3)

| Tool | Description |
|------|-------------|
| `patch_apply_check` | Preview a unified diff patch without modifying files |
| `patch_summary` | Summarize changes in a unified diff patch |
| `edit_preflight` | Composite: pre-check an edit operation using text tools |

### TOML (1)

| Tool | Description |
|------|-------------|
| `toml_shape` | Describe the structure of a TOML document |

### Unicode (2)

| Tool | Description |
|------|-------------|
| `unicode_policy_check` | Validate text against named Unicode safety policies |
| `canonicalize_text` | Normalize text using configurable canonicalization profiles |

### Versioning (2)

| Tool | Description |
|------|-------------|
| `version_constraint_check` | Check if a semver version satisfies a constraint |
| `version_compare` | Compare two semver versions |

### Cargo (1)

| Tool | Description |
|------|-------------|
| `cargo_toml_inspect` | Extract metadata from Cargo.toml files |

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
use eggsact::agent::ToolRegistry;

let registry = ToolRegistry::default();
let response = registry.call_json("text_equal", serde_json::json!({
    "a": "hello",
    "b": "hello",
})).unwrap();
assert!(response.ok);
```

### Profile selection

Profiles control which subset of tools is available:

```rust
use eggsact::agent::{ToolRegistry, Profile};

let registry = ToolRegistry::with_profile(Profile::CodeggCoreMin);
let tools = registry.available_tools();
assert!(tools.iter().any(|t| t.name == "math_eval"));
```

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
```

Available preflight wrappers: `ConfigPreflight`, `CommandPreflight`, `EditPreflight`.

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
│   │   ├── registry.rs      # Tool registration (ToolSpec declarations, single source of truth)
│   │   ├── protocol.rs      # JSON-RPC types (Request, Response, Error, InitializeResult)
│   │   ├── response.rs      # ToolResponse, error sanitization
│   │   ├── runtime.rs       # Rate limiter, cancelled requests, constants, profile management
│   │   ├── schema_validation.rs # MCP argument validation against tool schemas
│   │   └── schemas.rs       # Re-exports from protocol.rs and response.rs (backward compat)
│   ├── tools/               # MCP tool implementations (by category)
│   │   ├── mod.rs           # Module re-exports
│   │   ├── helpers.rs       # Shared constants, utilities, helper functions
│   │   ├── math.rs          # math_eval, unit_convert, unit_info, constant_lookup
│   │   ├── text.rs          # text_measure, text_equal, text_diff_explain, etc. (18 tools)
│   │   ├── json.rs          # json_extract, json_compare, json_canonicalize, etc. (6 tools)
│   │   ├── regex.rs         # validate_regex, regex_safety_check, regex_finditer
│   │   ├── validation.rs    # validate_json, validate_brackets, validate_toml, validate_schema_light
│   │   ├── path.rs          # path_normalize, path_analyze, path_compare, glob_match, etc.
│   │   ├── shell.rs         # shell_split, shell_quote_join, argv_compare, command_preflight
│   │   ├── list.rs          # list_compare, list_dedupe, list_sort
│   │   ├── markdown.rs      # markdown_structure, code_fence_extract
│   │   ├── patch.rs         # patch_apply_check, patch_summary, edit_preflight
│   │   ├── config.rs        # dotenv_validate, ini_validate, config_preflight
│   │   ├── identifier.rs    # identifier_analyze, identifier_inspect, identifier_table_inspect
│   │   ├── unicode.rs       # unicode_policy_check, canonicalize_text
│   │   ├── version.rs       # version_compare, version_constraint_check
│   │   └── cargo.rs         # cargo_toml_inspect
│   └── text/                # Text processing library (24 modules)
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
├── .skills/                 # Agent task skills
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

`eggsact` is a Rust reimplementation of the Python `eggcalc` project. The Python version uses AST parsing of natural language expressions and a plugin-based MCP server. `eggsact` reimplements the same normalization pipeline, evaluation engine, and all MCP tools in Rust for higher performance and easier distribution as a single binary.

The two projects are functionally equivalent for core math, unit conversion, and text processing operations.

## License

MIT -- see [LICENSE](LICENSE) for details.
