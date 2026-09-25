# Closure and Verification Records

This directory contains evidence-based completion records for
implementation milestones.

A closure record is not a retrospective summary alone. It is the gate
that determines whether the corresponding subsystem milestone can be
marked complete.

## Layout and naming

```text
closure/<subsystem>/NNN-status.md
```

Use the same milestone number as the source implementation plan
(eggsact preserves legacy IDs such as `003c`).

## Required closure-record template

```markdown
# <Subsystem> Milestone NNN — Closure Status

Status: closed | conditionally closed | corrective pass required | blocked

Source implementation plan:

- `plans/implementation/<subsystem>/NNN-...md`

Source subsystem roadmap:

- `plans/subsystems/<subsystem>-roadmap.md#...`

Repository baseline reviewed: `<SHA>`

Implementation commits or pull requests:

- `<SHA or link>` — summary

## 1. Executive finding

State whether the milestone's actual capability or infrastructure
boundary is complete and why.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| ... | test, code, docs, runtime output, CI run ID | pass/fail/partial/not run | ... |

## 3. Production implementation evidence

Describe the landed ownership, registry, protocol, runtime, operator,
and docs changes. Distinguish implemented behavior from planned but
absent behavior.

## 4. Verification executed

### Commands run

```bash
# exact commands
```

### Results

Record pass, fail, timeout, environmental block, skipped scope, and
relevant counts without concealing partial execution. Record remote CI
run IDs and native-runner gaps explicitly.

## 5. Invariant review

For each source-plan invariant, record evidence that it remains true.

## 6. Failure and recovery review

Cover applicable:

- oversize input and truncation (`limits_applied`);
- duplicate delivery and idempotency;
- cancellation races and correlated JSON-RPC `id`s;
- process restart;
- stale generation or lease;
- contention and resource release;
- malformed regex / heuristic YAML / cross-era input;
- bounded event and artifact behavior.

## 7. Migration and compatibility review

Record registration-order prefix status, deprecation handling, protocol
negotiation, configuration behavior, rollback limitations, and
legacy-path status.

## 8. Determinism and bounded-execution review

Record exact-input/exact-output checks, no-clock/TZ/env/net review for
touched utils, and limit/budget enforcement as applicable.

## 9. Documentation and operations

List updated architecture documents, commands, operator diagnostics,
static guards (including release-contract changes), and recovery
instructions.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| ... | ... | ... | ... |

Severity guidance:

- critical — unsafe to merge or operate;
- high — core correctness or trust/qualification contract incomplete;
- medium — important behavior incomplete but bounded;
- low — polish, maintainability, or optional evidence gap.

## 11. Roadmap disposition

State one:

- milestone closed and next dependency may proceed;
- milestone conditionally closed with named operational evidence outstanding;
- corrective implementation plan required;
- milestone blocked and reason;
- subsystem roadmap must be revised.

## 12. Registry updates

List changes required in `plans/registry.md` and the source subsystem roadmap.
```

## Closure rules

A milestone MUST NOT be marked closed when:

- only compilation or formatting was verified;
- required tests were not run and no justified substitute evidence exists;
- a user-visible capability has only internal infrastructure (e.g. a
  scorer without recorded traces);
- a trust, qualification, or compatibility requirement is unimplemented;
- a known high-severity defect remains;
- closure depends on unrecorded assumptions or fabricated external evidence.

A milestone MAY be conditionally closed when production implementation
is complete but named external or operational evidence cannot be
obtained in the current environment. The condition, risk, and exact
future evidence must be explicit.

## Corrective follow-up

When closure requires correction:

1. keep the closure record immutable except for factual corrections;
2. create a new implementation plan under the same subsystem;
3. reference every unclosed finding;
4. add regression tests or guards that prevent recurrence;
5. do not reopen unrelated closed scope.
