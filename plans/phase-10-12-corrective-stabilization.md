# Corrective Stabilization Plan for Phase 10–12 Work

## Purpose

This plan corrects the current post-phase-12-plan implementation state. Since the phase 12 handoff plans landed, the repo has advanced substantially: phase 06–09 release-polish documentation, phase 10 evaluator/context isolation, and phase 11 diagnostics/verification reporting have all partially landed. The direction is good, but the implementation now has several correctness and documentation-truthfulness gaps that should be fixed before any additional phase work continues.

This is a stabilization pass. It should not add new major capabilities. It should reconcile the architecture that landed with the contracts and documentation that now describe it.

## Current State Summary

### Good Progress

- `EvalContext` exists and carries per-evaluation PRNG state, Gaussian spare state, memory registers, user variables, and random/side-effect permission flags.
- `evaluate_with_context` exists and uses a context-aware parser/evaluator path.
- `ExecutionContext` exists and carries `EvalContext`, compatibility mode, profile, audience, budget, cancellation, request ID, and source.
- `call_json_with_execution_context` exists.
- `runtime_diagnostics` exists as a harness-only diagnostic MCP tool.
- `verify-eggsact` exists as a local verification-report binary.
- Diagnostics tests cover model-audience exclusion, harness visibility, env-var-name-only behavior, timeout/cancelled distinction, and profile/audience diagnostic messages.
- Context-isolation tests cover profile, audience, compatibility mode, cancellation, budget, parallel simple calls, PRNG, and memory-register isolation.

### Main Problems

1. `ExecutionContext` is ahead of dispatch behavior.

   `call_json_with_execution_context` currently uses only `ctx.budget` and `ctx.cancellation`. It still delegates to `self.call_json(...)`, so dispatch continues to use the registry's stored profile, audience, and compatibility mode. It does not use `ctx.profile`, `ctx.audience`, or `ctx.compatibility_mode`. It also does not pass `ctx.eval_ctx` into `math_eval` or calculator-backed tool execution.

2. `EvalContext` documentation overstates global-state replacement.

   Legacy globals still exist for the old `evaluate()` path: random allow flags, side-effect allow flags, memory registers, user variables, PRNG state, and Gaussian spare state. This may be acceptable for backward compatibility, but docs/comments must say the context-aware path replaces globals for context-aware calls, not that globals are gone.

3. Verification documentation is contradictory.

   The implementation summary records failing `cargo test --test lib` and failing parity tests, while also implying broader gates passed. It also includes a statement that CI runs `cargo test --all-features` and fails on main, while elsewhere implying the normal gate is acceptable. This must be reconciled using a current verification run.

4. Stale parity/tool-gap claims remain.

   The implementation summary says `config_file_inspect`, `dependency_edit_preflight`, and `repo_manifest_inspect` are not yet ported to Rust and are planned for phase 10. That is stale or misleading given the current Rust repo contains implementations/tests around config and dependency inspection. The docs should describe actual gaps, not stale historical gaps.

5. Cancellation/context tests are not strong enough in some areas.

   Some tests are useful smoke tests but allow broad outcomes. The repo needs at least one deterministic assertion per intended context behavior, especially where `ExecutionContext` claims to carry profile/audience/compatibility/eval state.

## Non-Goals

- Do not add new MCP tools.
- Do not implement Windows command parsing.
- Do not redesign the full MCP server concurrency model.
- Do not remove legacy `evaluate()`/`run()` APIs.
- Do not remove global evaluator state until there is a deliberate compatibility/deprecation plan.
- Do not expand diagnostics beyond fixing the current correctness/documentation issues.

## Workstream A: Correct `ExecutionContext` Dispatch Semantics

### Problem

`ExecutionContext` carries profile, audience, and compatibility mode, but `call_json_with_execution_context` does not honor them. This creates a dangerous false sense of isolation.

### Required Decision

Choose one of two implementation paths.

#### Preferred Path: Context Overrides Dispatch

Make `call_json_with_execution_context` construct an effective registry from the context before preparing the call:

```rust
let effective_profile = ctx.profile.clone().unwrap_or_else(|| self.profile.clone());
let effective_audience = ctx.audience.unwrap_or(self.audience);
let effective_compat = ctx.compatibility_mode;
let effective_registry = ToolRegistry::with_profile_and_audience(effective_profile, effective_audience)
    .with_compat_mode(effective_compat);
```

Then use `effective_registry.prepare_tool_call(...)` and dispatch through the prepared handler.

This makes the method name truthful: the passed context controls the dispatch-scoped state.

#### Acceptable Alternative: Rename/Scope Method Honestly

If using context profile/audience/compatibility is considered too invasive for now, rename or document the method as budget/cancellation-only and remove or de-emphasize unused fields. This is less desirable because the phase 10 goal is explicit execution context.

### Required Work

1. Update `call_json_with_execution_context` to honor profile, audience, and compatibility mode from `ExecutionContext`.

