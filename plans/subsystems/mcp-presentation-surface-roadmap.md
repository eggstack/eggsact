# MCP Presentation Surface Roadmap

Status: active; 03c active (deterministic preparation done; external evidence blocked)

Long-term references:

- `plans/000-long-term-specification.md#2` (MCP as transport adapter)
- `plans/000-long-term-specification.md#4.5` (presentation is not capability)
- `plans/000-long-term-specification.md#5` (era pinning, no per-call profile)
- `plans/000-long-term-specification.md#7` (invariants 4, 7)
- `plans/001-terminology-and-domain-model.md#5` (profile, audience, surface)
- `plans/001-terminology-and-domain-model.md#6` (eras, discovery evaluation)
- `plans/002-long-term-roadmap.md#phase-1`

Related ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

## 1. Purpose and ownership boundary

Own the MCP presentation layer: protocol eras, `server/discover`,
modern metadata/annotations/`structuredContent`, the
`Direct`/`Discovery` surfaces with pinned front doors and
`tool_search`/`tool_invoke` facades, and the deterministic discovery
evaluation harness plus the evidence-backed rollout decision.

Consumes: deterministic substrate registry + profile/audience policy.
Must not own: capability semantics, registry membership, ranking-weight
redesign, or generated-integration defaults (defaults change only via
the `03c` rollout decision).

## 2. Work classification

### Invariants

- One-era-per-stdio-connection; cross-era `-32022`; mismatched
  notifications dropped before side effects.
- Discovery is presentation-only; search/invoke enforce the same
  profile/audience rules; facades are not `ToolSpec`s.
- Direct remains the 1.x default until rollout gates pass.

### Capabilities

- Protocol-modern envelopes coexisting with legacy revisions.
- Searchable five-door discovery surface with deterministic facades.

### Infrastructure

- `src/mcp/discovery_eval.rs` measurements; generated registry facts;
  `scripts/score-discovery-traces.py` offline scorer (never a
  substitute for recorded traces).

### Polish

- Description/alias corrections that clarify capability distinctions
  for models and humans (preferred over ranking-weight tuning).

## 3. Non-goals

- No capability redesign, registry expansion, or ranking-weight tuning
  to chase a benchmark.
- No model-provider SDK in runtime deps or ordinary CI; no keys,
  credentials, provider calls, or nondeterministic execution in merge CI.
- No embeddings/vector store/search service; no telemetry or persistent
  tracking.

## 4. Current state

Protocol/runtime/discovery implementation is complete (`35f7dc2e`,
`a5c00b8d`, `77ff57a5`, `121babde`). Deterministic evaluation
preparation is landed and retained (`40222999` + `03c` Parts A-C, G1):
full/Model direct is 77 tools / 111,911 bytes vs discovery 7 / 6,088
bytes (5.44%, gate <=25%); registry-derived semantic coverage 100% (76
stable targets, 88 positive intents, 9 containment, no bare-name
fixtures); retrieval top-1 ~96.6% / top-3 100% / top-5 100% with zero
leaks; 48 task + 4 containment scenarios; strict plural scorer with
`--pair` deltas and 2pp noninferiority gates; CI `34546867650` green;
`test_discovery` (15 tests) green.

## 5. Target architecture

Same presentation architecture with the `03c` evidence closure
recorded: exact model/client versions, success/selection/invalid-arg/
retry/call-count results, instructions A/B result, and the final
integration/default decision. Only then is the line closed and `03c`
eligible for pruning.

## 6. Dependency graph

```text
01 protocol modernization (closed)
    |
    +--> 02 progressive discovery (closed)
              |
              +--> 02c contract corrective (closed)
                        |
                        +--> 02d era classification (closed)
                                  |
                                  `--> 03c evaluation closure (active/blocked)
```

02c/02d are hard predecessors of 03c. External model/client evidence is
an operational dependency of 03c Parts D-F.

## 7. Milestones

### Milestone 01 — Protocol modernization

Class: capability + infrastructure

Objective: modern envelopes coexist with legacy.

Dependencies: none.

Deliverable boundary: `2026-07-28` envelopes, `server/discover`,
metadata, annotations, `structuredContent` over existing paths.

User or operator value: protocol-current clients interoperate.

Exit conditions: closed (commit `35f7dc2e`).

Deferred work: none.

### Milestone 02 — Progressive discovery

Class: capability

Objective: smaller searchable surface over the same capabilities.

Dependencies: 01 (hard).

Deliverable boundary: five pinned front doors + facades;
profile/audience authoritative; direct default.

User or operator value: lower-context tool access where allowed.

Exit conditions: closed (`a5c00b8d` + `02c/02d` correctives).

Deferred work: rollout default deferred to 03c.

### Milestone 03c — Evaluation closure corrective

Class: infrastructure + invariant (evidence honesty)

Objective: close the evidence gap without reopening protocol or
capability architecture.

Dependencies: 02 series (hard); external model/client access
(operational, currently blocked).

Deliverable boundary: honest semantic coverage, valid Model scenario
contract, enforced paired scorer gates, recorded OpenAI + Anthropic
direct/discovery pairs, instructions A/B, rollout decision.

User or operator value: an evidence-backed default that does not trade
correctness for context savings.

Exit conditions: all durable rollout gates satisfied and recorded; only
then prune `03c`.

Deferred work: generated integrations stay on direct while `03c` is active.

## 8. Cross-cutting requirements

### Determinism and bounded execution

Evaluation uses exact serialized Tool definitions; no nondeterministic
model execution in CI.

### Protocol and compatibility

Era behavior from `02d` untouched unless an independently verified
protocol bug is found.

### Profile, audience, and surface

HarnessOnly stays containment (`must_not_expose`); deprecated stays
migration/negative.

### Documentation and generated assets

Roadmap honesty: distinguish implementation, deterministic
infrastructure, and pending external evidence.

### Release and qualification

Sanitized traces in `tests/fixtures/discovery_traces/` per its README;
scorer ready but not sufficient.

## 9. Verification strategy

`test_discovery` corpus + retrieval/leak/byte-ratio gates; official
`@modelcontextprotocol/client@2.0.0` smoke (modern with
`versionNegotiation=auto`, legacy by default); paired trace scoring
with noninferiority deltas; no fabricated traces.

## 10. Risks and decision points

Open: model/client access and budget. If unavailable, `03c` stays
active or explicitly blocked; no default change. No ADR required unless
ranking/contract ownership changes.

## 11. Completion definition

This roadmap closes when `03c` has accepted closure evidence proving
all deterministic + external gates, with versions, per-task results,
A/B outcome, and the integration/default decision recorded.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| 01 protocol modernization | closed | — (commit `35f7dc2e`) | — (history in `plans/archive/roadmap.md`) | — |
| 02 progressive discovery | closed | — (commit `a5c00b8d`) | — | — |
| 02c contract corrective | closed | — (commit `77ff57a5`) | — | — |
| 02d era classification | closed | — (commit `121babde`) | — | — |
| 03c evaluation closure | active/blocked | `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md` | — (pending) | Provider credentials / eval budget; instructions A/B blocked |
