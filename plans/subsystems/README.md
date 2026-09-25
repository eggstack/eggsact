# Subsystem Roadmaps

Subsystem roadmaps translate the canonical eggsact direction into
coherent, dependency-aware workstreams. They are not direct coding-agent
checklists.

Each roadmap should remain useful across several implementation
milestones and repository revisions. Commit-specific mechanics belong in
`plans/implementation/`.

## Naming

```text
<subsystem>-roadmap.md
```

Examples:

```text
deterministic-tool-substrate-roadmap.md
mcp-presentation-surface-roadmap.md
distribution-update-release-roadmap.md
harness-integration-roadmap.md
```

## Required roadmap structure

```markdown
# <Subsystem> Roadmap

Status: proposed | active | closing | closed | superseded

Long-term references:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`
- `plans/002-long-term-roadmap.md#...`

Related ADRs:

- `plans/adrs/ADR-NNNN-...md`

## 1. Purpose and ownership boundary

Define what the subsystem owns, what it consumes, and what it must not own.

## 2. Work classification

### Invariants

- ...

### Capabilities

- ...

### Infrastructure

- ...

### Polish

- ...

## 3. Non-goals

- ...

## 4. Current state

Summarize repository evidence, existing contracts, compatibility paths,
and known gaps. Avoid fragile line-number references unless essential.

## 5. Target architecture

Describe the end-state module, registry, protocol, ownership, and
lifecycle model for this subsystem.

## 6. Dependency graph

```text
Milestone A
    |
    +--> Milestone B
    |
    `--> Milestone C
             |
             `--> Milestone D
```

Classify each dependency as hard, interface, soft, or operational.

## 7. Milestones

### Milestone 1 — Title

Class: invariant | capability | infrastructure | polish

Objective:

Dependencies:

Deliverable boundary:

User or operator value:

Exit conditions:

Deferred work:

### Milestone 2 — Title

...

## 8. Cross-cutting requirements

### Determinism and bounded execution

### Protocol and compatibility

### Profile, audience, and surface

### Documentation and generated assets

### Release and qualification

## 9. Verification strategy

Define subsystem-level integration, property, truncation, cancellation,
era-classification, parity, and end-to-end evidence.

## 10. Risks and decision points

List unresolved decisions and identify which require ADRs.

## 11. Completion definition

Describe what must be true before the subsystem roadmap is closed.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| 1 | not started | — | — | — |
```

## Roadmap rules

A subsystem roadmap MUST:

- link to canonical long-term requirements rather than duplicating them wholesale;
- define ownership boundaries before milestones;
- distinguish infrastructure from completed capability;
- expose dependencies and decision points;
- preserve completed milestone history;
- link each active milestone to one implementation plan and later one closure record;
- state non-goals to prevent scope expansion;
- remain at the subsystem level rather than becoming a file-by-file implementation checklist.

A subsystem roadmap MAY be updated when implementation evidence changes
sequencing or decomposition. Material changes must record why the
roadmap changed.

## Initial subsystem roadmaps

eggsact starts with four workstreams (not a fixed release list):

1. deterministic tool substrate (cores, services, registry, limits);
2. MCP presentation surface (protocol eras, discovery, evaluation);
3. harness integration and docs (ToolRegistry, contexts, preflight, generation, parity);
4. distribution, update, and release (binaries, installers, self-update, gates).

Create only roadmaps that are ready to be reasoned about. Do not
generate speculative workstreams merely to populate the directory.