2. Ensure pre-execution checks use the effective registry:

   - tool lookup;
   - profile membership;
   - audience/exposure permission;
   - schema validation using context compatibility mode.

3. Preserve existing wrappers:

   - `call_json`;
   - `call_json_with_budget`;
   - `call_json_with_context`.

4. Decide precedence precisely:

   - if `ctx.profile` is `Some`, it overrides registry profile;
   - if `ctx.audience` is `Some`, it overrides registry audience;
   - `ctx.compatibility_mode` always controls validation for the context-aware call;
   - `ctx.budget` overrides resolved budget;
   - `ctx.cancellation` supplies cooperative cancellation.

5. Document the precedence in Rustdoc and `architecture/mcp-server.md`.

### Required Tests

1. A registry with `Profile::Full` plus an `ExecutionContext` with `Profile::HumanMath` rejects a full-only tool.

2. A registry with `ToolAudience::Harness` plus context `ToolAudience::Model` rejects a harness-only tool.

3. A registry with `ToolAudience::Model` plus context `ToolAudience::Harness` allows a harness-only tool when profile permits it.

4. A context compatibility mode test must cause different schema-validation wording or behavior where modes are expected to differ.

5. Existing `call_json` and `call_json_with_budget` behavior must remain unchanged.

### Acceptance Criteria

- `ExecutionContext` dispatch fields are actually honored.
- Tests fail if `call_json_with_execution_context` silently falls back to registry profile/audience/compatibility.
- Rustdoc no longer overclaims.

## Workstream B: Wire Calculator Context Into Calculator-Backed Tools

### Problem

`ExecutionContext` contains `EvalContext`, and `evaluate_with_context` exists, but `math_eval` and other calculator-backed tool paths may still call legacy global-state APIs. If so, phase 10 evaluator isolation is not effective for agent/MCP tools.

### Required Work

1. Identify all calculator-backed tool calls.

   Search:

   ```bash
   rg "calc::run|calc::evaluate|run\(|evaluate\(" src/tools src/mcp src/agent
   rg "math_eval" src/tools src/mcp tests
   ```

2. Add a context-aware internal helper for math evaluation.

   Candidate:

   ```rust
   pub fn math_eval_with_eval_context(args: &Value, eval_ctx: &mut EvalContext) -> ToolResponse
   ```

3. Use this helper from `call_json_with_execution_context` for `math_eval` if the generic `ToolHandler` signature cannot carry mutable context.

   A simple dispatch special-case is acceptable for phase 10 stabilization:

   ```rust
   if name == "math_eval" {
       let mut eval_ctx = ctx.eval_ctx.clone_or_new_somehow();
       return math_eval_with_eval_context(&args, &mut eval_ctx);
   }
   ```

   If mutable context persistence across calls is required, redesign carefully. Do not accidentally clone away state when caller expects mutation to persist.

4. Decide whether `ExecutionContext.eval_ctx` should be mutable in the dispatch call.

   Current method signature uses `&ExecutionContext`, which prevents mutating `ctx.eval_ctx` during evaluation. Options:

   - keep `&ExecutionContext` and treat `eval_ctx` as a seed/template cloned for a single call;
   - add `call_json_with_execution_context_mut(..., &mut ExecutionContext)` for tools that need persistent per-context state;
   - make `eval_ctx` internally synchronized/interior mutable.

   Preferred: add a clearly named mutable-context variant if persistent memory/PRNG state across calls is required.

5. Do not change default `math_eval` behavior until tests prove compatibility.

### Required Tests

1. Context-aware `math_eval` with `EvalContext::mcp_mode()` rejects `random()` and side-effect functions.

2. Context-aware `math_eval` with `EvalContext::new()` permits `random()`.

3. Context-aware `math_eval` with a seeded PRNG produces deterministic output.

4. If mutable context persistence is supported, repeated calls through the same context advance PRNG and preserve memory registers.

5. If context is clone-per-call only, docs must state that context state is used as an input seed and not mutated through `&ExecutionContext` dispatch.

### Acceptance Criteria

- Calculator-backed tool execution has a truthful context-aware path.
- Docs state whether evaluator context is persistent or per-call seeded.
- Legacy `math_eval` remains backward compatible.

## Workstream C: Correct EvalContext and Legacy Global Documentation

### Problem

Current comments imply that `EvalContext` replaces global statics, but legacy global statics remain and are used by legacy APIs.

### Required Work

1. Update `src/calc/context.rs` comments.

   Replace broad wording like:

   > Replaces the global statics...

   With precise wording:

   > Provides an explicit state container for context-aware evaluation. Legacy `evaluate()`/`run()` APIs still use process-global compatibility state; `evaluate_with_context()`/`run_with_context()` use `EvalContext`.

2. Update `src/calc/evaluator.rs` comments near legacy globals.

   Mark them as compatibility globals for legacy APIs.

3. Update architecture docs.

   Add a section:

   - legacy evaluator path;
   - context-aware evaluator path;
   - MCP-safe context behavior;
   - migration goal.

