# Shared Repository and Patch Analysis Consolidation

Status: planned
Priority: P1
Scope: remove duplicated classification/parsing while preserving external tools

## Objective

Consolidate the repository- and patch-analysis families around shared typed fact models so multiple tools project different answers from one deterministic analysis instead of independently re-detecting ecosystems, paths, manifests, diff categories, and risk signals.

Keep the existing external tool surface unless compatibility policy separately authorizes retirement of a deprecated tool. The goal is one source of truth per classification domain, not fewer user-visible tools for its own sake.

## Problem statement

The repo-analysis family currently repeats evidence extraction and classification across `repo_manifest_inspect`, `repo_tree_summarize`, `repo_language_detect`, and `test_command_suggest`. `repo_manifest_inspect` has `detect_project_types()`, while `repo_tree_summarize` independently re-derives Rust/Python/Node/Go project types from manifest paths. The same module also contains separate command templates and language-extension tables.

The patch family has similar overlap. `patch_summary`, `patch_contract_check`, and `diff_risk_classify` all operate on the same unified-diff facts. `patch_contract_check` and `diff_risk_classify` independently parse the diff and classify file paths, while `diff_risk_classify` also computes a patch summary. Incremental additions risk semantic drift: the same path can eventually receive different ecosystem/risk classifications depending on which tool is called.

## Non-goals

- Do not merge `repo_manifest_inspect`, `repo_tree_summarize`, `repo_language_detect`, or `test_command_suggest` into one giant tool.
- Do not merge `patch_summary`, `patch_contract_check`, and `diff_risk_classify`; their questions are distinct.
- Do not add filesystem access. All analysis remains explicit-input and deterministic.
- Do not introduce language parsers, tree-sitter, git libraries, vulnerability databases, package registries, or network access.
- Do not turn heuristic classification into a claim of semantic source-code correctness.

## Part A — Canonical repository facts

### A1. Introduce a typed `RepoFacts` model

Create an internal typed representation in a location that does not depend on MCP/tool adapters. The exact module name is implementation-defined; prefer a narrow module such as `src/repo_analysis.rs` or a suitable leaf under `src/text/` only if its responsibilities are truly text/path-only.

`RepoFacts` should capture reusable observations, not policy verdicts. Candidate fields:

- normalized/bounded input paths;
- path buckets (source, tests, docs, configs, manifests, lockfiles, CI, generated, vendor, scripts, entrypoints);
- detected ecosystems/project types;
- language evidence/counts where deterministically derivable;
- manifest and lockfile paths grouped by ecosystem;
- entrypoint candidates;
- high-leverage paths;
- unknown/mixed indicators;
- raw evidence/reasons sufficient for downstream confidence reporting.

Do not put `ToolResponse`, machine codes, verdicts, profiles, or MCP metadata into `RepoFacts`.

### A2. Centralize manifest/ecosystem detection tables

Move Rust/Python/Node/Go manifest recognition and source hints into one canonical table/set. Ensure all repo tools consume it.

Preserve current semantics initially. If conflicting existing behavior is discovered, add a regression test for each path and choose one documented canonical behavior before changing output.

Acceptance: adding a newly recognized manifest requires editing one classification source, and every repo tool observes the change.

### A3. Centralize path bucketing

Create one reusable path classifier for repository-role categories. Avoid separate rules for `repo_tree_summarize` and diff classification when the categories mean the same thing; if patch risk needs a different category vocabulary, derive it from canonical path facts through an explicit mapping.

Separate raw classification from policy. For example, `is_ci`, `is_manifest`, `is_lockfile`, `is_generated`, `is_vendor`, `is_security_sensitive_path` are facts; whether CI changes require review is policy.

### A4. Convert repo tools into projections

Refactor:

- `repo_manifest_inspect` to format manifest/ecosystem portions of `RepoFacts`;
- `repo_tree_summarize` to format buckets/entrypoints/high-leverage paths and tree-specific findings;
- `repo_language_detect` to use shared extension/manifest evidence rather than independent project detection;
- `test_command_suggest` to consume detected ecosystems/facts and apply its command template table.

