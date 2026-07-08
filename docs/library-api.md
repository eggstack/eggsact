# eggsact Library API Reference

## Overview

`eggsact` is a Rust crate providing deterministic utility tools and an MCP (Model Context Protocol) server for AI coding agents. Use it as a library dependency or as a standalone CLI.

```toml
[dependencies]
eggsact = "1.1.3"
```

The crate exposes three public modules:

- `eggsact::calc` -- math evaluation (natural language and direct expressions)
- `eggsact::text` -- text processing utilities (measurement, diff, validation, transforms)
- `eggsact::mcp` -- MCP server for AI tool integration

Core functions are re-exported at the crate root:

```rust
use eggsact::{run, evaluate, split_at_operators};
```

---

## Core Functions

### `run`

```rust
pub fn run(expr: &str) -> Result<(String, String), eggsact::calc::RunError>
```

Full natural language pipeline. Normalizes English text into a math expression, then evaluates it.

**Arguments:**
- `expr` -- a string slice containing the math expression (natural language or symbolic)

**Returns:**
- `Ok((result, type_name))` -- the result string and its type (`"int"`, `"float"`, `"nan"`, `"inf"`, `"-inf"`)
- `Err(RunError)` -- a human-readable error value with `Display` and `Error` implementations

**What it handles:**
- Number words: `"five plus three"` -> `("8", "int")`
- Operator words: `"thirty times two"` -> `("60", "int")`
- Function names: `"square root of 144"` -> `("12", "float")`
- Percentages: `"50 percent of 200"` -> `("100", "float")`
- Unit conversions: `"30m + 100ft"` -> `("60.480000000000004 m", "float")`
- Fillers: `"what's five plus three"` -> `("8", "int")`

**Examples:**

```rust
use eggsact::run;

let (result, typ) = run("thirty plus five").unwrap();
assert_eq!(result, "35");
assert_eq!(typ, "int");

let (result, _) = run("square root of 144").unwrap();
assert_eq!(result, "12");

let (result, _) = run("what is 50 percent of 200").unwrap();
assert_eq!(result, "100");
```

### `evaluate`

```rust
pub fn evaluate(expr: &str) -> Result<(String, String), String>
```

Direct mathematical expression evaluation. Parses Python-style math syntax without normalization. Use this when your input is already a valid math expression.

**Arguments:**
- `expr` -- a string slice containing a valid math expression

**Returns:**
- `Ok((result, type_name))` -- same as `run`
- `Err(message)` -- error string

**Supported syntax:**
- Arithmetic: `+`, `-`, `*`, `/`, `%`, `**` (power)
- Parentheses for grouping
- Functions: `sin()`, `cos()`, `sqrt()`, `abs()`, `log()`, `log2()`, `log10()`, etc.
- Constants: `pi`, `e`, `tau`, `c`, `gravity`, `na`, `h`, etc.
- Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Complex numbers: `3+4j`

**Examples:**

```rust
use eggsact::evaluate;

assert_eq!(evaluate("5 + 3").unwrap(), ("8", "int"));
assert_eq!(evaluate("2 ** 10").unwrap(), ("1024", "int"));
assert_eq!(evaluate("sqrt(144)").unwrap(), ("12", "float"));
assert_eq!(evaluate("sin(pi / 2)").unwrap(), ("1", "float"));
assert_eq!(evaluate("10 / 3").unwrap(), ("3.3333333333333335", "float"));
```

**Important:** `evaluate` does NOT parse natural language or unit suffixes:

```rust
// These fail:
evaluate("five plus three");    // Err("Parse error: ...")
evaluate("1km");                // Err("Parse error: ...")

// Use run() instead:
run("five plus three");         // Ok(("8", "int"))
```

### `split_at_operators`

```rust
pub fn split_at_operators(expr: &str) -> Vec<String>
```

Tokenizes a math expression into operands and operators, respecting parentheses.

**Arguments:**
- `expr` -- a string slice containing a math expression

**Returns:**
- A `Vec<String>` of tokens (operands and operators as separate strings)

**Examples:**

```rust
use eggsact::split_at_operators;

assert_eq!(
    split_at_operators("3 + 5 * 2"),
    vec!["3", "+", "5", "*", "2"]
);

assert_eq!(
    split_at_operators("(10 + 2) / 4"),
    vec!["(10 + 2)", "/", "4"]
);

assert_eq!(
    split_at_operators("2 ** 10"),
    vec!["2", "**", "10"]
);
```

