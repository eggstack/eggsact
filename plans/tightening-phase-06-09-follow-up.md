# Follow-Up Plan for Phase 06–09 Tightening

## Purpose

This plan closes the remaining gaps after the phase 06–09 corrective pass. The repository is now in a strong state: semantic drift was corrected, budget input/output enforcement improved, command-policy machine codes were enumerated, dependency preflight exposure was reduced, and unknown dependency ecosystems no longer silently default to Rust.

The remaining work is narrower and should be treated as a closure pass, not a redesign. The focus is:

1. Finalize `line_range` edit-preflight semantics.
2. Make command platform support unambiguous.
3. Replace heuristic config/dependency extraction with parsed traversal where practical.
4. Thread budget/deadline/cancellation context into cooperative handler execution.
5. Add tests that lock down these semantics.

## Current State

### Already Fixed

- `ToolRegistry::with_compat_mode` no longer claims bool-as-int coercion.
- `call_json_with_budget` rejects serialized arguments over `max_input_bytes` before dispatch.
- MCP applies budget-aware truncation before the absolute `MAX_OUTPUT_BYTES` hard cap.
- `truncate_response` keeps total findings length within `max_findings`.
- Oversized result payloads are replaced with route-safe summary objects.
- `EDIT_METADATA_TOO_LARGE`, `SHELL_POLICY_REVIEW`, and `DEPENDENCY_UNKNOWN_ECOSYSTEM` exist as machine-code constants.
- `cargo build` and `cargo bench` route to review in the default command policy.
- `dependency_edit_preflight` is now `Contextual` instead of `Default`.
- Route-contract tests now reject unenumerated uppercase finding codes in representative route-critical outputs.

### Remaining Gaps

- `line_range` accepts `new` but does not require it. It falls back to inspecting `original` when `new` is absent.
- `command_preflight` accepts `windows` as a valid platform but immediately returns `UNSUPPORTED_FEATURE`.
- `config_file_inspect` validates JSON/TOML parse status with real parsers, but semantic key extraction still uses line-oriented heuristics for JSON, TOML, package.json, pyproject, and Cargo.toml.
- `dependency_edit_preflight` still relies heavily on heuristic line parsing for Cargo/Python/Node metadata.
- Budget/deadline/cancellation support exists as data structures, but most handlers do not cooperatively check budget context during execution.

## Workstream A: Finalize `line_range` Edit-Preflight Semantics

### Decision Point

Choose one of the two contracts and make code, schemas, docs, typed wrappers, and tests agree.

### Preferred Contract: `line_range` Is a True Replacement Preflight

Under this contract:

- `line_range` requires `start_line`, `end_line`, and `new`.
- `line_range` rejects `old` and `patch`.
- `line_range` validates the selected source range and the proposed replacement text.
- Newline and Unicode checks inspect `new`.
- Fingerprint checks may continue to validate the original/range context.

This is preferred because `edit_preflight` is intended to gate model-authored edits, not only extract ranges.

### Alternative Contract: `line_range` Is Range Validation Only

Under this contract:

- `line_range` requires only `start_line` and `end_line`.
- `new` is optional.
- If `new` is absent, Unicode/newline checks must be skipped or explicitly marked `not_applicable`, not run against `original`.
- Docs must state that this mode validates the range but does not fully preflight a replacement unless `new` is supplied.

This alternative is less ideal for codegg because it gives harnesses a partially checked edit.

### Required Work

1. Pick the contract.

2. If using the preferred contract, update `edit_preflight` validation:

   - require `new` for `line_range`.
   - return `EDIT_ARGUMENTS_MISSING` when absent.
   - remove fallback to `original` for Unicode/newline checks.

3. If using the alternative contract:

   - do not inspect `original` as replacement text.
   - set `unicode_check`/`newline_check` to `not_applicable` or omit them when `new` is absent.
   - document that range-only validation is not sufficient to apply an edit.

4. Update schemas for `edit_preflight` if schema detail can express mode-specific required fields.

   If JSON Schema subset cannot express conditional required fields, document mode-specific requirements in tool description and generated cards.

5. Update typed wrappers and docs.

6. Add tests:

   - `line_range` missing `new` behavior matches chosen contract.
   - `line_range` with `new` inspects replacement text for Unicode risk.
   - `line_range` with `new` inspects newline policy.
   - `line_range` still rejects `old` and `patch`.
   - route fields remain stable: `machine_code`, `verdict`, `ok_to_apply`, and `summary`.

### Acceptance Criteria

- `line_range` semantics are unambiguous.
- The handler, schema/tool card, architecture docs, and tests agree.
- No path inspects `original` as replacement text unless explicitly documented.

## Workstream B: Make Command Platform Support Explicit

### Problem

`command_preflight` accepts `platform = windows`, but immediately returns `UNSUPPORTED_FEATURE`. This is acceptable only if schema/docs/tool cards clearly describe `windows` as recognized-but-unsupported.

### Preferred Action for This Pass

Do not implement Windows parsing yet. Instead, make the contract explicit:

- `posix`: fully supported.
- `auto`: currently resolves to POSIX behavior.
- `windows`: recognized input value, returns a structured `UNSUPPORTED_FEATURE` response.

