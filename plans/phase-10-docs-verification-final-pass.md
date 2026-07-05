# Final Documentation and Verification Pass for Phase 10 Context Work

## Purpose

This plan closes the remaining non-architectural gaps after the execution-context corrective implementation. The core technical issue is now substantially addressed: `call_json_with_execution_context` applies context profile, audience, compatibility mode, budget, cancellation, and threads `EvalContext` into `math_eval` through the handler bridge. The remaining work is documentation precision and verification evidence.

This pass should not add new features. It should make the documentation exactly match current behavior, record current verification status, and prevent future agents from misreading the context/evaluator semantics.

## Current State

### Strong Areas

- `call_json_with_execution_context` now resolves effective profile from `ctx.profile` or the registry fallback.
- `call_json_with_execution_context` now resolves effective audience from `ctx.audience` or the registry fallback.
- `call_json_with_execution_context` now validates schemas with `ctx.compatibility_mode`.
- `ctx.budget` controls input-size rejection and output truncation.
- `ctx.cancellation` is installed through the thread-local cancellation bridge.
- `ctx.eval_ctx` is installed through the thread-local eval-context bridge for handler execution.
- `math_eval` now uses `run_with_context` when a current eval context is present.
- Tests now cover profile override, audience override, compatibility override, budget override, cancellation override, MCP-mode eval context rejection of random functions, and seeded eval-context PRNG behavior.

### Remaining Gaps

1. `EvalContext` documentation still says it replaces global statics. That is imprecise because legacy `run()`/`evaluate()` paths still use compatibility globals. `EvalContext` replaces globals only for context-aware evaluation paths.

2. `ExecutionContext.eval_ctx` is cloned during dispatch. It behaves as a per-call seed/template, not as a persistent mutable context across repeated `call_json_with_execution_context(..., &ctx)` calls. Docs need to state this explicitly.

3. There is no mutable persistent context-dispatch API yet. That is acceptable, but it must not be implied.

4. Current verification status has not been recorded after the latest context implementation.

5. Generated docs may need regeneration after doc wording changes.

## Non-Goals

- Do not add new MCP tools.
- Do not add `call_json_with_execution_context_mut` in this pass.
- Do not redesign the generic `ToolHandler` signature.
- Do not remove legacy evaluator globals.
- Do not change runtime semantics except to fix documentation-proven mismatch.
- Do not claim release readiness without current verification evidence.

## Workstream A: Correct `EvalContext` Documentation

### Problem

`src/calc/context.rs` currently describes `EvalContext` as replacing global statics. That is too broad. Legacy evaluator APIs still use process-global compatibility state.

### Required Work

Update `src/calc/context.rs` Rustdoc to say:

- `EvalContext` is explicit per-evaluation state for context-aware APIs.
- `evaluate_with_context()` and `run_with_context()` use `EvalContext`.
- `ToolRegistry::call_json_with_execution_context()` installs an `EvalContext` for context-aware `math_eval` dispatch.
- Legacy `evaluate()` and `run()` remain backward-compatible and may use legacy process-global compatibility state.
- The context-aware API is the preferred path for deterministic agent calls.

Suggested wording:

```rust
/// Per-evaluation mutable state for context-aware calculator evaluation.
///
/// `EvalContext` provides explicit PRNG, Gaussian spare, memory-register,
/// user-variable, and function-permission state for `evaluate_with_context()`
/// and `run_with_context()`. Legacy `evaluate()` and `run()` remain
/// backward-compatible and use the legacy process-global compatibility state.
///
/// When used through `ToolRegistry::call_json_with_execution_context()`, the
/// context is currently cloned as a per-call seed/template for `math_eval`; state
/// mutations do not persist back into the caller's `ExecutionContext`.
```

### Acceptance Criteria

- No Rustdoc says `EvalContext` fully replaces global state.
- The distinction between legacy and context-aware APIs is explicit.
- Per-call seed/template behavior is documented.

## Workstream B: Clarify `ExecutionContext` Eval Semantics

### Problem

`ExecutionContext` carries `eval_ctx`, but dispatch currently clones it into a local mutable value before installing it in the thread-local bridge. This means evaluator mutations during a tool call do not persist back to the caller's context.

### Required Work

Update Rustdoc in `src/agent/mod.rs` for:

- `ExecutionContext`;
- `ExecutionContext::with_eval_context`;
- `ToolRegistry::call_json_with_execution_context`.

The docs must state:

- `profile`, `audience`, `compatibility_mode`, `budget`, and `cancellation` are active dispatch controls.
- `eval_ctx` is used by calculator-backed tools that opt into the eval-context bridge, currently `math_eval`.
- `eval_ctx` is cloned at dispatch and is therefore a per-call seed/template in the immutable-context API.
- PRNG/memory/variable mutations inside `math_eval` do not persist back to the caller's `ExecutionContext`.
- Future persistent state would require a mutable context API or a different handler signature.

### Acceptance Criteria

