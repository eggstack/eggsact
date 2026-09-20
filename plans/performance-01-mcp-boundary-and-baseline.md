# Performance 01 — Baseline and MCP Boundary Hot Path

Planning baseline: 23af42219ef1088ddeff080a2e07a3361e8468c3 (main, 2026-09-20)
Status: planned
Priority: P1
Depends on: none
Blocks: performance-02-patch-correctness-and-linear-apply.md, performance-03-tool-hotpaths-and-closure.md

## Objective

Establish reproducible performance evidence for Eggsact and remove the highest-confidence allocation/serialization overhead from the MCP and in-process dispatch boundary without changing any public API, MCP protocol shape, tool count, capability, profile/audience policy, timeout semantics, or output content.

The structural review at the planning baseline found that the architecture is generally healthy: release settings are already aggressive, schemas are cached, execution is bounded, and the server preserves panic/timeout/cancellation isolation. The main boundary cost is repeated serialization and ownership churn around otherwise deterministic tool work.

This plan intentionally starts with measurement and low-risk hot-path cleanup. It must not become a broad runtime redesign.

## Standing constraints

Preserve all of the following:

- one Rust crate and the current Rust 1.89 MSRV;
- all 86 registered tools and their canonical names;
- legacy and modern MCP protocol behavior and wire shapes;
- direct/discovery surfaces and current profile/audience/exposure rules;
- serde_json preserve_order behavior;
- public ToolRegistry, ToolCallOutcome, execution-context, preflight, text, calculator, and raw compatibility APIs;
- Python-style JSON text fallback semantics, including non-ASCII escaping behavior;
- current request/output limits and truncation behavior;
- current timeout, semaphore, spawn_blocking, panic-catch, cooperative cancellation, and writer ordering semantics;
- one flush per emitted MCP response unless separate evidence proves a safe protocol-equivalent alternative;
- no new runtime dependency solely for benchmarking or micro-optimization.

Do not use panic = abort. Eggsact intentionally catches handler/execution panics; abort would regress fault containment.

Do not remove serde_json preserve_order without a separate compatibility decision and byte-for-byte evidence.

## Part A — Add a lightweight release-mode performance harness

Create a maintainer-run benchmark/evidence path with no production dependency. Prefer a harness-false bench target or another small std-only release-mode runner over adding Criterion unless the existing implementation proves that insufficient.

The harness must be explicitly non-gating for wall-clock thresholds in ordinary CI. It should produce machine-readable or stable text output that can be compared before and after on the same host.

Record at minimum:

    baseline SHA
    candidate SHA
    rustc -Vv
    target
    OS / architecture
    release profile
    CPU model where available
    repetitions / warmup policy

Measure both warm and cold behavior where cache initialization materially changes the result.

### Required benchmark scenarios

Include representative measurements for:

1. ToolRegistry prepare/call setup for a cheap tool.
2. tools/list:
   - legacy full;
   - modern full;
   - compact;
   - narrow name filter;
   - discovery surface.
3. response wrapping/serialization:
   - small ASCII result;
   - large ASCII result near normal tool-output sizes;
   - non-ASCII result;
   - structured modern response.
4. input-size accounting on small and near-limit arguments.
5. schema validation for a simple and nested tool input.
6. end-to-end warm MCP stdio request/response loop for one cheap deterministic tool.
7. candidate tool-level workloads used by later plans:
   - text_replace_check with many matches;
   - json_compare unordered arrays;
   - json_extract summary;
   - patch_apply_check with multiple hunks;
   - tool_search;
   - repo_facts;
   - diff_spans;
   - codec_convert / hex.

Do not check host-specific timing constants into tests. The purpose is before/after evidence, not flaky performance CI.

## Part B — Eliminate duplicate MCP response serialization

Primary files:

- src/mcp/response.rs
- src/mcp/execution.rs
- src/mcp/server.rs
- relevant MCP response/modern protocol tests

