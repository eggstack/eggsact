# Corrective Pass: Schema Boundary, Verification, and Release Closure

Date: 2026-07-08

Repository: `eggstack/eggsact`

Related plans:

- `plans/2026-07-07-coding-agent-hardening-roadmap.md`
- `plans/2026-07-07-milestone-5-schema-validation-boundaries.md`
- `plans/2026-07-07-milestone-6-deterministic-coding-agent-tools.md`
- `plans/2026-07-07-milestone-7-profiles-exposure-agent-ergonomics.md`
- `plans/2026-07-07-milestone-8-diagnostics-self-inspection.md`
- `plans/2026-07-07-milestone-9-ci-release-closure.md`

## Objective

Run a corrective hardening pass after the rapid implementation of the 78-tool surface. The repo has gained the planned deterministic coding-agent tools, profile/docs invariants, and diagnostics/self-inspection tools. The remaining risk is that the implementation moved faster than final release verification. This pass closes the specific gaps most likely to create regressions: schema-boundary enforcement, generated-doc drift, profile/exposure consistency, diagnostics shape stability, package contents, parity documentation, and CI/local verification.

This is not a feature milestone. Treat it as a stabilization pass. Only implement feature changes if they are necessary to satisfy the invariants or make the release matrix pass.

## Current state to assume

Recent commits indicate:

- Tool count increased from 71 to 78.
- Category count increased from 19 to 20 with a new `analysis` category.
- New tools landed: `patch_contract_check`, `test_command_suggest`, `repo_language_detect`, `import_export_inspect`, `code_block_map`, `symbol_name_diff`, `lockfile_inspect`.
- Generated docs were updated.
- Profile/docs invariant tests were added.
- Diagnostics gained `profile_inspect` and `tool_availability_explain`.
- Runtime diagnostics were expanded.

Do not assume CI is green unless GitHub Actions or local verification confirms it. The corrective pass should produce explicit verification evidence.

## Non-goals

Do not add more coding-agent tools.

Do not broaden the schema validator into full JSON Schema.

Do not add network calls, registry lookups, vulnerability feeds, or command execution.

Do not convert heuristic source analysis into full parsers.

Do not loosen route-critical, profile, or schema invariants merely to make tests pass. Fix the underlying schema, metadata, tests, or documentation.

## Workstream 1: Establish a clean verification baseline

Start by running the full local matrix before changing code. Capture failures in notes so the pass is guided by evidence rather than assumptions.

Required commands:

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

Also run targeted filters for recent work:

```bash
cargo test --all-features analysis -- --nocapture
cargo test --all-features diagnostics -- --nocapture
cargo test --all-features profile -- --nocapture
cargo test --all-features route_contracts -- --nocapture
cargo test --all-features schema -- --nocapture
```

If command names differ because tests are nested under `tests/lib.rs`, adapt the filters, but run the equivalent coverage.

Deliverable:

- A short verification note in the closing commit message or a committed `docs/release-readiness.md` if failures require explanation.
- No undocumented failing command remains at pass close.

## Workstream 2: Implement schema-boundary enforcement

Milestone 5 is the remaining hardening gap. Implement it before further polishing generated docs because all new tools added schemas.

### 2.1 Document supported and unsupported schema keywords

Add or update architecture/developer docs to clearly state that the internal validator supports only a subset of JSON Schema.

Supported validation keywords should include, if still accurate:

- `type`
- `properties`
- `required`
- `additionalProperties`
- `items`
- `minItems`
- `maxItems`
- `uniqueItems`
- `minLength`
- `maxLength`
- `pattern`
- `minimum`
- `maximum`
- `exclusiveMinimum`
- `exclusiveMaximum`
- `multipleOf`
- `enum`
- `const`

Allowed annotation-only keywords may include:

- `description`
- `title`
- `default`
- `examples`
- `$schema` only if already present and explicitly ignored as annotation. Prefer avoiding it in built-in schemas.

Explicitly unsupported validation constructs:

- `$ref`
- `$defs`
- `definitions`
- `oneOf`
- `anyOf`
- `allOf`
- `not`
- `if` / `then` / `else`
- `patternProperties`
- `propertyNames`
- `dependentRequired`
- `dependentSchemas`
- `contains`
- `prefixItems`
- tuple validation
- `format`
- `contentEncoding`
- `contentMediaType`
- unevaluated properties/items

### 2.2 Add recursive schema keyword invariant test

Add a test that walks all registered tool input schemas and fails on unsupported validation keywords.

Requirements:

- Include the tool name in failure messages.
- Include a JSON path such as `inputSchema.properties.foo.items.oneOf`.
- Do not treat property names under `properties` as schema keywords.
- Recurse into schemas under `properties`, `items`, `additionalProperties` if it is an object, and nested object/array schemas.
- Allow annotation keywords deliberately.
- Fail on unsupported validation keywords.

Suggested test location:

- `tests/mcp/test_schema_boundaries.rs`, or
- `tests/mcp/test_tool_coverage.rs` if that is where registry invariants currently live.

### 2.3 Add focused validator behavior tests

Add or verify focused tests for:

