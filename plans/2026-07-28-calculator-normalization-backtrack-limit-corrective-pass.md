# Calculator Normalization Backtrack-Limit Corrective Pass

## Status

- **Status:** ready for implementation
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `b30e220bb2b0d2e58e7d271e157375de8ae7e810`
- **Invalidated release code baseline:** `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
- **Affected release:** `1.2.0` release candidate
- **Scope:** calculator normalization regex runtime-error handling, minimized regression coverage, fuzz-target clarity, and exact-SHA evidence reset
- **Publication:** out of scope; crates.io publication and tag creation remain direct maintainer actions

## Purpose

The runtime lifecycle, cancellation, synchronous execution pool, mutable execution context, and their deterministic tests are complete. Do not reopen those areas.

This pass addresses one newly discovered release-blocking calculator finding:

- exact-SHA fuzz workflow run `30306975485` executed against `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`;
- the `calculator_normalization` matrix produced crash artifact `8669774633`;
- GitHub archive digest: `sha256:939204248936a12eb4935dbeb97572ed4af17912f3427f450aad40bd566b0239`;
- the recorded minimized input is `32E73 33`;
- the observed failure was reported as `fancy-regex` `BacktrackLimitExceeded` during calculator normalization;
- the finding is real until reproduced and classified otherwise;
- the earlier successful fuzz run `30287151564` does not override a later exact-SHA crash finding.

The existing public calculator path is:

```text
calc::run
  -> normalize
  -> split_at_operators
  -> preprocess_units
  -> add_same_unit_division_parens
  -> evaluator
```

`normalize()` already returns `Result<String, String>` and `run()` maps normalization failures to `RunError::Internal`. However, the normalization implementation uses several input-driven `fancy_regex::Regex` operations through APIs such as `replace_all()` or direct `unwrap()` calls. In `fancy-regex 0.18`, infallible replacement APIs panic when runtime matching reports an error; `try_replacen(..., 0, ...)` is the fallible all-matches replacement API.

The goal is not merely to special-case one input. The goal is to ensure that user-controlled calculator text cannot turn a bounded regex runtime error into a process panic anywhere in the calculator normalization pipeline.

---

# Required outcome

After this pass:

1. the exact crash artifact is reproducible on the old code baseline or its prior failure is precisely explained;
2. the exact regex stage and operation responsible for the failure are identified;
3. `calc::run("32E73 33")` and `run_with_context("32E73 33", ...)` never panic;
4. the input produces the intended deterministic success or structured error according to calculator semantics and Python parity;
5. all input-driven `fancy-regex` execution errors reachable through calculator normalization are propagated rather than panicked or silently discarded;
6. the minimized input is committed as a unit/integration regression and a persistent fuzz corpus seed;
7. the calculator-normalization fuzz target distinguishes production panic, deterministic success, deterministic error, and inconsistent outcomes;
8. a new code SHA is selected;
9. ordinary CI, release verification, extended fuzz/sanitizer, latest-compatible dependencies, and Python parity all pass against that exact new SHA;
10. Release 5 and release-readiness evidence no longer claim that `3e5b41c` has no untriaged fuzz findings.

---

# Non-goals and hard constraints

This pass must not:

- redesign MCP handler lifecycle accounting;
- modify the synchronous execution pool;
- alter cancellation or timeout semantics;
- change mutable-context commit behavior;
- migrate the entire repository to a different regex engine;
- change regex backend selection outside calculator normalization;
- increase or disable the `fancy-regex` backtrack limit as the sole fix;
- hide the finding with `catch_unwind` in production;
- treat `BacktrackLimitExceeded` as a successful non-match;
- silently return partially normalized text after a regex execution error;
- remove or weaken the `calculator_normalization` fuzz target;
- delete the crash artifact or omit the failed run from release evidence;
- reduce fuzz duration to make the target appear green;
- publish to crates.io;
- create the `v1.2.0` tag;
- update release evidence before a new implementation SHA and complete exact-SHA workflow set exist.

`catch_unwind` is acceptable in regression tests to prove the public API does not panic. It is not an acceptable production error-handling boundary for regex runtime failures.

A higher backtrack limit may be evaluated only as diagnostic evidence. It must not be committed unless the root cause demonstrates that the current limit rejects a legitimate bounded operation, the chosen increase has a documented upper bound, and the underlying pattern cannot be made predictably cheaper. Prefer pattern correction, linear-backend use for non-fancy patterns, or structured error propagation.

---

# Current implementation facts to preserve

The implementation agent should preserve these contracts:

- `normalize(expr)` enforces `MAX_TEXT_LENGTH` before normalization work;
- `run(expr)` maps normalization failures to `RunError::Internal`;
- `run_with_context(expr, ctx)` follows the same normalization path;
- evaluator errors remain `RunError::Evaluation`;
- successful calculator output remains `(String, String)`;
- Python/eggcalc parity remains authoritative for accepted public input semantics;
- compile-time static regex construction may continue to use `Regex::new(...).unwrap()` when the pattern is repository-owned and covered by startup/tests;
- user-input-driven regex execution may not panic;
- error handling must be deterministic across repeated identical calls;
- no raw user expression needs to be included in an internal regex-error message.

---

# Required execution sequence

Execute in this order:

1. freeze and inspect the current repository state;
2. download and verify the exact crash artifact;
3. reproduce through both libFuzzer and direct public API calls;
4. determine whether the panic occurs on the first call, second call, or fuzz assertion;
5. identify the exact normalization stage and regex operation;
6. determine intended semantics through existing tests and Python parity;
7. implement a fallible regex execution boundary in calculator normalization;
8. repair the exact pathological pattern if successful evaluation is required;
9. add unit, integration, context, deterministic-repeat, and fuzz-corpus regression coverage;
10. audit calculator normalization for equivalent input-driven panic sites;
11. run focused tests and repeated reproduction loops;
12. run the full local release gate and MSRV gate;
13. commit implementation and tests as a new frozen `CODE_SHA`;
14. rerun all final workflows against the exact new SHA;
15. update release evidence once, accurately recording both the old failed finding and the new successful closure evidence.

Do not mark the plan complete before step 15.

---

# Workstream 1 — Freeze baseline and reopen release status

## Required baseline records

Record:

```text
PLAN_BASELINE=b30e220bb2b0d2e58e7d271e157375de8ae7e810
OLD_CODE_SHA=3e5b41c6ac5a8daaba11d5dfacb822f6da033464
FAILED_FUZZ_RUN=30306975485
CRASH_ARTIFACT_ID=8669774633
CRASH_ARCHIVE_SHA256=939204248936a12eb4935dbeb97572ed4af17912f3427f450aad40bd566b0239
```

Before editing implementation, confirm no newer source/test/fuzz/workflow commit has landed after the plan baseline:

```bash
git fetch origin main --prune
git log --oneline --decorate -20 origin/main
git diff --name-status b30e220bb2b0d2e58e7d271e157375de8ae7e810..origin/main
```

If a newer implementation-relevant commit exists, inspect it first and rebase this plan’s assumptions onto that code. Do not blindly implement against stale line numbers.

## Reopen tracked closure state

At the first documentation update associated with implementation, ensure tracked status no longer states that the release is fully closed.

Required eventual wording:

- Release 5: `reopened — calculator normalization backtrack-limit finding`;
- release readiness: `hold pending calculator normalization corrective pass`;
- prior single-SHA evidence pass: implementation/evidence completed for `3e5b41c`, subsequently invalidated for release by exact-SHA fuzz run `30306975485`;
- old successful fuzz run remains historical evidence, not final closure.

Do not create a documentation-only status commit before implementation unless repository process requires it. Prefer including status correction with the implementation/evidence sequence so history does not accumulate another evidence loop.

## Acceptance criteria

- The actual current implementation baseline is known.
- The failed run and artifact identities are preserved.
- No release document continues to present the finding as untriaged but non-blocking.
- No work proceeds from stale source assumptions.

---

# Workstream 2 — Reproduce the artifact exactly

## Download and verify

Download artifact `8669774633` from run `30306975485`:

```bash
gh run download 30306975485 \
  --name fuzz-crashes-calculator_normalization \
  --dir /tmp/eggsact-calculator-normalization-crash

