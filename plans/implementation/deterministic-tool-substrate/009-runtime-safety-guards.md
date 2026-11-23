# Deterministic Tool Substrate Milestone 009 — Runtime Safety Guards

Status: implemented — local full merge gate green; closure record pending

Repository baseline: `main` at `b4ec154` plus the uncommitted audit dependency refresh

Source roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7`
- `plans/subsystems/deterministic-tool-substrate-roadmap.md#10`

Long-term requirements:

- `plans/000-long-term-specification.md#4` (one crate, typed-first, determinism, bounds)
- `plans/000-long-term-specification.md#5` (compatibility requirements)
- `plans/000-long-term-specification.md#7` (invariants 1-3, 5-6, 8)
- `plans/001-terminology-and-domain-model.md#4` (cores, services, adapters)

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

Primary class: invariant

## 1. Objective

Make two implicit runtime-safety postures explicit and enforced:

1. The backtracking regex backend runs with documented, intentionally chosen
   compute and program-size limits instead of relying on crate defaults.
2. The crate denies `unsafe_code` at the package level, with the two existing
   sound uses carrying targeted, justified allow attributes.

No tool surface, schema, registry entry, or engine-selection behavior changes.

## 2. Why this milestone is ready

- The engine construction is a single site (`FancyRegex::new` in
  `src/text/regex_engine.rs`) and the current defaults are known.
- The two `unsafe` blocks are already documented with SAFETY comments and do
  not need redesign.
- No dependency change is required; the limits are exposed by the existing
  `fancy-regex 0.20.2` builder.
- The work is independent of distribution milestones 006-009.

## 3. Current implementation evidence

- `src/text/regex_engine.rs:377` constructs the backtracking backend with
  `FancyRegex::new(&full_pattern)`. No `RegexBuilder` options are set.
- `fancy-regex 0.20.2` defaults to `backtrack_limit: 1_000_000`
  (`fancy-regex-0.20.2/src/lib.rs:682`) and leaves `delegate_size_limit`
  unset, so the delegated `regex` crate's own default program-size limit
  applies.
- Text tools accept up to 100k characters and regex tools accept up to 100
  samples with a 1k pattern cap. Tool budgets and the execution timeout bound
  total work, but the regex backend itself has no eggsact-owned limit.
- `src/text/regex_safety.rs` provides advisory static analysis (nesting,
  adjacent/nested quantifiers). It is not an enforcement layer; it reports
  risk and does not prevent execution of an accepted pattern.
- `unsafe` appears at exactly two production-near sites:
  `src/mcp/budget.rs:134` (thread-local `EvalContext` pointer with a
  re-entrancy guard and SAFETY comment) and `src/mcp/execution.rs:802`
  (test-only `Arc::into_raw` / `Box::leak` metrics pointer with a SAFETY
  comment).
- There is no crate-level `unsafe_code` lint policy; `clippy -D warnings` is
  green today but does not forbid new `unsafe`.

## 4. Invariants that must not regress

- `engine_used` continues to report the backend that actually compiled the
  pattern; simple patterns stay on the linear-time `regex` engine.
- Accepted, non-pathological patterns keep identical results.
- A backend runtime-limit error is reported as an error, never as "no
  match", empty captures, or a silent skip.
- No panic, abort, or partial JSON envelope on limit exhaustion.
- Determinism: the same input and limits produce the same classification
  and error code across runs and platforms.
- No `unsafe` may be added anywhere; existing sites retain their SAFETY
  reasoning.
- No timing thresholds in tests; assert outcomes, not elapsed milliseconds.

## 5. Scope

### In scope

- A single documented construction point for the backtracking backend with
  explicit `backtrack_limit` and `delegate_size_limit`.
- Runtime-error propagation through the validating call sites.
- A package-level `unsafe_code = "deny"` policy with targeted allows.
- Focused tests and documentation of the new boundaries.

### Explicitly out of scope

- Replacing `fancy-regex`, adding a PCRE2 engine, or changing pattern
  classification.
- New regex tools, flags, schemas, limits fields, or machine codes unless an
  existing error path cannot represent a limit failure.
- Rewriting the `unsafe` blocks or the eval-context design.
- Changing tool budgets, timeouts, or text-size limits.
- Performance benchmarking (non-gating and not required).

