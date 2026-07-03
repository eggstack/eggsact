# Phase 8: Repo, Config, and Dependency Inspectors

## Goal

Expand eggsact from single-input preflight helpers into a stronger deterministic inspection layer for repository configuration and dependency metadata. codegg should be able to ask eggsact for fast, structured, non-model judgments about common project files before making edits, running commands, or proposing dependency changes.

Phase 8 should produce route-friendly inspectors that are cheap, bounded, schema-driven, and useful to harnesses and coding agents. The work should stay deterministic and local: no network package lookups, no vulnerability database queries, no external registry fetches, and no execution of project code.

## Current State

The repo already has useful building blocks:

- `config_preflight` validates JSON, TOML, dotenv, INI, and Cargo.toml-style config.
- `cargo_toml_inspect` exists for Cargo metadata inspection.
- `dotenv_validate`, `ini_validate`, `toml_shape`, `validate_json`, `validate_toml`, and `validate_schema_light` provide category-specific checks.
- Path tools can normalize, analyze, compare, and scope paths.
- Route-critical response contracts exist for `config_preflight` and related preflight tools.
- Generated docs and tool cards are registry-derived.

The gap is that codegg still lacks a coherent repo/config/dependency inspection surface. It needs fast answers such as: what kind of project is this, which config files are present, whether a proposed dependency edit is structurally suspicious, whether package metadata is malformed, and whether config changes require review.

## Scope

Phase 8 covers deterministic inspection of repository metadata and common configuration/dependency files. It does not cover network vulnerability scanning, package resolution, lockfile solving, build execution, or semantic source-code analysis.

## Workstream A: Define Inspector Surfaces

### Required Tools

Add or refine these inspector surfaces. Prefer composing existing tools before adding new primitives.

1. `repo_manifest_inspect`

   Purpose: classify project manifests and summarize repository shape from a bounded list of paths and optional file snippets.

   Inputs:

   - `paths`: list of relative paths.
   - optional `file_summaries`: map from path to small text sample or complete text for known manifests.
   - `workspace_root`: optional root for path-scope context.
   - `max_paths`: bounded.

   Outputs:

   - detected project types: Rust, Python, Node, Go, mixed, unknown.
   - manifest paths.
   - config paths.
   - lockfile paths.
   - test/build tool hints.
   - findings.
   - verdict: allow/review/block or inspect-only domain if not route-critical.

2. `dependency_edit_preflight`

   Purpose: inspect proposed dependency file changes before codegg applies them.

   Inputs:

   - `file_path`.
   - `old_text`.
   - `new_text`.
   - optional `ecosystem`: rust, python, node, go, auto.
   - optional `policy`.

   Outputs:

   - dependency additions/removals/changes.
   - version constraint changes.
   - source changes: registry/path/git/url.
   - script/hook changes if applicable.
   - findings.
   - verdict and machine code.

3. `config_file_inspect`

   Purpose: inspect a single config file beyond syntax validity.

   Inputs:

   - `file_path`.
   - `text`.
   - `format`: auto/json/toml/yaml-like/dotenv/ini/cargo_toml/package_json/pyproject.
   - optional `policy`.

   Outputs:

   - syntax status.
   - shape summary.
   - risky keys.
   - environment/secrets risk hints.
   - executable hook fields.
   - findings.
   - verdict and machine code where route-critical.

4. Optional: `repo_policy_suggest`

   Purpose: convert detected repo type into command/config policy hints for phases 7 and 8.

   This should be added only if it stays deterministic and small.

### Acceptance Criteria

- Tool surfaces are bounded and schema-driven.
- Inputs are text/path metadata supplied by the harness, not filesystem scans unless explicitly designed and bounded.
- Outputs use structured findings and stable machine codes.

## Workstream B: Rust Cargo Inspection

### Problem

Cargo.toml changes are high-value for codegg. Dependency additions, feature flags, path/git deps, build scripts, and workspace settings should be visible to the harness before edits land.

### Required Work

1. Extend `cargo_toml_inspect` or compose it into `dependency_edit_preflight`.

2. Detect and report:

   - package name/version/edition.
   - workspace members/excludes.
   - dependency sections: dependencies, dev-dependencies, build-dependencies, target-specific dependencies.
   - version constraint changes.
   - new git dependencies.
   - new path dependencies.
   - wildcard versions.
   - broad feature activation.
   - build script presence or change.
   - patch/replace sections.

3. Classify changes:

   - allow: syntax-only clean changes, version patch bump if policy permits.
   - review: new dependency, widened version, new feature, target-specific dependency.
   - block or high review: new git/path dependency, build script introduction, patch/replace override, suspicious package rename.

4. Add machine codes:

   - `DEPENDENCY_OK`
   - `DEPENDENCY_ADDED`
   - `DEPENDENCY_REMOVED`
   - `DEPENDENCY_VERSION_WIDENED`
   - `DEPENDENCY_GIT_SOURCE`
   - `DEPENDENCY_PATH_SOURCE`
   - `DEPENDENCY_BUILD_SCRIPT`
   - `DEPENDENCY_PATCH_OVERRIDE`
   - `CONFIG_PARSE_FAILED`

5. Add tests with representative Cargo.toml fixtures.

### Acceptance Criteria

- Cargo.toml edit preflight produces stable dependency diffs.
- New git/path/build-script changes produce review/block findings.
- Syntax failures are caught before semantic diffing.

