# Performance 02 — Patch Correctness and Linear Application

Planning baseline: 23af42219ef1088ddeff080a2e07a3361e8468c3 (main, 2026-09-20)
Status: planned
Priority: P1
Depends on: performance-01-mcp-boundary-and-baseline.md
Blocks: performance-03-tool-hotpaths-and-closure.md

## Objective

Correctly qualify patch_apply_check semantics, resolve the suspected nonzero-hunk reconstruction defect if reproduced, and then remove its repeated whole-file copying so multi-hunk patch validation/application scales linearly with input plus patch size rather than approximately hunks multiplied by file size.

Correctness is the gate. Do not optimize around an unproven assumption about the current patch semantics.

## Why this is a separate plan

The planning-baseline implementation validates a hunk against hunk.old_start, but apply_hunk initializes its reconstruction cursor at zero and consumes hunk lines from the beginning of original_lines. patch_apply_check also clones the full original line vector and replaces current_lines with a newly allocated full vector for each successful hunk.

That combination suggests two issues:

1. a correctness risk for hunks whose old_start is not the first line;
2. repeated O(file_size) reconstruction for every hunk.

Because patch_apply_check is a coding-agent safety/preflight tool, the correctness question must be proven and fixed before any performance rewrite.

## Standing constraints

Preserve:

- public PatchHunk, PatchFile, PatchParseResult, FailedHunk, LineRange, PatchApplyCheckResult and PatchSummaryResult shapes;
- parse_unified_diff output semantics and public visibility;
- strict and non-strict behavior unless a regression proves current behavior incorrect;
- exact findings/reason strings where tests or public behavior rely on them;
- affected_line_ranges semantics;
- newline_style_before/newline_style_after semantics;
- result_fingerprint semantics;
- return_result_text and return_result_fingerprint flags;
- MAX_ORIGINAL_LENGTH and MAX_PATCH_LENGTH protections;
- deterministic behavior and no filesystem access;
- current multi-file behavior unless tests/documentation establish that it is erroneous and a compatibility-safe correction is explicitly documented.

Do not silently redefine unified-diff semantics in the name of speed.

## Part A — Reproduce and characterize the suspected nonzero-hunk defect

Before restructuring implementation, add focused regressions in tests/text/test_patch.rs and, where the MCP adapter matters, the patch MCP tests.

Required minimal cases:

1. Replace a middle line:

       original:
       a
       b
       c

       patch:
       @@ -2,1 +2,1 @@
       -b
       +B

   Expected result must preserve a and c and replace only b.

2. Insert after a nonzero prefix.
3. Delete after a nonzero prefix.
4. A hunk with context lines before/after the changed line.
5. Two separated hunks in one file.
6. A second hunk after an earlier hunk changes line count.
7. CRLF input and existing newline-style expectations.
8. strict=true context mismatch at a nonzero old_start.
9. strict=false truncated end-of-file behavior.
10. return_result_text=false with fingerprint requested.

If the baseline implementation unexpectedly passes, document why the cursor behavior is correct before proceeding. Do not rewrite a correct path based only on inspection.

If it fails, record the smallest failing fixture and classify the fix as a correctness corrective within this plan.

## Part B — Lock down the effective public contract

Read existing tests, parity expectations, generated schemas/docs, and any callers of patch_apply_check.

Before implementing a new engine, explicitly document:

- whether hunk line numbers are interpreted relative to the original or evolving result;
- how prior insertions/deletions affect subsequent hunk positioning;
- how zero-count insertion/deletion ranges behave;
- whether multiple PatchFile entries are meaningful for one original_text argument or merely parse metadata;
- whether a trailing newline is intentionally removed/preserved by the current result_text path;
- strict vs non-strict context handling;
- behavior when one hunk fails and later hunks are examined;
- ordering of failed_hunks, affected_line_ranges, and findings.

Where the current behavior is odd but tested/public, preserve it unless it is the reproduced correctness bug being fixed.

