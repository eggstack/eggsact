# Milestone 8: Diagnostics and Self-Inspection Improvements

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Improve diagnostics and self-inspection so MCP clients, codegg-style harnesses, and maintainers can understand the active runtime, available tools, profile/audience filtering, generated asset status, route-critical contracts, and budget limits without manually reading repository files.

The intended outcome is a stable machine-readable diagnostic surface that helps answer questions such as:

- Why does tool X not appear in `tools/list`?
- Why is tool X rejected when called?
- Which profile and audience are active?
- Which schema detail level is active?
- Which tools are route-critical?
- Which generated assets are present?
- Which runtime limits are active?
- Which profiles are available and what are they intended for?

## Rationale

The runtime diagnostics have already been improved to include active audience, schema detail, and runtime limits. This milestone finishes that line of work by making diagnostics more complete and more useful to integrations. It should reduce support/debugging cost for codegg and any external MCP client.

The key design constraint is stability: diagnostics should be machine-readable and conservative. Avoid making free-text documentation the only source of truth for integration state.

## Scope

In scope:

- Expand or refine `runtime_diagnostics`.
- Add profile inspection details.
- Add tool availability explanations.
- Add generated asset and package-content status where feasible.
- Add docs for common diagnostics workflows.
- Add tests for diagnostic shape and audience restrictions.
- Ensure diagnostics never leak environment variable values or sensitive local paths.

Out of scope:

- Telemetry collection.
- Network status checks.
- Runtime performance profiling beyond existing limits/counts.
- Reading arbitrary local files.
- Exposing secrets or actual env var values.
- Building a dashboard.

## Files likely to change

- `src/tools/diagnostics.rs`
- `src/mcp/schemas/diagnostics.rs`
- `src/mcp/specs/diagnostics.rs`
- `src/mcp/runtime.rs`
- `src/mcp/registry/listing.rs`
- `src/main.rs`
- `tests/mcp/test_diagnostics.rs`
- `tests/mcp/test_mcp_tools.rs`
- `tests/mcp/test_response_structure.rs`
- `architecture/coding-agent-integration.md`
- `architecture/generated-assets.md`
- `docs/cli.md`
- `docs/mcp-tools.md`
- `README.md` generated sections if needed

## Diagnostic surfaces

### CLI diagnostics

`eggsact --diagnostics` and `eggsact --diagnostics --format json` should remain quick, local, and safe.

Recommended JSON fields:

```json
{
  "version": "...",
  "tool_count": 0,
  "profiles": {...},
  "runtime": {
    "active_profile": "...",
    "active_audience": "...",
    "schema_detail": "...",
    "compatibility_mode": "...",
    "limits": {
      "max_requests_per_second": 0,
      "max_in_flight_requests": 0,
      "max_tool_workers": 0,
      "max_request_bytes": 0,
      "max_output_bytes": 0
    }
  },
  "route_critical_tools": ["..."],
  "budget_tiers": {...},
  "env_var_names": ["..."],
  "generated_data": {...},
  "generated_doc_command": "cargo run --bin generate-docs -- --check",
  "verification_command": "cargo run --bin verify-eggsact"
}
```

Do not include env var values.

### MCP `runtime_diagnostics`

The MCP tool should expose a stable subset of the CLI JSON data. It should be available only to harness/debug audiences unless there is a deliberate reason to expose a reduced model-safe version.

Recommended additions:

- `active_profile`.
- `active_audience`.
- `schema_detail`.
- `profile_tool_count`.
- `model_visible_tool_count` if cheap to compute.
- `harness_visible_tool_count` if cheap to compute.
- `route_critical_tools`.
- `runtime.limits`.
- `generated_data` status.
- `compatibility_mode`.
- `budget_tier_summary`.

### Profile inspection

Either add a new tool such as `profile_inspect` or expand `profiles/list` if that is the better fit.

Recommended profile fields:

- `name`.
- `tool_count`.
- `intended_audience`: `model`, `harness`, `debug`, or `mixed`.
- `purpose`.
- `recommended_for` workflows.
- `contains_route_critical_tools`: boolean.
- `contains_harness_only_tools`: boolean.
- `representative_tools`.
- `warnings`.

