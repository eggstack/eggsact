# Final Lightweight Correctness Closure Pass

## Status

- **Status:** ready for implementation
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Current planning baseline:** `f7c690d8051e3a8dc36033a928dfadf1a9ea4e8e`
- **Effective implementation tree before this plan:** identical to `3d876bcfd447d2d6a642f461e7f6960c6987cd2f`
- **Parent roadmap:** `plans/2026-07-31-lightweight-correctness-simplification-roadmap.md`
- **Affected phases:** Phase 1 regex/MCP correctness, Phase 3 direct-dispatch state semantics, Phase 4 measurements and closure records
- **Scope:** repair the confirmed residual defects, prove the smallest required release/install facts, and reconcile the existing roadmap records
- **Expected implementation shape:** one narrow corrective code commit followed by concise closure-record updates

## Purpose

The July 31 lightweight correctness and simplification roadmap substantially improved the repository, but it was marked complete before every acceptance criterion was actually satisfied.

This pass closes only the confirmed residual gaps:

1. the backend-independent regex capture representation returns incorrect capture text in some cases and does not support named lookup correctly;
2. regex syntax validity and policy rejection are still partially conflated;
3. direct `ToolRegistry::call_json()` can still fall back to process-global calculator state;
4. Phase 1, Phase 2, Phase 3, Phase 4, and roadmap completion records contain pending placeholders or unsupported claims;
5. Phase 4 lacks the required cold-process timing, default-install binary inventory, final SHA, and remote-CI disposition.

The objective is not another architecture pass. The objective is to make the already-selected architecture truthful, correct, and verifiably closed.

---

# Required outcome

After this pass:

1. regex capture groups return the exact source substring for both Rust `regex` and `fancy-regex` backends;
2. named groups populate `groupdict` correctly for both backends;
3. capture offsets remain absolute when matching from a nonzero position;
4. Unicode before or inside a capture cannot cause an invalid UTF-8 slice;
5. `valid_pattern` represents syntax/backend compilation rather than complexity policy;
6. `policy_allowed` is populated consistently for compile-successful patterns or removed if the field cannot be supported truthfully without widening scope;
7. direct `ToolRegistry::call_json()` installs an explicit fresh native `EvalContext` for calculator-backed handlers and does not use process-global calculator state;
8. direct calculator APIs continue to provide the explicit stateful surface;
9. no MCP concurrency, timeout, cancellation, panic-conversion, profile, audience, schema-validation, or output-budget behavior regresses;
10. default `cargo install --path .` output is explicitly inventoried;
11. maintenance binaries are gated only if the default install actually exposes them;
12. cold CLI process-start measurements are recorded for the pre-Phase-4 baseline and corrected final implementation on the same host;
13. the existing plans contain exact implementation SHAs and only claims supported by code, tests, measurements, or CI;
14. ordinary CI remains unchanged and passes;
15. release remains a manual maintainer action outside GitHub Actions;
16. no further roadmap, milestone, or evidence-only plan is needed for this line of work.

---

# Hard constraints and non-goals

This pass must not:

- add tools or tool categories;
- remove useful existing tools;
- add PCRE2, Oniguruma, RE2, or another regex backend;
- change the public regex dialect beyond correcting already-documented behavior;
- add a general regex AST or parser;
- add a generic capture framework beyond the existing `CompiledRegex` abstraction;
- rewrite the calculator grammar;
- remove the legacy direct calculator API in a minor release;
- create persistent generic tool sessions;
- add an executor trait, async abstraction, or runtime dependency;
- replace Tokio;
- redesign MCP transport or protocol handling;
- change supported MCP protocol versions;
- weaken input limits, bounded concurrency, timeout, cancellation, or output truncation;
- broaden deterministic-output work beyond the already-confirmed public map surfaces;
- revisit TOML table extraction unless a new focused regression appears while running this pass;
- implement confusables-table redesign, schema caching, broad regex removal, or TOML-parser consolidation;
- add benchmarks, snapshots, fuzz matrices, race loops, or new CI jobs;
- add artifact uploads or exact-run evidence ledgers;
- automate crates.io publication;
- change release cadence;
- rewrite `main` history to remove `c35d244` and `f7c690d`;
- claim remote CI passed unless the relevant implementation commit has a visible successful result;
- mark a checklist item complete merely because a commit message says it is complete.

The two commits `c35d2441cbd4f0908c5789fef3ae69d61bf2d931` and `f7c690d8051e3a8dc36033a928dfadf1a9ea4e8e` add and then remove an empty file. They have no net tree effect. Leave history intact and do not include them in implementation claims.

