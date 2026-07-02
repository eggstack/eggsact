# eggsact Tightening Roadmap

## Purpose

This roadmap tightens eggsact after the current agent-substrate work. The repo is already moving in the right direction: it has a registry-backed MCP server, category-local tool specs, an in-process agent API, typed preflight wrappers, machine-code response fields, curated profiles, and CI. The next line of work should make those seams enforceable and durable enough for codegg to depend on directly while retaining general MCP support for other harnesses.

The target state is a deterministic local utility substrate with three stable use modes:

1. CLI calculator and utility binary.
2. General MCP stdio server for external harnesses and non-codegg clients.
3. In-process Rust library for codegg harness checks, preflight workflows, and low-latency deterministic operations.

MCP should remain an adapter over the same core registry and handlers, not the primary internal abstraction. codegg should call typed Rust APIs for high-frequency or harness-only checks and expose only small, contextual model-facing tool profiles.

## Tightening Principles

Enforce the architecture already documented. If profiles and audiences exist, they must affect dispatch, not just listing. If machine codes exist, callers must be able to route on them consistently. If typed wrappers exist, they should fail closed on contract drift instead of silently defaulting missing fields.

Keep compatibility at the adapter boundary. Python eggcalc parity is valuable for MCP compatibility and migration, but compatibility quirks should not leak into codegg-native APIs. Native calls should be strict, typed, deterministic, and explicit.

Prefer generated surfaces. Tool docs, profile tables, schema snapshots, and compact agent tool cards should derive from `ToolSpec` rather than manually maintained tables. This reduces drift as tools expand.

Separate execution success from workflow safety. `ok` should mean the tool executed successfully. A separate verdict/disposition should tell codegg whether to apply an edit, run a command, accept a config, or escalate to review.

Favor harness automation over broad model exposure. Most eggsact value comes from cheap deterministic checks the harness can run automatically. Ordinary coder agents should see a small tool list. Manager, reviewer, security, and repo-audit modes can opt into broader profiles.

## Phase Overview

### Phase 1: MCP Profile and Audience Enforcement

Fix the highest-priority correctness boundary. `tools/list` already resolves an effective profile, but `tools/call` must honor the same active profile and should not dispatch through a default full-profile registry. The in-process registry should also enforce audience during dispatch or explicitly rename audience as listing-only. The preferred outcome is dispatch enforcement: a model-audience registry cannot invoke harness-only tools by name.

Deliverables:

- MCP `tools/call` uses the active profile or a validated explicit per-call profile.
- `ToolRegistry::prepare_tool_call` checks profile and audience.
- Model-facing dispatch rejects harness-only tools.
- Harness dispatch can call harness-only tools intentionally.
- Tests cover profile/list/call consistency and exposure enforcement.

### Phase 2: Compatibility Boundary and Strict Native Mode

Separate eggcalc/Python compatibility behavior from eggsact-native behavior. MCP can retain compatibility quirks when necessary, but codegg-native calls should reject ambiguous coercions and rely on strict typed validation.

Deliverables:

- Runtime or validation policy for compatibility versus strict-native behavior.
- MCP adapter owns Python-parity coercions and error-shape quirks.
- In-process codegg APIs default to strict native validation.
- Documentation records all compatibility deviations.
- Tests prove strict/native and compatibility modes differ only where intended.

### Phase 3: Fail-Closed Typed Preflight Wrappers

Harden typed wrappers over edit, command, and config preflight. Current wrappers provide useful typed structs but still extract values from raw JSON with permissive defaults. Missing mandatory fields should become typed contract errors.

Deliverables:

- Versioned typed output structs for preflight tools.
- Distinct error categories: call error, tool rejection, contract violation.
- Mandatory fields deserialize strictly.
- Missing `machine_code`, verdict, or findings shape fails closed.
- Fixtures exercise malformed output contracts.

### Phase 4: Normalized Verdict, Finding, and Machine-Code Contract

Make the response contract reliable for codegg routing. `ToolResponse.ok` should describe handler execution, while workflow result is expressed through verdict, disposition, findings, and machine code. Composite/preflight tools should all return stable top-level verdict semantics.

Deliverables:

- Shared verdict/disposition/severity vocabulary.
- Structured `recommended_next_tool` object.
- Top-level verdict for all composite/preflight tools.
- Machine-code coverage tests.
- Compatibility preservation for existing response fields where needed.

### Phase 5: Generated Docs, Profile References, and Agent Tool Cards

Eliminate doc drift by generating registry-derived documentation. README tool tables, architecture profile tables, schema snapshots, machine-code references, and compact codegg tool cards should all be derived from `ToolSpec` and response-contract metadata.

Deliverables:

- Doc/tool-card generator command.
- Generated README and architecture blocks.
- Generated profile reference tables.
- Generated compact codegg tool cards.
- CI check that generated docs are current.

### Phase 6: First-Class codegg Edit Preflight

