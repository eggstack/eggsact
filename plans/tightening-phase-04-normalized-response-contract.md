# Tightening Phase 04: Normalized Response Contract

## Objective

Normalize eggsact response semantics so codegg and other harnesses can route on stable fields. The response envelope should clearly distinguish handler execution success from workflow safety. Composite and preflight tools should consistently return verdicts, findings, machine codes, and next-action hints.

## Problem Statement

`ToolResponse` already has useful fields: `ok`, `tool`, `result`, `error_type`, `error`, `hints`, `warnings`, `limits_applied`, `findings`, `machine_code`, and `recommended_next_tool`. The issue is semantic consistency. Some tools encode problems as `ok=false`; others execute successfully but include findings and machine codes; some result objects carry route fields inside `result`; some response envelope fields duplicate result fields.

For an agent harness, this is expensive. Callers need a uniform way to decide whether to proceed, ask for review, block an operation, or call a follow-up tool.

## Desired Contract

`ok` means the tool handler executed and returned a well-formed tool-level response. It does not necessarily mean the proposed edit, command, config, or text is safe.

`verdict` or an equivalent domain-specific route field tells the caller what to do.

`machine_code` is stable and suitable for programmatic routing.

`findings` use a consistent shape.

`recommended_next_tool` is structured and machine-readable.

Composite/preflight tools expose top-level route fields in a predictable place.

## Implementation Steps

### 1. Define canonical route vocabulary

Add or document shared verdicts and dispositions.

Base verdicts:

- `allow`
- `review`
- `block`

Domain verdicts may remain where useful:

- Edit: `safe_to_apply`, `safe_with_warnings`, `blocked`, `stale_context`, `patch_failed`, `line_range_invalid`.
- Config: `valid`, `valid_with_warnings`, `invalid`.
- Command: `allow`, `allow_with_confirmation`, `review`, `block`.

Findings should include:

- `code`
- `severity`
- `message`
- optional `disposition`
- optional `location`
- optional `span`
- optional `details`

Severity should use a shared vocabulary, ideally `info`, `low`, `medium`, `high`, `critical`, or the closest existing convention. Avoid mixing `warn`, `warning`, and `medium` without a normalization rule.

### 2. Normalize `recommended_next_tool`

Move toward a structured object:

```json
{
  "name": "text_diff_explain",
  "reason": "literal replacement was ambiguous",
  "arguments_hint": {}
}
```

Keep compatibility with existing string-shaped values where needed, but new codegg-native wrappers should parse the structured shape.

### 3. Add response builder helpers

Add helper constructors for common preflight outcomes, for example:

- `ToolResponse::preflight_allow(...)`
- `ToolResponse::preflight_review(...)`
- `ToolResponse::preflight_block(...)`

Or add lower-level helpers that attach verdict, machine code, findings, and next tool consistently.

The goal is not many constructors for every tool. The goal is to stop hand-assembling subtly different response objects.

### 4. Update composite/preflight tools first

Prioritize tools that codegg will route on:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `text_security_inspect`
- `patch_apply_check`
- `cargo_toml_inspect`

Ensure each has a clear top-level verdict or domain route field and machine code.

### 5. Add machine-code coverage tests

Add tests that enforce:

- Every `ok=false` response path uses `machine_code`.
- Every composite/preflight response includes a routeable machine code.
- Every documented machine code is either used or explicitly marked reserved.
- Findings have at least code, severity, and message.

### 6. Update docs and typed wrappers

Update machine-code and MCP architecture docs to describe the normalized contract.

Update phase 3 typed wrappers to consume the normalized fields once available.

## Test Plan

Add unit tests for response builder helpers.

Add tool-specific tests for edit, command, and config preflight route fields.

Add fixture tests for findings shape.

Add machine-code coverage tests.

Add backward-compatibility tests where old fields remain intentionally present.

## Acceptance Criteria

- Composite/preflight tools expose consistent top-level route fields.
- Findings use a common shape.
- `recommended_next_tool` is structured for new outputs.
- Non-OK responses always include machine codes.
- codegg wrappers can route without inspecting arbitrary nested raw JSON.
- Docs describe the semantics clearly.

## Non-Goals

Do not force every simple utility tool to have a complex verdict if the tool only returns deterministic data.

Do not remove existing compatibility fields abruptly.

Do not change all machine-code names unless necessary. Prefer additive normalization over churn.

## Handoff Notes

Start with the response helpers and tests. Then update `edit_preflight`, because it already has route concepts and recommended next tool behavior. After that, update command/config preflight and security/composite tools.
