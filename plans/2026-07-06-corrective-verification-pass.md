# Corrective verification pass after phases 01–05

## Purpose

This plan is a focused corrective verification pass for the broad phase 01–05 implementation series that followed the agent-tooling roadmap. The goal is to verify that the new docs, correctness hardening, codegg wrappers, concurrent MCP runtime, and repo/diff/path intelligence tools are coherent, tested, and release-safe before any additional feature work begins.

This pass should not add broad new capabilities. It should validate and correct the implementation that already landed.

## Current state summary

After the roadmap and five phase plan files, `main` advanced through five implementation commits:

1. Phase 01 docs drift repair and metadata repositioning.
2. Phase 02 correctness hardening, including UTF-8-safe secret masking and byte-based budget naming.
3. Phase 03 codegg API ergonomics and typed wrappers.
4. Phase 04 concurrent MCP request handling with true in-flight cancellation.
5. Phase 05 repo/diff intelligence tools: `repo_tree_summarize`, `diff_risk_classify`, and `path_batch_scope_check`.

The repo is now stronger in scope and capability, but the change set is large enough to require a deliberate verification pass. Key risks are wrapper/schema mismatches, runtime concurrency edge cases, stale documentation left behind after runtime changes, and insufficient tests around the new tools.

## Hard constraints

Do not start phase 06 dependency/lockfile expansion during this pass.
Do not add command sequence preflight during this pass.
Do not change public schemas unless fixing a concrete mismatch or bug.
Do not remove compatibility aliases or old APIs unless the crate is intentionally preparing a breaking release.
Prefer minimal corrective commits with direct tests.

## Verification command matrix

Run these commands before and after fixes:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Also run focused tests while iterating:

```bash
cargo test --all-features preflight
cargo test --all-features route_contracts
cargo test --all-features tool_gaps
cargo test --all-features generated_marker_integrity
```

If parity tests are available locally with Python `eggcalc` at `../eggcalc`, run:

```bash
cargo test --test lib parity
```

Parity failures that correspond to known Rust-superset tools should be documented, not treated as regressions unless existing matching Python/Rust tools changed unexpectedly.

## Task 1: validate typed wrapper/schema alignment

### Problem to check

The `PatchApplyCheck` typed wrapper must be audited first. Earlier `patch_apply_check` code and schema used `original_text` and `patch_text` as required arguments. The new wrapper appeared to construct JSON using `original` and `patch`. If no alias support was added, the wrapper will fail schema validation or handler extraction.

### Steps

1. Inspect `src/preflight/mod.rs` for `PatchApplyCheckInput` and `PatchApplyCheck::run`.
2. Inspect `src/mcp/schemas/patch.rs` for `patch_apply_check_input`.
3. Inspect `src/tools/patch.rs` for the handler argument extraction.
4. Ensure names match exactly.

The wrapper should emit the canonical schema names unless the handler and schema explicitly support aliases. Prefer canonical names:

```json
{
  "original_text": "...",
  "patch_text": "...",
  "return_result_text": false,
  "strict": false
}
```

5. Add a wrapper integration test that calls `PatchApplyCheck::run()` with a valid minimal patch and asserts success.
6. Add a wrapper test that parses a representative successful `ToolResponse` and asserts fields are extracted correctly.
7. Add a negative test for missing required fields that confirms `PreflightError::ToolCall` or schema rejection is surfaced rather than silently defaulting.

### Acceptance criteria

`PatchApplyCheck::run()` succeeds for a valid patch using canonical tool arguments. Wrapper tests fail before the fix if the argument names are wrong and pass after the fix. No schema alias is introduced unless needed for backwards compatibility and documented.

## Task 2: audit all typed wrappers for profile/audience correctness

### Problem to check

Some tools are harness-only. A typed wrapper using `ToolRegistry::default()` may have `ToolAudience::Model`, which should reject harness-only tools at dispatch time. If `PatchApplyCheck` wraps `patch_apply_check`, it must use a harness-capable registry/profile or document that callers must pass one.

### Steps

1. Inspect every wrapper in `src/preflight/mod.rs`:
   - `EditPreflight`
   - `CommandPreflight`
   - `ConfigPreflight`
   - `PatchApplyCheck`
   - `TextSecurityInspect`