---

## Text Module

The `eggsact::text` module provides text processing utilities. All public functions are re-exported from `eggsact::text`.

### Measurement

```rust
use eggsact::text::{text_length, word_count, line_count, char_frequency};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `text_length` | `(text: &str) -> usize` | Character count |
| `word_count` | `(text: &str) -> usize` | Word count (whitespace-delimited) |
| `line_count` | `(text: &str) -> usize` | Newline-separated line count |
| `char_frequency` | `(text: &str) -> HashMap<char, usize>` | Frequency map of each character |

### Diff and Similarity

```rust
use eggsact::text::{levenshtein_distance, diff_spans, DiffSpan};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `levenshtein_distance` | `(a: &str, b: &str) -> usize` | Edit distance between two strings |
| `diff_spans` | `(a: &str, b: &str, max_diffs: usize) -> Vec<DiffSpan>` | Semantic diff as a list of equal/insert/delete spans |

### Validation

```rust
use eggsact::text::{
    validate_brackets, validate_json, validate_regex,
    regex_test, json_shape, regex_finditer,
};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `validate_brackets` | `(input: &str) -> CheckBracketsResult` | Balanced bracket/paren/brace check (field: `balanced`) |
| `validate_json` | `(input: &str) -> ValidateJsonResult` | JSON syntax validation (field: `valid`) |
| `validate_regex` | `(pattern: &str, text: &str) -> Result<bool, String>` | Regex syntax validation and match test |
| `regex_test` | `(pattern: &str, samples: &[&str], flags: Option<&Vec<String>>, ignore_case: bool, multiline: bool, dotall: bool, ascii: bool) -> RegexTestResult` | Test a regex against text samples |
| `json_shape` | `(input: &str) -> JsonShapeResult` | Infer JSON structure/schema |
| `regex_finditer` | `(pattern: &str, text: &str) -> RegexFindIterResult` | Find all regex matches with positions |

### Transforms

```rust
use eggsact::text::{
    escape_text, unescape_text, text_transform,
    text_fingerprint, text_hash,
};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `escape_text` | `(text: &str, mode: &str) -> Result<EscapeTextResult, String>` | Escape characters (json, html, url, etc.) |
| `unescape_text` | `(text: &str, escape_type: &str) -> UnescapeTextResult` | Reverse escaping |
| `text_transform` | `(text: &str, transform: &str) -> TextTransformResult` | Apply named transform (upper, lower, snake, camel, etc.) |
| `text_fingerprint` | `(text: &str) -> TextFingerprintResult` | Stable text fingerprint for comparison |
| `text_hash` | `(text: &str, algorithms: &[String], encoding: &str) -> TextHashResult` | Hash text (sha256, sha1, md5, crc32) |

### Paths

```rust
use eggsact::text::{path_analyze, path_compare, path_scope_check};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `path_analyze` | `(path: &str, style: &str) -> PathAnalyzeResult` | Lexical analysis of a file path |
| `path_compare` | `(a: &str, b: &str) -> PathCompareResult` | Compare two paths for equivalence |
| `path_scope_check` | `(path: &str, scope: &str) -> PathScopeCheckResult` | Check if path is within a scope directory |

### Shell

```rust
use eggsact::text::{shell_split, shell_quote_join, argv_compare};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `shell_split` | `(command: &str, shell: &str, detect_risky_features: bool) -> ShellSplitResult` | Split a shell command string into argv |
| `shell_quote_join` | `(argv: &[String], shell: &str) -> ShellQuoteJoinResult` | Join args into a quoted shell string |
| `argv_compare` | `(a: &str, b: &str) -> ArgvCompareResult` | Compare two command strings |

### Identifiers

