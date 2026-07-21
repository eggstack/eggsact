# Final Runtime, Verification, and Fuzzing Closure Plan

## Status

Handoff plan for closing the remaining correctness and evidence gaps after implementation of the Release 4 verification infrastructure and Release 5 fuzz/property scaffolding.

Baseline reviewed commit:

```text
2ebe646db728b9381c50209ddd85e99e79aac657
```

This plan supersedes closure claims in status notes where those claims conflict with the current implementation. It does not supersede the original roadmap or the detailed Release 1–5 plans; it defines the final corrective work required before those releases may be marked complete.

## Purpose

The repository now has strong verification and fuzzing foundations, but the work landed out of dependency order. Release 4 and Release 5 were implemented before the final Releases 1–3 correctness prerequisite was closed. Several workflow and test-evidence defects also make current closure claims stronger than the evidence supports.

This pass has one objective:

> Produce one commit on `main` where the runtime guarantees are correct, Release 4 verification is internally reliable, Release 5 fuzz/property coverage is substantive and executable, documentation matches the package state, and closure evidence is tied to actual successful runs.

No new roadmap phase should begin until this plan is complete.

---

# 1. Current blocking findings

## 1.1 Runtime correctness remains open

The following defects remain in production paths:

1. `timed_out_handlers` can leak when a request times out while queued for the MCP worker semaphore.
2. `timed_out_handlers` can transiently underflow when timeout accounting races handler completion.
3. Synchronous `ToolRegistry` calls directly invoke potentially expensive handlers without a bounded execution boundary.
4. `RequestGuard::drop` uses `try_lock`; cleanup can be skipped permanently under contention.
5. Request-form `notifications/initialized` messages can be consumed without a JSON-RPC response.
6. Controlled tests for queued timeouts, timeout/finish races, cleanup contention, and non-empty capability retention are missing.
7. Package version, changelog sections, deprecation metadata, and release status notes disagree.

## 1.2 Release 4 verification is not yet authoritative

The infrastructure exists, but these gaps remain:

1. `latest-compatible.yml` summary expressions reference step IDs that do not exist.
2. The parity workflow says `pinned eggcalc` but installs an unpinned latest package.
3. Package-content assertions use a blanket dotfile rejection rather than explicit excluded paths.
4. `release.sh`, documentation, and the manual release-verification workflow do not share one exact canonical command sequence.
5. Third-party GitHub Actions are referenced by moving tags rather than immutable reviewed commit SHAs.
6. The selected MSRV is declared, but the repository lacks a durable evidence note describing how the minimum was established.
7. Current GitHub status evidence is not recorded in-repository for the candidate closure commit.

## 1.3 Release 5 closure is overstated

The fuzz workspace is useful, but the following gaps remain:

1. The extended fuzz workflow schedules twelve sequential 300-second campaigns inside a 60-minute job, leaving no time for setup or compilation.
2. Several so-called property tests use fixed examples rather than generated domains.
3. Some tests invoke functions without asserting the documented property.
4. Some fuzz target comments claim invariants that the target does not actually check.
5. The Release 5 status note has `Commit: (pending)` and claims complete evidence without workflow run identifiers.
6. The integration suite is described as hanging locally while simultaneously being marked passed without attached CI evidence.
7. No explicit matrix or artifact index records per-target fuzz outcomes and toolchain versions.

---

# 2. Scope

This closure pass includes only:

- timeout lifecycle and metric correctness;
- guaranteed active-request cleanup;
- bounded synchronous in-process dispatch;
- JSON-RPC lifecycle edge correctness;
- focused runtime regression tests;
- package/version/changelog reconciliation;
- Release 4 workflow correctness and reproducibility;
- Release 5 workflow executability and property-test quality;
- accurate status/evidence documentation.

## Non-goals

This pass must not:

- add new tools;
- add new MCP protocol revisions;
- implement roots, sampling, elicitation, or server-initiated requests;
- add benchmarks or begin Release 6;
- add Cargo feature decomposition or begin Release 7;
- redesign the entire runtime;
- replace Tokio;
- introduce a second async runtime;
- restore unbounded detached worker creation;
- publish to crates.io from GitHub Actions;
- add CI-held crates.io credentials;
- broaden public API stability promises beyond what the code already supports;
- declare Release 4 or Release 5 closed based only on local statements without reproducible evidence.

---

# 3. Required sequencing

The implementation must follow this order.

## Gate A — Runtime correctness

Complete Workstreams 1–5. Do not edit Release 4 or Release 5 status notes to say `closed` before Gate A passes.

## Gate B — Verification fidelity

Complete Workstream 6 after Gate A is green. Release 4 may only be marked complete after the corrected workflows run successfully against the Gate A commit or a descendant containing no runtime regressions.

## Gate C — Fuzz/property closure

Complete Workstream 7 after Gate A. Release 5 may only be marked complete after the corrected fuzz workflows and ordinary property tests run successfully against the final candidate.

## Gate D — Evidence and documentation

Complete Workstreams 8–10 last. Evidence must identify the exact commit SHA and workflow run IDs or locally generated command records.

Later gates must not compensate for an earlier failed gate. Fuzzing and broad CI do not prove timeout accounting, cleanup, or lifecycle correctness unless the targeted tests required below pass.

---

# 4. Workstream 1 — Exact timeout lifecycle accounting

## Affected files

Primary:

- `src/mcp/server.rs`
- `src/mcp/runtime.rs`
- `src/mcp/budget.rs`

Tests:

- `tests/mcp/test_execution_safety.rs`
- focused inline unit tests in `src/mcp/runtime.rs` or a dedicated internal lifecycle module
- `architecture/mcp-server.md`
- `architecture/testing.md`

## Required state semantics

The lifecycle must distinguish waiting for capacity from executing work.

At minimum represent:

```rust
Queued
Running
TimeoutAccounting
TimedOutRunning
Finished
TimedOutQueued
```

An atomic integer representation is acceptable, but define an internal enum or named constants and document every legal transition.

## Required behavior

### Queued request

- Initial state is `Queued`.
- Waiting for the semaphore does not increment `active_blocking_handlers`.
- A response deadline that fires while queued increments `total_timeouts` only.
- It does not increment `timed_out_handlers`.
- The queued future must be dropped so it cannot later acquire a permit and start work.

### Starting work

- After obtaining a permit, atomically transition `Queued -> Running` before spawning the blocking closure.
- If timeout or cancellation already won, release the permit and do not spawn.
- Move the owned permit into the blocking closure.
- Increment `active_blocking_handlers` only once the closure begins.

### Timeout while running

Do not publish a state that authorizes a decrement before the metric increment is complete.

Preferred pattern:

1. `Running -> TimeoutAccounting` by compare-exchange.
2. Increment `timed_out_handlers`.
3. Publish `TimedOutRunning`.
4. Handler completion observing `TimeoutAccounting` waits only for that bounded transition to complete.
5. Handler completion observing `TimedOutRunning` transitions to `Finished` and decrements exactly once.

An RAII token with single ownership is also acceptable if it makes increment/decrement pairing mechanically exact.

### Completion

- Normal completion, cancellation, and panic share one completion guard.
- Completion before timeout performs `Running -> Finished` and never touches `timed_out_handlers`.
- Completion after an accounted timeout decrements once.
- No code path performs `fetch_sub` unless it owns an established increment.
- No stable snapshot may show `timed_out_handlers > active_blocking_handlers`.

## Metric definitions

Document exactly:

- `active_requests`: registered request tasks still eligible for cancellation and duplicate-ID checks.
- `active_blocking_handlers`: blocking closures that have actually begun running.
- `timed_out_handlers`: running blocking closures whose client timeout response has already been returned.
- `total_timeouts`: cumulative timeout responses, including queue-wait timeouts.
- `peak_blocking_concurrency`: maximum simultaneous executing blocking closures.

## Required deterministic tests

Use barriers, channels, test-only semaphores, or injected lifecycle hooks. Do not depend on arbitrary sleeps.

