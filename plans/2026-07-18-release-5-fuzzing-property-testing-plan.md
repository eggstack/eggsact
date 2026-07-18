# Release 5 — Fuzzing and Property Testing Plan

## Purpose

Release 5 adds persistent fuzz corpora, targeted property tests, minimized regression fixtures, and reproducible triage workflows for eggsact’s parser-heavy and transformation-heavy surfaces.

This release does not replace ordinary unit, integration, parity, or protocol tests. Fuzzing explores malformed and adversarial input spaces; property tests assert invariants across broad generated input domains. Every discovered defect must be converted into a deterministic regression test before closure.

Release 5 depends on the final Releases 1–3 correctness closure and should use the Release 4 verification infrastructure for CI, dependency policy, package checks, and cross-platform validation.

## Release objective

At completion:

- parser-heavy public surfaces do not panic on arbitrary bounded input;
- transformations have explicit round-trip, idempotence, determinism, or monotonicity properties where applicable;
- fuzz targets enforce production input bounds rather than allocating without limit;
- persistent corpora are versioned and seeded with historical regressions;
- crashes and hangs have a documented reproduce/minimize/promote workflow;
- CI runs fast smoke fuzzing, while scheduled/manual jobs provide longer evidence;
- fuzz-only dependencies do not enter the published crate’s normal dependency graph.

## Non-goals

Release 5 must not:

- claim formal verification;
- fuzz network services or external hosts;
- fuzz unbounded input sizes;
- add arbitrary sleeps or nondeterministic timing assertions;
- make every ordinary PR run long fuzz campaigns;
- accept crashes as corpus entries without fixing or documenting them;
- add new tools;
- redesign parser APIs solely to make fuzzing convenient unless a small testability seam is justified;
- introduce unsafe code merely for fuzz throughput;
- publish fuzz harnesses as required runtime dependencies.

---

# Workstream 1 — Establish fuzz workspace and policy

## Tooling choice

Use `cargo-fuzz` with libFuzzer for coverage-guided fuzzing. Keep the fuzz package isolated under `fuzz/` so its dependencies do not affect normal consumers or package contents.

Suggested layout:

```text
fuzz/
  Cargo.toml
  fuzz_targets/
    calculator_expression.rs
    calculator_normalization.rs
    unified_diff.rs
    shell_tokenization.rs
    shell_quoting.rs
    regex_classification.rs
    regex_execution.rs
    json_pointer.rs
    toml_config.rs
    unicode_inspection.rs
    markdown_fences.rs
    glob_matching.rs
  corpus/
    <target>/
  artifacts/
    .gitignore
```

Do not commit raw crash artifacts containing uncontrolled large or sensitive data. Commit only minimized, reviewed regression seeds.

## Dependency isolation

- `libfuzzer-sys` belongs only in `fuzz/Cargo.toml`.
- Property-test dependencies such as `proptest` or `quickcheck` belong in root `dev-dependencies` only if used by normal test targets.
- Review all new dependencies through Release 4 cargo-deny policy.
- Exclude fuzz build outputs and crash artifacts from package contents.

## Input bounds

Each target must cap generated or interpreted input to realistic production limits before invoking expensive behavior.

Examples:

- expressions: no more than `MAX_EXPRESSION_LENGTH` characters;
- regex patterns: no more than `MAX_PATTERN_LENGTH`;
- regex text/sample totals: no more than production text limits;
- lists: no more than `MAX_LIST_ITEMS`;
- schema/config recursion: no more than production depth/element caps;
- Markdown and diff text: bounded to a small fuzz-specific size suitable for iteration speed.

Reject or truncate oversized fuzz input deterministically. Do not allow the fuzzer to spend most cycles in allocator exhaustion.

## Common assertions

Every target should assert as applicable:

- no panic;
- no process abort;
- no stack overflow under bounded input;
- no uncontrolled memory growth;
- no infinite loop or excessive timeout;
- deterministic result for identical input and fixed context;
- response serialization succeeds;
- returned spans and indices are valid UTF-8 or byte offsets according to contract;
- machine-code/error envelope invariants hold for tool-level surfaces.

## Documentation

Add `docs/fuzzing.md` covering:

- prerequisites;
- target list;
- local commands;
- corpus policy;
- crash reproduction;
- minimization;
- regression promotion;
- CI/scheduled job behavior;
- security-sensitive finding handling.

## Acceptance criteria

- `cargo fuzz list` succeeds.
- Every target builds.
- Fuzz-only dependencies are isolated.
- Corpus and artifact policy is documented.
- Normal `cargo package --list` excludes fuzz internals unless intentionally included.

---

# Workstream 2 — Calculator expression and normalization fuzzing

## Target A: calculator expression parser/evaluator

Feed bounded arbitrary UTF-8 expressions into the lowest practical parser/evaluator entry point with a deterministic `EvalContext`.

Test modes should include:

- default library context;
- MCP-safe context;
- seeded deterministic context;
- restricted side-effect/random settings.

Assertions:

- no panic or stack overflow;
- errors are structured and bounded in length;
- identical expression/context pairs produce identical results;
- failed parsing does not mutate the supplied context;
- evaluation failures follow documented transaction semantics;
- result strings and types are internally consistent;
- recursion/depth limits terminate gracefully.

Seed corpus with:

- historical calculator bug expressions;
- deeply nested parentheses;
- unary-operator chains;
- exponentiation chains;
- unit conversions;
- division edge cases;
- Unicode operators and confusables;
- random/memory syntax where supported;
- oversized but bounded numeric literals.

## Target B: normalization/token-preprocessing

Exercise any expression normalization or compatibility translation layer separately from evaluation.

Properties:

- normalization is deterministic;
- normalization is idempotent where contractually expected;
- normalized output remains valid UTF-8;
- output length is bounded relative to input;
- normalization does not introduce disallowed side-effect syntax;
- already canonical expressions remain unchanged where applicable.

## Property tests

Add generated tests for:

- integer addition/multiplication agreement with Rust checked arithmetic within safe ranges;
- parentheses preservation;
- whitespace invariance where grammar allows;
- decimal formatting stability;
- direct context isolation between separately seeded contexts;
- immutable template reproducibility.

## Acceptance criteria

- Both calculator targets run without crashes for the scheduled budget.
- Historical parser/evaluator regressions are corpus seeds.
- Transaction and determinism properties are covered by ordinary property tests.

---

# Workstream 3 — Unified diff and patch parser fuzzing

## Surfaces

Fuzz the lowest-level unified diff parser and the public patch preflight/apply-check wrapper separately.

Input dimensions:

- arbitrary text;
- malformed headers;
- missing newline at EOF markers;
- multiple files;
- rename-like metadata;
- overlapping hunks;
- negative/overflowing line numbers;
- very long paths;
- binary-looking content;
- Unicode paths and lines;
- CRLF and mixed line endings.

Assertions:

- no panic;
- bounded findings and output;
- hunk ranges never underflow/overflow;
- parsed spans stay within source input;
- malformed input returns deterministic structured failure;
- successful parse followed by canonical rendering, if supported, reparses equivalently;
- applying a generated valid patch to its matching source produces the expected target;
- reverse patch round-trip works where a reverse operation exists;
- path policy checks cannot be bypassed through normalization tricks.

## Property generators

Build a structured generator for valid small source/target line vectors, derive a diff, then verify:

1. patch preflight accepts the generated patch;
2. patch application yields the target;
3. generated hunk counts and ranges are consistent;
4. reversing source/target yields a patch that restores the original where supported.

Keep structured valid-input generation separate from arbitrary-byte malformed fuzzing.

## Corpus

Seed with:

- prior patch parsing bugs;
- empty patch;
- one-line add/delete;
- multi-hunk patch;
- no-newline markers;
- paths with spaces/tabs;
- traversal attempts;
- Windows separators;
- Unicode filenames.

## Acceptance criteria

- Malformed diff fuzzing produces no panic or unbounded work.
- Valid generated patches satisfy apply/round-trip properties.
- Every discovered parsing defect gains a minimized regression fixture.

---

# Workstream 4 — Shell tokenization and quoting fuzzing

## Scope

Fuzz shell parsing, tokenization, argv comparison, and quoting helpers for each explicitly supported shell/platform mode.

Do not claim full shell-language correctness if the tool intentionally supports a restricted grammar. Properties must match the documented subset.

## Inputs

