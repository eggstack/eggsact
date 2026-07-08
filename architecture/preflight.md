# Preflight Wrappers

The `src/preflight/` module provides typed Rust wrappers over the raw JSON MCP tool interface for common codegg workflows. Each wrapper has typed `Input`/`Output` structs, a `run()` method (using a default `ToolRegistry`), a `run_with_registry()` method (for caller-provided registries with custom profiles/audiences), and a public `parse_response()` method for testing contract parsing without a full registry call.

See also: [Agent API](agent-api.md), [Tool Implementations](tools.md)

## Files

| File | Purpose |
|------|---------|
| `src/preflight/mod.rs` | All preflight wrappers, input/output structs, verdict enums, finding parsing, `RecommendedNextTool`, contract parsing helpers |

All code lives in a single file. There are no sub-modules.

## Error Taxonomy

### `PreflightError`

Every wrapper returns `Result<Output, PreflightError>`. The error enum distinguishes three failure modes:

```rust
pub enum PreflightError {
    ToolCall(ToolCallError),
    ToolRejected {
        machine_code: Option<String>,
        error_type: Option<String>,
        message: String,
    },
    ContractViolation {
        tool: &'static str,
        field: &'static str,
        message: String,
    },
}
```

| Variant | Meaning | When it occurs |
|---------|---------|---------------|
| `ToolCall` | Registry rejected the call before execution | Unknown tool name, invalid arguments at registry level, audience/profile mismatch |
| `ToolRejected` | Tool executed but returned `ok: false` | Tool-level validation failure — includes `machine_code`, `error_type`, and `message` from the tool response |
| `ContractViolation` | Tool returned `ok: true` but response shape violated the typed contract | Missing mandatory field (`machine_code`, `verdict`, `ok_to_apply`, etc.), unexpected type, malformed finding. **This is a hard failure** — wrappers never silently default missing route-critical fields |

`PreflightError` implements `Display`, `Error`, and `From<ToolCallError>`.

## Available Wrappers

| Wrapper | Underlying Tool | Verdict Enum | Purpose |
|---------|----------------|-------------|---------|
| `EditPreflight` | `edit_preflight` | `EditVerdict` | Pre-check before file edits (literal replace, patch, line range) |
| `CommandPreflight` | `command_preflight` | `CommandVerdict` | Pre-check before shell command execution |
| `ConfigPreflight` | `config_preflight` | `ConfigVerdict` | Pre-check for config file syntax/schema validity |
| `PatchApplyCheck` | `patch_apply_check` | `EditVerdict` | Pre-check for unified diff patch application |
| `TextSecurityInspect` | `text_security_inspect` | (string verdict) | Unicode security inspection for text content |

All wrappers are zero-sized structs (`pub struct Foo;`) with `impl` blocks containing `run()`, `run_with_registry()`, and `parse_response()`.

## Typed Verdict Enums

