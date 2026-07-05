# Phase 10 Follow-Up Corrective Pass: ExecutionContext Semantics

## Purpose

This plan closes the remaining technical gap after the phase 10–12 corrective stabilization work. The latest pass improved release and compatibility documentation, but it did not resolve the core implementation mismatch: `ExecutionContext` advertises profile, audience, compatibility, budget, cancellation, and evaluator state, while `call_json_with_execution_context` currently only applies budget and cancellation and then delegates to registry-scoped `call_json`.

This follow-up pass should make the API truthful and testable. Either implement the full context semantics or deliberately narrow the API/docs. Preferred outcome: implement the full context semantics.

## Current Problem Statement

### Actual Behavior

`ToolRegistry::call_json_with_execution_context(name, args, ctx)` currently:

1. resolves a budget from `ctx.budget` or the tool's default budget;
2. checks serialized argument size;
3. installs `ctx.cancellation` as the thread-local cancellation flag;
4. calls `self.call_json(name, args)`;
5. truncates the response.

Because it delegates to `self.call_json`, the following values are not used for dispatch:

- `ctx.profile`;
- `ctx.audience`;
- `ctx.compatibility_mode`;
- `ctx.eval_ctx`.

### Documented/Implied Behavior

`ExecutionContext` documentation says it unifies profile, audience, budget, cancellation, and compatibility mode into one dispatch-scoped parameter. It also contains `EvalContext`, implying calculator-backed tools can execute against per-call evaluator state.

### Risk

Downstream codegg or other in-process callers may believe they are selecting a profile/audience/compatibility/evaluator context per call when they are not. This is worse than having no context API because it can silently bypass expected restrictions or fail to enforce the caller's intended mode.

## Required Product Decision

Choose one of these paths before implementation. The preferred path is Path A.

### Path A — Full Context-Aware Dispatch

Make `call_json_with_execution_context` honor all dispatch fields in `ExecutionContext`:

- profile;
- audience;
- compatibility mode;
- budget;
- cancellation;
- evaluator context for calculator-backed tools where feasible.

### Path B — Narrow API Semantics

Rename or document `call_json_with_execution_context` as budget/cancellation-only, de-emphasize or remove unused fields, and introduce a future plan for full context dispatch.

Path B should only be used if Path A cannot be completed safely in this pass.

## Workstream A: Implement Effective Dispatch Context

### Required Work

1. Define a helper that resolves the effective dispatch registry.

Suggested helper:

```rust
impl ToolRegistry {
    fn with_effective_context(&self, ctx: &ExecutionContext) -> ToolRegistry {
        let profile = ctx.profile.clone().unwrap_or_else(|| self.profile.clone());
        let audience = ctx.audience.unwrap_or(self.audience);
        ToolRegistry::with_profile_and_audience(profile, audience)
            .with_compat_mode(ctx.compatibility_mode)
    }
}
```

If `ToolAudience` is not `Copy`, clone it or adjust the enum accordingly.

2. Update `call_json_with_execution_context` to use the effective registry for all pre-execution checks.

The call should use:

```rust
let effective_registry = self.with_effective_context(ctx);
match effective_registry.prepare_tool_call(name, &args) { ... }
```

3. Keep budget resolution independent and explicit.

- `ctx.budget` overrides default budget.
- default budget still resolves from the target tool spec.

4. Keep cancellation behavior.

- `ctx.cancellation` is installed with `budget::with_cancel_flag(...)`.

5. Ensure response truncation still runs after handler execution.

6. Preserve behavior of existing wrappers:

- `call_json`;
- `call_json_with_budget`;
- `call_json_with_context`.

7. Update Rustdoc to state exact precedence:

- `ctx.profile` overrides registry profile when present;
- `ctx.audience` overrides registry audience when present;
- `ctx.compatibility_mode` is always used for schema validation in this method;
- `ctx.budget` overrides resolved budget;
- `ctx.cancellation` supplies cooperative cancellation;
- `ctx.eval_ctx` applies only to context-aware calculator-backed special paths, if implemented.