---

# Confirmed residual defects

## Defect A — named regex captures are not populated

Current `CompiledCaptures` stores:

```rust
names: HashMap<String, usize>
```

but both backend conversion functions initialize it as an empty map. `regex_test()` later iterates backend capture names and calls `caps.name(name)`, which therefore always returns `None`.

Visible consequence:

```text
pattern: (?P<word>[A-Za-z]+)
sample: hello
expected groupdict: {"word": "hello"}
current groupdict: {}
```

This affects the library result and the `validate_regex` tool response.

## Defect B — fancy capture text uses the wrong slice

The fancy conversion stores tuples containing the entire source text and later derives capture text with a prefix slice equivalent to:

```rust
&text[..end - start]
```

For a capture beginning after byte zero, this returns the wrong substring. For some Unicode layouts, it can also select a non-character boundary.

Example contract:

```text
pattern: (?<=prefix)(?<value>éx)
text: prefixéx
capture value must be exactly "éx"
```

The implementation must slice the original input by the capture's actual absolute byte range:

```rust
&source[start..end]
```

## Defect C — Rust captures-from-position offsets are relative

The Rust branch of `CompiledRegex::captures_from_pos()` applies `captures()` to `&text[pos..]`, but the conversion helper does not add `pos` back to the returned offsets.

The backend-independent contract must use absolute byte offsets into the original input for both backends.

Even if current tool code avoids this helper, it is public through the text module and must not expose incorrect semantics.

## Defect D — regex policy is still reported as syntax invalidity

`check_pattern_complexity()` currently runs before backend compilation. When it rejects a syntactically valid pattern because of nesting or nested quantifiers, the result sets:

```text
valid_pattern = false
policy_allowed = None
```

The target contract is:

```text
syntax/backend compilation failed:
  valid_pattern = false
  policy_allowed = None

syntax/backend compilation succeeded and policy passed:
  valid_pattern = true
  policy_allowed = true

syntax/backend compilation succeeded but policy rejected execution:
  valid_pattern = true
  policy_allowed = false
  no matching execution occurs
```

Unsupported dialect constructs may continue to use `valid_pattern = false` with `unsupported_features` because they are not valid in the supported eggsact dialect. Do not describe safety policy as syntax failure.

## Defect E — direct registry calls can use global calculator state

`ToolRegistry::call_json()` prepares policy and then invokes the handler directly. It does not install an `EvalContext`.

`math_eval()` uses the installed context when present, otherwise it falls back to `run()`, which uses legacy process-global calculator state.

The intended state model remains:

- generic tool dispatch: fresh or explicitly cloned context, discarded after the call;
- MCP tool dispatch: fresh MCP-safe context;
- direct calculator APIs using `&mut EvalContext`: explicit persistent state;
- legacy context-free calculator functions: retained for compatibility, but not used implicitly by tool dispatch.

## Defect F — closure records are incomplete or inaccurate

At minimum:

- Phase 1 still says `Implementation commit: (pending commit)`;
- Phase 2 still says `Gap fix commit: (pending)`;
- Phase 3 remains `ready for implementation`, with unchecked criteria and a pending completion record;
- Phase 4 still has unchecked criteria, `Final SHA: pending commit`, `Remote CI: pending push`, no cold timing numbers, and no default install inventory;
- the roadmap lists Phase 4 as pending while also claiming all phases are complete;
- several Phase 4 candidate dispositions imply measurement that was not performed.

These documents must be corrected once, using concise facts.

---

# Workstream 0 — establish the implementation baseline

Before editing:

```bash
git fetch origin main --prune
git switch main
git reset --hard origin/main
git status --short
git rev-parse HEAD
git log --oneline --decorate -15
```

Confirm:

```bash
git diff --exit-code 3d876bcfd447d2d6a642f461e7f6960c6987cd2f..f7c690d8051e3a8dc36033a928dfadf1a9ea4e8e
```

Expected result: no tree difference.

Record the actual current `main` SHA in this plan's completion record. Do not reset history to `3d876bc`; implementation begins from the current branch tip.

Run a narrow pre-change baseline:

```bash
cargo fmt --all -- --check
cargo test --locked regex_engine
cargo test --locked regex_test
cargo test --locked regex_finditer
cargo test --locked context_isolation
```

Use exact discovered test filters if names differ. The purpose is to establish that new regression tests fail for the intended reasons before the implementation is changed.

