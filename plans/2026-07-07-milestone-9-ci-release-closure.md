# Milestone 9: CI and Release Closure

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Close the hardening roadmap with a release-grade verification pass. The target is a repository state where CI, generated documentation, route-critical contracts, schema-boundary invariants, doctests, package contents, and release documentation all agree.

This milestone should be run after schema-boundary enforcement, deterministic coding-agent tool additions, profile/exposure cleanup, and diagnostics improvements. It is the final gate before treating `eggsact` as production-ready for broader codegg integration and public release.

## Rationale

The repo already has meaningful CI: formatting, generated-doc checks, Clippy, tests, integration tests with parity skipped, and cargo packaging. The remaining closure work is to make the release gates match the new quality bar established by the roadmap:

- Route-critical behavior must not drift.
- Tool schemas must not rely on unsupported keywords.
- Generated docs must remain synchronized with registry specs.
- Package contents must include generated source assets required by consumers.
- Doctests and public examples must compile.
- Diagnostics and profile docs must reflect the actual registry.
- Release steps must be explicit and repeatable.

## Scope

In scope:

- CI workflow audit and targeted additions.
- Local verification script alignment.
- Release checklist creation.
- Package content audit.
- Doctest coverage.
- Generated documentation drift check.
- Schema-boundary invariant check.
- Route-critical fixture contract check.
- Diagnostics/profile docs consistency check.
- Versioning and publish-readiness review.

Out of scope:

- New feature implementation.
- Major refactors.
- Changing the release artifact model.
- Automating crates.io publishing unless already part of repo practice.
- Adding external services or hosted CI dependencies beyond GitHub Actions.

## Files likely to change

- `.github/workflows/ci.yml`
- `src/bin/verify-eggsact.rs`
- `README.md`
- `docs/release.md` or `architecture/release.md`
- `AGENTS.md`
- `Cargo.toml`
- `tests/mcp/test_route_contracts.rs`
- `tests/mcp/test_tool_coverage.rs`
- `tests/mcp/test_schema_boundaries.rs` if added in milestone 5
- `generated/tool-cards.md`
- Any generated docs updated by the generator

## CI audit

Review the current CI workflow and ensure it includes these jobs or equivalent steps:

1. Formatting:

```bash
cargo fmt --all -- --check
```

2. Generated docs:

```bash
cargo run --bin generate-docs -- --check
```

3. Clippy:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

4. Library tests:

```bash
cargo test --all-features --lib
```

5. Binary tests:

```bash
cargo test --all-features --bins
```

6. Integration tests without parity:

```bash
cargo test --all-features --tests -- --skip parity
```

7. Doctests:

```bash
cargo test --all-features --doc
```

8. Package check:

```bash
cargo package --verbose
```

If CI runtime becomes too high, split jobs but do not remove coverage. Prefer parallel jobs over weakened checks.

## Required invariant checks

### Route-critical fixture contracts

Ensure CI runs tests that fail when:

- A route-critical tool lacks fixtures.
- A route-critical response lacks `machine_code` or required `verdict`.
- A route-critical finding emits an unregistered UPPERCASE machine/finding code.
- Harness-only route-critical tools become model-callable accidentally.

### Schema-boundary invariants

Ensure CI runs tests that fail when:

- A registered tool input schema uses unsupported validation keywords.
- A schema invariant failure lacks tool/path context.
- A supported keyword behavior regresses.

### Profile/exposure invariants

Ensure CI runs tests that fail when:

- Hidden tools appear in model/harness listings.
- Harness-only tools appear in model-facing listings.
- Documented profile names do not exist.
- New tools are missing profile placement.
- Alias conflicts are introduced.

### Generated-doc invariants

Ensure CI runs the generated-doc check and fails on drift. Generated docs should update in the same commit as registry/spec changes.

## Package-content audit

Add a package-content check if one does not already exist. The check should verify that `cargo package --list` includes required files and excludes development-only files according to `Cargo.toml` policy.

Required package contents should include:

- `src/` source files required to build.
- `src/text/confusables_generated.rs` or equivalent generated data required by Unicode tools.
- `README.md`.
- `LICENSE` if present.
- Any generated files required by docs.rs or runtime behavior.

Expected exclusions should include:

- `plans/`.
- `scripts/` if excluded by package policy.
- `data/` if generated source is vendored instead.
- `.github/`.
- Other development-only assets listed in `Cargo.toml` excludes.