- `type` string and type arrays.
- `enum` and `const`.
- string `minLength`, `maxLength`, and `pattern`.
- numeric `minimum`, `maximum`, exclusive bounds, and `multipleOf` tolerance.
- object `required`, nested `properties`, and `additionalProperties: false`.
- array `minItems`, `maxItems`, `uniqueItems`, and homogeneous `items`.
- maximum validation depth.
- booleans not accepted as numbers in strict/native mode.

### 2.4 Fix schemas instead of broadening allowed keywords

If the new analysis/repo/patch/diagnostics schemas use unsupported keywords, prefer simplifying schemas to the supported subset. Only extend the validator if the unsupported keyword is genuinely necessary and can be implemented fully with tests.

Acceptance:

- Built-in schemas cannot silently use unsupported JSON Schema constructs.
- Docs and tests agree on the supported subset.
- New 78-tool surface passes the schema-boundary invariant.

## Workstream 3: Audit new deterministic analysis tools

The new tools landed quickly. Run a targeted correctness and contract review.

### 3.1 `patch_contract_check`

Verify:

- It handles malformed patches safely.
- It detects lockfile, manifest, CI, generated/minified, large delete, scope escape, public API-like, and missing-test-risk conditions as documented.
- It does not claim semantic code-review correctness.
- It uses bounded input checks.
- It returns stable finding codes and machine codes.
- It is placed in coherent profiles.

Add tests if missing:

- Malformed hunk.
- Multiple files with mixed risk.
- Patch touching only docs.
- Patch touching lockfile only.
- Patch touching manifest plus source.
- Scope-denied path.

### 3.2 `test_command_suggest`

Verify:

- It suggests commands only from supplied repo tree/manifests.
- It does not execute commands.
- It does not hallucinate package scripts not present in supplied `package.json`.
- It marks script-runner commands as requiring preflight/review.
- It handles polyglot repos.
- It degrades gracefully for malformed manifests.

Add tests if missing:

- Rust-only project.
- Python-only project.
- Node project with scripts.
- Node project without scripts.
- Go module.
- Polyglot repo.
- Unknown repo.

### 3.3 `repo_language_detect`

Verify:

- Scoring is deterministic.
- Vendored/noisy paths do not dominate results without warnings.
- Lockfiles and manifests are weighted more strongly than random file extensions.
- Recommended profiles are valid registry profile names.

### 3.4 `import_export_inspect`

Verify:

- Rust, Python, JS/TS, and Go fixtures cover aliases, multiline cases, comments, star/blank imports, and malformed partial source.
- Output line numbers are stable.
- It reports confidence/limitations.

### 3.5 `code_block_map`

Verify:

- Brace-depth and indentation heuristics do not panic on malformed input.
- Markdown heading/fence behavior is deterministic.
- Large files obey budgets.
- It clearly reports heuristic confidence.

### 3.6 `symbol_name_diff`

Verify:

- It uses `code_block_map` consistently.
- It handles reorder-only changes without false add/remove where feasible.
- Rename detection has a configurable threshold or conservative defaults.
- It reports limitations.

### 3.7 `lockfile_inspect`

Verify:

- It does not query registries.
- It handles malformed lockfiles safely.
- Cargo/npm/go paths are reasonably structured.
- pnpm/yarn/uv/poetry heuristic behavior is documented as heuristic.
- Large churn is detected.

Acceptance:

- Each new tool has at least representative happy-path, malformed-input, and budget/large-input tests.
- Every new tool has generated docs and profile placement.
- Every heuristic output includes confidence, limitations, warnings, or equivalent caution fields.

## Workstream 4: Profile, exposure, and diagnostics consistency

Recent commits added invariant tests and diagnostics tools. This workstream validates that they fully reflect the new 78-tool surface.

### 4.1 Profile/exposure audit

Verify:

- Every non-hidden tool is in at least one named profile.
- No deprecated tool is present in codegg profiles unless intentionally grandfathered.
- No harness-only tool appears for `ToolAudience::Model`.
- Route-critical tools are not hidden.
- Heavy-cost tools are composite or have a documented reason.
- New analysis tools have appropriate cost classes.
- New analysis tools are not accidentally hidden from intended profiles.

### 4.2 Diagnostics shape stability

Verify:

- `runtime_diagnostics` returns stable fields documented in schemas.
- `profile_inspect` returns correct profile info for all named profiles.
- `tool_availability_explain` handles:
  - known model-safe tool available in model profile;
  - known harness-only tool unavailable to model audience;
  - known tool outside selected profile;
  - unknown tool with close-match suggestion;
  - unknown profile/audience input.
- Diagnostics do not expose env var values, secrets, absolute home paths, or file contents.

### 4.3 CLI diagnostics consistency

Verify:

- `eggsact --diagnostics --format json` parses.
- CLI diagnostic tool count matches registry count.
- Tool count in README/docs matches CLI diagnostics.
- Route-critical tool list matches registry constants.
- Generated asset status does not require network or command execution.

Acceptance:

- Diagnostics can explain missing/uncallable tools without reading repo files.
- Profile table/docs and actual registry agree.
- No diagnostics output leaks sensitive local information.

