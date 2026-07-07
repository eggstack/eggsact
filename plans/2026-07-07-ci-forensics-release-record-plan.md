# CI forensics and release-record closure plan

## Purpose

This is a narrow follow-up pass after the CI cleanup/polish work. The codebase now has a better CI workflow shape, local parity is bounded by an accepted-failures fixture, and `EGGCALC_MCP_AUDIENCE` has safer parsing. The remaining unresolved issue is process-level: recent head commits still did not show a visible GitHub Actions run or combined status through the review connector.

The objective of this pass is to make CI state auditable, not to change product behavior.

## Current known state

Recent cleanup added:

- `.github/workflows/ci.yml` with `push`, `pull_request`, and `workflow_dispatch` triggers;
- split Rust test jobs: `test-lib`, `test-bins`, and `test-integration` with parity skipped;
- package job depending on mandatory Rust gates;
- case-insensitive `EGGCALC_MCP_AUDIENCE` parsing with safe `Model` fallback;
- diagnostics output for `active_audience`;
- `tests/fixtures/accepted_parity_failures.txt` listing the 31 accepted parity failure names;
- updated AGENTS, CHANGELOG, skills, and parity docs.

However, remote status remained unresolved in review:

- GitHub combined status for the head commit returned no statuses.
- GitHub workflow runs associated with the head commit returned no workflow runs.

That means the repo may be technically ready, but it does not yet have remote CI evidence suitable for release closure.

## Non-goals

Do not add new MCP tools.
Do not modify tool behavior unless CI exposes a concrete failure.
Do not change parity semantics except to preserve the existing accepted-failures boundary.
Do not treat accepted parity differences as CI failures.
Do not overclaim release readiness without remote CI evidence.

## Task 1: verify workflow file validity locally

### Goal

Make sure `.github/workflows/ci.yml` is syntactically valid before investigating GitHub-side settings.

### Steps

1. Inspect `.github/workflows/ci.yml` for YAML syntax and indentation.
2. Confirm the top-level trigger key is parsed as `on`, not accidentally as a boolean by local tools.
3. If available, validate with one of:

```bash
ruby -e 'require "yaml"; p YAML.load_file(".github/workflows/ci.yml")'
python - <<'PY'
import yaml
with open('.github/workflows/ci.yml', 'r', encoding='utf-8') as f:
    print(yaml.safe_load(f).keys())
PY
```

4. Confirm the workflow jobs are valid GitHub Actions YAML:
   - `check`
   - `clippy`
   - `generated-docs`
   - `test-lib`
   - `test-bins`
   - `test-integration`
   - `package`
5. Confirm `package.needs` names match the exact job IDs.

### Acceptance criteria

The workflow file parses cleanly and all `needs` references point to existing jobs.

## Task 2: check repository Actions settings and permissions

### Goal

Determine whether Actions are disabled, restricted, or prevented from running connector-created commits.

### Steps

Using GitHub UI or `gh` CLI, check:

```bash
gh api repos/eggstack/eggsact/actions/permissions
gh api repos/eggstack/eggsact/actions/workflows
gh workflow list --repo eggstack/eggsact
```

Also inspect repository Settings -> Actions -> General for:

- whether Actions are enabled;
- whether all actions/reusable workflows are allowed;
- whether the workflow is disabled;
- whether branch filters or rules block runs;
- whether org-level policy restricts Actions for the repo.

### Acceptance criteria

The pass identifies the reason no workflow runs were visible, or confirms Actions are enabled and should run on the next normal push/manual dispatch.

## Task 3: manually dispatch CI if possible

### Goal

Generate an auditable workflow run without adding product changes.

### Steps

1. Use the GitHub UI or CLI to run the workflow manually:

```bash
gh workflow run CI --repo eggstack/eggsact --ref main
```

If the workflow name differs, list workflows first:

```bash
gh workflow list --repo eggstack/eggsact
```

2. Watch the run:

```bash
gh run list --repo eggstack/eggsact --branch main --limit 5
gh run watch --repo eggstack/eggsact <run-id>
```

3. If it fails, fetch logs:

```bash
gh run view --repo eggstack/eggsact <run-id> --log-failed
```

### Acceptance criteria

A workflow run exists for `main`, with a run ID, URL, head SHA, and conclusion. If manual dispatch is not available, document the exact reason.

## Task 4: if manual dispatch is unavailable, create a normal trigger commit

### Goal

Test whether ordinary GitHub push events trigger CI.

### Steps

Only do this if manual dispatch cannot be used or does not produce a run.

1. Make a minimal meaningful docs-only commit, for example updating the release decision record with a `Remote CI pending` note.
2. Push through the normal developer Git remote, not necessarily the connector path, to test whether connector-created commits are the issue.
3. Check run list and status after push:

```bash
gh run list --repo eggstack/eggsact --branch main --limit 5
gh status --repo eggstack/eggsact
```