- arbitrary command strings;
- quotes and escapes;
- empty arguments;
- whitespace variants;
- Unicode and control characters;
- shell metacharacters;
- variable-looking syntax;
- Windows backslashes and drive paths;
- POSIX paths;
- embedded newlines;
- very long arguments within bounds.

## Properties

Where quoting and parsing are inverse operations for the supported subset:

```text
parse(quote_argv(argv)) == argv
```

Additional properties:

- tokenization is deterministic;
- no token span exceeds source bounds;
- re-quoting a parsed safe argv is stable/canonical where documented;
- dangerous metacharacters are never silently dropped;
- empty strings remain representable;
- path separators are preserved under the selected platform mode;
- quoting one argument cannot merge it with adjacent arguments.

## Structured generators

Generate argv arrays from safe Unicode strings, excluding only values the target shell contract cannot represent. Test POSIX and Windows strategies separately.

## Acceptance criteria

- Supported quote/parse round trips pass generated property tests.
- Arbitrary malformed shell strings do not panic.
- Platform-specific behavior is explicitly separated in tests and corpora.

---

# Workstream 5 — Regex classification, safety, and execution fuzzing

## Target A: regex feature classification

Fuzz the `eggsact-regex` classifier independently.

Assertions:

- no panic on arbitrary UTF-8 pattern;
- escaped constructs and character-class contents are handled without out-of-bounds scanning;
- classification is deterministic;
- classifier output never claims a backend accepted a pattern unless compile fallback handles disagreement safely;
- known PCRE-only constructs produce stable unsupported classification where detectable.

Seed with:

- lookahead/lookbehind;
- escaped lookaround-like literals;
- nested character classes/escapes;
- backreferences;
- inline flags;
- branch reset;
- recursion/subroutines;
- control verbs;
- `\K`;
- malformed groups.

## Target B: regex compile and bounded execution

Feed bounded patterns and bounded text through the production regex tool surface.

Assertions:

- no panic;
- safety rejection precedes dangerous execution where intended;
- output match count and spans are bounded;
- spans are within text bounds and on valid boundaries according to the API contract;
- backend metadata reflects the actual engine used;
- unsupported features yield stable machine codes;
- deterministic repeated calls return equivalent results.

## Timeout handling

Fuzz jobs must not create unbounded abandonable regex workers. Use the production bounded execution boundary established by the correctness plan. Set per-input libFuzzer timeouts and production-level budgets.

Do not add unsafe patterns to the persistent corpus unless they are minimized and execute within the bounded harness.

## Property tests

For Rust-regex-compatible generated literals:

- escaping a literal and searching the original text finds that literal;
- match spans slice back to the matched string;
- `max_matches` is respected;
- case-insensitive behavior agrees with documented engine semantics for selected stable ASCII cases.

## Acceptance criteria

- Classifier and execution targets are separate.
- Fuzzing cannot exceed configured regex worker bounds.
- Backend metadata and span invariants are property-tested.

---

# Workstream 6 — JSON pointer and structured JSON fuzzing

## Scope

Fuzz JSON parsing helpers, JSON pointer extraction, canonicalization, shape analysis, and structured comparison at the lowest useful boundaries.

## Inputs

Use both:

- arbitrary bounded UTF-8 text as purported JSON;
- structured generated `serde_json::Value` trees with bounded depth and size;
- arbitrary JSON pointer strings.

## Properties

- no panic on malformed JSON or pointers;
- pointer extraction never reads outside the tree;
- pointer escaping `~0` and `~1` behaves consistently;
- canonicalization is deterministic;
- canonicalization is idempotent;
- canonicalized valid JSON reparses to an equivalent value;
- comparison is symmetric when options are symmetric;
- `compare(a, a)` is equal;
- shape summaries are deterministic and bounded;
- duplicate-key detection does not index beyond source text;
- maximum depth and element limits terminate with structured errors.

## Structured generators

Build recursive generators with explicit depth/size controls for:

- null/bool/number/string;
- arrays;
- objects with Unicode keys;
- keys containing `/`, `~`, dots, and empty strings;
- numeric edge values supported by `serde_json`.

## Acceptance criteria

- Canonicalization idempotence and reparse equivalence are property-tested.
- Pointer handling is fuzzed with escaped and malformed segments.
- Depth/element limits are exercised and stable.

