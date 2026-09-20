# Performance Evidence and Hot-Path Contracts

eggsact's performance work is measured with a dependency-free release-mode
harness rather than wall-clock assertions in ordinary CI:

```bash
cargo bench --locked --bench performance
```

The harness prints stable `key=value` records for the source label,
toolchain, target, operating system, architecture, CPU, warmup policy, and
scenario measurements. Set `EGGSACT_BENCH_SHA` when recording a candidate SHA.
Compare baseline and candidate output on the same host/toolchain; timing is
evidence, not a merge threshold. The matrix covers registry setup, legacy and
modern listings, discovery, response serialization, input accounting, MCP
stdio, patching, replacement, JSON, repo facts, diffs, and encodings.

## Boundary invariants

- `python_json_dumps` keeps the established Python-style separators, ordering,
  numeric formatting, and non-ASCII escaping. ASCII output uses a direct
  fast path; Unicode output still uses the escaping path.
- Budget-only JSON size checks use a counting writer. A payload that is
  returned is serialized once for the fallback text and reused by the wrapper.
- Prepared tool calls perform one `ToolSpec` lookup, apply profile and
  audience policy directly, and retain private cost metadata for the server.
  The public `ToolCallOutcome` shape is unchanged.
- The stdio writer owns reusable serialization storage and still emits one
  newline-terminated JSON object and flushes once per response.

## Patch contract

Unified-diff hunk coordinates refer to the original source line space, in
patch order. Successful application walks the source once, emitting unchanged
ranges and hunk output into one result buffer. A prior insertion or deletion
therefore changes destination line numbers but not the source coordinate of a
later hunk. Overlapping or out-of-order hunks fail deterministically.

Strict mode validates all expected context against the source slice. Lenient
mode permits context extending past EOF and reports the existing truncation
finding. Failure evidence remains owned only on failure. Result text is
materialized only when requested or when a fingerprint is requested, and
fingerprints describe the applied result even when `return_result_text` is
false. CRLF source lines retain CRLF style for emitted additions.

The three performance plans are execution records until their benchmark,
focused-test, merge-gate, and release-contract evidence is captured in
`plans/roadmap.md`. Plan files are pruned only after that record is complete;
git history retains the detailed implementation discussion.