1. Timeout while queued behind a fully occupied semaphore.
2. Queued timeout never starts the handler after capacity becomes available.
3. Handler completes before timeout.
4. Timeout occurs while handler runs, then handler completes.
5. Handler completion races the timeout reservation.
6. Handler completion races after timeout metric accounting.
7. Handler panics after timeout.
8. Cancellation causes handler exit after timeout.
9. At least 500 controlled race iterations leave all gauges at zero.
10. Snapshots never observe unsigned underflow or a value larger than the configured worker count.

## Acceptance criteria

- Queued timeout does not increment or leak `timed_out_handlers`.
- Every timed-out-running increment has one matching decrement.
- No decrement can precede its increment.
- All metrics return to zero after controlled workers exit.
- Peak concurrency never exceeds `MAX_TOOL_WORKERS`.

---

# 5. Workstream 2 — Guaranteed active-request cleanup

## Affected files

- `src/mcp/runtime.rs`
- `src/mcp/server.rs`
- `tests/mcp/test_execution_safety.rs`

## Required design

Do not rely on async mutex acquisition from `Drop` as the primary cleanup path.

Preferred design:

1. Keep registration atomic under the existing active-request mutex.
2. Add an explicit async cleanup operation:

```rust
async fn remove_active_request(
    active: &ActiveRequests,
    request_id: &Value,
    generation: RequestGeneration,
);
```

3. Call it in a guaranteed task-finalization path after request handling, including response serialization failure and sender failure.
4. Keep an RAII guard only as a fallback signal, not as the sole authoritative cleanup mechanism.
5. Preserve generation matching so an old task cannot remove a newer reused ID.

Alternative acceptable design:

- Replace the Tokio mutex map with a data structure that supports synchronous, generation-safe removal from `Drop`, provided contention and poisoning behavior are explicitly tested.

## Required tests

1. Hold the active-request lock while request work finishes; release it and verify awaited cleanup removes the entry.
2. Reuse an ID immediately after completion and verify it succeeds.
3. Repeated contention does not grow the map.
4. Panic path removes the entry.
5. Timeout response followed by eventual worker completion removes the entry at the correct task lifecycle point.
6. Shutdown drains all registered entries.
7. A stale generation cannot remove a newer request using the same ID.

## Acceptance criteria

- Cleanup is guaranteed, not best effort.
- No comment claims that an unspecified later operation cleans stale entries.
- Repeated completed requests cannot exhaust `MAX_IN_FLIGHT_REQUESTS`.
- Duplicate detection remains atomic.

---

# 6. Workstream 3 — Bounded synchronous in-process dispatch

## Affected files

- `src/agent/mod.rs`
- `src/mcp/budget.rs`
- potentially a new `src/agent/executor.rs` or `src/runtime/sync_executor.rs`
- `src/tools/math.rs`
- regex/config handlers only as needed to identify execution class
- `architecture/agent-api.md`
- `architecture/tools.md`
- `architecture/testing.md`

## Contract decision

The in-process API is first-class and must not become less safe than MCP dispatch.

Define two explicit surfaces:

1. **Raw direct execution** for callers that intentionally accept same-thread execution.
2. **Bounded registry execution** as the recommended default for potentially expensive handlers.

Do not silently change a public method’s blocking semantics without documenting the compatibility impact.

## Preferred implementation

Add a registry-owned or process-wide bounded synchronous executor.

Required properties:

- fixed maximum worker count;
- owned permit remains with worker until actual exit;
- bounded queue or bounded acquisition wait;
- caller-facing elapsed timeout may return before worker exit;
- repeated timed-out calls cannot create more live workers than the configured maximum;
- cancellation context and evaluator-context snapshot propagate into the worker;
- structured `TIMEOUT` or `RESOURCE_EXHAUSTED` response on deadline/capacity failure;
- no nested use when already running inside the MCP bounded blocking closure.

Suggested API shape:

```rust
pub enum ExecutionMode {
    Direct,
    Bounded,
}

pub fn call_json_bounded(
    &self,
    name: &str,
    args: Value,
    budget: Option<ToolBudget>,
) -> Result<ToolResponse, ToolCallError>;
```