Current behavior serializes the same ToolResponse more than once on a successful call:

1. truncate_response serializes response.result to evaluate the per-budget output ceiling;
2. build_tool_response / build_tool_response_modern serializes the ToolResponse to evaluate the outer response ceiling;
3. wrap_tool_response / modern equivalent serializes that ToolResponse again to build text content;
4. the stdio writer serializes the final outer JSON value.

Refactor internal ownership so the fallback text serialization needed by the wrapper is produced once and reused for the size check and response construction.

Acceptable implementation shape:

- add private/internal wrapper helpers that accept an already serialized fallback string;
- retain existing public wrapper functions and their behavior for 1.x compatibility;
- ensure legacy and modern paths share the optimized primitive where practical.

### Python-style JSON serialization

python_json_dumps currently serializes to a Vec, converts to String, then always allocates another String in escape_ascii_json.

Add an ASCII fast path:

- if the serialized output is ASCII, return/reuse it directly;
- if it contains non-ASCII, preserve the existing escaping behavior exactly.

Do not change spacing, ordering, numeric formatting, escaping, or error behavior.

### Size-only serialization

For code paths that serialize solely to count bytes, use a counting std::io::Write implementation with serde_json::to_writer / Serializer where it preserves the exact byte-count semantics.

Candidates:

- ToolResponse/result ceiling checks;
- agent input-size accounting.

Do not replace a serialization that is subsequently returned to the caller with a count-only pass.

### Acceptance

Add/retain byte-exact tests covering:

- ASCII strings;
- BMP non-ASCII;
- supplementary-plane Unicode;
- escaped control characters;
- nested arrays/objects;
- preserve_order object ordering;
- legacy text fallback;
- modern structuredContent plus text fallback;
- truncation boundary behavior.

## Part C — Remove avoidable request/dispatch cloning and repeated registry work

Primary files:

- src/mcp/server.rs
- src/agent/mod.rs
- src/mcp/registry/listing.rs
- src/mcp/registry/all_tools.rs

### C1. Tool-call arguments

handle_tools_call_shared currently obtains an owned arguments Value and then deep-clones it again before bounded execution.

Remove the second deep clone. Move the already-owned Value into the bounded execution path after all borrowing consumers are finished.

Preserve:

- validation order;
- error mapping;
- timeout/cancellation behavior;
- request id correlation;
- discovery facade behavior.

Add a regression using a large nested argument object so the ownership path is exercised.

### C2. Single ToolSpec lookup per prepared call

prepare_tool_call_with_policy currently performs multiple registry scans and materializes tools_for_profile() just to test membership.

Refactor the internal preparation path to:

1. look up ToolSpec once;
2. check profile membership directly against ToolSpec.profiles;
3. check exposure/audience directly against that spec;
4. validate arguments;
5. return enough private/internal metadata for the server to obtain ToolCost without a second get_tool lookup.

Do not change the public ToolCallOutcome shape. If needed, add a private PreparedToolCall or equivalent internal result and keep the public method as a compatibility projection.

With only 86 tools, do not add a HashMap solely to replace one linear scan unless measurement demonstrates value. Removing repeated scans/Vec allocation is the first goal.

### C3. Unknown-tool path

Avoid registry.tool_names() followed by an immediate to_vec() copy. Preserve the exact suggestion/error content and stable ordering.

## Part D — Make tools/list materialize only what survives filtering

Primary file:

- src/mcp/registry/listing.rs

Legacy list_tool_definitions currently materializes owned ToolDefinition values for the whole active profile before applying names/tier/tags filtering. Change the pipeline so ToolSpec references are filtered first and only surviving entries allocate owned strings/schema Values.

Preserve exact registry order and output shape.

For compact schema handling, remove output_schema.clone() when the clone exists only to borrow/compact and immediately replace the value. Compact from the original schema Value or restructure the local ownership.

For modern listing, avoid Vec<&&ToolSpec>; retain ordinary references with one level of indirection.

Required regression cases:

- no filter;
- name filter;
- tier filter;
- tags filter;
- combinations;
- compact/normal/full detail;
- direct/discovery;
- Model/Harness audiences;
- deprecated/hidden containment;
- stable output ordering.

Generated docs must remain unchanged unless the implementation legitimately changes source metadata, which this plan does not intend.

## Part E — Tighten schema-validation allocation without changing validation semantics

Primary file:

- src/mcp/schema_validation.rs

The schema cache is already correct and must remain.

Optimize only demonstrated per-call allocation:

- avoid allocating Vec<&str> for the common single expected-type case;
- recurse through borrowed additionalProperties schema Values instead of cloning a schema object per field;
- retain the existing pattern cache and validation/error wording.

Do not redesign uniqueItems or regex caching unless the benchmark identifies them as material after the boundary fixes.

## Part F — Reuse writer serialization storage

Primary file:

- src/mcp/server.rs

The dedicated writer task may own a reusable Vec<u8> scratch buffer:

1. clear the buffer;
2. serde_json::to_writer into it;
3. append newline;
4. write_all;
5. flush exactly as today.

This avoids a response-sized temporary String and reuses capacity across messages while preserving the single-writer ordering guarantee.

Add a test or structural assertion that responses remain one JSON object per line and are flushed through the same writer task.

Do not combine this with buffering multiple responses or changing channel capacity.

## Part G — Explicitly defer lower-confidence runtime changes

Do not modify in this plan unless the Part A evidence makes the case compelling:

- spawn_blocking / semaphore architecture;
- JoinSet plus inner spawn panic-isolation structure;
- sync_pool mpsc Receiver mutex / crossbeam migration;
- ACTIVE_PROFILE / schema-detail RwLocks;
- calculator regex warmup strategy;
- eggfetch features or updater transport;
- release profile;
- dependency graph.

Record measured observations if any of these appear unexpectedly dominant, then create a separate corrective rather than expanding this plan.

## Verification

Focused tests should cover the modified areas first, then run the repository merge gate from AGENTS.md:

    cargo fmt --all -- --check
    cargo run --locked --features dev-tools --bin generate-docs -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked --all-features -- --skip parity --test-threads=4
    cargo test --locked --doc

Also run:

- relevant MCP response/protocol/route/discovery tests;
- relevant execution safety/cancellation tests;
- the performance harness before and after on the same host;
- scripts/check-release-contract.py;
- cargo build --locked --release;
- staged eggsact --version and MCP smoke as documented by the repo.

If the implementation changes package count, dependency graph, or stripped release size by >=1 MiB or >=10%, stop and explain why before accepting it. No dependency growth is expected from this plan.

## Evidence to record at closure

Record in this plan or the roadmap before pruning:

    baseline SHA
    candidate SHA
    environment/toolchain
    benchmark scenario
    median or stable aggregate before
    median or stable aggregate after
    percent change
    stripped release bytes before/after
    Cargo.lock package count before/after
    full merge gate result
    MCP smoke result

Performance improvements should be described only where measured. Structural allocation removal may be noted separately from wall-clock improvement.

## Completion criteria

This plan is complete when:

- a repeatable release-mode benchmark path exists without a new runtime dependency;
- duplicate ToolResponse fallback serialization is removed;
- ASCII python_json_dumps avoids the second full-string escape allocation while Unicode output remains byte-compatible;
- size-only checks do not allocate full temporary JSON strings where avoidable;
- the second tool-call argument deep clone is gone;
- ordinary prepared tool calls use one ToolSpec lookup and no profile Vec allocation;
- tools/list filters ToolSpec values before owned schema/definition materialization;
- compact output-schema handling no longer deep-clones only to compact;
- schema validation removes the identified avoidable per-field allocations;
- the writer reuses serialization storage without changing flush/order semantics;
- public/MCP compatibility tests and the full merge gate pass;
- before/after evidence is recorded honestly, including neutral or negative results.
