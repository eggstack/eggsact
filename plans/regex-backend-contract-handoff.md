# Regex Backend Contract and Compatibility Handoff Plan

## Context

The regex tools should remain simple for coding agents. Agents should not be expected to decide whether a pattern needs Rust `regex`, `fancy-regex`, PCRE2, or any other engine. The tool contract should be: callers provide a pattern, text or samples, and flags; eggsact selects the safest compatible backend automatically; the response reports which backend was used and why a pattern was rejected when it is outside the supported dialect.

The current implementation already moves in this direction. `validate_regex` routes through `regex_test`, which compiles with `fancy_regex::Regex`. `regex_finditer` tries to use the standard Rust `regex` crate for simple patterns and falls back to `fancy-regex` for patterns detected as needing lookaround/backtracking support. The shape is correct, but the contract is not fully documented, the output does not make backend selection visible, and the feature detector appears too narrow and likely mishandles lookbehind routing.

This plan is a handoff for clarifying and hardening regex backend behavior before release.

## Goals

1. Preserve a simple agent-facing API: no required backend/engine input.
2. Implement deterministic automatic backend selection inside eggsact.
3. Document the supported regex dialect precisely: eggsact is not PCRE2, but supports Rust `regex` plus selected `fancy-regex` constructs.
4. Return explicit backend/engine metadata in regex tool output so failures and performance characteristics are explainable.
5. Fix `regex_finditer` backend classification so lookbehind and other extended constructs route correctly.
6. Add regression tests that lock the combined dialect and rejection behavior.
7. Keep ReDoS and resource safety constraints intact.

## Non-goals

This pass should not add a PCRE2 dependency or claim full PCRE2 compatibility.

This pass should not require agents to pass a backend selector during ordinary tool use.

This pass should not remove Rust `regex` from the fast path. Standard Rust `regex` remains preferable for patterns it supports because it is fast, linear-time, and safer.

This pass should not weaken existing complexity limits, timeout behavior, or safety checks merely to accept more PCRE-like patterns.

## Desired user-facing contract

The regex tools should be documented as using an eggsact-managed regex dialect:

`eggsact-regex` is an auto-selected backend layer. It uses Rust `regex` for linear-compatible regular expressions and `fancy-regex` for supported extended constructs such as lookaround. It is not a PCRE2 implementation and does not guarantee PCRE2 compatibility.

The default input schema should remain simple. Callers should provide the existing fields: `pattern`, `samples` or `text`, flags, limits, and output options. Backend selection should default to `auto` internally.

An optional debug/advanced selector may be added later if useful, but it should not be required. If added, it should be explicitly optional and default to automatic selection. Suggested enum: `auto`, `rust-regex`, `fancy-regex`. Do not expose this as part of the normal agent contract unless there is a concrete use case.

## Implementation plan

### 1. Introduce a small backend classification layer

Add a dedicated internal classifier, for example in `src/text/validate.rs` or a new `src/text/regex_engine.rs` module.