find /tmp/eggsact-calculator-normalization-crash -type f -print
find /tmp/eggsact-calculator-normalization-crash -type f -print0 \
  | sort -z \
  | xargs -0 shasum -a 256
```

If downloading the ZIP through the API instead, separately record:

- GitHub archive digest;
- downloaded ZIP checksum;
- extracted crash filename;
- extracted crash-file checksum;
- exact byte length;
- hex dump and UTF-8 rendering.

Verify the artifact’s payload rather than relying only on the prose record:

```bash
xxd -g 1 <CRASH_FILE>
python3 - <<'PY'
from pathlib import Path
p = Path("<CRASH_FILE>")
data = p.read_bytes()
print(repr(data))
try:
    print(data.decode("utf-8"))
except UnicodeDecodeError as exc:
    print(exc)
PY
```

Expected human-readable payload is currently reported as:

```text
32E73 33
```

If the extracted bytes differ, use the artifact bytes as authoritative and correct the documentation.

## Reproduce with libFuzzer

From a clean checkout of `OLD_CODE_SHA`:

```bash
git worktree add /tmp/eggsact-old-fuzz "$OLD_CODE_SHA"
cd /tmp/eggsact-old-fuzz

RUST_BACKTRACE=1 \
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
cargo fuzz run calculator_normalization <CRASH_FILE> -- -runs=1
```

Also run multiple direct reproductions:

```bash
for i in $(seq 1 20); do
  RUST_BACKTRACE=1 \
  RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
  cargo fuzz run calculator_normalization <CRASH_FILE> -- -runs=1 || true
done
```

Record whether the crash reproduces 20/20, intermittently, or not at all.

## Reproduce through public APIs

Create a temporary, uncommitted diagnostic test or example that invokes each call independently under `catch_unwind`:

```rust
let first = std::panic::catch_unwind(|| eggsact::calc::run("32E73 33"));
let second = std::panic::catch_unwind(|| eggsact::calc::run("32E73 33"));

