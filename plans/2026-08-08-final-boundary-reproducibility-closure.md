# Final Corrective Pass — JSONL Boundary Accounting, Unicode Source Pin, and Closure Hygiene

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Planning baseline:** `948c22d22a7e1765674d1b73602b9d3a811819af`
- **Parent roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Prior corrective plans:**
  - `plans/2026-08-07-corrective-runtime-soundness-and-boundaries.md`
  - `plans/2026-08-07-reproducibility-and-closure-correction.md`
- **Priority:** final release-blocking correctness correction plus narrow repository hygiene
- **Scope:** close one remaining bounded-reader CRLF accounting defect, make the Unicode 17.0.0 source URL genuinely version-specific, remove an accidentally committed Python cache artifact, correct one Windows drive-relative diagnostic, reconcile closure records once, run the canonical release gate once, and stop
- **Expected change size:** small; concentrated in `src/mcp/server.rs`, `scripts/generate_confusables.py`, `.gitignore`, one path diagnostic/test, and existing planning/documentation records

## Purpose

The August 7 corrective implementation substantially closed the original roadmap. The unsafe escaping `EvalContext` accessor is gone, re-entrant mutable context access is dynamically rejected, oversized JSONL draining no longer consumes bytes past the next newline, and Windows drive-relative paths are handled conservatively.

A follow-up audit of commit `948c22d` found one remaining release-relevant reader bug and several narrow closure/hygiene issues:

1. the bounded JSONL reader can under-count an oversized line when payload bytes beyond the cap occur in one buffer and a CRLF terminator occurs in a later buffer;
2. the confusables generator still uses the moving `/security/latest/` URL despite the closure record claiming otherwise;
3. a Python `__pycache__` bytecode file was accidentally committed;
4. the Windows drive-relative scope diagnostic hardcodes `C:` even when the actual target drive is different;
5. existing completion records now overstate closure because they mark the affected acceptance criteria complete.

This is not a new roadmap. It is the final bounded correction to the existing line of work. Do not reopen binary-size optimization, CI architecture, dependency consolidation, execution-engine design, or feature work.

---

# Hard constraints

This pass must not:

- add or remove MCP tools;
- change the 80-tool product surface;
- add profiles, audiences, protocol extensions, services, or daemons;
- add dependencies;
- add a new execution/context framework;
- redesign the MCP transport or switch away from JSONL over stdio;
- add a general streaming/framing abstraction;
- add a path abstraction crate or filesystem canonicalization;
- alter confusable mappings or upgrade Unicode beyond 17.0.0;
- introduce network access into ordinary CI or `scripts/release-check.sh`;
- add a Python test framework solely for the generator;
- add CI workflows, release automation, tagging, crates.io publication, artifact uploads, or evidence registries;
- revisit Tokio runtime selection, release-profile settings, `serde_json/preserve_order`, or TOML consolidation;
- perform unrelated CLI cleanup;
- create another polish/evidence/closure plan after this pass unless a new reproducible product defect is discovered.

Prefer deletion and simpler state accounting over additional flags/counters.

---

# Files to inspect first

At minimum inspect:

```text
src/mcp/server.rs
src/mcp/budget.rs
src/text/path.rs
tests/text/test_path.rs
scripts/generate_confusables.py
src/text/confusables_generated.rs
data/confusables.rs
src/text/confusables.rs
.gitignore
scripts/release-check.sh
CHANGELOG.md
AGENTS.md
architecture/generated-assets.md
architecture/text-library.md

plans/2026-08-04-bounded-correctness-simplification-roadmap.md
plans/2026-08-07-corrective-runtime-soundness-and-boundaries.md
plans/2026-08-07-reproducibility-and-closure-correction.md
```

Also inspect git history around:

```text
a3f78e30d27bd2e8a0629265ec0fca4992689ebd
948c22d22a7e1765674d1b73602b9d3a811819af
```

Search for:

```text
read_bounded_line
payload_total
tentative
last_byte
security/latest
UNICODE_SECURITY_VERSION
EXPECTED_SHA256
__pycache__
Drive-relative target 'C:
Status: complete
Final disposition
```

Do not assume the existing completion checkboxes prove the implementation; verify the code and tests directly.

---

# Workstream 1 — Fix CRLF accounting in `read_bounded_line`

