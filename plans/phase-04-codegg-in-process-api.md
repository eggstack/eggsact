# Phase 4: codegg-Native In-Process API

## Goal

Expose eggsact as a typed in-process Rust library for codegg, while preserving MCP as a transport adapter. codegg should be able to call deterministic tools directly without starting a stdio MCP server and without parsing JSON from MCP text content.

The current public crate surface remains calculator-oriented. That should remain supported, but eggsact now needs a first-class agent utility API centered on registries, tool calls, typed preflight workflows, and stable response contracts.

## Scope

In scope:

- Add a public library API over the consolidated registry.
- Support generic JSON-based in-process tool calls.
- Add typed wrappers for the highest-value codegg workflows.
- Preserve CLI and MCP behavior.
- Expose profile-aware registry views.
- Provide stable errors and machine codes through the library API.

Out of scope:

- Full typed input/output structs for every low-priority tool.
- codegg repo changes.
- New edit/command/config feature semantics beyond thin typed wrappers around existing tools.
- Replacing MCP response wrapping.

## Design Direction

MCP should become one adapter over a core tool execution layer:

```text
codegg direct calls ─┐
CLI utility calls ───┼── eggsact registry/runtime/tool implementations
MCP stdio calls ─────┘
```

This means the core should expose an API like:

```rust
let registry = ToolRegistry::default();
let response = registry.call_json("text_equal", serde_json::json!({
    "a": "foo",
    "b": "foo",
}))?;
```

For common codegg workflows, provide typed convenience APIs:

```rust
let verdict = Preflight::edit(EditPreflightInput { ... })?;
let command = Preflight::command(CommandPreflightInput { ... })?;
let config = Preflight::config(ConfigPreflightInput { ... })?;
```

The first pass should prefer pragmatic wrappers around existing tool handlers rather than a full internal rewrite.

## Proposed Public Modules

Add public modules carefully. Avoid exposing unstable implementation details.

```rust
pub mod agent;
pub mod preflight;
```

`agent` can contain:

- `ToolRegistry`
- `ToolCallError`
- `ToolCallResult`
- `ToolView`
- `Profile`
- `ToolExposure`
- `ToolSpecView`

`preflight` can contain typed workflow inputs and outputs:

- `EditPreflightInput`
- `EditPreflightOutput`
- `CommandPreflightInput`
- `CommandPreflightOutput`
- `ConfigPreflightInput`
- `ConfigPreflightOutput`
- `PathScopeInput`
- `PathScopeOutput`
- `UnicodeIngressInput`
- `UnicodeIngressOutput`

Keep structs small initially. They can wrap `serde_json::Value` internally if needed, but public fields should be typed where stable.

## ToolRegistry API

A first-pass API could be:

```rust
pub struct ToolRegistry {
    profile: Option<Profile>,
}

impl ToolRegistry {
    pub fn default() -> Self;
    pub fn with_profile(profile: Profile) -> Self;
    pub fn available_tools(&self) -> Vec<ToolSpecView>;
    pub fn get_tool(&self, name: &str) -> Option<ToolSpecView>;
    pub fn call_json(&self, name: &str, args: serde_json::Value) -> Result<ToolResponse, ToolCallError>;
    pub fn call_json_value(&self, name: &str, args: serde_json::Value) -> serde_json::Value;
}
```

`ToolResponse` can initially reuse the MCP response struct if it is suitable. If the MCP struct is too transport-oriented, introduce an internal/core response type and adapt it into MCP later.

`ToolCallError` should represent failures before tool execution, such as unknown tool, unavailable in profile, invalid arguments, or internal registry errors. Tool-level failures should usually return `Ok(ToolResponse { ok: false, ... })`, matching MCP behavior.

## Profile API

Avoid making downstream code pass raw strings for known profiles. Add a typed profile enum while preserving string conversion:

```rust
pub enum Profile {
    Full,
    Default,
    CodeggCoreMin,
    CodeggCore,
    CodeggPreflight,
    CodeggPatch,
    CodeggConfig,
    CodeggUnicodeSecurity,
    CodeggShell,
    CodeggRepoAudit,
    HumanMath,
    Custom(String),
}
```

If `Custom` is too permissive, omit it initially. The important part is that codegg can select profiles without typo-prone string constants.

## Typed Preflight APIs

### Edit Preflight

The first version can wrap the existing `edit_preflight` tool and normalize the result.

Suggested input:

```rust
pub struct EditPreflightInput {
    pub path: Option<String>,
    pub original_text: String,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub patch: Option<String>,
    pub language: Option<String>,
    pub expected_fingerprint: Option<String>,
    pub line_ending_policy: LineEndingPolicy,
    pub unicode_policy: UnicodePolicy,
}
```

Suggested output:

```rust
pub struct EditPreflightOutput {
    pub ok: bool,
    pub verdict: String,
    pub machine_code: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub raw: serde_json::Value,
}
```