println!("first: {first:?}");
println!("second: {second:?}");
```

Also exercise:

```rust
let mut ctx = eggsact::calc::EvalContext::default();
let contextual = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    eggsact::calc::run_with_context("32E73 33", &mut ctx)
}));
```

The diagnostic must establish which of these occurred:

1. the first production `run()` call panicked;
2. the first call returned `Ok`, then the second production call panicked;
3. both production calls returned results, but the fuzz target’s assertion panicked;
4. a production call returned `Err`, and the fuzz target’s second-call `unwrap()` obscured the real contract;
5. reproduction depends on process state, iteration order, or another corpus input.

Do not assume the existing evidence prose has already answered this distinction.

## Required evidence

Capture:

- complete panic message;
- top relevant backtrace frames;
- whether the panic comes from `Regex::replace_all`/`replacen`, direct `unwrap`, target assertion, or another location;
- first-call and second-call outcomes;
- `run_with_context` outcome;
- elapsed time for the minimized input;
- reproduction rate.

## Acceptance criteria

- The exact artifact bytes are known.
- The old SHA behavior is reproduced or a precise non-reproduction explanation is recorded.
- Production panic and fuzz-harness panic are distinguished.
- First-call versus second-call behavior is known.
- The responsible source frame is identified before implementation begins.

---

# Workstream 3 — Identify the exact normalization stage

## Do not patch by guesswork

`src/calc/normalize.rs` performs many regex transformations. The failing input must be traced to a named stage.

Use one of these diagnostic techniques:

### Preferred: test-only stage wrapper

Temporarily route each fallible operation through a diagnostic wrapper that records a stable stage name before execution:

```rust
fn diagnostic_regex_stage<T>(
    stage: &'static str,
    op: impl FnOnce() -> Result<T, fancy_regex::Error>,
) -> Result<T, fancy_regex::Error> {
    eprintln!("normalization regex stage: {stage}");
    op()
}
```

Do not commit unconditional `eprintln!` output.

### Alternative: binary-search the pipeline

Temporarily stop normalization after successive groups of transformations and reproduce until the first failing group is isolated. Then narrow to the individual regex.

### Audit commands

Run:

```bash
rg -n '\.replace_all\(|\.replacen\(|try_replacen' src/calc/normalize.rs
rg -n '\.(find|captures|is_match)\([^;]*\)\.(unwrap|expect)\(' src/calc/normalize.rs
rg -n '\.(find|captures|is_match)\([^;]*\)\.ok\(\)' src/calc/normalize.rs
rg -n 'fancy_regex::|use fancy_regex' src/calc/normalize.rs
```

For the exact failing regex, record:

- static name;
- pattern string;
- whether it requires a fancy feature;
- operation used (`find`, `captures`, `replace_all`, iterator, etc.);
- runtime error variant;
- why `32E73 33` reaches it;
- whether scientific notation, whitespace-separated numeric tokens, unit aliases, or another transformation is involved;
- whether the regex should match at all;
- whether the operation succeeds under the linear `regex` crate with equivalent semantics.

## Pattern classification

Classify the identified pattern into one of these categories:

### Category A — No fancy syntax required

If the pattern uses no lookaround, backreference, atomic group, conditional, or other fancy-only feature, consider compiling that one pattern with `regex::Regex` instead.

Requirements:

- prove equivalent captures/replacements for existing tests;
- do not migrate unrelated patterns;
- preserve case, Unicode, and boundary behavior;
- add a regression showing the minimized input completes without backtracking failure.

### Category B — Fancy syntax required but pattern is pathological

Rewrite the specific pattern to reduce ambiguity/backtracking while preserving behavior.

Possible techniques, only when applicable:

- anchor the pattern more tightly;
- replace nested ambiguous quantifiers;
- split one complex pattern into deterministic lexical checks plus a simpler regex;
- reorder alternations from specific/long to general/short;
- use an atomic group where semantics permit;
- parse scientific notation or token boundaries directly rather than through an ambiguous expression;
- avoid scanning every start position when a prefix test can reject immediately.

Every pattern rewrite requires focused positive, negative, boundary, Unicode, and parity tests.

### Category C — Legitimate bounded runtime error

If the input is intentionally outside the supported grammar and the backtrack limit is the correct bounded outcome, propagate it as a deterministic normalization error rather than panicking.

The public contract should be:

```text
Err(RunError::Internal(<stable bounded-regex message>))
```

or another already-established non-breaking error variant. Do not add a breaking public error enum change solely for this pass.

## Acceptance criteria

- One exact regex stage is named as root cause.
- The source operation and runtime error are documented.
- The pattern’s backend requirements are understood.
- The implementation strategy is selected from evidence, not assumption.
- No global regex-engine or backtrack-limit change is made without necessity.

---

# Workstream 4 — Establish intended calculator semantics

The input `32E73 33` resembles a scientific-notation value followed by another numeric token. Do not invent the intended result.

## Required semantic checks

Run the latest supported Python/eggcalc implementation with the exact bytes.

Record:

- success or error;
- normalized intermediate form, if observable;
- final value string;
- final type string;
- exception class/message if rejected;
- repeated-call determinism.

Inspect existing Eggsact behavior for adjacent forms:

```text
32E73
32e73
32E+73
32E-73
32E73 33
32e73 33
32E73 + 33
32E73 * 33
1e2 3
1e2 + 3
```

Also inspect number-word combination behavior for two ordinary numeric tokens, because normalization currently combines some consecutive number runs.

## Contract decision

Use this decision order:

1. preserve documented Eggsact behavior;
2. preserve accepted Python parity;
3. preserve existing adjacent regression semantics;
4. when the input is genuinely ambiguous and Python rejects it, return a deterministic structured error;
5. never choose a successful value merely to make the fuzz test pass.

Record the chosen contract in the regression test name and assertion.

Examples:

```rust
#[test]
fn scientific_notation_followed_by_numeric_token_matches_python() {
    assert_eq!(run("32E73 33"), <expected>);
}
```

or:

```rust
#[test]
fn ambiguous_scientific_notation_sequence_returns_bounded_error() {
    let result = run("32E73 33");
    assert!(matches!(result, Err(RunError::Internal(_))));
}
```

## Acceptance criteria

- The expected result is based on parity/documented semantics.
- `run` and `run_with_context` agree.
- The test asserts a specific outcome, not merely “did not panic.”
- Repeated identical calls produce the same outcome and message.

---

# Workstream 5 — Add a fallible calculator-regex boundary

## Core defect class

The calculator normalization path must not use an infallible `fancy-regex` execution API on user-controlled text when the corresponding fallible API exists.

For `fancy-regex 0.18`:

- `replace_all()` delegates to infallible replacement behavior;
- `replacen()` panics on matching errors;
- `try_replacen(text, 0, replacement)` performs replace-all behavior while returning runtime errors.

## Required helper

Introduce one private, stable error mapper in `src/calc/normalize.rs`:

```rust
fn normalization_regex_error(stage: &'static str, err: fancy_regex::Error) -> String {
    format!("calculator normalization regex stage '{stage}' failed: {err}")
}
```

Do not include the full user expression in this message.

Use either a helper function or a small private macro to avoid duplicating conversion logic. Illustrative function shape:

```rust
fn try_replace_all<'t, R>(
    stage: &'static str,
    re: &Regex,
    text: &'t str,
    replacement: R,
) -> Result<String, String>
where
    R: fancy_regex::Replacer,
{
    re.try_replacen(text, 0, replacement)
        .map(|value| value.into_owned())
        .map_err(|err| normalization_regex_error(stage, err))
}
```

Adjust generic/lifetime details to the actual crate API. A macro is acceptable if closure replacers make the function signature awkward, but the macro must remain local, readable, and type-safe.

Example macro shape:

```rust
macro_rules! replace_all_checked {
    ($stage:literal, $regex:expr, $text:expr, $replacement:expr) => {{
        $regex
            .try_replacen($text, 0, $replacement)
            .map(|value| value.into_owned())
            .map_err(|err| normalization_regex_error($stage, err))?
    }};
}
```

## Required propagation

At minimum, convert the exact failing stage from infallible execution to a propagated error.

Then audit every input-driven `fancy-regex` operation reachable from:

- `normalize()`;
- `normalize_lowercase_temperature_conversion()`;
- `binary_word_check()`;
- `preprocess_units()`;
- `add_same_unit_division_parens()`;
- `run()`;
- `run_with_context()`.

Required rules:

1. `replace_all()` and `replacen()` on user text must be replaced with a fallible call.
2. `find(...).unwrap()` and `captures(...).unwrap()` on user text must propagate errors.
3. `.ok().flatten()` must not silently convert a backtrack-limit error into “no match” when it changes normalization semantics.
4. iterator items that are `Result` must be handled explicitly.
5. static `Regex::new(...).unwrap()` for repository-owned patterns may remain.
6. parse/capture indexing assumptions may remain only when the regex statically guarantees the capture; prefer `get()` when a closure can safely fall back.

## Signature changes

Contained helper signature changes are acceptable:

```rust
fn normalize_lowercase_temperature_conversion(expr: &str) -> Result<String, String>;

