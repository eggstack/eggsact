# Milestone 5: Schema Validation Boundary Clarity

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Make the MCP argument schema validator’s supported subset explicit, documented, and enforced by tests. The validator is useful and intentionally lightweight, but it should not be mistaken for a complete JSON Schema implementation.

The target result is a clear contract: tool schemas may only use supported keywords unless explicitly exempted, unsupported JSON Schema features are documented as unsupported, and future contributors cannot accidentally add schemas that appear to enforce constraints the validator ignores.

## Rationale

`eggsact` performs internal argument validation against registered tool input schemas. The implementation supports a practical subset: types, enums, constants, string length and pattern checks, numeric bounds, object properties, required fields, additionalProperties, array item constraints, and uniqueness. That is sufficient for most MCP tool arguments.

However, JSON Schema is large. Constructs such as `$ref`, `oneOf`, `anyOf`, `allOf`, `not`, conditionals, `patternProperties`, `dependentSchemas`, `format`, and draft-specific features are not currently enforced. If a future tool schema includes one of those keywords, downstream callers may assume validation is stronger than it really is.

## Scope

In scope:

- Document the supported schema subset.
- Document unsupported schema constructs.
- Add invariant tests that every registered tool input schema uses only supported keywords.
- Add focused validator tests for supported keywords.
- Add negative tests demonstrating unsupported keyword handling policy.
- Update developer docs for adding new tools and schemas.

Out of scope:

- Implementing full JSON Schema.
- Adding `$ref` resolution.
- Replacing the validator with an external crate.
- Changing every schema shape unless tests reveal unsupported keywords are already present.
- Output-schema validation at runtime, unless the repo already has a specific pattern to extend.

## Files likely to change

- `src/mcp/schema_validation.rs`
- `tests/mcp/test_tool_coverage.rs`
- `tests/mcp/test_mcp_tools.rs`
- New test module such as `tests/mcp/test_schema_boundaries.rs`
- `architecture/mcp-server.md`
- New or existing developer doc for adding tools
- `README.md` if a short note is warranted

## Supported schema subset to document

Document the exact supported subset as implemented. At the time this plan was written, the validator supports:

- Top-level object schemas with `properties`.
- `required` fields.
- `additionalProperties` as a boolean.
- `type` as a string or array of strings.
- Primitive types: `string`, `number`, `integer`, `boolean`, `array`, `object`, and `null`.
- `const`.
- `enum`.
- String constraints:
  - `minLength`
  - `maxLength`
  - `pattern`
- Numeric constraints:
  - `minimum`
  - `maximum`
  - `exclusiveMinimum`
  - `exclusiveMaximum`
  - `multipleOf`
- Object constraints:
  - nested `properties`
  - nested `required`
  - nested `additionalProperties`
- Array constraints:
  - `minItems`
  - `maxItems`
  - `uniqueItems`
  - homogeneous `items`
- A fixed maximum nested validation depth.

Also document compatibility differences:

- `CompatibilityMode::EggcalcPython` changes selected error wording/type names.
- `CompatibilityMode::StrictNative` uses JSON-schema-like type names.
- JSON booleans are not accepted as numbers, even though Python historically treats `bool` as an `int` in some contexts.

## Unsupported constructs to document

Explicitly state that the validator does not enforce:

- `$schema`
- `$id`
- `$ref`
- `$defs` / `definitions`
- `oneOf`
- `anyOf`
- `allOf`
- `not`
- `if` / `then` / `else`
- `dependentRequired`
- `dependentSchemas`
- `patternProperties`
- `propertyNames`
- `contains`
- `prefixItems`
- tuple validation
- `minContains` / `maxContains`
- `format`
- `contentEncoding`
- `contentMediaType`
- `readOnly` / `writeOnly`
- unevaluated properties/items
- draft-specific annotation semantics

For `description`, `default`, `examples`, and similar annotation-only fields, decide whether they are allowed as documentation keywords. The invariant test should treat them as allowed annotations if they appear in schemas, but should make clear that they do not enforce runtime constraints.

## Implementation plan

### 1. Add supported-keyword constants