- A user can read docs and correctly predict that repeated immutable context calls with the same seed produce the same first random value.
- No docs imply persistent mutable state through `&ExecutionContext`.
- Docs identify `math_eval` as the current eval-context-aware tool.

## Workstream C: Update Architecture Docs

### Required Files

Review and update:

- `architecture/overview.md`;
- `architecture/calculator.md`;
- `architecture/mcp-server.md`;
- `docs/library-api.md`;
- `AGENTS.md`;
- `.skills/mcp-tools.md` and `.skills/testing.md` if relevant.

### Required Content

1. Add a short context-isolation model:

   - legacy calculator path;
   - context-aware calculator path;
   - context-aware in-process tool dispatch;
   - per-call seed/template eval context behavior;
   - cooperative cancellation semantics.

2. Add a short warning for agents:

   - Use `call_json_with_execution_context` for per-call profile/audience/compat/budget/cancellation overrides.
   - Use `evaluate_with_context`/`run_with_context` for persistent mutable `EvalContext` behavior across multiple calculator calls.
   - Do not assume `call_json_with_execution_context(&ctx)` mutates `ctx.eval_ctx` persistently.

3. Keep MCP server docs honest:

   - MCP still uses startup/config-derived active profile for JSON-RPC calls unless a future MCP request-level context API is added.
   - In-process `ExecutionContext` does not change the MCP wire protocol by itself.

### Acceptance Criteria

- Architecture docs agree with Rustdoc.
- Agent handoff docs are concise and practical.
- No architecture doc implies context features exist over MCP wire if they are only in-process.

## Workstream D: Verification Run and Evidence

### Required Work

Run the current canonical verification gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
cargo run --bin verify-eggsact -- --report markdown
```

If the full suite fails, record exact failure categories. Do not leave stale status from earlier commits.

If `verify-eggsact` provides enough detail, store or summarize its output in a new implementation note, for example:

```text
plans/phase-10-docs-verification-final-pass-implementation.md
```

The implementation note should include:

- date;
- commit SHA;
- each command;
- pass/fail/skip;
- parity availability;
- generated-doc freshness;
- known failing tests, if any.

### Acceptance Criteria

- Verification status is current for the final pass commit.
- No prior stale failure counts are presented as current unless revalidated.
- CI absence is not confused with CI success.
- Parity availability is explicit.

## Workstream E: Generated Documentation Freshness

### Required Work

Run:

```bash
cargo run --bin generate-docs -- --check
```

If it fails:

```bash
cargo run --bin generate-docs
```

Review generated diffs for:

- README generated tool block;
- architecture profile/tool reference blocks;
- `generated/tool-cards.md`;
- `.skills/mcp-tools.md` if maintained manually or by generator.

### Acceptance Criteria

- Generated docs check passes.
- Generated docs do not introduce stale tool counts.
- Any new context wording is not accidentally overwritten by the generator.

## Workstream F: Optional Test Precision Improvements

### Problem

The current tests cover the main context behaviors, but the per-call seed/template semantics would be clearer with one explicit regression test.

### Optional Test

Add a test named something like:

```rust
execution_context_eval_ctx_is_per_call_seed_for_immutable_dispatch
```

Behavior:

1. Create an `EvalContext` with a fixed PRNG seed.
2. Build an `ExecutionContext` with it.
3. Call `math_eval(random())` twice through `call_json_with_execution_context(&ctx)`.
4. Assert both calls produce the same first random value.
5. Add a comment explaining that persistent PRNG advancement requires `evaluate_with_context`/`run_with_context` directly or a future mutable tool-dispatch API.

This test is optional if docs are clear, but useful to lock down semantics.

### Acceptance Criteria

- If added, the test prevents future accidental assumptions about mutable persistence.
- The test name makes the semantic intent explicit.

## Recommended Implementation Order

1. Update `EvalContext` Rustdoc.
2. Update `ExecutionContext` and `call_json_with_execution_context` Rustdoc.
3. Update architecture/library/agent docs with per-call seed/template semantics.
4. Add the optional immutable-dispatch seed regression test if cheap.
5. Run generated-doc check and regenerate if needed.
6. Run full verification.
7. Add an implementation/verification note with exact current status.

## Final Acceptance Criteria

This final pass is complete when:

- docs accurately distinguish legacy global evaluator APIs from context-aware evaluator APIs;
- docs state that immutable `ExecutionContext` eval state is per-call cloned seed/template state;
- docs identify the current eval-context-aware tool path, especially `math_eval`;
- docs do not imply MCP wire-protocol request-level contexts;
- generated docs are fresh;
- current verification output is recorded;
- any failures are classified against current code, not old phase history;
- no unsupported release-ready claim remains.

## Non-Regression Requirements

- `call_json_with_execution_context` must continue to honor context profile, audience, compatibility mode, budget, and cancellation.
- `math_eval` must continue using `run_with_context` when an eval context is installed.
- Legacy `run()` and `evaluate()` behavior must remain compatible.
- `runtime_diagnostics` must remain harness-only.
- Generated docs/tool counts must remain consistent.