pub fn preprocess_units(
    tokens: &[String],
) -> Result<(Vec<String>, Option<String>), String>;

pub fn add_same_unit_division_parens(expr: &str) -> Result<String, String>;
```

If changing public hidden/helper signatures would create unnecessary compatibility churn, keep public wrappers and introduce private fallible implementations used by `run()`/`run_with_context()`. However, do not retain a public-input path that can panic merely to avoid an internal refactor.

Both `run()` and `run_with_context()` must use the same fallible path.

## Error contract

Map regex runtime errors through the existing normalization boundary:

```rust
normalize(...).map_err(RunError::Internal)
```

The message must be:

- deterministic;
- bounded in size;
- clear enough to diagnose the stage;
- free of full raw user input;
- consistent between `run` and `run_with_context`.

If the exact input should succeed, the pattern/backend fix must prevent the runtime error. The fallible boundary still remains as defense in depth.

## Prohibited implementation shortcuts

Do not:

```rust
let value = std::panic::catch_unwind(|| re.replace_all(...));
```

Do not:

```rust
re.find(text).unwrap_or(None)
```

Do not:

```rust
re.captures(text).ok().flatten()
```

when an execution error must be distinguished from no match.

Do not simply increase the global backtrack limit.

## Acceptance criteria

- The exact failing operation no longer panics.
- All user-input-driven infallible replacement calls in calculator normalization are removed.
- All input-driven `fancy-regex` errors are handled explicitly.
- Static pattern-construction unwraps are the only accepted regex unwrap class.
- `run` and `run_with_context` share the same error behavior.
- No production `catch_unwind` is added for this defect.

---

# Workstream 6 — Repair the exact pathological pattern when required

If parity or documented semantics require `32E73 33` to succeed, merely returning `BacktrackLimitExceeded` is insufficient.

## Required pattern repair process

1. minimize the responsible regex independently of the normalization pipeline;
2. create a focused test with the exact pattern and input;
3. demonstrate the old pattern reaches `BacktrackLimitExceeded`;
4. implement the smallest semantics-preserving rewrite;
5. verify all existing positive/negative tests for that transformation;
6. add adversarial variants around the minimized input;
7. measure repeated execution to ensure the new pattern does not approach the configured backtrack limit.

## Backend selection

If the pattern does not require fancy syntax, prefer `regex::Regex` for that one static. The repository already depends on `regex`.

Example approach:

```rust
use regex::Regex as LinearRegex;