Create a central list of supported validation keywords and allowed annotation keywords. The test suite can use this list to inspect schemas.

Suggested grouping:

```rust
const SUPPORTED_SCHEMA_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "pattern",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "enum",
    "const",
];

const ALLOWED_ANNOTATION_KEYWORDS: &[&str] = &[
    "description",
    "default",
    "examples",
    "title",
];
```

Decide whether these constants belong in production code or tests. If placed in tests, keep them synchronized with docs. If placed in production code, mark as internal/doc-hidden if not intended as public API.

### 2. Add schema traversal invariant test

Write a recursive test that walks every registered input schema and fails on unsupported keywords.

Rules:

- Ignore property names under `properties`; those are argument names, not schema keywords.
- Recurse into schemas under `properties`, `items`, and type arrays where applicable.
- Permit annotation keywords if intentionally allowed.
- Fail on validation-looking unsupported keywords such as `oneOf`, `anyOf`, `allOf`, `$ref`, `patternProperties`, and `format`.
- Include the tool name and JSON path in failure messages.

This test should run in normal CI.

### 3. Add focused validator behavior tests

Add unit tests or integration tests for the supported subset:

- Type arrays accept either valid type.
- `const` accepts exact match and rejects mismatch.
- `enum` rejects unknown value.
- `pattern` uses search semantics, not mandatory full-string matching unless anchored.
- `multipleOf` uses current tolerance behavior.
- Nested object required fields are enforced.
- Nested object `additionalProperties: false` rejects unknown fields.
- Array `minItems` and `maxItems` are enforced.
- Array `uniqueItems` is enforced by serialized item representation.
- Homogeneous `items` schema is enforced.
- Maximum validation depth fails gracefully.

Some of these may already be covered. Consolidate or add only missing cases.

### 4. Decide unsupported-keyword policy

Recommended policy: registered tool schemas must not use unsupported validation keywords. The runtime validator can continue ignoring unknown keywords inside arbitrary schema input if that behavior is needed for compatibility, but the built-in registered schemas should be strict.

This means:

- The schema traversal invariant fails if built-in tool schemas contain unsupported validation keywords.
- Documentation says unsupported keywords are not available for tool schemas.
- Developers adding tools must stay within the subset or first extend the validator and tests.

### 5. Update developer docs for adding tools

Add a section to the tool-authoring documentation:

- Choose a profile and exposure deliberately.
- Add input and output schemas.
- Use only supported input-schema keywords.
- Run schema invariant tests.
- Add route-critical tests if the tool influences action selection.
- Run generated docs.

If no tool-authoring document exists, add a concise one under `architecture/` or `docs/`.

### 6. Update user-facing docs carefully

The README should not need a long schema explanation. It can link to the architecture/developer doc. Avoid overwhelming ordinary users with JSON Schema internals.

The architecture doc should include the full supported/unsupported lists.

## Testing requirements

Run targeted schema tests while iterating:

```bash
cargo test --all-features schema -- --nocapture
cargo test --all-features test_tool_coverage -- --nocapture
```

Run full verification before handoff:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Acceptance criteria

- Supported schema keywords are documented.
- Unsupported JSON Schema constructs are documented.
- Registered tool input schemas are checked against the supported subset.
- The invariant test reports clear tool name and schema path on failure.
- Focused tests cover the major supported keyword behaviors.
- Built-in schemas do not include unsupported validation keywords.
- Tool-authoring docs tell contributors how to add schemas safely.
- Docs do not imply full JSON Schema conformance.

## Review checklist

Before closing the milestone, verify:

- The schema traversal test does not mistake property names for schema keywords.
- Annotation-only keywords are either allowed deliberately or removed.
- Unsupported validation keywords fail tests if added to registered schemas.
- Error messages are clear enough for future contributors.
- Compatibility-mode behavior remains unchanged except where explicitly tested.
- Existing MCP clients are not affected by the documentation/invariant changes.

## Handoff notes

This milestone is a boundary-hardening pass, not a request to implement full JSON Schema. Keep the validator small. If future tool work genuinely requires `oneOf` or `$ref`, that should become its own implementation and compatibility milestone with dedicated tests.
