# Performance 04 — Closure Evidence and Compatibility Corrective

Planning baseline: `cbb4047be3f41d6f4a3147cd38dad8be92efdda7` (`main`, 2026-09-20)
Implementation baseline: `85e10bf42436667cafa6380f8c871a4e4ee83a52`
Status: planned
Priority: P1
Scope: closure evidence and compatibility correction only; no new broad optimization campaign

## Objective

Reopen the 2026-09-20 performance campaign narrowly enough to resolve the
remaining closure defects discovered after the implementation and documentation
pass:

1. verify and, if required, restore `text_replace_check` line/column
   compatibility at newline boundaries;
2. replace the current process-startup-dominated MCP benchmark with a true
   persistent-process warm request/response measurement;
3. benchmark Eggsact's actual serialized-input budget path instead of an
   unrelated `serde_json::to_vec(...).len()` proxy;
4. requalify the `diff_spans` and `RepoFacts` changes with workloads that
   exercise the intended optimization;
5. explicitly implement or decline regex capture-name metadata reuse using a
   representative named-capture workload.

The performance implementation in `85e10bf` remains the working baseline.
This corrective is not authorization to revisit already-closed runtime,
dependency, calculator, transport, or tool-surface decisions.

## Why closure is being reopened

Commit `cbb4047` recorded the campaign as complete after local qualification
and a same-host benchmark run. A post-closure audit found several evidence and
compatibility gaps that were not sufficient reasons to reject the implementation,
but are sufficient reasons to keep the line open until corrected.

### A. Possible text_replace_check position regression

The new `SourceIndex` records a position for each codepoint boundary while
updating line/column state during iteration. Existing tests verify byte offsets,
Unicode handling, newline-style conversion, and multiline match counts, but do
not pin the reported `PositionInfo.line` / `column` for matches beginning on:

- LF;
- CR;
- the CR portion of CRLF;
- the LF portion of CRLF;
- a normal codepoint immediately after each newline form.

The pre-campaign implementation must be treated as the compatibility reference
unless a prior documented bug proves otherwise.

### B. The current "warm MCP" benchmark is not warm

`benches/performance.rs::mcp_stdio_warm_cheap_call` currently spawns a fresh
`eggsact --mcp` process inside every measured iteration, sends one request,
waits for process exit, and repeats.

The recorded +11.7% candidate delta therefore includes process startup,
runtime initialization, allocator state, and shutdown. It does not answer the
planned question of steady-state MCP request latency through a persistent stdio
server.

### C. Input-accounting benchmark does not execute the production path

The current benchmark labels:

- `input_accounting_small`;
- `input_accounting_near_limit`

measure `serde_json::to_vec(...).len()` directly. The implementation changed
`ToolRegistry::check_input_size` to a counting writer using
`serde_json::to_writer`. The benchmark therefore does not measure the code
whose performance was changed and its +11.1% / +7.4% deltas are not meaningful
closure evidence.

### D. diff_spans / RepoFacts scenarios are too small

The current evidence reports:

- `repo_facts`: +5.7% on four paths;
- Unicode `diff_spans`: +19.5% on two very small strings.

Those workloads do not substantially exercise the repeated normalization/path
passes or repeated UTF-8 offset scans the implementation was intended to
remove. The slower numbers must not be treated as proof that the changes are
regressions until representative workloads are measured, but they also cannot
be waved away without evidence.

### E. Regex capture-name reuse was neither implemented nor explicitly declined

The performance plan called for reuse of named-capture metadata within an
invocation. `85e10bf` did not modify `src/text/regex_engine.rs` or the
finditer capture-name path. The checked-in benchmark measures one small
`captures("hello world")` call and reports +5.5%, which does not answer
whether repeated named captures are worth optimizing.

Closure requires an explicit implement-or-decline decision backed by a
representative workload.

## Standing constraints

Preserve all of the following:

- one Rust crate and Rust 1.89 MSRV;
- all 86 registered capabilities and current ToolSpec schemas;
- public Rust API and MCP wire compatibility;
- legacy and modern protocol behavior;
- direct/discovery profile, audience, exposure, and ranking semantics;
- serde_json `preserve_order`;
- deterministic tool outputs and machine/error codes;
- request/output bounds, timeout, cancellation, semaphore, panic isolation, and
  writer ordering/flush semantics;
- existing patch corrective behavior from `85e10bf`;
- current dependency graph unless a corrective absolutely requires otherwise
  (none is expected).

Do not use this corrective to:

- redesign `spawn_blocking`, sync_pool, runtime config locks, or task topology;
- add a benchmark framework/runtime dependency;
- reopen eggfetch/updater features;
- add calculator fast paths;
- alter list near-match semantics;
- change discovery weights or descriptions;
- pursue binary-size work unrelated to a corrective change.

## Part A — Prove text_replace_check newline-position compatibility first

Primary files:

- `src/text/replace.rs`
- `tests/text/test_replace.rs`

Use a detached worktree at the planning baseline before `85e10bf`
(`23af42219ef1088ddeff080a2e07a3361e8468c3`) to record the exact historical
`PositionInfo` values for a minimal matrix.