---

# Workstream 7 — TOML and configuration parser fuzzing

## Scope

Fuzz TOML shape/inspection, dotenv validation, INI validation, and config auto-detection/preflight.

## Inputs

- arbitrary bounded UTF-8;
- mixed line endings;
- duplicate keys/sections;
- deeply nested TOML tables/arrays;
- dotted keys;
- malformed strings and escapes;
- dotenv export syntax;
- interpolation-looking syntax;
- caller-provided dotenv key regex;
- INI continuation/comment edge cases;
- ambiguous auto-detection inputs.

## Assertions

- no panic;
- bounded findings/output;
- duplicate handling follows selected policy;
- auto-detection is deterministic;
- validation result does not depend on hash-map iteration order;
- invalid caller regex is rejected before config scanning;
- unsafe regex is rejected with stable machine code;
- parser depth/size limits terminate;
- secret previews never expose full values.

## Property tests

- rendering a generated simple dotenv map and parsing it preserves keys/values within the documented grammar;
- duplicate-policy monotonicity: `allow` must not be stricter than `warn`, and `warn` must not be stricter than `error`, according to the exact response contract;
- generated simple INI documents retain section/key structure;
- TOML shape summaries are deterministic for equivalent parsed values.

## Acceptance criteria

- Each config format has direct malformed-input fuzz coverage.
- Auto-detection has a separate target or structured mode.
- Secret masking properties are explicitly tested.

---

# Workstream 8 — Unicode inspection and normalization fuzzing

## Scope

Fuzz text canonicalization, confusable/invisible inspection, normalization, grapheme counting, and safe representation.

## Inputs

- arbitrary valid UTF-8;
- combining-mark sequences;
- zero-width characters;
- bidi controls;
- variation selectors;
- emoji ZWJ sequences;
- regional indicators;
- invalid-looking but valid scalar sequences;
- long combining chains within bounds;
- mixed normalization forms.

## Properties

- NFC and NFKC normalization are idempotent;
- normalization output is valid UTF-8;
- grapheme positions remain within string bounds;
- safe representation is deterministic;
- safe representation does not emit raw invisible/bidi controls where contract says they are escaped;
- byte/codepoint/grapheme counts satisfy basic ordering/consistency constraints;
- canonicalization does not panic on unusual scalar sequences;
- inspection findings are bounded and stable;
- repeated calls produce identical ordering.

## Security properties

- bidi controls are always surfaced when present;
- zero-width/invisible characters are not silently lost from diagnostics;
- masked/escaped previews cannot recreate an unmarked dangerous string by concatenation where the contract intends visible markers.

## Acceptance criteria

- Unicode targets use valid UTF-8 generation rather than arbitrary invalid byte reinterpretation unless the API accepts bytes.
- Normalization idempotence is property-tested.
- Historical confusable/invisible bugs seed the corpus.

---

# Workstream 9 — Markdown fence and code-block fuzzing

## Scope

Fuzz Markdown structure extraction and fenced code-block parsing.

## Inputs

- arbitrary text;
- backtick and tilde fences;
- variable fence lengths;
- nested/adjacent fences;
- missing closing fences;
- language labels with punctuation/Unicode;
- CRLF/mixed endings;
- indented code;
- long lines;
- embedded fence-like content.

## Properties

- no panic;
- extracted ranges are ordered, non-overlapping where contract requires, and within source bounds;
- extracted code slices match reported content;
- unclosed fences return deterministic partial/error behavior;
- rendering/extracting generated well-formed fences preserves language and code content;
- output ordering is deterministic;
- maximum block/finding limits are respected.

## Acceptance criteria

- Arbitrary malformed Markdown is covered.
- Structured generated fences satisfy range/content round-trip properties.

---

# Workstream 10 — Glob and path matching fuzzing

## Scope

Fuzz glob parsing/matching and path normalization/classification for POSIX, Windows, and auto modes.

## Inputs

- arbitrary patterns and paths;
- separators;
- `.` and `..` segments;
- repeated separators;
- drive prefixes;
- UNC-like prefixes where supported;
- hidden files;
- Unicode;
- bracket expressions;
- recursive wildcards;
- escaped metacharacters;
- empty segments;
- long but bounded paths.

