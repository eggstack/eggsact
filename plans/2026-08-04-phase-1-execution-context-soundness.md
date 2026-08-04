# Phase 1 — Execution-Context Soundness

## Status

- **Status:** planned
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Roadmap commit:** `2211ebb3adae4df6551023676047d018e113a4f7`
- **Implementation baseline:** use the latest `origin/main` when execution begins
- **Priority:** critical; this phase blocks all later phases
- **Scope:** remove the unsound mutable evaluation-context thread-local API and make context/cancellation nesting restoration correct
- **Expected change size:** small-to-medium, localized to budget/context bridging, calculator-backed tool dispatch, and focused tests

## Objective

Eliminate undefined behavior reachable from safe Rust while preserving current deterministic calculator and registry behavior.

After this phase:

1. no safe function returns an escaping `&'static mut EvalContext` derived from a raw pointer;
2. callers can access the current evaluation context only inside a closure whose borrow cannot escape;
3. nested evaluation-context and cancellation scopes restore their immediate parent at arbitrary practical depth;
4. panic unwinding restores the previous scope;
5. `math_eval`, `ToolRegistry::call_json`, bounded registry calls, and MCP dispatch keep their documented state semantics;
6. no broad handler-signature or execution-engine redesign is introduced.

---

# Hard constraints

This phase must not:

- change the 80-tool catalog;
- add a new context framework;
- change every tool handler signature;
- replace thread-local bridging across the entire crate if a bounded safe repair is sufficient;
- introduce a global mutex around all tool execution;
- make calculator state persistent through generic dispatch when it is currently documented as clone-scoped;
- revive deprecated mutable-context commit-slot machinery;
- redesign MCP concurrency, budgets, or cancellation policy;
- add a dependency;
- add Miri, loom, sanitizer, or a new soundness workflow to ordinary CI;
- use `unsafe` merely to preserve the current API shape;
- retain `current_eval_context() -> Option<&'static mut EvalContext>` as a safe public compatibility shim.

A small internal callback API and RAII guard are the intended solution.

---

# Files to inspect first

At minimum inspect:

```text
src/mcp/budget.rs
src/tools/math.rs
src/agent/mod.rs
src/mcp/execution.rs
src/mcp/server.rs
src/calc/context.rs
src/calc/mod.rs
tests/mcp/
tests/calc/
tests/property/
architecture/calculator.md
architecture/budget-concurrency.md
architecture/agent-api.md
architecture/overview.md
```

Search for:

```text
CURRENT_EVAL_CONTEXT
PREV_EVAL_CONTEXT
CURRENT_CANCEL_FLAG
PREV_CANCEL_FLAG
with_eval_context
current_eval_context
with_cancel_flag
current_cancel_flag
EvalContextGuard
CancelFlagGuard
call_json_with_execution_context
call_json_with_execution_context_mut
run_with_context
```

Record every production caller before editing. Do not assume `math_eval` is the only caller without repository search.

---

# Defect statement

The current bridge stores `*mut EvalContext` in thread-local state and exposes it through a safe function returning `Option<&'static mut EvalContext>`.

This is unsound because safe callers may:

- call the function twice and hold two mutable references to the same object;
- retain the `'static` reference after `with_eval_context()` returns;
- move the reference into storage whose lifetime exceeds the pointed-to context.

The implementation also stores one prior value in `PREV_EVAL_CONTEXT` and one in `PREV_CANCEL_FLAG`. A third nested scope overwrites restoration state needed by the outer scope. The documentation promises nested restoration, so this is a correctness defect independent of the unsafe reference lifetime.

---

# Workstream 1 — Replace escaping context retrieval with closure access

## Required design

Replace the safe reference-returning function with a closure-based interface. Recommended shape:

```rust
pub fn with_current_eval_context<R>(
    f: impl FnOnce(Option<&mut EvalContext>) -> R,
) -> R
```

An equivalent name is acceptable if it is clearer and does not collide with the existing installer function.

Required properties:

- the mutable borrow exists only during `f`;
- the return type cannot contain a reference tied to the context;
- the raw pointer, if retained internally, is dereferenced only inside the callback scope;
- the function is internal or public only to the minimum module surface required by existing code;
- callers cannot obtain two simultaneous mutable references through safe API calls;
- no `'static` reference appears in the public or internal safe signature.