## 6. Required production changes

### Core/service/adapter

- Route fancy-engine construction through one helper that sets the limits,
  so no call site can bypass them.
- Choose limit values with a recorded rationale. Starting point:
  `backtrack_limit` 250,000 steps and `delegate_size_limit` 4 MiB. Adjust
  only with fixture evidence; do not silently inherit crate defaults.
- Audit every `CompiledRegex` match consumer for `unwrap_or`/`ok()` patterns
  that would collapse a runtime-limit error into a negative result. At
  minimum, `src/text/validate.rs:573`, `src/text/validate.rs:791`, and
  `src/text/validate.rs:1435` must propagate the error into the existing
  tool-error shape.

### Root and build policy

- Add a package-level lint configuration that denies `unsafe_code` for all
  targets (library, binary, tests), for example `[lints.rust]
  unsafe_code = "deny"` in `Cargo.toml`. Use `deny`, not `forbid`, so the two
  reviewed sites can carry `#[allow(unsafe_code)]`.
- Add `#[allow(unsafe_code)]` only at the two existing items, keeping their
  SAFETY comments. Do not allow it at module or function-call granularity
  broader than the unsafe expression.

### Registry, profile, audience, surface

None.

### Protocol and DTOs

None, unless a limit failure needs a new reason string in an existing error
field; prefer reusing the current invalid-pattern/runtime-error shape.

### Runtime and concurrency

No change to worker counts, budgets, cancellation, or context isolation.

### Documentation and static guards

- `architecture/text-library.md`: state the two limits, why they exist, and
  that `regex_safety` is advisory.
- `AGENTS.md`: add the regex-limit and no-new-unsafe gotchas.
- `docs/library-api.md`: only if a public constructor's behavior is
  described.

## 7. Ordered work packages

### Work package A — Explicit regex limits

Intent: bound CPU and compiled-program size at the backend, not only at the
budget layer.

Required changes:

- Introduce the single construction helper with explicit limits.
- Document the chosen values and the rejection behavior.
- Verify `engine_used` and classification are unchanged for all existing
  fixtures.

Acceptance evidence:

- A catastrophic pattern such as `(a+)+$` over a short adversarial input
  returns the documented error instead of consuming the budget.
- A normal lookahead/lookbehind/backreference fixture still matches.
- No test asserts wall-clock time.

### Work package B — Error propagation audit

Intent: prevent a limit failure from becoming a false negative.

Required changes:

- Inspect all `CompiledRegex::{find, captures, captures_from_pos, is_match}`
  consumers for error swallowing.
- Add a regression test at the tool layer asserting an error response for a
  pattern/input pair that exceeds the limit.

Acceptance evidence: the tool response carries an error (not `ok: true` with
no matches), and the machine code is stable.

### Work package C — Unsafe-code policy

Intent: make new `unsafe` a reviewed event.

Required changes:

- Add the package-level deny lint and the two targeted allows.
- Confirm no other `unsafe` exists, including tests and the docs generator.

Acceptance evidence:

- An intentional scratch `unsafe` block fails the build/clippy; remove the
  scratch change and record the check.
- Normal build, clippy, and tests remain green.

### Work package D — Documentation and closure

Record limit values, rationale, error behavior, and the lint policy.

## 8. Failure, cancellation, truncation, and contention semantics

- Limit exceeded at match time: return the backend error through the
  existing validation error path; no partial matches, no truncation marker
  that implies success.
- Limit exceeded at compile time (delegated program size): classify as an
  invalid/unsupported pattern using the current compile-error shape.
- Cancellation and budgets are unchanged; the new limits are a lower bound on
  worst-case work, not a replacement for timeouts.
- Concurrent calls are independent; the helper is stateless.
- Determinism: limits are constants, not environment- or load-dependent.

## 9. Compatibility and migration

- Normal patterns produce identical output.
- Pathological patterns that previously consumed the full budget now fail
  deterministically and earlier; document this as an intentional boundary.
- No schema, registry, profile, or wire change.
- The `unsafe_code` lint is compile-time only and does not affect the public
  API or MSRV (Rust 1.89 supports package lints).

## 10. Required tests

### Focused unit tests

