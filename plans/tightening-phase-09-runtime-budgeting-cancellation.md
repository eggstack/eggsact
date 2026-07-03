# Phase 9: Runtime Budgeting, Cancellation, and Resource Discipline

## Goal

Harden eggsact runtime behavior so codegg can rely on deterministic tools without risking runaway CPU, memory, output size, thread growth, or stale cancellation state. This phase establishes explicit budgets for tool execution, response size, spawned work, regex/sample workloads, and MCP request lifecycle behavior.

Phase 9 does not need to make the MCP stdio server fully concurrent. The current serial read-loop model can remain. The goal is resource discipline and predictable failure semantics across MCP and in-process harness usage.

## Current State

The repo already has several runtime guardrails:

- MCP request rate limiting.
- cancellation tracking.
- max request byte size.
- max output byte size.
- tool timeout constants.
- blocking tool execution guarded by a semaphore.
- helper-level text/list/regex/sample limits.
- schema validation before handler execution.
- route-critical response contracts and machine codes.

Known limitations remain:

- Some timeout paths rely on spawned blocking work that cannot be forcibly killed once running.
- In-process `ToolRegistry::call_json` does not have the same lifecycle wrapper as MCP async dispatch.
- Resource budgets are spread across runtime, helpers, schemas, and individual handlers.
- Cancellation semantics are MCP-oriented and do not yet provide a general in-process cancellation token.
- Output truncation and machine-code behavior should be made uniform.
- Composite tools need sub-budget propagation to avoid multiplying work.

## Scope

Phase 9 covers budgets, cancellation semantics, output discipline, and runtime observability. It does not cover evaluator context isolation, full async server redesign, filesystem sandboxing, or daemon-mode execution. Those are later concerns.

## Workstream A: Centralize Budget Definitions

### Problem

Runtime limits are currently distributed. This makes it hard for codegg to know what a tool may consume and hard for maintainers to reason about budget changes.

### Required Work

1. Introduce a central `ToolBudget` type.

   Suggested shape:

   ```rust
   pub struct ToolBudget {
       pub max_input_bytes: usize,
       pub max_output_bytes: usize,
       pub max_text_chars: usize,
       pub max_list_items: usize,
       pub max_regex_pattern_chars: usize,
       pub max_regex_samples: usize,
       pub max_elapsed_ms: u64,
       pub max_spawned_workers: usize,
       pub max_findings: usize,
   }
   ```

2. Provide default budget tiers.

   - `cheap`
   - `moderate`
   - `heavy`

   These should align with `ToolCost` where possible, but do not overload `ToolCost` itself. `ToolCost` is descriptive; `ToolBudget` is enforceable.

3. Add budget lookup by tool.

   Options:

   - Add budget metadata to `ToolSpec`.
   - Derive budget tier from `ToolCost` initially.
   - Override specific tools where needed.

4. Document budget policy in architecture docs.

### Acceptance Criteria

- There is one obvious place to inspect default runtime budgets.
- Tool-specific overrides are explicit.
- Budget semantics are visible to maintainers and codegg.

## Workstream B: Unify MCP and In-Process Execution Wrappers

### Problem

MCP `tools/call` wraps execution with async timeout/semaphore/output handling, while in-process calls are more direct. codegg will likely prefer in-process calls, so the same budget behavior should be available there.

### Required Work

1. Add an execution wrapper in the agent layer.

   Suggested API:

   ```rust
   pub fn call_json_with_budget(
       &self,
       name: &str,
       args: Value,
       budget: Option<ToolBudget>,
   ) -> Result<ToolResponse, ToolCallError>
   ```

2. Keep existing `call_json` as a compatibility method that uses default budget behavior.

3. Ensure MCP and in-process paths use shared preparation logic:

   - profile check.
   - audience check.
   - compatibility mode.
   - schema validation.
   - budget selection.
   - output normalization.

4. Avoid duplicating error mapping.

   MCP may still convert registry errors into JSON-RPC errors, but underlying machine codes and budget failures should be shared.

### Acceptance Criteria

- In-process codegg use can opt into explicit budgets.
- Existing public API remains source-compatible where possible.
- MCP and in-process behavior do not drift for budget failures.

## Workstream C: Timeout Semantics and Non-Killable Work

### Problem

Rust cannot safely kill arbitrary blocking work in another thread. A timeout can stop waiting, but the underlying work may continue. This must be explicit and bounded.

### Required Work

1. Audit all timeout wrappers.

   Identify:

   - thread-spawn timeout helpers.
   - `spawn_blocking` paths.
   - regex/sample loops.
   - composite tool calls.

2. Document timeout semantics precisely.

   Example:

   - MCP timeout means the request returns timeout once the wait budget is exceeded.
   - Blocking worker may continue until natural completion.
   - Spawn semaphore bounds total concurrent blocking work.

3. Reduce non-killable work risk.

   - Check budgets before spawning.
   - Bound sample counts and input sizes aggressively.
   - Prefer incremental loops that check deadlines.
   - Avoid nested unbounded spawn calls inside composite tools.

4. Add `Deadline` or `BudgetContext` for cooperative checks.

   Suggested shape:

   ```rust
   pub struct BudgetContext {
       pub deadline: Option<Instant>,
       pub budget: ToolBudget,
       pub cancellation: Option<CancellationTokenLike>,
   }
   ```

   Avoid pulling in a heavy dependency unless necessary. A small internal struct is enough.