### Acceptance criteria

A push-triggered workflow run appears. If it does not, Actions settings or workflow recognition are still broken and must be fixed before release.

## Task 5: fix CI failures if a run appears

### Goal

Make mandatory Rust CI green remotely.

### Expected jobs

- `check`: `cargo fmt --all -- --check`
- `clippy`: `cargo clippy --all-targets --all-features -- -D warnings`
- `generated-docs`: `cargo run --bin generate-docs -- --check`
- `test-lib`: `cargo test --all-features --lib`
- `test-bins`: `cargo test --all-features --bins`
- `test-integration`: `cargo test --all-features --tests -- --skip parity`
- `package`: `cargo package --verbose`

### Steps

1. For each failing job, inspect logs before changing code.
2. If `--skip parity` does not exclude all Python-dependent tests, retag or refactor tests so CI can cleanly exclude parity-only cases while still running all deterministic Rust integration tests.
3. If clippy fails on deprecated compatibility shims, use local `#[allow(deprecated)]` only at call sites that intentionally test shims.
4. If generated docs fail, regenerate through `cargo run --bin generate-docs` and commit generated artifacts.
5. If package fails because of included/excluded files, update package metadata deliberately.

### Acceptance criteria

All mandatory CI jobs pass remotely. Any optional parity job, if added later, is non-blocking and clearly labeled.

## Task 6: stabilize accepted parity fixture usage

### Goal

Make sure the accepted parity failures fixture does not become dead documentation.

### Steps

1. Inspect `tests/fixtures/accepted_parity_failures.txt`.
2. Confirm it contains exactly the currently accepted 31 failure test names.
3. Decide whether to add a helper script or test that compares local parity failures against this fixture.

Recommended lightweight implementation:

- Add `scripts/check_parity_failures.py` or a Rust helper test ignored by default.
- It should read parity output and compare failures to the fixture.
- It should report:
  - unexpected failures;
  - accepted failures still failing;
  - accepted failures that now pass and should be removed from the fixture.

If implementing this helper is too much for the CI forensics pass, add a note to the release decision record saying the fixture is currently documentation-only.

### Acceptance criteria

The repo clearly states whether `accepted_parity_failures.txt` is enforced by tooling or documentation-only. Future agents should not have to infer this.

## Task 7: update release decision record with evidence

### Goal

Close the loop with a concrete release decision tied to a commit and CI run.

### File targets

Use the existing release decision file if present:

- `plans/2026-07-07-release-decision-record.md`

Prefer also creating or moving stable final release evidence into docs:

- `docs/release-readiness-2026-07-07.md`

### Required contents

- evaluated head SHA;
- workflow name;
- workflow run ID and URL;
- workflow conclusion;
- local verification commands and results, if rerun;
- parity result: `385 passed / 31 failed / 2 ignored` or updated result;
- accepted parity fixture path;
- explanation that parity is excluded from normal CI because Python `eggcalc` is unavailable;
- final decision: `release-ready-with-documented-parity-deltas`, `release-candidate-local-only`, or `not-release-ready`.

### Decision rules

Use `release-ready-with-documented-parity-deltas` only if:

- mandatory GitHub Actions jobs pass remotely;
- no CI job is missing or skipped unexpectedly;
- accepted parity failures are documented and granular;
- no unknown parity failures remain;
- generated docs check passes.

Use `release-candidate-local-only` if:

- local gates pass;
- remote CI cannot be made visible due to repo/org settings outside the codebase;
- the limitation is documented clearly.

Use `not-release-ready` if:

- mandatory remote CI fails;
- Actions are disabled and no local verification evidence is current;
- unknown parity failures remain;
- route-critical tool contract tests fail.

### Acceptance criteria

The release decision file contains evidence, not just assertions.

## Task 8: final documentation cleanup

### Steps

After CI status is known, update:

- `CHANGELOG.md` with remote CI result;
- `AGENTS.md` if the release decision or parity count changes;
- `.skills/release.md` and `.skills/testing.md` if CI invocation changed;
- `docs/parity.md` if parity count or accepted category wording changes.

Then run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

### Acceptance criteria

Docs do not overclaim. The release decision and CI evidence agree.

## Suggested commit structure

1. `ci: repair workflow trigger or syntax for visible runs`
2. `test: adjust CI integration filters for parity-free Rust tests`
3. `docs(release): add CI-backed release decision evidence`
4. `docs(parity): clarify accepted failure fixture enforcement status`
5. `docs: update changelog and skills after CI forensics`

## Done criteria

This pass is complete when:

- the reason for missing GitHub Actions runs is identified;
- a remote workflow run is visible, or absence is documented with cause;
- mandatory CI is green remotely, or release status is downgraded honestly;
- release decision record cites concrete evidence;
- accepted parity fixture status is clear;
- no product behavior changes were made except fixes required by CI failures.