```rust
use eggsact::text::{identifier_analyze, identifier_inspect, identifier_table_inspect};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `identifier_analyze` | `(text: &str, languages: Option<Vec<&str>>) -> IdentifierAnalyzeResult` | Classify naming convention (snake, camel, etc.) |
| `identifier_inspect` | `(name: &str) -> IdentifierInspectResult` | Detect confusable characters in an identifier |
| `identifier_table_inspect` | `(names: &[&str]) -> IdentifierTableInspectResult` | Batch identifier analysis |

### Config and Data Formats

```rust
use eggsact::text::{dotenv_validate, ini_validate, validate_toml, toml_shape};
use eggsact::text::{cargo_toml_inspect, check_version_constraint, version_compare};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `dotenv_validate` | `(text: &str, allow_export: bool, key_pattern: &str, duplicate_policy: &str) -> DotenvValidateResult` | Validate .env file syntax |
| `ini_validate` | `(text: &str, duplicate_policy: &str) -> IniValidateResult` | Validate INI file syntax |
| `validate_toml` | `(input: &str) -> ValidateTomlResult` | Validate TOML syntax |
| `toml_shape` | `(text: &str, max_tables: usize) -> TomlShapeResult` | Infer TOML structure |
| `cargo_toml_inspect` | `(input: &str) -> CargoInspectResult` | Extract metadata from Cargo.toml |
| `check_version_constraint` | `(version: &str, constraint: &str, scheme: &str) -> VersionConstraintResult` | Check semver constraint |
| `version_compare` | `(a: &str, b: &str, scheme: &str) -> VersionCompareResult` | Compare two version strings |

### Markdown

```rust
use eggsact::text::{markdown_structure, code_fence_extract};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `markdown_structure` | `(text: &str) -> MarkdownStructureResult` | Analyze markdown headings and structure |
| `code_fence_extract` | `(text: &str) -> CodeFenceExtractResult` | Extract code blocks with language tags |

### Patch

```rust
use eggsact::text::{patch_apply_check, patch_summary};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `patch_apply_check` | `(patch: &str, target: &str) -> PatchApplyCheckResult` | Check if a unified diff applies cleanly |
| `patch_summary` | `(patch: &str) -> PatchSummaryResult` | Summarize a unified diff |

### Unicode and Regex Safety

