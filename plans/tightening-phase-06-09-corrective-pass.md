# Corrective Tightening Plan for Phases 06–09

## Purpose

This plan tightens the broad multi-phase implementation that landed after the phase 06–09 handoff plans. The recent commits moved the repository substantially forward: edit preflight is closer to a canonical harness gate, command preflight has a policy engine, repo/config/dependency inspectors exist, and a budget module now provides runtime resource metadata.

The implementation is directionally correct, but it landed across phases 06, 07, 08, and 09 at once. This corrective pass should align semantics, close correctness gaps, harden parsers, tune exposure/profile boundaries, and make runtime budgets enforceable rather than mostly descriptive.

## Current State Summary

### Phase 6: Edit Preflight

Implemented:

- `edit_preflight` validates replacement modes: `literal`, `patch`, `line_range`.
- New edit machine codes exist: `EDIT_MODE_INVALID`, `EDIT_ARGUMENTS_MISSING`, `EDIT_ARGUMENTS_CONFLICT`, and `LINE_RANGE_INVALID`.
- Mode-specific missing/conflicting argument checks exist.
- Metadata field length bounds exist.
- Path scope, newline, Unicode, and fingerprint checks are partially integrated.

Known gaps:

- `line_range` mode currently rejects `new`, so it does not clearly validate the replacement text for a line-range edit.
- Some mode-contract failures return non-OK tool responses instead of route-critical allow/review/block style responses. This may be fine, but it must be documented and tested consistently.
- Metadata oversize uses `EDIT_ARGUMENTS_MISSING`, which is semantically wrong.
- Path/newline/Unicode priority mapping should be reviewed so primary machine code, verdict, and `ok_to_apply` cannot diverge.

### Phase 7: Command Preflight

Implemented:

- `command_preflight` supports `default`, `strict`, and `permissive` policies.
- `policy_config` supports allow/deny commands, allow/deny subcommands, and behavior toggles such as `allow_network` and `allow_filesystem_write`.
- POSIX shell parsing is composed through `shell_split`.
- Program/subcommand classification exists.
- Destructive pattern checks exist for pipe-to-shell, `rm`, destructive git, `chmod`, and `chown` patterns.
- Findings, matched rules, primary machine-code selection, and next-tool hints are emitted.

Known gaps:

- Schema accepts `windows`, but the handler immediately returns `UNSUPPORTED_FEATURE` for Windows.
- `custom` policy mode was planned but is not accepted.
- `POLICY_REVIEW` is emitted as a finding code but is not a machine-code constant.
- Some policy defaults are debatable: `cargo build` and `cargo bench` are allowed by default even though they execute build scripts/tests/bench code and can be expensive.
- Feature findings can duplicate behavioral findings and may overcount risk.

### Phase 8: Repo, Config, and Dependency Inspectors

Implemented:

- New `repo_manifest_inspect`, `config_file_inspect`, and `dependency_edit_preflight` tools exist.
- Repo classification supports Rust, Python, Node, Go, mixed, and unknown from bounded path lists.
- Dependency edit preflight detects additions, removals, version/source changes, build scripts, patch sections, Python URL deps, and Node script changes.
- Config inspection detects secret-like keys, insecure URLs, debug flags, command hooks, TLS disablement, and wildcard host settings.
- Specs, schemas, generated docs, tool cards, and machine codes were expanded.

Known gaps:

- Many parsers are line-oriented heuristics. They are useful for first pass inspection but not production-grade parsing.
- `config_file_inspect` treats JSON/package_json/pyproject/cargo_toml/toml/yaml through a TOML-like key-value extractor, and JSON parse validation checks only outer delimiters.
- Cargo dependency parsing likely misses inline table dependencies and richer TOML forms.
- `dependency_edit_preflight` defaults unknown auto-detected ecosystems to Rust, which can produce misleading results.
- `dependency_edit_preflight` is `ToolExposure::Default` even though it is composite and harness-oriented.

### Phase 9: Runtime Budgeting

Implemented:

- `ToolBudget`, `BudgetTier`, `BudgetContext`, `SubBudget`, and `CompositeBudgetAllocator` exist.
- Tool cost maps to budget tier; composite tools get heavy budget overrides.
- MCP `tools/call` derives timeout from tool budget.
- `truncate_response` caps findings and annotates `limits_applied`.
- In-process `ToolRegistry::call_json_with_budget` exists.

Known gaps:

- `call_json_with_budget` executes normally and only truncates afterward. It does not enforce input size, elapsed deadlines, cancellation, or cooperative budget checks.
- MCP timeout still cannot kill spawned blocking work; this is documented in comments but not fully exposed to callers.
- Result truncation does not actually truncate result payloads; it only annotates `limits_applied`.
- Findings truncation keeps `max_findings` and then appends one more synthetic finding, so behavior contradicts some docs.
- Composite sub-budgets exist as types but are not clearly applied through existing composite tools.