This keeps the public schema stable while preventing accidental claims of Windows support.

### Required Work

1. Update command preflight schema descriptions:

   - `windows` is recognized but unsupported.
   - `auto` currently uses POSIX parsing.

2. Update generated tool cards and architecture docs.

3. Add tests:

   - `platform = windows` returns non-OK response with `UNSUPPORTED_FEATURE`.
   - `platform = auto` uses POSIX path and succeeds for a simple command.
   - docs/tool-card generated output contains the unsupported Windows caveat.

4. Add a future-plan note for Windows support:

   - `cmd.exe /c` chaining.
   - PowerShell invocation.
   - encoded PowerShell command flags.
   - redirection and command chaining.

### Optional Alternative

Implement minimal Windows parsing now. Only do this if it can be tested thoroughly. Minimal support must detect:

- `cmd.exe /c`.
- PowerShell and `pwsh`.
- `-EncodedCommand`.
- `&`, `&&`, `||` chaining.
- `>`/`>>` redirection.

### Acceptance Criteria

- Schema, docs, generated cards, and implementation agree on platform support.
- No user or codegg harness can mistake `windows` for supported execution-policy analysis.

## Workstream C: Replace Heuristic Config Extraction With Parsed Traversal

### Problem

`config_file_inspect` now validates JSON/TOML parsing with real parsers, but key extraction still uses line scanning for JSON/package.json/TOML/Cargo/pyproject/YAML-like formats. That can miss nested values, misread syntax, and produce false negatives/positives.

### Required Work

1. Add parsed traversal for JSON/package.json.

   Use `serde_json::Value` and recursively walk object keys.

   Emit key paths as JSON pointers or dotted paths:

   - `/scripts/postinstall`
   - `/dependencies/serde`
   - `/debug`

   For config-risk heuristics, inspect:

   - object keys.
   - string values.
   - boolean values.
   - arrays of strings.

2. Add parsed traversal for TOML/Cargo/pyproject.

   Use `toml::Value` or `toml_edit::DocumentMut`.

   Emit dotted paths:

   - `package.build`
   - `dependencies.serde.version`
   - `project.dependencies`
   - `tool.ruff.line-length`

3. Keep dotenv and INI line scanners.

   They are appropriate enough for these formats.

4. Handle YAML honestly.

   Unless a YAML parser dependency is intentionally added, keep YAML as heuristic-only and document it. Prefer a `CONFIG_HEURISTIC_ONLY` finding or a `limits_applied` note for YAML mode.

5. Preserve masking of secret-like values.

   Do not emit full secret-like values in results or findings.

6. Add tests:

   - nested JSON secret-like key.
   - package.json scripts detection through parsed traversal.
   - malformed JSON returns `CONFIG_PARSE_FAILED`.
   - TOML nested secret/debug/TLS keys.
   - Cargo package build key.
   - pyproject tool config keys.
   - YAML heuristic-only behavior is documented and tested.

### Acceptance Criteria

- JSON and TOML semantic extraction is parser-backed.
- Existing heuristic coverage is retained where useful.
- Findings include stable key paths.
- Secret-like values remain masked.

## Workstream D: Harden Dependency Metadata Parsing

### Problem

`dependency_edit_preflight` is now safer about unknown ecosystems, but dependency extraction is still primarily line-oriented. It needs parsed traversal for common metadata files.

### Required Work

1. Cargo.toml parsing.

   Use `toml` or `toml_edit` to parse dependency sections:

   - `[dependencies]`
   - `[dev-dependencies]`
   - `[build-dependencies]`
   - `[target.'cfg(...)'.dependencies]`
   - `[workspace.dependencies]`
   - `[patch.*]`
   - `[replace]`
   - `[package] build = ...`

   Detect dependency forms:

   - simple string version.
   - inline table `{ version = "...", features = [...] }`.
   - git source.
   - path source.
   - registry source.
   - workspace dependency inheritance.
   - wildcard or broad version.

2. pyproject.toml parsing.

   Use TOML parser to inspect:

   - `[project] dependencies` arrays.
   - `[project.optional-dependencies]`.
   - `[build-system] requires` and `build-backend`.
   - tool-specific dependency sections if simple and safe.

3. package.json parsing.

   Use `serde_json` to inspect:

   - dependencies.
   - devDependencies.
   - peerDependencies.
   - optionalDependencies.
   - scripts.
   - packageManager.
   - direct URL/tarball/git specifiers.

4. requirements.txt parsing.

   Keep line parser but improve detection of:

   - direct URLs.
   - editable installs `-e`.
   - local paths.
   - unconstrained specs.
   - constraints/includes flags should be surfaced as review, not silently skipped.

5. Result shape.

   Preserve existing output fields but add structured detail:

   ```json
   {
     "dependency_changes": {
       "added": [{"name": "serde", "section": "dependencies", "source": "registry", "version": "1"}],
       "removed": [],
       "version_changed": [],
       "source_changed": []
     },
     "metadata_changes": {
       "scripts": [],
       "build_backend": [],
       "package_manager": []
     }
   }
   ```

   If backward compatibility requires arrays of strings, add detailed fields alongside existing fields rather than replacing them abruptly.