```rust
use eggsact::text::{regex_safety_check, text_replace_check};
use eggsact::text::{unicode_policy_check, canonicalize_text};
use eggsact::text::{has_confusables, find_confusables};
use eggsact::text::{CONFUSABLES};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `regex_safety_check` | `(pattern: &str) -> RegexSafetyResult` | Detect ReDoS-vulnerable patterns |
| `text_replace_check` | `(text: &str, find: &str, replace: &str) -> TextReplaceCheckResult` | Check replacement safety |
| `unicode_policy_check` | `(text: &str) -> UnicodePolicyCheckResult` | Check text against Unicode safety policies |
| `canonicalize_text` | `(text: &str, profile: &str) -> CanonicalizeResultWithMapping` | Normalize Unicode text |
| `has_confusables` | `(text: &str) -> bool` | Check for homoglyph characters |
| `find_confusables` | `(text: &str) -> Vec<(char, &'static str)>` | Find confusable characters with mappings |

### Position and Line Ranges

```rust
use eggsact::text::{line_range_extract, line_range_compare};
use eggsact::text::position::{text_position, text_window};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `line_range_extract` | `(text: &str, start_line: usize, end_line: usize, line_base: usize, include_line_numbers: bool, include_fingerprint: bool) -> LineRangeExtractResult` | Extract a line range from text |
| `line_range_compare` | `(left_text: &str, right_text: &str, start_line: usize, end_line: usize, line_base: usize, comparison_mode: &str) -> LineRangeCompareResult` | Compare text in a line range |

### Grapheme Primitives

```rust
use eggsact::text::{count_graphemes, truncate_to_grapheme};
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `count_graphemes` | `(text: &str) -> usize` | Count Unicode grapheme clusters |
| `truncate_to_grapheme` | `(text: &str, max: usize) -> String` | Truncate to N grapheme clusters |

### Glob

```rust
use eggsact::text::glob::glob_match;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `glob_match` | `(pattern: &str, path: &str, platform: &str, case_sensitive: bool) -> GlobMatchResult` | Test if a path matches a glob pattern |

---

## MCP Module

The `eggsact::mcp` module provides a JSON-RPC 2.0 server over stdio for AI coding agent integration.

### Starting the Server

```rust
#[tokio::main]
async fn main() -> ! {
    eggsact::mcp::server::main().await
}
```

The server reads JSON-RPC requests from stdin and writes responses to stdout. It supports the following MCP methods:

| Method | Description |
|--------|-------------|
| `initialize` | Returns server info and capabilities |
| `tools/list` | Returns the list of available tools |
| `tools/call` | Executes a tool by name |

### Available MCP Tools

The server exposes 78 tools, including:

- `math_eval` -- evaluate math expressions (NL or symbolic)
- `text_measure` -- measure text properties (bytes, chars, words, lines)
- `validate_json` / `validate_brackets` / `validate_regex` -- validation
- `text_diff_explain` -- semantic diff
- `text_fingerprint` / `text_hash` -- text comparison
- `escape_text` / `unescape_text` -- escaping
- `unit_convert` / `unit_info` / `constant_lookup` -- units and constants
- `path_normalize` / `path_analyze` / `path_compare` -- path operations
- `shell_split` / `shell_quote_join` / `argv_compare` -- shell operations
- `markdown_structure` / `code_fence_extract` -- markdown parsing
- `patch_apply_check` / `patch_summary` -- patch operations
- `identifier_analyze` / `identifier_inspect` -- identifier analysis
- `json_extract` / `json_compare` / `json_query` / `json_canonicalize` -- JSON operations
- `glob_match` -- glob pattern matching

### Programmatic Usage

For most use cases, call `run()` or `evaluate()` directly rather than starting the MCP server:

```rust
use eggsact::{run, evaluate};

// Natural language
let (result, _typ) = run("thirty plus five").unwrap();

// Direct math
let (result, _typ) = evaluate("sqrt(144)").unwrap();
```

Use the MCP server only when integrating with an AI agent framework that speaks JSON-RPC 2.0 over stdio.

---

## Agent Module

The `eggsact::agent` module provides an in-process API for calling tools without starting the MCP server. Use this when integrating eggsact as a Rust library dependency.

### ToolRegistry

```rust
use eggsact::agent::{ToolRegistry, Profile, ToolAudience};

// Default: StrictNative compat, full profile, Model audience
let registry = ToolRegistry::default();

// Model-facing codegg session
let registry = ToolRegistry::with_profile_and_audience(
    Profile::CodeggCoreMin, ToolAudience::Model,
);
let tools = registry.available_tools_model_safe();

// Harness preflight checks
let harness_registry = ToolRegistry::with_profile_and_audience(
    Profile::CodeggPreflight, ToolAudience::Harness,
);
```

> **Note**: `available_tools()` is deprecated since 0.3.0. Use
> `available_tools_model_safe()`, `available_tools_for_audience()`, or
> `available_tools_for_current_audience()` instead.

### Calling Tools

```rust
use eggsact::agent::{ToolRegistry, ExecutionContext};

let registry = ToolRegistry::default();

// Simple call
let response = registry.call_json("math_eval", serde_json::json!({"expression": "2 + 3"}))?;
assert!(response.ok);

// Context-aware call (recommended for new code)
let ctx = ExecutionContext::builder()
    .profile(Profile::Full)
    .audience(ToolAudience::Model)
    .compatibility_mode(eggsact::agent::CompatibilityMode::StrictNative)
    .build();
let response = registry.call_json_with_execution_context(
    "math_eval",
    serde_json::json!({"expression": "2 + 3"}),
    &ctx,
)?;
```

### ExecutionContext

`ExecutionContext` carries per-request state for context-aware dispatch:

| Field | Type | Purpose |
|-------|------|---------|
| `profile` | `Option<Profile>` | Override tool availability (falls back to registry default) |
| `audience` | `Option<ToolAudience>` | Override exposure checks (falls back to registry default) |
| `compatibility_mode` | `CompatibilityMode` | Controls validation error type names |
| `eval_ctx` | `EvalContext` | Calculator state (PRNG seed, memory registers, variables) |
| `budget` | `Option<ToolBudget>` | Per-call resource limits |
| `cancellation` | `Option<Arc<AtomicBool>>` | Cooperative cancellation flag |

Builder methods: `with_eval_context()`, `with_budget()`, `with_cancellation()`, `with_request_id()`.

### Typed Preflight Wrappers

```rust
use eggsact::preflight::{EditPreflight, EditPreflightInput, ReplacementMode};

let input = EditPreflightInput {
    original: "hello world".to_string(),
    mode: ReplacementMode::Literal,
    old: Some("world".to_string()),
    new: Some("rust".to_string()),
    ..Default::default()
};
let output = EditPreflight::run(&input).unwrap();
assert!(output.ok_to_apply);
```

Available wrappers: `EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect`.

All return `Result<Output, PreflightError>` where `PreflightError` distinguishes `ToolCall`, `ToolRejected`, and `ContractViolation` (missing mandatory fields are hard failures).

---

## Error Handling

`evaluate()` returns `Err(String)`. `run()` returns `Err(RunError)`, which displays as the underlying message. The common error patterns are:

| Error | Cause |
|-------|-------|
| `"Input exceeds 100000 characters"` | Input too long |
| `"Parse error: ..."` | Invalid syntax in `evaluate()` |
| `"Unknown function: ..."` | Unrecognized function name |
| `"Unknown constant: ..."` | Unrecognized constant name |
| `"Division by zero"` | Division by zero |
| `"Stack overflow (expression too nested)"` | Recursion depth exceeded (max 100) |
| `"Value overflow"` | Numeric overflow |
| `"No unit to convert from"` | Unit conversion without a source unit |
| `"Cannot convert between ... and ..."` | Incompatible unit categories |

```rust
use eggsact::{run, evaluate};

// Error case
match evaluate("1 / 0") {
    Ok(_) => unreachable!(),
    Err(e) => assert_eq!(e, "Division by zero"),
}

// Natural language error
match run("sqrt of negative one") {
    Ok(_) => unreachable!(),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Complete Examples

### Basic Natural Language Calculator

```rust
use eggsact::run;

fn main() {
    let expressions = vec![
        "five plus three",
        "twenty times six",
        "one hundred divided by four",
        "what is the square root of 144",
        "calculate 2 to the power of 10",
        "50 percent of 200",
    ];

    for expr in expressions {
        match run(expr) {
            Ok((result, typ)) => println!("{} = {} ({})", expr, result, typ),
            Err(e) => eprintln!("{}: error: {}", expr, e),
        }
    }
}
```

### Unit Conversions

```rust
use eggsact::run;

fn main() {
    let conversions = vec![
        "30m + 100ft",        // meters + feet
        "1km in miles",       // km to miles
        "72F in C",           // Fahrenheit to Celsius
        "1024KB in MB",       // kilobytes to megabytes
        "1gal in L",          // gallons to liters
    ];

    for expr in conversions {
        match run(expr) {
            Ok((result, _)) => println!("{} => {}", expr, result),
            Err(e) => eprintln!("{}: {}", expr, e),
        }
    }
}
```

### Text Processing Library

```rust
use eggsact::text::{
    text_length, word_count, levenshtein_distance,
    validate_json, validate_brackets,
    escape_text, text_fingerprint, text_hash,
};

fn main() {
    // Measurement
    let text = "Hello, world!";
    println!("length: {}", text_length(text));
    println!("words: {}", word_count(text));

    // Similarity
    let dist = levenshtein_distance("kitten", "sitting");
    println!("edit distance: {}", dist);

    // Validation
    let json_result = validate_json(r#"{"key": "value"}"#);
    println!("valid JSON: {}", json_result.valid);

    let bracket_result = validate_brackets("({[]})");
    println!("balanced: {}", bracket_result.balanced);

    // Transforms
    let escaped = escape_text("line1\nline2", "json").unwrap();
    println!("escaped: {}", escaped.escaped);

    let hash = text_hash("hello world", &["sha256".to_string()], "utf-8");
    println!("sha256: {}", hash.hashes.get("sha256").unwrap());
}
```

### Constants and Scientific Math

```rust
use eggsact::evaluate;

fn main() {
    // Mathematical constants
    let (r, _) = evaluate("pi * 2").unwrap();
    println!("tau = {}", r);

    // Physical constants
    let (r, _) = evaluate("c").unwrap();
    println!("speed of light = {} m/s", r);

    let (r, _) = evaluate("na").unwrap();
    println!("Avogadro = {}", r);

    // Scientific computation
    let (r, _) = evaluate("6.626e-34 * 3e8 / 500e-9").unwrap();
    println!("photon energy (500nm) = {} J", r);
}
```