2. For each wrapper, identify the underlying tool exposure and profile membership.
3. Ensure harness-only wrappers use `ToolRegistry::with_profile_and_audience(Profile::CodeggPatch or Profile::CodeggPreflight, ToolAudience::Harness)` or accept a caller-supplied registry.
4. Prefer adding `run_with_registry(&ToolRegistry, &Input)` variants for all wrappers, then make `run()` choose a safe default registry for that wrapper.
5. Add tests proving harness-only wrappers can execute via their default `run()` method.
6. Add tests proving model-facing wrappers still use model-safe behavior where appropriate.

### Acceptance criteria

All wrapper `run()` methods use a profile/audience that can execute their underlying tools. Harness-only wrappers do not fail by default due to audience rejection. Documentation explains wrapper default profile/audience choices.

## Task 3: verify concurrent MCP runtime behavior

### Problem to check

Phase 04 replaced the serial read loop with concurrent request tasks, active request tracking, an `mpsc` writer, and in-flight cancellation. This is valuable but invasive. It needs direct tests and a careful audit.

### Steps

1. Inspect `src/mcp/server.rs` and `src/mcp/runtime.rs` for:
   - active request insertion and removal;
   - cancellation notification handling;
   - request ID validation;
   - cleanup after normal completion;
   - cleanup after timeout;
   - cleanup after handler error;
   - handling of notifications that should not produce responses;
   - handling of requests without IDs;
   - rate-limit and in-flight-limit error paths;
   - writer task shutdown and channel close behavior.
2. Confirm `notifications/cancelled` still validates request IDs as string or integer and ignores bool/object/oversized IDs.
3. Confirm active request map entries are always removed even if a spawned task returns early.
4. Confirm parse errors, invalid requests, and batch rejections do not consume active request slots.
5. Confirm stdout writes cannot interleave and always emit one JSON object per line.
6. Confirm EOF handling drains in-flight requests and then flushes the writer.

### Tests to add

Where possible, add async tests around helper functions. If the server is hard to test as a stdio binary, extract testable helpers rather than mocking stdin/stdout with brittle code.

Add tests for:

- Active request insertion/removal.
- Cancellation notification sets an active request flag.
- Cancellation notification for unknown request ID is harmless.
- Bool/object/oversized cancellation IDs are ignored.
- In-flight limit returns a structured JSON-RPC error.
- Notification requests do not generate responses.
- Writer channel serializes multiple response values as separate lines if a testable writer helper exists.

### Acceptance criteria

Concurrent MCP behavior has direct tests for the active-request and cancellation paths. The architecture docs accurately describe the current implementation. No stale references to the removed `CancelledRequests`/`MAX_CANCELLED_REQUESTS` model remain unless a bounded cancelled-ID cache still exists in code.

## Task 4: clean runtime and parity documentation drift

### Problems to check

After phase 04, documentation may still mention `MAX_CANCELLED_REQUESTS` or a cancellation set even though the implementation moved to `ActiveRequests`. After phase 05, parity docs/agent notes may still say the Rust full profile has 68 tools or that the Python gap is exactly three tools.

### Steps

1. Search for stale text:

```bash
rg "68 tools|MAX_CANCELLED_REQUESTS|CancelledRequests|cancellation set|3-tool gap|53 known failures|Rust `full` profile ships 68|Python defines 67" .
```

2. Replace stale fixed counts where generated docs are not responsible for them.
3. In parity docs, distinguish:
   - Python parity failures for tools that should match existing Python behavior;
   - Rust-superset tools intentionally absent from Python;
   - known test-harness audience/profile issues.
4. Ensure docs do not imply generated docs are dynamic if they are committed static generated blocks.
5. Run `cargo run --bin generate-docs -- --check` after doc edits.

### Acceptance criteria

No stale references remain to 68-tool inventory, removed cancelled request tracking, or old parity gap counts unless explicitly historical and dated. Docs consistently state 71 registered tools only where generated from the current registry, or avoid fixed counts in hand-maintained prose.

## Task 5: verify new repo/diff/path tools are bounded and route-safe

### Tools in scope

- `repo_tree_summarize`
- `diff_risk_classify`
- `path_batch_scope_check`

### Steps

1. Inspect each tool's input schema and handler for bounds:
   - maximum path count;
   - maximum patch/text length;
   - maximum output findings;
   - budget checks for long loops;
   - no filesystem access;
   - no network access;
   - no command execution.
2. Confirm machine codes are declared in `src/mcp/machine_codes.rs` and documented in `architecture/machine-codes.md`.
3. Confirm route-critical status is intentional. If `diff_risk_classify` or `path_batch_scope_check` are used by harness routing, add them to the route-critical list and route-critical tests. If not, document that they are advisory.
4. Confirm `path_batch_scope_check` is harness-only or contextual intentionally. Harness-only is reasonable for patch gate use; contextual is reasonable only if model-visible path triage is needed.
5. Confirm `repo_tree_summarize` and `diff_risk_classify` have enough tests for common repo shapes and patch categories.