A builder-level default execution mode is acceptable if backward compatibility is explicit.

## Handler classification

At minimum route these through bounded execution for in-process callers:

- `math_eval`;
- `validate_regex`;
- `regex_finditer`;
- `dotenv_validate` when custom patterns are accepted;
- any composite tool that invokes those surfaces.

Prefer metadata-driven classification from `ToolSpec.cost` or an explicit execution class over hard-coded name lists.

## Required tests

1. Saturate the sync executor and verify additional work receives deterministic capacity behavior.
2. A timed-out worker retains its permit until it exits.
3. Repeated timeouts do not exceed the worker limit.
4. Direct mode remains direct and is clearly documented.
5. Bounded mode preserves profile, audience, compatibility, cancellation, and evaluator-context behavior.
6. Panic is converted into the documented structured failure.
7. Output truncation still occurs after bounded execution.

## Acceptance criteria

- Recommended in-process execution is bounded.
- No per-call unbounded thread creation exists.
- MCP dispatch does not nest the synchronous executor.
- Public documentation distinguishes direct and bounded semantics.

---

# 7. Workstream 4 — JSON-RPC lifecycle edge correctness

## Affected files

- `src/mcp/server.rs`
- `src/mcp/protocol.rs`
- `tests/mcp/test_protocol.rs`
- `tests/mcp/test_lifecycle_and_gaps.rs`
- `architecture/mcp-server.md`

## Required behavior

Differentiate notification form from request form using the presence of an ID.

### True notification

```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

- valid state: transition to `Ready`, no response;
- wrong state: ignore or log according to policy, no response.

### Request-form misuse

```json
{"jsonrpc":"2.0","method":"notifications/initialized","id":7}
```

- must return a deterministic JSON-RPC error;
- recommended code: `-32600 Invalid Request`;
- must not silently consume the request;
- must not transition state unless the documented policy explicitly permits it, with tests.

### Capability retention

Add a lifecycle test with non-empty values for:

- roots support;
- sampling support;
- elicitation support if represented;
- experimental object values;
- client implementation name and version.

Assert values survive `Uninitialized -> AwaitingInitialized -> Ready` unchanged.

## Acceptance criteria

- Notifications remain response-free.
- Request-form notification misuse receives exactly one response.
- Capability retention is tested with non-default data.

---

# 8. Workstream 5 — Version, changelog, and documentation reconciliation

## Affected files

- `Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
- `README.md`
- architecture documents mentioning removed timeout helpers
- deprecation attributes in `src/agent/mod.rs` and calculator/global-mode APIs
- `docs/release-readiness.md`
- `docs/release-5-status.md`

## Required decisions

Before editing versions, verify the latest actually published crates.io release manually or through the release process available to maintainers.

Then choose one consistent state:

### If `1.2.0` is the current package candidate and `1.3.0` is unpublished

- move `1.3.0` entries back under `Unreleased` or set the manifest to the intended next version;
- remove a dated release heading that was never published;
- set deprecation `since` values to the first actual crate release containing them.

### If `1.3.0` was published

- update `Cargo.toml` and `Cargo.lock` to the next valid version for new changes;
- preserve historical `1.3.0` entries;
- document the published tag/commit;
- correct any deprecation version that predates introduction.

## Documentation cleanup

- Remove stale references to deleted `run_with_timeout`, spawn semaphores, and old constants.
- Document the final runtime execution model.
- Mark Release 5 status as `implementation complete, evidence pending` until actual workflow evidence exists.
- Replace `Commit: (pending)` with the final closure commit only after that commit exists.
- Do not state that a command passed if it was not run successfully.

## Acceptance criteria

- Manifest, lockfile, changelog, tags, and status notes describe one coherent version history.
- Every `#[deprecated(since = ...)]` value corresponds to an actual release containing the deprecation.
- No architecture document describes removed infrastructure as current.

---

# 9. Workstream 6 — Release 4 verification corrections

