# Phase 1 — Regex and MCP Contract Repairs

## Status

- **Status:** ready for implementation
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-07-31-lightweight-correctness-simplification-roadmap.md`
- **Roadmap commit:** `795784519238d483f4474d91bad45658ea45f103`
- **Implementation baseline:** use the latest `origin/main` when execution begins
- **Scope:** repair one MCP Unicode panic path and make regex backend, flag, validity, and runtime-error reporting truthful
- **Expected change size:** small-to-medium, localized to MCP server helpers, regex core, regex tools, schemas, and focused tests

## Objective

Repair the highest-priority correctness findings without redesigning MCP or expanding regex scope.

After this phase:

1. a long Unicode request ID cannot panic the duplicate-ID error path;
2. the regex backend reported in output is the backend eggsact actually constructed and executed;
3. ASCII mode is not silently ignored;
4. matching-engine runtime failures are not converted into ordinary no-match or silently truncated success;
5. pattern syntax, backend support, eggsact policy, and execution status are represented accurately enough for an agent to choose its next action;
6. existing simple and fancy regex features remain available;
7. no PCRE2 dependency or new regex backend is introduced.

---

# Hard constraints

This phase must not:

- implement PCRE2;
- add an FFI dependency;
- broaden the supported regex dialect beyond what `regex` and `fancy-regex` already support;
- remove lookaround or backreference support already provided by `fancy-regex`;
- remove the existing regex safety check;
- replace MCP stdio transport;
- redesign request concurrency or timeout handling;
- add a new test harness;
- add fuzzing to ordinary CI;
- change release workflows;
- add a generic abstraction framework for two regex engines.

Use a small internal enum/helper. Do not create a trait hierarchy.

---

# Files to inspect first

At minimum inspect:

```text
src/mcp/server.rs
src/mcp/protocol.rs
src/mcp/machine_codes.rs
src/mcp/response.rs
src/text/regex_engine.rs
src/text/validate.rs
src/text/regex_safety.rs
src/tools/regex.rs
src/mcp/schemas/regex.rs
src/mcp/specs/regex.rs
tests/
architecture/mcp-server.md
architecture/text-library.md
docs/mcp-tools.md
```

Use repository search for:

```text
truncate_id_display
engine_used
RegexEngineUsed
regex_test
regex_finditer
validate_regex
captures_from_pos
ascii
valid_pattern
unsupported_features
REGEX_UNSUPPORTED_FEATURE
REGEX_UNSAFE
```

Do not assume all relevant tests are in one file.

---

# Workstream 1 — UTF-8-safe MCP diagnostic truncation

## Current defect

The duplicate request-ID path converts the ID to a string and slices it at a fixed byte offset. Rust string slicing requires a UTF-8 character boundary. A long string ID containing multibyte characters can therefore panic while the server is trying to report a duplicate ID.

## Required implementation

Create one small helper in the nearest existing MCP utility location, or keep it private in `server.rs` if no other production caller needs it.

Recommended contract:

```rust
fn truncate_utf8_bytes(input: &str, max_bytes: usize, suffix: &str) -> String
```

Required behavior:

- if `input.len() <= max_bytes`, return the input unchanged;
- otherwise reserve suffix bytes, move the content endpoint backward to a valid UTF-8 boundary, and append the suffix;
- never panic for valid UTF-8;
- never exceed the intended maximum by more than a consciously documented suffix convention;
- support very small limits without underflow;
- do not allocate more than one result string on the truncation path.

Use this helper in `truncate_id_display()`.

Review other direct string byte slices in the MCP request/error path. Replace only clearly equivalent unsafe diagnostic truncations found during this bounded inspection. Do not turn this phase into a repository-wide string utility refactor.

## Focused tests

Add unit tests for:

1. ASCII input below the limit;
2. ASCII input above the limit;
3. a long all-multibyte string where the nominal cut falls inside a code point;
4. mixed ASCII and multibyte input;
5. zero or suffix-sized limits if the helper permits them;
6. duplicate request-ID handling using a long Unicode string, proving a JSON-RPC error is produced rather than a panic.

Prefer testing the helper directly plus one integration-level request test. Do not add a subprocess test if the existing server test utilities can exercise the path.

## Acceptance criteria

- No direct `&string[..constant]` remains in the duplicate-ID display path.
- Long Unicode duplicate IDs produce a bounded error response.
- The server remains usable after the error in the integration-level test.
- Existing request-ID validation behavior remains unchanged.

---

# Workstream 2 — One regex classification/compilation path

## Current defect

The classifier chooses `rust-regex` or `fancy-regex`. `regex_finditer` follows that choice, but the validation/test path constructs `fancy_regex::Regex` unconditionally and reports the classifier result as `engine_used`. The output can therefore claim that eggsact executed `rust-regex` when it did not directly construct that backend.

## Required internal design

Add a minimal compiled representation in `src/text/regex_engine.rs` or the nearest existing regex core module.

Recommended shape:

```rust
enum CompiledRegex {
    Rust(regex::Regex),
    Fancy(fancy_regex::Regex),
}