### Tests to add or confirm

For `repo_tree_summarize`:

- Rust crate path list.
- Mixed Rust/Python/Node repo.
- Docs-only repo.
- Generated/vendor-heavy repo.
- Unicode and Windows-style paths.
- Too many paths.

For `diff_risk_classify`:

- Docs-only diff.
- Test-only diff.
- Source diff.
- Dependency manifest diff.
- CI workflow diff.
- Security-sensitive path diff.
- Binary patch or malformed diff.
- Oversized patch.

For `path_batch_scope_check`:

- All paths inside root.
- `../` escape.
- absolute targets allowed/disallowed.
- duplicate normalized paths.
- Windows-style separators.
- Unicode paths.
- excessive target count.

### Acceptance criteria

All three tools are bounded, deterministic, file-system-free, and covered by category-specific tests. Machine codes and verdicts are stable enough for codegg routing. Generated docs include all three tools in the intended categories and profiles.

## Task 6: verify route-critical response contracts

### Steps

1. Inspect `tests/mcp/test_route_contracts.rs` and confirm it covers:
   - `edit_preflight`
   - `command_preflight`
   - `config_preflight`
   - `patch_apply_check`
   - `text_security_inspect`
   - `dependency_edit_preflight` if used as a harness gate
   - `diff_risk_classify` if route-critical
   - `path_batch_scope_check` if route-critical
2. Ensure each tested tool has success and review/block/error coverage.
3. Assert response envelope and result object include expected `machine_code` and `verdict` where contractually required.
4. Assert `recommended_next_tool` names, if present, exist in the registry.
5. Assert findings use canonical fields and known severities/dispositions.

### Acceptance criteria

Route-critical tests fail if a route-critical tool omits machine code, verdict, canonical findings, or references a nonexistent next tool.

## Task 7: verify generated docs and profile snapshots

### Steps

1. Run generated docs check.
2. Inspect the generated README section for exactly one begin and one end marker.
3. Inspect `architecture/mcp-server.md` profile reference for exactly one begin and one end marker.
4. Confirm `generated/tool-cards.md` includes tool cards for the new model-visible tools:
   - `repo_tree_summarize`
   - `diff_risk_classify`
5. Confirm harness-only `path_batch_scope_check` appears as harness-only where appropriate and not in model-facing cards unless the generator intentionally includes harness cards.
6. Confirm profile counts match registry counts.

### Acceptance criteria

Generated docs are clean, marker integrity tests pass, and no manual generated-block edits are needed.

## Task 8: verify public API compatibility impact

### Steps

1. Check the `max_text_chars` to `max_text_bytes` rename for public API breakage.
2. If `ToolBudget` is public and the crate is not making a breaking version bump, consider restoring a compatibility method or deprecated alias where possible.
3. Check `SubBudget` public visibility and fields for the same issue.
4. Check `ToolRegistry::available_tools()` deprecation impact. Deprecation is acceptable; removal is not.
5. Run docs generation and `cargo package` to catch docs.rs/API issues.

### Acceptance criteria

Any public API break is either reverted, made additive/backward-compatible, or explicitly documented with an appropriate versioning decision.

## Task 9: final verification report

At the end of the pass, add a short verification note to the PR or commit message summarizing:

- commands run and pass/fail status;
- whether parity was run and, if not, why;
- any known remaining failures and whether they are pre-existing;
- corrected issues;
- deferred items.

Do not claim CI is green unless GitHub Actions or local command output confirms it.

## Suggested commit structure

Use small commits:

1. `fix(preflight): align patch apply wrapper with schema and harness audience`
2. `test(mcp): cover active request cancellation and cleanup`
3. `docs: remove stale runtime and parity drift`
4. `test(tools): harden repo diff path intelligence coverage`
5. `chore: refresh generated docs and verification notes`

## Done criteria

This corrective pass is complete when:

- the full verification command matrix passes locally or failures are explicitly documented as pre-existing/out-of-scope;
- wrapper/schema/audience mismatches are fixed;
- concurrent MCP runtime behavior has direct tests;
- stale docs are cleaned;
- new repo/diff/path tools have adequate bounds and tests;
- generated docs are current;
- no unintentional public API break remains.
