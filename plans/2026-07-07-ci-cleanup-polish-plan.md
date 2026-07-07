# CI cleanup and release-polish plan

## Purpose

This plan covers the final cleanup and polish work needed to move `eggsact` from a well-documented release-candidate state to a clean, auditable GitHub release posture.

The repository has already completed the major agent-tooling/corrective sequence:

- generated documentation drift was repaired;
- UTF-8 masking and byte-budget semantics were corrected;
- codegg-facing typed wrappers were added and fixed;
- the MCP server gained concurrent request handling and id-based response correlation in tests;
- repo/diff/path intelligence tools were added;
- `EGGCALC_MCP_AUDIENCE` was added so MCP subprocesses can run with `Model`, `Harness`, or `Debug` audience;
- parity improved to a documented `385 passed / 31 failed / 2 ignored` state;
- remaining parity failures were classified as accepted behavioral differences.

The remaining work is process cleanup: make GitHub Actions clean and visible, ensure release documentation matches actual CI evidence, and polish the new audience/parity behavior so it remains maintainable.

## Non-goals

Do not add new MCP tools.
Do not start dependency/lockfile phase work.
Do not broaden model-visible exposure for harness-only tools.
Do not force ordered JSON-RPC responses.
Do not treat Rust-only tools as Python parity regressions.
Do not claim CI is green unless a GitHub Actions run or explicitly documented local command output proves it.

## Current known risks

1. GitHub status checks/workflow runs have not been visible for recent connector-created head commits.
2. Local verification was documented as passing for several gates, but remote CI evidence is absent.
3. The new `EGGCALC_MCP_AUDIENCE` env var is now part of MCP startup semantics and needs complete tests/docs.
4. The remaining 31 parity failures are accepted, but the acceptance record should be easy to audit and should not mask future regressions.
5. `cargo test --all-features` appears to include parity/MCP harness failures in some runs; CI should distinguish release gates from optional parity comparisons cleanly.

## Task 1: inspect and repair GitHub Actions triggers

### Goal

Ensure every push or pull request to `main` runs an auditable CI workflow.

### Steps

