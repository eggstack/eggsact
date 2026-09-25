# eggsact Long-Term Implementation Roadmap

Status: execution roadmap for `plans/000-long-term-specification.md`

Terminology: `plans/001-terminology-and-domain-model.md`

This roadmap orders the work needed to keep eggsact a deterministic,
bounded, local utility layer while modernizing its MCP presentation and
distribution. Each phase MUST leave the repository in a coherent state
and MUST include focused implementation plans, migrations where
applicable, tests, documentation, and closure evidence before the next
dependent phase is treated as available.

The roadmap is dependency-ordered, not calendar-ordered. Parallel work is
appropriate only where the dependency notes allow it.

## Cross-phase execution rules

Every phase MUST:

1. preserve the single-crate and single-`ToolSpec`-source invariants;
2. preserve profile/audience/exposure filtering and stable machine codes;
3. use explicit inputs for deterministic utils (no clock, TZ db, env, net);
4. keep limits/truncation contractual with `limits_applied` reporting;
5. keep generated docs in sync (`generate-docs --check` green);
6. include truncation, cancellation, oversize, malformed-input, and
   cross-era tests where applicable;
7. update architecture docs and release-contract guards with code;
8. leave direct mode as the 1.x default until evidence-backed rollout
   gates pass;
9. record explicit exit evidence in the implementation plan or closure
   record.

## Phase 0 — Deterministic substrate and registry integrity

### Objective

Protect the typed-first composition hierarchy and the 86-tool registry
as the foundation for all later work without changing user behavior.

### Deliverables

- Formalize `ToolSpec` -> schema -> `ToolRegistry` -> `preflight` ->
  MCP server ownership in `docs/library-api.md` and
  `architecture/overview.md`.
- Keep `RepoFacts`/`PatchAnalysis` as canonical ecosystem/diff
  classifiers with differential anti-drift tests.
- Keep `config_file_inspect` heuristic-only for YAML and
  `config_preflight` YAML exclusion.
- Keep `compile_regex()` engine selection with `engine_used` reporting.

### Dependencies

None beyond the current single-crate baseline.

### Exit criteria

- New production code follows `calc`/root -> typed `text` ->
  `ToolRegistry`/contexts -> typed `preflight` -> MCP server.
- No new adapter-to-adapter calls outside the three documented
  same-module reuses.
- Registry sync test and generated docs checks pass.

### Required tests

- `tool_registration_tables_are_in_sync`;
- differential repo/patch semantics;
- limits/truncation reporting;
- regex engine selection;
- YAML heuristic vs parser mode.

## Phase 1 — MCP presentation modernization and discovery evaluation

### Objective

Keep all underlying deterministic capabilities while adding a smaller,
searchable, protocol-current presentation surface with evidence-backed
rollout gates.

### Deliverables

- Legacy + `2026-07-28` modern envelopes, `server/discover`, modern
  cache/result metadata, Tool annotations, `structuredContent`.
- `McpSurface::{Direct, Discovery}` with five profile-filtered pinned
  front doors plus MCP-only `tool_search` / `tool_invoke`.
- Race-safe one-era-per-stdio-connection state; `-32022` cross-era
  behavior; mismatched-notification drop before side effects.
- Deterministic discovery evaluation: registry-derived semantic
  coverage, retrieval gates, scenario corpus with containment split,
  offline trace scorer, byte-ratio gate.
- Recorded direct-vs-discovery traces through real relevant clients
  for at least one current OpenAI coding/agent model and one current
  Anthropic coding/agent model, plus a server-instructions A/B and the
  rollout/default decision.

### Dependencies

Phase 0 registry integrity. No capability redesign.

### Exit criteria

- Deterministic gates green: 100% stable-Model coverage, retrieval
  top-1 >=90% / top-3 >=98% / top-5 100%, 0 Model->HarnessOnly leaks,
  discovery/direct byte ratio <=25%, >=40 Model task scenarios after
  containment split.
- External gates recorded: OpenAI pair, Anthropic pair,
  server-instructions A/B, integration/default decision.
- Generated integrations stay on direct until the above pass.

### Required tests

- `tests/mcp/test_discovery.rs` (15 tests + expanded corpus);
- era-classification and cross-era guards;
- profile/audience enforcement on search/invoke;
- scorer plural-contract and `--pair` delta gates.

## Phase 2 — Harness integration and documentation generation

### Objective

Make the in-process library the first-class harness surface with typed
wrappers and generated docs that cannot drift.