## Severity

**Release-blocking correctness defect.**

The August 7 rewrite fixed over-consumption during oversized-line draining, but the line-length accounting can still under-count payload when the line is already over the cap before a later chunk containing the CRLF terminator is observed.

## Current failure mode

The current reader tracks:

```text
accumulated
payload_total
tentative
last_byte
```

When no newline exists in a chunk, bytes that no longer fit in the retained buffer are added to `tentative`.

When a later chunk contains `\r\n` with the LF at `pos > 0`, the current CRLF branch computes the line payload from `payload_total + (pos - 1)` and does not include previously accumulated `tentative` payload bytes.

A concrete regression shape is:

```text
max_bytes = 3
fill 1: "xxx"
fill 2: "YYY"
fill 3: "\r\nnext\n"
```

The actual first-line payload is six bytes:

```text
xxxYYY
```

but the current accounting can classify the line as a three-byte payload because the three tentative `Y` bytes are omitted from the `pos > 0 && crlf` calculation. The reader can therefore return a truncated accepted line instead of `TooLarge`.

This is a boundary-dependent bug: ordinary `Cursor` tests and LF-only oversize tests do not prove the CRLF case.

## Required outcome

After this workstream:

- every byte before the LF is counted regardless of how many `fill_buf()` chunks were consumed;
- a trailing `\r` immediately before LF is excluded from payload length exactly once;
- prior discarded/unstored payload bytes can never disappear from length accounting;
- payload length exactly equal to `max_bytes` is accepted for LF, CRLF, and supported EOF termination;
- payload length `max_bytes + 1` or larger is rejected for LF, CRLF, and EOF termination;
- oversized lines are drained through exactly the terminating LF and never consume bytes from the next JSONL frame;
- EOF after an oversized unterminated line returns `TooLarge`;
- memory retained for one request remains bounded to `max_bytes + O(1)` or `max_bytes + small fixed buffer`, not proportional to an arbitrarily large input line;
- UTF-8 conversion behavior remains unchanged unless required by the existing contract;
- no new transport/framing abstraction is introduced.

## Preferred simplification

Do not patch the existing `payload_total + tentative` formulas with another branch-specific counter unless that is demonstrably simpler than replacing the accounting model.

A simpler model is preferred:

1. maintain a total `bytes_before_lf` count for **all** bytes consumed before the line-ending LF, including bytes no longer retained in the output buffer;
2. retain at most `max_bytes + 1` bytes (or an equivalent narrowly bounded amount) so the exactly-at-cap plus possible trailing-CR case can be reconstructed;
3. track the byte immediately preceding LF across buffer boundaries;
4. when LF is found:
   - add `pos` to `bytes_before_lf`;
   - determine whether the byte immediately before LF is `\r`;
   - compute `payload_len = bytes_before_lf - 1` only when that final byte is the CR of CRLF;
   - otherwise `payload_len = bytes_before_lf`;
5. classify `payload_len > max_bytes` as `TooLarge`;
6. consume exactly `pos + 1` bytes from the current `fill_buf()` result and leave everything after LF untouched;
7. at EOF, use the total bytes seen as payload because there is no terminator to discount.

It is acceptable to mark a line definitively oversized before its terminator once more than `max_bytes + 1` bytes have been observed, because at most one trailing CR can later be removed from the payload count. Even then, continue draining with `fill_buf()/consume()` only until LF or EOF.

The exact implementation can differ, but the invariants above are mandatory.

## Required regression tests

Add tests that force small/controlled buffer boundaries. Use the existing small-capacity Tokio `BufReader` test helper or a minimal test-only `AsyncBufRead` if necessary. Do not add a production abstraction only to test chunking.

At minimum add:

### WS1-1 — Oversized payload then CRLF in later fill

Exact shape equivalent to:

```text
max = 3
chunks expose: "xxx" / "YYY" / "\r\nnext\n"
```

Expected:

```text
first read  -> TooLarge
second read -> Line("next")
```

This is the specific regression that must fail before the fix and pass after it.

### WS1-2 — Cap+1 payload with CRLF

```text
payload = "x" * (max + 1)
terminator = "\r\n"
```

Force the trailing CR and/or CRLF into a later fill than the byte that crosses the cap.