5. Add tests for fast timeout behavior using controlled slow helper code if feasible.

### Acceptance Criteria

- Timeout behavior is honest and documented.
- Non-killable work is bounded by input limits and semaphore limits.
- Composite tools propagate or respect deadlines.

## Workstream D: Cancellation Semantics

### Problem

MCP has cancellation notifications, but in-process codegg workflows need a predictable way to cancel or avoid stale work. Current cancellation tests focus on JSON-RPC request IDs, not a general cancellation model.

### Required Work

1. Clarify MCP cancellation semantics.

   - Cancellation before execution.
   - Cancellation during queued execution.
   - Cancellation during running blocking execution.
   - Cancellation after completion.

2. Add MCP tests for each state where feasible.

3. Introduce an in-process cancellation handle only if needed.

   Suggested minimal API:

   ```rust
   pub struct ToolCallContext {
       pub budget: Option<ToolBudget>,
       pub cancelled: Arc<AtomicBool>,
   }
   ```

   Do not introduce full async cancellation unless the rest of the agent API becomes async.

4. Ensure composite tools can check cancellation before sub-tool calls.

5. Emit stable machine code for cancellation.

   Use existing `CANCELLED` if already present. Ensure envelope responses and JSON-RPC mappings are consistent.

### Acceptance Criteria

- MCP cancellation behavior is documented and tested.
- In-process callers have a clear cancellation story, even if minimal.
- Cancellation does not leave unbounded state in the runtime.

## Workstream E: Output and Finding Truncation

### Problem

Large outputs and large finding arrays can overwhelm agents. Output caps exist, but route-critical truncation semantics need to be explicit.

### Required Work

1. Add uniform response truncation helpers.

   - cap large string fields.
   - cap arrays such as findings.
   - cap nested subresults.
   - annotate `limits_applied`.

2. Define truncation behavior for route-critical tools.

   Route-critical outputs must not silently drop all evidence. If findings are truncated:

   - keep the highest severity findings first.
   - include count of omitted findings.
   - set a `limits_applied` entry.
   - consider machine code `OUTPUT_TOO_LARGE` only if truncation changes routing.

3. Ensure generated docs mention output limits.

4. Add tests:

   - huge text output truncated.
   - huge findings list truncated deterministically.
   - limits_applied set.
   - JSON remains valid.

### Acceptance Criteria

- Output limits are uniform.
- Truncation is visible to callers.
- Route-critical routing is not invalidated by truncation.

## Workstream F: Composite Tool Sub-Budgets

### Problem

Composite tools such as `edit_preflight`, `config_preflight`, and later dependency inspectors call sub-tools internally. Without sub-budgets, one top-level call can multiply work across helpers.

### Required Work

1. Define sub-budget allocation.

   Options:

   - static fractions per sub-tool.
   - per-composite explicit budget split.
   - shared deadline with per-sub-call input/output caps.

2. Apply to existing composites:

   - `edit_preflight`
   - `config_preflight`
   - `command_preflight`
   - `text_security_inspect`
   - `patch_apply_check`
   - `cargo_toml_inspect` if composite.

3. Ensure sub-tool failures map to parent findings or parent errors consistently.

4. Add tests for composite behavior when one sub-tool hits a budget.

### Acceptance Criteria

- Composite tools cannot amplify resource use unexpectedly.
- Sub-tool budget failures are visible and route-safe.
- Existing successful cases remain unaffected.

## Workstream G: Runtime Metrics and Debug Diagnostics

### Problem

codegg can optimize tool usage only if eggsact provides basic metrics. This phase should add lightweight diagnostics without building a full telemetry stack.

### Required Work

1. Add optional per-call metrics in `ToolResponse` or a debug-only field.

   Suggested fields:

   - elapsed milliseconds.
   - input bytes.
   - output bytes before/after truncation.
   - budget tier.
   - limits applied.
   - sub-tool count for composites.

2. Gate metrics if necessary.

   Options:

   - always include in `limits_applied`/result diagnostics.
   - only include under debug schema detail.
   - enable with env var.

3. Avoid leaking host paths or sensitive data.

4. Add tests that metrics are present only when expected.

### Acceptance Criteria

- Metrics are useful for codegg but not noisy for ordinary clients.
- Metrics do not leak sensitive filesystem/user data.

## Workstream H: Tests and Verification

### Required Tests

1. Budget selection:

   - cheap/moderate/heavy defaults.
   - tool override.
   - explicit caller override.

2. Input limits:

   - too-large text.
   - too many list items.
   - too many regex samples.

3. Output limits:

   - huge output string.
   - huge findings list.
   - nested subresult cap.

4. Timeout/cancellation:

   - pre-execution cancelled request.
   - during-queue cancellation if feasible.
   - timeout returns stable code.

5. Composite budgets:

   - sub-tool failure visible in parent response.
   - deadline propagation.

6. Regression:

   - existing phase 01–08 tests still pass.

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Final Acceptance Criteria

Phase 9 is complete when:

- Tool budgets are centralized, documented, and applied consistently.
- MCP and in-process calls have aligned budget semantics.
- Timeout limitations are documented and bounded by semaphores/input caps.
- Cancellation behavior is explicit and tested.
- Output/finding truncation is deterministic and visible.
- Composite tools propagate sub-budgets or shared deadlines.
- Optional metrics/debug diagnostics are available without leaking sensitive data.
- Full CI passes.