All verdict/severity/disposition enums implement `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and provide `as_str()` and `parse(&str)` methods. Each includes an `Other(String)` variant for forward compatibility — unknown string values from the tool response map to `Other`.

### `EditVerdict`

| Variant | String value | Notes |
|---------|-------------|-------|
| `Allow` | `"allow"` | Edit is safe to apply |
| `Review` | `"review"` | Edit needs human review |
| `Block` | `"block"` | Edit must not be applied |
| `SafeToApply` | `"safe_to_apply"` | Legacy alias, kept for backward compatibility |
| `SafeWithWarnings` | `"safe_with_warnings"` | Legacy alias, kept for backward compatibility |
| `Other(String)` | (raw string) | Forward compatibility for future values |

### `CommandVerdict`

| Variant | String value |
|---------|-------------|
| `Allow` | `"allow"` |
| `Review` | `"review"` |
| `Block` | `"block"` |
| `Other(String)` | (raw string) |

### `ConfigVerdict`

| Variant | String value |
|---------|-------------|
| `Valid` | `"valid"` |
| `ValidWithWarnings` | `"valid_with_warnings"` |
| `Invalid` | `"invalid"` |
| `Other(String)` | (raw string) |

### `FindingSeverity`

| Variant | String value |
|---------|-------------|
| `Info` | `"info"` |
| `Low` | `"low"` |
| `Medium` | `"medium"` |
| `High` | `"high"` |
| `Critical` | `"critical"` |
| `Other(String)` | (raw string) |

### `FindingDisposition`

| Variant | String value |
|---------|-------------|
| `Informational` | `"informational"` |
| `Caution` | `"caution"` |
| `Blocking` | `"blocking"` |
| `Other(String)` | (raw string) |

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

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `code` | `String` | Yes | Machine-readable finding code (e.g. `EDIT_AMBIGUOUS_MATCH`, `UNICODE_RISK`) |
| `severity` | `String` | Yes | Severity level string — use `severity_enum()` for typed access |
| `message` | `String` | Yes | Human-readable description |
| `disposition` | `Option<String>` | No | Advisory disposition — use `disposition_enum()` for typed access |
| `location` | `Option<Value>` | No | JSON value indicating where in the content the finding occurred |
| `details` | `Option<Value>` | No | Arbitrary JSON payload with additional context |

### Parsing Methods

| Method | Behavior | When to use |
|--------|----------|-------------|
| `from_value(v)` | Permissive — returns `None` if missing `code` or `message`; defaults missing `severity` to `"info"` | Legacy integrations, best-effort parsing |
| `from_array(arr)` | Permissive — drops malformed entries silently | Legacy integrations |
| `try_from_value_strict(v, tool)` | Strict — requires `code`, `severity`, and `message` as strings; returns `ContractViolation` on missing fields | All typed preflight wrappers |
| `from_array_strict(arr, tool)` | Strict — `ContractViolation` if any element is malformed (does not drop) | All typed preflight wrappers |

### Typed Accessors

```rust
impl Finding {
    pub fn severity_enum(&self) -> FindingSeverity;
    pub fn disposition_enum(&self) -> Option<FindingDisposition>;
}
```

Strict parsing requires all three fields (`code`, `severity`, `message`) to be present strings. Unknown severity/disposition strings parse as `Other(String)` — they do **not** cause contract violations. Non-string values (e.g. numeric `severity: 42`) **do** cause `ContractViolation`.

## `RecommendedNextTool`

```rust
pub struct RecommendedNextTool {
    pub name: String,
    pub reason: Option<String>,
    pub arguments_hint: Option<Value>,
}
```

Parsed from the `recommended_next_tool` field on `ToolResponse`. Accepts two wire shapes:

| Shape | Example | Result |
|-------|---------|--------|
| String (legacy) | `"command_preflight"` | `name: "command_preflight"`, `reason: None`, `arguments_hint: None` |
| Object | `{"name": "edit_preflight", "reason": "...", "arguments_hint": {...}}` | Parsed into all three fields |

Malformed values (objects missing `name`, non-string/non-object types like numbers) return `ContractViolation`. A null `recommended_next_tool` field produces `None` (no error).

## Edit Preflight

Typed wrapper for the `edit_preflight` tool. Supports three replacement modes and composes several sub-tools internally.

### Input

```rust
pub struct EditPreflightInput {
    pub original: String,
    pub mode: ReplacementMode,
    pub old: Option<String>,
    pub new: Option<String>,
    pub patch: Option<String>,
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
    pub expected_fingerprint: Option<String>,
    pub strict: bool,
    pub file_path: Option<String>,
    pub workspace_root: Option<String>,
    pub newline_policy: EditNewlinePolicy,
    pub unicode_policy: EditUnicodePolicy,
    pub edit_metadata: Option<EditMetadata>,
}
```

#### Replacement Modes

| Mode | Variant | Fields used | Description |
|------|---------|------------|-------------|
| Literal | `ReplacementMode::Literal` | `old`, `new` | Exact string find-and-replace |
| Patch | `ReplacementMode::Patch` | `patch` | Unified diff patch application |
| Line Range | `ReplacementMode::LineRange` | `start_line`, `end_line`, `new` | Replace lines `start_line..=end_line` with `new` |

#### Sub-tool Composition

| Sub-tool | Triggered when | Output field |
|----------|---------------|--------------|
| `path_scope_check` | Both `file_path` and `workspace_root` are provided | `path_scope: Option<PathScopeResult>` |
| `text_fingerprint` (newline detection) | `newline_policy != Skip` | `newline_check: Option<NewlineCheckResult>` |
| `text_security_inspect` | `unicode_policy != Skip` | `unicode_check: Option<UnicodeCheckResult>` |
| `text_fingerprint` (SHA-256) | `expected_fingerprint` is provided | `fingerprint: Option<FingerprintResult>` |

#### EditNewlinePolicy

| Variant | String value | Description |
|---------|-------------|-------------|
| `Skip` | `"skip"` | No newline checking (default) |
| `Check` | `"check"` | Flag mixed newlines (CRLF/LF in same file) |
| `NormalizeLf` | `"normalize_lf"` | Normalize to LF before comparison |
| `NormalizeCrlf` | `"normalize_crlf"` | Normalize to CRLF before comparison |

#### EditUnicodePolicy

| Variant | String value | Description |
|---------|-------------|-------------|
| `Skip` | `"skip"` | No unicode security checks (default) |
| `Default` | `"default"` | Default security checks (invisible chars, confusables, bidi) |
| `SourceCode` | `"source_code"` | Stricter policy for source code files |
| `Identifier` | `"identifier"` | Policy for identifier text |

#### EditMetadata

```rust
pub struct EditMetadata {
    pub description: Option<String>,
    pub author: Option<String>,
    pub source_tool: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
}
```

Passthrough metadata for logging and diagnostics. All fields optional.

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

#### Sub-result Structs

**PathScopeResult** — from `path_scope_check`:

| Field | Type | Description |
|-------|------|-------------|
| `inside_root` | `bool` | Whether target path is inside workspace root |
| `escapes_via_dotdot` | `bool` | Whether path uses `..` traversal |
| `relative_path` | `String` | Normalized relative path from root |
| `normalized_target` | `Option<String>` | Normalized absolute target path (lexical only) |
| `reason` | `Option<String>` | Human-readable reason for scope decision |

**NewlineCheckResult** — from newline detection:

| Field | Type | Description |
|-------|------|-------------|
| `style` | `String` | Detected style: `"LF"`, `"CRLF"`, `"CR"`, `"mixed"`, or `"none"` |
| `mixed` | `bool` | Whether mixed newlines were detected |
| `policy` | `Option<String>` | Applied policy name |
| `recommended_normalization` | `Option<String>` | Recommended target (`"lf"` or `"crlf"`) |
| `original_style` | `Option<String>` | Newline style in original text |
| `replacement_style` | `Option<String>` | Newline style in replacement text |

**UnicodeCheckResult** — from `text_security_inspect`:

| Field | Type | Description |
|-------|------|-------------|
| `verdict` | `String` | Overall verdict: `"allow"`, `"review"`, or `"block"` |
| `machine_code` | `String` | Machine code from security inspect |
| `finding_count` | `usize` | Number of findings |
| `findings` | `Vec<Value>` | Raw JSON findings (structured when available) |

**FingerprintResult** — from `text_fingerprint`:

| Field | Type | Description |
|-------|------|-------------|
| `sha256` | `String` | SHA-256 fingerprint of the text |
| `newline_style` | `String` | Detected newline style |

## Command Preflight

Typed wrapper for the `command_preflight` tool. Analyzes shell commands for safety before execution.

### Input

```rust
pub struct CommandPreflightInput {
    pub command: String,
    pub platform: String,
    pub policy: CommandPolicy,
    pub policy_config: Option<CommandPolicyConfig>,
    pub working_directory: Option<String>,
}
```

#### CommandPolicy

| Variant | String value | Description |
|---------|-------------|-------------|
| `Default` | `"default"` | Standard policy checks (default) |
| `Strict` | `"strict"` | Stricter policy — fewer allowed commands |
| `Permissive` | `"permissive"` | Relaxed policy — more commands allowed |

#### CommandPolicyConfig

Structured overrides that refine or override the built-in policy. Deny beats allow when both are set for the same category.

```rust
pub struct CommandPolicyConfig {
    pub allow_commands: Option<Vec<String>>,
    pub deny_commands: Option<Vec<String>>,
    pub allow_subcommands: Option<HashMap<String, Vec<String>>>,
    pub deny_subcommands: Option<HashMap<String, Vec<String>>>,
    pub allow_network: Option<bool>,
    pub allow_filesystem_write: Option<bool>,
    pub allow_process_control: Option<bool>,
    pub allow_env_mutation: Option<bool>,
    pub max_command_length: Option<u64>,
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `allow_commands` | `Option<Vec<String>>` | none | Explicit allow list of program names |
| `deny_commands` | `Option<Vec<String>>` | none | Explicit deny list (overrides allow) |
| `allow_subcommands` | `Option<HashMap<String, Vec<String>>>` | none | Per-program allowed subcommands |
| `deny_subcommands` | `Option<HashMap<String, Vec<String>>>` | none | Per-program denied subcommands (overrides allow) |
| `allow_network` | `Option<bool>` | `false` | Allow network access |
| `allow_filesystem_write` | `Option<bool>` | `false` | Allow filesystem writes |
| `allow_process_control` | `Option<bool>` | `false` | Allow process control |
| `allow_env_mutation` | `Option<bool>` | `false` | Allow environment variable mutation |
| `max_command_length` | `Option<u64>` | none | Maximum command length in characters |

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

| Field | Type | Description |
|-------|------|-------------|
| `program` | `Option<String>` | Extracted program name (e.g. `"ls"`, `"git"`) |
| `subcommand` | `Option<String>` | Extracted subcommand (e.g. `"commit"`, `"status"`) |
| `features` | `Vec<String>` | Detected risky shell features (e.g. pipes, redirects, subshells) |
| `matched_rules` | `Vec<String>` | Policy rule identifiers that matched during analysis |
| `argv` | `Option<Vec<String>>` | Parsed argv if shell splitting succeeded |

## Config Preflight

Typed wrapper for the `config_preflight` tool. Validates config file syntax and optional schema.

### Input

```rust
pub struct ConfigPreflightInput {
    pub text: String,
    pub format: ConfigFormat,
    pub schema: Option<Value>,
    pub strict: bool,
}
```

#### ConfigFormat

| Variant | String value | Description |
|---------|-------------|-------------|
| `Auto` | `"auto"` | Auto-detect format from content (default) |
| `Json` | `"json"` | JSON |
| `Toml` | `"toml"` | TOML |
| `Dotenv` | `"dotenv"` | Dotenv `.env` files |
| `Ini` | `"ini"` | INI |
| `CargoToml` | `"cargo_toml"` | Cargo.toml (TOML with Cargo-specific validation) |

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

| Field | Type | Description |
|-------|------|-------------|
| `valid` | `bool` | Whether the config is structurally valid |
| `detected_format` | `Option<String>` | Format that was actually used for validation (may differ from input when `Auto`) |

## Patch Apply Check

Typed wrapper for the `patch_apply_check` tool. Tests whether a unified diff patch applies cleanly to the original text without actually modifying anything.

### Input

```rust
pub struct PatchApplyCheckInput {
    pub patch_text: String,
    pub original_text: String,
    pub return_result_text: bool,
    pub strict: bool,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `patch_text` | `String` | Unified diff patch to test |
| `original_text` | `String` | Original file content to apply against |
| `return_result_text` | `bool` | Whether to include the resulting text in the output |
| `strict` | `bool` | Strict mode for patch matching |

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

| Field | Type | Description |
|-------|------|-------------|
| `patch_parse_ok` | `bool` | Whether the patch text parsed successfully |
| `applies` | `bool` | Whether the patch applies cleanly to the original |
| `hunks_total` | `u64` | Total number of hunks in the patch |
| `hunks_applied` | `u64` | Number of hunks that applied successfully |
| `hunks_failed` | `u64` | Number of hunks that failed to apply |
| `failed_hunks` | `Value` | JSON array of failed hunk details |
| `affected_line_ranges` | `Value` | JSON array of `[start, end]` line ranges affected |
| `newline_style_before` | `Option<String>` | Newline style detected in original text |
| `newline_style_after` | `Option<String>` | Newline style detected in result text |
| `result_fingerprint` | `Option<String>` | SHA-256 fingerprint of the result text |
| `result_text` | `Option<String>` | Result text (only when `return_result_text` is `true`) |

## Text Security Inspect

Typed wrapper for the `text_security_inspect` tool. Checks text content for unicode security issues (invisible characters, confusables, bidi attacks).

### Input

```rust
pub struct TextSecurityInspectInput {
    pub text: String,
    pub policy: String,
    pub detail: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `text` | `String` | Text content to inspect |
| `policy` | `String` | Security policy to apply (e.g. `"default"`, `"source_code"`, `"identifier"`) |
| `detail` | `Option<String>` | Additional policy detail/parameter |

### Output

```rust
pub struct TextSecurityInspectOutput {
    pub verdict: String,
    pub policy: String,
    pub machine_code: String,
    pub normalized_changed: bool,
    pub recommended_action: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub subresults: Option<Value>,
    pub raw: Value,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `verdict` | `String` | Overall verdict: `"allow"`, `"review"`, or `"block"` |
| `policy` | `String` | Policy that was applied |
| `normalized_changed` | `bool` | Whether text normalization changed the content |
| `recommended_action` | `String` | Recommended follow-up action |
| `summary` | `String` | Human-readable summary |
| `subresults` | `Option<Value>` | Sub-tool results when composition is used (e.g. fingerprint, confusables analysis) |

## Usage Examples

### Basic Config Validation

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
assert_eq!(output.verdict, ConfigVerdict::Valid);
assert!(!output.machine_code.is_empty());
```

### Edit Preflight with Path Scope and Unicode Check

```rust
use eggsact::preflight::{
    EditPreflight, EditPreflightInput, ReplacementMode,
    EditNewlinePolicy, EditUnicodePolicy, EditVerdict,
};

let input = EditPreflightInput {
    original: "hello world".to_string(),
    mode: ReplacementMode::Literal,
    old: Some("hello".to_string()),
    new: Some("goodbye".to_string()),
    file_path: Some("src/main.rs".to_string()),
    workspace_root: Some(".".to_string()),
    newline_policy: EditNewlinePolicy::Check,
    unicode_policy: EditUnicodePolicy::Default,
    ..Default::default()
};
let output = EditPreflight::run(&input).unwrap();
assert!(output.ok_to_apply);
assert_eq!(output.verdict, EditVerdict::Allow);
assert!(output.path_scope.is_some());
assert!(output.unicode_check.is_some());
```

### Command Preflight with Policy Config

```rust
use eggsact::preflight::{
    CommandPreflight, CommandPreflightInput, CommandPolicy,
    CommandPolicyConfig, CommandVerdict,
};

let config = CommandPolicyConfig {
    deny_commands: Some(vec!["rm".to_string()]),
    allow_network: Some(false),
    ..Default::default()
};
let input = CommandPreflightInput {
    command: "ls -la".to_string(),
    platform: "posix".to_string(),
    policy: CommandPolicy::Default,
    policy_config: Some(config),
    working_directory: None,
};
let output = CommandPreflight::run(&input).unwrap();
assert_eq!(output.verdict, CommandVerdict::Allow);
```

### Patch Apply Check

```rust
use eggsact::preflight::{PatchApplyCheck, PatchApplyCheckInput, EditVerdict};

let input = PatchApplyCheckInput {
    patch_text: "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
    original_text: "old\n".to_string(),
    return_result_text: true,
    strict: true,
};
let output = PatchApplyCheck::run(&input).unwrap();
assert!(output.patch_parse_ok);
assert!(output.applies);
assert_eq!(output.verdict, EditVerdict::Allow);
```

### Text Security Inspect

```rust
use eggsact::preflight::{TextSecurityInspect, TextSecurityInspectInput};

let input = TextSecurityInspectInput {
    text: "Hello, world!".to_string(),
    policy: "default".to_string(),
    detail: None,
};
let output = TextSecurityInspect::run(&input).unwrap();
assert_eq!(output.verdict, "allow");
assert!(!output.machine_code.is_empty());
```

### Using a Custom ToolRegistry

```rust
use eggsact::agent::{ToolRegistry, Profile, ToolAudience};
use eggsact::preflight::{ConfigPreflight, ConfigPreflightInput, ConfigFormat};

let registry = ToolRegistry::with_profile_and_audience(
    Profile::Full,
    ToolAudience::Harness,
);
let input = ConfigPreflightInput {
    text: "key = \"value\"\n".to_string(),
    format: ConfigFormat::Toml,
    ..Default::default()
};
let output = ConfigPreflight::run_with_registry(&registry, &input).unwrap();
assert!(output.valid);
```

## Testing Approach

### `parse_response()` — Contract Parsing in Isolation

Every wrapper exposes `parse_response(response: ToolResponse) -> Result<Output, PreflightError>`. This allows testing the typed contract parsing without constructing a full `ToolRegistry` or executing tool logic.

```rust
use eggsact::preflight::{EditPreflight, EditPreflightInput, EditVerdict};
use eggsact::mcp::response::ToolResponse;

// Construct a synthetic ToolResponse
let response = ToolResponse::success(
    serde_json::json!({
        "ok_to_apply": true,
        "mode": "literal",
        "summary": "edit is safe",
        "verdict": "allow",
    }),
    Some("edit_preflight"),
)
.with_machine_code("EDIT_OK");

let output = EditPreflight::parse_response(response).unwrap();
assert_eq!(output.verdict, EditVerdict::Allow);
assert!(!output.machine_code.is_empty());
```

### Contract Violation Tests

Wrappers are tested for fail-closed behavior on malformed responses. Each mandatory field is individually omitted to verify it triggers `ContractViolation`:

```rust
// Missing verdict -> ContractViolation
let response = ToolResponse::success(
    serde_json::json!({
        "ok_to_apply": true,
        "mode": "literal",
        "summary": "test",
    }),
    Some("edit_preflight"),
).with_machine_code("EDIT_OK");

let err = EditPreflight::parse_response(response).unwrap_err();
assert!(matches!(
    err,
    PreflightError::ContractViolation { field, .. } if field == "verdict"
));
```

### Finding Parsing Tests

Strict finding parsing is tested for:
- Missing `code` → `ContractViolation`
- Missing `severity` → `ContractViolation`
- Missing `message` → `ContractViolation`
- Non-string severity (e.g. `42`) → `ContractViolation`
- Unknown severity string → parses as `Other(String)`, not an error
- Well-formed findings → pass through correctly

### RecommendedNextTool Parsing Tests

- String shape → `RecommendedNextTool { name, reason: None, arguments_hint: None }`
- Object shape with all fields → fully populated struct
- Object missing `name` → `ContractViolation`
- Non-string/non-object type (e.g. number) → `ContractViolation`

### Running Tests

```bash
cargo test --lib -- preflight     # unit tests in src/preflight/mod.rs
cargo test --test lib -- preflight # integration tests
```

### Testing Pattern Summary

| Test category | What it verifies | Approach |
|--------------|-----------------|----------|
| Success path | Correct parsing of well-formed responses | `ToolResponse::success()` → `parse_response()` → assert output fields |
| Contract violation | Fail-closed on missing mandatory fields | Omit one field at a time → `parse_response()` → assert `ContractViolation` with correct `field` |
| Tool rejection | `ok: false` maps to `ToolRejected` | `ToolResponse::error_with_code()` → `parse_response()` → assert error variant |
| Finding strictness | Malformed findings fail closed | `with_findings()` with bad data → `parse_response()` → assert `ContractViolation` |
| Legacy compat | Old verdict values still parse | `verdict: "safe_to_apply"` → assert `EditVerdict::SafeToApply` |
| Sub-tool composition | Optional sub-results parse correctly | Include `path_scope`, `newline_check`, etc. in response → assert output fields |
| Enum roundtrips | `parse()` ↔ `as_str()` consistency | `EditVerdict::parse(v.as_str()) == v` for all variants |