Expected: `TooLarge`.

### WS1-3 — Exactly-cap payload with CRLF split at boundary

```text
payload = "x" * max
terminator = "\r\n"
```

Force the final payload byte, CR, and LF across useful buffer boundaries.

Expected: accepted intact payload.

### WS1-4 — Oversized CRLF frame followed by multiple valid frames

```text
<oversized>\r\nfirst\nsecond\r\n
```

Expected:

```text
TooLarge
Line("first")
Line("second")
```

No byte from either valid frame may be lost.

### WS1-5 — EOF boundaries

Cover:

```text
exactly max payload + EOF -> Line
max + 1 payload + EOF     -> TooLarge
```

### WS1-6 — Existing LF behavior remains correct

Retain the existing LF tests; do not regress them while simplifying CRLF accounting.

## Rejection searches

After implementation, inspect `read_bounded_line` and confirm there is no branch where previously consumed but unretained bytes can be excluded from the final payload count.

Search for stale logic/comments tied to the old model:

```bash
rg "tentative|payload_total|reached_cap|AsyncReadExt::read" src/mcp/server.rs
```

It is fine for one of these names to remain if the implementation is genuinely correct, but every remaining counter must have one unambiguous meaning across all branches.

---

# Workstream 2 — Make the Unicode 17.0.0 source URL genuinely version-specific

## Severity

**Reproducibility/closure correctness.**

The current generator now verifies a pinned version and SHA-256 before writing, which is good. However it still contains:

```python
CONFUSABLES_URL = "https://www.unicode.org/Public/security/latest/confusables.txt"
```

The existing completion record incorrectly checks off “no moving `/latest/` source” while retaining that URL.

The checksum guard prevents a silent data change, but the source locator itself is still moving and the documentation is internally contradictory.

## Required outcome

After this workstream:

- `CONFUSABLES_URL` points to the official **version-specific Unicode Security 17.0.0** `confusables.txt` source;
- no `/latest/` URL remains in generator code or documentation that claims to identify the pinned source;
- `UNICODE_SECURITY_VERSION` remains exactly `17.0.0`;
- `EXPECTED_SHA256` remains the current known checksum unless the official versioned source proves the previous byte-level checksum was taken from different content;
- the downloaded bytes are still checksum-verified before any output write;
- the file header version is still verified before any output write;
- a mismatch still fails loudly and leaves generated outputs untouched;
- the generated 6565-entry semantic table does not change;
- ordinary CI and `scripts/release-check.sh` remain offline with respect to Unicode downloads.

## Source verification rule

Before changing the constant, verify the official Unicode directory rather than relying on a guessed URL pattern.

The expected versioned pattern is likely under Unicode's `Public/security/<version>/` hierarchy, for example:

```text
https://www.unicode.org/Public/security/17.0.0/confusables.txt
```

but the implementer must confirm the exact official URL before committing it. Do **not** replace `/latest/` with an unverified path merely to satisfy a string check.

Verification must establish:

1. the URL is hosted by `www.unicode.org` under the official Public data tree;
2. the file header reports version `17.0.0`;
3. the exact fetched bytes hash to the expected SHA-256:

```text
091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a
```

If the official versioned source exists but its bytes do not match the current checksum:

- stop;
- diff the fetched bytes/content against the currently expected dataset;
- determine whether the difference is transport/header-only or semantic;
- do not silently update `EXPECTED_SHA256`;
- do not upgrade Unicode;
- do not regenerate a changed table until the discrepancy is understood.

## Required verification

With network access available for this one manual generator check:

```bash
python3 scripts/generate_confusables.py
git diff -- src/text/confusables_generated.rs data/confusables.rs
```

Expected semantic result:

- 6565 entries;
- sorted numeric table remains sorted;
- representative substitutions unchanged;
- ideally generated table content is byte-for-byte unchanged apart from a source-URL comment if that comment is embedded in the generated output.

Run the generator twice and confirm deterministic output.

Do not add this network operation to CI or the release gate.

---

# Workstream 3 — Remove Python cache artifact and prevent recurrence

## Problem

Commit `948c22d` includes:

```text
scripts/__pycache__/generate_confusables.cpython-312.pyc
```

This is an interpreter cache artifact, not source. `scripts/` is excluded from the crates.io package, so this does not change packaged product behavior, but it should not remain in the repository.

