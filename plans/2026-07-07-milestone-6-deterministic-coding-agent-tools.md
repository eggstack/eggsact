# Milestone 6: Deterministic Coding-Agent Tool Additions

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Add a focused set of deterministic coding-agent tools that fit the existing `eggsact` model: local execution, no network access, no command execution, bounded inputs, machine-readable outputs, explicit profiles, and clear heuristic limits.

The goal is to give codegg-style agents cheap deterministic primitives for patch review, verification planning, repository language detection, lightweight import/export inspection, code block mapping, symbol-level diff summaries, and lockfile review. These tools should reduce avoidable model guesswork without turning `eggsact` into a full parser, LSP, dependency resolver, or vulnerability scanner.

## Tool list

Implement the following tools, in this recommended order:

1. `patch_contract_check`
2. `test_command_suggest`
3. `repo_language_detect`
4. `import_export_inspect`
5. `code_block_map`
6. `symbol_name_diff`
7. `lockfile_inspect`

The order matters. `patch_contract_check` builds on existing patch/diff work and route-critical contracts. `repo_language_detect` can support `test_command_suggest`, but `test_command_suggest` can ship first with direct manifest detection. `symbol_name_diff` should reuse `code_block_map` once available.

## Global requirements for every tool

Every tool must include:

- Handler implementation under `src/tools/`.
- Input schema under `src/mcp/schemas/`.
- `ToolSpec` entry under the appropriate `src/mcp/specs/` category file.
- Output schema where the registry pattern supports it.
- Generated docs via `cargo run --bin generate-docs`.
- Unit or integration tests through the in-process API.
- MCP tests where the tool is exposed through MCP profiles.
- Profile assignments.
- Audience/exposure assignment.
- Cost assignment.
- Stability assignment.
- Bounded input checks through `BudgetContext` or existing validation helpers.
- Clear machine codes for non-OK responses.

Every tool must avoid:

- Network access.
- Running commands.
- Reading arbitrary local files unless the schema explicitly accepts file content or repo tree text supplied by the caller.
- Claiming full semantic accuracy.
- Adding large third-party parser dependencies unless separately justified.

Every heuristic tool should return:

- `confidence` or per-finding confidence.
- `limitations` or `notes` when useful.
- Source line ranges where available.
- Stable enum-like finding codes.

## Tool 1: `patch_contract_check`

### Purpose

Classify a unified diff or patch by contract-relevant change categories. This is a deterministic patch risk summary for coding-agent harnesses, not a semantic code review.

### Inputs

Suggested input fields:

- `patch` or `diff`: required string.
- `repo_root`: optional string for path-context reporting only.
- `allowed_paths`: optional array of path prefixes.
- `deny_paths`: optional array of path prefixes.
- `language_hint`: optional string.
- `risk_profile`: optional enum: `default`, `strict`, `release`.
- `max_files`: optional integer.
- `max_hunks`: optional integer.

### Output

Suggested output fields:

- `verdict`: `allow`, `review`, or `block`.
- `machine_code`.
- `files`: array of changed-file summaries.
- `summary`: compact human-readable summary.
- `findings`: array with code, severity, disposition, path, line/hunk where applicable.
- `counts`: files added/modified/deleted/renamed, hunks, additions, deletions.
- `touches_tests`: boolean.
- `touches_docs`: boolean.
- `touches_lockfiles`: boolean.
- `touches_manifests`: boolean.
- `touches_generated`: boolean.
- `touches_public_api_like_paths`: boolean.
- `large_delete`: boolean.
- `path_scope_violations`: array.
- `recommended_next_tool`: optional object.

### Finding categories

At minimum detect:

- Path traversal or scope escape.
- Manifest changes: `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, etc.
- Lockfile changes: `Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `uv.lock`, `poetry.lock`, `go.sum`.
- Test changes or absence of tests in risky patch.
- Generated file changes.
- Large deletion.
- Binary/minified-looking file changes.
- CI/workflow changes.
- Security-sensitive file changes: auth, crypto, permissions, shell scripts, deployment config.
- Public API-looking file changes: `src/lib.rs`, exported module files, `__init__.py`, package entrypoints, type declarations.
- Patch parse failures.

### Route-critical status

Start as `HarnessOnly` or model-visible review helper depending existing profile design. Do not immediately classify as route-critical unless the route contract is exact and stable. If it becomes route-critical, add it to `ROUTE_CRITICAL_TOOLS` and add fixture coverage in the same commit.

### Tests

Required fixtures:

- Small safe source-only patch.
- Patch touching source and tests.
- Patch touching source without tests.
- Patch touching manifest.
- Patch touching lockfile.
- Patch touching CI workflow.
- Patch deleting many lines.
- Patch with malformed hunk.
- Patch touching generated/minified file.
- Patch outside allowed path.

