# Phase 02: correctness hardening and edge-case safety

## Purpose

Fix concrete correctness issues and harden externally callable tools before adding new capabilities. This phase targets panics, Unicode hazards, inconsistent budget semantics, route-critical response guarantees, and malformed-input behavior.

## Current observations

`config_file_inspect` masks secret-like values using byte slicing: a leading `&val[..2]` and trailing `&val[val.len() - 2..]` when the value byte length is greater than four. That can panic on arbitrary UTF-8 if either boundary falls inside a multibyte scalar. Config values are untrusted repo text, so this should be treated as a real handler panic risk.

`BudgetContext::check_text_len` compares `text.len()` to `max_text_chars`. In Rust, `str::len()` returns bytes, not Unicode scalar values. Some tools use `.chars().count()` for length limits, creating inconsistent semantics and confusing error messages.

The route-critical tools are intended to produce stable machine-readable outputs. Current route-critical tools include `edit_preflight`, `command_preflight`, `config_preflight`, `patch_apply_check`, and `text_security_inspect`. New and existing tests should guarantee that these tools do not silently omit `machine_code`, `verdict`, or canonical findings on important paths.

## Implementation plan

1. Add UTF-8-safe masking helper.

   Create a helper function in an appropriate shared module, likely `src/tools/helpers.rs` or local to `src/tools/repo.rs` if use is narrow:

   ```rust
   fn mask_secret_preview(value: &str) -> String
   ```

   Required behavior:

   - Empty string returns `***` or another constant redaction.
   - Very short strings return `***`.
   - Longer strings preserve a small number of leading and trailing Unicode scalar values without byte-boundary panics.
   - The helper should not leak full values for short secrets.
   - The helper should not allocate excessively for normal config values.

   Replace the byte slicing in `config_file_inspect` with this helper.

2. Add Unicode regression tests for config masking.

   Add tests that call `config_file_inspect` with secret-like keys and values containing:

   - ASCII secret values.
   - Emoji suffix/prefix values.
   - CJK characters.
   - Combining marks.
   - Two-byte and three-byte UTF-8 characters near the preview boundary.
   - Short values of length 0, 1, 2, 3, and 4 scalar values.

   Assert no panic, result is successful or review as expected, and `value_preview` never equals the full original secret value for short or normal secrets.

3. Decide budget length semantics.

   Make an explicit decision for text budget limits:

   - Preferred for resource safety: byte limits. Rename documentation and error messages from `max_text_chars` to byte-oriented language where possible, and consider adding future-compatible fields/helpers for char-count limits separately.
   - If API compatibility prevents renaming now, keep the field but document that it is currently enforced as bytes and file a follow-up for a semver cleanup.
   - If character semantics are chosen instead, change `check_text_len` to use `.chars().count()` and audit all tools for memory-risk coverage through `max_input_bytes` and serialized argument checks.

   Avoid a mixed state where some handlers reject by bytes and others by scalar count without documentation.

4. Audit `unwrap`/`expect` in public handlers.

   Search `src/tools/`, `src/mcp/server.rs`, `src/agent/`, and route-critical helper paths for `unwrap`, `expect`, indexing, and byte slicing. Classify each occurrence:

   - Compile-time/static invariant, acceptable.
   - Internal invariant guarded by prior validation, acceptable but should have a test.
   - User-input reachable, must become structured error.

   For public tool handlers, malformed user input should produce `ToolResponse::error_with_code` rather than panic.

5. Add route-critical response contract tests.

   For each route-critical tool, add at least one allow/success case and one review/block/error case. Assert:

   - Response envelope has `machine_code`.
   - Response envelope has `verdict` where applicable.
   - Result object contains `machine_code` and/or `verdict` if the tool contract says so.
   - Findings, if present, contain canonical `code`, `severity`, `message`, and `disposition` fields.
   - Recommended next tool names, if present, exist in the registry.

6. Harden malformed-input behavior.

   Add tests for malformed or edge-case inputs across high-risk tools:

   - `config_file_inspect` with invalid JSON/TOML/YAML-like text.
   - `repo_manifest_inspect` with non-string path array entries.
   - `patch_apply_check` and `patch_summary` with malformed diffs and oversized text.
   - `edit_preflight` with conflicting mode arguments and invalid line ranges.
   - `text_security_inspect` with mixed Unicode controls/confusables.

7. Ensure all hardening preserves compatibility.

   Do not change existing machine codes, verdict strings, field names, or public schemas unless required to fix a bug. If a schema change is necessary, update generated docs and tests in the same commit.

## Tests to add or update

- Unit tests for `mask_secret_preview` with Unicode boundary cases.
- Integration-style tool tests invoking `config_file_inspect` through `ToolRegistry` where possible.
- Route-critical contract tests covering success and failure paths.
- Budget length tests that document byte/character semantics.
- Panic-free tests for malformed input to repo/config/patch/edit tools.

## Acceptance criteria

- No UTF-8 byte slicing panic remains in secret masking.
- Budget text length semantics are explicit in code comments, docs, and error messages.
- Public handlers do not panic on malformed user-controlled input found by the audit.
- Route-critical tools have contract tests for machine code, verdict, findings, and recommended next-tool validity.
- `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, and generated-docs check pass.

## Risks and constraints

Be careful not to accidentally expose secret values in test snapshots or error text. Tests should use dummy secrets only. Avoid large API-breaking renames in this phase unless the project is intentionally preparing a breaking release. Prefer additive helpers and clearer docs if compatibility is uncertain.

## Handoff notes

Prioritize the UTF-8 masking fix first because it is a concrete panic. Then do budget semantics and route-critical contract tests. If the unwrap audit is large, create a checklist in the PR description or a follow-up plan for non-route-critical cleanup rather than blocking the entire phase.