The current `.gitignore` does not ignore Python bytecode/cache directories.

## Required outcome

- delete the committed `scripts/__pycache__/generate_confusables.cpython-312.pyc` file;
- add narrow standard Python cache ignores to `.gitignore`;
- do not add broad ignores that could hide intended project source or generated assets.

Preferred entries:

```gitignore
__pycache__/
*.py[cod]
```

One of these may be sufficient if repository convention prefers fewer patterns. The important invariant is that running the generator cannot re-add interpreter bytecode artifacts.

## Verification

After running the generator:

```bash
git status --short
```

must not show `__pycache__` or `.pyc` files.

---

# Workstream 4 — Correct the Windows drive-relative diagnostic

## Severity

**Low; diagnostic correctness only.**

The safety behavior is already correct: a drive-relative target is conservatively outside the supplied root.

The current finding message is built with a literal `C:` prefix. A target such as:

```text
D:foo
```

can therefore receive a diagnostic referring to the wrong drive.

## Required outcome

- preserve the existing `inside_root == false` behavior for all drive-relative targets;
- construct the finding from the actual target and/or classified drive;
- do not alter Windows path resolution semantics beyond the diagnostic text;
- keep the message concise and deterministic.

Preferred shape:

```text
Drive-relative target 'D:foo' cannot be resolved lexically; the result depends on the current directory on drive D
```

Use the actual normalized/pre-normalized input rather than reconstructing it with a hardcoded drive.

## Required regression test

Add a test with a non-C drive, for example:

```rust
let result = path_scope_check("C:\\work", "D:foo", "windows", true);
assert!(!result.inside_root);
assert!(result.findings.iter().any(|f| f.contains("D:foo")));
assert!(!result.findings.iter().any(|f| f.contains("C:foo")));
```

Do not expand into device paths, UNC redesign, per-drive CWD modeling, or filesystem resolution.

---

# Workstream 5 — Reconcile closure records once, after implementation passes

## Problem

The prior corrective plans and parent roadmap are currently marked complete, but at least two completion assertions are now known to be inaccurate:

- the bounded-reader acceptance checklist does not account for the remaining CRLF under-count regression;
- the reproducibility plan says the generator no longer uses `/latest/` even though the code still does.

Do not perform a broad historical rewrite. Add a concise final corrective record once the code is fixed.

## Required updates

After Workstreams 1–4 pass:

### `plans/2026-08-07-corrective-runtime-soundness-and-boundaries.md`

Update the completion record to note:

- the August 7 implementation commit `a3f78e3` fixed the original drain-overconsumption issue;
- the August 8 follow-up found and corrected a separate CRLF cross-buffer payload-accounting defect;
- record the final corrective implementation SHA;
- keep the original completed work intact rather than rewriting history.

### `plans/2026-08-07-reproducibility-and-closure-correction.md`

Correct the inaccurate source statement:

- remove the claim that a version-specific source is unavailable if that claim is not true;
- record the exact official version-specific Unicode 17.0.0 URL actually used;
- retain the pinned checksum and entry count;
- record the final corrective implementation SHA.

### `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`

Add one concise final follow-up note:

- residual reader/accounting and source-pin issues were found after the initial closure declaration;
- the final corrective SHA closed them;
- the roadmap remains complete only after the acceptance criteria in this plan pass.

Do not create a new roadmap, evidence registry, or retrospective report.

### This plan

Fill the completion record below with concrete results and mark `Status: complete` only after every acceptance item passes.

---

# Execution order for a smaller implementation model

Follow this order exactly. Do not combine unrelated refactors.

## Step 1 — Establish the reader regression

1. Read the current `read_bounded_line` implementation.
2. Read all existing bounded-line tests.
3. Add WS1-1 first: oversized bytes beyond cap in one fill, CRLF in a later fill, followed by a valid frame.
4. Confirm the new test fails on baseline `948c22d` for the expected reason.
5. Add the other CRLF/exact-cap regressions before rewriting the reader.

If WS1-1 unexpectedly passes, inspect buffer sizing and adjust the test harness until it deterministically exposes the intended fill boundaries. Do not declare the defect absent merely because a `Cursor` coalesces data differently.

## Step 2 — Simplify bounded-line accounting