## Tool 2: `test_command_suggest`

### Purpose

Given supplied repo tree and manifest/config snippets, suggest likely verification commands without executing anything.

### Inputs

Suggested input fields:

- `repo_tree`: required or optional string/array of paths.
- `manifest_files`: optional object mapping path to content.
- `language_hint`: optional string.
- `risk_policy`: optional enum: `default`, `strict`, `minimal`.
- `include_lints`: optional boolean.
- `include_format`: optional boolean.
- `max_commands`: optional integer.

### Output

Suggested output fields:

- `commands`: array of command suggestions.
- Each command should include:
  - `command`
  - `ecosystem`
  - `purpose`: `test`, `lint`, `format_check`, `typecheck`, `build`, `package_check`
  - `confidence`
  - `rationale`
  - `risk`: `low`, `medium`, `high`
  - `requires_network`: boolean or `unknown`
  - `should_preflight`: boolean
- `detected_ecosystems`.
- `warnings`.
- `limitations`.

### Ecosystem heuristics

Support at least:

Rust:

- `Cargo.toml` -> `cargo test --all-features`.
- Rust library/bin -> `cargo check --all-targets --all-features`.
- Lint option -> `cargo clippy --all-targets --all-features -- -D warnings`.
- Format option -> `cargo fmt --all -- --check`.

Python:

- `pyproject.toml`, `pytest.ini`, `tests/` -> `python -m pytest`.
- Ruff config -> `python -m ruff check .`.
- Mypy config -> `python -m mypy .`.

Node:

- `package.json` scripts -> suggest script commands found in scripts.
- Prefer `npm test`, `npm run lint`, `npm run typecheck`, `npm run build` only when scripts exist.
- Mark script commands as review-worthy because scripts execute project-defined commands.

Go:

- `go.mod` -> `go test ./...`.
- Optional format/lint suggestions should be conservative.

Generic:

- If no known ecosystem detected, return low-confidence suggestions only if there is clear evidence.

### Tests

Required fixtures:

- Rust crate with `Cargo.toml`.
- Python project with `pyproject.toml` and `pytest.ini`.
- Node project with `package.json` scripts.
- Go module with `go.mod`.
- Polyglot repo.
- Unknown repo.
- Malformed manifest content.

## Tool 3: `repo_language_detect`

### Purpose

Detect likely languages, ecosystems, package managers, test frameworks, and recommended `eggsact` profiles from a supplied repo tree and optional manifest contents.

### Inputs

- `repo_tree`: required string or path array.
- `manifest_files`: optional object path -> content.
- `max_paths`: optional integer.
- `include_profile_recommendations`: optional boolean.

### Output

- `languages`: array with language, score, evidence.
- `ecosystems`: array with ecosystem, score, manifests, package managers.
- `test_frameworks`: array with name, score, evidence.
- `build_systems`: array.
- `recommended_profiles`: array of profile names and rationale.
- `warnings`.

### Detection signals

Use deterministic scoring from:

- File extensions.
- Manifest filenames.
- Lockfiles.
- Config files.
- Test directory names.
- Script names from supplied manifests.

Do not inspect filesystem directly. Only use caller-supplied tree/content.

### Tests

Include single-language, polyglot, docs-only, vendored-heavy, and malformed-tree fixtures.

## Tool 4: `import_export_inspect`

### Purpose

Extract import/export/module-use statements from source text using lightweight language-aware heuristics.

### Inputs

- `source`: required string.
- `language`: optional enum: `rust`, `python`, `javascript`, `typescript`, `go`, `auto`.
- `include_line_text`: optional boolean.
- `max_statements`: optional integer.

### Output

- `language`.
- `statements`: array with kind, module, symbols, alias, line, column, raw_text optional, confidence.
- `warnings`.
- `limitations`.

### Initial language support

Rust:

- `use` statements.
- `mod` declarations.
- `pub use`.
- `extern crate` if present.

Python:

- `import x`.
- `import x as y`.
- `from x import y`.
- Relative imports.
- Star imports flagged.

JavaScript/TypeScript:

- `import ... from`.
- `import "side-effect"`.
- `export ...`.
- `require(...)` where simple.

Go:

- Single import.
- Import block.
- Aliased imports.
- Blank imports flagged.

### Tests

Add fixtures for normal imports, aliases, multiline statements, star/blank imports, comments, and malformed partial source.

## Tool 5: `code_block_map`

### Purpose

Return approximate top-level block ranges from source text or Markdown. This helps agents target edits without full parsing.

### Inputs

- `source`: required string.
- `language`: optional enum: `rust`, `python`, `javascript`, `typescript`, `go`, `markdown`, `auto`.
- `max_blocks`: optional integer.
- `include_nested`: optional boolean.

### Output

- `blocks`: array with kind, name, start_line, end_line, confidence, raw_signature optional.
- `warnings`.
- `limitations`.