## 9.1 Fix latest-compatible workflow reporting

File:

- `.github/workflows/latest-compatible.yml`

Add explicit IDs:

```yaml
- name: Check
  id: check
  run: cargo check --all-targets --all-features
```

Do the same for library, binary, and integration tests. The summary must reference valid IDs.

Also record:

- updated `Cargo.lock` diff as an artifact or step summary;
- `cargo metadata --format-version 1` checksum or dependency list;
- exact Rust version.

The workflow remains scheduled/manual and non-blocking for ordinary PRs unless project policy changes.

## 9.2 Define parity policy precisely

File:

- `.github/workflows/parity.yml`

Choose one of:

### Preferred dual-mode policy

- pinned compatibility job: install an explicit version such as `eggcalc==X.Y.Z` from a repository variable or checked-in configuration;
- latest-reference drift job: install latest `eggcalc`, clearly named as such.

### Minimum acceptable policy

- rename the job from `pinned` to `latest`;
- record the resolved version;
- document that results are drift detection, not reproducible baseline evidence.

The parity report must include:

- eggsact commit SHA;
- eggsact package version;
- requested eggcalc version policy;
- resolved eggcalc version;
- Python version;
- test command;
- success/failure outcome;
- accepted parity failure fixture checksum.

## 9.3 Make package assertions explicit

File:

- `.github/workflows/release-verification.yml`

Replace blanket `^\.` rejection with explicit forbidden prefixes:

- `.github/`
- `.opencode/`
- `.agents/`
- `plans/`
- `scripts/`
- `fuzz/`
- `deny.toml`
- `AGENTS.md`
- local build artifacts

Assert required files individually. Save and upload `package-list.txt` with provenance.

## 9.4 Unify the canonical release gate

Files:

- `release.sh`
- `.github/workflows/release-verification.yml`
- `docs/release.md`
- `.opencode/skills/release/SKILL.md`
- `AGENTS.md`

Define one canonical ordered command list and keep all surfaces aligned:

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

If confusables regeneration is part of release preparation, separate mutation from verification:

```bash
python3 scripts/generate_confusables.py

git diff --exit-code -- src/text/confusables_generated.rs
```

Do not let the release gate silently modify tracked files and continue.

## 9.5 Pin third-party actions

Pin every non-GitHub-maintained action to a reviewed full commit SHA where practical, including:

- Rust toolchain action;
- Rust cache action;
- cargo-deny action.

Keep a comment with the human-readable release tag. GitHub-maintained actions may follow the project’s chosen policy, but apply it consistently.

## 9.6 Record MSRV evidence

Add `docs/msrv.md` or a section in `docs/compatibility-policy.md` recording:

- candidate versions tested;
- first passing version;
- dependency or language feature setting the floor;
- exact command set;
- date and commit SHA;
- policy for raising MSRV.

## Release 4 acceptance criteria

- All workflow YAML parses.
- Latest-compatible summaries show real outcomes.
- Parity naming and installation semantics agree.
- Package assertions pass on the real package listing.
- Local and GitHub release gates use the same commands.
- Third-party actions follow the documented pinning policy.
- MSRV evidence is durable.
- A successful manual Release Verification run exists for the final candidate.

---

# 10. Workstream 7 — Release 5 fuzz and property closure

## 10.1 Convert extended fuzzing to a matrix

File:

- `.github/workflows/fuzz-scheduled.yml`

Use one target per matrix job or small bounded groups. Preferred:

```yaml
strategy:
  fail-fast: false
  matrix:
    target:
      - calculator_expression
      - calculator_normalization
      - unified_diff
      - shell_tokenization
      - shell_quoting
      - regex_classification
      - regex_execution
      - json_pointer
      - toml_config
      - unicode_inspection
      - markdown_fences
      - glob_matching
```

Each job should:

- build or reuse a correctly keyed cache;
- run one target for the configured duration;
- record target, commit SHA, nightly version, cargo-fuzz version, sanitizer, duration, corpus count, and exit outcome;
- upload only that target’s crash artifacts and summary metadata;
- fit comfortably inside its timeout including setup.

