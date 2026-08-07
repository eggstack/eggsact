# Corrective Pass — Runtime Soundness and Boundary Semantics

## Status

- **Status:** completed
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Planning baseline:** `468a812780e9199ca6002bbd0f2b3b9a41aeaa55`
- **Parent roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Priority:** release-blocking
- **Scope:** close the remaining soundness defect in the evaluation-context bridge, repair the bounded JSONL reader's chunk-boundary behavior, and make Windows drive-relative path semantics consistent across normalize/analyze/scope APIs
- **Expected change size:** small-to-medium, localized to three implementation surfaces and focused tests

## Purpose

The August 4 bounded-correctness roadmap largely landed, but post-implementation review found three correctness issues that prevent the roadmap from being considered fully closed:

1. the deprecated safe `current_eval_context() -> Option<&'static mut EvalContext>` API remains callable, and the replacement `with_current_eval_context()` does not currently prevent re-entrant mutable access to the same raw pointer;
2. the bounded MCP JSONL reader can discard bytes belonging to the next request while draining an oversized line and can reject an exactly-at-limit line depending on underlying buffer boundaries;
3. Windows drive-relative paths such as `C:foo` are correctly non-absolute in `path_normalize()` but are still classified inconsistently by `path_analyze()` and may be resolved under an unrelated root by `path_scope_check()`.

This pass is intentionally narrow. It does not reopen the execution architecture, transport design, path subsystem, CI design, or tool surface.

---

# Hard constraints

This pass must not:

- add MCP tools, profiles, audiences, or protocol extensions;
- redesign handler signatures;
- unify or replace the MCP and synchronous execution engines;
- add a new context framework or context crate;
- replace Tokio or the stdio transport;
- introduce a generic codec/framing framework;
- add filesystem canonicalization, symlink resolution, or host-filesystem access to lexical path tools;
- add a third-party path crate;
- add a new dependency unless absolutely unavoidable; the expected solution requires none;
- increase request-size limits or tool budgets;
- add a new CI workflow, benchmark harness, or evidence system;
- change unrelated CLI behavior;
- perform further binary-size optimization;
- create a new roadmap after this corrective pass.

Prefer deletion, explicit invariants, and focused tests.

---

# Files to inspect first

At minimum inspect:

```text
src/mcp/budget.rs
src/tools/math.rs
src/agent/mod.rs
tests/test_context_isolation.rs

src/mcp/server.rs
src/mcp/runtime.rs
tests/mcp/

src/text/path.rs
src/tools/path.rs
tests/text/test_path.rs

architecture/budget-concurrency.md
architecture/mcp-server.md
architecture/text-library.md
AGENTS.md
CHANGELOG.md
plans/2026-08-04-phase-1-execution-context-soundness.md
plans/2026-08-04-phase-2-path-and-wire-boundary-corrections.md
```

Useful rejection searches before editing:

```text
current_eval_context(
&'static mut EvalContext
CURRENT_EVAL_CONTEXT
read_bounded_line
AsyncReadExt
C:foo
is_absolute
path_scope_check
_split_windows_components
```

---

# Workstream 1 — Finish evaluation-context soundness

## Problem

Current `src/mcp/budget.rs` still exposes:

```rust
pub fn current_eval_context() -> Option<&'static mut EvalContext>
```

The function is deprecated but remains a safe callable API that can manufacture an escaping mutable reference from a raw thread-local pointer. Deprecation does not remove the undefined-behavior surface.

The replacement:

```rust
pub fn with_current_eval_context<R>(
    f: impl FnOnce(Option<&mut EvalContext>) -> R,
) -> R
```

copies the raw pointer out of the `RefCell`, releases the `RefCell` borrow, then calls the callback. A callback can call `with_current_eval_context()` again while the first `&mut EvalContext` remains live, creating aliased mutable references to the same object from safe Rust.

The August 4 roadmap explicitly requires that safe Rust cannot obtain either an escaping or aliased mutable evaluation context. This is therefore a correctness defect, not API polish.

