# Phase 12 Plan: Release Hardening and Long-Term Maintenance

## Purpose

Phase 12 prepares eggsact for a stable maintenance/release posture after the tightening work. Phases 01–09 hardened tool contracts and preflight behavior; phase 10 isolates execution context; phase 11 adds diagnostics and verification reporting. Phase 12 should make the crate easier to release, version, audit, and maintain without surprising downstream agent harnesses such as codegg.

This phase is intentionally conservative. It should stabilize what exists rather than adding new capability.

## Goals

- Formalize release gates and versioning policy.
- Lock down public API expectations.
- Validate crate packaging and generated artifacts.
- Improve dependency and supply-chain hygiene.
- Add maintenance docs for future tool additions.
- Define deprecation policy for tools, schemas, machine codes, and compatibility behavior.

## Non-Goals

- Do not add new MCP tools.
- Do not redesign the tool registry.
- Do not change public response contracts except for documented bug fixes.
- Do not remove compatibility shims without a deprecation path.
- Do not introduce remote telemetry or online dependency checks.

## Workstream A: Release Gate Standardization

### Required Work

Define the canonical release gate in one place and make all docs/scripts match it.

Canonical gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Optional environment-dependent gate:

```bash
cargo test --test lib parity
```

Only run the parity gate when the Python `eggcalc` reference is available.

Update:

- `release.sh`;
- `AGENTS.md`;
- `.skills/testing.md`;
- `.skills/release.md`;
- `docs/contributing.md`;
- README release/contribution references if present.

### Acceptance Criteria

- All release/check docs use the same canonical command list.
- Optional parity gate is clearly marked environment-dependent.
- `release.sh` matches the documented gate.

## Workstream B: Versioning and Compatibility Policy

### Required Work

Create or update a compatibility policy document, likely `docs/compatibility-policy.md` or `architecture/compatibility.md`.

Cover:

1. Semantic versioning expectations.
2. Public Rust API stability.
3. MCP tool name stability.
4. MCP input schema compatibility.
5. MCP output schema compatibility.
6. Machine-code stability.
7. Profile/audience compatibility.
8. CompatibilityMode behavior.
9. Deprecation timelines.
10. What constitutes a breaking change.

Recommended rules:

- Adding a new optional field is non-breaking.
- Removing or renaming a tool is breaking.
- Changing a machine code is breaking unless the old code remains as an alias.
- Tightening unsafe behavior may be allowed in minor releases if documented as security hardening.
- Hidden/harness-only tools have weaker public compatibility but should still be documented for codegg.

### Acceptance Criteria

- Compatibility policy exists and is referenced from `AGENTS.md` and docs.
- Maintainers have a clear rule for breaking versus non-breaking changes.

## Workstream C: Public API Surface Audit

### Required Work

Audit exported Rust API:

```bash
cargo doc --all-features --no-deps
```

Review:

- `lib.rs` re-exports;
- public modules;
- `ToolRegistry` API;
- preflight typed wrappers;
- `ExecutionContext` if phase 10 has landed;
- `ToolResponse`, machine-code helpers, and diagnostics APIs;
- accidental public internals.

Classify public items:

- stable public API;
- internal but public for integration/testing;
- deprecated/legacy;
- should be hidden or doc-hidden.

Add Rustdoc where missing for stable APIs.

### Acceptance Criteria

- Public APIs are intentional.
- Legacy APIs have deprecation notes where appropriate.
- Important public types have enough Rustdoc for downstream users.
- `cargo doc --all-features --no-deps` passes.

## Workstream D: Crate Packaging Audit

### Required Work

Run:

```bash
cargo package --list
cargo package --verbose
```

Review package contents for:

- missing README/LICENSE/docs;
- accidental large artifacts;
- generated data required at runtime;
- test fixtures that should or should not be packaged;
- plans directory inclusion policy;
- `.skills/` inclusion policy;
- `AGENTS.md` inclusion policy.

Decide whether to add `include`/`exclude` fields in `Cargo.toml`.

Recommended approach:

- include source, README, LICENSE, architecture docs, generated runtime data, generated tool cards if useful;
- exclude local reports under `target/`, generated verification outputs, editor files, and temporary plans if plans are not intended for crate consumers.

### Acceptance Criteria

- Package contents are deliberate.
- No large/unnecessary files are shipped.
- Required generated data is included.
- `cargo package --verbose` passes.

