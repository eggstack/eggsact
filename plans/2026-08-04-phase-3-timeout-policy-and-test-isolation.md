# Phase 3 — Timeout Policy and Test Isolation

## Status

- **Status:** planned
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Roadmap commit:** `2211ebb3adae4df6551023676047d018e113a4f7`
- **Depends on:** Phase 1 execution-context soundness and Phase 2 path/wire corrections
- **Priority:** high
- **Scope:** remove test-load-specific production timeout overrides, make default budget resolution consistent across dispatch surfaces, and stabilize affected tests through bounded test isolation
- **Expected change size:** small-to-medium; mostly deletion plus focused test adjustments

## Objective

Restore product-driven timeout behavior.

After this phase:

1. `math_eval`, `text_diff_explain`, and `regex_finditer` no longer receive a 120-second production timeout because of parallel integration-test pressure;
2. MCP and in-process dispatch derive default budgets from the same declared tool-cost policy;
3. any transport-specific timeout difference has an explicit product reason and focused test, not a test-suite comment;
4. tests that exercise queueing, timeouts, or subprocess MCP behavior are deterministic without inflating production limits;
5. cooperative cancellation, bounded workers, panic conversion, and existing response envelopes remain intact;
6. the phase does not attempt to unify or replace both execution engines.

---

# Hard constraints

This phase must not:

- rewrite `src/mcp/execution.rs` and `src/mcp/sync_pool.rs` into a new common framework;
- add a third execution engine;
- replace Tokio or `spawn_blocking`;
- increase production worker counts to make tests pass;
- increase production queue sizes to make tests pass;
- add sleep-based retry loops;
- add per-tool production exceptions justified by CI load;
- expose a new public timeout configuration API;
- add environment variables used only by tests;
- require serial execution for the entire repository unless a measured, documented bounded thread count is the smallest stable solution;
- add stress loops to ordinary CI;
- add timing evidence files, workflow artifacts, or run-ID tracking;
- remove timeout/cancellation tests merely because they are difficult.

Prefer deleting the workaround and reducing test contention.

---

# Files to inspect first

At minimum inspect:

```text
src/mcp/budget.rs
src/mcp/execution.rs
src/mcp/sync_pool.rs
src/mcp/server.rs
src/agent/mod.rs
src/mcp/registry/types.rs
src/mcp/specs/
tests/mcp/test_execution_safety.rs
tests/mcp/test_hardening.rs
tests/mcp/test_context_isolation.rs
tests/mcp/test_comprehensive_parity.rs
tests/mcp/test_lifecycle_and_gaps.rs
tests/property/
.github/workflows/ci.yml
scripts/release-check.sh
architecture/budget-concurrency.md
architecture/mcp-server.md
architecture/agent-api.md
```

Search for:

```text
LOAD_TOLERANT_BUDGET_MS
load_tolerant_budget
120_000
spurious TIMEOUT
parallel integration
starves spawn_blocking
budget_for_tool
max_elapsed_ms
test-threads
std::thread::sleep
tokio::time::sleep
recv_timeout
spawn MCP
Command::new
```

Inventory every test that can leave a blocking handler running after the caller receives a timeout.

---

# Defect statement

The ordinary moderate budget is 30 seconds, but three MCP tools receive a 120-second override because the parallel integration suite can temporarily starve Tokio blocking workers. This is a test-harness accommodation compiled into production.

Consequences:

- MCP and in-process APIs disagree on the default elapsed budget for the same tool;
- diagnostics/documentation describe a 30-second moderate tier while production can use 120 seconds;
- a genuinely stuck or pathological invocation can retain capacity four times longer;
- tests no longer validate the documented product policy.

The production workaround must be removed. The test contention must be corrected at its source or bounded at the test-runner level.

---

# Workstream 1 — Establish one default budget-resolution path

## Required implementation

Default budget resolution should be:

```text
ToolSpec.cost
    -> budget_for_tool(tool_name, cost)
    -> optional explicit caller-provided override
```

MCP dispatch and in-process bounded dispatch must call the same production helper for the default case.

Required actions:

- delete `LOAD_TOLERANT_BUDGET_MS`;
- delete `load_tolerant_budget()`;
- remove the MCP branch that selects it;
- use `budget_for_tool(name, spec.cost)` in MCP dispatch;
- confirm synchronous registry dispatch already uses the same helper or adjust the smallest internal path;
- update diagnostics/docs only if they currently mention exceptional values;
- preserve the existing cheap/moderate/heavy tier values unless a separate product defect is demonstrated.

