# Machine Codes Reference

Machine-readable response codes for tool results. Every non-OK `ToolResponse` carries a `machine_code` field so that harnesses, orchestrators, and downstream tooling can route, classify, and act on results without parsing human-readable messages.

## Source of Truth

All constants live in `src/mcp/machine_codes.rs`. The `ToolResponse` struct and helper functions live in `src/mcp/response.rs`.

## ToolResponse Shape

Every tool call returns a `ToolResponse` (serialized as JSON) with these 11 fields:

| Field | Type | Always present | Description |
|-------|------|---------------|-------------|
| `ok` | bool | yes | `true` for success, `false` for error |
| `tool` | string | yes | Tool name that was invoked |
| `result` | object | when `ok=true` | Tool-specific result payload |
| `error_type` | string | when `ok=false` | Legacy error category (e.g. `evaluation_error`) |
| `error` | string | when `ok=false` | Sanitized human-readable error message |
| `hints` | string[] | when `ok=false` | Suggestions for the caller |
| `warnings` | string[] | optional | Non-fatal warnings |
| `limits_applied` | string[] | optional | Which input limits were enforced |
| `findings` | object[] | optional | Structured findings with codes, severity, messages |
| `machine_code` | string | when set | Machine-readable code from `machine_codes` module |
| `recommended_next_tool` | object | optional | Suggested next tool for the caller |

## Constructor Helpers

| Constructor | When to use |
|-------------|-------------|
| `ToolResponse::success(result, tool)` | Successful result (no machine code needed) |
| `ToolResponse::success_with_machine_code(result, tool, code)` | Success with an explicit code |
| `ToolResponse::error_without_code_for_legacy_tests_only(type, error, hints, tool)` | **Deprecated/hidden/test-only** — legacy error constructor (no machine code). Only available under `#[cfg(test)]`. Do not use in new code. |
| `ToolResponse::error_with_code(type, code, error, hints, tool)` | **Preferred** error constructor — ensures machine code is set |
| `.with_machine_code(code)` | Add a machine code to any response via builder |
| `.with_findings(findings)` | Attach structured findings |
| `.with_warnings(warnings)` | Attach warnings |
| `.with_limits_applied(limits)` | Record which limits were enforced |
| `.with_verdict(verdict)` | Set verdict inside result JSON (for composite tools) |
| `.with_recommended_next_tool(tool)` | Suggest the next tool to call (structured `{name, reason, arguments_hint}`) |
| `preflight_allow(tool)` | Quick preflight success — `ok=true`, `machine_code=OK`, `verdict=allow` |
| `preflight_review(tool, findings)` | Quick preflight with warnings — `ok=true`, verdict=`review`, findings attached |
| `preflight_block(tool, machine_code, findings)` | Quick preflight failure — `ok=true`, verdict=`block`, findings attached |
| `ToolResponse::next_tool(name, reason, arguments_hint)` | Static helper returning structured `recommended_next_tool` JSON |

## Finding Helpers

Structured findings are JSON objects with `code`, `severity`, `message`, and optional `disposition` fields. Three helpers are available in `src/mcp/response.rs`:

```rust
// Simple finding (disposition is optional)
finding(code, severity, message, details, disposition)

// Finding with source location (line/column)
finding_with_location(code, severity, message, line, column, disposition)

// Finding for prompt inspection (span instead of location)
prompt_finding(code, severity, message, byte_offset, end_byte_offset, details, disposition)
```

Use `severity::*` constants (`info`, `low`, `medium`, `high`, `critical`) and `disposition::*` constants (`informational`, `caution`, `blocking`) from `machine_codes.rs`.

## Severity / Disposition / Verdict Constants

These constants are defined in `src/mcp/machine_codes.rs` (and re-exported from `src/mcp/response.rs`):

**Severity** (`machine_codes::severity`):
`info` · `low` · `medium` · `high` · `critical`

**Disposition** (`machine_codes::disposition`):
`informational` · `caution` · `blocking`

**Verdict** (`machine_codes::verdict`):
`allow` · `review` · `block` · `valid` · `valid_with_warnings` · `invalid` · `safe_to_apply` · `safe_with_warnings`

## Composite Tool Verdicts

Composite tools (`edit_preflight`, `command_preflight`, `config_preflight`, `text_security_inspect`, `cargo_toml_inspect`) emit a `verdict` field in their `result` object via the `.with_verdict(verdict)` builder. Verdicts use the `verdict` constants above. Composite tools also emit a `machine_code` at the top level to summarize the overall outcome (e.g. `COMMAND_OK`, `SHELL_RISK`, `CONFIG_OK`, `TEXT_SECURITY_OK`).

All composite tools use `finding()` / `finding_with_location()` helpers with canonical `severity::*` and `disposition::*` constants for structured findings. Severity values map from legacy vocab: `"error"` → `severity::HIGH`, `"warn"` → `severity::MEDIUM`, `"info"` → `severity::INFO`.

