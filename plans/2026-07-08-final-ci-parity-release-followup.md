# Final Follow-Up Plan: CI Visibility, Parity Audit, and Release Polish

Date: 2026-07-08

Repository: `eggstack/eggsact`

Related plans:

- `plans/2026-07-08-corrective-verification-schema-release-pass.md`
- `plans/2026-07-07-milestone-9-ci-release-closure.md`
- `plans/2026-07-07-remaining-hardening-polish-plan.md`

## Objective

Close the remaining release-readiness gaps after the corrective verification/schema pass. The repo now has schema-boundary invariants, expanded analysis-tool tests, release checklist documentation, doctests in CI, and reported local verification success. The remaining work is narrow: verify GitHub Actions visibility, audit parity-failure classifications, run the release checklist exactly as documented, and polish any inconsistencies discovered during that execution.

This plan should not add new tools or expand the product surface. Treat it as final release hygiene.

## Current state to assume

Recent corrective work reports:

- 80 registered tools.
- Schema-boundary invariant coverage for all registered tool schemas.
- `tests/mcp/test_schema_boundaries.rs` added.
- `tests/mcp/test_analysis_tools.rs` added.
- CI split to lib/bin/integration/doc test jobs.
- Release checklist added.
- `release.sh` and `verify-eggsact` aligned with the split test matrix.
- Local verification reported: fmt pass, clippy pass, 404 unit tests, 3164 integration tests, 10 doctests, generated-docs check, and package pass.
- Parity documented as 33 known failures out of 418 tests.

The only externally visible uncertainty is GitHub Actions status: prior checks via connector showed no workflow runs/statuses for the latest commits. This may be a connector limitation, a workflow trigger issue, or an Actions configuration issue. It must be resolved before claiming CI is green.

## Non-goals

Do not add more MCP tools.

Do not change command-preflight policy unless a release gate fails.

Do not broaden the JSON Schema validator beyond the documented subset.

Do not remove parity failures from accepted lists without running the parity suite.

Do not publish or tag as part of this plan unless explicitly instructed after verification.

## Workstream 1: GitHub Actions visibility and trigger validation

### 1.1 Inspect workflow trigger configuration

Review `.github/workflows/ci.yml` and confirm it has the expected triggers:

- `push` to `main`, or no branch restriction that excludes `main`.
- `pull_request` targeting `main`, if PR validation is intended.
- `workflow_dispatch`, if manual runs are intended.

If pushes to `main` should trigger CI but no runs appear, inspect whether the workflow file is syntactically valid and whether workflow permissions/settings could disable it.

### 1.2 Confirm job graph

Verify the workflow contains separate jobs for:

- format check;
- clippy;
- generated docs;
- lib tests;
- binary tests;
- integration tests with parity skipped;
- doctests;
- package job depending on all prior jobs.

Confirm job names are stable and obvious in GitHub’s UI.

### 1.3 Force a harmless CI-triggering commit if needed

If workflow triggers are correct but no run appears, make a minimal docs-only or comment-only commit that should trigger CI. Examples:

- Update a release checklist timestamp.
- Fix a typo in release docs.
- Add a short note to `docs/release.md` clarifying CI trigger expectations.

Do not make a no-op whitespace-only commit unless that is normal practice for the repo.

### 1.4 Document the outcome

Add a short note to release docs or a release-readiness note:

- GitHub Actions trigger behavior verified on commit `<sha>`.
- CI run URL or status if available.
- If CI cannot be observed through the connector, state that local verification is the source of evidence and explain why.

Acceptance:

- CI trigger behavior is understood.
- Either a green GitHub Actions run is observed, or the absence of runs is explicitly explained and documented.
- The release checklist no longer leaves CI visibility ambiguous.

## Workstream 2: Parity failure classification audit

### 2.1 Reconcile counts

Review all parity-related references and ensure they agree:

- `docs/parity.md`.
- `tests/fixtures/accepted_parity_failures.txt`.
- `AGENTS.md`.
- `.skills/testing.md`.
- release docs.
- README if parity status is mentioned.

Current expected statement:

- Python reference: 67 tools.
- Rust implementation: 80 tools.
- Parity suite: 418 tests.
- Known accepted failures: 33.

If any number differs, update it or explain why the context is different.

### 2.2 Audit accepted failure entries

For every entry in `tests/fixtures/accepted_parity_failures.txt`, confirm:

- It appears in `docs/parity.md` or a categorized parity section.
- It has a category label.
- It has a short rationale.
- It is marked as accepted, deferred, or to-fix.
- It is not a newly introduced regression masquerading as accepted.

Prefer category-level rationale for groups of similar failures, but individual failures with unusual causes should have individual notes.

### 2.3 Run parity if Python reference is available

If `../eggcalc` exists and the Python reference can run, execute:

```bash
cargo test --test lib parity
```

Expected result may be nonzero if accepted failures are implemented as expected-fail logic. If the harness has an accepted-failure filter, run the accepted-filtered mode as well. Capture:

- total tests;
- passed;
- accepted failures;
- unexpected failures;
- unexpected passes.

If Python reference is unavailable, do not fabricate results. Document that parity could not be run locally and leave current documented status as last-known evidence.