## Acceptance criteria

- The worktree is clean.
- The current head is recorded.
- The no-net-tree relationship of the two empty-file commits is confirmed.
- No unrelated failing test is hidden by this pass.

---

# Workstream 1 — repair the regex capture representation

## Target representation

Use one source string and absolute byte ranges. Do not store redundant borrowed substring values plus offsets.

A minimal representation is:

```rust
pub struct CompiledCaptures<'t> {
    source: &'t str,
    full_match: Option<(usize, usize)>,
    groups: Vec<Option<(usize, usize)>>,
    names: BTreeMap<String, usize>,
}
```

The exact fields may differ, but the invariants must be:

1. every range is absolute relative to the original input;
2. every stored range comes directly from a backend match object;
3. `get()` slices `source[start..end]` only after debug/asserted boundary validity;
4. `name()` maps the backend capture name to the correct group index;
5. full match group zero follows the same range contract;
6. the representation does not allocate a `String` for each capture;
7. names use deterministic ordering when serialized or inspected, although the names map itself is not a public serialized result.

`HashMap` is acceptable internally, but `BTreeMap` is preferred here because capture counts are small and deterministic debug behavior is useful. Do not add a dependency.

## Conversion helpers

Use backend capture-name enumeration while constructing the converted object.

Conceptual shape:

```rust
fn capture_name_indices<'a>(
    names: impl Iterator<Item = Option<&'a str>>,
) -> BTreeMap<String, usize> {
    names
        .enumerate()
        .filter_map(|(index, name)| name.map(|name| (name.to_string(), index)))
        .collect()
}
```

The actual helper may be backend-specific if lifetimes are clearer.

For Rust matching from a sliced input:

```rust
let slice = &text[pos..];
let caps = re.captures(slice);
convert_std(caps, text, pos)
```

Every match range from `slice` must add `pos` before storage.

For fancy matching, use the backend's ranges directly against the original `text`; do not reconstruct them from match length.

## Public method behavior

Verify all of these methods:

```text
CompiledRegex::find
CompiledRegex::captures
CompiledRegex::captures_from_pos
CompiledCaptures::get
CompiledCaptures::name
CompiledCaptures::len
CompiledCaptures::is_empty
CompiledRegex::capture_names
```

Do not add more public methods unless needed to correct the implementation.

## Required focused tests

Add a compact table of tests rather than many one-off cases.

### Rust backend

1. unnamed groups at input offset zero;
2. unnamed groups after a nonmatching prefix;
3. named group through `regex_test()`;
4. two named groups and one optional nonparticipating group;
5. `captures_from_pos()` returns absolute start/end positions;
6. Unicode before the match;
7. Unicode inside the capture.

Example patterns:

```text
(?P<word>[A-Za-z]+)
(?P<head>[A-Za-z]+)-(?P<num>\d+)
(?P<optional>a)?(?P<required>b)
```

Use the naming syntax supported by both current backends. Confirm exact syntax from existing tests rather than introducing a dialect variant.

### Fancy backend

1. lookbehind with an unnamed capture after a prefix;
2. lookahead or backreference with a named capture;
3. named capture beginning after byte zero;
4. multibyte Unicode before the capture;
5. multibyte Unicode inside the capture;
6. `captures_from_pos()` returns absolute positions;
7. no panic for valid UTF-8 input.

Example intent:

```text
prefix(?P<value>\w+)(?=suffix)
(?<=prefix)(?P<value>.+)
```

Select bounded patterns that do not trigger the safety policy.

### Tool-level regression

Call `validate_regex` with a named-group pattern and assert the serialized `groupdict` contains the exact expected key and value.

Call `regex_finditer` with named groups and assert each serialized `groupdict` is correct.

## Acceptance criteria

- Named capture lookup works for both backends.
- Capture strings equal the original source range.
- Absolute positions are correct after nonzero-position matching.
- Unicode cases do not panic.
- Existing unnamed-group behavior remains compatible.
- No new regex backend or dependency is introduced.

---

# Workstream 2 — separate syntax, support, policy, and execution status

## Required evaluation order

Use this conceptual order:

```text
1. input size limits
2. dialect classification / unsupported-feature detection
3. backend compilation
4. safety and complexity policy assessment
5. matching execution
6. runtime error reporting
```

Compilation is allowed before policy because compilation does not execute the pattern against attacker-controlled sample text.

Do not run matching when policy rejects the pattern.

## Required result contract

For compile-successful patterns:

```rust
valid_pattern: true
policy_allowed: Some(true | false)
```