### Acceptance Criteria

- A context profile override can make a full registry reject a full-only tool.
- A context audience override can make a harness registry reject a harness-only tool under model audience.
- A context audience override can make a model registry allow a harness-only tool under harness audience, if the profile permits it.
- A context compatibility override is observable in schema validation behavior or error wording.
- Existing non-context dispatch APIs remain unchanged.

## Workstream B: Decide and Implement Calculator Context Integration

### Problem

`ExecutionContext` owns `eval_ctx`, but generic `ToolHandler` has the signature `fn(&Value) -> ToolResponse`. That handler cannot receive mutable evaluator state through the current registry path.

### Required Decision

Choose one of two explicit semantics.

#### Option B1 — Per-Call EvalContext Seed

`ctx.eval_ctx` is treated as a template/seed. `call_json_with_execution_context` clones or reconstructs evaluator state for that one call. Changes from memory/store/random advancement do not persist back to the caller.

This is easiest with an immutable `&ExecutionContext`, but it must be documented.

#### Option B2 — Mutable EvalContext Persistence

Introduce:

```rust
pub fn call_json_with_execution_context_mut(
    &self,
    name: &str,
    args: Value,
    ctx: &mut ExecutionContext,
) -> Result<ToolResponse, ToolCallError>
```

This method can pass `&mut ctx.eval_ctx` to calculator-backed tools and preserve PRNG/memory state across calls.

Preferred: implement B2 if practical. Otherwise implement B1 honestly and add a phase-10 follow-up note for persistence.

### Required Work

1. Identify calculator-backed tools.

At minimum:

- `math_eval`.

Search for any others:

```bash
rg "calc::run|calc::evaluate|run_with_context|evaluate_with_context|math_eval" src/tools src/mcp src/agent
```

2. Add a context-aware helper in the math tool module.

Suggested shape:

```rust
pub fn math_eval_with_context(args: &Value, eval_ctx: &mut EvalContext) -> ToolResponse
```

3. In context-aware dispatch, special-case `math_eval` to call the context-aware helper.

This is acceptable because the generic `ToolHandler` signature cannot carry evaluator state.

4. Ensure the helper reuses existing response shape exactly.

Do not change the `math_eval` result envelope unless a test proves the current envelope is wrong.

5. If B2 is implemented, add mutable dispatch wrapper and update docs.

6. If B1 is implemented, document that `eval_ctx` is copied/seeded per call and is not persisted through `&ExecutionContext`.

### Required Tests

1. `ExecutionContext::mcp_default(...).eval_ctx` rejects `random()` through context-aware `math_eval`.

2. `ExecutionContext::library_default().eval_ctx` allows `random()` through context-aware `math_eval`.

3. Seeded PRNG produces deterministic output through context-aware `math_eval`.

4. If mutable dispatch is implemented, repeated calls through the same mutable context advance PRNG state.

5. If mutable dispatch is not implemented, a test/doc explicitly proves per-call seed semantics.

### Acceptance Criteria

- The presence of `eval_ctx` in `ExecutionContext` is no longer misleading.
- Context-aware `math_eval` behavior is covered by tests.
- Legacy `math_eval` behavior through `call_json` remains unchanged.

## Workstream C: Strengthen ExecutionContext Tests

### Required Tests

Add tests in `tests/test_context_isolation.rs` or a new focused module.

1. `execution_context_profile_override_restricts_tool`

- Registry: `ToolRegistry::with_profile(Profile::Full)`.
- Context: `ExecutionContext::test_default()` with `profile = Some(Profile::HumanMath)`.
- Tool: a full-only non-human-math tool such as `text_equal`.
- Expected: `ToolUnavailable` for `human_math`.

2. `execution_context_audience_override_rejects_harness_only`

- Registry: Full + Harness.
- Context: audience `Model`.
- Tool: `runtime_diagnostics` or another HarnessOnly full-profile tool.
- Expected: `ToolNotAllowedForAudience`.

3. `execution_context_audience_override_allows_harness_only`

