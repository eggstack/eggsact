# Release-readiness follow-up: parity acceptance, CI execution, and decision record

## Purpose

This plan covers the remaining follow-up work after the final closure pass. The repo is now in good technical shape for the agent-tooling/corrective line: multi-request MCP parity now correlates by JSON-RPC `id`, the response-ordering contract is documented, `ToolBudget` compatibility shims are in place, generated-docs/package/clippy/fmt gates were reported passing locally, and parity improved to 362 passed / 54 failed / 2 ignored out of 416 parity tests.

The remaining work is not another feature phase. It is release-readiness hygiene:

1. Determine whether the remaining 54 parity failures are release-blocking, test-harness-only, Python/Rust output drift, or accepted Rust-superset deltas.
2. Ensure GitHub Actions actually runs on the current head and records an auditable result.
3. Capture a concise release decision record so future agents do not rediscover the same parity caveats.

## Current known state

From the final closure pass notes:

- `mcp_request_multi()` now correlates by JSON-RPC `id`; the earlier concurrent-ordering parity failures are resolved.
- `BudgetContext::check_text_len(...)` remains as a deprecated compatibility alias for `check_text_bytes(...)`.
- `ToolBudget::with_max_text_bytes(n)` was added.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo run --bin generate-docs -- --check`, and `cargo package --verbose` were reported passing locally.
- Full non-parity test suite was reported as `2885 passed, 130 failed`, where the remaining 130 are documented as pre-existing parity/MCP harness failures.
- Parity suite was reported as `362 passed, 54 failed, 2 ignored`.
- GitHub status checks/workflow runs were not visible through the connector at the time of review.

## Non-goals

Do not add phase 06 dependency/lockfile tools here.
Do not add new MCP tools.
Do not redesign the concurrent MCP runtime.
Do not force ordered JSON-RPC responses unless a real client requires it.
Do not make Rust tools match absent Python tools solely to reduce parity failure counts.

## Task 1: create a parity failure decision table

### Goal

Every remaining parity failure should have a classification and release decision. The goal is to stop treating the number `54` as opaque.

### Steps

1. Run parity locally where Python `eggcalc` is available:

```bash
cargo test --test lib parity -- --nocapture 2>&1 | tee target/parity-2026-07-07.log
```

2. Extract failing test names into a temporary working file:

```bash
rg "FAILED|failures:|test .*FAILED" target/parity-2026-07-07.log
```

3. Build a markdown table in `docs/parity.md` or a new companion file `docs/parity-decision-record.md` with columns:

| Test | Category | Root cause | Release blocking? | Fix path | Owner notes |
|------|----------|------------|-------------------|----------|-------------|

4. Use these categories:

- `harness-audience`: test invokes MCP with model/default audience but expects harness-only tools.
- `tool-output-drift`: Rust and Python both implement the tool but output shape, formatting, warning, or exact error differs.
- `rust-superset`: Rust intentionally exposes tools that Python does not implement.
- `schema-contract-drift`: Rust schema or validation differs from intended wire contract.
- `runtime-protocol`: JSON-RPC/MCP behavior difference not attributable to the test helper.
- `unknown`: not yet classified; unknown failures are release-blocking until triaged.

5. For every failure, assign `Release blocking?` as one of:

- `yes`: real Rust behavior issue or contract break.
- `no`: known accepted delta.
- `defer`: real work, but not blocking this release line if documented.

### Acceptance criteria

No remaining parity failure is unclassified. `unknown` count is zero before release. Any `yes` item must have a fix plan or a blocking decision.

## Task 2: fix low-risk parity failures if they are clearly test-harness-only

### Goal

If some of the remaining failures are purely caused by parity helper profile/audience setup, fix the test harness rather than documenting them indefinitely.

### Steps

1. Inspect failures categorized as `harness-audience`.
2. Identify whether the affected test should use:
   - `EGGCALC_MCP_PROFILE=codegg_preflight`;
   - `EGGCALC_MCP_PROFILE=codegg_patch`;
   - `EGGCALC_MCP_PROFILE=codegg_config`;
   - or in-process `ToolRegistry::with_profile_and_audience(..., ToolAudience::Harness)`.
3. If parity is comparing Rust MCP against Python MCP, prefer setting the Rust MCP profile/environment in the harness rather than rewriting tool semantics.
4. Do not make harness-only tools model-visible just to satisfy parity tests.
5. Add regression tests ensuring model audience still rejects harness-only tools where appropriate.

### Acceptance criteria

Any fixed harness-audience failures reduce the parity failure count without broadening model-visible exposure. Remaining harness-audience failures are documented with a concrete reason they were deferred.

## Task 3: classify and optionally fix tool/output drift

### Goal

Determine which `tool-output-drift` failures should be fixed now versus accepted as compatibility drift.

### Steps

1. For each drift failure, compare Rust and Python output at the JSON field level.
2. Identify whether the difference is:
   - harmless ordering/formatting drift;
   - missing machine-code/verdict field;
   - incorrect severity/disposition;
   - incorrect parser/validator behavior;
   - intentionally stricter Rust behavior;
   - Python bug compatibility not worth preserving.
3. Fix immediately if:
   - Rust omits a documented field;
   - Rust returns a wrong machine code;
   - Rust accepts unsafe input Python rejects and the stricter behavior is desired;
   - Rust rejects safe input due to a clear bug.
4. Defer/document if:
   - Python behavior is inconsistent or unsafe;
   - Rust intentionally moved to a stronger route-critical contract;
   - difference is cosmetic and no downstream code depends on exact text.

### Acceptance criteria

Every output drift failure is either fixed or explicitly marked accepted/deferred. No route-critical field omissions remain.

## Task 4: handle Rust-superset tool gaps explicitly

### Goal

Rust-only tools should not pollute Python parity metrics without explanation.

### Steps

1. Confirm current Rust-only tools relative to Python, including:
   - `config_file_inspect`
   - `dependency_edit_preflight`
   - `repo_manifest_inspect`
   - any newer tools such as `repo_tree_summarize`, `diff_risk_classify`, `path_batch_scope_check` if included in parity inventory comparisons.
2. Decide the parity policy:
   - Rust-superset tools are excluded from Python parity equality; or
   - Rust-superset tools are expected to produce a documented `not implemented in Python` gap entry; or
   - Python parity fixture is updated to know these are Rust-only additions.
3. Update `docs/parity.md` with the policy.
4. Update parity helper code if it can mechanically skip Rust-superset tools while still checking that the Rust registry is internally consistent.

### Acceptance criteria

Rust-superset tools are no longer confused with parity regressions. Docs clearly separate Python parity from Rust registry completeness.

## Task 5: ensure GitHub Actions runs on head

### Goal

The final closure notes report local gate success, but no GitHub Actions run was visible. Create an auditable remote CI signal.

### Steps

1. Inspect `.github/workflows/ci.yml` and confirm triggers include `push` and/or `pull_request` for `main`.
2. If workflow triggers are missing or branch filters exclude this branch, fix the workflow trigger.
3. If Actions did not run because files were pushed through the connector in a way that did not trigger workflows, create a no-op CI trigger commit only if necessary. Prefer a meaningful docs touch rather than empty churn.
4. Check status for the resulting head commit.
5. Record exact workflow name, run URL, head SHA, and conclusion in the release decision record.

### Acceptance criteria

GitHub Actions has a visible run for the head commit or there is a documented repository setting/permission reason why it cannot run. Release notes should not claim remote CI passed unless a workflow run confirms it.

## Task 6: add a release decision record

### Goal

Create a stable handoff artifact that tells future agents and maintainers whether this line is release-ready and why.

### File

Create `docs/release-readiness-2026-07-07.md` or an equivalent date-stamped file.

### Required contents

- Head commit SHA evaluated.
- Commands run locally and results.
- GitHub Actions run status and URL if available.
- Parity result summary.
- Parity failure decision table or link to it.
- List of accepted non-blocking deltas.
- List of release blockers, if any.
- Decision: `release-ready`, `release-ready-with-documented-parity-deltas`, or `not-release-ready`.
- Next recommended phase.

### Recommended decision criteria

Use `release-ready-with-documented-parity-deltas` only if:

- all non-parity gates pass;
- GitHub Actions has an auditable result or a documented reason it cannot run;
- no parity failure is unknown;
- no parity failure is classified release-blocking;
- docs and changelog reflect the exact current state.

Use `not-release-ready` if:

- any non-parity gate fails;
- GitHub Actions cannot be assessed and no local verification evidence is available;
- any parity failure remains unknown;
- any route-critical tool has a contract failure.

## Task 7: update changelog and agent notes after decision

### Steps

1. Update `CHANGELOG.md` under the final closure section with:
   - final parity count;
   - final CI status;
   - release decision record link;
   - accepted deltas.
2. Update `AGENTS.md` only if the parity count or gotchas change.
3. Update `.skills/testing.md` with the correct parity command and the rule that multi-request helpers must correlate by JSON-RPC `id`.
4. Run generated-docs check if README/generated sections changed.

### Acceptance criteria

Docs and handoff notes agree with the release decision record.

## Verification commands

Run at minimum:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Run parity if Python `eggcalc` is available:

```bash
cargo test --test lib parity -- --nocapture
```

If `cargo test --all-features` includes known parity failures, split and document:

```bash
cargo test --all-features --lib
cargo test --test lib -- --skip parity
cargo test --test lib parity -- --nocapture
```

Use the actual test layout if those commands need adjustment.

## Suggested commit structure

1. `test(parity): classify remaining parity failures`
2. `test(parity): fix harness-audience setup for deferred tool groups`
3. `docs(parity): add parity decision record`
4. `ci: ensure release gates run on main`
5. `docs(release): add release readiness decision record`
6. `docs: update changelog and agent notes for release decision`

## Done criteria

This follow-up is complete when:

- each remaining parity failure is classified;
- unknown parity failure count is zero;
- any release-blocking parity failures are fixed or explicitly block release;
- GitHub Actions has an auditable status for head, or the absence is documented with cause;
- a release decision record exists;
- docs/changelog/agent notes match the release decision;
- no new features were introduced during the pass.
