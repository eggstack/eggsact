# eggsact agent-tooling correctness and utility roadmap

## Context

This roadmap covers the next line of work for `eggsact`: tightening correctness, documentation, and integration ergonomics while expanding the deterministic tool surface that is most useful to coding agents. The repository has moved beyond its original framing as a natural-language calculator. It is now a deterministic MCP and in-process utility layer for codegg-style agents, with tools spanning math, text, JSON, regex, validation, paths, shell command preflight, Markdown, config, Unicode, identifiers, versions, TOML, patches, Cargo, dependency changes, repository manifests, and diagnostics.

The immediate review found the core architecture sound: tool declarations are centralized through `ToolSpec` files under `src/mcp/specs/`, aggregated by `src/mcp/registry/all_tools.rs`, and filtered through profile/audience-aware registry APIs. The in-process `ToolRegistry` and `ExecutionContext` are the right direction for codegg because they avoid MCP stdio overhead and allow explicit profile, audience, budget, cancellation, request, source, compatibility, and calculator-state control.

The major remaining risks are release-polish and correctness issues rather than architectural reversal. The generated README/tool documentation appears stale or malformed, some hand-maintained architecture text has drifted from the implemented server and registry, `config_file_inspect` has a likely UTF-8 byte-slicing panic in secret preview masking, budget naming mixes bytes and characters, and MCP cancellation/concurrency semantics are easy to overstate. The best feature work is a narrow expansion of deterministic repo/diff/dependency/command-sequence utilities that reduce model-side parsing and improve agent routing.

## Objectives

1. Restore trust in the docs and release artifacts by ensuring generated blocks, tool counts, profile references, tool cards, and architecture prose match the registry and server implementation.
2. Harden route-critical and externally callable tools against panics, Unicode edge cases, malformed input, oversized input, and ambiguous byte/character limit semantics.
3. Make the codegg integration path unambiguous: in-process registry and typed wrappers for hot-path calls, model-safe tool listing for model-visible tools, harness audience only for automatic safety gates.
4. Clarify or improve MCP runtime semantics for cancellation, timeout, and serial versus concurrent request handling.
5. Add deterministic tools that materially improve coding-agent effectiveness: repo tree summarization, diff risk classification, batch path scope checks, lockfile/dependency inspection, and command sequence preflight.
6. Preserve compatibility unless a breaking change is explicitly justified and versioned.

## Non-goals

This is not a plan to turn eggsact into a general static analyzer, a networked package reputation service, a shell executor, or an agent framework. New tools should remain local-only, deterministic, bounded, schema-driven, and safe to run on untrusted repository text supplied by the caller. Any ecosystem heuristics should produce structured findings and conservative verdicts rather than pretending to prove semantic correctness.

## Design principles

The registry is the single source of truth. Counts, profiles, generated docs, tool cards, route-critical lists, and public references should be generated from or tested against `ToolSpec` data wherever possible.

The in-process API is the preferred codegg integration path. MCP remains important for external clients, but codegg should avoid serial stdio and JSON-RPC overhead where it can call `ToolRegistry` directly.

Route-critical tools must be mechanically consumable. Their outputs should always carry stable `machine_code`, `verdict`, canonical `findings`, and optional `recommended_next_tool` fields. The codegg harness should not have to parse prose.

Agent tools should compress deterministic context. A good eggsact tool converts raw paths, diffs, manifests, lockfiles, config text, shell strings, or Unicode-heavy text into bounded structured evidence.

Failure modes should be explicit. Panics, silent nulls, misleading successful responses, and vague warnings should be replaced with structured `ToolResponse` errors and machine codes.

## Roadmap phases

### Phase 01: generated docs, metadata, and release-blocking drift

Repair generated documentation first. Regenerate and verify `README.md`, `architecture/mcp-server.md`, and `generated/tool-cards.md`. Ensure generated blocks have matched begin/end markers, no orphaned generated blocks, and no stale fixed counts such as `68 tools` where the registry now contains additional categories. Update package and README framing from calculator-first to deterministic coding-agent utility while retaining math as one supported category. Update architecture method lists to include implemented methods such as `ping` and `profiles/list` and align category tables with `ToolSpec` categories.

