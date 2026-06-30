# Phase 3: Stable Response Contracts and Machine Codes

## Goal

Make eggsact responses reliable as machine-consumable decision inputs for codegg. Human-readable strings should remain useful, but codegg should not have to route on prose. Every error, warning, finding, and composite verdict should have stable, documented machine-readable fields.

The current `ToolResponse` shape already has the right direction: `ok`, `tool`, `result`, `error_type`, `error`, `hints`, `warnings`, `limits_applied`, `findings`, `machine_code`, and `recommended_next_tool`. Phase 3 should make the optional machine-readable fields systematic.

## Scope

In scope:

- Define a stable machine-code taxonomy.
- Require `machine_code` for every non-OK response.
- Add structured finding/warning/verdict types.
- Update common response constructors.
- Normalize composite tool response structure.
- Document machine codes and test them.

Out of scope:

- Large semantic rewrites of tools.
- New codegg in-process APIs.
- Changing MCP transport shape unless necessary to include structured data in the existing text JSON payload.
- Replacing all result payloads with typed Rust structs. That is Phase 4.

## Design Requirements

A codegg caller should be able to answer these questions without parsing prose:

- Did the tool succeed?
- If not, what stable error category occurred?
- Is the result safe to act on?
- Did the tool produce warnings?
- Are warnings informational, cautionary, or blocking?
- Is there a source location or input span associated with a finding?
- What should the harness or model do next?

This phase should preserve the existing MCP wrapping model, but the JSON payload embedded in MCP content should become more disciplined.

## Proposed Response Types