Command suggestions should remain a separate policy/projection layer. Do not embed suggested commands into the canonical fact model.

### A5. Differential tests

Create fixtures covering pure Rust, Python, Node, Go, mixed repos, unknown repos, monorepo-like nested manifests, generated/vendor-heavy trees, common CI/config paths, and ambiguous file extensions.

Assert shared project/ecosystem facts are identical across public tools where they expose the same concept. These are stronger than independent snapshot tests because they guard against future divergence.

## Part B — Canonical patch analysis

### B1. Introduce a typed `PatchAnalysis`

Build on the existing unified-diff parser. `PatchAnalysis` should own or reference one parse result and compute reusable facts once, including:

- parse success/error;
- changed file records and canonical effective paths;
- file/hunk counts;
- additions/deletions;
- renames and binary-patch status;
- line ranges;
- path-role classification using the canonical repository/path classifier where applicable;
- manifest/lockfile/CI/config/generated/vendor/security-sensitive flags;
- per-file deletion counts or other metrics needed by contract/risk policies.

Do not include final verdicts or policy settings in the fact model.

### B2. Refactor `patch_summary`

`patch_summary` becomes the neutral presentation of `PatchAnalysis`. Its existing public output and findings semantics should remain stable.

### B3. Refactor `patch_contract_check`

Consume `PatchAnalysis` and apply contract policy separately:

- workspace lexical scope checks;
- contract-relevant categories;
- large-deletion threshold;
- security/special-path rules currently in the contract tool;
- verdict/machine-code derivation.

Do not reparse the diff inside this tool after migration.

### B4. Refactor `diff_risk_classify`

Consume the same `PatchAnalysis`, then apply review-routing policy (`review_ci_changes`, dependency review, security-sensitive paths, docs-only allowance, etc.). Preserve `review_focus`, `recommended_next_tool`, verdict, and machine code behavior.

Do not call `patch_summary` as a tool adapter or independently parse the patch after migration. It may project the shared summary fields directly from `PatchAnalysis`.

### B5. Resolve overlapping path-risk semantics explicitly

Audit `patch_contract_check` and `diff_risk_classify` for similar concepts with different names/severity, especially manifests, lockfiles, CI, security-sensitive paths, generated/vendor files, and large diffs/deletions.

Keep policy differences when intentional, but encode the underlying classification once. Add comments/tests making the distinction clear: e.g. a security-sensitive path is a shared fact; `patch_contract_check` and `diff_risk_classify` may choose different verdicts based on different policy goals.

### B6. Property/fuzz opportunities

Without adding new heavy infrastructure, extend existing patch fuzz/property coverage so all public patch analyzers share parse acceptance/rejection and never disagree about neutral counts/path identity for the same parsed patch.

## Documentation updates

Update:

- `architecture/tools.md` with `RepoFacts`/`PatchAnalysis` layering;
- `architecture/overview.md` dependency flow;
- MCP docs only if wording needs to distinguish shared facts from tool-specific policy;
- comments in `src/tools/repo.rs`/`patch.rs` to identify adapters versus shared analysis.

Avoid exposing internal type names in user-facing docs unless they become public library API.

## Verification

Run the full ordinary verification sequence from `AGENTS.md`. Add focused tests first so behavior-preserving refactors are reviewable in small commits.

Recommended implementation order:

1. characterize current repo tool overlap;
2. add `RepoFacts` + tests;
3. migrate repo tools one by one;
4. characterize current patch overlap;
5. add `PatchAnalysis` + tests;
6. migrate patch tools one by one;
7. remove obsolete duplicate helpers;
8. update architecture docs and run full verification.

## Completion criteria

The plan is complete when repository ecosystem/path classification has one canonical implementation, patch parsing and neutral diff analysis happen once per operation, public tools are projections/policies over shared typed facts, cross-tool differential tests prevent semantic drift, and no external MCP or documented library behavior regresses.