1. Replace or correct the `payload_total/tentative` state so every consumed pre-LF byte contributes to one total.
2. Discount only the final CR immediately preceding LF.
3. Retain bounded payload bytes only.
4. Drain oversized frames with `fill_buf()/consume()` only.
5. Consume exactly through LF.
6. Run all bounded-line unit tests.

Do not change JSON-RPC parsing, request concurrency, writer behavior, or server limits.

## Step 3 — Correct Unicode source pin

1. Confirm the official version-specific Unicode 17.0.0 security-data URL.
2. Confirm the header version and SHA-256.
3. Replace `/latest/` with the verified version-specific URL.
4. Run the generator once.
5. Confirm no semantic table change.
6. Run it a second time and confirm deterministic output.

Do not modify the checksum merely because a download differs; investigate first.

## Step 4 — Repository hygiene

1. Delete the committed `.pyc` file.
2. Add narrow Python cache ignores.
3. Re-run the generator.
4. Confirm no ignored bytecode artifact appears in git status.

## Step 5 — Diagnostic correction

1. Replace the hardcoded drive in the drive-relative finding.
2. Add the non-C-drive regression test.
3. Run focused path tests.

## Step 6 — Focused verification

Run at minimum:

```bash
cargo test --locked --all-features bounded_line -- --test-threads=1
cargo test --locked --test lib text::test_path -- --test-threads=1
cargo test --locked --test test_context_isolation -- --test-threads=1
```

Adjust the exact integration-test command if the repository's test layout requires it; the intent is to run the bounded-reader, path, and context regression layers directly.

The `EvalContext` tests are included as a guard that this final pass did not disturb the already-corrected soundness work.

## Step 7 — Ordinary verification

Run the repository's ordinary implementation gate:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Do not add additional verification infrastructure.

## Step 8 — Reconcile records

Only after Steps 1–7 are green:

1. update the two August 7 corrective completion records;
2. add the concise parent-roadmap final-follow-up note;
3. update `CHANGELOG.md` only for actual externally/developer-visible corrections;
4. fill this plan's completion record.

## Step 9 — Canonical release gate

Commit the implementation/documentation correction, ensure the worktree is clean, then run once:

```bash
scripts/release-check.sh
```

The script remains the sole canonical full local release check.

Do not publish or tag.

## Step 10 — Stop

If the canonical release gate passes and no new reproducible defect appears, this line of work is complete.

Do not create another closure-only plan.

---

# Acceptance checklist

This plan is complete only when every item below is true.

## Bounded reader

- [x] The specific oversized-payload-then-later-CRLF regression returns `TooLarge`.
- [x] Bytes discarded from the retained request buffer still count toward total payload length.
- [x] Exactly `max_bytes` payload bytes followed by LF are accepted.
- [x] Exactly `max_bytes` payload bytes followed by CRLF are accepted across chunk boundaries.
- [x] Exactly `max_bytes` payload bytes followed by EOF are accepted.
- [x] `max_bytes + 1` payload bytes followed by LF are rejected.
- [x] `max_bytes + 1` payload bytes followed by CRLF are rejected across chunk boundaries.
- [x] `max_bytes + 1` payload bytes followed by EOF are rejected.
- [x] Oversized LF frames preserve the immediately following valid frame.
- [x] Oversized CRLF frames preserve the immediately following valid frame.
- [x] Multiple frames following an oversized frame remain intact.
- [x] Oversized unterminated input remains bounded in retained memory.
- [x] Draining uses `fill_buf()/consume()` and does not perform a read that can consume bytes past LF.
- [x] Existing JSONL/JSON-RPC behavior outside the size boundary is unchanged.

## Unicode source

- [x] `scripts/generate_confusables.py` no longer contains `/security/latest/` as the pinned source.
- [x] The configured source is the verified official version-specific Unicode Security 17.0.0 `confusables.txt` URL.
- [x] `UNICODE_SECURITY_VERSION` remains `17.0.0`.
- [x] The expected SHA-256 is verified against the official versioned source before output is written.
- [x] Header version verification still occurs before output is written.
- [x] Mismatch still fails without partially rewriting generated files.
- [x] The semantic confusables table remains 6565 entries.
- [x] Representative substitutions remain unchanged.
- [x] Two successive generator runs against the pinned source are deterministic.
- [x] Ordinary CI does not download Unicode data.
- [x] `scripts/release-check.sh` does not download Unicode data.