static SCIENTIFIC_TOKEN_RE: LazyLock<LinearRegex> =
    LazyLock::new(|| LinearRegex::new(...).unwrap());
```

Do not alias both regex types ambiguously. Use clear names such as:

```rust
use fancy_regex::{Captures as FancyCaptures, Regex as FancyRegex};
use regex::Regex as LinearRegex;
```

Only introduce aliases if the local file needs both engines.

## Pattern-specific tests

Test at least:

- exact crash input;
- lowercase and uppercase exponent markers;
- explicit exponent sign;
- negative exponent;
- adjacent whitespace;
- multiple spaces/tabs;
- malformed exponent;
- unit-like suffixes;
- boundary at maximum supported input length;
- Unicode adjacent text;
- Python parity examples.

## Acceptance criteria

- A legitimate input no longer returns a backtrack-limit error.
- Pattern semantics remain covered by focused tests.
- The pattern uses the simplest backend that supports its syntax.
- No global backend or limit change is introduced.
- Adversarial variants complete deterministically.

---

# Workstream 7 — Correct the fuzz target

## Current target ambiguity

The current target effectively performs:

```rust
if let Ok(result1) = run(expr) {
    let result2 = run(expr).unwrap();
    assert_eq!(result1, result2);
}
```

This makes a second-call error appear as an unwrap panic and does not clearly distinguish:

- production panic;
- deterministic `Ok`;
- deterministic `Err`;
- `Ok` then `Err`;
- `Err` then `Ok`;
- differing successful values;
- differing error variants/messages.

## Required target structure

Invoke both calls independently and require that neither panics:

```rust
let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(expr)));
let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(expr)));

assert!(first.is_ok(), "first run panicked");
assert!(second.is_ok(), "second run panicked");

let first = first.unwrap();
let second = second.unwrap();
```

Then compare outcomes explicitly:

```rust
match (first, second) {
    (Ok(a), Ok(b)) => {
        assert_eq!(a, b);
        assert!(std::str::from_utf8(a.0.as_bytes()).is_ok());
        assert!(a.0.len() <= expr.len() * 1000 + 10_000);
    }
    (Err(a), Err(b)) => {
        assert_eq!(std::mem::discriminant(&a), std::mem::discriminant(&b));
        assert_eq!(a.to_string(), b.to_string());
    }
    (left, right) => {
        panic!("non-deterministic calculator outcome: {left:?} vs {right:?}");
    }
}
```

If deriving `PartialEq`/`Eq` for `RunError` is semantically safe and non-breaking, direct comparison is acceptable. Do not change the public error type solely for test convenience if discriminant/message comparison suffices.

The production-panic assertion must remain. Do not convert a production panic into an ignored fuzz input.

## Persistent corpus

Copy the minimized artifact bytes into:

```text
fuzz/corpus/calculator_normalization/<descriptive-or-hash-name>
```

Use the exact bytes from the artifact.

Add a short corpus note only if repository convention supports it; otherwise record the run/artifact relationship in release evidence and commit message.

## Acceptance criteria

- The target reports production panic distinctly.
- Deterministic errors are valid fuzz outcomes.
- `Ok`/`Err` instability fails clearly.
- Successful value/type instability fails clearly.
- The exact minimized bytes are committed to the corpus.
- Existing output-bound and valid-UTF-8 properties remain.

---

# Workstream 8 — Add deterministic regression tests

## Required unit regression

Add a test near calculator normalization/run tests using the exact artifact input.

The test must:

1. call `run()` under `catch_unwind`;
2. assert the call did not panic;
3. assert the exact intended success or error contract;
4. repeat the call and assert identical outcome;
5. exercise `run_with_context()` and assert equivalent behavior.

Illustrative shape:

```rust
#[test]
fn calculator_normalization_backtrack_artifact_is_bounded_and_deterministic() {
    const INPUT: &str = "32E73 33";

    let first = std::panic::catch_unwind(|| run(INPUT));
    assert!(first.is_ok(), "run must not panic for fuzz regression input");
    let first = first.unwrap();

    let second = std::panic::catch_unwind(|| run(INPUT));
    assert!(second.is_ok(), "repeated run must not panic");
    let second = second.unwrap();

    assert_same_run_outcome(&first, &second);
    assert_expected_contract(first);

    let mut ctx = EvalContext::default();
    let contextual = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_with_context(INPUT, &mut ctx)
    }));
    assert!(contextual.is_ok());
    assert_same_run_outcome(&second, &contextual.unwrap());
}
```

Use repository-local helper conventions rather than introducing a broad test framework.

## Required direct normalization regression

Test `normalize()` independently so failures are not confused with evaluator behavior.

Assert either:

- exact normalized output; or
- exact error class/stage substring.

## Required repeat stress

Run at least 1,000 calls in one process:

```rust
#[test]
fn calculator_normalization_backtrack_artifact_repeats_deterministically() {
    let expected = snapshot_run_outcome(run("32E73 33"));
    for _ in 0..1_000 {
        assert_eq!(snapshot_run_outcome(run("32E73 33")), expected);
    }
}
```

The test must not use sleep.

## Required adjacent cases

Add a compact table-driven test covering the semantic cases selected in Workstream 4.

## Error-message test

When the chosen result is an error, assert:

- correct `RunError` variant;
- stable stage identifier;
- mention of execution/backtrack limit where appropriate;
- no full raw input echo;
- bounded message length.

## Acceptance criteria

- Exact artifact input is covered outside fuzzing.
- Public APIs do not panic.
- Direct normalization is covered.
- Repeated outcomes are deterministic.
- Context and non-context paths agree.
- Adjacent scientific-notation cases retain expected behavior.
- No sleep-based correctness test is added.

---

# Workstream 9 — Audit equivalent calculator-normalization panic paths

This is a narrow audit of `src/calc/normalize.rs`, not a repository-wide regex rewrite.

## Required audit inventory

Create a temporary checklist of every input-driven `fancy-regex` execution site under the public calculator path.

Classify each as:

- fallible and propagated;
- infallible but converted in this pass;
- static compile only;
- deliberately non-match-on-error, with written justification;
- unreachable from public input, with written justification.

## Required zero-result checks

After implementation, these commands should return no unsafe input-driven sites:

```bash
rg -n '\.replace_all\(|\.replacen\(' src/calc/normalize.rs
rg -n '\.(find|captures|is_match)\([^;]*\)\.(unwrap|expect)\(' src/calc/normalize.rs
```

A result is allowed only when it is clearly static construction or test-only and has an adjacent justification. `replace_all()` on user text is not allowed because its runtime-error behavior is infallible.

Also review:

```bash
rg -n '\.(find|captures|is_match)\([^;]*\)\.ok\(\)' src/calc/normalize.rs
rg -n 'unwrap_or\(false\)' src/calc/normalize.rs
```

For each result, determine whether swallowing a runtime error can alter user-visible normalization. Propagate errors when it can.

## Scope boundary

Do not expand into unrelated modules unless the same helper is directly called by calculator normalization and can panic on this input path.

Do not change the general regex validation/execution tools in this pass.

## Acceptance criteria

- The calculator normalization path has a complete execution-site inventory.
- No input-driven infallible fancy replacement remains.
- No input-driven fancy match error is unwrapped.
- Silent error-to-no-match conversion is justified or removed.
- Static regex construction remains simple and unchanged where safe.

---

# Workstream 10 — Focused verification

Run from a clean working tree after implementation.

## Formatting and compile

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
```