## Part C — Replace per-hunk full-file reconstruction

Primary file:

- src/text/patch.rs

Preferred architecture after contract qualification:

1. parse the patch once;
2. retain source lines as borrowed slices or a lightweight line index where practical;
3. walk applicable hunks in deterministic order;
4. append unchanged ranges directly;
5. validate context against the correct source/evolving position;
6. append context/addition lines and skip deletion lines;
7. append the remaining tail exactly once;
8. materialize owned FailedHunk contexts only on failure;
9. construct result text once when required by output/fingerprint/newline semantics.

The implementation may instead maintain a single mutable output buffer if that more exactly preserves current multi-hunk behavior. The key invariant is that successful application must not clone/rebuild the entire file for each hunk.

### Allocation targets

Remove or reduce:

- original_text.to_string followed by split-to-owned String for every source line where borrowing suffices;
- original_lines.clone before the first hunk;
- new Vec<String> containing the whole file per hunk;
- expected_context and actual_context owned copies on successful strict checks;
- repeated normalize_line String allocations when trim_end_matches can be compared as borrowed slices.

Owned strings remain appropriate in public result objects and failure evidence.

### Capacity planning

Where result length can be estimated safely from input and additions/deletions, reserve a reasonable output capacity. Do not perform a second expensive full pass solely for exact capacity.

## Part D — Preserve cancellation/bounds behavior

Patch operations are bounded by existing input limits. If the new single-pass engine contains a long loop over hunks or lines, use the repository's cooperative cancellation mechanism at a coarse interval consistent with other moderate-cost tools.

Do not add per-line atomics if measurement shows that materially harms the common path; periodic checks are sufficient.

## Part E — Add performance scenarios

Use the Part 01 harness to compare baseline/corrected behavior on at least:

- 100k-character source with one late hunk;
- 100k-character source with 10 separated hunks;
- 100k-character source with 100 small separated hunks, if allowed by current patch limits;
- strict success;
- strict failure;
- non-strict truncated context;
- return_result_text on/off;
- fingerprint on/off.

Track time and, if available without a runtime dependency, peak/resident memory or allocation observations.

The expected complexity target is approximately O(source + patch + emitted output), not O(hunks × source).

Do not check fixed latency thresholds into ordinary CI.

## Part F — Cross-check patch consumers

Run focused tests for:

- parse_unified_diff;
- patch_apply_check;
- patch_summary;
- PatchAnalysis / shared patch service;
- edit/dependency preflight consumers;
- MCP response/golden fixtures involving patch tools;
- parity fixtures where applicable.

The optimization must not create independent patch parsing logic in an adapter. Keep typed-core/service layering from AGENTS.md.

## Verification

Run the full AGENTS.md merge gate:

    cargo fmt --all -- --check
    cargo run --locked --features dev-tools --bin generate-docs -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked --all-features -- --skip parity --test-threads=4
    cargo test --locked --doc

Also run focused patch/property/parity tests and the release-mode performance scenarios from Part E.

If fixing the suspected defect changes a golden/parity result, document the before/after case and why the previous output was incorrect rather than hiding it as an optimization delta.

## Evidence to record at closure

Record:

    baseline failing/passing nonzero-hunk fixture
    correctness-fix commit if needed
    single-hunk benchmark before/after
    10-hunk benchmark before/after
    100-hunk benchmark before/after where applicable
    result-text/fingerprint compatibility evidence
    full merge gate
    relevant parity status

## Completion criteria

This plan is complete when:

- nonzero-hunk reconstruction behavior is proven with regression tests;
- the suspected cursor defect is either fixed or explicitly disproven by evidence;
- multi-hunk semantics are documented by tests before/with the rewrite;
- successful hunk application no longer rebuilds/clones the whole file per hunk;
- success-path context validation uses borrowed data where practical;
- output/fingerprint/newline/error shapes remain compatible except for a documented correctness correction;
- representative multi-hunk workloads show the expected complexity improvement;
- all focused and repository-wide gates pass.