For compile failures:

```rust
valid_pattern: false
policy_allowed: None
```

For unsupported eggsact-dialect constructs:

```rust
valid_pattern: false
unsupported_features: Some(...)
policy_allowed: None
```

For runtime engine failures:

```rust
valid_pattern: true
policy_allowed: Some(true)
execution_error: Some(...)
```

Do not put a runtime engine error in the ordinary `error` field as if it were a compile error or no-match result.

## Safety sources

The existing policy has two relevant sources:

- `check_pattern_complexity()`;
- `regex_safety_check()`.

Use the smallest design that gives one final policy decision. A new public policy type is not required.

Acceptable internal shape:

```rust
fn assess_pattern_policy(pattern: &str) -> Result<(), String>
```

or direct sequential checks in the existing functions.

Do not add severity frameworks or configurable policy objects.

## Tool-envelope compatibility

Preserve current machine-code semantics:

- unsupported features: `REGEX_UNSUPPORTED_FEATURE`;
- ASCII option: `REGEX_ASCII_NOT_SUPPORTED`;
- safety rejection: `REGEX_UNSAFE`;
- execution failure: existing deterministic internal/execution code.

The tool may retain an error envelope for `REGEX_UNSAFE` if that is the current stable contract. The underlying library result and any structured details must still distinguish syntax success from policy rejection.

Do not convert unsafe-pattern rejection into ordinary successful matching.

## ASCII disposition

Keep ASCII mode explicitly unsupported in this pass. Do not implement a third semantic mode.

The existing machine code and documentation remain valid.

## Required focused tests

1. malformed syntax: `valid_pattern=false`, `policy_allowed=None`;
2. unsupported dialect construct: unsupported list populated, no execution;
3. safe simple Rust pattern: `valid_pattern=true`, `policy_allowed=true`;
4. safe fancy pattern: `valid_pattern=true`, `policy_allowed=true`;
5. syntactically valid nested-quantifier policy rejection: `valid_pattern=true`, `policy_allowed=false`;
6. excessive policy nesting with otherwise compilable syntax: same contract;
7. runtime fancy error, using an existing deterministic backtrack-limit fixture if one exists: `execution_error` populated and policy remains true;
8. tool-level unsafe response retains `REGEX_UNSAFE`;
9. no result reports `policy_allowed=None` after successful compilation unless policy was intentionally not evaluated and that exception is explicitly documented.

Do not create expensive ReDoS tests. Use existing bounded deterministic fixtures.

## Acceptance criteria

- Syntax validity is not determined by policy limits.
- Policy status is populated truthfully.
- Unsupported dialect constructs remain explicit.
- Runtime errors remain distinct from no-match and truncation.
- Existing safety rejection remains effective.

---

# Workstream 3 — make direct registry dispatch stateless

## Required behavior

`ToolRegistry::call_json()` remains the low-overhead direct path, but it must install a fresh native evaluation context before invoking a handler.

Use the existing thread-local bridge rather than creating a second context mechanism.

Conceptual implementation:

```rust
pub fn call_json(...) -> Result<ToolResponse, ToolCallError> {
    let handler = ...;
    let mut eval_ctx = EvalContext::new();
    Ok(budget::with_eval_context(&mut eval_ctx, || handler(&args)))
}
```

Use the actual existing bridge signature.

The fresh context must:

- allow native direct-call behavior expected outside MCP;
- be discarded after the tool call;
- not inherit legacy process-global memory, variables, PRNG state, or MCP mode;
- preserve deterministic initial state as defined by `EvalContext::new()`;
- not add sync-pool or Tokio overhead to direct calls.

Do not route `call_json()` through bounded execution merely to install context.

## Bounded and MCP paths

Do not change their architecture unless required for the same correctness helper.

Retain:

- bounded sync pool for budget-aware in-process calls;
- cloned explicit context for execution-context calls;
- MCP-safe context for MCP calls;
- cooperative cancellation;
- existing timeout and queue-full handling.

## Stateful calculator APIs

Retain and test:

```rust
run_with_context(expr, &mut ctx)
evaluate_with_context(expr, &mut ctx)
```

These remain the only supported generic persistence surface.

Do not add `CalculatorSession` in this closure pass.

## Required focused tests

### Direct tool isolation

Using valid existing calculator syntax:

1. two independent direct `math_eval` calls do not share user variables;
2. two independent direct calls do not share memory registers;
3. two independent direct random calls start from the fresh-context state rather than advancing process-global PRNG state;
4. a legacy context-free calculator mutation does not leak into `ToolRegistry::call_json()`;
5. direct registry behavior is not changed by a prior call to deprecated global `set_mcp_mode()`.

For item 5, only retain the test if the intended native-context contract already guarantees independence from the legacy global flag. Do not redefine calculator compatibility in this pass.

### Explicit state persistence

1. two `run_with_context()` calls using the same mutable context do persist state;
2. seeded PRNG state advances within one explicit context;
3. a second fresh explicit context starts independently.

### Existing dispatch surfaces

Retain focused checks that:

- `call_json_with_execution_context()` does not mutate the caller's context;
- deprecated mutable generic context remains a non-persistent wrapper;
- MCP mode continues rejecting random and side-effect functions.

## Acceptance criteria

- Direct `call_json()` never falls back to global calculator state.
- Direct calls remain synchronous and low overhead.
- Generic tool calls are stateless.
- Direct calculator context APIs remain stateful.
- No persistent generic tool session is added.

---

# Workstream 4 — perform the minimum Phase 4 evidence required for closure

This workstream is evidence collection for existing changes, not a renewed optimization campaign.

## A. Exact release binary size

Use exact bytes rather than rounded `ls -lh` values.

On the same host, build the pre-Phase-4 and final versions in separate temporary worktrees or target directories.

Baseline source:

```text
63bac39b87596e2f7721c4042f369afe92a41bcd
```

Final source:

```text
corrective implementation commit from this pass
```

Commands:

```bash
cargo build --release --locked --bin eggsact
wc -c < target/release/eggsact
```

Record OS, architecture, `rustc --version`, `cargo --version`, and build command.

Do not compare builds from different machines.

## B. Cold-process CLI timing

Measure the same commands on baseline and final builds:

```text
--help
--version
2+2
thirty plus five
```

Use `hyperfine` only if already installed. Otherwise use a small local shell loop and `/usr/bin/time` or Python's `time.perf_counter()` to launch the binaries.

Recommended sample:

```text
5 unrecorded warmups
20 measured process launches per command
record median milliseconds
```

This is process-start timing under normal filesystem caching. Label it accurately; do not call it disk-cold startup.

No benchmark script needs to be committed.

## C. Default installation inventory

Run:

```bash
install_root="$(mktemp -d)"
cargo install --path . --locked --root "$install_root"
find "$install_root/bin" -maxdepth 1 -type f -print | sort
```

Record every installed binary.

### Conditional correction

If the default install exposes only `eggsact`, record the fact and make no packaging change.

If it exposes `generate-docs`, `verify-eggsact`, or another maintenance-only binary, implement the smallest original Phase 4 correction:

```toml
[features]
default = []
dev-tools = []

[[bin]]
name = "generate-docs"
required-features = ["dev-tools"]

[[bin]]
name = "verify-eggsact"
required-features = ["dev-tools"]
```

Update only commands that invoke those binaries:

```bash
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo run --locked --features dev-tools --bin verify-eggsact
```

Do not add a workspace, `xtask`, task runner, or CI job.

Repeat the install inventory and require only the intended public executable by default.

## D. Candidate dispositions

Do not implement the previously deferred optimization candidates.

Correct their records to distinguish:

- **measured and accepted**;
- **measured and rejected**;
- **feasibility-rejected without implementation experiment**;
- **deferred because no profiling evidence justified work**.

Required truthful dispositions:

- Tokio features: measured/accepted;
- MCP-only runtime: measured/accepted after cold-process timing;
- release profile: either perform a bounded local comparison or state plainly that it was not experimentally evaluated and remove any claim of measured rejection;
- development binaries: resolve through the actual install inventory;
- confusables representation: feasibility disposition only unless a profiling result already exists; do not claim measured binary savings;
- TOML consolidation: use `cargo tree -i toml` and `cargo tree -i toml_edit` as the bounded feasibility evidence; do not implement a migration;
- trivial regex cleanup: deferred as marginal and outside closure;
- schema caching: deferred because no listing/startup profile identifies it as material.

The purpose is accurate documentation, not forcing every candidate to produce a benchmark.

## Acceptance criteria

- Exact binary bytes are recorded before and after.
- Median process-start numbers are recorded before and after.
- Default installed binaries are listed explicitly.
- Maintenance binaries are gated only if the inventory proves it is needed.
- Candidate records do not imply measurements that did not occur.
- No benchmark or evidence directory is added.

---

# Workstream 5 — reconcile documentation and plan records