## Focused tests

Use actual test names after implementation:

```bash
cargo test --locked --all-features --lib calculator_normalization_backtrack
cargo test --locked --all-features --lib calc::normalize
cargo test --locked --all-features --test lib calculator
```

## Repeated regression

```bash
for i in $(seq 1 100); do
  cargo test --locked --all-features --lib calculator_normalization_backtrack || exit 1
done
```

No retries after failure. Investigate the first failure.

## Old artifact replay

```bash
RUST_BACKTRACE=1 \
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
cargo fuzz run calculator_normalization <CRASH_FILE> -- -runs=1000
```

The command must complete without a crash artifact.

## Corpus replay

```bash
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
cargo fuzz run calculator_normalization -- -runs=0
```

Use the repository’s established corpus-replay invocation if different.

## Short focused fuzz session

```bash
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
cargo fuzz run calculator_normalization -- -max_total_time=300
```

Run the ASan build and, if supported by repository workflow, a short ASan fuzz session:

```bash
RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build --sanitizer=address
```

## MSRV focused gate

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib calculator_normalization_backtrack
```

## Acceptance criteria

- Focused regression passes 100/100.
- Exact artifact replay passes 1,000 runs.
- Five-minute focused fuzzing produces no crash.
- ASan fuzz target builds.
- MSRV focused check/test passes.
- Clippy has zero warnings.

---

# Workstream 11 — Full local release gate

After focused verification passes, run the canonical release gate:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo run --locked --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

Run MSRV:

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

Build all fuzz targets:

```bash
RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build
RUSTUP_TOOLCHAIN=nightly-2026-05-07 cargo fuzz build --sanitizer=address
```

Use a clean worktree for the final local proof:

```bash
NEW_CODE_SHA=$(git rev-parse HEAD)
git worktree add /tmp/eggsact-normalization-closure "$NEW_CODE_SHA"
cd /tmp/eggsact-normalization-closure