A subset of these tools are further classified as **route-critical** (`is_route_critical()` in `registry/listing.rs`). Route-critical tools (`edit_preflight`, `command_preflight`, `config_preflight`, `patch_apply_check`, `text_security_inspect`) must always emit `machine_code` and `verdict` fields in their response envelope. Route-critical tests verify this contract.

### Finding-Code Enumeration

Every UPPERCASE_SNAKE finding `code` emitted by a route-critical tool must be present in `machine_codes::ALL`. This is enforced by `test_route_critical_finding_codes_are_enumerated` in `tests/mcp/test_route_contracts.rs`, which iterates over each tool's `findings[]` collection and asserts each `code` appears in the canonical table. When adding a new finding code:

1. Add the `pub const` to `src/mcp/machine_codes.rs` and append it to `ALL`.
2. Use the constant via `machine_codes::FOO` in tool code (never a raw string).
3. The route-contract test will fail otherwise.

This prevents stringly-typed finding codes from drifting out of the public contract.

## Category-Prefixed Aliases

Common error codes have category-prefixed aliases for use by orchestration layers (e.g. codegg) that prefer a uniform `CATEGORY_DETAIL` naming pattern. The Rust constant name differs but the string value is identical to the original, so they are wire-compatible.

### Common Error Aliases

| Alias Constant | Original Constant | String Value |
|----------------|-------------------|--------------|
| `COMMON_CANCELLED` | `CANCELLED` | `"CANCELLED"` |
| `COMMON_TIMEOUT` | `TIMEOUT` | `"TIMEOUT"` |
| `COMMON_OUTPUT_TOO_LARGE` | `OUTPUT_TOO_LARGE` | `"OUTPUT_TOO_LARGE"` |
| `COMMON_INPUT_TOO_LARGE` | `INPUT_TOO_LARGE` | `"INPUT_TOO_LARGE"` |
| `COMMON_INTERNAL_ERROR` | `INTERNAL_ERROR` | `"INTERNAL_ERROR"` |
| `COMMON_INVALID_ARGUMENTS` | `INVALID_ARGUMENTS` | `"INVALID_ARGUMENTS"` |

### Codegg Routing Aliases

Category-specific aliases for codegg routing. These share string values with their originals and are wire-compatible.