If a required generated asset is intentionally excluded because it is regenerated at build time, document that explicitly. Prefer shipping generated source assets needed for reproducible builds.

## `verify-eggsact` alignment

Audit `cargo run --bin verify-eggsact` and align it with the release gate.

Recommended behavior:

- Run fmt.
- Run Clippy with `-D warnings`.
- Run tests excluding parity by default.
- Run doctests.
- Run generated-doc check.
- Run package check.
- Optionally run parity when a flag is supplied.
- Emit a concise Markdown summary.
- Return nonzero on failure.

If `verify-eggsact` is intended to be faster than full CI, document the difference and provide a `--full` flag or release command sequence.

## Release checklist

Create a release checklist document. Suggested path: `docs/release.md` or `architecture/release.md`.

Checklist should include:

### Pre-release validation

- Confirm version in `Cargo.toml`.
- Confirm README generated sections are current.
- Confirm generated tool cards are current.
- Confirm architecture docs reflect current tool count/profile count.
- Confirm route-critical fixtures pass.
- Confirm schema-boundary tests pass.
- Confirm package contents are correct.
- Confirm doctests pass.
- Confirm parity tests are either run locally or explicitly skipped with rationale.

### Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --all-features --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
cargo run --bin verify-eggsact
```

Optional parity:

```bash
cargo test --test lib parity
```

Adjust the exact parity command to match repo behavior.

### Publish dry run

Use:

```bash
cargo publish --dry-run
```

Only add this to CI if credentials are not required and runtime is acceptable. Otherwise document it as a local release-manager step.

### Tagging and changelog

If the repo does not maintain a changelog, either add a minimal `CHANGELOG.md` or document that GitHub releases/commit history are the changelog source. For public release, a short changelog is preferable.

## Documentation consistency audit

Before release, verify:

- README opening matches crate-level docs.
- Crate docs compile.
- MCP protocol version is documented consistently.
- Tool count and category count are consistent across generated docs and manual docs.
- Profile names in docs match registry names exactly.
- Env vars are documented consistently.
- JSON Schema support is described as a subset.
- POSIX-only command-preflight scope is clear.
- Route-critical tool list matches registry constants.
- Generated-assets/parity workflow doc is current.

## CI workflow recommendations

If current CI is flaky because of subprocess-heavy MCP tests, keep the recent in-process conversions and prefer in-process API tests for high-volume cases. Reserve subprocess MCP tests for protocol/wire coverage and representative route-critical cases.

If CI runtime is high, split into jobs:

- `fmt`
- `generated-docs`
- `clippy`
- `unit-tests`
- `integration-tests`
- `doctests`
- `package`

Keep all required checks enabled on protected branches if branch protection is used.

## Failure triage guidance

### Generated-doc failure

Run:

```bash
cargo run --bin generate-docs
```

Inspect README/generated diffs. If expected, commit them with the spec changes.

### Route-contract failure

Identify whether behavior changed intentionally. If intentional, update machine-code docs and fixtures in the same commit. If accidental, fix the tool.

### Schema-boundary failure

Do not ignore unsupported schema keywords. Either remove the keyword, mark it as annotation-only if appropriate, or extend the validator and tests.

### Package failure

Check `Cargo.toml` include/exclude rules. Verify generated source files required at build time are present in the package.

### Doctest failure

Prefer fixing examples rather than disabling doctests. If an example requires harness-only behavior, mark it `no_run` or use a model-safe example.

## Testing requirements

Run the full matrix locally before closing:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --all-features --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
cargo run --bin verify-eggsact
```

If GitHub Actions is available, confirm the latest `main` workflow run is green before release. If the workflow does not run for a commit, treat that as a release blocker until the trigger/branch behavior is understood.

## Acceptance criteria

- CI includes or is equivalent to the full release matrix.
- Route-critical contract tests are required in CI.
- Schema-boundary invariant tests are required in CI.
- Profile/exposure invariant tests are required in CI.
- Generated-doc drift fails CI.
- Doctests pass or documented non-runnable examples are intentionally marked.
- `cargo package --verbose` passes.
- Required generated assets are included in the package or intentionally generated during build with docs.
- Release checklist exists and is executable.
- Latest `main` CI run is green or local verification is documented if CI is unavailable.

## Handoff notes

Do this last. If earlier milestones are still changing tool specs, profiles, diagnostics, or schemas, release closure will churn. The milestone should be treated as a stabilization gate, not a place to add new feature scope.
