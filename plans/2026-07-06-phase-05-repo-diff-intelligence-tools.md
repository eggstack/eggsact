# Phase 05: repo and diff intelligence tools

## Purpose

Add the first batch of new deterministic agent-utility tools: `repo_tree_summarize`, `diff_risk_classify`, and `path_batch_scope_check`. These tools should reduce model-side parsing, improve codegg routing, and provide bounded structured evidence for repo review and edit preflight workflows.

## Design constraints

All tools in this phase must be local-only and file-system-free. They should operate on in-memory inputs supplied by the caller: path lists, optional file sizes/statuses, workspace root strings, and unified diff text. They must not walk the real filesystem, call git, query package registries, or execute commands.

All tools must be bounded by existing budget conventions: maximum input bytes, maximum path count, maximum diff length, maximum findings, and cooperative cancellation where loops may be non-trivial.

All tools must use stable machine codes and verdicts where they are route-critical or likely to influence codegg routing. They should emit concise summaries and structured details, not prose blobs.

## Tool 1: `repo_tree_summarize`

### Goal

Summarize repository shape from a bounded path list and optional metadata. This complements `repo_manifest_inspect`, which detects manifest/project types from paths, by adding path bucketing, entrypoint/config/test/source/generated/vendor classification, and recommended next tools.

### Proposed input schema

```json
{
  "type": "object",
  "required": ["paths"],
  "properties": {
    "paths": {
      "type": "array",
      "items": {"type": "string"},
      "description": "Repository-relative paths."
    },
    "file_sizes": {
      "type": "object",
      "additionalProperties": {"type": "integer"},
      "description": "Optional byte sizes keyed by path."
    },
    "statuses": {
      "type": "object",
      "additionalProperties": {"type": "string"},
      "description": "Optional git-like status keyed by path."
    },
    "max_paths": {"type": "integer", "default": 1000},
    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
  }
}
```

### Proposed output fields

- `project_types`: reuse or align with `repo_manifest_inspect` project type labels.
- `path_count`, `directory_count`, `file_count` if derivable from path endings.
- `buckets`: `source`, `tests`, `docs`, `configs`, `manifests`, `lockfiles`, `ci`, `scripts`, `generated`, `vendor`, `assets`, `unknown`.
- `entrypoint_candidates`: likely entry files such as `src/main.rs`, `src/lib.rs`, `main.py`, `package.json` scripts, `go.mod` plus `cmd/` hints.
- `high_leverage_paths`: CI, dependency manifests, security/auth config, build scripts, lockfiles.
- `tool_hints`: recommended eggsact tools such as `cargo_toml_inspect`, `config_file_inspect`, `dependency_edit_preflight`, `markdown_structure`, `code_fence_extract`.
- `findings`: unknown repo, mixed repo, unusually many generated/vendor paths, missing lockfile hints, manifest-without-source hints.
- `verdict`: usually `allow` or `review`.
- `machine_code`: e.g. `REPO_TREE_SUMMARY_OK`, `REPO_TREE_REVIEW`, `INPUT_TOO_LARGE`.

### Implementation notes

Build path classifier helpers that can later be reused by `diff_risk_classify`. Keep classifiers intentionally heuristic and transparent. Do not attempt full language detection beyond path and manifest conventions. Treat hidden directories, vendored directories, generated directories, and lockfiles explicitly.

### Tests

- Rust crate path list.
- Mixed Rust/Python/Node repo.
- Docs-only repo.
- Repo with generated/vendor-heavy tree.
- Too many paths.
- Non-string path entries should either be schema-rejected or produce structured invalid arguments depending on dispatch path.
- Unicode paths and Windows-style separators.

## Tool 2: `diff_risk_classify`

### Goal

Classify unified diffs by review risk and routing category. This builds on `patch_summary`, which already reports file counts, hunk counts, additions, deletions, renames, binary patch detection, and line ranges. `diff_risk_classify` should add semantic path/change risk categories useful to reviewer agents.

### Proposed input schema

```json
{
  "type": "object",
  "required": ["patch_text"],
  "properties": {
    "patch_text": {"type": "string"},
    "workspace_root": {"type": "string"},
    "max_patch_chars": {"type": "integer", "default": 200000},
    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"},
    "policy": {
      "type": "object",
      "properties": {
        "review_ci_changes": {"type": "boolean", "default": true},
        "review_dependency_changes": {"type": "boolean", "default": true},
        "review_security_sensitive_paths": {"type": "boolean", "default": true},
        "allow_docs_only": {"type": "boolean", "default": true}
      }
    }
  }
}
```

### Proposed output fields

- `summary`: compact human-readable summary.
- `patch_summary`: selected fields from shared patch parsing or `patch_summary` logic.
- `risk_categories`: array of category strings such as `docs_only`, `tests_only`, `source_change`, `config_change`, `dependency_change`, `lockfile_change`, `ci_change`, `security_sensitive`, `generated_change`, `vendor_change`, `binary_change`, `large_diff`, `rename`.
- `files_by_category`: paths grouped by risk category.
- `review_focus`: small ordered list of path/category reasons for reviewer agents.
- `recommended_next_tool`: e.g. `dependency_edit_preflight`, `config_file_inspect`, `patch_apply_check`, `text_security_inspect`.
- `findings`: canonical findings with severity and disposition.
- `verdict`: `allow`, `review`, or `block`.
- `machine_code`: e.g. `DIFF_RISK_OK`, `DIFF_RISK_REVIEW`, `DIFF_RISK_BLOCK`, `PATCH_FAILED`, `INPUT_TOO_LARGE`.