Promote edit preflight into a single harness API for model-authored edits. It should compose path scope checks, fingerprint verification, literal/patch/line-range checks, newline policy, Unicode policy, and structured findings.

Deliverables:

- Expanded `EditPreflightInput` with path, workspace root, file kind, fingerprint, newline policy, Unicode policy, and edit metadata.
- Unified verdicts such as `safe_to_apply`, `safe_with_warnings`, `stale_context`, `old_text_not_found`, `multiple_matches`, `patch_failed`, `line_range_invalid`, `path_scope_escape`, `unicode_risk`, and `blocked`.
- Concise TUI summary plus full diagnostic result.
- codegg harness integration target documented.

### Phase 7: Command Preflight Policy Engine

Turn command preflight into a deterministic policy classifier for shell execution. eggsact should classify and explain command risk; codegg remains responsible for actual enforcement.

Deliverables:

- Classification for destructive filesystem mutation, package install, network access, credential exposure, privilege escalation, background/daemon behavior, long-running processes, shell metacharacter risk, recursive traversal, test/build commands, and unknown executables.
- Verdicts: `allow`, `allow_with_confirmation`, `review`, `block`.
- Policy levels: strict, default, permissive.
- Typed codegg wrapper and fixtures.

### Phase 8: Repo, Config, and Dependency Inspectors

Expand deterministic repo-audit coverage without turning eggsact into a build system. Start from Cargo and add common manifest/config formats useful to codegg planning and review.

Deliverables:

- Stabilized `cargo_toml_inspect` output contract.
- Inspectors for `pyproject.toml`, `requirements.txt`, `package.json`, `.env`, GitHub Actions workflows, Dockerfiles, and lockfiles where practical.
- Repo-delta preflight that recommends which checks to run based on changed paths.
- Structural extraction and risk flags, not full dependency resolution.

### Phase 9: Runtime Budgeting, Cancellation, and Bounded Execution

Make resource bounds explicit and cooperative. Timeouts around blocking work do not necessarily stop underlying computation, so heavy tools need internal budgets and cancellation checkpoints.

Deliverables:

- Central `ToolBudget` policy.
- Cooperative limits for loop-heavy tools.
- Cancellation token plumbing for in-process and MCP execution.
- Limit reporting through `limits_applied`.
- Tests for cap behavior.

### Phase 10: Evaluator Context Isolation

Move calculator mutable state out of process-global flags and maps where practical. This improves embedded behavior for codegg and prevents MCP-safe mode from unexpectedly mutating global evaluator behavior.

Deliverables:

- `EvalContext` or `CalcRuntime` with deterministic settings, memory registers, variables, and PRNG state.
- `evaluate_with_context` internal API.
- CLI compatibility wrapper retained.
- MCP uses deterministic safe context.
- codegg can instantiate isolated calculator contexts.

### Phase 11: Fixtures, Fuzzing, and Compatibility Gates

Strengthen CI from general test coverage into contract enforcement. Every tool should have fixture coverage for normal success, invalid input, limit behavior, and finding-producing cases.

Deliverables:

- Per-tool fixture suite.
- Profile/audience snapshot expansion.
- Machine-code coverage tests.
- Property/fuzz tests for path normalization, line/column mapping, Unicode normalization, JSON extraction, patch parsing, shell splitting, and regex safety heuristics.
- CI gate for generated docs and tool cards.

### Phase 12: codegg Integration Pass

Wire eggsact into codegg in three layers: small model-visible profiles, harness-only automatic preflight, and manager/reviewer repo-audit helpers.

Deliverables:

- codegg uses `codegg_core_min` or similarly small model profile by default.
- codegg calls typed eggsact preflight APIs before edits, shell commands, config writes, path mutation, and suspicious ingress.
- codegg tracks local call counts, verdicts, machine codes, and latency.
- Model-facing tool exposure remains small and contextual.

## Recommended Execution Order

Execute phases 1 through 5 before expanding features. They fix the current correctness and maintainability risks: dispatch boundaries, compatibility boundaries, typed wrapper contracts, normalized response semantics, and generated documentation.

After those land, phases 6 through 8 provide the highest codegg value: edit preflight, command policy, and repo/config inspection. Phases 9 through 11 harden the runtime and compatibility gates. Phase 12 is the integration closeout.

## Milestone 1 Completion Target

The first milestone is complete when codegg can safely embed eggsact and call typed edit, command, and config preflight APIs with fail-closed contracts, while MCP `tools/list` and `tools/call` enforce the same profile/audience boundaries.

Minimum acceptance criteria:

- MCP cannot call out-of-profile tools under restricted active profiles.
- Model-audience dispatch cannot invoke harness-only tools by name.
- Typed wrappers fail closed on malformed outputs.
- Composite tools expose normalized verdicts and machine codes.
- Generated documentation/tool-card scaffolding exists and is CI-checkable.
