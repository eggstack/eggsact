# Preflight Wrappers

The `src/preflight/` module provides typed Rust wrappers over the raw JSON tool interface for common codegg workflows. Each wrapper has typed `Input`/`Output` structs, a `run()` method, and a `parse_response()` method for testing contract parsing without a full registry call.

See also: [Agent API](agent-api.md), [Tool Implementations](tools.md)

## Files

| File | Purpose |
|------|---------|
| `src/preflight/mod.rs` | All preflight wrappers, input/output structs, verdict enums, finding parsing |

## Error Taxonomy

### `PreflightError`

```rust
pub enum PreflightError {
    ToolCall(ToolCallError),          // Registry rejected before execution
    ToolRejected {                    // Tool returned ok: false
        machine_code: Option<String>,
        error_type: Option<String>,
        message: String,
    },
    ContractViolation {               // ok: true but missing mandatory field
        tool: &'static str,
        field: &'static str,
        message: String,
    },
}
```

`ContractViolation` is a **hard failure** — wrappers will not silently default `machine_code`, `verdict`, or other route-critical fields.

## Available Wrappers

| Wrapper | Tool | Verdict Enum | Purpose |
|---------|------|-------------|---------|
| `EditPreflight` | `edit_preflight` | `EditVerdict` | Edit operation pre-check |
| `CommandPreflight` | `command_preflight` | `CommandVerdict` | Shell command pre-check |
| `ConfigPreflight` | `config_preflight` | `ConfigVerdict` | Config file pre-check |
| `PatchApplyCheck` | `patch_apply_check` | `EditVerdict` | Patch apply pre-check |
| `TextSecurityInspect` | `text_security_inspect` | (string verdict) | Text security inspection |

All wrappers return `Result<Output, PreflightError>`.

## Typed Verdict Enums

### `EditVerdict`

`Allow`, `Review`, `Block`, `SafeToApply`, `SafeWithWarnings`, `Other(String)`

### `CommandVerdict`

`Allow`, `Review`, `Block`, `Other(String)`

### `ConfigVerdict`

`Valid`, `ValidWithWarnings`, `Invalid`, `Other(String)`

### `FindingSeverity`

`Info`, `Low`, `Medium`, `High`, `Critical`, `Other(String)`

### `FindingDisposition`

`Informational`, `Caution`, `Blocking`, `Other(String)`

All verdict/severity/disposition enums have `Other(String)` for forward compatibility.

## `Finding` Struct

```rust
pub struct Finding {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub disposition: Option<String>,
    pub location: Option<Value>,
    pub details: Option<Value>,
}
```

### Parsing Methods

| Method | Behavior |
|--------|----------|
| `from_value(v)` | Permissive — drops malformed entries |
| `from_array(arr)` | Permissive — drops malformed entries |
| `try_from_value_strict(v, tool)` | Strict — `ContractViolation` on missing `code`/`severity`/`message` |
| `from_array_strict(arr, tool)` | Strict — `ContractViolation` on any malformed element |

Strict parsing is used by all typed preflight wrappers.

## `RecommendedNextTool`

```rust
pub struct RecommendedNextTool {
    pub name: String,
    pub reason: Option<String>,
    pub arguments_hint: Option<Value>,
}
```

Parsed from both string (`"tool_name"`) and object (`{ name, reason, arguments_hint }`) shapes. Fails closed on malformed values.

## Edit Preflight

### Input

```rust
pub struct EditPreflightInput {
    pub original: String,
    pub mode: ReplacementMode,         // Literal, Patch, LineRange
    pub old: Option<String>,
    pub new: Option<String>,
    pub patch: Option<String>,
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
    pub expected_fingerprint: Option<String>,
    pub strict: bool,
    pub file_path: Option<String>,     // enables path_scope_check
    pub workspace_root: Option<String>,
    pub newline_policy: EditNewlinePolicy,
    pub unicode_policy: EditUnicodePolicy,
    pub edit_metadata: Option<EditMetadata>,
}
```

### Output

```rust
pub struct EditPreflightOutput {
    pub ok_to_apply: bool,
    pub mode: String,
    pub verdict: EditVerdict,
    pub machine_code: String,
    pub secondary_machine_codes: Vec<String>,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub recommended_next_tool: Option<RecommendedNextTool>,
    pub path_scope: Option<PathScopeResult>,
    pub newline_check: Option<NewlineCheckResult>,
    pub unicode_check: Option<UnicodeCheckResult>,
    pub fingerprint: Option<FingerprintResult>,
    pub raw: Value,
}
```

## Command Preflight

### Input

```rust
pub struct CommandPreflightInput {
    pub command: String,
    pub platform: String,
    pub policy: CommandPolicy,         // Default, Strict, Permissive
    pub policy_config: Option<CommandPolicyConfig>,
    pub working_directory: Option<String>,
}
```

### Output

```rust
pub struct CommandPreflightOutput {
    pub verdict: CommandVerdict,
    pub machine_code: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub program: Option<String>,
    pub subcommand: Option<String>,
    pub features: Vec<String>,
    pub matched_rules: Vec<String>,
    pub argv: Option<Vec<String>>,
    pub recommended_next_tool: Option<RecommendedNextTool>,
    pub raw: Value,
}
```

## Config Preflight

### Input

```rust
pub struct ConfigPreflightInput {
    pub text: String,
    pub format: ConfigFormat,          // Auto, Json, Toml, Dotenv, Ini, CargoToml
    pub schema: Option<Value>,
    pub strict: bool,
}
```

### Output

```rust
pub struct ConfigPreflightOutput {
    pub valid: bool,
    pub verdict: ConfigVerdict,
    pub detected_format: Option<String>,
    pub machine_code: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub raw: Value,
}
```

## Patch Apply Check

### Input

```rust
pub struct PatchApplyCheckInput {
    pub patch_text: String,
    pub original_text: String,
    pub return_result_text: bool,
    pub strict: bool,
}
```

### Output

```rust
pub struct PatchApplyCheckOutput {
    pub patch_parse_ok: bool,
    pub applies: bool,
    pub hunks_total: u64,
    pub hunks_applied: u64,
    pub hunks_failed: u64,
    pub failed_hunks: Value,
    pub affected_line_ranges: Value,
    pub newline_style_before: Option<String>,
    pub newline_style_after: Option<String>,
    pub result_fingerprint: Option<String>,
    pub result_text: Option<String>,
    pub verdict: EditVerdict,
    pub machine_code: String,
    pub findings: Vec<Finding>,
    pub raw: Value,
}
```

## Text Security Inspect

### Input

```rust
pub struct TextSecurityInspectInput {
    pub text: String,
    pub policy: String,
    pub detail: Option<String>,
}
```

## Usage Example

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

## Testing

Each wrapper has a `parse_response()` method for testing contract parsing without a full registry call. This allows testing the typed contract in isolation.

```rust
use eggsact::preflight::{EditPreflight, EditPreflightInput};

// Test contract parsing directly
let response = /* ... raw ToolResponse ... */;
let output = EditPreflight::parse_response(response).unwrap();
assert_eq!(output.verdict, EditVerdict::Allow);
```

```bash
cargo test --test lib -- test_preflight   # integration tests
```
