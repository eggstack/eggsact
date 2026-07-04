# Release Polish Plan for Phase 06–09 Closure

## Purpose

This plan closes the remaining release-polish items after the phase 06–09 tightening, cleanup, and final-polish passes. The implementation is now technically solid and should not receive new feature work in this pass. The goal is to remove the last documentation/verification inconsistencies and leave a clean handoff point before phase 10 begins.

## Current State

The repo has substantially improved across the phase 06–09 line of work:

- profile and audience enforcement is in place;
- route-critical tools have normalized response contracts;
- generated-doc workflows exist;
- `edit_preflight` has stricter replacement-mode contracts;
- `command_preflight` has explicit platform-support semantics;
- config and dependency inspectors have parser-backed coverage for core formats;
- budget envelopes, truncation, and cooperative cancellation are present;
- `AGENTS.md` was restored and now contains useful agent handoff instructions;
- parser-backed fixture tests and deterministic cancellation tests have been added.

The remaining issues are narrow and should be completed before moving on.

## Remaining Polish Items

### 1. Align `release.sh` with CI-equivalent formatting

#### Problem

`AGENTS.md` documents `cargo fmt --all -- --check`, while `release.sh` still uses `cargo fmt --check`. The latter may be sufficient for a single-package crate, but the documented CI-equivalent gate should be consistent everywhere.

#### Required Work

Update `release.sh`:

```bash
cargo fmt --all -- --check
```

Do not otherwise expand release scope unless necessary.

#### Acceptance Criteria

- `release.sh` and `AGENTS.md` use the same format-check command.
- `.skills/testing.md` and `.skills/release.md`, if present, do not contradict the release script.

### 2. Soften or verify parity-pass claims

#### Problem

`docs/parity.md` states that all parity tests pass. That is only safe if parity tests were actually run with the Python `eggcalc` reference available. Since connector-visible CI status has not provided evidence, the docs should avoid unverified absolute claims or include a verification note.

#### Required Work

Choose one path:

Path A — verification-backed wording:

- Run parity tests locally with `eggcalc` available.
- Add a short verification note, including command and date/commit.
- Keep the claim if the run passes.

Path B — environment-qualified wording:

- Replace absolute wording with environment-qualified wording such as:

  > The parity suite is intended to validate all Rust tools against the Python reference when `eggcalc` is available in the expected path.

- Keep the requirements section explicit.

Preferred action: Path B unless local parity output is being committed or recorded elsewhere.

#### Acceptance Criteria

- `docs/parity.md` no longer makes unverified unconditional claims.
- The parity-test environmental dependency is explicit.

### 3. Record verification evidence

#### Problem

GitHub combined status currently returns no commit statuses through the connector. Handoff should not imply CI passed without evidence.

#### Required Work

Run or request a local run of:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If parity tests need `../eggcalc`, document whether they ran or were skipped.

If there is no place to commit command output, add a small handoff note in the implementation summary or update `plans/` with a verification note after execution.

#### Acceptance Criteria

- Verification output is recorded or explicitly marked unavailable.
- CI absence is not presented as CI success.
- Any environmental caveat is explicit.

### 4. Final generated-doc freshness check

#### Problem

Generated docs and tool counts have drifted several times during this line of work. Before phase 10 begins, generated docs should be checked one more time.

#### Required Work

Run:

```bash
cargo run --bin generate-docs -- --check
```

If it fails, run:

```bash
cargo run --bin generate-docs
```

Then review generated diffs for:

- README tool tables;
- architecture profile reference block;
- `generated/tool-cards.md`;
- `.skills/mcp-tools.md`, if the generator or docs process maintains it.

#### Acceptance Criteria

- Generated docs check passes.
- No hard-coded count contradicts generated counts.

### 5. Final targeted regression checks

#### Required Work

Run focused tests before the full suite:

```bash
cargo test --test lib mcp::test_cancellation
cargo test --test lib mcp::test_tool_gaps
cargo test --test lib mcp::test_route_contracts
cargo test --test lib mcp::test_edit_preflight_enhanced
```

If the current test harness does not support module-qualified invocations in this exact form, document the correct equivalent command and update `AGENTS.md`/`.skills/testing.md` if needed.

#### Acceptance Criteria

- The documented focused test commands are valid, or the documentation is corrected.
- Cancellation, tool-gap, route-contract, and edit-preflight tests pass.

## Recommended Implementation Order

1. Update `release.sh` formatting command.
2. Review and adjust `docs/parity.md` wording.
3. Run generated-doc check and commit any generated updates.
4. Run focused tests and correct any invalid command documentation.
5. Run full verification suite.
6. Record verification status in the final implementation handoff.

## Completion Criteria

This polish pass is complete when:

- release script and docs agree on verification gates;
- parity claims are either verified or environment-qualified;
- generated docs are current;
- focused regression tests pass;
- full verification status is recorded;
- no new feature scope has been introduced.

## Non-Goals

Do not add new MCP tools.

Do not change parser semantics except to fix test-proven bugs.

Do not implement Windows command parsing.

Do not begin phase 10 evaluator/context isolation work in this pass.