Do not require every field to be used by the existing tool on day one. The typed API can initially map only supported fields and preserve the rest for future phases.

### Command Preflight

Input:

```rust
pub struct CommandPreflightInput {
    pub command: String,
    pub cwd: Option<String>,
    pub policy: CommandPolicy,
}
```

Output:

```rust
pub struct CommandPreflightOutput {
    pub ok: bool,
    pub verdict: String,
    pub machine_code: String,
    pub risk_level: String,
    pub findings: Vec<Finding>,
    pub argv: Option<Vec<String>>,
    pub raw: serde_json::Value,
}
```

### Config Preflight

Input:

```rust
pub struct ConfigPreflightInput {
    pub text: String,
    pub format: ConfigFormat,
    pub schema: Option<serde_json::Value>,
    pub strict: bool,
}
```

Output:

```rust
pub struct ConfigPreflightOutput {
    pub ok: bool,
    pub valid: bool,
    pub detected_format: Option<String>,
    pub machine_code: String,
    pub findings: Vec<Finding>,
    pub raw: serde_json::Value,
}
```

## Implementation Sequence

### Step 1: Identify Stable Core Types

Decide whether `ToolResponse` remains in `mcp::response` or moves to a protocol-neutral module such as `agent::response` or `core::response`. The preferred direction is protocol-neutral ownership with MCP wrapping as an adapter.

Avoid a large breaking move if Phase 2 already established a clean `mcp::response`. Re-export types from a public `agent` module if needed.

### Step 2: Add ToolRegistry

Implement `ToolRegistry` over the Phase 1 registry. It should:

- Filter tools by profile.
- Validate tool availability.
- Validate arguments using the same schema validator as MCP.
- Invoke the same handler functions as MCP.
- Return the same `ToolResponse` as MCP tool execution.

This eliminates duplicate behavior between MCP and direct calls.

### Step 3: Refactor MCP `tools/call` to Use ToolRegistry

Once `ToolRegistry::call_json` exists, modify MCP `tools/call` to delegate to it. MCP should retain transport-specific error wrapping and content serialization, but tool lookup, profile enforcement, argument validation, and execution should be shared.

This is the most important integration point: there should not be separate MCP and direct execution paths with subtly different validation.

### Step 4: Add Typed Preflight Wrappers

Add typed wrappers for edit, command, and config preflight. These wrappers should call `ToolRegistry::call_json`, then parse stable fields from the response.

Use conservative parsing. If a result lacks an expected field, return a typed error or include the raw response with a clear `internal_error` machine code.

### Step 5: Add Examples

Add simple examples in `examples/` or doc tests:

- direct `ToolRegistry` call for `text_equal`
- direct `validate_json`
- edit preflight wrapper
- command preflight wrapper
- config preflight wrapper

These examples will guide codegg integration.

### Step 6: Document Public API Stability

Add documentation that distinguishes stable public API from internal MCP implementation modules. The public API should be intentionally small.

## Testing Plan

Run:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Add tests for:

- `ToolRegistry::default().available_tools()` returns expected tools.
- `ToolRegistry::with_profile(Profile::CodeggCoreMin)` filters correctly.
- Unknown tool returns `ToolCallError`.
- Tool outside profile is rejected.
- Invalid arguments are rejected the same way MCP rejects them.
- Representative direct calls match MCP tool call behavior for result payloads.
- Edit/command/config typed wrappers parse expected fixture outputs.

Where practical, add a test that calls a tool through both MCP dispatch and `ToolRegistry` and asserts equivalent `ToolResponse` before MCP wrapping.

## Compatibility Requirements

This phase must not break:

- `eggsact "expression"`
- `eggsact --mcp`
- existing MCP tool names and response shapes
- existing public `run` and `evaluate` exports

The new API should be additive. If internal modules need to move, provide re-exports or document that they were `doc(hidden)` internals.

## Risks

The main risk is exposing too much too early. Keep the public API narrow and stable. Do not expose internal registry structs directly if their fields are likely to change.

The second risk is duplicating execution paths. Ensure MCP calls delegate to the same registry call mechanism as the direct API.

The third risk is typed wrappers depending on unstable result payloads. Start with high-value wrappers and include raw response fields for forward compatibility.

## Acceptance Criteria

Phase 4 is complete when:

- codegg can call eggsact tools in-process through `ToolRegistry`.
- MCP `tools/call` uses the same underlying execution path.
- Typed wrappers exist for edit, command, and config preflight.
- Existing CLI and MCP behavior remains compatible.
- Public API docs and examples exist.
- Formatting, clippy, and tests pass.

## Handoff Notes

This phase should not try to type every tool. The highest value is giving codegg a stable direct execution path and typed wrappers for harness-critical workflows. Broader typed coverage can be incremental.