### Deliverables

- `agent::ToolRegistry`, `ExecutionContext`, and typed `preflight`
  wrappers as the documented import surface.
- `DependencyPreflight` typed facade; patch-review/repo-audit facades
  evaluated and declined where projections already answer the workflow.
- `generate-docs` pipeline with `--check` in CI; never hand-edit
  generated registry facts, tool cards, profile blocks, or confusables.
- Parity baseline maintenance (37 accepted C1-C6 failures); only
  non-listed failures are regressions.

### Dependencies

Phase 0. May proceed in parallel with Phase 1 after Phase 0.

### Exit criteria

- `docs/library-api.md` hierarchy matches implementation.
- Raw `tools::*`/`services::*`/`mcp` internals documented as 1.x-compat
  adapter internals.
- Parity regressions distinguishable from accepted failures.

### Required tests

- Registry/profile/audience unit tests;
- context isolation (clone vs persistent state);
- doc generation `--check`;
- parity suite with accepted-failure list.

## Phase 3 — Distribution, self-update, and release qualification

### Objective

Ship stripped standalone binaries on all five qualified targets with a
self-contained updater and a reproducible manual release gate.

### Deliverables

- Five-target binary matrix with staged version/help/MCP smoke.
- Exact-tag Unix installer verification with checksum sidecars.
- In-process `eggfetch-core` self-update transport with the pinned
  feature/trust profile; bootstrap installers keep external download
  tooling.
- `scripts/check-release-contract.py` invariant guards; `cargo-deny`,
  MSRV 1.89, and `scripts/release-check.sh` qualification.

### Dependencies

Phases 0-1 for registry/protocol stability. No library/MCP network API.

### Exit criteria

- All five targets smoke-tested; installer payloads verified.
- `eggsact update` needs no post-install `curl`; Windows staged
  replacement documented as staged.
- Release contract, cargo-deny, and MSRV green.

### Required tests

- 16+ updater unit/integration tests (policy, status/redirect/
  streaming/timeout/proxy-config/no-curl guards);
- release-contract script;
- release build + MCP smoke (77 tools);
- native Windows/macOS supported-platform checks via maintenance lane.

## Phase 4 — Performance evidence without behavior change

### Objective

Retain hot-path improvements with honest, non-gating benchmark evidence
and no public shape, schema, policy, protocol, ranking, determinism, or
bounded-execution change.

### Deliverables

- Response/list/search/JSON/text-replace/patch hot-path work behind
  differential correctness tests (nonzero hunks, CRLF, EOF truncation,
  newline-boundary matrix).
- Representative workloads (multi-span diffs, 100/1,000-path repo
  facts, production input-budget path, persistent warm MCP calls).
- Explicit decline records where caching adds no benefit (e.g.
  per-pattern regex metadata reuse).

### Dependencies

Phases 0-2. Never gates the merge.

### Exit criteria

- Public Rust/MCP shapes, schemas, policy, protocol, ranking,
  determinism, and bounded semantics unchanged.
- `architecture/performance.md` boundary and patch contracts honored;
  no host-specific timing thresholds in ordinary tests.

### Required tests

- Focused text/diff/repository/regex suites;
- detached-baseline newline differential matrix;
- merge gate + release-contract + MCP smoke unchanged.

## Recommended immediate execution sequence

```text
Phase 0  substrate and registry integrity (continuous guard)
Phase 1  MCP discovery evaluation closure (active: 03c external evidence)
Phase 2  harness integration and docs generation (continuous guard)
Phase 3  distribution and self-update qualification (closed; maintenance only)
Phase 4  performance evidence (closed; non-gating future patches only)
```

The only active milestone at adoption is the Phase 1 `03c` external
model/client evidence closure. Phases 3-4 are closed lines under
maintenance; new work there requires a corrective plan, not a silent
roadmap edit.

## Roadmap governance

Implementation plans derived from this roadmap SHOULD cite the exact
phase and specification sections they satisfy. A phase is not complete
because code exists; it is complete only when its ownership model,
compatibility, tests, documentation, failure semantics, and closure
evidence are present.

When implementation reveals that a term or ownership boundary is wrong,
update the terminology document and long-term specification first, then
adjust the roadmap. Avoid accumulating incompatible local meanings in
phase plans.

New scope SHOULD be evaluated against the non-goals in the
specification. Features that do not strengthen deterministic local
utility, bounded MCP presentation, harness integration, or qualified
distribution SHOULD not displace the roadmap's core work.
