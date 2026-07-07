# Milestone 3: Route-Critical Contract Tightening

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Make route-critical tools contractually stable through exact fixture-backed tests. A route-critical tool is one whose result can influence downstream agent or harness decisions: allow, review, block, retry, shrink input, ask for human approval, or select a different execution path.

The target result is that every route-critical tool has deterministic fixtures asserting response envelope fields, verdict, machine code, and representative findings. Unsafe inputs should not merely produce “some error”; they should produce expected machine-readable outcomes.

## Current route-critical tools

The current route-critical set should be read from `src/mcp/registry/listing.rs` and must remain the source of truth. At the time this plan was written, the list is:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `text_security_inspect`

Do not duplicate this list in tests without also asserting that the fixture registry and `ROUTE_CRITICAL_TOOLS` are in sync.

## Rationale

Coding-agent harnesses need stable machine-readable outputs. If a command is dangerous, a patch escapes scope, or a config file is malformed, the caller should be able to branch on `machine_code`, `verdict`, and structured findings. Free-text errors are insufficient for reliable routing.

The repo already has typed preflight wrappers that treat missing mandatory fields as contract violations. This milestone extends that philosophy to broader MCP/in-process fixture coverage and closes permissive test patterns that can hide regressions.

## Scope

In scope:

- Fixture suite for route-critical tools.
- Exact expectations for allow/review/block and valid/invalid flows.
- Tests for in-process API and MCP stdio response shape.
- Tightening permissive unsafe-input tests.
- Registry invariant tests tying fixtures to `ROUTE_CRITICAL_TOOLS`.
- Documentation updates for route-critical response expectations.

Out of scope:

- Large behavior redesign of route-critical tools.
- New route-critical tools, except if a clear existing tool is already de facto route-critical and the change is justified.
- New command-preflight pattern coverage beyond the baseline needed here; deeper shell hardening belongs in milestone 4.
- Schema validator changes; those belong in milestone 5.

## Files likely to change

- `tests/mcp/test_route_contracts.rs`
- `tests/mcp/test_preflight_wrappers.rs`
- `tests/mcp/test_error_structure.rs`
- `tests/mcp/test_response_structure.rs`
- `tests/mcp/test_tool_gaps.rs`
- New fixture module or fixture data file under `tests/fixtures/`
- `architecture/machine-codes.md`
- `architecture/mcp-server.md`
- Possibly `src/mcp/registry/listing.rs` only if route-critical classification changes

## Fixture design

Use table-driven fixtures. Each fixture should include:

- Tool name.
- Input arguments.
- Expected top-level success or failure shape.
- Expected `ok` value.
- Expected `machine_code`.
- Expected `verdict` when the tool is supposed to produce one.
- Expected minimum severity/disposition where findings are present.
- Expected finding code or finding type when applicable.
- Whether the fixture should be exercised through in-process API, MCP stdio, or both.
- Optional notes explaining why the expectation matters.

Suggested Rust fixture type:

```rust
struct RouteFixture {
    name: &'static str,
    tool: &'static str,
    args: serde_json::Value,
    expected_ok: bool,
    expected_verdict: Option<&'static str>,
    expected_machine_code: &'static str,
    expected_finding_code: Option<&'static str>,
    expected_min_findings: usize,
    exercise_mcp: bool,
}
```

Keep fixtures close to tests unless they become large. If they become large, move JSON cases into `tests/fixtures/route_contracts/*.json` and write a small loader.

## Required fixture coverage

### `edit_preflight`

Required cases:

- Safe exact replacement in normal source text.
- Replacement text not found.
- Replacement would affect multiple occurrences when exactly one is expected.
- Edit outside allowed path scope when path scope input is supplied.
- Edit introducing suspicious Unicode or confusable characters.
- Edit changing newline style or trailing newline in a way the tool is expected to flag.
- Oversized input rejected with the expected machine code.

Expected result classes:

- Safe edit: allow or equivalent safe verdict.
- Ambiguous/missing replacement: review or block depending current behavior.
- Scope violation: block.
- Unicode hazard: review or block depending severity.

### `command_preflight`

Required cases:

- Benign read-only command such as `git status` or `cargo check`.
- Network command such as `curl https://example.com`.
- Destructive filesystem command such as `rm -rf /` or a safe test fixture equivalent.
- Privilege escalation command such as `sudo cargo test`.
- Pipe-to-shell pattern such as `curl ... | sh`.
- Destructive git operation such as `git reset --hard` or `git clean -fdx`.
- Command with unbalanced quotes or parse failure.
- Command using redirection or command substitution.

Expected result classes:

- Read-only: allow under default policy if current policy allows it.
- Network: review.
- Destructive: block.
- Privilege escalation: block.
- Pipe-to-shell: block.
- Parse failure: review or block with parse-specific machine code.