- Registry: Full + Model.
- Context: audience `Harness`.
- Tool: `runtime_diagnostics`.
- Expected: success.

4. `execution_context_compatibility_mode_controls_validation`

Find an argument-validation case where EggcalcPython and StrictNative differ in error wording. Assert that the context mode changes the observed validation message.

If no current behavior differs, document that compatibility mode is currently reserved and remove or narrow the field claim.

5. `execution_context_budget_override_rejects_large_input`

Keep existing budget test, but assert `machine_code == INPUT_TOO_LARGE` rather than only checking the error string.

6. `execution_context_cancellation_override_reaches_cooperative_handler`

Use a handler that deterministically checks cancellation early, such as `command_preflight`, and assert `machine_code == CANCELLED`.

7. `execution_context_does_not_poison_later_calls`

Keep smoke coverage that a cancelled/tiny-budget call does not affect later normal calls.

### Acceptance Criteria

- Tests fail if `ctx.profile`, `ctx.audience`, `ctx.compatibility_mode`, `ctx.budget`, or `ctx.cancellation` are ignored.
- Weak permissive assertions are replaced where deterministic behavior is intended.

## Workstream D: Align Documentation With Actual Semantics

### Required Work

1. Update Rustdoc on `ExecutionContext`.

It must not claim to replace global state unless the relevant path actually does so.

2. Update Rustdoc on `call_json_with_execution_context`.

Document:

- exact field precedence;
- whether `eval_ctx` is used;
- whether evaluator state is persistent or per-call seeded;
- that legacy wrappers remain registry-scoped.

3. Update architecture docs.

Files likely impacted:

- `architecture/overview.md`;
- `architecture/calculator.md`;
- `architecture/mcp-server.md`;
- `docs/library-api.md`.

4. Update `AGENTS.md` gotchas.

Add a short warning:

- context-aware dispatch is the preferred path for per-call overrides;
- legacy `call_json` uses registry-scoped profile/audience/compat mode;
- legacy calculator APIs still use compatibility globals.

5. Update diagnostics docs only if output changes.

### Acceptance Criteria

- Docs match code behavior exactly.
- No docs imply `ctx.profile`/`ctx.audience` are honored unless they are.
- No docs imply `EvalContext` persistence unless it exists.

## Workstream E: Verification and Generated Docs

### Required Work

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
cargo run --bin verify-eggsact -- --report markdown
```

If `cargo test --all-features` still fails, record exact failure categories. Do not claim release readiness.

If generated docs drift, run:

```bash
cargo run --bin generate-docs
```

Then review generated diffs.

### Acceptance Criteria

- Verification status is current and internally consistent.
- Generated docs are fresh.
- Any remaining failures are classified against current code, not stale phase history.

## Recommended Implementation Order

1. Implement effective dispatch context for profile/audience/compatibility.
2. Strengthen tests for profile/audience/compat/budget/cancellation context overrides.
3. Decide and implement calculator `EvalContext` semantics for `math_eval`.
4. Add calculator-context tests.
5. Update Rustdoc and architecture docs to match final semantics.
6. Run generated-doc check and full verification.
7. Record current verification results.

## Final Acceptance Criteria

This pass is complete when:

- `call_json_with_execution_context` honors context dispatch fields or has been honestly narrowed/renamed;
- `ExecutionContext.eval_ctx` has implemented and documented semantics for calculator-backed tools;
- tests prove each advertised context field is meaningful;
- legacy APIs remain backward compatible;
- docs no longer overclaim context isolation;
- generated docs are current;
- verification output is recorded without unsupported release-ready claims.

## Non-Regression Requirements

- MCP `tools/call` active-profile behavior must not regress.
- Model audience must still reject HarnessOnly and Hidden tools.
- Harness audience must still reject Hidden tools.
- `runtime_diagnostics` must remain HarnessOnly.
- Route-critical response contracts must retain `machine_code` and `verdict` where required.
- Legacy `run()` and `evaluate()` behavior must remain compatible unless explicitly documented and tested.