test "$(git rev-parse HEAD)" = "$NEW_CODE_SHA"
test -z "$(git status --porcelain)"
```

The worktree must remain clean after verification.

## Acceptance criteria

- Full stable gate passes.
- Full MSRV gate passes.
- Package and publish dry run pass.
- All 12 fuzz targets build normally and under ASan.
- Exact test counts are recorded.
- Clean worktree remains clean.

---

# Workstream 12 — Freeze the new code SHA

## Commit composition

The implementation commit may include only the files necessary for this correction, expected to be a subset of:

```text
src/calc/normalize.rs
fuzz/fuzz_targets/calculator_normalization.rs
fuzz/corpus/calculator_normalization/**
tests/** or calculator-local test modules
```

A small documentation note explaining the reopened finding may be included if repository convention requires it, but do not mix final workflow evidence into the code commit.

Suggested commit message:

```text
fix(calc): bound normalization regex runtime errors
```

Record the full 40-character SHA as `NEW_CODE_SHA`.

## New baseline rule

Any later change to:

- source;
- tests;
- fuzz targets;
- fuzz corpus;
- workflows;
- manifest;
- lockfile;

creates a new code baseline and invalidates workflow evidence collected before that change.

## Acceptance criteria

- One exact implementation SHA contains the complete fix and regression coverage.
- The old `3e5b41c` baseline is no longer described as release-ready.
- No evidence workflow is dispatched before the implementation SHA is frozen.

---

# Workstream 13 — Exact-SHA workflow rerun

Create an immutable verification branch pointing exactly to `NEW_CODE_SHA`.

```bash
git branch verification/normalization-closure "$NEW_CODE_SHA"
git push origin verification/normalization-closure
```

Confirm the remote ref equals the full SHA.

Run all required workflows against that exact ref:

1. ordinary CI;
2. release verification;
3. extended fuzz plus all sanitizer jobs;
4. latest-compatible dependencies;
5. Python parity.

## Extended fuzz requirements

The final extended fuzz run must include:

- all 12 fuzz matrix targets;
- all seven sanitizer targets;
- `calculator_normalization` success;
- no uploaded calculator-normalization crash artifact;
- no missing, skipped, cancelled, or neutral required job;
- exact `head_sha == NEW_CODE_SHA`.

Because the finding was intermittent or stochastic on the old baseline, one green replay of the minimized input is insufficient. The full scheduled/manual duration must pass.

If the workflow supports a longer manual duration, run the repository-standard extended duration rather than reducing it.

## Release verification requirements

The provenance artifact must record `NEW_CODE_SHA` and current package metadata.

## Parity requirements

- zero unaccepted failures;
- accepted differences explicitly reported;
- report artifact retained and checksummed;
- exact `head_sha == NEW_CODE_SHA`.

## Failure policy

If any workflow reveals another code/test/fuzz/workflow defect:

1. do not rerun until green without investigation;
2. preserve the failed run and artifact;
3. fix the defect;
4. create a new SHA;
5. restart all five final workflows.

Environmental reruns are acceptable only for documented runner, network, GitHub service, or artifact-service failures unrelated to repository behavior.

## Acceptance criteria

- All five workflow families pass on one exact SHA.
- All required jobs and matrix entries are accounted for.
- No calculator-normalization crash artifact is produced.
- Provenance and parity artifacts name the exact SHA.
- Failed historical run `30306975485` remains recorded.

---

# Workstream 14 — Release evidence correction

After every exact-SHA workflow succeeds, update in one documentation commit:

- `docs/release-5-status.md`;
- `docs/release-readiness.md`;
- `docs/releases/2026-07-final-closure-evidence.md`;
- `docs/release-4-status.md` only if its shared exact-SHA workflow table must move to the new baseline;
- `plans/2026-07-27-final-single-sha-evidence-only-closure-pass.md` status;
- this plan’s status.

## Required historical record

Preserve:

```text
Failed exact-SHA fuzz run: 30306975485
Old head SHA: 3e5b41c6ac5a8daaba11d5dfacb822f6da033464
Crash artifact: 8669774633
Artifact digest: sha256:939204248936a12eb4935dbeb97572ed4af17912f3427f450aad40bd566b0239
Finding: calculator normalization fancy-regex runtime backtrack limit
Resolution commit: <NEW_CODE_SHA>
Regression seed: <path>
```

Do not call the old finding non-blocking after a corrective implementation was required.

## Required final evidence

For each workflow record:

- run ID;
- immutable URL;
- full head SHA;
- event/ref;
- conclusion;
- every required job conclusion;
- attempt number;
- artifact identities and digests.

For release provenance record:

- artifact ID;
- artifact name;
- GitHub archive digest;
- extracted filename;
- extracted-file SHA-256;
- package version;
- Rust stable version;
- MSRV;
- lockfile checksum;
- package count semantics;
- exact `NEW_CODE_SHA`.

For parity record:

- artifact ID/name;
- archive digest;
- extracted filename/checksum;
- eggsact version;
- eggcalc version;
- Python version;
- zero unaccepted failures.

## Avoid another evidence loop

Make one final documentation commit after all immutable runs and artifacts are known.

Then require ordinary CI on that documentation commit, but do not create another commit solely to record that CI run ID. Report the evidence-commit CI operationally in the handoff summary.

## Acceptance criteria

- Release 5 no longer contradicts the known finding.
- New exact-SHA workflow set replaces old final evidence.
- Old run remains historical and clearly failed.
- One documentation commit records final immutable evidence.
- No self-referential current-head field is added.
- Publication remains manual.

---

# Required test matrix

## Exact regression

| Case | Required assertion |
|---|---|
| exact artifact bytes | no panic; exact intended result/error |
| first repeated call | same outcome as baseline assertion |
| second repeated call | same variant, value/type or message |
| 1,000 in-process repetitions | identical outcomes, no panic |
| `run_with_context` | equivalent outcome |
| direct `normalize` | exact normalized value or stage error |

## Scientific notation adjacency

| Input | Required source of truth |
|---|---|
| `32E73` | existing semantics/parity |
| `32e73` | existing semantics/parity |
| `32E+73` | existing semantics/parity |
| `32E-73` | existing semantics/parity |
| `32E73 33` | exact chosen regression contract |
| `32e73 33` | same case-folded contract |
| `32E73 + 33` | explicit operator semantics |
| `1e2 3` | adjacent-number semantics |
| malformed exponent | deterministic structured error |

## Regex boundary

| Operation class | Required behavior |
|---|---|
| fallible replace | `Result` propagated |
| fallible find/captures | error distinct from no match |
| static compile | compile-time/startup unwrap allowed |
| backtrack limit | structured error, never panic |
| legitimate input pattern | succeeds without limit exhaustion |

## Fuzz

| Gate | Requirement |
|---|---|
| exact artifact replay | 1,000 runs, zero crashes |
| focused target | five minutes, zero crashes |
| corpus replay | zero crashes |
| full extended matrix | 12/12 success |
| sanitizer matrix | 7/7 success |

---

# Explicit acceptance criteria

## Root cause

- [ ] Crash artifact `8669774633` is downloaded and checksummed.
- [ ] Exact artifact bytes are recorded.
- [ ] Old-SHA behavior is reproduced or precisely explained.
- [ ] First-call, second-call, and fuzz-target behavior are distinguished.
- [ ] Exact regex static/stage/operation is identified.
- [ ] Runtime error variant is recorded.

## Semantics

- [ ] Python/eggcalc behavior is recorded.
- [ ] Intended Eggsact result/error is explicitly selected.
- [ ] Adjacent scientific-notation behavior remains correct.
- [ ] `run` and `run_with_context` agree.

## Implementation

- [ ] No production panic occurs for the artifact input.
- [ ] No input-driven `replace_all`/infallible `replacen` remains in calculator normalization.
- [ ] Input-driven find/capture errors are not unwrapped.
- [ ] Backtrack-limit errors are not treated as no match.
- [ ] Existing `RunError` contract is preserved unless a clearly non-breaking addition is justified.
- [ ] No production `catch_unwind` masks the defect.
- [ ] No global backtrack-limit increase is used as the sole fix.
- [ ] Exact pathological pattern is repaired if the input should succeed.

## Regression coverage

- [ ] Unit regression uses exact artifact bytes.
- [ ] Direct `normalize` regression exists.
- [ ] `run` regression exists.
- [ ] `run_with_context` regression exists.
- [ ] 1,000-call deterministic repeat test passes.
- [ ] Adjacent scientific-notation table test exists.
- [ ] Persistent fuzz corpus seed is committed.
- [ ] Fuzz target compares success and error outcomes explicitly.
- [ ] Fuzz target separately asserts no production panic.

## Verification

- [ ] Focused test passes 100/100.
- [ ] Exact artifact fuzz replay passes 1,000 runs.
- [ ] Five-minute focused fuzz run passes.
- [ ] Stable full gate passes.
- [ ] MSRV `1.89.0` gate passes.
- [ ] All fuzz targets build normally and under ASan.
- [ ] Package and publish dry run pass.
- [ ] Clean worktree remains clean.

## Exact-SHA closure

- [ ] New implementation SHA is frozen.
- [ ] Ordinary CI passes on the new SHA.
- [ ] Release verification passes on the new SHA.
- [ ] Extended fuzz passes 12/12 on the new SHA.
- [ ] Sanitizers pass 7/7 on the new SHA.
- [ ] Latest-compatible passes on the new SHA.
- [ ] Python parity passes on the new SHA.
- [ ] Provenance artifact records the new SHA.
- [ ] No calculator-normalization crash artifact is generated.

## Documentation

- [ ] Failed run `30306975485` remains documented.
- [ ] Artifact `8669774633` remains documented.
- [ ] Release 5 no longer claims no untriaged finding on `3e5b41c`.
- [ ] Old successful fuzz evidence is historical only.
- [ ] Final workflow records all use one new SHA.
- [ ] Evidence commit is documentation-only.
- [ ] No follow-up commit is created solely to record evidence-commit CI.
- [ ] crates.io publication is not performed.
- [ ] release tag is not created.

---

# Stop conditions

Stop and report rather than weakening the fix if:

- the artifact bytes cannot be recovered;
- the old failure cannot be reproduced and no source-level explanation can establish the panic path;
- intended semantics conflict between documented behavior and Python parity;
- fixing the pattern would require a broad calculator grammar redesign;
- a dependency upgrade is required and changes MSRV or public behavior;
- the only proposed fix is to raise/disable the backtrack limit;
- the fix converts errors into silent non-matches;
- regression tests remain scheduler- or timing-dependent;
- any final workflow runs against a SHA other than the frozen new baseline;
- extended fuzz or sanitizer produces any untriaged crash/hang/OOM/overflow;
- Python parity has an unaccepted difference;
- package/publish dry run fails;
- closure would require automated crates.io publication.

When a stop condition occurs, preserve the exact evidence and create a separate plan rather than broadening this pass silently.

---

# Definition of done

This corrective pass is complete only when all of the following are true:

1. the old fuzz artifact is reproduced and root-caused;
2. the exact affected regex stage is known;
3. calculator normalization uses fallible input-driven `fancy-regex` execution;
4. `32E73 33` has an explicit parity-backed contract;
5. `run`, `run_with_context`, and `normalize` do not panic for the minimized input;
6. repeated identical calls are deterministic;
7. the exact bytes are committed as a fuzz corpus seed;
8. the fuzz target distinguishes panic, success, error, and inconsistent outcomes;
9. equivalent calculator-normalization panic sites are audited and closed;
10. focused, full stable, MSRV, package, and fuzz-build gates pass;
11. a new implementation SHA is frozen;
12. all five final workflow families pass on that exact SHA;
13. all 12 fuzz and seven sanitizer jobs pass without new artifacts;
14. release provenance and parity artifacts identify the exact new SHA;
15. Release 5 and release-readiness documents accurately record the old failure and new resolution;
16. one documentation-only evidence commit lands without another self-referential update loop;
17. crates.io publication and tag creation remain direct maintainer actions.

Until all seventeen conditions hold, describe Eggsact as implementation-strong but release-blocked by the calculator-normalization fuzz finding.