### 2.4 Guard against drift

Add or verify a test that checks accepted parity failure names remain valid. The desired behavior:

- An accepted failure entry that no longer fails should be surfaced as stale.
- A new parity failure not in the accepted list should fail the parity audit.
- Category comments remain human-readable.

Acceptance:

- Parity counts are consistent across docs.
- Accepted failures are auditable.
- Any run result is recorded honestly.
- No unknown parity regression is silently accepted.

## Workstream 3: Final release checklist dry run

Run the documented release gate exactly in the order listed in release docs:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Then run:

```bash
cargo run --bin verify-eggsact
./release.sh
```

If `verify-eggsact` or `release.sh` duplicates the full matrix, still run them because they are release surfaces that must stay correct.

Optional but recommended before public release:

```bash
cargo publish --dry-run
```

Acceptance:

- The release gate passes.
- `verify-eggsact` passes and reports the same expected steps.
- `release.sh` passes and matches docs.
- Any failure is fixed, not documented away, unless it is external-environment-only and explicitly out of scope.

## Workstream 4: Package and docs polish

### 4.1 Package content check

Run:

```bash
cargo package --list
```

Verify package includes:

- all source files required by the 80-tool registry;
- `src/tools/analysis.rs`;
- `src/mcp/schemas/analysis.rs`;
- `src/mcp/specs/analysis.rs`;
- `src/text/confusables_generated.rs` or any generated data required for build/runtime;
- README;
- license;
- docs.rs-relevant docs if expected.

Verify package excludes:

- `plans/`;
- `.github/`;
- local scripts/data intentionally excluded by package metadata;
- development-only test fixtures if excluded by policy.

If a required file is missing, fix `Cargo.toml` package include/exclude rules.

### 4.2 Docs consistency pass

Check exact counts and names in:

- README;
- `architecture/overview.md`;
- `architecture/tools.md`;
- `architecture/mcp-server.md`;
- `architecture/coding-agent-integration.md`;
- `docs/mcp-tools.md`;
- `generated/tool-cards.md`;
- release docs;
- AGENTS.md;
- skills files.

Expected current values unless implementation has changed:

- 80 tools.
- 20 categories.
- 11 profiles if docs now say 11; otherwise reconcile to actual registry.
- 33 accepted parity failures out of 418 parity tests.

### 4.3 Remove duplicate release-doc ambiguity

The repo now has both `docs/release.md` and `docs/architecture/release.md` or similarly named release documents. Decide whether both should remain.

Preferred options:

1. Keep one canonical release checklist and make the other a short pointer.
2. Keep `docs/release.md` as canonical and let architecture docs link to it.
3. Keep `docs/architecture/release.md` only if the repo convention expects architecture docs under `docs/architecture/`, and update all references.

Avoid two divergent release checklists.

Acceptance:

- Package contents are correct.
- Tool/profile/parity counts agree across docs.
- There is one canonical release checklist or an explicit pointer relationship.

## Workstream 5: Small test polish for release confidence

Do not add broad new suites. Add narrowly targeted tests only if gaps are found during audit.

Potential high-value additions:

- A test that CLI diagnostics tool count equals registry count.
- A test that generated docs mention every category.
- A test that release docs mention every release gate command.
- A package-list smoke check in `verify-eggsact`, if practical without making it slow.
- A parity accepted-failures stale-entry check, if not already present.

Acceptance:

- Any added tests are narrow, stable, and connected to release gates.
- No new flaky subprocess-heavy tests are introduced.

## Workstream 6: Final status note

At the end of the pass, add a short release-readiness note if it does not already exist. Suggested path:

- `docs/release-readiness.md`, or
- a section in `docs/release.md`.

Include:

- latest verified commit SHA;
- local release-gate result;
- CI run result or CI visibility explanation;
- parity status;
- package status;
- known deferred items.

Known deferred items should be limited and explicit. Expected candidates:

- accepted parity differences versus Python reference;
- any GitHub Actions visibility limitation if not solvable from repo code;
- optional `cargo publish --dry-run` if not run.

Acceptance:

- Release readiness can be assessed without rereading the entire conversation or commit history.
- Deferred items are explicit and bounded.

## Suggested commit structure

Use small commits:

1. `ci: verify workflow triggers and document actions behavior`
2. `docs(parity): reconcile accepted parity failure status`
3. `docs(release): make release checklist canonical`
4. `test(release): add narrow release-gate consistency tests`
5. `docs: add release readiness note`

If CI trigger validation requires only docs, combine it with release documentation cleanup.

## Final acceptance criteria

This follow-up is complete when:

- GitHub Actions visibility is verified or its absence is explained.
- Release docs have one canonical checklist.
- Parity counts and accepted failures are consistent and auditable.
- The documented release gate passes locally.
- `verify-eggsact` and `release.sh` both pass or are fixed.
- Package contents are verified.
- Any remaining deferred item is explicitly documented.
- No new tool, schema, profile, or diagnostic drift is introduced.

## Handoff note

The repo is already in strong shape. This pass should be short and evidence-oriented. Avoid feature creep; the value is in proving the release surface is clean and reproducible.
