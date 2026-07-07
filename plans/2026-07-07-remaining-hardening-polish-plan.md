# Remaining Hardening and Polish Plan

Date: 2026-07-07

Repository: `eggstack/eggsact`

Related roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Purpose

This plan captures the remaining hardening and polish work after the initial roadmap execution. The repository has already advanced through documentation repositioning, runtime configuration hardening, route-critical fixture tests, and command-preflight improvements. The remaining work is now less about broad architectural correction and more about closure: schema-boundary enforcement, implementation verification, new deterministic coding-agent tools, profile/exposure cleanup, diagnostics completeness, and release gates.

The work should preserve the project constraints: local deterministic execution, no network-backed tool behavior, no command execution inside analysis tools, bounded resource usage, profile/audience separation, machine-readable route contracts, and generated documentation as a source of release consistency.

## Current state summary

Completed or substantially completed:

- Documentation no longer presents the crate primarily as a calculator.
- Coding-agent integration and generated-assets documentation exist.
- Runtime schema detail handling was hardened.
- Runtime diagnostics expose more active configuration and limits.
- Heavy/moderate handlers gained cooperative budget checks.
- Route-critical tools gained fixture-backed contract tests.
- `command_preflight` gained wrapper detection, script-runner handling, environment mutation scanning, and policy matrix tests.

Remaining:

- Milestone 5 schema validation boundary enforcement is still the most immediate closure item.
- Milestone 6 deterministic coding-agent tools are still planned feature work.
- Milestone 7 profile/exposure ergonomics should be revisited after tools are added.
- Milestone 8 diagnostics/self-inspection should be completed after profile/tool additions.
- Milestone 9 release/CI closure should be the final gate.

## Priority order

### Priority 0: Establish a clean baseline

Before implementing the remaining roadmap items, run the full verification matrix and capture any failures in a short release-readiness note.

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

If `verify-eggsact` duplicates some of the individual steps, still run the individual commands during this pass so failures can be isolated.

Acceptance:

- All commands pass locally.
- Any known skipped parity behavior is documented.
- Any warnings are either fixed or captured as explicit follow-up items.

### Priority 1: Finish schema-boundary hardening

Implement milestone 5 before adding new tools. New tool work will add new schemas; schema-boundary invariants should exist first so future additions are checked automatically.

Key deliverables:

- Document supported schema subset.
- Document unsupported JSON Schema constructs.
- Add a recursive schema-keyword invariant test across all registered tool input schemas.
- Add focused tests for supported keyword behavior.
- Add developer guidance for adding schemas safely.

Acceptance:

- Built-in schemas fail tests if they use unsupported validation keywords.
- Failure messages include tool name and JSON path.
- Docs do not imply full JSON Schema support.

### Priority 2: Corrective pass over recent runtime/route/command work

The recent hardening commits changed important behavior. Run a targeted review pass before adding larger tool features.

Review areas:

- Ensure `command_preflight` wrapper detection does not over-review benign commands without explanation.
- Confirm `python -m pytest` behavior is acceptable. If it is now review rather than allow, docs and fixtures should explain why.
- Verify `npm test`, `npm run`, `make`, `just`, and `task` are classified consistently across policy modes.
- Verify `EGGCALC_MCP_SCHEMA_DETAIL` warning behavior does not pollute JSON-RPC stdout. Warnings must go to stderr only.
- Verify cooperative budget checks do not create false cancellations for normal-sized inputs.
- Verify route-critical tests cover both successful and blocking paths for each route-critical tool.

Acceptance:

- No route-critical fixture depends on brittle human-readable prose.
- Warnings never corrupt MCP stdout.
- Common coding-agent verification commands have documented preflight behavior.

### Priority 3: Implement deterministic coding-agent tools in dependency order

Implement milestone 6 incrementally. The safest order is:

1. `patch_contract_check`
2. `test_command_suggest`
3. `repo_language_detect`
4. `import_export_inspect`
5. `code_block_map`
6. `symbol_name_diff`
7. `lockfile_inspect`

Rationale:

- `patch_contract_check` builds directly on existing patch/diff and route-critical work.
- `test_command_suggest` provides immediate coding-agent value without execution.
- `repo_language_detect` supports `test_command_suggest` and profile recommendation.
- `import_export_inspect` and `code_block_map` provide lightweight code navigation primitives.
- `symbol_name_diff` depends naturally on `code_block_map`.
- `lockfile_inspect` has broader ecosystem-specific edge cases and should follow the simpler deterministic inspectors.

Acceptance:

- Every tool has spec, schema, output schema, generated docs, tests, profile assignments, and bounded input handling.
- No tool performs network access or executes commands.
- Every heuristic output includes confidence or limitations where appropriate.

### Priority 4: Profile and exposure audit

After new tools land, run milestone 7. Do not finalize profile docs before tool placement is complete.

Audit each tool for:

- `tier`
- `profiles`
- `tags`
- `exposure`
- `harness_use`
- `cost`
- `stability`
- `composite`
- aliases
- route-critical status if applicable

Acceptance:

- Model-facing profiles do not expose harness-only tools.
- Harness workflows have access to required preflight tools.
- Generated docs explain profile selection by workflow.

### Priority 5: Diagnostics and self-inspection polish

After profile/tool additions, finish milestone 8.

Key deliverables:

- Runtime diagnostics include active profile, active audience, schema detail, limits, route-critical tool list, profile tool count, generated asset status, and budget summary.
- Profile inspection can explain intended profile purpose, representative tools, and audience expectations.
- Troubleshooting docs explain why a tool may not appear or may not be callable.

Acceptance:

- A codegg-style harness can diagnose profile/audience/schema mismatches without reading repo files.
- Diagnostics remain machine-readable and stable.

### Priority 6: Release and CI closure

Milestone 9 should be the final closure pass.

Key deliverables:

- CI covers schema-boundary invariants.
- CI covers route-critical fixture contracts.
- CI covers generated-doc drift.
- CI covers doctests if practical.
- Package contents are audited.
- Release checklist is committed.

Acceptance:

- CI and local verification agree.
- Release checklist is short, concrete, and executable.
- Package includes all required generated assets.

## Known risks and mitigations

### Risk: schema-boundary tests break existing schemas

Mitigation: Start the schema traversal test in report mode locally, inspect all unsupported-keyword findings, then decide whether each keyword should be allowed as annotation, removed, or implemented.

### Risk: command preflight becomes too conservative

Mitigation: Preserve the three policy modes and document intended behavior. If a common coding command moves from allow to review, ensure the finding explains the reason. Avoid marking opaque script runners as allow just because they are common.

### Risk: new coding-agent tools become parser projects

Mitigation: Each new tool must be explicitly heuristic unless it operates on simple structured formats. Avoid full language parsing. Prefer line ranges, confidence, and findings over semantic claims.

### Risk: generated docs drift during tool additions

Mitigation: Run `cargo run --bin generate-docs -- --check` after every tool-spec change. Commit generated output with the same change that modified the registry.

### Risk: profile exposure regressions

Mitigation: Add invariant tests for model-safe listings and harness-only tools. At minimum, assert that known harness-only tools do not appear for `ToolAudience::Model`.

## Suggested handoff sequence

1. Implement milestone 5 schema-boundary enforcement.
2. Run the baseline verification matrix.
3. Implement `patch_contract_check` as the first milestone 6 tool.
4. Re-run route-critical and generated-doc tests.
5. Implement `test_command_suggest` and `repo_language_detect`.
6. Add profile/exposure invariants.
7. Implement remaining code-inspection tools.
8. Complete diagnostics/profile inspection.
9. Run release/CI closure.

## Definition of done

The remaining hardening and polish work is complete when the crate has schema-boundary invariants, a verified clean CI/local matrix, deterministic coding-agent inspection tools with tests/docs/profiles, coherent profile and exposure assignments, self-diagnostic tooling sufficient for codegg integration, and a repeatable release checklist that prevents documentation, generated assets, package contents, and route-critical contracts from drifting.
