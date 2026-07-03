# Phase 6: First-Class codegg Edit Preflight

## Goal

Promote `edit_preflight` into the canonical harness API for model-authored edits. codegg should be able to call one typed preflight path before applying an edit and receive a deterministic route decision with machine-readable reasons, strict findings, optional sub-results, and stable audit metadata.

This phase builds on the phase 01–05 closure work. Profile/audience dispatch, route-critical response contracts, generated docs, strict typed preflight parsing, structured next-tool hints, and active-profile MCP regression coverage are now in place. Phase 6 should avoid reopening those seams and focus on making edit preflight complete, precise, and dependable for harness use.

## Current State

Recent implementation already moved `edit_preflight` forward:

- `EditPreflightInput` includes target path/workspace root fields, newline policy, Unicode policy, and edit metadata.
- `EditPreflightOutput` includes typed verdict, machine code, summary, findings, structured next-tool hints, optional `path_scope`, `newline_check`, `unicode_check`, and `fingerprint` sub-results.
- `edit_preflight` composes literal replacement, patch, line-range, fingerprint, path scope, newline detection, and Unicode security checks.
- It emits canonical route fields: `ok_to_apply`, `verdict`, `machine_code`, `summary`, and response-level findings.
- Route-critical tests cover the basic requirement that `edit_preflight` emits machine code and verdict.
- Enhanced edit preflight tests cover path scope, newline, Unicode, fingerprint, and metadata paths at a high level.

The remaining work is refinement and hardening, not a ground-up implementation.

## Scope

Phase 6 covers only edit preflight. Do not expand command policy, dependency inspectors, runtime cancellation, evaluator isolation, or broad repo-audit work in this phase. Those are phases 7–10.

## Workstream A: Tighten Edit Mode Semantics

### Problem

`edit_preflight` supports literal, patch, and line-range modes. The API is powerful, but it needs explicit per-mode argument contracts so model/harness misuse fails predictably.

### Required Work

1. Define exact mode contracts in code and docs.

   - `literal` requires `old` and `new`.
   - `patch` requires `patch`.
   - `line_range` requires `start_line`, `end_line`, and replacement text. Prefer `new` for replacement text unless a distinct field already exists.
   - Common optional inputs: `expected_fingerprint`, `file_path`, `workspace_root`, `newline_policy`, `unicode_policy`, and `edit_metadata`.

2. Add mode-specific validation before sub-tool execution.

   Invalid mode/argument combinations should return a route-critical non-OK response or registry validation error consistently. Prefer schema validation for structural failures and handler machine codes for semantically invalid combinations.

3. Add machine codes for mode contract failures if existing codes are too broad.

   Candidate codes:

   - `EDIT_MODE_INVALID`
   - `EDIT_ARGUMENTS_MISSING`
   - `EDIT_ARGUMENTS_CONFLICT`

   If new codes are added, update `machine_codes::ALL`, architecture docs, and tests.

4. Add tests for each invalid mode shape.

   Required cases:

   - literal without `old`.
   - literal without `new`.
   - patch without `patch`.
   - line_range without `start_line`.
   - line_range with `start_line > end_line`.
   - unknown replacement mode.
   - conflicting inputs where a mode-specific conflict should be rejected.

### Acceptance Criteria

- Every edit mode has a documented argument contract.
- Invalid mode shapes fail deterministically.
- Error responses use stable machine codes or schema errors.
- Tests cannot pass by accidentally exercising a different edit mode.

## Workstream B: Normalize Verdict and Machine-Code Taxonomy

### Problem

`edit_preflight` currently maps findings into `allow`, `review`, and `block` and picks the first machine code from a code list. This is good enough for coarse routing, but codegg needs predictable priority ordering and stable mapping from cause to action.

### Required Work

1. Define priority order for findings.

   Recommended priority:

   1. path scope escape or traversal.
   2. invalid line range.
   3. patch parse/apply failure.
   4. no match or multiple matches.
   5. fingerprint mismatch.
   6. Unicode block/review.
   7. newline inconsistency.
   8. success.

2. Implement a single helper to derive:

   - primary machine code.
   - verdict.
   - `ok_to_apply`.
   - `recommended_next_tool`.
   - summary.

   This should replace scattered ad hoc machine-code selection.

3. Keep the response-level `machine_code` as the primary code.

   If multiple findings exist, consider adding `result.machine_codes: Vec<String>` or `result.secondary_machine_codes` while keeping the envelope `machine_code` stable.

4. Document the mapping in `architecture/machine-codes.md` or the MCP architecture doc.

5. Add table-driven tests for the mapping.

### Acceptance Criteria

- Given the same findings, the primary machine code is deterministic.
- More severe findings always dominate less severe findings.
- `ok_to_apply` and `verdict` cannot disagree.
- Tests cover mixed-finding cases.

## Workstream C: Improve Path Scope Handling

### Problem

Path scope is now present, but edit harnesses need exact behavior for repository-root constraints, relative paths, symlinks, and traversal-style inputs.

### Required Work

1. Clarify path semantics.

   - `workspace_root` should be the canonical project root passed by codegg.
   - `file_path` may be relative or absolute.
   - Relative paths are resolved against `workspace_root`.
   - `..` traversal should be flagged even if final normalized path remains inside root.
   - Absolute paths outside root block.