4. Update `AGENTS.md` gotchas if needed.

### Acceptance Criteria

- Docs are accurate about remaining global state.
- No file implies global state is fully removed when it is not.
- Future maintainers can tell which API path is isolated.

## Workstream D: Reconcile Verification and Release-Polish Documentation

### Problem

`plans/phase-06-09-release-polish-implementation.md` contains contradictory verification statements and stale tool-gap statements.

### Required Work

1. Run the current verification command:

   ```bash
   cargo run --bin verify-eggsact -- --report markdown
   ```

2. Also run the canonical gates manually if needed:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   cargo run --bin generate-docs -- --check
   cargo package --verbose
   ```

3. Replace contradictory verification text in `plans/phase-06-09-release-polish-implementation.md` with current, internally consistent status.

4. If tests fail, classify failures against current code, not historical buckets.

5. Remove or correct stale statements that config/dependency/repo tools are unported if they are now implemented.

6. If parity still has known failures, document them in `docs/parity.md` with current categories.

7. Do not claim CI passed unless CI evidence is present.

### Acceptance Criteria

- Verification docs do not contradict themselves.
- Stale tool-gap claims are removed or corrected.
- Current failing tests, if any, are precisely categorized.
- `verify-eggsact` output is treated as the source of truth for local report status.

## Workstream E: Stabilize Diagnostics Scope

### Problem

Diagnostics are mostly safe, but they should be checked against the new context architecture and documentation.

### Required Work

1. Confirm `runtime_diagnostics` reports only names/booleans/counts and no sensitive values.

2. Confirm `runtime_diagnostics` remains `HarnessOnly` and is not visible to Model audience.

3. Confirm generated docs/tool cards represent the diagnostic tool correctly.

4. Decide whether diagnostic output should include context-aware dispatch status:

   - whether `ExecutionContext` dispatch honors profile/audience/compatibility;
   - whether calculator context is persistent or per-call seed;
   - whether legacy globals remain for compatibility.

   Keep this concise.

5. Add or adjust tests if diagnostic output changes.

### Acceptance Criteria

- Diagnostics remain safe and deterministic.
- Diagnostics do not overclaim context isolation.
- Harness-only exposure remains enforced.

## Workstream F: Strengthen Context-Isolation Tests

### Problem

Some context-isolation tests are too permissive to catch broken semantics.

### Required Work

1. Replace permissive cancellation/context tests with deterministic assertions where possible.

2. Add tests specifically for `ExecutionContext` dispatch override behavior:

   - context profile override;
   - context audience override;
   - context compatibility override;
   - budget override;
   - cancellation override.

3. Add tests for calculator context integration if Workstream B changes `math_eval`.

4. Keep smoke tests that ensure cancelled calls do not poison later calls, but do not treat them as proof of enforcement.

### Acceptance Criteria

- Tests fail if context fields are ignored.
- Tests distinguish smoke tests from semantic enforcement tests.
- Context isolation is proven for the fields the API claims to carry.

## Workstream G: Generated Docs and Release Gates

### Required Work

1. Run generated-doc check:

   ```bash
   cargo run --bin generate-docs -- --check
   ```

2. If generated docs changed due to diagnostics/context wording, regenerate and review.

3. Confirm `release.sh` uses canonical gates:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   cargo run --bin generate-docs -- --check
   cargo package --verbose
   ```

4. Confirm `verify-eggsact` command list matches docs or intentionally differs and documents why.

### Acceptance Criteria

- Generated docs are current.
- Release script, AGENTS, skills, and verification binary agree on canonical gates.
- Any optional parity step is clearly environment-dependent.

## Recommended Implementation Order

1. Fix `ExecutionContext` dispatch semantics or scope/rename it honestly.
2. Decide and implement calculator context integration for `math_eval`.
3. Correct EvalContext/global-state documentation.
4. Strengthen context-isolation tests around actual dispatch semantics.
5. Reconcile release-polish implementation summary and parity docs using current verification output.
6. Review diagnostics output and generated docs.
7. Run full verification and record exact status.

## Final Acceptance Criteria

This corrective pass is complete when:

- `call_json_with_execution_context` honors the context fields it advertises, or the API/docs are narrowed to match actual behavior;
- calculator-backed context isolation is implemented or explicitly documented as not yet wired into tool dispatch;
- legacy evaluator globals are documented as compatibility state, not claimed removed;
- verification docs are internally consistent and current;
- stale parity/tool-gap statements are removed or corrected;
- diagnostics remain harness-only and safe;
- tests prove context dispatch semantics instead of only smoke-testing them;
- generated docs are fresh;
- full verification output is recorded with no unsupported claims.

## Non-Regression Requirements

- Existing public APIs must remain available unless explicitly deprecated.
- Default CLI behavior must remain compatible.
- MCP `tools/call` profile/audience enforcement must not regress.
- Route-critical machine-code/verdict contracts must not regress.
- `runtime_diagnostics` must not become model-visible.
- No environment variable values or secrets may appear in diagnostics.
