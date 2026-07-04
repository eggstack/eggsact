# Final Polish Plan for Phase 06–09 Closure

## Purpose

This plan captures the narrow remaining polish after the phase 06–09 cleanup execution. The repo is now close to closure for this line of work: documentation/handoff guidance was restored, README tool-count drift was fixed, `line_range` fallback behavior was tightened, parser-backed config/dependency logic is in place, and cooperative cancellation is now threaded through the MCP/server and in-process API paths.

The remaining work should avoid new feature scope. It should prove the implementation through explicit tests, tighten any weak assertions, and record verification evidence.

## Current State

### Strong Areas

- `AGENTS.md` has been restored with commands, CI gates, structure, Agent API notes, exposure/audience guidance, codegg usage guidance, skills, and gotchas.
- README static tool-count references now say 67 tools, matching the generated block.
- `edit_preflight` no longer falls back to `original` for `line_range` replacement text after validation.
- `command_preflight` platform docs/schema are explicit: POSIX supported, auto resolves to POSIX, Windows recognized but unsupported.
- `config_file_inspect` has parser-backed recursive traversal for JSON and TOML-family formats.
- `dependency_edit_preflight` has parser-backed extraction for Cargo, pyproject, and package.json.
- `BudgetContext`, thread-local cancellation propagation, `for_handler()`, `call_json_with_context()`, and MCP timeout cancellation flag wiring now exist.
- Cancellation tests exist for thread-local flag scoping, `for_handler()`, `call_json_with_context()`, and budget expiry.

### Remaining Polish Items

- Cancellation tests currently prove propagation but use permissive assertions in some cases (`resp.ok || resp.error.is_some()`). High-risk tools should have at least one deterministic pre-cancelled test that asserts `CANCELLED`.
- Parser-backed tests should explicitly prove traversal paths for config and dependency inspectors, not just broad tool success.
- Documentation should be checked for hard-coded tool counts or stale references outside README.
- `AGENTS.md` now documents `cargo test --test lib mcp`; confirm this command matches the current test harness layout and does not mislead agents.
- CI/status could not be verified from GitHub combined status; local or workflow evidence should be recorded if available.

## Workstream A: Strengthen Cancellation Assertions

### Problem

The cancellation propagation path exists, but some tests only assert that calls either succeed or error. That proves the API call does not panic, but not that high-risk handlers deterministically honor cancellation when they are expected to.

### Required Work

1. Identify handlers that call `for_handler()` or otherwise inherit the thread-local cancel flag.

   Minimum expected set:

   - `command_preflight`.
   - `edit_preflight`.
   - `config_preflight`.
   - `config_file_inspect`.
   - `dependency_edit_preflight`.

2. For each handler that checks cancellation early, add deterministic tests using `ToolRegistry::call_json_with_context(..., Some(cancel_flag))` with a pre-set `AtomicBool(true)`.

3. Assert the exact cancellation contract where expected:

   - `ok == false`.
   - `error_type == "cancelled"` or equivalent envelope field.
   - `machine_code == CANCELLED`.
   - `tool == <tool_name>` if present.

4. Keep a separate compatibility test for handlers that intentionally do not check cancellation early.

   Example: `math_eval` may continue to completion. That test should explicitly say it is a non-cooperative compatibility example, not a proof of cancellation enforcement.

5. Add a cancellation test for a context-aware composite path that loops over multiple items.

   Good candidates:

   - `config_file_inspect` with many parsed keys.
   - `dependency_edit_preflight` with many dependencies.

   The test can set the flag before dispatch. It should fail fast before doing substantial work.

### Acceptance Criteria

- At least one high-risk handler test deterministically returns `CANCELLED`.
- Non-cooperative handlers are documented as such in test names/comments.
- The test suite differentiates propagation from enforcement.

## Workstream B: Audit Context Creation in High-Risk Handlers

### Problem

`for_handler()` exists, but all high-risk handlers must actually use it instead of `BudgetContext::new(...)` directly. A handler that creates `BudgetContext::new(...)` will not inherit MCP/in-process cancellation flags.

### Required Work

1. Search for direct high-risk handler context construction:

   ```bash
   rg "BudgetContext::new" src/tools src/mcp src/agent
   rg "for_handler" src/tools src/mcp src/agent
   ```

2. Replace direct `BudgetContext::new(...)` inside handler entry points with:

   ```rust
   crate::mcp::budget::for_handler(<budget>)
   ```

3. Keep direct `BudgetContext::new(...)` in tests or non-handler construction paths where no caller context exists.

4. Add a regression test or code comment for the policy:

   - handler entry points use `for_handler()`;
   - callers/wrappers use `BudgetContext::new(...)`.

5. Ensure documentation in `AGENTS.md` remains accurate after any change.

### Acceptance Criteria

- High-risk handler entry points inherit thread-local cancellation.
- Direct `BudgetContext::new(...)` remains only in tests, caller contexts, or clearly justified non-handler paths.

## Workstream C: Parser-Backed Config Fixture Audit

### Problem

The implementation has parser-backed JSON/TOML traversal, but tests must prove behavior that line scanners could not easily provide.

### Required Config Tests

Add or verify these exact-style fixtures:

1. Nested JSON secret key.

   Input:

   ```json
   {"auth": {"api_key": "secret-value"}}
   ```

   Expected:

   - finding code `CONFIG_RISK_SECRET_KEY`.
   - key path includes `auth.api_key`.
   - value preview is masked and does not contain full `secret-value`.

2. JSON boolean debug flag.

   Input:

   ```json
   {"debug": true}
   ```

   Expected:

   - finding code `CONFIG_RISK_DEBUG_FLAG`.
   - key path is `debug`.
   - boolean value is handled by parsed traversal.