Explicit caller overrides in the in-process API remain valid. This phase does not remove `ToolBudget` customization.

## Transport-specific differences

A transport-specific budget is allowed only when all are true:

1. the difference is required by actual transport work rather than test contention;
2. the difference is documented;
3. a focused test proves the requirement;
4. it is applied by transport policy rather than a list of tool names.

No such difference is expected. Prefer identical defaults.

## Acceptance criteria

- repository search finds no `LOAD_TOLERANT_BUDGET_MS` or `load_tolerant_budget`;
- no production comment mentions test starvation as a budget reason;
- MCP and in-process default-budget tests assert the same values for representative cheap, moderate, and heavy tools;
- diagnostics match the effective defaults.

---

# Workstream 2 — Reproduce the masked test pressure before changing tests

## Required procedure

Before restructuring tests:

1. remove or locally bypass the 120-second override;
2. run the affected test filters under normal test scheduling;
3. run the full non-parity suite once;
4. identify whether failures arise from:
   - actual handler runtime;
   - Tokio blocking-pool starvation;
   - too many subprocess MCP servers;
   - detached timed-out handlers accumulating;
   - global sync-pool saturation;
   - timing assertions that assume an idle host.

Record the classification in the phase completion record. Do not preserve raw logs or create an evidence artifact.

Do not assume all three tools share the same root cause merely because they share the workaround.

---

# Workstream 3 — Isolate unit tests that own queue/timeout behavior

## Preferred pattern

Tests of `SyncExecutionPool` should instantiate a local pool with explicit worker/queue limits. Tests of MCP bounded execution should use a local semaphore and isolated metrics, as existing test hooks permit.

Required actions:

- ensure queue-saturation tests do not consume the process-global sync pool;
- ensure timeout tests release blocking gates before test teardown;
- ensure detached/timed-out handlers complete quickly after the assertion;
- replace remaining sleeps used for state observation with gates/channels/notifications where the existing test utilities already support them;
- keep long-running work simulated through blocking gates rather than multi-second sleeps;
- avoid leaking test-owned metrics or tasks beyond the smallest existing mechanism.

Do not add another test-hook framework. Reuse the current `BlockingTestGate`, `AsyncTestGate`, `TestEnqueueSignal`, local pools, and local semaphores.

## Required assertions

- timeout returns within a broad non-flaky upper bound;
- timed-out work remains counted/contained until its gate is released;
- queue-full behavior is observed deterministically;
- the pool/semaphore recovers after blocked work exits;
- cancellation flag visibility is explicit;
- no test accepts `TIMEOUT` or `RESOURCE_EXHAUSTED` as an arbitrary alternative for a simple successful call.

That last rule is important: simple functional tests should not normalize infrastructure starvation as valid behavior.

---

# Workstream 4 — Reduce subprocess and suite-level contention narrowly

## Audit questions

Determine:

- how many tests spawn the `eggsact --mcp` binary;
- whether helper functions start a fresh process for each request when a session can safely be reused inside one test;
- whether tests intended to verify tool semantics can call the in-process registry instead of spawning MCP;
- whether duplicate parity/contract cases run through multiple layers without adding coverage;
- whether test processes are left waiting for natural timeout rather than explicitly closed/killed.

## Allowed simplifications

- use in-process registry calls for tests that do not assert transport behavior;
- reuse one MCP subprocess within a single test when lifecycle/session behavior permits;
- explicitly close stdin and wait/kill with a bounded cleanup path;
- remove duplicate transport tests that prove the same contract at lower fidelity;
- reduce intentionally huge parallel case counts to the smallest representative set;
- set a modest test-thread bound for the main integration test binary or CI command only if deterministic isolation still cannot prevent host-wide blocking-pool starvation.

## Test-thread bound rule

A bounded `--test-threads` value is a last simple fallback, not the first fix.

It may be adopted when:

- focused tests are isolated;
- subprocess cleanup is bounded;
- remaining failures are reproducibly host-contention driven;
- one stable value such as 4 or 8 avoids oversubscription;
- the same command is used in ordinary CI and the local release gate;
- it reduces complexity compared with further test-only scheduler machinery.

Do not force `--test-threads=1` across all tests unless measurements show no smaller restriction works.

## Acceptance criteria

- simple tool tests no longer fail because unrelated timeout tests consume shared resources;
- MCP subprocess helpers have bounded cleanup;
- no new test-only production environment variable exists;
- any test-thread bound has a concise rationale and is applied consistently.

---

# Workstream 5 — Preserve product timeout and cancellation contracts

The correction must retain:

- cheap/moderate/heavy default budgets;
- per-call explicit `ToolBudget` overrides in supported in-process APIs;
- MCP semaphore worker bound;
- sync-pool worker/queue bound;
- cooperative cancellation flag propagation;
- timeout response machine code;
- panic-to-structured-error conversion;
- output truncation after successful completion;
- the documented fact that timed-out blocking work may continue briefly until cooperative cancellation is observed.

Add focused tests that call representative tools through both dispatch surfaces and inspect resolved elapsed budgets where an internal test seam already exists. Do not expose budgets in new public response fields solely for testing.

---

# Workstream 6 — Documentation cleanup

Update only affected documents:

```text
CHANGELOG.md
architecture/budget-concurrency.md
architecture/mcp-server.md
architecture/agent-api.md
architecture/testing.md
AGENTS.md              # only high-signal command/gotcha changes
```

Required statements:

- production budgets derive from declared cost/tool policy;
- tests do not change production timeout values;
- timeout enforcement is cooperative for blocking handlers;
- any adopted test-thread bound is a test-runner containment measure, not a product budget;
- MCP and in-process default budget semantics are consistent.

Do not create benchmark, scheduler, or test-isolation architecture documents.

---

# Rejection searches

Before completion, search for and disposition:

```text
LOAD_TOLERANT_BUDGET_MS
load_tolerant_budget
120_000
spurious TIMEOUT
parallel integration test harness
accept TIMEOUT
accept RESOURCE_EXHAUSTED
std::thread::sleep(Duration::from_secs
```

Some sleeps may remain in non-concurrency functional tests. Review only timeout/pool/server tests relevant to this phase.

---

# Execution order for a smaller implementation agent

1. Sync to latest `origin/main`; confirm Phases 1 and 2 are complete.
2. Inventory the production budget-resolution call sites and affected tests.
3. Temporarily remove the load-tolerant override and reproduce/classify failures.
4. Add/adjust tests asserting equal default budget resolution.
5. Delete the production override and route MCP through `budget_for_tool`.
6. Isolate local pool/semaphore tests and remove shared-resource acceptance alternatives.
7. Bound subprocess cleanup and convert non-transport cases to in-process calls.
8. Re-run affected filters under normal scheduling.
9. Adopt a modest suite thread bound only if still necessary and documented.
10. Run the full ordinary verification gate.
11. Update bounded documentation and this completion record once.

Do not begin release/CI simplification until the normal test suite is stable without production inflation.

---

# Verification

Targeted commands should include discovered exact filters. At minimum:

```bash
cargo test --locked --all-features sync_pool
cargo test --locked --all-features execution_safety
cargo test --locked --all-features timeout
cargo test --locked --all-features cancellation
cargo test --locked --all-features math_eval
cargo test --locked --all-features text_diff_explain
cargo test --locked --all-features regex_finditer
```

Run the affected filters repeatedly only enough to establish stability, for example 10 local repetitions. Do not turn repetition into a permanent CI loop.

Then run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

If a test-thread bound is adopted, run the exact final ordinary command as well.

---

# Acceptance checklist

- [ ] `LOAD_TOLERANT_BUDGET_MS` is removed.
- [ ] `load_tolerant_budget()` is removed.
- [ ] MCP and in-process default budget resolution use the same production helper.
- [ ] No production timeout is justified by test-suite load.
- [ ] Affected simple tools succeed under the ordinary documented budget.
- [ ] Queue/timeout tests use isolated pools, semaphores, metrics, or existing deterministic gates.
- [ ] Timed-out test handlers receive bounded cleanup.
- [ ] Simple functional tests no longer accept timeout/exhaustion as arbitrary success alternatives.
- [ ] MCP subprocess tests are bounded and used only where transport behavior matters.
- [ ] Any suite thread bound is modest, documented, and consistent.
- [ ] Cooperative cancellation, panic conversion, and worker bounds remain intact.
- [ ] No new execution framework, configuration surface, workflow, or dependency was added.
- [ ] Focused tests and ordinary verification pass.
- [ ] Documentation matches effective product policy.

---

# Completion record

Fill once when implementation lands:

- **Implementation commit(s):** pending
- **Masked failure classification:** pending
- **Production budget path:** pending
- **Test isolation changes:** pending
- **Subprocess cleanup changes:** pending
- **Test-thread bound disposition:** pending
- **Targeted stability runs:** pending
- **Ordinary verification:** pending
- **Documentation updated:** pending
- **Deferred findings:** broad execution-engine unification remains out of scope
- **Final phase disposition:** pending