## Workstream A: Semantic Alignment and Documentation Corrections

### Required Fixes

1. Correct `ToolRegistry::with_compat_mode` docs.

   The comment currently says `EggcalcPython` preserves bool-as-int coercion. Earlier closure work intentionally aligned behavior so booleans are rejected for numeric fields. Update the comment to say:

   - EggcalcPython preserves Python-style type names and selected compatibility error wording.
   - It does not allow JSON booleans for numeric fields.

2. Audit all docs for `generate_docs` versus `generate-docs` and for bool-as-int claims.

3. Update architecture docs for the phase 06–09 implementation state.

   Add concise sections for:

   - edit mode contracts.
   - command policy semantics.
   - repo/dependency inspector limitations.
   - budget semantics and known non-killable blocking work limitation.

4. Regenerate generated docs and cards after any spec/schema exposure changes.

### Acceptance Criteria

- No docs claim bool-as-int coercion where validators reject bools.
- Implementation comments match runtime behavior.
- Generated docs pass check mode.

## Workstream B: Edit Preflight Correctness Pass

### Required Fixes

1. Decide and implement the final `line_range` contract.

   Preferred contract:

   - `line_range` requires `start_line`, `end_line`, and `new` replacement text.
   - `line_range` rejects `old` and `patch`.
   - Unicode/newline inspection should inspect `new`.
   - Fingerprint behavior should remain source/range-oriented as currently documented.

   Alternative contract:

   - `line_range` is only a range extraction preflight and does not validate replacement text.
   - If this is chosen, rename or document it clearly because it is not a full edit preflight mode.

   Preferred action: implement `new` replacement support.

2. Fix metadata error codes.

   Oversized metadata should use `EDIT_ARGUMENTS_INVALID`, `EDIT_METADATA_TOO_LARGE`, or generic `INVALID_ARGUMENTS`, not `EDIT_ARGUMENTS_MISSING`.

   If adding a new code:

   - add constant to `machine_codes.rs`.
   - add to `ALL`.
   - update `architecture/machine-codes.md`.
   - add tests.

3. Normalize edit-finding codes.

   Replace local string-only codes such as `NO_MATCH`, `MULTIPLE_MATCHES`, `PATCH_ERROR`, and `INVALID_RANGE` with established machine codes where possible:

   - no match: `AMBIGUOUS_REPLACEMENT` or add `EDIT_OLD_TEXT_NOT_FOUND` as a true constant rather than alias.
   - multiple matches: `AMBIGUOUS_REPLACEMENT`.
   - patch error: `PATCH_FAILED` or `EDIT_FAILED`.
   - invalid range: `LINE_RANGE_INVALID`.

4. Centralize primary-code selection.

   Implement a helper that maps edit findings to:

   - primary machine code.
   - verdict.
   - `ok_to_apply`.
   - summary.
   - optional recommended next tool.

   Priority should be:

   1. path scope escape.
   2. invalid/missing/conflicting mode arguments.
   3. invalid line range.
   4. patch failure.
   5. no match/multiple matches.
   6. fingerprint mismatch.
   7. Unicode risk.
   8. newline inconsistency.
   9. success.

5. Add/update tests.

   Required cases:

   - line_range requires replacement text if preferred contract is chosen.
   - line_range replacement text is checked for Unicode/newline issues.
   - metadata oversize uses correct machine code.
   - no local ad hoc finding codes appear for known edit failure cases.
   - mixed findings select the highest-priority primary machine code.

### Acceptance Criteria

- `edit_preflight` has one documented contract per mode.
- Machine codes and finding codes are stable and enumerated.
- `verdict`, `ok_to_apply`, and primary machine code are deterministic and aligned.

## Workstream C: Command Policy Tightening Pass

### Required Fixes

1. Resolve Windows semantics.

   Choose one:

   - remove `windows` from accepted schema/valid platforms until implemented; or
   - implement minimal Windows feature detection for `cmd.exe`, PowerShell, encoded commands, redirection, and chaining.

   Preferred action for this corrective pass: remove or mark Windows as unsupported in schema/tool cards if full parsing is not ready.

2. Add `custom` policy or remove it from docs.

   If implementing `custom`:

   - require `policy_config`.
   - default classification should be review when no allow rule matches.
   - deny rules still dominate.

   If not implementing: update plan/docs/tool cards to only list `default`, `strict`, and `permissive`.

3. Add stable machine code for policy-review findings.

   Options:

   - define `SHELL_POLICY_REVIEW`; or
   - use `SHELL_RISK` consistently.

   Preferred action: add `SHELL_POLICY_REVIEW` if the distinction is useful to codegg; otherwise use `SHELL_RISK`.