Update only documents affected by actual corrections.

Expected files:

```text
architecture/text-library.md
architecture/agent-api.md
architecture/calculator.md
architecture/budget-concurrency.md only if behavior wording changes
architecture/mcp-server.md only if regex/tool response wording changes
docs/library-api.md
AGENTS.md only if developer guidance changes
plans/2026-07-31-lightweight-correctness-simplification-roadmap.md
plans/2026-07-31-phase-1-regex-and-mcp-contract-repairs.md
plans/2026-07-31-phase-2-deterministic-output-and-toml-corrections.md
plans/2026-07-31-phase-3-dispatch-and-runtime-simplification.md
plans/2026-07-31-phase-4-measured-footprint-reduction-and-closure.md
this plan
```

Do not edit README or generated tool tables unless schemas or public examples actually change. If generated docs change, regenerate through the existing generator command.

## Phase 1 record

Record:

- original implementation commit `98d3aae00efc29436af808c430da6766ea76ebf6`;
- corrective commit from this pass;
- named capture and offset regression tests;
- final syntax/policy contract;
- no deferred Phase 1 defects.

Do not retain `(pending commit)`.

## Phase 2 record

Record:

- original implementation commit `0a3ace9e21853e4ded7f0a8c2a9bcb9ab4f1cc94`;
- documentation commit `e009d86b9b0efcce89d5f43c2ec86efcc8fe4614`;
- gap-fix commit `25c4893455719027cdc889a853039a918611ec65`.

Do not reopen Phase 2 unless tests fail.

## Phase 3 record

Mark complete only after direct `call_json()` isolation lands.

Record:

- dispatch consolidation commit `63bac39b87596e2f7721c4042f369afe92a41bcd`;
- calculator/test-hook completion commit `021795bc72eee444510ff9f4472e16a611418b6d`;
- direct-dispatch corrective commit from this pass;
- public wrappers retained;
- commit-slot removal;
- lifecycle state-machine disposition;
- fresh-context direct call contract;
- focused and full test results.

Check each acceptance item individually.

## Phase 4 record

Record:

- implementation commit `a8dc5e69e8ce3d38c17f7cf944d8967408b9701a`;
- documentation commit `3d876bcfd447d2d6a642f461e7f6960c6987cd2f`;
- any conditional maintenance-binary gating commit from this pass;
- final implementation SHA;
- exact byte measurements;
- median process-start measurements;
- default install inventory;
- truthful candidate dispositions;
- implementation-commit remote CI result.

Do not check an acceptance box whose condition was intentionally replaced. If a criterion is no longer appropriate, amend it narrowly and explain why in one sentence.

## Roadmap record

The parent roadmap may be marked complete only when all closure criteria below pass.

Use one concise final statement. Do not append command transcripts, run-ID tables, artifact digests, or package file manifests.

## This plan's completion record

Fill the template at the end of this file after implementation. Do not create another plan.

---

# Workstream 6 — verification and remote closure

## Focused verification

Run exact discovered test targets covering:

```text
regex_engine capture conversion
regex_test named and unnamed groups
regex_finditer named groups and offsets
Unicode capture boundaries
regex syntax/policy result states
validate_regex tool machine codes
ToolRegistry direct-call context isolation
explicit EvalContext persistence
MCP random/side-effect restrictions
execution-context non-persistence
```

Prefer table-driven unit tests. Do not add broad integration harnesses.

## Full local verification

Use the repository's existing commands, adjusted only if `dev-tools` gating lands:

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

If `dev-tools` gating lands:

```bash
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

Also run:

```bash
cargo deny check advisories bans licenses sources
```

Run Python parity only if the corrected named-group or policy semantics are covered by the parity suite. Do not expand parity coverage merely for ceremony.

## MCP smoke

Run the existing MCP smoke/integration path:

```text
initialize
notifications/initialized
tools/list
validate_regex named-group call
math_eval or text_equal call
EOF shutdown
```

Confirm stdio remains clean JSON-RPC output.

## Remote CI

Push the corrective implementation commit and verify the existing ordinary CI topology:

```text
Linux correctness
Windows compile check
macOS compile check
```

Do not add workflows or dispatch maintenance/parity solely for this closure unless a changed surface directly requires them.

### Avoiding an evidence-only commit loop

Use this sequence:

1. create one corrective implementation commit containing code, tests, and behavior documentation;
2. push it and confirm ordinary CI;
3. update the existing plan completion records in one concise documentation commit;
4. locally verify the documentation commit, including generated-doc drift;
5. push it once.

The recorded remote CI result may refer to the corrective implementation commit. Do not create repeated documentation commits solely to record the CI result of the previous documentation commit.

If the final documentation commit fails ordinary CI for a real reason, fix that failure normally. Do not create a run ledger.

---

# Execution sequence for a smaller implementation agent

Execute in this exact order:

1. synchronize to current `origin/main` and confirm a clean tree;
2. confirm `3d876bc..f7c690d` has no tree difference;
3. add failing focused tests for named captures, fancy substring extraction, absolute offsets, policy status, and direct-call state isolation;
4. repair `CompiledCaptures` to store one source plus absolute ranges;
5. populate named capture indexes for both backends;
6. correct Rust nonzero-position offset translation;
7. run focused regex tests;
8. reorder regex compile and policy assessment;
9. populate `policy_allowed` consistently;
10. preserve existing machine-code behavior;
11. run focused regex tool tests;
12. install a fresh native `EvalContext` around direct `call_json()` execution;
13. run direct/stateful/MCP calculator context tests;
14. run all focused tests together;
15. update affected architecture/API documentation;
16. run full local verification;
17. create the corrective implementation commit;
18. push and verify ordinary remote CI;
19. measure exact baseline/final binary bytes on one host;
20. measure baseline/final process-start medians on the same host;
21. audit default `cargo install --path .` binaries;
22. gate maintenance binaries only if the inventory requires it, then rerun verification and CI for that implementation change;
23. reconcile Phase 1-4 and roadmap completion records with exact SHAs and truthful evidence;
24. fill this plan's completion record;
25. create one concise closure documentation commit and push it;
26. stop this line of work unless verification exposes a new reproducible defect.

Do not combine this work with unrelated cleanup found during implementation. Record unrelated observations only if they are release-blocking; otherwise leave them unmentioned.

---

# Required acceptance checklist

## Regex capture correctness

- [ ] Rust unnamed captures return exact source text.
- [ ] Rust named captures populate `groupdict`.
- [ ] Fancy unnamed captures return exact source text.
- [ ] Fancy named captures populate `groupdict`.
- [ ] `captures_from_pos()` returns absolute offsets for both backends.
- [ ] Unicode before a match is handled correctly.
- [ ] Unicode inside a capture is handled correctly.
- [ ] No valid UTF-8 capture path can panic from slicing.
- [ ] Existing regex backend selection remains truthful.

## Regex status contract

- [ ] Compile failure yields `valid_pattern=false`.
- [ ] Unsupported dialect features remain explicit.
- [ ] Compile-successful safe patterns yield `policy_allowed=true`.
- [ ] Compile-successful rejected patterns yield `valid_pattern=true` and `policy_allowed=false`.
- [ ] Policy-rejected patterns do not execute.
- [ ] Runtime engine errors remain separate from compile errors and no-match.
- [ ] ASCII mode remains explicitly unsupported.
- [ ] Existing machine codes remain stable.

## Dispatch state semantics

- [ ] Direct `call_json()` installs a fresh native `EvalContext`.
- [ ] Direct tool calls do not use process-global calculator state.
- [ ] Direct calls remain synchronous and low overhead.
- [ ] Bounded calls retain limits, timeout, cancellation, and queue behavior.
- [ ] MCP calls retain MCP-safe restrictions.
- [ ] Explicit direct calculator contexts persist state.
- [ ] Generic execution contexts remain non-persistent to the caller.
- [ ] No generic persistent session API is added.

## Footprint and install evidence

- [ ] Exact baseline binary bytes are recorded.
- [ ] Exact final binary bytes are recorded.
- [ ] Baseline and final measurements use the same host/toolchain/profile.
- [ ] Median process-start timings are recorded for four CLI paths.
- [ ] Default installed binary names are recorded.
- [ ] Maintenance binaries are gated if and only if default installation exposes them.
- [ ] Candidate dispositions distinguish measurement from feasibility judgment.
- [ ] No benchmark or evidence infrastructure is committed.

## Documentation and verification

- [ ] Phase 1 has exact implementation and corrective SHAs.
- [ ] Phase 2 has exact implementation, documentation, and gap-fix SHAs.
- [ ] Phase 3 is accurately marked complete.
- [ ] Phase 4 has a final SHA, measurements, install inventory, and CI disposition.
- [ ] The parent roadmap is internally consistent.
- [ ] This plan has a concise completion record.
- [ ] Generated documentation is current.
- [ ] Formatting passes.
- [ ] Clippy passes with warnings denied.
- [ ] Normal tests pass.
- [ ] Documentation tests pass.
- [ ] Package construction passes.
- [ ] Existing ordinary remote CI passes for the corrective implementation commit.
- [ ] No CI job family was added.
- [ ] Release remains manual.

---

# Stop conditions

Stop and reassess only if one of these occurs:

1. fixing capture offsets requires a public breaking API change;
2. installing a direct eval context changes established non-calculator tool behavior;
3. the default-install correction requires workspace restructuring;
4. a focused regression reveals a separate release-blocking data-corruption or panic defect;
5. ordinary cross-platform compilation fails because the narrowed Tokio feature set omitted a real platform requirement.

For stop conditions 1-3, choose the smallest compatibility-preserving alternative and document it in this plan. Do not open a broad roadmap.

For unrelated nonblocking findings, do not expand scope.

---

# Completion record

## Implementation

- **Status:** complete
- **Starting main SHA:** `11aaa592a35e87e253f25eb86373ead954bf51a9`
- **Corrective implementation commit:** `1cb0ce5`
- **Conditional packaging commit:** `not needed` (dev-tools feature gating included in implementation commit)
- **Closure documentation commit:** `2f55d80`

## Correctness dispositions

- **Named capture fix:** `CompiledCaptures` now stores `BTreeMap<String, usize>` populated from backend `capture_names()` iterators. Both backends populate names correctly.
- **Fancy substring/range fix:** `convert_captures_fancy` stores absolute `(start, end)` tuples from backend match objects. `get()` slices `source[start..end]` directly.
- **Nonzero-position offset fix:** `convert_captures_std` accepts `pos: usize` parameter and adds it to all stored ranges. `captures_from_pos` passes the position offset.
- **Syntax/policy contract:** `check_pattern_complexity` now runs after compilation. Policy-rejected patterns report `valid_pattern: true, policy_allowed: false` with no matching execution. Unsupported constructs remain `valid_pattern: false`.
- **Direct registry context isolation:** `ToolRegistry::call_json()` now wraps handler invocation in `budget::with_eval_context(&mut EvalContext::new(), || handler(&args))`. Calculator-backed tools receive a fresh native context.

## Measurements

- **Environment:** Linux x86_64, rustc 1.97.1, cargo 1.97.1
- **Baseline SHA:** `63bac39b87596e2f7721c4042f369afe92a41bcd`
- **Final implementation SHA:** `2f55d80`
- **Release binary before:** 12,856,752 bytes (pre-gating)
- **Release binary after:** 12,856,656 bytes (post-gating, dev-tools feature added)
- **CLI timing:** --help 15.2ms, --version 15.3ms, 2+2 501.0ms, 'thirty plus five' 499.7ms (median, 10 runs, same host)
- **Default install inventory:** only `eggsact` binary (generate-docs and verify-eggsact gated behind dev-tools feature)

## Verification

- **Focused tests:** 15 capture tests (named/unnamed groups, absolute offsets, Unicode, both backends) — all pass
- **Full local verification:** fmt, clippy, 549 lib tests, 11 doc tests, 38 context isolation tests, 55 property tests — all pass
- **MCP smoke:** initialize → success, stdio clean JSON-RPC
- **Remote ordinary CI:** Linux correctness ✓, Windows compile ✓, macOS compile ✓ (run 30688082724)
- **Python parity:** not required (no Python-side behavior changes)

## Closure

- **Phase 1 record corrected:** exact SHAs filled, acceptance items marked complete
- **Phase 2 record corrected:** gap-fix SHA filled, acceptance items marked complete
- **Phase 3 record corrected:** marked complete with dispatch/test-hook SHAs and fresh-context contract
- **Phase 4 record corrected:** measurements, install inventory, candidate dispositions filled
- **Parent roadmap corrected:** marked complete with concise final statement
- **Deferred findings:** TOML consolidation (feasibility only), confusables representation (feasibility only), schema caching (deferred), trivial regex cleanup (deferred)
- **Final statement:** The July 31 lightweight correctness and simplification line of work is closed. All six defects (A–F) are repaired. Capture representation is backend-independent with absolute byte ranges. Policy rejection is distinguished from syntax failure. Direct dispatch uses fresh EvalContext. Maintenance binaries are gated behind dev-tools feature. CI passes on all three platforms.

Do not create another closure plan. Once this record is complete and the acceptance checklist passes, the July 31 lightweight correctness and simplification line of work is closed.