## Properties

- no panic;
- normalization is idempotent for canonical inputs;
- normalization never introduces traversal outside the represented root according to contract;
- matching is deterministic;
- literal-escaped patterns match the intended literal path;
- platform mode changes only documented separator/root semantics;
- batch and single-item matching agree;
- classification buckets are stable for normalized equivalent paths;
- path traversal findings cannot be bypassed through separator mixing or Unicode confusables where those are in scope.

## Acceptance criteria

- POSIX and Windows corpora are distinct where semantics differ.
- Normalization and batch/single consistency properties are covered.

---

# Workstream 11 — Corpus seeding and regression promotion

## Initial corpus sources

Seed each target from:

- existing unit/integration fixtures;
- historical bug reproductions in tests and changelog;
- Python parity edge cases;
- machine-code error cases;
- minimum/maximum boundary inputs;
- representative valid examples from documentation;
- malformed examples already asserted by tests.

Do not bulk-copy arbitrary external corpora without reviewing licensing and relevance.

## Crash workflow

For every crash, timeout, OOM, or invariant failure:

1. Preserve the raw artifact locally.
2. Reproduce with the exact target and commit.
3. Run `cargo fuzz tmin` or equivalent minimization.
4. Determine whether the finding is:
   - production bug;
   - harness bug;
   - expected resource rejection;
   - duplicate of known issue.
5. Fix the production or harness bug.
6. Add a deterministic ordinary regression test.
7. Add the minimized input to the persistent corpus if it improves coverage.
8. Record the machine code/contract change if externally visible.
9. Re-run the target and full test gate.

A fuzz target is not considered fixed merely because the input was filtered out. Filters must correspond to production preconditions or documented target scope.

## Corpus maintenance

- Keep corpus filenames content-addressed or otherwise stable.
- Remove redundant seeds only after coverage comparison.
- Keep minimized corpus size small enough for CI smoke runs.
- Do not commit multi-megabyte generated corpora without explicit justification.

## Acceptance criteria

- Every initial target has meaningful seeds.
- Historical regressions are represented.
- Crash triage and promotion workflow is documented and demonstrated on at least one test fixture or known historical case.

---

# Workstream 12 — CI and scheduled fuzz evidence

## Pull-request smoke fuzzing

Add a short smoke job that:

- builds all fuzz targets;
- runs a selected high-value subset for a small fixed duration or bounded run count;
- uses the committed corpus;
- uploads crash artifacts only on failure;
- has a strict overall timeout.

Suggested PR targets:

- calculator expression;
- unified diff;
- shell tokenization;
- regex classification;
- JSON pointer;
- Unicode inspection.

Keep total ordinary PR overhead controlled.

## Scheduled extended fuzzing

Add a scheduled and manually dispatchable workflow that runs all targets for longer budgets.

Record:

- commit SHA;
- target;
- duration;
- corpus size;
- executions;
- coverage counters when available;
- crashes/timeouts;
- toolchain and sanitizer configuration.

## Sanitizers

Where supported, add manual or scheduled sanitizer runs:

- AddressSanitizer for memory issues;
- UndefinedBehaviorSanitizer if any unsafe code exists or is introduced;
- LeakSanitizer where practical.

Do not claim sanitizer coverage on unsupported hosted platforms.

## Reproducibility

Pin or record:

- nightly toolchain used by cargo-fuzz;
- cargo-fuzz version;
- target revision;
- command-line options;
- per-input timeout;
- maximum input length;
- RSS/memory limit where supported.

## Security handling

If a fuzz finding has plausible security impact:

- avoid publishing exploit details before assessment;
- use the repository’s security reporting process;
- minimize and fix privately if required;
- add a public regression test only after disclosure policy permits.

## Acceptance criteria

- PR smoke fuzzing builds and runs selected targets.
- Scheduled/manual extended fuzzing covers every target.
- Toolchain and command metadata are recorded.
- Failures preserve reproducible artifacts without exposing secrets.

---

# Workstream 13 — Property-test suite organization

## Placement

Use ordinary Rust tests for properties that should run deterministically on every CI pass.

Suggested modules:

- `tests/property/test_calculator_properties.rs`
- `tests/property/test_diff_properties.rs`
- `tests/property/test_shell_properties.rs`
- `tests/property/test_regex_properties.rs`
- `tests/property/test_json_properties.rs`
- `tests/property/test_config_properties.rs`
- `tests/property/test_unicode_properties.rs`
- `tests/property/test_markdown_properties.rs`
- `tests/property/test_path_glob_properties.rs`

Register them through the repository’s integration-test structure.

## Case counts

- Keep default CI case counts sufficient to catch broad regressions without excessive runtime.
- Allow an environment variable or test configuration for extended local/scheduled case counts.
- Persist failing seeds in test output.
- Use deterministic seeded runners in CI where supported, while periodically varying seeds in scheduled workflows.

## Shrinking

Prefer property frameworks with effective shrinking. Failing output must include a minimal or reproducible case.

## Acceptance criteria

- Core algebraic/round-trip/idempotence properties run in ordinary CI.
- Failing seeds are reproducible.
- Property-test runtime remains within documented budgets.

---

# Suggested implementation sequence

1. Complete Releases 1–3 final correctness closure.
2. Complete Release 4 dependency and CI foundations.
3. Add `fuzz/` workspace and documentation.
4. Add common bounded-input helpers for fuzz targets.
5. Implement calculator, diff, shell, regex, JSON, and Unicode high-priority targets.
6. Seed corpora from historical tests.
7. Add corresponding ordinary property tests.
8. Implement TOML/config, Markdown, and glob/path targets.
9. Add PR smoke fuzz workflow.
10. Add scheduled/manual extended fuzz workflow.
11. Exercise crash minimize/promote process on known historical fixtures.
12. Run all targets, property tests, full CI, package checks, and documentation checks.

Recommended commit structure:

1. `test(fuzz): establish isolated cargo-fuzz workspace`
2. `test(fuzz): add calculator diff and shell targets`
3. `test(fuzz): add regex json and unicode targets`
4. `test(fuzz): add config markdown and glob targets`
5. `test(property): add round-trip and idempotence suites`
6. `ci: add fuzz smoke and scheduled campaigns`
7. `docs: document fuzz triage and corpus policy`

---

# Required local commands

Install reviewed tooling:

```bash
cargo install cargo-fuzz --locked --version <reviewed-version>
```

Build targets:

```bash
cargo fuzz list
cargo fuzz build
```

Run representative targets:

```bash
cargo fuzz run calculator_expression -- -max_total_time=60 -timeout=5
cargo fuzz run unified_diff -- -max_total_time=60 -timeout=5
cargo fuzz run shell_tokenization -- -max_total_time=60 -timeout=5
cargo fuzz run regex_classification -- -max_total_time=60 -timeout=5
cargo fuzz run json_pointer -- -max_total_time=60 -timeout=5
cargo fuzz run unicode_inspection -- -max_total_time=60 -timeout=5
```

Run property and ordinary gates:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features property
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo run --locked --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --verbose
```

Adapt `--locked` only if Release 4 deliberately chose a different documented policy.

---

# Release 5 closure criteria

Release 5 is complete only when:

- Releases 1–3 final correctness closure is complete.
- Release 4 verification infrastructure is green or its required foundation is otherwise present.
- Every planned fuzz target builds and runs against bounded input.
- Persistent corpora are committed and seeded with historical regressions.
- Calculator, diff, shell, regex, JSON, TOML/config, Unicode, Markdown, and glob/path surfaces have fuzz coverage.
- Core round-trip, idempotence, determinism, symmetry, transaction, and span-validity properties are enforced in ordinary tests.
- No known crash, hang, OOM, stack overflow, or invariant failure remains untriaged.
- Every fixed finding has a deterministic regression test.
- PR smoke fuzzing is active and bounded.
- Scheduled/manual extended fuzzing covers all targets with recorded toolchain and command metadata.
- Fuzz dependencies and artifacts are excluded from normal package/runtime dependencies.
- Fuzzing documentation explains reproduce, minimize, fix, promote, and security handling.
- Full ordinary CI, cargo-deny, generated docs, and package gates pass.

The implementing agent should leave a release status note containing target names, corpus seed counts, campaign durations, property-test modules, findings fixed, findings deliberately deferred with rationale, current CI evidence, and exact reproduction commands for any unresolved non-release-blocking issue.