Run sanitizer coverage as a separate matrix or a smaller explicitly justified high-value target set.

## 10.2 Correct smoke workflow scope

File:

- `.github/workflows/fuzz-pr.yml`

- Keep PR runtime bounded.
- Build all targets.
- Run a documented high-value subset.
- Use short per-target budgets.
- Pin nightly by date or record exact resolved nightly in the artifact.
- Add concurrency cancellation for superseded PR runs.

## 10.3 Replace vacuous tests

Files:

- `tests/property/test_calculator_properties.rs`
- `tests/property/test_diff_properties.rs`
- `tests/property/test_shell_properties.rs`
- `tests/property/test_regex_properties.rs`
- `tests/property/test_json_properties.rs`
- `tests/property/test_config_properties.rs`
- `tests/property/test_unicode_properties.rs`
- `tests/property/test_markdown_properties.rs`
- `tests/property/test_path_glob_properties.rs`

Every test named `deterministic`, `roundtrip`, `idempotent`, `symmetric`, `transaction`, `bounded`, or `span-valid` must assert that property.

Remove or rewrite tests that only call a function and discard the result.

## 10.4 Add substantive generated domains

An external property framework is optional. A deterministic custom generator is acceptable if it explores a real domain and shrinks are not essential. However, fixed arrays alone must be described as example regression tests, not property tests.

Required generated properties:

### Calculator

- checked integer addition and multiplication agreement over safe ranges;
- whitespace invariance where grammar permits;
- normalization idempotence across generated valid expressions;
- failed parsing leaves context state unchanged;
- same seed and input produce identical result and resulting context;
- separate contexts do not leak state.

### Unified diff

- generate small source and target line vectors;
- derive or construct a valid patch;
- apply result equals target;
- generated hunk ranges are consistent;
- reverse operation restores source where supported;
- malformed inputs never panic and remain bounded.

### Shell

- generated argv vectors satisfy `split(quote(argv)) == argv` for supported POSIX domain;
- Windows quoting receives a separate platform/domain contract;
- deterministic parse and quote results are actually compared.

### Regex

- classification determinism compares full relevant outputs;
- repeated execution compares match spans and errors;
- all spans are valid for the documented byte/codepoint contract;
- max-match limit is never exceeded.

### JSON

- canonicalization is idempotent for generated JSON values;
- comparison symmetry is asserted only for symmetric option sets;
- JSON-pointer extraction agrees with a small reference implementation for generated trees and valid pointers.

### Config

- generated TOML tables parse deterministically;
- duplicate-policy behavior is explicit;
- dotenv and INI result bounds hold;
- malformed generated inputs never panic.

### Unicode

- NFC/NFKC idempotence;
- casefold output remains valid UTF-8;
- reported spans align with the documented indexing unit;
- inspection is deterministic.

### Markdown

- generated fenced blocks produce ordered non-overlapping spans;
- extracted contents match generated payloads where syntax is valid;
- unclosed fences produce deterministic bounded behavior.

### Path/glob

- path normalization idempotence for supported path modes;
- generated simple glob patterns agree with a small reference matcher for the supported subset;
- traversal and separator normalization cannot bypass scope checks.

## 10.5 Align fuzz comments with assertions

Review all twelve targets.

For every claim in the module comment:

- either implement the assertion;
- or remove/narrow the claim.

Examples:

- calculator parse-failure transaction claim must snapshot and compare context;
- regex determinism claim must execute twice and compare relevant outputs;
- output-bounded claims must assert concrete limits;
- span-validity claims must use the correct indexing contract.

## 10.6 Corpus and regression policy

- Preserve reviewed seeds.
- Add minimized seeds for any defect found during this pass.
- Do not count `.gitkeep` as a corpus seed.
- Generate corpus counts programmatically in evidence notes.
- Every discovered crash/hang/invariant violation receives a normal deterministic regression test before closure.

## Release 5 acceptance criteria

