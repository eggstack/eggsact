# Phase 11 Plan: Observability, Diagnostics, and Verification Reporting

## Purpose

Phase 11 makes eggsact easier for agents and maintainers to diagnose. Earlier phases hardened behavior and boundaries; phase 10 adds explicit context isolation. Phase 11 should expose that behavior through structured diagnostics, verification reporting, and low-noise observability without turning eggsact into a telemetry-heavy service.

The goal is local, deterministic, privacy-preserving diagnostics suitable for codegg and other agent harnesses.

## Goals

- Add structured diagnostics for tool registry/profile/audience/context state.
- Add local verification reports for release gates and generated-doc freshness.
- Improve failure explanations for budget, cancellation, profile, audience, and parser-backed preflight paths.
- Provide agent-friendly diagnostic tools or CLI commands without exposing hidden/internal tools to model audience.
- Keep diagnostics deterministic and free of external network calls.

## Non-Goals

- Do not add remote telemetry.
- Do not phone home.
- Do not collect user project contents by default.
- Do not add a dashboard server.
- Do not expose harness-only/internal diagnostics to model-facing MCP profiles.
- Do not create a persistent metrics database.

## Workstream A: Diagnostic Inventory

### Required Work

Inventory existing diagnostic surfaces:

- CLI flags and help output;
- MCP `initialize`, `tools/list`, `tools/call`, `profiles/list` behavior;
- `ToolResponse` fields such as `machine_code`, `verdict`, `findings`, `limits_applied`, and warnings;
- generated docs and tool cards;
- tests and release script output;
- `AGENTS.md` and `.skills/` guidance.

Classify missing diagnostics:

- registry/profile mismatch;
- hidden/harness-only tool rejection;
- compatibility-mode validation error;
- budget input/output/truncation behavior;
- cancellation/timeout distinction;
- parser-backed versus heuristic config/dependency behavior;
- generated docs stale;
- parity tests unavailable due to missing Python reference.

### Acceptance Criteria

- A concise inventory is added to `architecture/mcp-server.md`, `architecture/overview.md`, or a new `architecture/diagnostics.md`.
- Missing diagnostic gaps are mapped to concrete follow-up tasks in this plan.

## Workstream B: Verification Report Command

### Problem

Release verification currently depends on manual command execution and CI status may not always be visible through connectors. A local verification report makes handoff clearer.

### Required Work

Add a non-invasive verification report mechanism.

Preferred option: a cargo-integrated binary or script:

```bash
cargo run --bin verify-eggsact -- --report markdown
```

Alternative: enhance `release.sh` to emit a structured local report file under `target/verification/`.

The report should include:

- commit/ref if available;
- command list;
- pass/fail/skip status for each command;
- duration per command if cheap to capture;
- generated-doc freshness result;
- parity-test availability status;
- environment caveats.

Initial commands:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Parity checks should be recorded as `skipped` if `../eggcalc` is not present.

### Acceptance Criteria

- Maintainers can run one command/script to produce a verification report.
- Report output is local and ignored by git unless intentionally committed.
- Failures include exact command and exit status.
- Parity dependency is reported honestly.

## Workstream C: Diagnostic CLI Surface

### Required Work

Add or improve CLI diagnostics without cluttering normal help.

Potential commands/flags:

```bash
eggsact --diagnostics
eggsact --diagnostics --format json
eggsact --mcp-diagnostics
```

The diagnostic output should include:

- crate version;
- tool count;
- profile names and counts;
- generated-doc command;
- default compatibility mode by surface;
- budget tier summary;
- known environment variables;
- whether generated data files are present;
- whether optional parity reference path exists, if checked locally.

Do not include user file contents, environment variable values, API keys, or secrets.

### Acceptance Criteria

- Diagnostic command emits stable JSON when requested.
- Plain-text diagnostics are concise.
- No secret-bearing values are printed.
- Tests cover JSON shape.

## Workstream D: MCP Diagnostic Tooling for Harness Audience

### Problem

Agents and harnesses benefit from structured introspection, but model-facing profiles should not receive excessive internals.

### Required Work

Consider adding one harness/contextual diagnostic MCP tool, or reuse existing profile/tool listing if sufficient.

Potential tool:

`runtime_diagnostics`

Audience/exposure:

- `HarnessOnly` or at most `Contextual` depending on content;
- not listed to `Model` audience unless output is highly sanitized.

Output fields:

- active profile;
- audience;
- compatibility mode;
- tool count visible to current context;
- route-critical tool availability;
- budget tier limits summary;
- generated-doc version marker if available;
- warnings for profile/tool mismatches.

Do not include:

- environment variable values;
- filesystem listings;
- user paths except sanitized current binary path if needed;
- secrets.

### Acceptance Criteria

- Diagnostic tool is not model-visible unless explicitly safe.
- Output is deterministic and schema-backed.
- Tests verify audience filtering and no hidden leakage.

## Workstream E: Improve Error Diagnostics

### Required Work

Review errors for these paths and ensure they include actionable, sanitized messages:

1. Profile rejection.

   Include tool name, active profile, and reason. Do not suggest bypassing profile.

2. Audience rejection.

   Include exposure/audience mismatch. Do not expose hidden tool details to model audience.

3. Compatibility validation failure.

   Include expected type, received type, and compatibility mode.

4. Budget input too large.

   Include serialized input size and max input bytes.

5. Output truncation.

   Include `limits_applied` details and preserved route-critical fields.

6. Cancellation versus timeout.

   Ensure `CANCELLED` and `TIMEOUT` are distinct and routeable.

7. Parser-backed config/dependency fallback.

   Include `parse_ok`, `format`, and heuristic-only warning where appropriate.

### Acceptance Criteria

- Error messages are actionable and sanitized.
- Machine codes remain stable and enumerated.
- Tests assert exact machine codes for key diagnostics.

## Workstream F: Generated Diagnostics Documentation

### Required Work

Extend docs generation or manual docs to cover:

- diagnostic command/tool usage;
- machine-code categories;
- budget/cancellation semantics;
- generated-doc check workflow;
- verification report workflow.

If a diagnostic tool is added, ensure generated tool cards include it with correct audience/exposure.

### Acceptance Criteria

- Docs accurately describe diagnostics.
- Generated docs remain fresh.
- `AGENTS.md` tells agents how to run diagnostics without overusing them.

## Workstream G: Test Matrix

### Required Tests

1. Diagnostic CLI JSON shape test.
2. Diagnostic CLI does not include known secret-looking env var values.
3. Harness-only diagnostic tool is not listed to Model audience.
4. Harness audience can call diagnostic tool if added.
5. Profile/audience rejection messages contain expected safe fields.
6. Budget/cancellation diagnostics preserve machine codes.
7. Verification report command records skip for missing parity reference.
8. Generated-doc check includes diagnostic docs/tool if added.

### Acceptance Criteria

- Diagnostics tests are deterministic.
- Tests do not depend on external network or user-specific paths.

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Run the new verification report command/script and inspect output.

## Final Acceptance Criteria

Phase 11 is complete when:

- diagnostic inventory exists;
- maintainers can generate a local verification report;
- diagnostic CLI output is available and tested;
- any MCP diagnostic tool is audience-safe;
- errors for profile/audience/compatibility/budget/cancellation/parser paths are actionable;
- docs and generated docs cover diagnostics;
- full verification passes or failures are precisely documented.
