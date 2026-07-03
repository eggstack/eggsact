# Final Cleanup Plan for Phase 06–09 Work

## Purpose

This plan closes the remaining cleanup items after the phase 06–09 follow-up implementation. The technical direction is now strong: `line_range` edit preflight has been tightened, command platform support is explicitly documented, config/dependency inspectors use parser-backed traversal for core formats, and budget/cancellation scaffolding has been expanded.

The remaining work is mostly hygiene and precision:

- verify documentation deletions were intentional and restore any lost handoff content;
- remove unreachable fallbacks and stale comments after semantic tightening;
- ensure parser-backed paths are covered by tests, not only heuristic fallback paths;
- ensure cooperative budget checks are consistently threaded or explicitly staged;
- run and record final verification.

This is a cleanup pass, not a new feature pass.

## Current State Summary

### Strongly Improved Areas

- `edit_preflight` `line_range` mode now requires `start_line`, `end_line`, and `new`.
- `line_range` rejects `old` and `patch`.
- `command_preflight` schema clearly states: `posix` supported, `auto` currently POSIX, `windows` recognized but unsupported.
- `config_file_inspect` now uses recursive parsed traversal for JSON and TOML-family formats.
- `dependency_edit_preflight` now uses parser-backed extraction for Cargo, pyproject, and package.json metadata.
- requirements.txt parsing was improved for editable installs, includes/constraints, direct URLs, local paths, and unconstrained specs.
- `BudgetContext` carries deadline/cancellation state and helper methods.
- MCP sets a cancellation flag after timeout while documenting that blocking threads cannot be killed.
- Tests expanded significantly, especially around tool gaps, edge cases, composite tools, and route contracts.

### Remaining Concerns

- `AGENTS.md` was deleted. This may be intentional, but it should be verified because agent handoff instructions are often valuable for this repo workflow.
- `README.md` lost a large amount of content. It still exists and includes generated tool docs, but the deleted content should be reviewed for important lost context.
- `line_range` validation now requires `new`, but replacement-text extraction still uses `unwrap_or(original)` after validation. That fallback is unreachable for valid inputs and should be removed or made explicit.
- `BudgetContext` is now present in some handlers, but most public handlers still create local contexts rather than receiving caller-provided context. Cooperative cancellation therefore remains partial.
- Parser-backed dependency/config paths exist, but tests should prove they are used and not only that fallback heuristics still work.
- README/tool count and generated docs must be regenerated/checked after the new tools and schema descriptions.

## Workstream A: README and AGENTS Hygiene

### Problem

The last follow-up implementation deleted `AGENTS.md` and removed a large amount of README content. This may be a deliberate documentation contraction, but it risks dropping important contributor, agent, or codegg handoff details.

### Required Work

1. Review the deleted `AGENTS.md` content from the previous commit.

   Use the prior commit before deletion as the source of truth. Determine whether its guidance is:

   - obsolete and safely removable;
   - duplicated in `.skills/`, architecture docs, or plans;
   - still valuable and should be restored.

2. If useful content remains, either:

   - restore `AGENTS.md`; or
   - move the surviving guidance into `.skills/`, `architecture/overview.md`, or a concise `CONTRIBUTING.md`/`AGENTS.md` replacement.

   Preferred action: restore a compact `AGENTS.md` unless the repo has a clear policy against it.

3. Review the README deletion.

   Confirm the removed sections did not include:

   - install/use details;
   - MCP configuration examples;
   - profile guidance;
   - architecture or codegg integration notes;
   - generated-block markers required by `generate-docs`.

4. If the README contraction is intentional, ensure each removed long-form section has a replacement link to architecture docs.

   Recommended README structure:

   - short project summary;
   - install and quick start;
   - MCP mode quick start;
   - generated tool summary block;
   - links to architecture docs, machine codes, compatibility, and generated tool cards.

5. Add a small documentation regression test if practical.

   At minimum, generator tests should assert generated block markers remain in README.

### Acceptance Criteria

- `AGENTS.md` deletion is either reversed or explicitly justified by moved content.
- README remains concise but not hollow.
- Generated-doc markers are intact.
- Contributor/agent handoff guidance is preserved somewhere discoverable.

## Workstream B: Remove Unreachable Edit-Preflight Fallbacks

### Problem