Do not overfit to human-readable message text. Assert stable codes and verdicts.

### `config_preflight`

Required cases:

- Valid JSON config.
- Invalid JSON syntax.
- Valid TOML config.
- Invalid TOML syntax.
- Unknown config format if supported by schema.
- Strict schema failure if schema input is supplied.
- Secret-looking key/value if the tool currently flags this.
- Oversized input rejected.

Expected result classes:

- Valid: valid or allow-style verdict as currently defined.
- Invalid syntax: invalid with parse/config machine code.
- Schema failure: invalid or review depending current design.

### `patch_apply_check`

Required cases:

- Patch applies cleanly to provided text.
- Patch context does not match.
- Patch modifies outside allowed path scope if path data is supplied.
- Patch deletes a large amount of content.
- Patch touches dependency/build files.
- Patch touches lockfiles.
- Patch with malformed hunk.
- Oversized patch rejected.

Expected result classes:

- Clean small patch: allow.
- Context mismatch/malformed hunk: block or invalid.
- Dependency/build/lockfile touches: review unless current behavior blocks.

### `text_security_inspect`

Required cases:

- Plain ASCII source text.
- Text with bidirectional override/control characters.
- Text with mixed-script confusables.
- Text with zero-width characters.
- Text containing likely secret/token material if currently in scope.
- Large text that triggers truncation or budget behavior.

Expected result classes:

- Plain ASCII: allow.
- Bidi/control characters: review or block.
- Confusables: review.
- Zero-width: review.
- Secrets: review or block depending current severity model.

## Registry and fixture invariants

Add tests that:

- Every tool in `ROUTE_CRITICAL_TOOLS` has at least one fixture.
- Every route-critical fixture references an existing tool.
- Every route-critical fixture references a tool listed in `ROUTE_CRITICAL_TOOLS`, unless explicitly marked as transitional.
- Every route-critical successful response contains `machine_code`.
- Every route-critical successful response contains `verdict` where the route-critical contract says it must.
- No route-critical tool returns `ok: true` with missing mandatory route fields.

If some current route-critical tool does not always produce a verdict, either fix the tool or update the route-critical classification and docs. Do not silently weaken the tests.

## MCP response coverage

For each route-critical tool, at least one fixture should exercise MCP stdio through `tools/call`. The test should parse the JSON-RPC response, extract the MCP content text, parse the embedded `ToolResponse`, and assert the same route fields as the in-process call.

Also include at least one unavailable-tool/audience test:

- Calling a harness-only route-critical tool from model audience should fail with an explicit profile/audience error.
- Calling the same tool from harness audience should reach argument validation or handler execution.

## Tightening permissive tests

Review tests in `test_tool_gaps.rs` and related files that use patterns like:

```rust
if raw.get("error").is_none() {
    ...
}
```

For safety-relevant cases, replace permissive branching with exact assertions. The cases most in need of tightening are unsafe regex patterns, oversized lists, destructive commands, and route-critical invalid inputs.

If a test must allow two shapes during a transition, mark it with a clear TODO and a deadline milestone. Do not leave broad permissive assertions permanently.

## Documentation updates

Update route-critical documentation to state:

- Which tools are route-critical.
- Which fields are mandatory on successful route-critical responses.
- Which fields are stable enough for downstream routing.
- Which fields are diagnostic/free-text and should not be used for routing.
- That route-critical contract changes are breaking changes for harness consumers.

Update machine-code docs if new or clarified codes are needed.

## Testing requirements

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Also run the specific route-contract tests while iterating:

```bash
cargo test --test lib test_route_contracts -- --nocapture
```

Adjust the exact command if the integration test module structure requires a different filter.

## Acceptance criteria

- Every route-critical tool has fixture coverage.
- Fixtures assert exact `machine_code` and `verdict` where applicable.
- Route-critical successful responses cannot omit mandatory route fields without failing tests.
- Unsafe inputs have exact expected failures or review/block outcomes.
- MCP stdio and in-process paths are both covered.
- Permissive safety-relevant tests are tightened or explicitly marked transitional.
- Route-critical docs match registry state.

## Review checklist

Before closing this milestone, verify:

- Fixture names describe the scenario and expected routing decision.
- Assertions do not depend on brittle human-readable message wording.
- Machine-code constants are used where possible instead of string duplication.
- Tests fail if a route-critical tool is added without fixtures.
- Tests fail if a route-critical response changes verdict/code unexpectedly.
- Harness-only tool behavior is tested against model and harness audiences.

## Handoff notes

This milestone should precede new route-critical or composite tool work. Once these tests exist, future tools such as `patch_contract_check` can be added with stronger confidence and less risk of unstable downstream routing behavior.