Suggested internal types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegexEngineUsed {
    RustRegex,
    FancyRegex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexFeature {
    LookAhead,
    LookBehind,
    Backreference,
    NamedCapture,
    InlineFlags,
    UnsupportedPcreConstruct,
}

#[derive(Debug, Clone)]
pub struct RegexClassification {
    pub preferred_engine: RegexEngineUsed,
    pub features: Vec<RegexFeature>,
    pub unsupported_features: Vec<String>,
}
```

The initial classifier does not need to be a complete parser. It should be a conservative scanner that identifies constructs that are known to require `fancy-regex` or are known unsupported. It should handle escapes and character classes correctly enough to avoid obvious false positives.

Minimum constructs to classify:

- Lookahead: `(?=...)`, `(?!...)` -> `FancyRegex`.
- Lookbehind: `(?<=...)`, `(?<!...)` -> `FancyRegex`.
- Inline flags: `(?i)`, `(?m)`, `(?s)`, `(?x)` -> allowed; backend depends on remaining pattern.
- Named captures accepted by supported backend syntax -> allowed and tested.
- Backreferences such as `\1`, `\k<name>`, or `(?P=name)` -> classify explicitly; confirm current `fancy-regex` support with tests before documenting as supported.
- PCRE-only constructs should become structured unsupported-dialect failures, not opaque compile errors where reasonably detectable. Examples: branch reset `(?|...)`, recursion/subroutine constructs such as `(?R)`, `(?1)`, `(?&name)`, `\K`, callouts, and PCRE control verbs such as `(*SKIP)` / `(*PRUNE)` / `(*ACCEPT)`.

Do not overfit this classifier to every regex dialect feature. The point is reliable auto-routing and clearer errors for common agent-generated patterns.

### 2. Fix lookbehind detection in the current routing path

The current `needs_fancy_regex`-style logic should be replaced or corrected. It appears to detect `(?=` and `(?!` lookahead, but likely fails on lookbehind because it peeks at `<` without advancing to the following `=` or `!`.

Acceptance criteria:

- `regex_finditer(r"(?<=\$)\d+", "$100 €200", ...)` routes to `fancy-regex` and returns the `$100` match span for `100`.
- `regex_finditer(r"(?<!\$)\d+", "$100 €200", ...)` routes to `fancy-regex` and matches the non-dollar-prefixed number.
- Escaped strings such as `\(\?<=literal` do not force fancy routing.
- Lookbehind-like text inside a character class does not force fancy routing.

### 3. Preserve fast-path semantics

For simple patterns, `regex_finditer` should continue to use Rust `regex`. This is important for deterministic performance and avoids invoking a backtracking engine unnecessarily.

Acceptance criteria:

- Plain patterns like `r"\b[a-z_][a-z0-9_]*\b"`, `r"^foo"`, and `r"(foo)-(bar)"` report `engine_used = "rust-regex"`.
- Patterns with lookahead/lookbehind report `engine_used = "fancy-regex"`.
- If Rust `regex` rejects a pattern that the classifier thought was Rust-compatible, the tool may attempt a secondary `fancy-regex` compile if doing so is safe and useful. If it does, the output must report the actual engine used.
- If both engines reject the pattern, return `valid_pattern = false` and an actionable error.

### 4. Add backend metadata to regex outputs

Extend `RegexTestResult` and `RegexFindIterResult` with backend metadata. Keep serde defaults/backwards compatibility where practical.

Suggested fields:

```json
{
  "engine_used": "rust-regex" | "fancy-regex",
  "dialect": "eggsact-regex",
  "unsupported_features": [],
  "feature_notes": []
}
```

For `validate_regex`, which currently routes through `regex_test`, the result should include `engine_used = "fancy-regex"` unless that path is refactored to also use auto-selection.

Preferred end state: factor `regex_test` and `regex_finditer` through the same classifier and auto-selection helper so both tools share behavior and metadata.

Acceptance criteria:

- Successful regex responses include `dialect` and `engine_used`.
- Unsupported dialect failures include `unsupported_features` where classification can identify them.
- Existing fields remain present: `valid_pattern`, `results` or `matches`, `error`, `flags_used`, `truncated`, and `match_count` as applicable.
- Existing callers that ignore unknown fields continue to work.

### 5. Clarify machine-code behavior

Unsupported regex dialect should be distinguishable from unsafe regex and invalid arguments.

Current behavior has `REGEX_UNSAFE` for safety/timeouts and generic invalid pattern errors from compilation. Consider adding a machine code such as `REGEX_UNSUPPORTED_FEATURE` or `REGEX_UNSUPPORTED_DIALECT` if one does not already exist.

Suggested classification:

- Invalid JSON/tool arguments -> existing invalid arguments code.
- Pattern exceeds size/complexity limits -> existing input too large or regex safety/complexity code.
- Pattern is unsafe/ReDoS risk -> existing regex unsafe code.
- Pattern times out -> existing regex unsafe/timeout code.
- Pattern syntax is invalid for all supported engines -> regex invalid pattern code, if available; otherwise add one.
- Pattern uses known unsupported PCRE-only features -> new unsupported dialect/feature code.

Acceptance criteria:

- Agent callers can programmatically distinguish “try a simpler safer pattern” from “this PCRE2 feature is not implemented.”
- Documentation lists machine-code meanings for regex failures.

### 6. Update documentation

Update README and architecture/tool reference docs so the regex contract is explicit.

Add a short section near the regex tool documentation:

- eggsact does not expose PCRE2.
- eggsact does not require callers to choose a backend.
- eggsact auto-selects Rust `regex` or `fancy-regex` based on pattern features.
- Rust `regex` is preferred for compatible patterns.
- `fancy-regex` is used for supported extended constructs.
- Some PCRE2 constructs are unsupported and return structured errors.
- Outputs report `dialect` and `engine_used`.

Suggested wording:

> Regex tools use the `eggsact-regex` dialect. The implementation auto-selects the safest compatible backend: Rust `regex` for linear-compatible patterns and `fancy-regex` for supported extended constructs such as lookaround. This is not PCRE2. Agents do not need to select a backend; outputs report the backend used and return structured errors for unsupported constructs.

Also update generated tool docs if README content is generated. If the generator owns the tool table or schema text, update the source metadata rather than hand-editing generated blocks.

### 7. Add regression tests

Add tests at both the lower-level text API and MCP/tool wrapper layer.

Minimum test matrix:

Simple Rust-compatible patterns:

- `r"\d+"` over `abc123`.
- `r"^foo"` with multiline flag behavior.
- Captures and named captures.
- Unicode span behavior with non-ASCII text.

Extended supported patterns:

- Positive lookahead: `r"\d+(?=px)"`.
- Negative lookahead: `r"\d+(?!px)"`.
- Positive lookbehind: `r"(?<=\$)\d+"`.
- Negative lookbehind: `r"(?<!\$)\d+"`.

Routing correctness:

- Simple pattern reports `rust-regex` in `regex_finditer`.
- Lookaround pattern reports `fancy-regex`.
- `validate_regex` reports the backend it actually used.
- Escaped lookaround-like literals do not force fancy routing.
- Character-class lookaround-like literals do not force fancy routing.

Unsupported PCRE-like constructs:

- Branch reset `(?|a)` should fail with unsupported dialect/feature if detected.
- Recursion `(?R)` or subroutine `(?1)` should fail with unsupported dialect/feature if detected.
- `\K` should fail with unsupported dialect/feature if detected.
- Control verb such as `(*SKIP)` should fail with unsupported dialect/feature if detected.

Safety behavior:

- Existing nested quantifier rejection remains intact.
- Timeout protection remains intact.
- Max pattern and max input limits remain intact.

### 8. Keep public API compatibility in mind

If adding fields to serialized result structs, ensure existing Rust callers are not broken unnecessarily. Adding public struct fields can be a semver concern for downstream code constructing these structs directly.

Options:

1. If semver strictness matters, add non-exhaustive-style construction patterns where possible or add optional fields with defaults carefully.
2. If current public API already exposes these structs for direct construction, consider a minor version note and update examples.
3. Avoid removing or renaming existing fields.

Given the crate is preparing for crates.io polish, document this as a minor release behavior extension.

## Suggested implementation order

1. Add classifier and unit tests for classification only.
2. Replace/fix `needs_fancy_regex` with classifier-backed routing.
3. Add `engine_used` and `dialect` metadata to internal result structs and MCP output.
4. Add unsupported feature detection and machine-code mapping.
5. Expand lower-level and MCP wrapper tests.
6. Update README, architecture docs, and generated docs source.
7. Run local validation: `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and the existing docs/verification binary if applicable.

## Validation checklist

- `cargo fmt` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- `cargo test --all-features` passes.
- Existing regex tests still pass.
- New lookbehind `regex_finditer` regression tests pass.
- README/tool docs explicitly state the non-PCRE2 contract.
- Tool output shows backend metadata.
- Unsupported PCRE-like patterns fail deterministically and do not get misreported as unsafe unless the safety gate is the true reason for rejection.

## Release note candidate

Regex tools now use an explicit `eggsact-regex` compatibility contract. Callers do not need to choose a backend: eggsact automatically uses Rust `regex` for linear-compatible patterns and `fancy-regex` for supported extended constructs. Regex outputs report the backend used, and unsupported PCRE2-only constructs return clearer structured failures.
