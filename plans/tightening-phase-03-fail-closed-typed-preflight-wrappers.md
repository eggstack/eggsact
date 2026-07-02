# Tightening Phase 03: Fail-Closed Typed Preflight Wrappers

## Objective

Make the typed preflight APIs reliable enough for codegg to call as enforcement gates. The wrappers should not silently accept malformed tool outputs, missing fields, or schema drift. They should fail closed with explicit contract errors.

## Problem Statement

The existing preflight module provides useful typed input and output structs, but the wrappers still build JSON manually, call `ToolRegistry::call_json`, and extract result fields with permissive defaults. That is convenient during early development, but it hides contract drift.

For codegg, a missing machine code, missing verdict, malformed finding, or absent required field is not a harmless parse issue. It means the deterministic safety substrate is no longer trustworthy for that workflow.

## Desired Behavior

Typed wrappers deserialize mandatory fields strictly.

Missing or malformed mandatory fields return a typed contract error.

Tool-level rejections remain distinguishable from wrapper contract failures.

Outputs expose typed enums where practical, while preserving raw JSON for diagnostics.

## Implementation Steps

### 1. Define preflight error taxonomy

Add a preflight-level error enum, for example:

```rust
pub enum PreflightError {
    ToolCall(ToolCallError),
    ToolRejected { machine_code: Option<String>, error_type: Option<String>, message: String },
    ContractViolation { tool: &'static str, field: &'static str, message: String },
}
```

The exact shape can differ, but callers must be able to distinguish these cases:

- The registry rejected the call before execution.
- The tool executed and returned a non-OK result.
- The tool returned a shape that violated the typed contract.

### 2. Add strict output structs

For each typed preflight wrapper, define internal serde-deserializable result structs for mandatory fields.

Start with:

- `EditPreflightResultContract`
- `CommandPreflightResultContract`
- `ConfigPreflightResultContract`

Mandatory fields should be mandatory. Do not default `machine_code`, verdict, or critical booleans silently.

### 3. Introduce typed enums for stable route fields

Where practical, replace string-only public fields with enums plus string fallback if needed.

Candidate enums:

- `EditVerdict`
- `CommandVerdict`
- `ConfigVerdict`
- `FindingSeverity`
- `FindingDisposition`

If compatibility concerns make enums too strict initially, use `KnownOrOther<T>` style wrappers so codegg can handle new values deliberately.

### 4. Preserve raw diagnostics without relying on them

Keep a `raw: Value` or `raw_response: ToolResponse` field for diagnostics and forward compatibility. The typed wrapper should not use raw JSON defaults as a substitute for required fields.

### 5. Update wrapper return types

Change typed wrapper methods from returning `Result<Output, ToolCallError>` to returning `Result<Output, PreflightError>` or an equivalent wrapper-specific error.

Use `From<ToolCallError>` for ergonomic propagation.

### 6. Add contract fixtures

Add test fixtures or direct test helpers that simulate malformed tool responses:

- Missing `machine_code`.
- Missing verdict field.
- Finding without code.
- Finding without severity.
- `recommended_next_tool` with unexpected shape.
- Result object missing entirely despite `ok=true`.

Each should fail closed.

## Test Plan

Add success-path tests for edit, command, and config preflight using real tool calls.

Add malformed-contract tests using helper functions that parse artificial `ToolResponse` values into typed outputs.

Add tool-rejection tests proving non-OK responses map to `PreflightError::ToolRejected` or the chosen equivalent.

Add regression tests proving no mandatory route field falls back to an empty string.

## Acceptance Criteria

- Typed preflight wrappers no longer silently default missing route-critical fields.
- codegg can distinguish call errors, tool rejections, and contract violations.
- Edit, command, and config wrappers use strict contract parsing.
- Malformed output fixtures fail closed.
- Raw JSON remains available only for diagnostics and forward compatibility.

## Non-Goals

Do not convert every eggsact tool to typed wrappers in this phase.

Do not redesign the underlying tool result schemas beyond what is needed for strict parsing.

Do not remove the raw JSON `ToolRegistry::call_json` API; it remains useful for general harnesses and tests.

## Handoff Notes

Start with helper parsing functions so tests can exercise contract parsing without mocking the whole registry. Then update one wrapper at a time: config first, command second, edit third. Edit has the richest result and should be last.