| Alias Constant | Original Constant | String Value |
|----------------|-------------------|--------------|
| `EDIT_SAFE_TO_APPLY` | `EDIT_OK` | `"EDIT_OK"` |
| `EDIT_OLD_TEXT_NOT_FOUND` | `AMBIGUOUS_REPLACEMENT` | `"AMBIGUOUS_REPLACEMENT"` |
| `EDIT_MULTIPLE_MATCHES` | `AMBIGUOUS_REPLACEMENT` | `"AMBIGUOUS_REPLACEMENT"` |
| `EDIT_STALE_CONTEXT` | `FINGERPRINT_MISMATCH` | `"FINGERPRINT_MISMATCH"` |
| `SHELL_SAFE_COMMAND` | `COMMAND_OK` | `"COMMAND_OK"` |
| `SHELL_DESTRUCTIVE_COMMAND` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_NETWORK_ACCESS` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_FILESYSTEM_WRITE` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_PROCESS_CONTROL` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_ENV_MUTATION` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_PRIVILEGE_ESCALATION` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_COMMAND_SUBSTITUTION` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_REDIRECTION` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_PIPELINE` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_BACKGROUND_EXECUTION` | `SHELL_RISK` | `"SHELL_RISK"` |
| `SHELL_UNAPPROVED_COMMAND` | `SHELL_RISK` | `"SHELL_RISK"` |
| `CONFIG_VALID` | `CONFIG_OK` | `"CONFIG_OK"` |
| `CONFIG_INVALID` | `CONFIG_PARSE_FAILED` | `"CONFIG_PARSE_FAILED"` |
| `UNICODE_BIDI_DETECTED` | `BIDI_DETECTED` | `"BIDI_DETECTED"` |
| `PATH_SCOPE_ESCAPE` | `PATH_HAS_TRAVERSAL` | `"PATH_HAS_TRAVERSAL"` |

These aliases are included in the `ALL` array and are interchangeable with their originals at the wire level.

## Full Code Table

### Common Error Codes

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `OK` | No findings or errors | info | no | proceed | all tools |
| `CANCELLED` | Request cancelled before execution | info | yes | abort | server |
| `TIMEOUT` | Tool exceeded timeout | high | yes | retry or skip | server |
| `OUTPUT_TOO_LARGE` | Output truncated at MAX_OUTPUT_BYTES | medium | no | truncate/summarize | all tools |
| `INPUT_TOO_LARGE` | Input exceeded size limit | medium | yes | reject input | all tools |
| `SERIALIZATION_ERROR` | Failed to serialize response | high | yes | report bug | server |
| `UNSUPPORTED_FEATURE` | Operation not supported | medium | yes | skip | any tool |
| `INTERNAL_ERROR` | Unexpected internal error | critical | yes | report bug | any tool |
| `INVALID_ARGUMENTS` | Arguments don't match schema | medium | yes | fix arguments | any tool |

### Edit / Patch

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `EDIT_OK` | Edit safe to apply | info | no | apply | `edit_preflight` |
| `EDIT_FAILED` | Edit could not be applied | high | yes | investigate | `edit_preflight` |
| `EDIT_MODE_INVALID` | Unknown or unsupported replacement_mode | high | yes | fix mode | `edit_preflight` |
| `EDIT_ARGUMENTS_MISSING` | Mode-specific required arguments are missing | high | yes | add missing args | `edit_preflight` |
| `EDIT_ARGUMENTS_INVALID` | One or more arguments present but invalid (wrong type, malformed value) | high | yes | fix arguments | `edit_preflight` |
| `EDIT_ARGUMENTS_CONFLICT` | Conflicting arguments provided for the current mode | high | yes | remove conflicts | `edit_preflight` |
| `EDIT_METADATA_TOO_LARGE` | A metadata field exceeded `MAX_METADATA_FIELD_LENGTH` (1000 chars) | high | yes | trim metadata | `edit_preflight` |
| `AMBIGUOUS_REPLACEMENT` | Multiple matches found | medium | yes | disambiguate | `edit_preflight`, `text_replace_check` |
| `PATCH_FAILED` | Patch parse/apply error | high | yes | fix patch | `patch_apply_check` |
| `LINE_RANGE_INVALID` | Line range out of bounds | medium | yes | fix range | `line_range_extract`, `line_range_compare` |
| `FINGERPRINT_MISMATCH` | Source changed since fingerprint | high | yes | re-fetch source | `edit_preflight` |
| `NEWLINE_INCONSISTENCY` | Newline style is inconsistent across the file (mixed CRLF/LF) | medium | no | review warnings | `edit_preflight` |

### Shell / Command

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `COMMAND_OK` | Command safe to execute | info | no | execute | `command_preflight` |
| `SHELL_RISK` | Command has risky features | medium | review | review before exec | `command_preflight` |
| `SHELL_PARSE_ERROR` | Shell command unparseable | high | yes | fix command | `shell_split`, `command_preflight` |
| `SHELL_POLICY_REVIEW` | Command requires review under current policy (e.g. `cargo build`, `cargo bench`, `npm run`) | medium | review | review | `command_preflight` |
| `REGEX_RISK` | Regex in command has safety issues | medium | review | review pattern | `command_preflight` |
| `SHELL_NETWORK_ACCESS` | Command accesses the network | medium | review | review network access | `command_preflight` |
| `SHELL_FILESYSTEM_WRITE` | Command writes to the filesystem | medium | review | review write ops | `command_preflight` |
| `SHELL_PROCESS_CONTROL` | Command controls processes (kill, etc.) | medium | review | review process ops | `command_preflight` |
| `SHELL_ENV_MUTATION` | Command mutates environment | low | no | note env change | `command_preflight` |
| `SHELL_PRIVILEGE_ESCALATION` | Command elevates privileges (sudo, etc.) | high | yes | block or escalate | `command_preflight` |
| `SHELL_COMMAND_SUBSTITUTION` | Command uses command substitution | info | no | note | `command_preflight` |
| `SHELL_REDIRECTION` | Command uses I/O redirection | info | no | note | `command_preflight` |
| `SHELL_PIPELINE` | Command uses pipes | info | no | note | `command_preflight` |
| `SHELL_BACKGROUND_EXECUTION` | Command runs in background (&, nohup) | info | no | note | `command_preflight` |
| `SHELL_UNAPPROVED_COMMAND` | Command not on the allow list (policy_config) | varies | varies | depends on policy | `command_preflight` |

### JSON

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `JSON_VALID` | JSON input is valid | info | no | proceed | `validate_json` |
| `JSON_INVALID` | JSON input is invalid | medium | yes | fix JSON | `validate_json` |

### Structured Data Compare

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `DATA_EQUAL` | Structures are equal | info | no | proceed | `structured_data_compare` |
| `DATA_DIFF` | Structures differ | low | no | review diffs | `structured_data_compare`, `json_compare` |
| `INVALID_INPUT` | One or both inputs invalid | medium | yes | fix inputs | `structured_data_compare`, `json_compare` |

### Path

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `PATH_WITHIN_SCOPE` | Path is within scope | info | no | proceed | `path_scope_check` |
| `PATH_HAS_TRAVERSAL` | Path escapes scope | high | yes | reject path | `path_scope_check` |
| `PATH_IS_HIDDEN` | Path is hidden file/dir | low | no | note | `path_analyze` |

### Config

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `CONFIG_OK` | Config valid | info | no | proceed | `config_preflight` |
| `CONFIG_PARSE_FAILED` | Config parse error | high | yes | fix config | `config_preflight`, `dotenv_validate`, `ini_validate` |
| `CONFIG_SCHEMA_MISMATCH` | Config violates schema | medium | yes | fix config | `config_preflight` |
| `CONFIG_HAS_WARNINGS` | Config valid with warnings | low | no | review warnings | `config_preflight` |

### Identifier / Naming

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `IDENT_COLLISIONS` | Naming collisions detected | medium | review | rename | `identifier_inspect`, `identifier_table_inspect` |
| `IDENT_INVALID` | Invalid identifier | medium | yes | rename | `identifier_analyze` |
| `RESERVED_KEYWORDS` | Reserved keyword used | medium | review | rename | `identifier_table_inspect` |
| `IDENT_WARNING` | Mixed naming styles | low | no | note | `identifier_analyze`, `identifier_table_inspect` |

### Text / Prompt Inspection

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `PROMPT_HIDDEN_CONTENT` | Hidden/suspicious content | medium | review | investigate | `prompt_input_inspect` |
| `PROMPT_HAS_FLAGS` | Suspicious flags/sequences | medium | review | investigate | `prompt_input_inspect` |
| `PROMPT_INJECTION_RISK` | Possible injection attempt | high | yes | reject/escalate | `prompt_input_inspect` |
| `IDENTIFIER_COLLISION_RISK` | Identifier collision in prompt | low | no | note | `prompt_input_inspect` |

### Unicode / Safety

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `INVISIBLES_DETECTED` | Invisible characters present | low | no | note | `text_inspect`, `text_security_inspect` |
| `BIDI_DETECTED` | Bidi control characters present | medium | review | investigate | `text_inspect`, `text_security_inspect` |
| `CONFUSABLES_DETECTED` | Confusable characters present | low | no | note | `text_inspect`, `identifier_inspect` |
| `UNICODE_RISK` | Unicode policy violation | high | yes | fix text | `unicode_policy_check` |
| `NORMALIZATION_DIFF` | Normalization changed text | low | no | note | `text_transform`, `canonicalize_text` |
| `TEXT_SECURITY_OK` | Security inspection passed | info | no | proceed | `text_security_inspect` |

### Regex

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `REGEX_SAFE` | Pattern is safe | info | no | proceed | `regex_safety_check` |
| `REGEX_UNSAFE` | Pattern has safety issues | medium | review | fix pattern | `regex_safety_check` |

### Version / Cargo

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `CONSTRAINT_NOTE` | Version satisfies constraint | info | no | proceed | `version_constraint_check` |
| `CONSTRAINT_NOT_SATISFIED` | Version violates constraint | medium | yes | fix version | `version_constraint_check` |
| `CARGO_OK` | Cargo.toml parsed ok | info | no | proceed | `cargo_toml_inspect` |
| `CARGO_PARSE_FAILED` | Cargo.toml parse failed | high | yes | fix Cargo.toml | `cargo_toml_inspect` |
| `CARGO_HAS_FINDINGS` | Cargo.toml has findings | low | no | review findings | `cargo_toml_inspect` |

### Dependency Manifests

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `DEPENDENCY_UNKNOWN_ECOSYSTEM` | Ecosystem value (`cargo`/`npm`/`pip`/...) is not recognized | high | yes | specify valid ecosystem | `dependency_edit_preflight` |
| `DEPENDENCY_ADDED` | A dependency was added | low | no | review new dep | `dependency_edit_preflight` |
| `DEPENDENCY_REMOVED` | A dependency was removed | low | no | review removal | `dependency_edit_preflight` |

### TOML

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `TOML_VALID` | TOML input is valid | info | no | proceed | `validate_toml`, `toml_shape` |
| `TOML_INVALID` | TOML input is invalid | medium | yes | fix TOML | `validate_toml` |

### Text Comparison / Transform

| Code | Meaning | Severity | Blocking | Harness Action | Used by |
|------|---------|----------|----------|----------------|---------|
| `TEXT_EQUAL` | Texts are equal | info | no | proceed | `text_equal` |
| `TEXT_NOT_EQUAL` | Texts are not equal | low | no | review diffs | `text_equal` |

## Forward-Looking: Dotted Taxonomy

The UPPERCASE codes above are the current wire format, chosen for parity with the Python `eggcalc` server. A future evolution may adopt a dotted taxonomy (e.g. `edit.safe_to_apply`, `shell.risk`, `config.valid`) for finer-grained categorization. That design is not yet implemented; the current UPPERCASE constants are the active contract.