`line_range` now requires `new`, but later code still uses fallback behavior such as `args.get("new").and_then(...).unwrap_or(original)`. That fallback is now unreachable for valid calls and weakens the contract if future validation changes.

### Required Work

1. Replace `line_range` replacement-text extraction with a direct validated access.

   Recommended pattern:

   ```rust
   let replacement_text = args
       .get("new")
       .and_then(|v| v.as_str())
       .expect("line_range mode validates required 'new' before inspection");
   ```

   Or refactor mode validation to return a typed enum carrying validated fields.

2. Update comments.

   Remove comments implying `new` is optional or that fallback to original is acceptable for `line_range`.

3. Apply the same cleanup to newline-policy logic if it still has optional/fallback branches for `line_range` replacement text.

4. Add tests:

   - `line_range` without `new` fails before Unicode/newline inspection.
   - `line_range` with `new` and a Unicode issue inspects `new`, not `original`.
   - `line_range` with `new` and mixed newline policy inspects `new`, not `original`.

### Acceptance Criteria

- No valid `line_range` path inspects `original` as replacement text.
- Comments, docs, and code all state `new` is required.
- Tests fail if fallback-to-original behavior returns.

## Workstream C: Parser-Backed Path Test Coverage

### Problem

Parser-backed traversal has been implemented, but tests must distinguish parser-backed behavior from heuristic fallback behavior. Without explicit fixtures, future changes could silently regress to line scanning.

### Required Work: Config Inspector Tests

Add or verify tests for `config_file_inspect` that prove parsed traversal is active.

Required cases:

1. Nested JSON secret key.

   Example:

   ```json
   {"auth": {"api_key": "secret-value"}}
   ```

   Expected: finding key path includes `auth.api_key` or equivalent parsed path.

2. JSON boolean debug flag.

   Example:

   ```json
   {"debug": true}
   ```

   Expected: debug finding despite value not being a string line token.

3. package.json nested script hook.

   Example:

   ```json
   {"scripts": {"postinstall": "node install.js"}}
   ```

   Expected: command-hook finding with key path such as `scripts.postinstall`.

4. TOML nested secret/debug/TLS keys.

   Example:

   ```toml
   [service.auth]
   token = "abc123"
   verify_tls = false
   ```

   Expected: parsed dotted key paths.

5. malformed JSON.

   Expected: `CONFIG_PARSE_FAILED` and block/invalid route behavior.

6. YAML heuristic-only behavior.

   Expected: documented heuristic path, ideally with `limits_applied`, informational finding, or result flag indicating parser precision is limited.

### Required Work: Dependency Inspector Tests

Add or verify tests for `dependency_edit_preflight` that prove parser-backed dependency extraction is active.

Required cases:

1. Cargo inline table dependency.

   ```toml
   serde = { version = "1", features = ["derive"] }
   ```

2. Cargo git dependency.

   ```toml
   mydep = { git = "https://example.invalid/repo.git" }
   ```

3. Cargo path dependency.

   ```toml
   local = { path = "../local" }
   ```

4. Cargo target-specific dependency.

   ```toml
   [target.'cfg(unix)'.dependencies]
   nix = "0.28"
   ```

5. Cargo workspace dependency.

   ```toml
   [workspace.dependencies]
   anyhow = "1"
   ```

6. pyproject dependency array and optional dependency group.

7. pyproject build-backend change.

8. package.json dependency and postinstall script via valid JSON parsing.

9. requirements editable/direct URL/local path.

### Acceptance Criteria

- Tests prove recursive JSON/TOML traversal, not just line scanning.
- Parser-backed dependency cases cover inline tables and nested sections.
- Fallback heuristics remain tested separately and clearly named as fallback tests.

## Workstream D: Cooperative Budget Context Threading

### Problem

`BudgetContext` and `CompositeBudgetAllocator` now exist, but handlers often create local contexts internally. That improves local checks, but it does not yet fully propagate caller cancellation/deadline into nested sub-tools.

### Required Work

1. Define the staged API approach.

   Preferred staged approach:

   - keep public handler signatures stable as `fn(&Value) -> ToolResponse`;
   - add internal `*_with_context(args, &BudgetContext) -> ToolResponse` helpers;
   - public handlers create default contexts;
   - `ToolRegistry::call_json_with_budget` or future registry path can call context-aware helpers for tools that support them.