- `compile_regex` still selects `RustRegex` for simple patterns and
  `FancyRegex` for lookaround/backreference patterns.
- A limit-exceeding pattern returns `Err` from the match API.
- A pattern just under the 1k cap compiles (delegate-size limit is not
  accidentally restrictive).
- Compile-time invalid patterns keep their current messages.

### Integration tests

- A tool-level call with the catastrophic pattern returns an error response
  with a stable machine code.
- Existing regex tool fixtures remain green.

### Restart and recovery tests

Not applicable; no persistent state.

### Contention and cancellation tests

- Existing budget/cancellation suites remain green; no new shared state.

### Negative and boundary tests

- Empty pattern, maximum-length pattern, nested-quantifier pattern, and a
  pattern that is valid only on the fancy engine.
- A caller that previously used `ok()`/`unwrap_or` on a match result must now
  surface the error (regression test for the specific site).

### Parity and compatibility tests

Parity is unaffected (no tool semantics change for accepted inputs). Record
that parity was skipped if it was not run.

## 11. Required verification commands

```bash
cargo test --locked --lib text::regex_engine -- --test-threads=4
cargo test --locked --test lib regex -- --test-threads=4
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

No generated-doc change is expected; run `generate-docs -- --check` only if a
ToolSpec or schema is touched.

## 12. Documentation updates

- `architecture/text-library.md` — limits, rationale, advisory-only
  `regex_safety`, error behavior.
- `AGENTS.md` — regex default limits and the no-new-unsafe rule.
- `docs/library-api.md` — only if a public constructor contract changes.

## 13. Acceptance criteria

- Fancy-engine construction always applies explicit, documented limits; no
  call site can bypass the helper.
- Limit failures surface as errors at the tool layer, never as no-match.
- `engine_used`, pattern classification, and accepted-input results are
  unchanged.
- The package denies `unsafe_code`; only the two documented sites are
  allowed; a new `unsafe` fails the build.
- Full merge gate is green.

## 14. Stop conditions

Stop and report when:

- the builder cannot express limits for the compiled engine variant used;
- the chosen limits reject fixtures that are legitimately supported, and no
  documented value satisfies both safety and compatibility;
- a limit failure cannot be represented without a schema or machine-code
  change (that becomes a separate plan);
- the lint policy requires touching an unrelated crate root or generated
  file;
- scope expands into engine replacement or performance work.

## 15. Closure evidence required

- Construction-site diff with the chosen limit values and rationale.
- Catastrophic-pattern error output at the engine and tool layers.
- Normal-pattern regression evidence.
- Scratch-unsafe rejection evidence and the two allow attributes.
- Full merge-gate output.
- Documentation links.

## 16. Handoff notes

- Do not change budgets or timeouts to compensate for the new limits; they
  are complementary.
- Keep `regex_safety` advisory; it must not be repurposed as an allow/deny
  gate in this milestone.
- Test limit exhaustion with a deterministic adversarial input, never with a
  wall-clock assertion.
- The two `unsafe` sites are sound as written; do not rewrite them to satisfy
  the lint.
- `Cargo.toml` package lints apply to tests too; keep the test-only allow
  narrow and commented.
- Implementation note (2026-11-17): a third `unsafe` block existed beyond the
  two listed in §3 — `static mut CMD_ID` in
  `tests/mcp/test_composite_tools.rs`. It was replaced with an `AtomicU32`
  (removing the data race and the need for a third allow), not allowed.
  The catastrophic fixture is `(?P<w>a+)(?P=w)b$` over 1,000 `a` characters:
  the advisory scanner only matches `\1`/`\g<`, so this reaches the backend
  and returns `BacktrackLimitExceeded`. The tool layer maps it to
  `execution_error` plus the stable `INTERNAL_ERROR` machine code.
- The consumer audit found two further end-to-end gaps beyond the listed
  lines: `validate_schema_light` constructed fancy-regex directly (bypassing
  the limits) and discarded match errors; it now uses `compile_fancy` and
  propagates compile/match errors. `config_preflight` was the sole production
  caller of that validator and discarded its `Err`; it now reports a
  `SCHEMA_ERROR` finding with `CONFIG_SCHEMA_MISMATCH` instead of leaving the
  verdict clean. Regression tests cover both.