## Workstream C: Python and Node Metadata Inspection

### Required Python Files

- `pyproject.toml`
- `requirements.txt`
- `setup.cfg`
- `setup.py` only as text heuristics; do not execute.

Detect:

- dependency additions/removals.
- extras changes.
- build backend changes.
- script/entry-point changes.
- direct URL dependencies.
- editable/path dependencies.
- unconstrained or wildcard specs.

### Required Node Files

- `package.json`
- `package-lock.json` summary only.
- `pnpm-lock.yaml` summary only if lightweight parsing is feasible.

Detect:

- dependency additions/removals.
- devDependency movement.
- script changes, especially install/postinstall/preinstall/prepare.
- package manager changes.
- registry or tarball URL dependencies.
- wildcard versions.

### YAML Constraint

If YAML support is needed, do not introduce a large dependency casually. Either:

- keep lockfile support to heuristic line scanning, or
- explicitly add a lightweight dependency with a separate rationale.

### Acceptance Criteria

- Python/Node metadata inspection is useful without executing code.
- Script/hook changes route to review/block.
- Direct URL/path/editable dependencies are flagged.

## Workstream D: Config Risk Heuristics

### Problem

Syntax validity is not enough. Config edits can introduce secret exposure, command hooks, insecure endpoints, dangerous flags, or production-impacting settings.

### Required Work

1. Define risky-key patterns.

   Examples:

   - keys containing `secret`, `token`, `password`, `private_key`, `api_key`.
   - URLs using `http://` where `https://` expected.
   - debug flags enabled.
   - TLS verification disabled.
   - shell command hooks.
   - wildcard hosts or permissive CORS.

2. Keep heuristics explainable.

   Each finding must include:

   - key/path.
   - reason.
   - severity.
   - disposition.
   - machine code category.

3. Avoid false precision.

   Heuristic detections should generally route to review, not block, unless the input is structurally invalid or explicitly dangerous.

4. Add format-specific path reporting.

   - JSON pointer for JSON.
   - dotted key path for TOML.
   - key name for dotenv/INI.

### Acceptance Criteria

- Config risk findings are actionable.
- Heuristic findings do not silently block common benign configs.
- Tests cover secret-like keys, insecure URLs, debug flags, and command hooks.

## Workstream E: Repo Manifest Classification

### Required Work

1. Implement project type detection from path lists.

   Examples:

   - Rust: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`.
   - Python: `pyproject.toml`, `requirements.txt`, `setup.cfg`.
   - Node: `package.json`, lockfiles.
   - Go: `go.mod`, `go.sum`.

2. Emit tool hints for phase 7 command policy.

   Examples:

   - Rust: allow candidates `cargo check`, `cargo test`, `cargo fmt --check`.
   - Python: `python -m pytest`, `ruff check`, `mypy` if files imply them.
   - Node: `npm test`, `npm run lint` only as review unless scripts are inspected.

3. Emit config/dependency inspector hints.

   Example: if `Cargo.toml` is present, suggest `cargo_toml_inspect` or `dependency_edit_preflight` for edits touching it.

4. Keep this inspection path-only unless text snippets are explicitly passed.

### Acceptance Criteria

- Repo classification is deterministic and cheap.
- It does not read the filesystem unless a bounded filesystem mode is explicitly added later.
- Hints are conservative.

## Workstream F: Registry, Profiles, and Tool Cards

### Required Work

1. Add new tools to appropriate profiles.

   Recommended:

   - `repo_manifest_inspect`: `codegg_repo_audit`, possibly `codegg_core` if compact.
   - `dependency_edit_preflight`: `codegg_config`, `codegg_repo_audit`, harness-oriented profiles.
   - `config_file_inspect`: `codegg_config`, `codegg_repo_audit`.

2. Decide exposure.

   - `repo_manifest_inspect`: contextual or expert-only.
   - `dependency_edit_preflight`: harness-only or contextual depending on intended model visibility.
   - `config_file_inspect`: contextual.

3. Add schemas and output schemas.

4. Regenerate docs/tool cards.

5. Add route-critical classification only for tools that directly produce harness routing decisions.

### Acceptance Criteria

- Profiles expose tools intentionally.
- Model-facing profiles do not get heavy harness-only tools accidentally.
- Generated docs reflect new tools.

## Workstream G: Tests and Fixtures

### Required Fixtures

- Cargo.toml clean package.
- Cargo.toml new crates.io dependency.
- Cargo.toml new git dependency.
- Cargo.toml new build script.
- pyproject dependency addition.
- requirements direct URL.
- package.json script change.
- package.json postinstall addition.
- dotenv secret-like key.
- JSON debug flag.
- TOML insecure endpoint.
- path-list-only mixed repo manifest.

### Test Types

- unit tests for parsers/diff helpers.
- tool handler tests for each inspector.
- route-contract tests for route-critical inspectors.
- schema validation tests for invalid inputs.
- generated-doc check.

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Final Acceptance Criteria

Phase 8 is complete when:

- codegg has deterministic repo/config/dependency inspection tools.
- Cargo.toml dependency edit risks are detected and tested.
- Python and Node metadata risks are detected without executing code.
- Config risk heuristics produce structured findings.
- Repo manifest classification emits conservative project/tool hints.
- Profiles, schemas, docs, and tool cards are updated.
- Full CI passes.