Required cases:

1. match starts on LF in `"a\nb"`;
2. match starts on CR in `"a\rb"`;
3. match starts on CR in `"a\r\nb"`;
4. where representable under exact search, match starts on LF in CRLF;
5. match starts on `b` immediately after LF;
6. match starts on `b` immediately after CR;
7. match starts on `b` immediately after CRLF;
8. Unicode codepoints before and after the newline so byte/codepoint/line/column
   mappings are all exercised;
9. empty-string matching at newline boundaries.

Record for each:

    codepoint_index
    byte_start
    byte_end
    line
    column

### Decision rule

If `85e10bf` differs from the historical behavior:

- first determine whether the old value was covered/documented as intentional or
  was a known correctness bug;
- absent evidence of an intentional bug fix, restore historical semantics;
- add exact regression assertions so future indexing changes cannot drift.

Prefer correcting `SourceIndex` construction/state ordering rather than
special-casing newline matches after the fact.

Do not change the existing 99% many-match optimization architecture merely to
restore position parity.

## Part B — Replace the MCP benchmark with a true persistent-process measurement

Primary files:

- `benches/performance.rs`
- `architecture/performance.md`
- `docs/verification.md` if benchmark instructions need clarification

Keep cold-start measurement separate from warm request latency.

### Warm benchmark contract

For the warm MCP scenario:

1. locate the release/bench `eggsact` binary as today;
2. spawn one `eggsact --mcp` child outside the timed iteration loop;
3. keep stdin/stdout pipes open;
4. perform any required protocol setup outside timing;
5. run at least the existing warmup count through the same child;
6. for each measured iteration:
   - send one valid request with a unique JSON-RPC id;
   - flush stdin;
   - read exactly one complete response line;
   - validate/correlate the response id or otherwise prove the expected reply was
     received;
7. stop timing before child shutdown;
8. close stdin and reap the child after measurements.

Do not queue all requests and time bulk shutdown; this benchmark should measure
round-trip sequential request latency through a warm server.

If a cold-start benchmark remains useful, name it explicitly
(`mcp_stdio_cold_start_cheap_call` or equivalent) so it cannot be confused
with steady-state latency.

### Acceptance

- process spawn is not inside the warm measurement closure;
- each measured operation includes request serialization/write, server
  parse/dispatch/execution/response, and response read;
- response correlation is checked;
- baseline and candidate are rerun on the same host/toolchain;
- the roadmap replaces the old +11.7% row with the corrected warm evidence and
  optionally records cold-start separately.

No fixed latency threshold belongs in ordinary CI.

## Part C — Benchmark actual input-size accounting

Primary files:

- `benches/performance.rs`
- `src/agent/mod.rs` only if a small internal refactor is needed to make the
  production path benchmarkable without public API expansion

Do not benchmark a duplicated counting-writer implementation.

Preferred approach: exercise an existing public bounded call path that invokes
`ToolRegistry::check_input_size` and short-circuits before handler execution.

For example:

- use a cheap deterministic tool;
- provide an explicit `ToolBudget` with `max_input_bytes` lower than the
  serialized argument length;
- benchmark small and near-limit argument objects;
- assert the response is the expected `input_too_large` path so the benchmark
  cannot silently start executing the handler.

If exact micro-isolation cannot be achieved without exposing new public API,
prefer this real end-to-end budget path over adding a public benchmark-only
function.

Rename any retained `serde_json::to_vec` primitive measurement so it does not
claim to represent Eggsact input accounting.

Rerun the baseline and candidate and replace the current +11.1% / +7.4% rows.

## Part D — Requalify diff_spans with an intended-workload fixture

Primary files:

- `benches/performance.rs`
- `src/text/diff.rs` only if evidence justifies a corrective

Keep the existing small Unicode case as a micro-case if useful, but add a
representative scenario that causes multiple emitted spans and repeated
codepoint-to-byte conversion in the baseline while staying within current DP
bounds.

Suggested characteristics:

- substantial non-ASCII content;
- hundreds to low-thousands of codepoints, sized so the bounded LCS path rather
  than the coarse fallback is exercised;
- multiple separated edits/spans;
- deterministic input construction;
- identical output asserted between baseline/candidate fixtures where practical.

Run at least three independent benchmark batches on baseline and candidate and
record a median or otherwise stable aggregate.

### Decision rule

- if the representative workload improves and the tiny case regresses only by
  sub-microsecond noise, keep the indexed implementation and record both;
- if representative workloads are consistently slower with no measurable
  allocation/complexity benefit, revert the diff-specific optimization;
- if results are effectively neutral, prefer the simpler implementation unless
  the indexed version materially improves worst-case scaling.

Do not replace the bounded LCS algorithm in this corrective.

## Part E — Requalify RepoFacts at realistic path counts

Primary files:

- `benches/performance.rs`
- `src/services/repo.rs` only if evidence justifies a corrective

Add deterministic mixed repository path sets at representative scale, including
at least:

- ~100 paths;
- ~1,000 paths or the maximum ordinary tool-facing bound if lower.