struct RegexCompileOutcome {
    compiled: CompiledRegex,
    engine_used: RegexEngineUsed,
}
```

A separate outcome struct is optional if the enum can report its own engine.

Required helper responsibilities:

1. classify the unmodified user pattern;
2. reject explicitly unsupported PCRE-only constructs;
3. apply supported flags in one normalized step;
4. compile with the selected backend;
5. return the actual selected engine;
6. provide small match/capture operations needed by `regex_test` and `regex_finditer`.

Do not create a general regex trait. A `match` on a two-variant enum is clearer.

## Flag normalization

Move flag normalization into one function shared by validation and iteration.

It must define accepted spellings consistently. Preserve currently documented spellings unless correcting an inconsistency is necessary.

Examples to consider:

```text
IGNORECASE / I
MULTILINE / M
DOTALL / S
VERBOSE / X
```

Unknown flags must follow the existing public contract. Do not silently introduce additional aliases.

## Backend behavior

Required backend routing:

- patterns supported by Rust regex and not requiring fancy constructs use `regex::Regex`;
- patterns requiring supported lookaround/backreferences use `fancy_regex::Regex`;
- unsupported PCRE-only constructs are rejected before compilation with explicit unsupported-feature details;
- compilation errors identify the backend that attempted compilation;
- `engine_used` is populated from the compiled variant, not recomputed separately.

## Required tests

Use table-driven tests covering at least:

| Pattern | Expected engine | Expected behavior |
|---|---|---|
| `\d+` | `rust-regex` | matches digits |
| `(?i)hello` | `rust-regex` | case-insensitive match |
| `\d+(?=px)` | `fancy-regex` | lookahead works |
| `(?<=\$)\d+` | `fancy-regex` | lookbehind works |
| `(\w+)\1` | `fancy-regex` | backreference works |
| `(*SKIP)foo` | none | unsupported feature response |
| `(?>abc)` | none | unsupported feature response |
| malformed character class | selected compile path | compilation error |

Assert both matching behavior and reported `engine_used`.

## Acceptance criteria

- `regex_test` and `regex_finditer` use the same classification/compilation helper.
- No output path reports an engine from classification without using the compiled variant.
- Existing lookaround/backreference coverage remains functional.
- No third regex backend is added.

---

# Workstream 3 — Resolve ASCII-mode behavior

## Current defect

The public regex tool accepts `ascii`, returns it in `flags_used`, but the core implementation ignores the value.

## Required decision process

Attempt the smallest correct implementation first. The implementation must establish what ASCII mode means for eggsact.

Minimum intended semantics:

- `\w`, `\W`, `\d`, `\D`, `\s`, `\S`, and word boundaries use ASCII-oriented behavior when possible;
- case-insensitive matching must not claim Python-identical ASCII semantics unless tests establish it;
- both backends must behave consistently enough for the documented contract.

For Rust regex, scoped Unicode disabling such as `(?-u:...)` may be usable, but test boundary and casefold behavior rather than assuming equivalence.

For fancy-regex, determine whether the same syntax is accepted and behaves consistently. Use primary crate behavior and tests, not undocumented assumptions.

## Allowed outcomes

### Preferred outcome: implement

Implement one shared pattern wrapper or builder that applies ASCII semantics to both backends. Document exact limitations.

### Acceptable bounded outcome: reject

If consistent implementation across both engines is not possible without extensive rewriting, reject `ascii: true` before compilation with:

- a stable invalid/unsupported machine code already present, or one narrowly added code if necessary;
- a clear message that ASCII mode is not supported by the selected eggsact dialect;
- no `flags_used.ascii: true` success response.

Do not leave the option silently accepted.

## Tests

At minimum use non-ASCII examples that differentiate Unicode from ASCII behavior:

```text
é
١
β
ASCII_123
```

Test `\w`, `\d`, boundaries, and one case-insensitive example. Run the same contract through both a simple pattern and a fancy pattern where feasible.

## Acceptance criteria

- `ascii: true` changes behavior according to documented semantics or returns an explicit unsupported response.
- The result never echoes successful ASCII application when none occurred.
- `ascii: false` remains backward compatible.

---

# Workstream 4 — Preserve runtime errors as errors

## Current defect

Fancy-regex operations are fallible at runtime. Some current paths convert a runtime error into `matches: false`, discard capture errors, or break iteration and return accumulated results without identifying the failure.

## Required result model

Do not collapse these states:

```text
pattern did not match
pattern failed to compile
pattern is unsupported
pattern was blocked by policy
pattern execution failed
pattern matched successfully
```

Use existing response structures where possible. Add the minimum fields required for truthful results.

Recommended additions to core result structs:

```rust
pub execution_error: Option<String>
pub policy_allowed: Option<bool>
pub syntax_valid: Option<bool>
pub supported: Option<bool>
```

The exact fields may differ to preserve compatibility. Avoid introducing redundant fields when existing `valid_pattern`, `error`, `unsupported_features`, and machine codes can express the state accurately.

A practical compatibility approach is:

- retain `valid_pattern` as syntax/backend-compilation validity;
- use `unsupported_features` for unsupported constructs;
- return `error` for compile or runtime errors;
- add `policy_allowed` only if needed to distinguish safety/complexity rejection;
- set a non-OK tool response or machine code for runtime execution failure;
- never return `matches: false` as the sole representation of an engine error.

## Runtime handling requirements

For `regex_test`:

- an error from `find`, `is_match`, or `captures` must end that tool call with an explicit execution error;
- do not fabricate a no-match result for the affected sample;
- identify the sample index without echoing an unbounded sample body.

For `regex_finditer`:

- an error from `captures_from_pos` must be surfaced;
- partial matches may be retained only if the response clearly states that execution failed after partial progress;
- `truncated` must remain reserved for caller-requested match limits, not engine failure;
- prevent zero-length-match loops as currently intended.

Sanitize engine errors through existing error sanitization/length limits.

## Machine codes

Reuse existing codes when semantically correct. Search `machine_codes.rs` before adding anything.

Possible mapping:

```text
unsupported construct -> REGEX_UNSUPPORTED_FEATURE
safety/policy block   -> REGEX_UNSAFE
runtime engine error  -> INTERNAL_ERROR or a narrow REGEX_EXECUTION_ERROR if an existing generic code would misroute callers
invalid syntax        -> INVALID_ARGUMENTS
```

Add a new machine code only if the routing distinction is materially useful and update the generated/documented code table through existing mechanisms.

## Tests

Add focused tests that force a fallible fancy-regex execution path. Prefer a known low backtrack-limit pattern/input supported by the crate's configuration rather than sleeps or timing thresholds.

Assert:

- runtime failure is not represented as `matches: false` success;
- runtime failure is not represented as ordinary truncation;
- engine and error fields are populated;
- output remains bounded;
- a later normal call still succeeds.

If reliably forcing the library runtime error is not possible without modifying global crate configuration, test the internal error-mapping helper directly and retain an existing fuzz regression path. Do not add timing-flaky CI tests.

---

# Workstream 5 — Separate syntax, support, policy, and execution status

## Goal

Make the response truthful without performing a broad response-schema redesign.

## Required semantic definitions

Document these terms in code comments and `architecture/text-library.md`:

- **syntax valid:** the selected backend can compile the pattern after supported flag transformation;
- **supported:** eggsact recognizes no explicitly unsupported dialect construct;
- **policy allowed:** complexity and safety policy permit execution;
- **execution successful:** all requested matching operations completed without engine failure;
- **matched:** a specific sample or iteration produced a match.

## Minimal compatibility strategy

Retain existing fields and refine their meaning:

- `valid_pattern` means the pattern compiled successfully for the selected backend;
- `unsupported_features` explains unsupported dialect constructs;
- `error` carries compile or runtime failure;
- tool-level `ok` and `machine_code` identify policy/runtime failure;
- `results[*].matches` is used only after successful engine execution for that sample.

If complexity checks currently occur before backend compilation, classify their failure explicitly as policy rejection rather than asserting invalid syntax. This may require a `policy_allowed` field in the result or a tool-level policy error envelope.

Do not add five new booleans if two existing fields plus one new field express the contract.

## Acceptance criteria

- Documentation no longer equates safety-policy rejection with invalid syntax.
- Callers can distinguish unsupported, invalid, unsafe, runtime-failed, no-match, and matched outcomes.
- Existing consumers that inspect `valid_pattern`, `error`, and `results` are not needlessly broken.

---

# Schema and documentation updates

Update the existing regex input/output schemas to match the implemented behavior.

Required schema checks:

- ASCII behavior is documented accurately;
- `engine_used` enum remains accurate;
- error/unsupported fields are present where returned;
- any new policy/runtime field has a description;
- no field is documented as Python/PCRE compatible beyond tested behavior.

Regenerate documentation through the existing generator. Do not hand-edit generated blocks.

Update only affected prose:

```text
architecture/mcp-server.md
architecture/text-library.md
docs/mcp-tools.md or generated equivalent
README.md only if the public regex summary is currently inaccurate
```

---

# Execution sequence for a smaller implementation agent

Follow exactly this order:

1. fetch and inspect latest `origin/main`;
2. run existing focused MCP and regex tests;
3. add the UTF-8-safe truncation helper and its unit tests;
4. add the duplicate Unicode ID regression test;
5. create the minimal compiled-regex enum/helper;
6. migrate `regex_test` to the helper;
7. migrate `regex_finditer` to the helper;
8. verify backend reporting tests;
9. implement or explicitly reject ASCII mode;
10. correct runtime-error propagation;
11. refine validity/policy response semantics with the smallest compatible schema change;
12. update schemas and affected documentation;
13. run generated-doc check;
14. run focused tests;
15. run full local verification;
16. commit once with a concise summary and update this plan's completion record.

Do not start workstream 4 before backend unification passes tests. Otherwise error handling will be duplicated.

---

# Required verification

## Focused

Use the exact existing test target names discovered during inspection. At minimum run tests covering:

```text
MCP request validation and duplicate IDs
regex classifier
regex validation tool
regex finditer tool
regex schemas/registry contracts
```

## Full

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

Run Python parity only if this phase changes a behavior covered by parity. Record the command and result; do not add parity to ordinary CI.

---

# Acceptance checklist

- [ ] Long Unicode duplicate request IDs cannot panic.
- [ ] Diagnostic truncation is UTF-8 safe.
- [ ] Simple regex patterns construct and report `rust-regex`.
- [ ] Supported fancy patterns construct and report `fancy-regex`.
- [ ] Unsupported PCRE-only constructs remain explicit.
- [ ] Validation and iteration share classification/compilation logic.
- [ ] ASCII mode is implemented or explicitly rejected.
- [ ] ASCII mode is never silently echoed as applied.
- [ ] Runtime regex errors are not converted into ordinary no-match.
- [ ] Runtime regex errors are not mislabeled as truncation.
- [ ] Safety/complexity policy is not mislabeled as syntax invalidity.
- [ ] Existing regex functionality remains available.
- [ ] No PCRE2/FFI dependency was added.
- [ ] Generated documentation is current.
- [ ] Full local verification passes.

---

# Completion record

Fill only after implementation.

- **Status:** pending
- **Implementation commit:** pending
- **Focused tests:** pending
- **Full verification:** pending
- **ASCII disposition:** pending
- **Response-schema changes:** pending
- **Deferred findings:** pending

Do not create a separate evidence-only plan. Record concise closure here.