Do not return giant full schemas from profile inspection. It should be a routing/debugging tool, not a replacement for `tools/list`.

### Tool availability explanation

Consider adding a small diagnostic helper, either as a new tool or as part of profile inspection:

`tool_availability_explain`

Inputs:

- `tool`: required string.
- `profile`: optional string.
- `audience`: optional string.

Output:

- `exists`: boolean.
- `available_in_profile`: boolean.
- `callable_by_audience`: boolean.
- `exposure`.
- `profiles`.
- `reason`.
- `suggested_profile`.
- `suggested_audience`.
- `close_match` if tool name appears misspelled.

This is useful for MCP clients asking why a tool is not listed or callable. Keep it harness/debug only unless there is a model-safe use case.

## Generated asset diagnostics

Diagnostics should report presence/absence for required generated assets without dumping file content:

- README generated block status if easy to check.
- `generated/tool-cards.md` existence if present in repo.
- `src/text/confusables_generated.rs` existence.
- Parity reference availability if detectable.
- Generator command.
- Verification command.

Do not run generation as part of diagnostics. Diagnostics should inspect state cheaply.

## Security and privacy constraints

Diagnostics must not expose:

- Actual environment variable values.
- Full local filesystem paths beyond repo-relative names where already public.
- User home directory.
- Secrets, tokens, or config contents.
- Full process environment.

If diagnostics need to say an env var is set, prefer boolean presence only, and even that should be considered carefully. Current safer default is to expose known env var names, not values or presence.

## Testing plan

### CLI tests

Add tests for:

- `--diagnostics` text output includes runtime section.
- `--diagnostics --format json` parses as JSON.
- JSON includes required stable keys.
- JSON does not include actual env var values.
- Invalid format handling remains correct.

### MCP tool tests

Add tests for:

- `runtime_diagnostics` callable by harness audience.
- `runtime_diagnostics` rejected or hidden for model audience if intended.
- Output includes active profile, audience, schema detail, limits, route-critical tools.
- Tool output schema matches returned shape.

### Profile inspection tests

If adding profile inspection:

- Known profiles are listed.
- Tool counts are nonzero where expected.
- Unknown profile returns clear machine code.
- Profile purpose fields are stable.

### Availability explanation tests

If adding availability explanation:

- Existing model-safe tool in model profile returns callable.
- Harness-only tool in model audience returns not callable with reason.
- Unknown tool returns close match where applicable.
- Tool outside active profile returns suggested profile where applicable.

## Documentation updates

Add a diagnostics workflow section:

### Why does my MCP client not see a tool?

Explain:

1. Active profile may not include the tool.
2. Active audience may hide harness-only tools.
3. `tools/list` schema detail may be compact/normal/full.
4. Tool may be hidden or deprecated.
5. Tool name may be misspelled.

### Why did `tools/call` reject a tool?

Explain:

1. Tool does not exist.
2. Tool exists but is not in the active profile.
3. Tool exists but audience cannot execute it.
4. Arguments failed schema validation.
5. Input exceeded budget.

### Which diagnostics command should I run?

Document:

```bash
eggsact --diagnostics --format json
```

and MCP:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"runtime_diagnostics","arguments":{}}}
```

## Testing requirements

Run targeted tests:

```bash
cargo test --all-features diagnostics -- --nocapture
cargo test --all-features profile -- --nocapture
cargo test --all-features tool_availability -- --nocapture
```

Run full verification:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --all-features --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Acceptance criteria

- CLI diagnostics and MCP diagnostics expose stable runtime metadata.
- Diagnostics do not leak env var values or secrets.
- Profile inspection or equivalent explains profile purpose and audience expectations.
- Tool availability diagnostics can explain missing/uncallable tools or docs clearly cover the workflow if no tool is added.
- Tests cover diagnostics shape and access restrictions.
- Generated docs are current.
- Docs explain common MCP integration/debugging failures.

## Handoff notes

Run this milestone after profile and exposure cleanup. Diagnostic output should reflect the final tool/profile model, not a transitional one. If milestone 6 tools are implemented in phases, diagnostics may be extended incrementally, but final acceptance should happen after the profile audit.