## Workstream E: Supply-Chain and Dependency Hygiene

### Required Work

Perform an offline/local dependency hygiene pass.

Commands where available:

```bash
cargo tree
cargo tree -d
cargo deny check   # if cargo-deny is adopted
cargo audit        # if cargo-audit is available and allowed in the environment
```

Do not require network access in CI unless explicitly configured.

Review:

- duplicate dependency versions;
- unused dependencies;
- default features that can be disabled;
- heavy dependencies only used in tests;
- license compatibility;
- `serde_json` `preserve_order` requirement;
- regex/fancy-regex safety assumptions.

If adopting `cargo-deny`, add a minimal config and document local use.

### Acceptance Criteria

- Dependency tree has been reviewed.
- Any intentional duplicate/heavy dependency is documented.
- Optional audit/deny tooling is documented without making offline workflows fail unexpectedly.

## Workstream F: Test Suite Maintenance

### Required Work

Review test organization after phases 01–11.

Tasks:

1. Ensure test names clearly map to features.
2. Remove duplicate tests that assert the same behavior without adding coverage.
3. Mark environment-dependent tests clearly.
4. Ensure parity tests fail with actionable messages when `eggcalc` is missing.
5. Ensure generated-doc tests are not brittle to harmless formatting changes.
6. Ensure cancellation/budget tests are deterministic and not timing-flaky.
7. Add a short test map to `.skills/testing.md` or `AGENTS.md`.

### Acceptance Criteria

- Test layout is discoverable.
- Environment-dependent tests are explicit.
- No known flaky timing tests remain.
- Focused test commands in docs are correct.

## Workstream G: Machine Code and Schema Freeze Review

### Required Work

Review machine codes and route-critical schema behavior before release.

1. Ensure every upper-snake finding/machine code used by route-critical tools appears in `machine_codes::ALL` or an equivalent registry.

2. Ensure docs list current machine-code categories.

3. Ensure deprecated aliases are documented.

4. Review output schemas for route-critical tools:

- `edit_preflight`;
- `command_preflight`;
- `config_preflight`;
- `patch_apply_check`;
- `text_security_inspect`;
- `config_file_inspect`;
- `dependency_edit_preflight` if treated as route-critical by codegg.

5. Add schema regression tests if missing.

### Acceptance Criteria

- Machine-code registry and usage are synchronized.
- Route-critical outputs are schema-stable.
- Codegg can route on verdict/machine_code/finding codes reliably.

## Workstream H: Documentation Finalization

### Required Work

Prepare documentation for maintainers and agent users.

Minimum docs:

- README quickstart;
- architecture overview;
- MCP server architecture;
- compatibility policy;
- diagnostics/verification docs from phase 11;
- tool-card docs;
- testing guide;
- release guide;
- AGENTS.md.

Checklist:

- no stale tool counts;
- no unverified claims like unconditional parity-pass status;
- no contradiction between release script and docs;
- generated docs are up to date;
- Windows command-platform limitation is documented;
- cooperative cancellation limitation is documented.

### Acceptance Criteria

- Docs are internally consistent.
- Generated docs check passes.
- Agent handoff docs are concise and accurate.

## Workstream I: Release Candidate Procedure

### Required Work

Define a release candidate process.

Recommended process:

1. Create release branch or tag candidate.
2. Run canonical release gate.
3. Run optional parity gate if Python reference is available.
4. Run package audit.
5. Inspect generated verification report.
6. Review public API docs.
7. Confirm version bump and changelog.
8. Tag release.
9. Publish only after package dry-run passes.

Add a checklist to `.skills/release.md` or `docs/release.md`.

### Acceptance Criteria

- Release candidate procedure is documented.
- The procedure includes rollback/failure handling.
- Version/changelog expectations are clear.

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo doc --all-features --no-deps
cargo package --verbose
```

Optional, if tools/environment exist:

```bash
cargo tree -d
cargo audit
cargo deny check
cargo test --test lib parity
```

## Final Acceptance Criteria

Phase 12 is complete when:

- release gates are standardized across scripts and docs;
- compatibility/deprecation policy exists;
- public Rust API surface has been audited;
- crate packaging contents are deliberate;
- dependency hygiene has been reviewed;
- tests are organized and documented;
- machine-code/schema registry is synchronized;
- docs are internally consistent;
- release candidate checklist exists;
- full verification passes or exact failures/environmental skips are documented.