2. Decide whether symlink resolution is in scope.

   If not in scope, document that phase 6 performs lexical path checks only and add a phase-9 or later hardening note for canonical filesystem-aware checks.

3. Ensure `path_scope` sub-result contains stable fields:

   - `inside_root`
   - `escapes_via_dotdot`
   - `relative_path`
   - optional `normalized_target`
   - optional `reason`

4. Add tests for:

   - relative safe path.
   - absolute safe path under root.
   - relative `../` traversal.
   - absolute outside-root path.
   - path with redundant segments.

### Acceptance Criteria

- Path violations produce block verdicts.
- Path warnings are represented as structured findings.
- Safe paths do not produce spurious findings.

## Workstream D: Complete Newline Policy

### Problem

Newline inspection exists, but the policy should define whether mixed newlines are review-only, block-worthy, or normalized.

### Required Work

1. Define policy values and semantics.

   Recommended values:

   - `skip`: do not inspect newlines.
   - `check`: inspect and warn/review on mixed newlines.
   - `normalize_lf`: report expected normalization to LF.
   - `normalize_crlf`: report expected normalization to CRLF.

2. Decide whether `edit_preflight` actually normalizes candidate replacement text or only reports normalization requirements.

   Recommended for this phase: report only. Do not mutate replacement text inside preflight.

3. Ensure `newline_check` sub-result includes:

   - original style.
   - replacement style if applicable.
   - mixed flag.
   - policy.
   - recommended normalization if any.

4. Add tests for LF, CRLF, mixed, empty, and no-newline inputs.

### Acceptance Criteria

- Newline policy is deterministic and documented.
- Preflight does not silently mutate edit text.
- Mixed newline warnings produce review verdict unless combined with block-level findings.

## Workstream E: Complete Unicode Policy

### Problem

Unicode inspection is wired in, but policy handling needs exact source-text selection and stable finding projection.

### Required Work

1. Define which text is inspected by mode.

   - literal: inspect `new`.
   - patch: inspect added lines from patch, not the entire raw patch if feasible.
   - line_range: inspect replacement text.

2. Define policy values.

   Recommended values:

   - `skip`
   - `default`
   - `source_code`
   - `identifier`

3. Preserve findings from `text_security_inspect` rather than collapsing all Unicode risks into one generic finding where feasible.

   Use a parent finding only when the sub-tool output lacks structured findings.

4. Add tests for:

   - harmless ASCII.
   - bidi control character.
   - invisible separator.
   - known confusable in source-code context.
   - policy skip.

### Acceptance Criteria

- Unicode risk routes to review or block according to sub-tool verdict.
- Findings include enough detail for codegg to display actionable diagnostics.
- Policy skip avoids sub-tool execution and output fields.

## Workstream F: Audit Metadata and Traceability

### Problem

`edit_metadata` exists, but it must be safe, bounded, and useful for logs without becoming an arbitrary unbounded blob.

### Required Work

1. Define metadata shape.

   Suggested fields:

   - `description`
   - `author`
   - `source_tool`
   - `session_id`
   - `request_id`

2. Bound metadata field lengths.

   Apply existing text length policies or add explicit metadata limits.

3. Reflect metadata in result only when useful.

   Do not echo sensitive or large values by default. Prefer summaries or normalized fields.

4. Add tests for metadata presence, oversized metadata, and unknown metadata keys.

### Acceptance Criteria

- Metadata is bounded.
- Unknown or oversized metadata fails predictably or is ignored according to documented policy.
- Logs/results do not echo unbounded user text.

## Workstream G: Typed API and Generated Tool Cards

### Problem

codegg should consume the typed Rust API, but generated tool cards and schema docs need to match the final edit preflight contract.

### Required Work

1. Update schemas for every new input/result field.

2. Update `EditPreflightInput` and `EditPreflightOutput` docs.

3. Regenerate docs and tool cards.

4. Add generator tests if route-critical field metadata is extended.

5. Add examples showing typical harness calls:

   - safe literal replacement.
   - blocked path escape.
   - review due to fingerprint mismatch.
   - review due to Unicode risk.

### Acceptance Criteria

- `cargo run --bin generate-docs -- --check` passes.
- Generated cards include all edit preflight required and optional fields.
- Typed API examples compile or are covered by tests.

## Test Plan

Add or update tests in these areas:

- `tests/mcp/test_edit_preflight_enhanced.rs` for subprocess MCP behavior.
- `src/preflight/mod.rs` unit tests for typed parsing and contract violations.
- route-contract tests for route-critical response shape.
- schema tests for input/output fields.
- generated-doc tests if tool-card metadata changes.

Required commands:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Final Acceptance Criteria

Phase 6 is complete when:

- `edit_preflight` is the single recommended harness preflight path for model-authored edits.
- Each edit mode has strict, tested argument semantics.
- Path scope, newline, Unicode, fingerprint, and metadata behavior are documented and tested.
- Route decisions use deterministic machine-code priority.
- Typed Rust wrappers expose all route-critical output fields.
- Generated docs/tool cards are current.
- Full CI passes.