## Hygiene and diagnostics

- [x] `scripts/__pycache__/generate_confusables.cpython-312.pyc` is deleted from the repository.
- [x] Python cache/bytecode artifacts are ignored narrowly in `.gitignore`.
- [x] Running the generator does not leave trackable Python cache files.
- [x] Drive-relative scope diagnostics use the actual target drive/path.
- [x] A `D:foo` regression test proves the diagnostic does not report `C:foo`.
- [x] Drive-relative targets remain conservatively `inside_root == false`.

## Closure

- [x] `2026-08-07-corrective-runtime-soundness-and-boundaries.md` records the final CRLF correction SHA.
- [x] `2026-08-07-reproducibility-and-closure-correction.md` records the exact version-specific Unicode source and final corrective SHA.
- [x] The parent roadmap contains one concise final corrective-follow-up note.
- [x] No prior plan claims `/latest/` was removed while the generator still uses it.
- [x] Ordinary verification passes.
- [x] `scripts/release-check.sh` passes from a clean worktree, or a precise external blocker is recorded.
- [x] No new dependency, workflow, subsystem, tool, protocol surface, or release automation was added.
- [x] No additional closure/evidence plan is created after successful completion.

---

# Explicit non-goals

Do not use this pass to fix or reconsider:

- `current_eval_context` architecture beyond preserving the already-landed soundness fix;
- mutable dispatch semantics;
- calculator PRNG behavior;
- `rand()`/`random()` compatibility differences;
- unknown CLI flag handling;
- invalid diagnostics-format fallback;
- broader Windows device/extended path syntax;
- UNC architecture beyond regression preservation;
- request concurrency or cancellation semantics;
- response schema changes;
- tool budgets or timeout policy;
- test-thread strategy;
- CI cadence;
- dependency upgrades;
- dependency consolidation;
- binary-size optimization;
- release profile changes;
- Tokio runtime changes;
- crates.io automation;
- parity work unless the implementation unexpectedly changes calculator compatibility behavior.

If an unrelated issue appears while executing this plan, record it as a deferred finding only if it is reproducible and material. Do not expand this pass.

---

# Completion record

Fill only after implementation and verification are complete:

- **Implementation commit(s):** `324006f` (implementation and general documentation)
- **Reader regression test(s):** cross-fill oversized CRLF, cap+1 CRLF, exact-cap split CRLF, oversized CRLF with multiple following frames, and cap+1 EOF; 21 bounded-reader tests pass
- **Reader accounting design:** one saturating total for every byte before LF; bounded prefix retention; subtract only the final CR of CRLF
- **Reader frame-preservation result:** oversized LF/CRLF input drains through exactly LF; following one and multiple frames remain intact
- **Pinned Unicode source URL:** `https://www.unicode.org/Public/17.0.0/security/confusables.txt`
- **Pinned Unicode Security version:** `17.0.0`
- **Pinned SHA-256:** `091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a`
- **Confusables semantic diff:** none; two generator runs were byte-identical and parsed 6565 entries
- **Confusables entry count:** expected `6565`
- **Python cache cleanup:** tracked `.pyc` deleted; `__pycache__/` and `*.py[cod]` ignored
- **Drive-relative diagnostic correction:** `D:foo` reports `D:foo`, remains outside root, and never reports `C:foo`
- **Focused verification:** bounded reader 21 passed; path 62 passed; context isolation 50 passed
- **Ordinary verification:** fmt, clippy `-D warnings`, 3565 non-parity tests (1 ignored), 11 doc tests, and generate-docs check passed
- **Canonical release check:** passed from clean `main` after `96f6102`; cargo-deny, package verification, and publish dry-run passed with no publication
- **Prior corrective-plan reconciliation:** runtime and reproducibility records updated with `324006f` and the verified source URL
- **Parent roadmap final note:** added final corrective follow-up for residual CRLF accounting and source-pin closure
- **Deferred findings:** none expected
- **Final disposition:** complete after the clean canonical release check; remote CI verification follows the push

When every acceptance item is satisfied, mark this plan `complete`, record the implementation SHA(s), and stop this line of work.