## Required outcome

After this workstream:

- `current_eval_context()` no longer exists as a callable safe function;
- no public or crate-visible safe helper returns `&'static mut EvalContext`;
- only one mutable accessor to the installed evaluation context can be active at a time for the same thread-local bridge;
- re-entrant mutable access fails safely instead of creating a second `&mut`;
- normal access still works for `math_eval`;
- nested `with_eval_context()` scopes still restore parent contexts correctly;
- unwind/panic paths restore both the installed pointer and mutable-access guard state;
- no global mutex or broad handler redesign is introduced.

## Preferred implementation shape

Use the smallest explicit borrow-state mechanism around the existing bridge.

A suitable design is one of the following, in preference order:

1. keep an exclusive `RefCell`/borrow guard alive across the callback if doing so does not break required nested `with_eval_context()` behavior; or
2. add a tiny thread-local mutable-access flag/guard that prevents a second accessor from entering while the first accessor's `&mut` is alive, with RAII restoration on unwind.

Do not silently return `None` on re-entry because that makes a violated invariant look like an absent context. A deterministic panic/assertion for an impossible internal re-entrant mutable borrow is acceptable if it is documented and tested. If a non-panicking internal error path can be implemented without broad API churn, that is also acceptable.

Do not build a generalized borrowing abstraction.

## Required code changes

- Delete `current_eval_context()` rather than retaining another deprecated compatibility shim.
- Remove documentation and changelog wording that claims the unsound API is merely deprecated.
- Ensure `with_current_eval_context()` holds whatever exclusive-access state is needed for the entire duration of `f`.
- Ensure any access-state guard is restored by `Drop`, including during unwind.
- Keep `with_eval_context()` guard-owned parent restoration from the previous pass.
- Keep `math_eval` on the closure-scoped API.

## Required tests

Add focused tests proving all of the following:

1. no context installed -> callback receives `None`;
2. one installed context -> callback receives and can mutate that exact context;
3. sequential accessor calls succeed;
4. re-entrant accessor on the same installed context cannot obtain a second mutable reference;
5. access-state restoration works after panic/unwind;
6. depth-3 nested `with_eval_context()` scopes still restore the immediate parent correctly;
7. depth-3 cancellation nesting remains unaffected;
8. `math_eval` through direct registry dispatch still uses the installed per-call context;
9. bounded/in-process context-aware dispatch still preserves deterministic seed/context behavior;
10. no test uses the removed `current_eval_context()` API.

The re-entry regression test is mandatory. A closure-only signature by itself is not sufficient evidence of soundness.

## Rejection checks

The workstream is not complete if any of these remain in production code:

```text
pub fn current_eval_context
Option<&'static mut EvalContext>
#[allow(deprecated)] ... current_eval_context
```

A raw pointer may remain inside the implementation only if the safe API around it enforces the lifetime/exclusivity invariant described above.

---

# Workstream 2 — Make bounded JSONL reading boundary-correct

## Problem

`read_bounded_line()` correctly avoids allocating an arbitrarily large line, but its oversize-drain path switches from `AsyncBufRead::fill_buf()/consume()` to `AsyncReadExt::read()`.

When the drain read returns a buffer containing both:

- the newline terminating the oversized request, and
- bytes from the next JSONL request,

all bytes in that read have already been consumed. The function stops at the newline but cannot put the following bytes back, so the next request can be partially or completely discarded.

There is a second edge case: if payload length equals `MAX_REQUEST_BYTES` but the newline/EOF is only visible in the next underlying buffer fill, the current `total >= max_bytes` branch can classify the line as oversized before determining whether the next byte is the terminator.

These are framing correctness defects.

## Required outcome

After this workstream:

- at most `max_bytes` payload bytes plus a small constant amount of framing state are retained;
- oversized-line draining uses `fill_buf()/consume()` or an equivalent buffered mechanism that consumes exactly through the terminating newline and no farther;
- bytes after the oversized line's newline remain available for the next call;
- a payload of exactly `max_bytes` bytes followed by LF is accepted;
- a payload of exactly `max_bytes` bytes followed by CRLF is accepted;
- a payload of exactly `max_bytes` bytes followed by EOF is accepted if EOF-terminated final lines are part of the existing contract;
- `max_bytes + 1` payload bytes are rejected;
- oversized unterminated input remains bounded and is rejected;
- one oversized request produces one error and the following valid request is still processed;
- no generic framing subsystem is added.

## Preferred implementation shape

Keep `read_bounded_line()` as a small private helper.

Use only the `AsyncBufRead` buffer while deciding and draining a frame:

1. inspect the current buffer with `fill_buf()`;
2. if a newline exists, consume only through that newline;
3. append only payload bytes that fit within the cap;
4. if the accumulated payload reaches exactly the cap without a visible terminator, continue peeking without appending payload bytes;
5. accept if the next framing bytes are LF, CRLF, or EOF according to the existing final-line contract;
6. otherwise mark oversized and drain using repeated `fill_buf()/consume()` calls, consuming only through the next newline;
7. never call a raw `read()` that can over-consume bytes past the newline.

Do not solve this with an unbounded `read_line()` or by raising the size limit.

## Required tests

Use small-capacity buffered readers so chunk boundaries are deterministic. `tokio::io::BufReader::with_capacity(...)` around an in-memory reader is sufficient; do not add a test dependency.

Mandatory cases:

1. short LF line;
2. short CRLF line;
3. exactly-at-cap payload + LF in same buffer;
4. exactly-at-cap payload with LF in the next buffer fill;
5. exactly-at-cap payload with CRLF split across buffer fills;
6. exactly-at-cap EOF-terminated final line;
7. cap+1 payload rejected;
8. multi-megabyte unterminated input rejected without retaining the whole input;
9. oversized line followed by valid line where the newline and beginning of the valid line would have appeared in one underlying read under the old implementation;
10. oversized line followed by two valid lines, both preserved;
11. empty line behavior remains unchanged.

At least one regression test must fail under the current implementation specifically because the old drain path consumes bytes after the newline.

## Rejection checks

The workstream is not complete if the oversize drain path still uses raw `AsyncReadExt::read()` on the buffered reader in a way that can consume beyond the frame terminator.

---

# Workstream 3 — Make Windows drive-relative semantics consistent

## Problem

Windows syntax distinguishes:

```text
C:\foo   # drive-rooted absolute path
C:/foo    # drive-rooted absolute path
C:foo     # drive-relative path; relative to the current directory on drive C
C:        # drive-relative drive designator
```

`path_normalize()` now correctly reports `C:foo` as non-absolute, but the broader path APIs are inconsistent:

- `_split_windows_components()` represents the drive prefix as a `root` whenever it sees `C:`;
- `path_analyze()` currently defines `absolute = root.is_some()`, so `C:foo` can still be reported as absolute;
- `path_scope_check()` treats a non-absolute target as an ordinary relative target and can concatenate `C:foo` under an unrelated root such as `D:\workspace`, producing a lexical result that does not model Windows semantics.

Because these tools are used for preflight/scope decisions, ambiguous drive-relative paths should not be treated as safely inside an arbitrary root.

## Required outcome

After this workstream:

- drive-relative and drive-rooted prefixes are structurally distinguishable in all relevant Windows path helpers;
- `path_analyze("C:foo", "windows")` reports `absolute == false`;
- `path_analyze("C:\\foo", "windows")` and `path_analyze("C:/foo", "windows")` report `absolute == true`;
- `path_scope_check()` never resolves a drive-relative target beneath an unrelated root by string concatenation;
- because the lexical API has no per-drive current-working-directory state, a drive-relative target is treated conservatively as not provably inside the supplied root;
- the result includes an existing-compatible finding/warning explaining the ambiguity where practical;
- normal relative paths such as `src\\main.rs` continue to resolve against the supplied root;
- UNC behavior from the previous pass remains intact;
- no filesystem lookup or Windows API call is introduced.

