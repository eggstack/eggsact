# Tightening Phase 01: MCP Profile and Audience Enforcement

## Objective

Make profile and audience selection apply consistently to both tool listing and tool execution. This is the first tightening pass because codegg depends on curated tool surfaces. A restricted profile should mean the same thing whether a caller asks for `tools/list` or `tools/call`.

## Current Issue

The MCP `tools/list` path resolves an effective profile and passes that profile into registry listing. The `tools/call` path should use the same active profile when preparing a tool call. The in-process registry also stores an audience value, but dispatch should explicitly check that the selected audience may execute the requested tool exposure level.

## Desired Behavior

Profile checks happen before handler execution.

Audience checks happen before handler execution.

A model-audience registry can execute only tools that are suitable for model-facing use in the selected profile.

A harness-audience registry can execute harness-oriented tools in the selected profile.

Internal-only tools remain outside ordinary model and harness execution paths.

## Implementation Steps

### 1. Add dispatch-time exposure checking

Update `ToolRegistry::prepare_tool_call` so its order is:

1. Resolve tool spec and handler by name.
2. Check profile membership.
3. Check audience/exposure compatibility.
4. Validate arguments.
5. Return the handler.

Add a distinct error variant for exposure mismatch, such as `ToolNotAllowedForAudience`, with fields for tool, profile, audience, and exposure.

### 2. Add an audience helper

Add a small helper that answers whether a `ToolExposure` is executable for a `ToolAudience`.

Recommended rules:

- Model audience rejects harness-only and internal-only exposure.
- Harness audience accepts harness-oriented exposure but rejects internal-only exposure.
- Debug audience remains developer-oriented and should not be used by codegg model sessions.

### 3. Make MCP `tools/call` profile-aware

Replace any default full-profile registry construction in the MCP call path with a registry based on `get_active_profile()`.

If per-call profile override is added, validate it exactly like `tools/list`. If not, use the active runtime profile only.

MCP `tools/call` should use model audience by default. codegg harness checks should prefer the in-process API with harness audience.

### 4. Update MCP error mapping

Map profile mismatch and audience mismatch to invalid params errors with actionable text. The error should tell the caller the tool name, selected profile, selected audience, and why the call is not executable.

### 5. Update docs

Update architecture docs so they state that profile and audience affect both listing and execution. Document that ordinary MCP calls use model audience, while codegg harness checks should use the in-process harness audience.

## Test Plan

Add unit tests for the exposure helper.

Add `ToolRegistry` tests proving that restricted profiles reject out-of-profile tools and model audience rejects harness-oriented tools.

Add MCP request tests proving that `tools/list` and `tools/call` agree under a restricted active profile.

Add a regression test that fails if `tools/call` uses a full-profile registry while a restricted profile is active.

## Acceptance Criteria

- `tools/call` honors the active MCP profile.
- `ToolRegistry::prepare_tool_call` enforces both profile and audience.
- Model-facing calls cannot execute harness-only tools.
- Harness calls can execute harness-oriented tools intentionally.
- Tests cover list/call consistency.
- Architecture docs match the implemented behavior.

## Non-Goals

Do not redesign the profile set in this phase.

Do not add new tools.

Do not change default full-profile behavior for existing unconstrained MCP clients.

## Handoff Notes

Start in `src/agent/mod.rs`, then update `src/mcp/server.rs`. Keep behavior changes small and well-tested. This phase should be a targeted correctness patch, not a broader registry redesign.