4. Revisit command default policy.

   Suggested adjustments:

   - `cargo check`: allow.
   - `cargo test`: allow or review depending on codegg policy; default can allow if project-local execution is expected.
   - `cargo build`: review, because build scripts may run.
   - `cargo bench`: review, because it can be expensive and executes code.
   - `cargo fmt --check`: allow.
   - `cargo fmt`: review.
   - `python -m pytest`: review or allow only when explicitly in project policy.
   - `npm run`: review by default.

5. De-duplicate feature findings.

   Avoid emitting both a specific behavioral finding and a generic `RISKY_SHELL_FEATURE` for the same underlying condition unless the generic finding provides distinct value.

6. Add tests.

   Required cases:

   - `cargo build` routes to review if policy is adjusted.
   - `cargo bench` routes to review if policy is adjusted.
   - `curl URL | sh` blocks.
   - `rm -rf /` blocks.
   - `git reset --hard` blocks or reviews according to documented policy.
   - custom policy behavior if implemented.
   - Windows request behavior matches schema/docs.
   - policy-review finding code is enumerated.

### Acceptance Criteria

- Schema, docs, and implementation agree on platform and policy values.
- Built-in policy defaults are conservative enough for codegg.
- Findings use enumerated codes.
- Duplicate/low-value findings are reduced.

## Workstream D: Repo/Config/Dependency Inspector Hardening

### Required Fixes

1. Improve JSON/package.json parsing.

   For JSON-like formats, use `serde_json` parsing rather than delimiter checks.

   - If parse fails, return `CONFIG_PARSE_FAILED`.
   - For package.json, inspect parsed object fields for dependencies/scripts instead of line scanning where feasible.
   - Preserve line-scanning fallback only if documented as heuristic.

2. Improve TOML parsing for Cargo and pyproject.

   The repo already depends on TOML-related crates. Prefer `toml` or `toml_edit` parsing for:

   - Cargo dependency sections.
   - inline table dependencies.
   - `[target.'cfg(...)'.dependencies]`.
   - `[workspace.dependencies]`.
   - `[patch.*]` and `[replace]`.
   - package build script fields.

   Keep text heuristics only as fallback, not primary parsing.

3. Fix unknown ecosystem default.

   `dependency_edit_preflight` should not default unknown auto-detection to Rust. Preferred behavior:

   - `ecosystem=auto` and unknown file/content returns review with `DEPENDENCY_UNKNOWN_ECOSYSTEM` or `INVALID_ARGUMENTS` depending on whether this is considered input error.
   - Add a machine code if needed.

4. Tighten dependency finding codes.

   Replace string literals like `DEPENDENCY_ADDED` with `machine_codes::DEPENDENCY_ADDED` consistently.

5. Revisit exposure/profile settings.

   Recommended:

   - `repo_manifest_inspect`: `Contextual` is acceptable.
   - `config_file_inspect`: `Contextual` is acceptable if model-facing inspection is intended.
   - `dependency_edit_preflight`: change from `Default` to `HarnessOnly` or `Contextual`. Preferred: `HarnessOnly` if used as an edit gate; `Contextual` if models should ask for dependency risk summaries.

   If changed to `HarnessOnly`, update profile tests and generated docs expectations.

6. Add parser fixtures.

   Required fixtures:

   - Cargo inline dependency: `serde = { version = "1", features = ["derive"] }`.
   - Cargo git dependency.
   - Cargo path dependency.
   - Cargo target-specific dependency.
   - Cargo workspace dependency.
   - Cargo build script added.
   - package.json valid JSON with scripts/dependencies.
   - malformed package.json.
   - pyproject dependencies from valid TOML.
   - requirements direct URL.
   - unknown file with ecosystem auto.

### Acceptance Criteria

- Inspector parsers are format-aware for JSON and TOML.
- Unknown ecosystems do not silently become Rust.
- Dependency/config findings use stable machine-code constants.
- Exposure settings match intended codegg usage.

## Workstream E: Runtime Budget Enforcement Pass

### Required Fixes

1. Clarify and enforce input budgets.

   Add pre-execution checks in `call_json_with_budget` and MCP `tools/call` for serialized argument size against `ToolBudget.max_input_bytes`.

   Decide whether `max_text_chars` remains handler-local or is centrally enforced through schemas/helpers. Document the split.

2. Align MCP global output limit with tool budget output limit.

   Current MCP path checks `MAX_OUTPUT_BYTES` before applying budget truncation. This can bypass per-tool budget semantics. Preferred behavior:

   - apply `truncate_response` first according to tool budget.
   - serialize.
   - if still over MCP absolute hard cap, return `OUTPUT_TOO_LARGE`.

