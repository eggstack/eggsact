# Phase 2: MCP, Runtime, Validation, and Tool Module Split

## Goal

Split the current MCP implementation into smaller, responsibility-focused modules without changing observable behavior. Phase 1 centralizes tool declarations; Phase 2 should make the codebase maintainable enough for future codegg-specific feature work.

The current shape places too many responsibilities in `src/mcp/server.rs` and `src/mcp/tools.rs`. This phase should leave the external MCP protocol and current tool semantics intact while clarifying ownership boundaries.

## Scope

In scope:

- Split protocol handling from registry lookup, runtime controls, response wrapping, argument validation, and tool implementations.
- Move tool implementations into category modules.
- Keep `eggsact --mcp` behavior compatible.
- Preserve all tool names, schemas, response shapes, profile behavior, and limits unless a test documents a compatibility-preserving internal change.
- Improve module-level tests where split boundaries make that practical.

Out of scope:

- New tools.
- Typed codegg API.
- Response contract redesign.
- True concurrent MCP transport changes.
- Major cancellation semantics changes.

## Target Module Layout

A reasonable first target is:

```text
src/
  mcp/
    mod.rs
    protocol.rs
    runtime.rs
    registry.rs
    response.rs
    schema_validation.rs
    server.rs
  tools/
    mod.rs
    math.rs
    text.rs
    json.rs
    regex.rs
    path.rs
    shell.rs
    patch.rs
    config.rs
    unicode.rs
    identifier.rs
    markdown.rs
    list.rs
    version.rs
    cargo.rs
```

This is a target, not a rigid requirement. If moving from `src/mcp/tools.rs` to `src/tools/*` is too disruptive in one pass, use `src/mcp/tools/*` as an intermediate layout. The key objective is separating categories and removing protocol/runtime concerns from implementation files.

## Responsibility Boundaries

### `mcp/protocol.rs`

Own JSON-RPC and MCP request/response protocol concerns:

- `JsonRpcRequest`
- `JsonRpcResponse`
- `JsonRpcError`
- `InitializeResult`
- `Capabilities`
- `ServerInfo`
- protocol constants
- JSON-RPC error constructors
- top-level request validation helpers

If these types currently live in `schemas.rs`, either rename `schemas.rs` to `protocol.rs` or keep `schemas.rs` only for serializable protocol structs while adding protocol orchestration elsewhere.

### `mcp/response.rs`

Own tool response shape and sanitization:

- `ToolResponse`
- `sanitize_error`
- response builders
- warning/finding helpers
- MCP content wrapping
- output truncation helpers

This keeps machine-readable response handling in one place before Phase 3 formalizes machine codes.

### `mcp/runtime.rs`

Own runtime control and resource management:

- request rate limiter
- cancelled request tracking
- tool worker semaphore setup
- timeout wrappers
- spawn/blocking helper functions
- request/output byte limit constants
- request ID limits

This module should not know individual tool semantics beyond invoking a handler.

### `mcp/schema_validation.rs`

Own MCP argument validation against tool input schemas:

- `validate_arguments`
- `validate_property`
- `value_matches_type`
- numeric/string/object/array constraint checks
- schema validation tests

Keep the lightweight validator separate from actual `validate_schema_light` tool implementation. The MCP validator validates tool arguments; the tool implementation validates user-provided JSON against user-provided schema.

### `mcp/registry.rs`

Own registry types and derived lookups introduced in Phase 1:

- `ToolSpec`
- profile constants
- exposure/cost/stability enums
- registry lookup
- tool listing generation
- profile filtering
- tag/tier/name filtering

### `mcp/server.rs`

Become the orchestration layer:

- stdio read loop
- validated JSON-RPC request dispatch
- call into protocol/runtime/registry/schema modules
- write JSON lines
- preserve `pub async fn main() -> !`

`server.rs` should no longer contain large schema literals, large metadata tables, or individual tool implementation code.

### Tool Category Modules

Each tool category should own its implementation functions and local helpers. Examples:

- `tools/math.rs`: `math_eval`, `unit_convert`, `unit_info`, `constant_lookup`.
- `tools/text.rs`: measurement, equality, diff, count, truncate, fingerprint, hash, transform, window, replacement checks, line range helpers.
- `tools/json.rs`: JSON validation/extraction/canonicalization/comparison/shape.
- `tools/path.rs`: normalize, analyze, compare, scope check, glob.
- `tools/shell.rs`: split, quote join, argv compare, command preflight.
- `tools/patch.rs`: patch apply check, summary, edit preflight.
- `tools/config.rs`: TOML, dotenv, INI, config preflight, Cargo TOML if not split.
- `tools/unicode.rs`: Unicode policy and canonicalization.
- `tools/identifier.rs`: identifier analysis/inspection/table inspection.

