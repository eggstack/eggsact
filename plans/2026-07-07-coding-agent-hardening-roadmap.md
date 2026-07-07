# Coding-Agent Hardening and Utility Expansion Roadmap

Date: 2026-07-07

Repository: `eggstack/eggsact`

## Purpose

This roadmap defines the next line of work for hardening `eggsact` as a deterministic MCP and in-process utility substrate for coding agents. The intended outcome is not a broad semantic code intelligence platform, an LSP replacement, or a remote service-backed analyzer. The intended outcome is a compact, deterministic, locally runnable tool suite that helps coding agents make safer decisions before editing files, changing dependencies, executing shell commands, modifying configs, or reviewing patches.

The current repo is already in strong shape for this direction. It has a single consolidated tool registry, category-local `ToolSpec` declarations, generated tool docs, explicit profile and audience filtering, an in-process `ToolRegistry`, an MCP stdio server, route-critical response contracts, machine-readable response codes, resource budgets, cancellation plumbing, and a CI workflow covering formatting, generated docs, Clippy, lib/bin/integration tests, and package creation.

This roadmap focuses on the remaining release-quality work: documentation coherence, runtime contract hardening, exact route-critical tests, stronger shell preflight fixtures, clearer schema validation boundaries, and a small set of deterministic coding-agent tools that fit the existing toolset.

## Non-goals

Do not add network-backed functionality to these tools. `eggsact` should remain usable offline and should not depend on package registries, search APIs, vulnerability feeds, LSP servers, model calls, embeddings, or external command execution.

Do not turn `eggsact` into a parser-heavy semantic engine. Lightweight heuristics are acceptable where they are explicitly labeled as such. Full AST correctness belongs in language-specific tooling or codegg integrations, not in this crate.

Do not expose harness-only safety tools directly to ordinary model-facing profiles. The existing audience model should remain the separation boundary between model-visible utilities and harness-driven preflight checks.

Do not weaken Python `eggcalc` compatibility where it is intentionally preserved for MCP behavior. Compatibility behavior should be documented and tested rather than removed opportunistically.

## Current strengths to preserve

The registry architecture is the correct foundation. Tool declarations live in category-specific spec files and are aggregated by the central registry. Continue using the registry as the single source of truth for names, handlers, schemas, profile membership, exposure, cost, stability, aliases, and generated docs.

The in-process API is a major advantage for codegg and other Rust consumers. `ToolRegistry` should remain the primary embedded integration point, with MCP stdio as the process boundary integration point.

The profile and audience model is appropriate. Model-facing code should default to smaller, safe tool lists. Harness code should call preflight and route-critical tools with explicit `ToolAudience::Harness` where appropriate.

The route-critical tool concept is the right abstraction. Tools that influence downstream action selection must have stable machine-readable contracts.

The resource-budget model should remain deterministic and conservative. Input limits, output truncation, cooperative cancellation, and explicit cost classes are more valuable here than maximum throughput.

## Milestone overview

### Milestone 1: Documentation and positioning cleanup

Reframe public docs so `eggsact` is consistently described as a deterministic utility/preflight substrate for coding agents. Fix drift between README, crate docs, architecture docs, generated docs, and crate metadata. Add generated-assets/parity workflow notes and an MCP integration guide for coding agents.

Deliverable file: `plans/2026-07-07-milestone-1-docs-positioning.md`

### Milestone 2: Runtime and configuration contract hardening

Validate `EGGCALC_MCP_SCHEMA_DETAIL` at startup, document all runtime environment variables, audit cooperative cancellation in heavy handlers, and add tests for invalid configuration and cancellation behavior.

Deliverable file: `plans/2026-07-07-milestone-2-runtime-config-hardening.md`

### Milestone 3: Route-critical contract tightening

Create exact fixture-backed tests for route-critical tools. Assert stable `ok`, `verdict`, `machine_code`, severity/disposition where applicable, and expected findings for safety-relevant inputs. Tighten permissive tests that currently allow unsafe cases to pass without exact failure assertions.

Deliverable file: `plans/2026-07-07-milestone-3-route-critical-contracts.md`

### Milestone 4: Shell and command preflight hardening

Build an adversarial command fixture suite and improve command classification around interpreter wrappers, package-manager scripts, `make`/`just`/`task`, shell indirection, pipe-to-shell patterns, and destructive git/filesystem flows.

Deliverable file: `plans/2026-07-07-milestone-4-command-preflight-hardening.md`

### Milestone 5: Schema validation boundary clarity

Document the supported schema subset, add invariant tests that tool schemas only use supported keywords, and prevent future contributors from assuming unsupported JSON Schema constructs are enforced.

Deliverable file: `plans/2026-07-07-milestone-5-schema-validation-boundaries.md`

### Milestone 6: Deterministic coding-agent tool additions

Add narrow deterministic tools that fit the existing philosophy: `patch_contract_check`, `test_command_suggest`, `import_export_inspect`, `code_block_map`, `symbol_name_diff`, `repo_language_detect`, and `lockfile_inspect`. Each must have a `ToolSpec`, schemas, generated docs, MCP tests, in-process tests, profile assignments, bounded input handling, and no network or command execution.

### Milestone 7: Profiles, exposure, and agent ergonomics

Audit all existing and new tools for correct tier, profile membership, exposure, tags, cost, stability, aliases, and composite flags. Add workflow-oriented profile docs and compile-tested examples for model-facing and harness-facing integrations.

### Milestone 8: Diagnostics and self-inspection improvements

Expand diagnostics so MCP clients and in-process consumers can determine active profile, audience, schema detail, tool counts, budget limits, generated asset status, compatibility mode, and profile contents without reading repo files.

### Milestone 9: CI and release closure

Add CI coverage for schema-keyword invariants, route-critical contracts, doctests, package-content checks, and release checklist validation. Preserve current generated-doc, Clippy, formatting, test, and package jobs.

## Recommended execution order

Start with milestones 1 through 5. They are hardening and contract work, not feature expansion. This reduces the chance that new tools are added on top of unclear docs or unstable response contracts.

After milestone 5, implement milestone 6 in this order:

1. `patch_contract_check`
2. `test_command_suggest`
3. `import_export_inspect`
4. `code_block_map`
5. `symbol_name_diff`
6. `repo_language_detect`
7. `lockfile_inspect`

The first three provide the highest utility for coding agents relative to implementation cost. `patch_contract_check` also builds directly on the route-critical and patch-review hardening from milestones 3 and 4.

## Cross-cutting requirements

Every new or modified tool must have an input schema, output schema, generated documentation, profile assignment, exposure assignment, cost assignment, route-contract tests if route-critical, MCP tests where applicable, and in-process tests where applicable.

Every heavy or moderate tool must honor resource budgets. Long loops should poll cancellation and deadline state at deterministic boundaries.

Every route-critical tool must return stable machine-readable fields on successful responses. Missing `machine_code` or `verdict` should be a test failure, not a tolerated edge case.

Every heuristic tool must say it is heuristic in its description and output. It should return confidence and findings rather than implying complete semantic correctness.

Every new coding-agent tool must operate only on provided input text, file paths, repo tree summaries, manifests, diffs, or lockfile contents. It must not read arbitrary local files unless the existing tool category already supports that behavior and the schema explicitly allows it.

## Definition of done

This roadmap is complete when `eggsact` consistently presents itself as a deterministic coding-agent utility substrate, runtime configuration is validated and documented, route-critical tools have exact fixture-backed contracts, command preflight has adversarial coverage, schema validation boundaries are explicit and enforced, and the first wave of deterministic coding-agent tools ships with full registry, schema, profile, doc, and test coverage.