3. Make result truncation real or rename it.

   Current `truncate_response` records `result_truncated` but does not truncate the result. Choose one:

   - actually truncate large string fields and/or replace oversized result with a summary object; or
   - rename the behavior to `result_over_budget` and document that it is annotation-only.

   Preferred action: implement deterministic truncation for large string fields and large arrays, while preserving route-critical keys.

4. Fix findings cap semantics.

   Choose one:

   - cap retained findings at `max_findings - 1` and append truncation notice so total length <= `max_findings`; or
   - document and rename budget field to `max_retained_findings`.

   Preferred action: total findings length should not exceed `max_findings`.

5. Wire sub-budgets into composites.

   Start with:

   - `edit_preflight`.
   - `command_preflight`.
   - `config_preflight`.
   - `config_file_inspect`.
   - `dependency_edit_preflight`.

   At minimum, create a `BudgetContext` at the top-level in typed/in-process calls and pass reduced limits into sub-tool checks where practical. If full threading through every handler is too invasive, document the staged approach and add tests for the helpers already introduced.

6. Add budget-related tests.

   Required cases:

   - `call_json_with_budget` rejects serialized args over max input bytes.
   - MCP applies budget truncation before hard output cap.
   - findings total length does not exceed configured cap.
   - result truncation behavior matches docs.
   - cancellation/deadline limitations are documented in architecture.

### Acceptance Criteria

- Budget APIs enforce at least input and output constraints, not only annotate after execution.
- Per-tool budget limits and MCP hard caps have clear precedence.
- Findings cap semantics are internally consistent.
- Composite sub-budget story is either implemented or explicitly staged with tests.

## Workstream F: Machine-Code and Route-Contract Consistency

### Required Fixes

1. Audit all newly added finding codes.

   Ensure every machine-like code emitted by route-critical tools is either:

   - a constant in `machine_codes.rs` and listed in `ALL`; or
   - documented as a local finding `kind`, not a machine code.

2. Add a test helper that scans route-critical tool outputs for known fixture cases and asserts:

   - envelope `machine_code` is in `machine_codes::ALL`.
   - every finding `code` that is uppercase snake-case is either in `ALL` or in an explicitly allowed local-finding list.

3. Update `architecture/machine-codes.md`.

4. Regenerate docs/tool cards.

### Acceptance Criteria

- Route-critical outputs have stable enumerated machine codes.
- Ad hoc uppercase finding codes do not proliferate.
- Documentation and generated cards are current.

## Workstream G: Test and CI Closure

### Required Test Additions

1. Edit preflight:

   - line-range replacement contract.
   - metadata oversize code.
   - priority mapping.

2. Command preflight:

   - adjusted cargo defaults.
   - platform schema/implementation agreement.
   - enumerated policy-review code.
   - no duplicate feature findings for common cases.

3. Inspectors:

   - serde_json parse success/failure.
   - TOML parse for inline dependencies.
   - unknown ecosystem.
   - exposure/profile expectations for dependency preflight.

4. Budgets:

   - input cap.
   - output/result truncation.
   - findings cap exact length.
   - MCP hard cap fallback.

5. Generated docs:

   - `cargo run --bin generate-docs -- --check` passes after profile/spec changes.

### Verification Commands

Run the full gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If any command cannot be run, record it in the handoff summary with exact reason and known risk.

## Recommended Implementation Order

1. Fix semantic drift and docs (`with_compat_mode`, generated command/docs, architecture notes).
2. Tighten edit preflight contract and error/machine-code mapping.
3. Tighten command policy schema/platform/defaults and enumerated codes.
4. Harden JSON/TOML parsing in repo/dependency/config inspectors.
5. Adjust tool exposure/profile settings.
6. Make budget enforcement real for input/output/finding caps.
7. Add route-contract/machine-code consistency tests.
8. Regenerate docs and run full verification.

## Final Acceptance Criteria

This corrective pass is complete when:

- Phase 6 edit preflight mode contracts are explicit, tested, and route-safe.
- Phase 7 command policy schema/docs/implementation agree, and default policy is conservative for codegg.
- Phase 8 inspectors use real JSON/TOML parsing where available and do not silently misclassify unknown ecosystems.
- Phase 9 budgets enforce input/output/finding caps consistently in MCP and in-process paths.
- Tool exposures match intended model versus harness usage.
- Machine-code and finding-code usage is enumerated and documented.
- Generated docs/tool cards are current.
- Full CI verification passes or failures are documented precisely.

## Non-Goals

Do not add new large external data sources or network lookups.

Do not implement vulnerability scanning or package registry queries.

Do not redesign the MCP protocol layer.

Do not implement a full async/concurrent MCP server.

Do not start phase 10 evaluator isolation work in this pass.