Preferred use in `math_eval`:

```rust
let eval_result = with_current_eval_context(|ctx| match ctx {
    Some(ctx) => run_with_context(&expr_owned, ctx),
    None => run(&expr_owned),
});
```

Do not copy this literally if the surrounding panic conversion requires a different closure arrangement. Preserve panic containment.

## Compatibility decision

`current_eval_context()` is not a sound compatibility surface. Remove it or make it unavailable to safe callers. Do not deprecate it while leaving the unsafe behavior callable.

If it is part of the published API, document this as a soundness correction in `CHANGELOG.md`. A breaking source-level removal is acceptable when retaining the function would preserve undefined behavior.

## Acceptance criteria

- repository search finds no safe function returning `&'static mut EvalContext`;
- `math_eval` uses closure-scoped access;
- direct `ToolRegistry::call_json("math_eval", ...)` continues to install a fresh native context;
- MCP dispatch continues to install an MCP-safe context;
- no fallback to process-global calculator state is introduced for dispatch paths that currently isolate state.

---

# Workstream 2 — Make restoration guard-owned and truly nestable

## Current problem

The current installer stores the displaced value in a second thread-local `PREV_*` slot. This is global per thread rather than per scope, so deeper nesting overwrites parent restoration state.

## Required implementation

Make each guard own the value it must restore.

Recommended conceptual shape:

```rust
struct EvalContextGuard {
    previous: Option<*mut EvalContext>,
}

impl Drop for EvalContextGuard {
    fn drop(&mut self) {
        CURRENT_EVAL_CONTEXT.with(|cell| {
            *cell.borrow_mut() = self.previous.take();
        });
    }
}
```

Use the analogous pattern for cancellation flags.

An explicit thread-local stack is acceptable but likely more complex than guard ownership. Prefer the guard-owned previous value unless testing proves it insufficient.

Required properties:

- one nested scope cannot overwrite another scope's restoration data;
- normal return restores the immediate parent;
- panic/unwind restores the immediate parent;
- three or more nested scopes work;
- the cancellation flag and evaluation context use the same structural pattern where practical;
- no extra allocation is required for ordinary nesting.

## Borrowing caution

Do not hold a `RefCell` borrow while invoking user/tool code. Install the pointer/flag, release the cell borrow, then call the closure. The guard should restore state during unwind.

## Acceptance criteria

- `PREV_EVAL_CONTEXT` and `PREV_CANCEL_FLAG` are removed;
- or, if an explicit stack is used, no single previous-value slot remains;
- nesting tests pass at depth at least three;
- unwind tests prove parent restoration;
- an inner scope with `None` correctly restores a non-`None` parent and vice versa.

---

# Workstream 3 — Focused regression coverage

Add tests at the nearest existing module. Do not build a new integration harness.

## Required evaluation-context tests

1. **Single scope:** installed context is visible inside the callback and absent afterward.
2. **Three nested contexts:** each callback observes its own context; each return restores its immediate parent.
3. **Nested absence:** an inner cleared/absent scope restores the outer context.
4. **Unwind restoration:** panic inside an inner scope is caught by the test; the outer context is visible afterward.
5. **No escaping API:** this is primarily compile-structure enforcement; add a source-level or API test only if the repository already has an appropriate pattern. Do not add compiletest infrastructure.
6. **Repeated access:** sequential callback accesses work without allowing simultaneous mutable borrows.

Use distinguishable `EvalContext` state, such as seed or variable state already exposed by test helpers. Do not add test-only production fields merely to identify contexts.

## Required cancellation tests

1. three nested flags restore in LIFO order;
2. an inner cancellation mutation does not replace the outer flag object;
3. panic/unwind restores the outer flag;
4. no flag remains installed after the outer scope exits.

## Required dispatch smoke tests

- direct registry `math_eval` returns the expected result;
- bounded registry `math_eval` returns the expected result;
- MCP `math_eval` remains deterministic and rejects side-effect/random behavior according to existing MCP mode;
- persistent calculator state still works through `run_with_context()` or `evaluate_with_context()` directly.