Include Rust/Python/JS/config/workflow/test/docs/manifests/lockfiles/hidden paths
so all major classifiers do useful work.

Measure the current one-pass analysis against the planning baseline on the same
host/toolchain.

### Decision rule

- keep the refactor if medium/large path sets improve or are meaningfully more
  allocation-efficient despite tiny-list noise;
- if it regresses across representative scales, simplify/revert only the
  RepoFacts optimization while preserving all output-order contracts.

Add or retain byte/order regression tests for buckets, entrypoints,
high-leverage paths, language evidence, ecosystems, and tool hints.

## Part F — Resolve regex named-capture metadata explicitly

Primary files if implemented:

- `src/text/regex_engine.rs`
- `src/text/validate.rs`
- relevant regex tests
- `benches/performance.rs`

Replace the single tiny named-capture benchmark with a workload that exercises
the suspected repeated metadata cost, for example:

- a compiled pattern with 2–4 named groups;
- input containing hundreds or thousands of matches;
- `regex_finditer` or equivalent path that builds output groupdict metadata for
  every match.

Run baseline/current evidence first.

### Decision rule

Implement per-compiled-pattern capture-name metadata reuse only if the workload
shows a material repeated cost.

If implemented:

- cache only metadata belonging to the already compiled regex instance;
- preserve backend selection, capture ordering, span semantics, zero-length
  advancement, and error behavior;
- do not add a process-global arbitrary-pattern compiled-regex cache.

If the measured benefit is negligible, explicitly decline the optimization in
the roadmap and remove/rename misleading benchmark evidence. A documented
decline is valid closure.

## Part G — Correct the durable performance documentation

Primary files:

- `plans/roadmap.md`
- `architecture/performance.md`

While this plan is active, the roadmap must state that:

- `85e10bf` implementation is landed and mostly qualified;
- the patch correctness fix and major measured wins remain valid;
- full campaign closure is reopened only for this plan's compatibility/evidence
  gaps.

At final closure:

1. record the newline-position parity result and any corrective commit;
2. replace the old MCP warm row with true persistent-process evidence;
3. replace the old input-accounting rows with actual production-path evidence;
4. record both small and representative `diff_spans` results and the keep/revert
   decision;
5. record 100/~1000-path RepoFacts results and the keep/revert decision;
6. record the representative regex named-capture result and explicit
   implement/decline decision;
7. preserve the original benchmark table as historical evidence only where
   still accurately labeled;
8. state that the performance plan is complete only after the corrected evidence
   is present.

After closure, prune this plan according to the repository convention; git
history retains the detailed handoff.

## Part H — Verification

Focused verification before the full gate:

    cargo test --locked --test lib text_replace
    cargo test --locked --test lib patch
    cargo test --locked --test lib diff
    cargo test --locked --test lib repo
    cargo test --locked --test lib regex
    cargo test --locked --test lib mcp
    cargo bench --locked --bench performance

Use the repository's actual accepted test filters if module filter names differ.

Then run the AGENTS.md merge gate in order:

    cargo fmt --all -- --check
    cargo run --locked --features dev-tools --bin generate-docs -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked --all-features -- --skip parity --test-threads=4
    cargo test --locked --doc

Also run:

    python3 scripts/check-release-contract.py
    cargo build --locked --release

and the documented release MCP smoke. Run `cargo deny` if dependencies or the
lockfile move; no dependency movement is expected.

Remote push CI must be green before final pruning/closure.

## Evidence format

Record at minimum:

    baseline SHA
    corrective candidate SHA
    rustc / target / OS / CPU
    benchmark warmup/repetition policy

    text_replace newline-position matrix baseline/current/final
    MCP persistent warm call baseline/final
    optional MCP cold-start baseline/final
    input accounting small baseline/final
    input accounting near-limit baseline/final
    diff small Unicode baseline/final
    diff representative multi-span baseline/final
    RepoFacts ~100 paths baseline/final
    RepoFacts ~1000 paths baseline/final
    regex named-capture many-match baseline/final
    regex decision: implemented | declined

    stripped release bytes baseline/final
    Cargo.lock package count baseline/final
    full merge gate
    release-contract smoke
    remote CI run

Do not invent precision beyond the benchmark method. For suspect/noisy scenarios,
prefer three independent runs and report the median rather than relying on one
batch.

## Completion criteria

This corrective is complete when all of the following are true:

- newline-boundary `PositionInfo` semantics are differential-tested against the
  pre-campaign implementation and compatibility is restored or an intentional
  bug correction is documented;
- the MCP warm benchmark uses one persistent server process and correlated
  per-request round trips;
- input-accounting evidence executes Eggsact's actual budget path;
- diff and RepoFacts optimizations are measured at representative scale and
  kept/reverted based on that evidence;
- regex capture-name reuse is either implemented with evidence or explicitly
  declined with evidence;
- no public API, MCP schema, tool count, capability, profile/audience, or
  deterministic-output regression is introduced;
- the full local qualification and remote push CI pass;
- `plans/roadmap.md` contains corrected closure evidence;
- this plan is pruned only after those conditions are met.
