# Milestone Implementation Plans

This directory contains bounded plans handed directly to implementation
agents.

Implementation plans are operational documents tied to the current
repository state. They may be corrected, superseded, or archived without
modifying the canonical long-term documents.

## Layout and naming

```text
implementation/<subsystem>/NNN-short-title.md
```

Milestone numbering is local to the subsystem roadmap unless the roadmap
defines another stable identifier (eggsact preserves legacy IDs such as
`03c` for the MCP evaluation closure corrective).

## Required implementation-plan template

```markdown
# <Subsystem> Milestone NNN — <Title>

Status: ready for handoff | active | blocked | implemented | superseded

Repository baseline: `<commit SHA or branch state>`

Source roadmap:

- `plans/subsystems/<subsystem>-roadmap.md#...`

Long-term requirements:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`

Applicable ADRs:

- `plans/adrs/ADR-NNNN-...md`

Primary class: invariant | capability | infrastructure | polish

## 1. Objective

State one bounded outcome.

## 2. Why this milestone is ready

List closed hard dependencies and stable interface dependencies.

## 3. Current implementation evidence

Describe the relevant code, tests, registry, guards, and known gaps at
the repository baseline.

## 4. Invariants that must not regress

- ...

## 5. Scope

### In scope

- ...

### Explicitly out of scope

- ...

## 6. Required production changes

Describe required behavior and ownership changes. Name likely modules
where useful, but do not prescribe blind mechanical edits when
alternatives are valid.

### Core/service/adapter

### Registry, profile, audience, surface

### Protocol and DTOs

### Runtime and concurrency

### Operator surface (CLI, update, integrate)

### Documentation and static guards

## 7. Ordered work packages

### Work package A — Title

Intent:

Required changes:

Acceptance evidence:

### Work package B — Title

...

## 8. Failure, cancellation, truncation, and contention semantics

State expected behavior for partial failure, oversize input, duplicate
delivery, process restart, cancellation races, stale generations, and
concurrent callers (correlate concurrent MCP responses by JSON-RPC `id`).

## 9. Compatibility and migration

Describe backward compatibility, registration-order prefix preservation,
deprecation policy, protocol negotiation, configuration fallback, and
removal criteria for legacy paths.

## 10. Required tests

### Focused unit tests

### Integration tests

### Restart and recovery tests

### Contention and cancellation tests

### Negative and boundary tests

### Parity and compatibility tests

## 11. Required verification commands

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
# plus: release contract, parity (with eggcalc at ../eggcalc),
# discovery scorer, bench (non-gating) as applicable
```

Do not claim commands that were not actually run in the closure record.

## 12. Documentation updates

- ...

## 13. Acceptance criteria

Use externally observable or contract-level statements.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- an unresolved architecture decision materially changes ownership;
- a hard dependency is absent;
- required migration cannot be made safely;
- repository evidence contradicts a canonical invariant;
- external evidence is required but unavailable (do not fabricate
  model traces or benchmark claims);
- scope would expand into another subsystem roadmap.

## 15. Closure evidence required

List the exact evidence the later closure record must contain.

## 16. Handoff notes

Include known hazards, resource constraints, serial-test requirements
(`--test-threads=4` for integration tests), environment requirements,
and preserved user changes.
```

## Handoff rules

Before assigning a plan to an agent:

- confirm the repository baseline is current;
- confirm all hard dependencies are closed;
- confirm unresolved decisions have ADRs or are explicitly out of scope;
- ensure the milestone can be completed in one coherent pass;
- ensure tests and closure evidence are specific;
- register the plan in `plans/registry.md`.

The implementation agent may adjust file-level mechanics based on
current code. It may not weaken canonical invariants or silently enlarge
scope.

## Corrective plans

Corrective work receives a new plan in the same subsystem directory, for
example:

```text
004-correctness-and-recovery-closure.md
```

The corrective plan must reference the original plan and closure record,
enumerate unclosed findings, and add regression evidence that would have
caught them. Original closures stay immutable.