## Workstream 5: Generated docs and package-content closure

### 5.1 Generated docs

Run:

```bash
cargo run --bin generate-docs -- --check
```

If it fails, regenerate:

```bash
cargo run --bin generate-docs
```

Then inspect diffs. Commit generated docs only if they match registry/spec changes.

Verify generated docs include:

- 78 tools.
- 20 categories.
- `analysis` category.
- New diagnostic tools.
- New patch/repo tools.
- Updated profile tables.

### 5.2 Package contents

Run:

```bash
cargo package --list
cargo package --verbose
```

Verify package includes:

- Required source files under `src/`.
- `src/tools/analysis.rs`.
- `src/mcp/schemas/analysis.rs`.
- `src/mcp/specs/analysis.rs`.
- generated source data required at build time, especially `src/text/confusables_generated.rs` if used.
- README and license.

Verify package excludes:

- `plans/`.
- `.github/`.
- development scripts/data intentionally excluded in `Cargo.toml`.

If package excludes required runtime/build files, fix `Cargo.toml` include/exclude configuration.

Acceptance:

- Generated docs are synchronized.
- Package builds from packaged contents.
- Package contents match documented policy.

## Workstream 6: CI and release checklist

### 6.1 CI visibility

Check whether GitHub Actions runs on `main` commits. If not, inspect workflow triggers and branch filters.

If no workflow run appears for recent `main` commits, determine whether:

- workflow is disabled;
- triggers only run on pull requests;
- branch filters exclude `main`;
- GitHub connector cannot see runs;
- repository settings block Actions.

Document the result.

### 6.2 CI matrix alignment

Ensure CI includes or intentionally delegates:

- fmt check;
- generated docs check;
- clippy with `-D warnings`;
- lib tests;
- bin tests;
- integration tests with parity skipped;
- doctests;
- package check;
- schema-boundary invariant tests;
- route-critical contract tests;
- profile/exposure invariant tests.

### 6.3 Release checklist

Add or update `docs/release.md` if not present.

Required sections:

- version bump procedure;
- generated docs procedure;
- full verification command list;
- package-content check;
- parity test policy;
- cargo publish dry run;
- post-release tag/checklist.

Acceptance:

- CI behavior is understood and documented.
- Release checklist exists and matches actual commands.
- Local full matrix passes.

## Workstream 7: Parity documentation and accepted failures

Recent commit messages mention parity results with accepted failures. Ensure this is documented precisely.

Tasks:

- Review `tests/fixtures/accepted_parity_failures.txt`.
- Confirm newly accepted failures are genuinely pre-existing or intentionally accepted.
- Document why each accepted failure exists, or group them by category with rationale.
- Ensure parity docs explain how to run parity locally.
- Ensure CI skip behavior is documented.

Acceptance:

- Accepted parity failures are auditable.
- No new parity regression is silently accepted without rationale.
- Parity docs match actual test commands.

## Workstream 8: Final corrective review checklist

Before closing, verify:

- `README.md` says 78 tools / 20 categories if that remains true.
- `architecture/tools.md` agrees with README.
- `generated/tool-cards.md` includes all new tools.
- `src/mcp/registry/all_tools.rs` includes `analysis::TOOLS` or equivalent.
- New schemas are included in schema module exports.
- New specs are included in spec module exports.
- `machine_codes::ALL` includes new emitted codes.
- Route-critical tools remain exactly documented unless intentionally changed.
- New tools do not become route-critical accidentally.
- `full` profile includes the new tools if expected.
- codegg profiles include intended new tools.
- MCP tool listing works at compact, normal, and full schema detail.
- Diagnostics tools are harness-only if intended.

## Suggested commit structure

Prefer small commits:

1. `test(schema): add registered schema keyword invariants`
2. `docs(schema): document supported schema validation subset`
3. `fix(schemas): align tool schemas with supported subset`
4. `test(analysis): add corrective fixtures for new coding-agent tools`
5. `test(diagnostics): tighten profile and availability diagnostics coverage`
6. `docs(release): add release checklist and package audit notes`
7. `ci: align release verification matrix`

Do not bundle all changes into one large commit unless the patch is very small.

## Required final verification

Run and capture results:

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

Optional but recommended:

```bash
cargo publish --dry-run
```

Parity, if Python reference is available:

```bash
cargo test --test lib parity
```

## Acceptance criteria

This corrective pass is complete when:

- Schema-boundary invariant tests exist and pass for the 78-tool registry.
- Supported/unsupported schema keywords are documented.
- New analysis tools have corrective fixtures for malformed input, core happy paths, and budget/large-input behavior.
- Profile/exposure invariants pass.
- Diagnostics tools have stable shape tests and do not leak sensitive data.
- Generated docs are synchronized.
- Package contents are verified.
- CI behavior is either green or its absence is understood and documented.
- Release checklist exists and is accurate.
- Full local verification matrix passes or any remaining failure is explicitly documented with a fix plan.

## Handoff notes

The most important part of this pass is not adding functionality; it is creating reliable guardrails around the large new tool surface. If time is limited, prioritize schema-boundary invariants, generated-doc verification, package verification, and full local test evidence before adding any extra polish.