Introduce stable helper structs in `mcp/response.rs` or an adjacent module:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FindingDisposition {
    Informational,
    Caution,
    Blocking,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub byte_offset: Option<usize>,
    pub end_byte_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub kind: String,
    pub machine_code: String,
    pub severity: Severity,
    pub disposition: FindingDisposition,
    pub message: String,
    pub location: Option<Location>,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NextAction {
    pub action: String,
    pub reason: String,
    pub tool: Option<String>,
    pub arguments: Option<serde_json::Value>,
}
```

Avoid over-engineering the first pass. The important part is a stable shape, not a perfect ontology.

## Machine Code Taxonomy

Use dotted machine codes. Top-level domains should be stable; subcodes can grow over time.

Common codes:

- `ok`
- `common.invalid_arguments`
- `common.input_too_large`
- `common.output_too_large`
- `common.timeout`
- `common.cancelled`
- `common.serialization_error`
- `common.unsupported_feature`
- `common.internal_error`
- `common.limit_applied`

Parse and validation codes:

- `parse.json_error`
- `parse.toml_error`
- `parse.regex_error`
- `parse.patch_error`
- `validation.failed`
- `validation.schema_violation`
- `validation.type_mismatch`
- `validation.missing_required`
- `validation.unknown_argument`
- `validation.enum_mismatch`

Text and Unicode codes:

- `text.not_equal`
- `text.first_difference_found`
- `text.normalization_changed`
- `text.truncated`
- `unicode.invisible_detected`
- `unicode.bidi_control_detected`
- `unicode.mixed_script_detected`
- `unicode.confusable_detected`
- `unicode.policy_violation`

Edit and patch codes:

- `edit.safe_to_apply`
- `edit.safe_with_warnings`
- `edit.old_text_not_found`
- `edit.multiple_matches`
- `edit.ambiguous_match`
- `edit.stale_context`
- `edit.line_ending_changed`
- `edit.unicode_risk`
- `edit.scope_escape`
- `edit.patch_parse_error`
- `edit.needs_human_review`

Shell codes:

- `shell.safe_command`
- `shell.needs_confirmation`
- `shell.destructive_command`
- `shell.network_access`
- `shell.package_install`
- `shell.privilege_escalation`
- `shell.credential_exposure_risk`
- `shell.background_process`
- `shell.long_running_process`
- `shell.injection_risk`

Path codes:

- `path.within_scope`
- `path.scope_escape`
- `path.traversal_detected`
- `path.normalized_changed`
- `path.platform_ambiguous`

JSON/config codes:

- `json.valid`
- `json.invalid`
- `json.equal`
- `json.not_equal`
- `json.duplicate_key`
- `config.valid`
- `config.invalid`
- `config.format_unknown`
- `config.schema_failed`

Math/unit codes:

- `math.evaluation_error`
- `math.conversion_error`
- `math.unknown_unit`
- `math.unknown_constant`

This taxonomy should be stored in documentation and preferably in constants/enums for critical codes.

## Implementation Sequence

### Step 1: Add Machine Code Constants

Create a `machine_codes` module or constants section. Avoid string literals scattered across tool implementations. The first pass can use `pub const` strings rather than full enums to reduce friction.

Example:

```rust
pub mod machine_codes {
    pub const INVALID_ARGUMENTS: &str = "common.invalid_arguments";
    pub const INPUT_TOO_LARGE: &str = "common.input_too_large";
    pub const TIMEOUT: &str = "common.timeout";
}
```

### Step 2: Update Response Constructors

Change `ToolResponse::error` so callers must provide a machine code, or add a new constructor and migrate tools gradually:

```rust
pub fn error_with_code(
    error_type: &str,
    machine_code: &str,
    error: &str,
    hints: Option<Vec<String>>,
    tool: Option<&str>,
) -> Self
```

Keep the old constructor temporarily only if needed, but mark it internal/deprecated and add a test that all externally returned non-OK tool responses have `machine_code`.

### Step 3: Normalize Warnings and Findings

Today warnings are a vector of strings and findings are untyped JSON values. Preserve serialization compatibility where necessary, but introduce structured forms for new and migrated tools.

A pragmatic migration path:

- Keep `warnings: Option<Vec<String>>` for backward compatibility.
- Add `structured_warnings: Option<Vec<Finding>>` or use `findings` consistently.
- For migrated tools, put structured findings in `findings` with stable fields.
- Do not remove simple warnings until downstream code no longer relies on them.

### Step 4: Migrate Common Error Paths

Start with MCP-level and helper-generated tool errors:

- invalid arguments
- input too large
- output too large
- timeout
- serialization error
- cancelled
- unknown/unsupported where represented as tool responses

Then migrate category tools in priority order:

1. edit/patch/config/shell/path/unicode tools, because codegg will route on these.
2. JSON/TOML/regex validation tools.
3. text comparison and transform tools.
4. math/unit tools.
5. lower-priority list/markdown/identifier/reporting tools.

### Step 5: Composite Tool Verdicts

Composite tools should emit a top-level `verdict` field inside `result`, plus structured child summaries.

Suggested shape:

```json
{
  "verdict": "safe_with_warnings",
  "machine_code": "edit.safe_with_warnings",
  "summary": "Replacement is unique but changes line endings.",
  "findings": [...],
  "child_results": [
    {"tool": "text_replace_check", "ok": true, "machine_code": "edit.safe_with_warnings"},
    {"tool": "unicode_policy_check", "ok": true, "machine_code": "ok"}
  ]
}
```

The exact result fields can vary by tool, but `verdict`, `machine_code`, `summary`, and `findings` should be consistent for composite preflight tools.

### Step 6: Document Machine Codes

Add or generate a machine-code reference in `architecture/` or `plans/` initially, then later move it into generated docs. Include:

- code
- meaning
- severity default
- whether it is blocking
- recommended harness action
- relevant tools

### Step 7: Add Compatibility Tests

Add tests that enforce:

- Every non-OK `ToolResponse` has `machine_code`.
- Every structured finding has `kind`, `machine_code`, `severity`, and `message`.
- Composite tools emit top-level verdict and machine code.
- Critical codegg preflight tools use expected machine codes for representative fixtures.

## Testing Plan

Run:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Add targeted fixtures for:

- invalid arguments on representative tools
- oversized text input
- timeout path if deterministic test exists
- JSON parse error
- TOML parse error
- regex parse/safety finding
- path scope escape
- text replacement no-match and multiple-match
- command preflight risky command
- Unicode hidden character finding

## Compatibility Requirements

Avoid breaking existing MCP clients. The simplest path is additive: include `machine_code` more consistently and add structured fields without removing old fields.

If the old `error_type` values are used by tests or downstream code, preserve them. Treat `error_type` as the coarse legacy category and `machine_code` as the stable precise code.

## Risks

The main risk is overfitting a taxonomy too early. Mitigate by keeping top-level domains stable and allowing new dotted subcodes.

The second risk is inconsistent migration. Mitigate with tests that assert non-OK responses always have `machine_code`.

The third risk is bloating responses. Use compact detail modes where needed and keep structured findings concise.

## Acceptance Criteria

Phase 3 is complete when:

- Every non-OK tool response has `machine_code`.
- Common warnings/findings have structured machine-readable fields.
- Composite preflight tools emit top-level verdicts and child summaries.
- Machine codes are documented.
- Critical codegg-relevant tools have fixture tests for expected codes.
- Existing MCP behavior remains compatible.
- Formatting, clippy, and tests pass.

## Handoff Notes

Do not try to make all successful result payloads perfectly typed in this phase. The main deliverable is reliable machine routing for errors, warnings, and preflight verdicts. Phase 4 will add the typed in-process API.