Do not duplicate the complete calculator test suite.

---

# Workstream 4 — Documentation and API reconciliation

Update only documentation that describes the bridge or state model:

```text
CHANGELOG.md
architecture/calculator.md
architecture/budget-concurrency.md
architecture/agent-api.md
architecture/overview.md
src/agent/mod.rs rustdoc
src/mcp/budget.rs rustdoc
```

Required documentation statements:

- generic dispatch owns or clones an evaluation context for the duration of the call;
- mutable references do not escape the dispatch bridge;
- persistent calculator sessions use the direct calculator context APIs;
- generic `_mut` dispatch remains deprecated and does not imply calculator-state persistence;
- nesting restoration is guard-scoped and unwind-safe.

Do not add a new architecture document for this small correction.

---

# Rejection searches

Before completion, repository search must confirm:

```text
current_eval_context() -> Option<&'static mut EvalContext>   # absent
PREV_EVAL_CONTEXT                                           # absent
PREV_CANCEL_FLAG                                            # absent
&'static mut EvalContext                                    # absent outside clearly justified tests or unrelated code
```

Review all remaining `unsafe` blocks involving `EvalContext`. Any retained raw-pointer dereference must be inside the bounded callback implementation and documented with the exact lifetime invariant.

Do not expand this into a repository-wide unsafe audit.

---

# Execution order for a smaller implementation agent

1. Sync to latest `origin/main` and record the baseline SHA.
2. Search and list all callers of the four context/cancellation bridge functions.
3. Add depth-three and unwind regression tests for current behavior; the nesting test should fail before repair.
4. Refactor the two guards to own their previous state.
5. Add the closure-based current-context accessor.
6. Migrate `math_eval` and any other production callers.
7. Remove the unsound accessor and prior-value thread-locals.
8. Run targeted budget/math/context tests.
9. Run the ordinary verification gate.
10. Update bounded documentation and the completion record in this file.

Do not combine this work with timeout-policy, path, CI, or footprint changes.

---

# Verification

Targeted commands should include the exact existing test filters discovered during implementation. At minimum:

```bash
cargo test --locked --all-features budget
cargo test --locked --all-features context
cargo test --locked --all-features math_eval
cargo test --locked --all-features test_context_isolation
```

Then run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Miri is optional maintainer-local evidence if already available. It is not an acceptance dependency and must not be added to CI.

---

# Acceptance checklist

- [ ] No safe API returns an escaping mutable evaluation-context reference.
- [ ] Guard-owned or stack-based restoration supports at least three nested scopes.
- [ ] Evaluation context restores correctly after normal return.
- [ ] Evaluation context restores correctly after panic/unwind.
- [ ] Cancellation flags restore correctly after normal return and unwind.
- [ ] `math_eval` uses closure-scoped context access.
- [ ] Direct, bounded, and MCP math dispatch retain current behavior.
- [ ] Direct calculator context APIs retain persistent-state behavior.
- [ ] No new dependency, subsystem, or handler-signature migration was introduced.
- [ ] Focused tests and ordinary verification pass.
- [ ] Documentation reflects the corrected lifetime/state model.

---

# Completion record

- **Implementation commit(s):** pending
- **Unsound API disposition:** `current_eval_context()` deprecated, replaced by `with_current_eval_context()` closure-based accessor
- **Restoration design:** Guard-owned previous value (`CancelFlagGuard { previous }`, `EvalContextGuard { previous }`); `PREV_CANCEL_FLAG` and `PREV_EVAL_CONTEXT` thread-locals removed
- **Targeted tests:** depth-3 nesting (eval context + cancel flag), depth-3 unwind restoration, existing panic-safety tests all pass
- **Ordinary verification:** fmt, clippy, 3505 tests, doc tests, generate-docs check all pass
- **Documentation updated:** CHANGELOG.md, architecture/budget-concurrency.md, architecture/mcp-server.md, architecture/calculator.md, src/agent/mod.rs rustdoc, AGENTS.md, mcp-tools skill, testing skill
- **Deferred findings:** none
- **Final phase disposition:** complete