## Preferred implementation shape

Introduce one small private prefix classifier rather than duplicating drive/UNC tests across functions. For example, an internal enum along the lines of:

```rust
enum WindowsPrefix<'a> {
    None,
    DriveRelative { drive: &'a str },
    DriveRooted { drive: &'a str },
    Unc { host: &'a str, share: Option<&'a str> },
}
```

The exact type is implementation-defined. Keep it private and minimal.

Use the classifier consistently in:

- Windows splitting/analysis;
- `path_normalize()` absolute determination;
- scope checking;
- any helper that reconstructs Windows prefixes.

Do not expand this pass into full support for device namespaces (`\\?\`, `\\.\`) unless an existing test/contract already requires them.

## Scope semantics for drive-relative targets

Use the following conservative contract:

- `C:foo` is not absolute;
- it is not equivalent to `foo`;
- it cannot be safely resolved under `C:\root` or `D:\root` without the current directory for drive C;
- therefore `path_scope_check(root, "C:foo", "windows", ...)` returns `inside_root == false`;
- a finding should state that a drive-relative target is ambiguous/not safely resolvable lexically;
- ordinary path-relative targets without a drive prefix retain current behavior.

This is preferable to inventing per-drive CWD state or silently changing Windows meaning.

## Required tests

Add focused tests for:

1. analyze `C:foo` -> relative;
2. analyze `C:` -> relative;
3. analyze `C:\\foo` -> absolute;
4. analyze `C:/foo` -> absolute;
5. normalize `C:foo` remains drive-relative and preserves the drive designator;
6. scope root `C:\\work`, target `C:foo` -> not inside;
7. scope root `D:\\work`, target `C:foo` -> not inside;
8. scope root `C:\\work`, target `C:..\\secret` -> not inside;
9. ordinary relative target `src\\main.rs` still resolves under a Windows root;
10. existing UNC share-root and traversal tests still pass;
11. case-insensitive comparison behavior is unchanged.

---

# Execution order for a smaller implementation model

Execute strictly in this order. Do not combine the three workstreams into one refactor.

## Step 1 — Soundness

1. Read `budget.rs` and context-isolation tests.
2. Add the re-entrant-access failing test first.
3. Remove `current_eval_context()`.
4. Add the minimum exclusive-access guard around `with_current_eval_context()`.
5. Add unwind restoration coverage.
6. Run focused context/math tests.
7. Stop and fix failures before touching the JSONL reader.

## Step 2 — JSONL reader

1. Read the existing bounded-line tests and main read loop.
2. Add deterministic small-buffer regression tests.
3. Replace raw drain reads with exact buffered consumption.
4. Correct the exact-cap terminator/EOF decision.
5. Run only bounded-line tests until green.
6. Run MCP server/hardening tests.
7. Stop and fix failures before touching path semantics.

## Step 3 — Drive-relative paths

1. Read all Windows path parsing helpers.
2. Add analyze/scope failing tests for `C:foo`.
3. Add the smallest private prefix classification needed.
4. Apply it consistently to analyze/normalize/scope.
5. Run the full path test module.
6. Run patch/repo/preflight tests that depend on path scope.

## Step 4 — Documentation

Update only documentation directly affected by changed contracts:

- execution-context bridge no longer exposes `current_eval_context()`;
- re-entrant mutable accessor behavior/invariant;
- bounded request reader preserves subsequent frames;
- drive-relative Windows path scope is intentionally conservative.

Do not update roadmap completion records in this implementation plan; the separate reproducibility/closure plan handles final record reconciliation after this pass is verified.

---

# Verification

Run focused tests during each workstream, then the ordinary repository gate:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Do not add packaging/publish checks to ordinary implementation commits. Those remain in `scripts/release-check.sh` and are run during the final closure plan.

Parity is not required unless implementation unexpectedly changes calculator compatibility semantics. This pass should not do so.

Fuzzing is not required for closure; the new deterministic boundary tests are sufficient for these specific defects.

---

# Acceptance checklist

This plan is complete only when all items are true:

- [x] `current_eval_context()` has been deleted from production code.
- [x] No safe production API returns `&'static mut EvalContext` from the thread-local bridge.
- [x] Re-entrant mutable eval-context access cannot create a second live `&mut`.
- [x] Exclusive-access state is restored on normal return and unwind.
- [x] Existing nested `with_eval_context()` and cancellation restoration remain correct.
- [x] `math_eval` and context-aware registry dispatch retain deterministic context behavior.
- [x] Oversized JSONL draining cannot consume bytes after the terminating newline.
- [x] Exactly-at-cap LF, CRLF, and supported EOF termination are accepted across buffer boundaries.
- [x] Cap+1 and oversized unterminated requests are rejected while retaining bounded memory.
- [x] A valid request immediately following an oversized request is preserved and processed.
- [x] `path_analyze("C:foo", "windows")` is relative.
- [x] Drive-rooted Windows paths remain absolute.
- [x] `path_scope_check()` does not place drive-relative targets under arbitrary roots.
- [x] Ordinary relative Windows targets still resolve under the supplied root.
- [x] UNC share-boundary behavior remains correct.
- [x] No new dependency, tool, workflow, subsystem, or public protocol surface was added.
- [x] Ordinary verification passes.
- [x] Documentation for the three corrected contracts is accurate.

---

# Explicit non-goals / deferred work

Do not address the following in this pass:

- Unicode source pinning and closure-record reconciliation — handled by the companion plan `2026-08-07-reproducibility-and-closure-correction.md`;
- unknown CLI flag handling;
- broader Windows device-path syntax;
- engine unification;
- replacing raw-pointer TLS with a project-wide context architecture;
- reducing test subprocess count beyond what is required for these regressions;
- changing the existing `--test-threads=4` containment decision;
- additional binary-size work;
- dependency consolidation;
- release automation.

If implementation uncovers a new issue unrelated to the listed acceptance criteria, record it in the completion record and stop rather than expanding scope.

---

# Completion record

Fill once implementation lands:

- **Implementation commit(s):** pending (to be committed after this update)
- **Unsound accessor disposition:** deleted from `src/mcp/budget.rs` (no shim retained)
- **Re-entrant mutable-access design:** thread-local `EVAL_CONTEXT_MUTABLY_BORROWED` flag with RAII `EvalBorrowGuard`; panics on re-entry, restores on drop/unwind
- **Context regression tests:** 10 new tests in `tests/test_context_isolation.rs` (WS1-1 through WS1-10); all pass
- **Bounded-reader implementation:** `read_bounded_line()` rewritten to use only `fill_buf()`/`consume()`; line-length tracking via `payload_total` + `tentative` counters; CRLF detection across chunk boundaries via `last_byte` state
- **Chunk-boundary regression tests:** 9 new tests in `src/mcp/server.rs` (buffered_cursor helpers, exactly-cap, CRLF split, oversized-then-valid, multi-line preservation); all pass
- **Drive-relative classification design:** private `WindowsPrefix` enum with `_classify_windows_prefix()` classifier; used consistently in `path_analyze` (absolute check + warning) and `path_scope_check` (conservative rejection)
- **Path regression tests:** 9 new tests in `tests/text/test_path.rs` (WS3-1 through WS3-11); all pass
- **Ordinary verification:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --locked --all-features --skip parity --test-threads=4` (3560 passed), `cargo test --locked --doc` (11 passed), `cargo run --features dev-tools --bin generate-docs -- --check` — all green
- **Documentation updated:** AGENTS.md, architecture/budget-concurrency.md, architecture/text-library.md, CHANGELOG.md, plans/2026-08-07-corrective-runtime-soundness-and-boundaries.md
- **Deferred findings:** none expected beyond companion closure plan
- **Final disposition:** implementation complete, all acceptance items verified

Do not mark the parent roadmap fully closed from this plan alone. Proceed to the companion reproducibility/closure plan only after every acceptance item above passes.