### Phase 02: correctness hardening and edge-case safety

Fix concrete correctness bugs and make failure behavior consistent. Replace UTF-8-unsafe secret masking in `config_file_inspect`; standardize byte-versus-character budget semantics; audit route-critical handlers for `unwrap`/`expect` paths that can panic on malformed input; add tests for malformed JSON/TOML/config text, Unicode-heavy inputs, generated-doc marker integrity, and route-critical response contracts.

### Phase 03: codegg API ergonomics and typed wrappers

Make the intended integration path obvious. Deprecate or strongly relabel legacy, not-model-safe listing APIs; document `available_tools_model_safe` and harness-audience listing; promote `ExecutionContext` as the canonical per-call state object; add typed wrappers for common codegg workflows such as edit, command, config, dependency, repo manifest, and text-security preflights.

### Phase 04: MCP runtime semantics and optional concurrency upgrade

Decide whether MCP stdio remains serial or gains true in-flight cancellation and concurrent request handling. If keeping serial semantics, document that cancellation notifications are mostly pre-start cancellation and timeout-triggered cooperative cleanup. If upgrading, restructure the read loop to continue reading while requests execute, track active request cancellation flags, and serialize stdout writes safely.

### Phase 05: repo and diff intelligence tools

Add the first new agent-utility tool batch: `repo_tree_summarize`, `diff_risk_classify`, and `path_batch_scope_check`. These tools should be bounded, local-only, file-system-free, and built from in-memory path/diff data supplied by codegg. They should emit structured routing evidence for reviewer and edit-harness agents.

### Phase 06: dependency and lockfile expansion

Add `lockfile_inspect` and extend `dependency_edit_preflight` to compose lockfile awareness when lockfile text is supplied. Support Cargo, npm/package-lock, pnpm, yarn, Poetry, uv, and Go checksum files as feasible. Focus on source changes, path/git/url dependencies, manifest-lockfile drift, install/build hooks, and suspicious package-name risks.

### Phase 07: command sequence preflight

Add `command_sequence_preflight` to analyze multi-command plans. Compose existing command preflight logic, then identify sequence-level hazards: network install followed by execution, destructive cleanup before verification, environment mutation, git state mutation, nested shells, redirections, background processes, and sensitive-path writes.

### Phase 08: route-critical contracts and machine-code discipline

Generate or maintain a route-critical contract document. Assert every route-critical tool emits `machine_code`, `verdict`, canonical findings, and valid recommended next-tool names. Add tests that fail when route-critical tools drift or omit envelope fields.

### Phase 09: performance and resource discipline

Audit duplicate scans and allocations in composite tools. Consolidate fingerprint/newline/config/path scanners where possible. Consider minimizing Tokio features or feature-gating MCP runtime if this does not complicate packaging. Add benchmarks for hot tools such as `text_security_inspect`, `edit_preflight`, `config_file_inspect`, `patch_summary`, `repo_manifest_inspect`, and new diff/repo tools.

### Phase 10: release polish and compatibility verification

Run and document the full verification matrix: format, clippy, tests, generated-docs check, verify binary, and package build. Prepare changelog notes grouped by correctness, docs, API, new tools, route-critical contracts, and compatibility notes. Ensure docs.rs-facing module docs explain CLI, MCP, in-process registry, and typed wrappers.

## Cross-phase acceptance criteria

All externally callable tools must be bounded by input size and runtime budget expectations. All route-critical tools must return structured verdict and machine-code data. Generated docs must be reproducible from the registry. No model-facing listing should expose harness-only tools by accident. New tools must include `ToolSpec`, schema builders, handler implementation, tests, profile membership, exposure classification, generated docs/tool cards, and at least one codegg-oriented example.

## Recommended execution order

Start with phases 01 and 02 before adding new tools. The stale generated docs and concrete UTF-8 masking bug are high-confidence corrective work. Then complete phase 03 so codegg integration semantics are stable. Phase 04 can either remain a documentation cleanup if serial MCP is acceptable or become a deeper runtime change if external clients require in-flight cancellation. Only after that should phases 05 through 07 add the new repo/diff/dependency/command tools.