Shared helper functions should move to narrow modules only when reused by multiple categories. Avoid creating a broad `util.rs` dumping ground.

## Implementation Sequence

### Step 1: Establish New Modules with Re-exports

Create the target modules and re-export old symbols to keep compilation stable. This allows moving code incrementally without breaking call sites on every step.

Example:

```rust
pub mod protocol;
pub mod registry;
pub mod response;
pub mod runtime;
pub mod schema_validation;
pub mod server;
```

Temporarily re-export compatibility aliases if tests import old paths.

### Step 2: Move Response and Sanitization

Move `ToolResponse`, `sanitize_error`, and response wrapping into `mcp/response.rs`. Keep serialization behavior identical. Add tests proving representative error sanitization still works.

### Step 3: Move Protocol Types and Error Constructors

Move JSON-RPC structs, initialize result structs, protocol constants, and error constructors into `mcp/protocol.rs`. Keep serialized field names unchanged.

### Step 4: Move Schema Validation

Move argument schema validation into `mcp/schema_validation.rs`. Preserve existing behavior for:

- unknown arguments
- missing required fields
- type mismatch
- enum mismatch
- min/max length
- min/max items
- object additional property handling
- regex pattern checking
- `multipleOf` tolerance behavior

Keep all existing bug regression tests near this module if possible.

### Step 5: Move Runtime Controls

Move rate limiter, cancelled request tracking, timeout constants, worker limit constants, and spawn/blocking helpers into `mcp/runtime.rs`.

Do not change the semantics of cancellation or timeout yet. Document current limitations, especially that timeout does not necessarily terminate already-running blocking work.

### Step 6: Move Tool Implementations by Category

Move tools category by category. After each category move, run focused tests if possible. Keep handler function names stable to minimize registry churn.

Suggested order:

1. Math tools.
2. JSON/config/TOML tools.
3. Path/glob tools.
4. Regex tools.
5. Text tools.
6. Shell tools.
7. Patch/composite tools.
8. Unicode/identifier/markdown/list/version/cargo tools.

Move local helpers with their primary users. If helpers become shared, give them a specific module name, for example `tools/text_positions.rs` rather than `misc.rs`.

### Step 7: Reduce `server.rs`

After moves, simplify `server.rs` to request-loop and dispatch orchestration. It should read as a protocol server, not as a registry and tool implementation module.

## Testing Plan

Run full checks after the split:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Add or preserve tests for:

- protocol initialization response
- invalid JSON-RPC request handling
- rate-limit behavior if already testable
- `tools/list` filtering
- `tools/call` representative calls from every category
- schema validator edge cases
- response sanitization
- timeout behavior as currently defined

For each moved category, make sure category-local tests still compile and do not depend on MCP stdio when pure function tests are possible.

## Compatibility Requirements

This phase should not change:

- CLI flags.
- MCP protocol version.
- server name.
- tool names.
- profile names.
- default active profile behavior.
- input or output schema field names.
- response wrapping shape.
- existing error strings unless tests are updated for a documented internal reason.

## Risks

The main risk is accidental visibility/API breakage. Some tests or downstream users may import internal modules despite `doc(hidden)`. Use re-exports temporarily where cheap.

The second risk is circular dependencies between registry, response, validation, and tool modules. Resolve by keeping registry dependent only on response and tool handler function types, while protocol/server depend on registry.

The third risk is creating too many tiny modules without clear ownership. Prefer category-level modules first, then split further only where there is real complexity.

## Acceptance Criteria

Phase 2 is complete when:

- `server.rs` is primarily protocol orchestration.
- Response handling, runtime control, schema validation, and registry lookup live in separate modules.
- Tool implementations are grouped by category.
- Existing behavior remains compatible.
- Full formatting, clippy, and tests pass.
- The module layout makes Phase 3 response-contract work and Phase 4 in-process API work straightforward.

## Handoff Notes

Avoid feature edits while moving code. Mechanical refactors should be reviewed with small diffs where possible. If a category move exposes duplicated helpers, prefer leaving duplication temporarily over introducing an over-broad abstraction during this phase.