### Classification heuristics

Flag dependency changes when paths include `Cargo.toml`, `Cargo.lock`, `package.json`, lockfiles, `pyproject.toml`, requirements files, `go.mod`, `go.sum`, or related manifest paths. Flag CI when paths include `.github/workflows/`, `.gitlab-ci.yml`, `Dockerfile`, compose files, Makefiles, or release scripts. Flag security-sensitive paths by names containing auth, token, secret, crypto, tls, permission, policy, sandbox, exec, command, shell, or similar terms. Flag generated/vendor paths by common directories and filename markers.

Docs-only and tests-only should be computed conservatively. If any source/config/dependency/CI path appears, do not call the diff docs-only.

### Tests

- Docs-only diff yields allow or low review.
- Source diff yields review focus.
- Cargo manifest without lockfile yields dependency review finding.
- Lockfile-only diff yields review or informational depending on policy.
- CI workflow diff yields review.
- Security-sensitive path diff yields review/block depending on policy.
- Binary patch produces review/block.
- Malformed diff produces structured error or patch-parse finding.

## Tool 3: `path_batch_scope_check`

### Goal

Check many target paths against a workspace root in one call. This reduces repeated `path_scope_check` calls for patch-level operations and gives consolidated findings for path traversal, escaping paths, absolute paths, duplicate normalized targets, and suspicious path forms.

### Proposed input schema

```json
{
  "type": "object",
  "required": ["root", "targets"],
  "properties": {
    "root": {"type": "string"},
    "targets": {"type": "array", "items": {"type": "string"}},
    "max_targets": {"type": "integer", "default": 1000},
    "allow_absolute": {"type": "boolean", "default": false},
    "case_sensitive": {"type": "boolean", "default": true}
  }
}
```

### Proposed output fields

- `all_inside_root`: boolean.
- `targets_checked`: count.
- `escaping_targets`: paths that resolve outside root.
- `absolute_targets`: paths that were absolute.
- `dotdot_targets`: paths containing traversal segments.
- `normalized_targets`: array or map of original to normalized path.
- `duplicate_normalized_targets`: normalized targets reached by multiple inputs.
- `findings`: canonical findings.
- `verdict`: `allow`, `review`, or `block`.
- `machine_code`: e.g. `PATH_SCOPE_OK`, `PATH_SCOPE_ESCAPE`, `PATH_BATCH_REVIEW`.

### Implementation notes

Reuse existing path normalization/scope logic where possible. Avoid platform-specific surprises by documenting lexical path semantics if the tool does not access the filesystem. Treat symlink resolution as out of scope because the tool is file-system-free.

### Tests

- All relative paths inside root.
- `../` escape paths.
- Absolute paths with allow/disallow policy.
- Duplicate normalized paths.
- Windows-style separators.
- Unicode paths.
- Empty targets.
- Excessive targets.

## Registry/profile integration

Add each tool to `src/mcp/specs/repo.rs` or a new category file if that is cleaner. Suggested categories and profiles:

- `repo_tree_summarize`: category `repo`, profiles `full`, `codegg_repo_audit`, exposure `Contextual`, cost `Moderate`.
- `diff_risk_classify`: category `patch` or `repo`, profiles `full`, `codegg_patch`, `codegg_repo_audit`, exposure `Contextual`, cost `Moderate`, route-critical candidate.
- `path_batch_scope_check`: category `path`, profiles `full`, `codegg_preflight`, `codegg_patch`, exposure `HarnessOnly` or `Contextual` depending on intended model visibility. If it gates harness-applied patches, prefer `HarnessOnly`; if models may inspect path issues directly, use `Contextual` with care.

Add schema builders, output schemas, tool specs, generated docs, and tests. Update profile snapshot tests intentionally.

## Machine-code additions

Add machine codes before handlers so tests can assert stable values. Proposed names:

- `REPO_TREE_OK`
- `REPO_TREE_REVIEW`
- `DIFF_RISK_OK`
- `DIFF_RISK_REVIEW`
- `DIFF_RISK_BLOCK`
- `PATH_BATCH_OK`
- `PATH_BATCH_REVIEW`

Reuse existing `PATH_SCOPE_ESCAPE`, `PATCH_FAILED`, `INPUT_TOO_LARGE`, and `INVALID_ARGUMENTS` where applicable.

## Acceptance criteria

- All three tools have schemas, specs, handlers, tests, and generated docs.
- Tools are bounded, local-only, and file-system-free.
- Profile snapshot tests are updated intentionally.
- `diff_risk_classify` and `path_batch_scope_check` emit stable verdict and machine-code data.
- Shared path classification helpers are reused where practical.
- Full CI verification passes.

## Risks and constraints

Avoid overclaiming semantic understanding. These tools classify path/diff patterns; they do not prove code safety. Keep severity conservative and findings explainable. Do not add network lookups or live filesystem access. Do not make path scope claims involving symlinks unless the caller supplies resolved path metadata.

## Handoff notes

Implement `path_batch_scope_check` first because it can reuse existing path logic and has clear tests. Then implement shared path bucketing helpers and `repo_tree_summarize`. Implement `diff_risk_classify` last because it benefits from both path bucketing and existing patch parsing.