1. Inspect `.github/workflows/ci.yml`.
2. Confirm triggers include at least:

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  workflow_dispatch:
```

3. If current triggers omit connector-created pushes or only run on pull requests, add `push` and `workflow_dispatch`.
4. Confirm the workflow does not rely on secrets for normal Rust CI.
5. Add a short comment in the workflow explaining that parity tests requiring Python `eggcalc` are intentionally excluded from CI unless a separate optional job is added.

### Acceptance criteria

`ci.yml` clearly runs on `push`, `pull_request`, and manual dispatch for `main`. Normal Rust release gates are not blocked by Python `eggcalc` availability.

## Task 2: split CI into mandatory and optional jobs

### Goal

Keep GitHub CI clean without hiding parity status.

### Recommended workflow structure

Mandatory `rust-ci` job:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Optional `parity-local-doc` or `parity-note` job:

- Do not fail CI because Python `eggcalc` is not installed.
- Print a clear note that full parity requires a local checkout of Python `eggcalc` at `../eggcalc` or an explicit configured path.
- If parity is later made runnable in CI, gate it behind an opt-in env var or dedicated workflow.

If `cargo test --all-features --tests -- --skip parity` does not skip all parity-dependent tests, introduce test filters or features so CI can run deterministic Rust tests without external Python state.

### Acceptance criteria

Mandatory CI passes without Python `eggcalc`. Parity status remains documented, but optional parity gaps do not make normal CI red.

## Task 3: add a lightweight CI trigger commit only if needed

### Goal

Produce remote evidence for the current head or a successor head.

### Steps

1. After workflow trigger cleanup, push a meaningful small commit if needed.
2. Prefer one of:
   - update `.skills/testing.md` with the exact CI/parity split;
   - update `docs/release-readiness-2026-07-07.md` or the release decision record with the CI workflow run URL placeholder/result;
   - update `CHANGELOG.md` final closure notes after the workflow run completes.
3. Avoid empty commits or churn that obscures release history.
4. Check GitHub status for the resulting commit.

### Acceptance criteria

A GitHub Actions workflow run is visible for the head commit. If Actions still do not run, document the reason in the release decision record and in `docs/parity.md` or the release-readiness file.

## Task 4: harden `EGGCALC_MCP_AUDIENCE` behavior

### Goal

Ensure the new audience env var is safe, deterministic, and documented as startup-only MCP context.

### Steps

1. Inspect `src/mcp/runtime.rs` parsing for `EGGCALC_MCP_AUDIENCE`.
2. Confirm accepted values are explicit and case behavior is documented. Recommended values:
   - `model`
   - `harness`
   - `debug`

   If implementation accepts `Model`, `Harness`, `Debug`, decide whether to preserve that and document it. Prefer case-insensitive parsing for usability.
3. Confirm invalid values fail safely:
   - either default to `Model` with a diagnostic warning;
   - or reject startup with a clear error.

   Recommended: default to `Model` for safety and expose the parsed audience in diagnostics.
4. Add or confirm tests for:
   - unset env var -> `Model`;
   - `Model`/`model` -> `Model`;
   - `Harness`/`harness` -> `Harness`;
   - `Debug`/`debug` -> `Debug`;
   - invalid value -> safe behavior;
   - MCP tools/call uses `get_active_audience()` rather than hardcoded model.
5. Confirm `runtime_diagnostics` includes known env var names but does not leak env var values unless intentionally safe.

### Acceptance criteria

Audience parsing is tested, safe by default, and reflected in diagnostics/docs. Harness-only tools remain hidden from `Model` audience.

## Task 5: preserve parity acceptance without masking regressions

### Goal

The 31 accepted parity differences should remain a bounded known set. Future regressions should fail visibly.

### Steps

1. In `docs/parity.md`, ensure the decision table has enough specificity to identify all 31 accepted failures by test name.
2. Add a small machine-readable or test-readable allowlist if practical, for example:

```rust
const ACCEPTED_PARITY_FAILURES: &[&str] = &[
    "test_name_here",
];
```

or a text fixture under `tests/fixtures/accepted_parity_failures.txt`.

3. Add a parity summary helper that reports:
   - accepted failures;
   - unexpected failures;
   - unexpectedly passing accepted failures.
4. If implementing the allowlist now is too much, add a specific follow-up note in the release decision record.
5. Do not make parity green by ignoring broad modules. Keep acceptance granular.

### Acceptance criteria

The accepted 31 failures are explicit. A new parity failure cannot be silently hidden under a broad category.

## Task 6: clean and stabilize release decision record

### Goal

Create or update a release-readiness artifact that is stable and easy for future agents to cite.

### File target

Use the existing `plans/2026-07-07-release-decision-record.md` if it already landed, but consider moving or copying final decision content to `docs/release-readiness-2026-07-07.md` so it is not buried in planning docs.

### Required contents

- evaluated head commit SHA;
- CI workflow name and run URL;
- CI conclusion;
- exact local commands run and results;
- parity command and result;
- count of accepted parity differences;
- link to parity decision table;
- release decision: `release-ready`, `release-ready-with-documented-parity-deltas`, or `not-release-ready`;
- list of release blockers, if any;
- next recommended phase.

### Acceptance criteria

The release decision record reflects actual remote CI evidence. If remote CI cannot run, the decision must not say `release-ready`; use `not-release-ready` or `release-candidate-local-only` wording.

## Task 7: update changelog, README, AGENTS, and skills after CI is clean

### Steps

1. Update `CHANGELOG.md` final closure section with actual GitHub Actions status.
2. Update `AGENTS.md` if parity counts or env-var gotchas changed.
3. Update `.skills/testing.md` with:
   - mandatory CI command set;
   - optional local parity command;
   - note that multi-request MCP helpers must correlate by JSON-RPC `id`;
   - note that MCP subprocess parity helpers use `EGGCALC_MCP_AUDIENCE=Harness` for harness-only tools.
4. Update README only if user-facing release readiness or env-var docs changed outside generated sections.
5. Run generated-docs check if generated docs are touched.

### Acceptance criteria

All release-facing docs align with current CI/parity state and do not overclaim.

## Task 8: final command matrix

Run locally before pushing final cleanup:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Run parity locally if Python `eggcalc` is available:

```bash
cargo test --test lib parity -- --nocapture
```

Then check remote status for head:

```bash
gh run list --branch main --limit 5
gh run view <run-id> --log-failed
```

If `gh` is unavailable, use GitHub web UI or API.

## Suggested commit structure

1. `ci: run rust release gates on push and pull request`
2. `test(mcp): harden audience env parsing and diagnostics`
3. `docs(parity): lock accepted parity failure decision table`
4. `docs(release): add CI-backed release readiness record`
5. `docs: update changelog and testing notes after CI cleanup`

## Done criteria

This cleanup pass is complete when:

- GitHub Actions runs on the current head;
- mandatory Rust CI is green remotely;
- optional parity state is documented but does not make normal CI red;
- `EGGCALC_MCP_AUDIENCE` is tested and documented;
- the 31 accepted parity differences are explicit and granular;
- release decision record is backed by remote CI evidence;
- changelog, README/AGENTS/skills, and parity docs agree;
- no new agent-tooling features were added.