2. Implement context-aware helpers for high-risk composite tools:

   - `edit_preflight_with_context`;
   - `command_preflight_with_context`;
   - `config_file_inspect_with_context`;
   - `dependency_edit_preflight_with_context`.

3. Thread sub-budget contexts into sub-tool-like phases.

   Minimum acceptable checks:

   - before expensive parser traversal;
   - before each major sub-tool call;
   - inside long loops over dependencies/config keys/findings;
   - after each sub-tool call before continuing.

4. Clarify limits of MCP cancellation.

   MCP currently creates a cancellation flag at dispatch and sets it after timeout. However, because the handler closure does not currently receive the same context directly through the generic handler signature, only context-aware paths can honor that flag.

   Document this precisely:

   - pre-dispatch cancellation is honored through existing request-id cancellation set;
   - timeout returns to caller and sets an internal flag;
   - blocking work may continue until cooperative checks observe cancellation or natural completion;
   - full generic cancellation propagation requires context-aware handler dispatch.

5. Add tests using intentionally expired/cancelled contexts.

   Unit-level tests are acceptable if generic MCP cancellation is hard to trigger deterministically.

   Required cases:

   - expired context makes context-aware config/dependency helper return `TIMEOUT`.
   - cancelled context makes context-aware helper return `CANCELLED`.
   - composite allocator splits budget and shares deadline.

### Acceptance Criteria

- Cooperative budget checks are not just local default contexts; there is a path to pass caller context into high-risk tools.
- Documentation is honest about what MCP timeout/cancellation does and does not stop.
- Tests cover expired/cancelled context behavior directly.

## Workstream E: Generated Docs and Tool Counts

### Problem

The README still says the crate ships with 64 tools in the introduction, while the generated block reports 67 tools. That discrepancy creates immediate documentation drift.

### Required Work

1. Fix static tool count references.

   Options:

   - remove hard-coded count from the prose intro; or
   - make the generator update that count too.

   Preferred action: avoid hard-coded counts outside generated blocks.

2. Regenerate docs.

   Run:

   ```bash
   cargo run --bin generate-docs
   cargo run --bin generate-docs -- --check
   ```

3. Verify generated files:

   - README generated tool table;
   - architecture profile reference block;
   - `generated/tool-cards.md`;
   - `.skills/mcp-tools.md`, if maintained by the generator or manual docs process.

4. Add/adjust generator tests to prevent count drift if practical.

### Acceptance Criteria

- No static prose conflicts with generated tool counts.
- Generated docs pass check mode.
- Tool cards mention Windows unsupported caveat and `line_range` required `new` contract.

## Workstream F: Repo Hygiene and Verification

### Required Work

1. Confirm `.gitignore` additions are intentional and do not hide source, fixtures, or generated docs that must be tracked.

2. Confirm the README contraction was intentional.

3. Restore or replace `AGENTS.md` as needed.

4. Verify no generated docs are stale.

5. Run full verification:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   cargo run --bin generate-docs -- --check
   cargo package --verbose
   ```

6. If CI/status checks are unavailable, record local command output in the handoff commit message or final implementation notes.

### Acceptance Criteria

- No accidental deletion of agent/contributor guidance.
- No stale README/generated-doc contradictions.
- Full verification passes or failures are documented with exact output.

## Recommended Implementation Order

1. Inspect deleted `AGENTS.md` and README diff; restore or relocate important content.
2. Remove unreachable `line_range` replacement-text fallbacks and update comments.
3. Add parser-backed-path tests for config and dependency inspectors.
4. Add context-aware helper path for high-risk composite tools or document staged implementation if too invasive.
5. Fix tool-count/documentation drift and regenerate docs.
6. Run full verification.

## Final Acceptance Criteria

This cleanup pass is complete when:

- README and agent/contributor guidance are intentionally shaped, with no accidental deletion.
- `line_range` replacement semantics have no unreachable fallback paths or stale comments.
- Config/dependency tests prove parser-backed traversal behavior.
- Budget context has a clear context-aware propagation path for high-risk composites, or a documented staged follow-up with tests for the implemented portion.
- Generated docs are current and tool counts do not drift.
- Full verification passes or exact failures are documented.

## Non-Goals

Do not add new tools.

Do not implement Windows shell parsing in this pass.

Do not add external dependency/vulnerability lookups.

Do not redesign MCP transport concurrency.

Do not start phase 10 evaluator isolation work.