3. package.json nested script hook.

   Input:

   ```json
   {"scripts": {"postinstall": "node install.js"}}
   ```

   Expected:

   - finding code `CONFIG_RISK_COMMAND_HOOK`.
   - key path includes `scripts.postinstall`.

4. TOML nested TLS disable.

   Input:

   ```toml
   [service.auth]
   verify_tls = false
   token = "abc123"
   ```

   Expected:

   - TLS-disable finding uses dotted parsed key path.
   - secret finding uses `service.auth.token` or equivalent.

5. Malformed JSON.

   Expected:

   - `CONFIG_PARSE_FAILED` finding.
   - blocking/high severity or documented route behavior.

6. YAML heuristic-only behavior.

   Expected:

   - either explicit heuristic note in result/limits/findings, or documented lack of parser-backed precision.

### Acceptance Criteria

- Tests distinguish parser traversal from line-based fallback.
- Secret masking is verified.
- Parse failure behavior is deterministic.

## Workstream D: Parser-Backed Dependency Fixture Audit

### Problem

Cargo, pyproject, and package.json parser-backed extraction exists, but fixture coverage should lock down the cases that motivated the parser work.

### Required Dependency Tests

1. Cargo inline table dependency addition.

   ```toml
   [dependencies]
   serde = { version = "1", features = ["derive"] }
   ```

   Expected: `DEPENDENCY_ADDED` for `serde`.

2. Cargo git dependency addition.

   ```toml
   [dependencies]
   mydep = { git = "https://example.invalid/repo.git" }
   ```

   Expected: `DEPENDENCY_GIT_SOURCE`.

3. Cargo path dependency addition.

   ```toml
   [dependencies]
   local = { path = "../local" }
   ```

   Expected: `DEPENDENCY_PATH_SOURCE` unless policy allows path deps and the tool intentionally downgrades severity. In either case, result should identify `path` source.

4. Cargo target-specific dependency.

   ```toml
   [target.'cfg(unix)'.dependencies]
   nix = "0.28"
   ```

   Expected: dependency addition detected.

5. Cargo workspace dependency.

   ```toml
   [workspace.dependencies]
   anyhow = "1"
   ```

   Expected: dependency addition detected with workspace source/section detail where available.

6. pyproject dependency and optional dependency.

   Expected: additions detected from parsed TOML arrays.

7. pyproject build-backend change.

   Expected: metadata/build-backend finding or hook change detail.

8. package.json postinstall script.

   Expected: script/hook finding from parsed JSON, not line scanner.

9. package.json git/tarball dependency.

   Expected: source/risk classification.

10. requirements editable/direct URL/local path.

   Expected: review findings for each source type.

### Acceptance Criteria

- Tests cover TOML inline tables and nested sections.
- Tests cover parsed JSON script/dependency maps.
- Result shape includes enough detail for codegg to explain why the edit is review/block.

## Workstream E: Documentation and Command Accuracy Pass

### Problem

The README and AGENTS content is now restored/aligned, but command snippets and generated docs should be audited one more time for stale references.

### Required Work

1. Verify all documented commands are valid for this repo.

   Check in particular:

   - `cargo test --test lib mcp`.
   - `cargo test --test lib parity`.
   - `cargo test --test lib text`.
   - `cargo fmt --check` versus `cargo fmt --all -- --check`.
   - `cargo clippy --all-targets --all-features` versus CI’s `-D warnings` form.

2. Prefer CI-equivalent command forms in `AGENTS.md` and `.skills/testing.md`.

3. Remove or generator-manage hard-coded tool counts.

   If hard-coded counts remain, add generator tests to catch drift.

4. Verify generated docs:

   ```bash
   cargo run --bin generate-docs -- --check
   ```

5. Ensure tool cards mention:

   - `line_range` requires `new`.
   - Windows command platform is recognized but unsupported.
   - dependency/config parser limitations and YAML heuristic behavior.

### Acceptance Criteria

- No stale or misleading command snippets remain.
- Generated docs check passes.
- Tool count drift cannot recur silently or is minimized by removing static counts.

## Workstream F: Verification Evidence

### Problem

GitHub combined status has repeatedly returned no statuses, so repo state cannot be externally verified from the connector. The implementation handoff should record local verification evidence.

### Required Work

Run and record:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If parity tests require `../eggcalc`, document whether they ran or were skipped, and why.

If any command fails, include:

- exact command;
- exact failure summary;
- first relevant error block;
- whether failure is environmental or code-related.

### Acceptance Criteria

- Final implementation notes contain command evidence.
- CI absence is not confused with CI success.
- Any known environmental dependency, such as Python `eggcalc`, is documented.

## Recommended Implementation Order

1. Audit `BudgetContext::new` versus `for_handler()` usage and patch handler entry points.
2. Strengthen cancellation tests to assert `CANCELLED` for at least one high-risk handler.
3. Add parser-backed config fixtures.
4. Add parser-backed dependency fixtures.
5. Audit docs/commands/tool-card caveats.
6. Run full verification and record evidence.

## Final Acceptance Criteria

This polish pass is complete when:

- High-risk handlers inherit cancellation via `for_handler()` or have an explicit reason not to.
- Tests prove deterministic cancellation for cooperative high-risk handlers.
- Tests prove parser-backed config and dependency behavior for nontrivial nested/inline fixtures.
- Documentation commands match the actual repo and CI-equivalent gates.
- Generated docs are current.
- Full verification output is recorded, including any environmental caveats.

## Non-Goals

Do not add new MCP tools.

Do not implement Windows shell parsing.

Do not add package registry or vulnerability lookups.

Do not redesign the MCP server concurrency model.

Do not begin phase 10 evaluator isolation work.