- `cargo fuzz list` returns all intended targets.
- `cargo fuzz build` succeeds with the committed fuzz lockfile.
- Every scheduled target has an executable per-target job budget.
- PR smoke workflow finishes within its timeout.
- No property test is vacuous.
- Generated-domain tests enforce the principal planned invariants.
- Fuzz comments match actual assertions.
- Per-target run evidence exists for the final candidate.

---

# 11. Workstream 8 — CI and test-hang diagnosis

The status note currently says the full integration command hangs locally but passes in CI. This must be resolved or precisely scoped.

## Required investigation

Run integration tests with per-test visibility:

```bash
cargo test --locked --all-features --tests -- --skip parity --nocapture --test-threads=1
```

If it hangs:

1. identify the exact test process;
2. add explicit child-process timeouts;
3. ensure stdin is closed where EOF is expected;
4. ensure stdout/stderr pipes are drained;
5. ensure child processes are killed and waited on after timeout;
6. avoid relying on GitHub runner behavior to terminate leaked children.

Add a test helper for subprocess lifecycle if multiple tests duplicate this logic.

## Acceptance criteria

- The full non-parity integration suite completes locally in a bounded time.
- No status note dismisses a hang as pre-existing without an issue and explicit exception.
- CI timeout is a last-resort guard, not ordinary process cleanup.

---

# 12. Workstream 9 — Closure evidence

Create a final evidence document only after all implementation work lands.

Suggested path:

```text
docs/releases/2026-07-final-closure-evidence.md
```

It must contain:

- exact commit SHA;
- package version;
- lockfile checksum;
- stable Rust version;
- MSRV version;
- nightly version used for fuzzing;
- cargo-fuzz version;
- eggcalc parity policy and resolved version;
- accepted parity fixture checksum;
- workflow run IDs and links for:
  - ordinary CI;
  - Release Verification;
  - latest-compatible dependencies;
  - parity;
  - fuzz smoke;
  - fuzz extended matrix;
  - sanitizer matrix;
- per-target fuzz duration and result;
- corpus count per target;
- exact local commands run;
- test counts by partition;
- any intentionally deferred item with issue reference and rationale.

Do not write `PASS` without one of:

- a linked workflow run;
- captured command output in an artifact;
- a reproducible local evidence record generated by a checked-in script.

Update `docs/release-5-status.md` to reference this evidence document rather than duplicating mutable claims.

---

# 13. Workstream 10 — Final validation gate

## Runtime-focused gate

```bash
cargo test --locked --test lib mcp -- --nocapture
cargo test --locked --test test_context_isolation -- --nocapture
```

Run any new dedicated lifecycle/metrics test target explicitly.

Required stress loops:

```bash
for i in $(seq 1 20); do
  cargo test --locked --test lib mcp::test_execution_safety -- --test-threads=1
 done
```

Use the actual module filter matching the final test layout.

## Ordinary release gate

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

## MSRV gate

Using the declared exact toolchain:

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

If the final selected MSRV changes, substitute the documented value everywhere.

## Property gate

```bash
cargo test --locked --test lib property -- --nocapture
```

## Fuzz build and smoke gate

```bash
RUSTUP_TOOLCHAIN=nightly cargo fuzz list
RUSTUP_TOOLCHAIN=nightly cargo fuzz build
```

Run every target for a short local smoke budget or rely on the corrected matrix with linked evidence.

## Cross-platform gate

The final candidate must pass:

- Ubuntu full CI;
- Windows non-parity supported suite;
- macOS non-parity supported suite;
- exact MSRV suite;
- cargo-deny;
- package gate.

---

# 14. Suggested implementation commits

Keep the pass reviewable with narrow commits:

1. `fix(runtime): make timeout lifecycle accounting exact`
2. `fix(runtime): guarantee active request cleanup`
3. `feat(agent): add bounded synchronous registry execution`
4. `fix(mcp): reject request-form initialized notifications`
5. `test(runtime): add deterministic timeout and cleanup races`
6. `docs(release): reconcile package and changelog versions`
7. `ci: correct release 4 workflow fidelity`
8. `test(property): replace vacuous checks with generated invariants`
9. `ci(fuzz): matrix scheduled targets and record evidence`
10. `docs(release): add final closure evidence`