6. Add tests:

   - Cargo inline dependency addition.
   - Cargo git dependency addition.
   - Cargo path dependency addition.
   - Cargo target-specific dependency addition.
   - Cargo workspace dependency addition.
   - Cargo build script addition.
   - Cargo patch section addition.
   - pyproject build backend change.
   - pyproject optional dependency addition.
   - package.json postinstall addition.
   - package.json git/tarball dependency.
   - requirements direct URL/editable/local path.

### Acceptance Criteria

- Cargo, pyproject, and package.json dependency parsing is parser-backed.
- requirements.txt parser is explicit about heuristic limitations.
- New risky metadata routes to review/block according to policy.
- Existing generated docs and typed wrappers remain consistent.

## Workstream E: Cooperative Budget, Deadline, and Cancellation Checks

### Problem

Budgets now enforce serialized input size and output shape, but deadlines/cancellation remain mostly structural. Long-running loops and composites do not consistently check a `BudgetContext` during execution.

### Required Work

1. Define a minimal internal call context.

   Suggested shape:

   ```rust
   pub struct ToolCallContext {
       pub budget: ToolBudget,
       pub deadline: Option<Instant>,
       pub cancelled: Option<Arc<AtomicBool>>,
   }
   ```

   If `BudgetContext` already fits, reuse it rather than adding a second type.

2. Add helper methods:

   - `check_not_cancelled()`.
   - `check_deadline()`.
   - `check_text_len(name, text)`.
   - `check_list_len(name, len)`.
   - `remaining_time_ms()`.

3. Thread context through high-risk handlers first.

   Priority handlers:

   - `edit_preflight`.
   - `command_preflight`.
   - `config_preflight`.
   - `config_file_inspect`.
   - `dependency_edit_preflight`.
   - regex/sample-heavy helpers.
   - text security inspection.

4. Composite sub-budgets.

   Apply `CompositeBudgetAllocator` in at least:

   - `edit_preflight`.
   - `command_preflight`.
   - `config_file_inspect`.
   - `dependency_edit_preflight`.

   If handler signatures cannot be changed without churn, implement internal helper functions that accept context and have public handlers call them with default context.

5. MCP cancellation.

   Current MCP cancellation checks before execution. Add documentation and tests for:

   - cancelled before dispatch.
   - timeout during execution.
   - running blocking work may continue after timeout.

   Do not claim hard cancellation of running blocking work unless actually implemented.

6. Tests:

   - budget context expires and route-critical handler returns `TIMEOUT`/budget error from a controlled test helper.
   - cancelled context returns `CANCELLED` before sub-tool dispatch.
   - composite allocator reduces sub-tool max text/output/findings caps.
   - old `call_json` remains compatibility path.

### Acceptance Criteria

- Budget/deadline/cancellation is no longer only descriptive for high-risk handlers.
- Public compatibility APIs remain usable.
- Timeout/cancellation limitations are documented honestly.
- Tests cover cooperative stop checks.

## Workstream F: Generated Docs and Tool Card Closure

### Required Work

1. Update generator metadata for:

   - line-range mode contract.
   - Windows platform caveat.
   - JSON/TOML parser-backed inspection.
   - dependency parser limitations and supported formats.
   - budget behavior and truncation semantics.

2. Regenerate:

   - README generated block.
   - `architecture/mcp-server.md` generated profile block.
   - `generated/tool-cards.md`.
   - `.skills/mcp-tools.md` if generated/maintained by the workflow.

3. Add or update generator tests if cards now include route-critical caveats.

### Acceptance Criteria

- Generated docs match implementation behavior.
- No stale docs after `cargo run --bin generate-docs -- --check`.
- Tool cards do not overclaim Windows support or parser precision.

## Workstream G: Final Verification

Run the full gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If any command cannot run in the implementation environment, document exactly which command was skipped and why.

## Recommended Implementation Order

1. Decide and lock `line_range` contract.
2. Document or remove Windows support claims in `command_preflight` schema/cards.
3. Add parsed traversal for config inspection JSON/TOML paths.
4. Add parser-backed dependency inspection for Cargo, pyproject, and package.json.
5. Add cooperative budget/deadline/cancellation helper path for high-risk composites.
6. Regenerate docs/tool cards.
7. Run full verification.

## Final Acceptance Criteria

This follow-up pass is complete when:

- `line_range` edit preflight has a single, documented, tested contract.
- `command_preflight` platform support is explicit and not misleading.
- `config_file_inspect` uses parsed traversal for JSON/TOML-family formats.
- `dependency_edit_preflight` uses parser-backed metadata extraction for Cargo, pyproject, and package.json.
- Budgets are checked cooperatively in high-risk composite handlers.
- Generated docs/tool cards are current.
- Full verification passes or failures are documented with exact output.

## Non-Goals

Do not add network vulnerability lookups.

Do not add package registry fetching.

Do not implement full filesystem sandboxing.

Do not redesign MCP transport concurrency.

Do not begin phase 10 evaluator context isolation in this pass.
