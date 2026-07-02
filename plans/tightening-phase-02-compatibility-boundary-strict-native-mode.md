# Tightening Phase 02: Compatibility Boundary and Strict Native Mode

## Objective

Separate compatibility behavior for eggcalc/Python parity from strict eggsact-native behavior. MCP can preserve compatibility where useful for migration and general clients, but codegg-facing in-process APIs should be strict, typed, deterministic, and explicit.

## Problem Statement

The current implementation intentionally preserves some Python-shaped behavior in the MCP layer and validation paths. That is useful for parity, but it can create ambiguity for codegg. A Rust harness should not inherit surprising coercions or compatibility wording unless it explicitly asks for that behavior.

The immediate design goal is not to remove compatibility. The goal is to localize it.

## Desired Behavior

MCP adapter behavior may remain compatibility-oriented when required.

In-process codegg APIs default to strict native validation.

Compatibility decisions are documented in one place.

Tests prove that strict mode and compatibility mode differ only in intended cases.

## Implementation Steps

### 1. Define a validation/runtime policy

Introduce a small policy type, for example:

```rust
pub enum CompatibilityMode {
    EggcalcPython,
    StrictNative,
}
```

The exact name is not important. The important part is that call sites can declare whether they want compatibility behavior or strict native behavior.

### 2. Keep compatibility at the MCP adapter boundary

MCP request validation can continue to preserve existing compatibility behavior where needed. Move any compatibility-specific coercion or wording into the MCP adapter layer where practical.

Avoid embedding compatibility assumptions deep in shared code unless the behavior is intentionally universal.

### 3. Make in-process calls strict by default

`ToolRegistry` and typed preflight wrappers should default to strict native behavior. They should reject ambiguous or incorrectly typed inputs instead of accepting them through compatibility coercion.

If compatibility is required for a direct call, expose an explicit constructor or call option.

### 4. Audit current compatibility quirks

Create a short internal document or architecture section listing known compatibility choices, including:

- Python-style JSON formatting if retained.
- Python-parity error text if retained.
- Boolean handling in numeric-like request fields if retained by MCP.
- Legacy `EGGCALC_*` environment variable names.
- Evaluator behaviors inherited from eggcalc.

This list should become the review checklist when future parity changes are proposed.

### 5. Add strict validation tests

Add tests that show strict native mode rejects ambiguous values that compatibility mode may accept. Keep these tests narrowly focused so they do not become a second full validation framework.

## Test Plan

Add unit tests for compatibility policy selection.

Add MCP tests confirming existing compatibility-sensitive behavior is preserved where intended.

Add in-process tests confirming strict native behavior rejects ambiguous argument types.

Add regression tests for any behavior that previously leaked from MCP compatibility into native wrappers.

## Acceptance Criteria

- There is an explicit compatibility/native policy boundary.
- MCP can retain compatibility behavior without forcing it onto codegg-native APIs.
- Typed preflight wrappers use strict native behavior by default.
- Compatibility choices are documented.
- Tests cover at least three compatibility-sensitive cases.

## Non-Goals

Do not remove eggcalc compatibility wholesale.

Do not rewrite the schema validator from scratch.

Do not change public behavior for ordinary full-profile MCP clients unless a behavior is clearly erroneous.

## Handoff Notes

Keep this phase small. It should introduce an explicit policy seam and move obvious compatibility decisions toward MCP-facing code. Avoid large refactors until phase 3 and phase 4 have tightened typed outputs and response contracts.