Combining closely related commits is acceptable, but do not mix runtime semantics, workflow-only edits, and evidence-note edits into one opaque commit.

---

# 15. Stop conditions

Stop and do not mark closure if any of these occur:

- a queued timeout can later start work;
- `timed_out_handlers` can underflow or remain nonzero after workers finish;
- active-request entries remain after completed tasks;
- bounded in-process calls can exceed the configured worker limit;
- request-form lifecycle misuse receives no response;
- the non-parity integration suite hangs;
- package/changelog versions remain inconsistent;
- parity policy says pinned while installing latest;
- a scheduled fuzz job’s configured work cannot fit inside its timeout;
- property tests retain names that overstate their assertions;
- fuzz comments claim checks not present in code;
- Release 4 or Release 5 status claims lack exact commit/run evidence.

---

# 16. Final closure checklist

## Runtime

- [ ] Queueing and running states are distinct.
- [ ] Queued timeout cannot start work later.
- [ ] Timeout metric accounting cannot underflow.
- [ ] Timed-out-running gauges return to zero.
- [ ] Worker concurrency is a hard bound.
- [ ] Active-request cleanup is guaranteed.
- [ ] Reused request IDs work after completion.
- [ ] Bounded synchronous registry execution exists and is documented.
- [ ] MCP does not nest the sync executor.
- [ ] Request-form `notifications/initialized` receives an error.
- [ ] True notifications remain response-free.
- [ ] Non-empty client capabilities persist through lifecycle transitions.

## Release 4

- [ ] Exact MSRV is tested and documented with evidence.
- [ ] `Cargo.lock` is current and all blocking jobs use `--locked`.
- [ ] cargo-deny passes under the documented policy.
- [ ] Windows and macOS supported suites pass.
- [ ] Latest-compatible workflow reports real outcomes.
- [ ] Parity policy and installation behavior agree.
- [ ] Package assertions are explicit and pass.
- [ ] Release script, workflow, docs, and skill use one command list.
- [ ] Third-party action pinning follows project policy.
- [ ] Manual release-verification run succeeds.

## Release 5

- [ ] All fuzz targets build.
- [ ] Extended fuzzing uses an executable matrix or bounded grouping.
- [ ] Sanitizer jobs fit within configured timeouts.
- [ ] PR smoke fuzzing is bounded and cancellable.
- [ ] Every property-named test asserts the stated property.
- [ ] Principal domains use generated inputs, not fixed examples only.
- [ ] Fuzz comments match implemented assertions.
- [ ] Corpus counts exclude placeholders.
- [ ] Discovered failures become minimized regression tests.
- [ ] Per-target run evidence is recorded.

## Release state

- [ ] Manifest, lockfile, changelog, tags, and deprecations agree.
- [ ] Architecture docs describe current runtime behavior.
- [ ] Full integration suite completes locally and in CI.
- [ ] Final evidence document identifies exact commit and workflow runs.
- [ ] Release 4 status is marked complete only after evidence exists.
- [ ] Release 5 status is marked complete only after evidence exists.
- [ ] No unresolved item is hidden behind a blanket `PASS` statement.

---

# 17. Definition of done

This closure pass is complete only when all of the following are true on the same final candidate commit:

1. Runtime timeout, cleanup, bounded execution, and lifecycle behavior satisfy their focused deterministic tests.
2. Ordinary locked CI, MSRV, Windows, macOS, cargo-deny, packaging, and publish dry-run gates pass.
3. Latest-compatible and parity workflows have truthful, reproducible semantics.
4. Every fuzz target builds and the scheduled matrix completes within configured limits.
5. Property tests enforce real generated-domain invariants rather than merely exercising functions.
6. Version and deprecation metadata match actual release history.
7. The evidence document names the exact commit and successful workflow runs.

Only after that point should Releases 1–5 be considered closed and roadmap work proceed to Release 6 performance characterization.