### Heuristics

Brace languages:

- Detect function/type/module/class-like signatures.
- Track brace depth.
- Return top-level or nested blocks depending input.

Python:

- Detect `def`, `async def`, `class`.
- Track indentation.

Markdown:

- Detect headings.
- Detect fenced code blocks.

### Tests

Include Rust modules/functions, Python classes/defs, JS/TS functions/classes, Go funcs/types, Markdown headings/fences, malformed braces, and empty files.

## Tool 6: `symbol_name_diff`

### Purpose

Compare old/new source text and report added, removed, and possibly renamed top-level symbols using `code_block_map` heuristics.

### Inputs

- `old_source`: required string.
- `new_source`: required string.
- `language`: optional enum.
- `rename_similarity_threshold`: optional number.

### Output

- `added`: array of symbols.
- `removed`: array of symbols.
- `unchanged`: array if useful.
- `possible_renames`: array with old, new, confidence.
- `warnings`.
- `limitations`.

### Tests

Include added function, removed function, renamed function, reordered functions, malformed source, and language-auto cases.

## Tool 7: `lockfile_inspect`

### Purpose

Inspect lockfile content or lockfile diffs for deterministic dependency-change signals. This is not a vulnerability scanner and should not query registries.

### Inputs

- `path`: optional lockfile path.
- `before`: optional string.
- `after`: optional string.
- `diff`: optional string.
- `ecosystem`: optional enum: `cargo`, `npm`, `pnpm`, `yarn`, `uv`, `poetry`, `go`, `auto`.
- `max_packages`: optional integer.

### Output

- `ecosystem`.
- `changes`: array of added/removed/updated packages where detectable.
- `source_changes`: array.
- `git_or_path_dependencies`: array.
- `large_churn`: boolean.
- `summary`.
- `findings`.
- `limitations`.

### Ecosystem scope

Initial support should be conservative:

- Cargo.lock: package name/version/source/checksum sections.
- package-lock.json: JSON parse and dependencies/packages changes.
- pnpm-lock.yaml/yarn.lock: heuristic line-based detection if no YAML parser is present.
- uv.lock/poetry.lock: TOML-ish package sections where feasible.
- go.sum: line-based module/version additions/removals.

### Tests

Include small changes for each ecosystem, large churn, malformed lockfile, git/path dependency, and source URL change.

## Profile placement

Initial suggested profile assignments:

- `patch_contract_check`: `codegg_patch`, `codegg_preflight`, `codegg_repo_audit`.
- `test_command_suggest`: `codegg_core`, `codegg_repo_audit`, possibly `codegg_shell`.
- `repo_language_detect`: `codegg_core`, `codegg_repo_audit`.
- `import_export_inspect`: `codegg_core`, `codegg_repo_audit`.
- `code_block_map`: `codegg_core`, `codegg_repo_audit`.
- `symbol_name_diff`: `codegg_patch`, `codegg_repo_audit`.
- `lockfile_inspect`: `codegg_repo_audit`, `codegg_patch`, possibly `codegg_preflight`.

Revisit placement during milestone 7.

## Machine-code guidance

Prefer existing generic codes where suitable. Add new codes only when downstream routing benefits from stable specificity.

Likely new codes:

- `PATCH_CONTRACT_REVIEW`
- `PATCH_CONTRACT_BLOCK`
- `PATCH_LOCKFILE_CHANGE`
- `PATCH_MANIFEST_CHANGE`
- `PATCH_SCOPE_ESCAPE`
- `PATCH_LARGE_DELETE`
- `REPO_LANGUAGE_DETECTED`
- `TEST_COMMANDS_SUGGESTED`
- `SOURCE_INSPECT_HEURISTIC`
- `LOCKFILE_CHANGE_DETECTED`
- `LOCKFILE_PARSE_ERROR`

Every new code must be added to the canonical machine-code table and tested if emitted by route-critical or near-route-critical tools.

## Testing matrix

Run after each tool:

```bash
cargo fmt --all -- --check
cargo test --all-features --lib
cargo test --all-features --tests -- --skip parity <tool_or_module_filter>
cargo run --bin generate-docs -- --check
```

Run before milestone close:

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

- All seven tools are implemented or explicitly deferred with rationale.
- Implemented tools have registry specs, schemas, docs, tests, budgets, and profile placement.
- No implemented tool executes commands or performs network access.
- Heuristic tools expose confidence/limitations.
- Generated docs are current.
- CI-equivalent test matrix passes.

## Handoff notes

Implement this milestone incrementally. Do not batch all seven tools into one large commit. Each tool should be reviewable independently with its own tests and generated-doc updates. If the milestone is split, ship `patch_contract_check`, `test_command_suggest`, and `repo_language_detect` first; they provide the most immediate codegg value.
