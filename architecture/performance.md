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

The MCP warm scenario is a sequential request/response measurement through one
persistent `eggsact --mcp` child: process startup, protocol setup, and warmup
are outside the timed loop; each measured operation writes and flushes one
request, reads one complete response line, and verifies its JSON-RPC id. Any
cold-start measurement must use an explicitly cold-start name. Input-budget
scenarios call `ToolRegistry::call_json_with_budget` with a budget that forces
the production `input_too_large` short circuit, rather than timing a duplicate
JSON length calculation. Representative `diff_spans` and `RepoFacts` cases
are retained alongside small micro-cases so scaling evidence is not inferred
from tiny inputs.

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
- `text_replace_check` position indexes preserve the historical contract that
  a match beginning on CR, LF, or either codepoint of CRLF reports the start of
  the following line. The index is built in one pass; boundary regressions are
  pinned by differential tests.

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

The three performance plans were execution records until their benchmark,
focused-test, merge-gate, and release-contract evidence was captured in
`plans/roadmap.md`; they were then pruned. Git history retains the detailed
implementation discussion.
