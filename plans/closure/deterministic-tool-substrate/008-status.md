# Deterministic Tool Substrate Milestone 008 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/deterministic-tool-substrate/008-prompt-wire-compatibility-and-layering-guard-corrective.md`

Source subsystem roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#8` (Milestone 8)

Repository baseline reviewed: `5d57b89d30fdd226da9e99db23d943eaecfd0150`
(plan baseline; three registration commits `7155b83`/`721761c`/`794e0d2`
landed before implementation with planning-only changes; production tree at
handoff matched `5d57b89` for all `src/`/`tests/`/`architecture/` paths)

Implementation commits or pull requests:

- `475fc19` — Substrate milestone 008: prompt wire compatibility and generic
  layering guard (work packages A–D: `test_substrate_008` fixtures + generic
  guard, typed `prompt_wire_recommendations` + adapter null/string/array
  projection, docs/CHANGELOG reconciliation)

Milestone 007 closure reference (immutable history):

- `plans/closure/deterministic-tool-substrate/007-status.md`
  (implementation `237afb5` + `7176857`, closure `5d57b89`)

Pre-007 compatibility reference:

- `38aa6da3e8745ed4599362e2ecb0837bc20fca90`
  (`src/tools/text.rs::_pi_recommend_next_tool` returning
  `Option<serde_json::Value>` with five ordered conditions; result and
  `ToolResponse` envelope shared the same `Value`)

## 1. Executive finding

The two post-007 closure defects are corrected without reopening 007's
typed-core ownership. `src/text/inspect_prompt.rs` remains the sole semantic
owner; a new crate-private `prompt_wire_recommendations()` derives the
historical MCP/tool recommendation list from the already-computed findings,
and `prompt_input_inspect_tool` only serializes that list as
null/string/array, using the same `Value` for the result field and the
`ToolResponse` envelope. Historical fixed order and duplicate entries are
preserved. The typed `PromptInspectResult::recommended_next_tool:
Option<String>` API and behavior are unchanged. The layering guard is now a
repository-wide discovered handler graph covering all 86 public
`ToolResponse` handlers across all 24 `src/tools/*.rs` files (same-file and
cross-file), with synthetic self-tests proving detection; the four-edge 007
assertions remain as supplemental diagnostics. No ToolSpec, schema
capability, profile, audience, exposure, machine-code, protocol, dependency,
or MSRV change (`generate-docs --check` clean; `Cargo.toml`/`Cargo.lock`
untouched). The ordered merge gate, focused suites, parity, and remote
ordinary CI are green. No critical, high, or medium findings remain.

## 2. Requirement-to-evidence matrix

| Requirement (work package) | Evidence | Result | Notes |
|---|---|---|---|
| A: pre-007 recommendation matrix recorded from baseline | Pre-007 `_pi_recommend_next_tool` source + isolated worktree execution at `38aa6da` (see §3); 74-char base64 blob triggers both old (64+ with case/digit) and current (40+ entropy) detectors | pass | Old/new finding deltas (C0 ownership, TERMINAL details, base64 threshold) documented as accepted 007 fixes; projection logic is the compatibility target |
| A: current-tree regression fixtures for zero/single/multi + envelope equality + schema | `tests/mcp/test_substrate_008.rs` 14 prompt tests (clean, hidden, ANSI, terminal-isolated, markdown, HTML, base64, instruction, hidden+markdown, hidden+instruction duplicate, hidden+base64 duplicate, three-element, hidden+terminal, terminal+markdown, envelope equality, schema union, typed non-regression) | pass | 9/14 failed on `5d57b89` baseline for the intended lost array/legacy reasons before the fix; 14/14 pass after |
| B: wire projection restored without typed-API drift | `prompt_wire_recommendations()` in `src/text/inspect_prompt.rs` (crate-private, findings-only, no ToolResponse/MCP deps); adapter calls it and only serializes; `substrate_008_typed_preferred_recommendation_unchanged` | pass | `src/tools/text.rs` contains no finding-code-to-recommendation table; no `_pi_*` family restored |
| C: generic handler-graph guard + self-tests | `substrate_008_generic_handler_graph_has_no_handler_to_handler_edges` (24 files, 86 handlers, zero edges) + 6 scanner self-tests + `substrate_008_four_historical_edges_remain_absent` | pass | Injected `structured_data_compare -> json_compare` edge makes the generic guard fail locally; reverted |
| C: four historical edges absent | Same test asserts `structured_data_compare` ∉ {`json_compare`, `json_shape_tool`}, `config_preflight` ∉ {`toml_shape_tool`}, `command_preflight` ∉ {`shell_split`} | pass | 007 guard still green as supplemental |
| D: docs/schema reconciliation, no registry drift | `architecture/tools.md` (typed vs wire, generic guard), `architecture/text-library.md` (typed preferred vs wire list), `CHANGELOG.md` 008 entry; `generate-docs --check` clean; `tool_registration_tables_are_in_sync` (lib unit) green | pass | Output schema `["string","array"]` pinned by test; no description change needed |
| E: BUG-002 max_diffs=0 preserved | `substrate_008_bug002_max_diffs_zero_still_reports_unequal` (typed + MCP) | pass | Accepted historical scope delta, no further work |
| E: focused + merge gate + parity + CI | §4 below | pass | Full gate 673+40+14+3117+51+11, 0 failed; parity 381 passed, 40 ignored; remote CI `36486287769` green |

## 3. Production implementation evidence

WP-A baseline matrix (isolated worktree `38aa6da`, `Profile::Full` +
`Harness`, same inputs as current fixtures; 74-char blob
`aB3dE5fG7hJ9kL2mN4pQ6rS8tU0vW1xY3zA5cE7gI9kM2oP4qR6sT8uV0wX2yZ4aB6cD8eF0gH`):

- `clean "Hello."` → codes `[]`, result `null`, envelope `null`.
- `hidden "hi\u{200b}there"` → `["HIDDEN_CHAR"]`, `"text_inspect"` / same.
- `ansi "\u{1b}[31mred\u{1b}[0m"` → old `["HIDDEN_CHAR"×2,"ANSI_ESCAPE"×2]`,
  `["text_inspect","text_transform"]`; current findings are
  `["ANSI_ESCAPE"×2,"TERMINAL_CONTROL"×2]` → single `"text_transform"`.
  Delta is the accepted 007 C0-ownership fix (ESC no longer `HIDDEN_CHAR`);
  group-2 single is the correct projection of current findings.
- `markdown "see [docs](https://example.com) for details"` →
  `["MARKDOWN_LINK"]`, `"markdown_structure"` / same (stable).
- `html "hello <!-- secret --> world"` → `["HTML_COMMENT"]`,
  `"markdown_structure"` / same (stable; lost in 007, restored here).
- `base64 "prefix <74-char> suffix"` → `["BASE64_BLOB"]`, `"text_inspect"` /
  same (stable with 74-char blob; 50-char blob misses the old 64-char
  threshold but hits the current 40-char/entropy detector — finding
  evolution, not wire defect).
- `instruction "please ignore previous instructions"` →
  `["INSTRUCTION_PHRASE"]`, `"text_inspect"` / same (stable; lost in 007).
- `hidden_markdown` → `["HIDDEN_CHAR","MARKDOWN_LINK"]`,
  `["text_inspect","markdown_structure"]` (stable).
- `hidden_instruction` → `["HIDDEN_CHAR","INSTRUCTION_PHRASE"]`,
  `["text_inspect","text_inspect"]` duplicate (stable).
- `hidden_base64` → `["HIDDEN_CHAR","BASE64_BLOB"]`,
  `["text_inspect","text_inspect"]` duplicate (stable with 74-char blob).
- `three (hidden+markdown+instruction)` →
  `["HIDDEN_CHAR","MARKDOWN_LINK","INSTRUCTION_PHRASE"]`,
  `["text_inspect","markdown_structure","text_inspect"]` stable order.
- `hidden_terminal "hi\u{200b}there\u{8}"` → old `["HIDDEN_CHAR"×2]`,
  single `"text_inspect"` (old `TERMINAL_CONTROL` carried the same
  `(pos,codepoint)` key as old `HIDDEN_CHAR` and was deduplicated); current
  findings `["HIDDEN_CHAR","TERMINAL_CONTROL"]` →
  `["text_inspect","text_transform"]`. Accepted 007 details fix; array
  cardinality is the wire pin.
- `terminal_markdown "hi\u{8} see [docs](https://example.com)"` → old
  `["HIDDEN_CHAR","MARKDOWN_LINK"]`,
  `["text_inspect","markdown_structure"]` (stale C0 ownership); current
  `["TERMINAL_CONTROL","MARKDOWN_LINK"]` →
  `["text_transform","markdown_structure"]`. Accepted 007 ownership fix.

Pre-007 source (exact, `src/tools/text.rs` at `38aa6da`):

```rust
fn _pi_recommend_next_tool(findings: &[serde_json::Value]) -> Option<serde_json::Value> {
    // ... codes: HashSet<String> from findings ...
    // 1. HIDDEN_CHAR/BIDI_CONTROL -> text_inspect
    // 2. ANSI_ESCAPE/TERMINAL_CONTROL -> text_transform
    // 3. BASE64_BLOB -> text_inspect
    // 4. HTML_COMMENT/MARKDOWN_LINK -> markdown_structure
    // 5. INSTRUCTION_PHRASE -> text_inspect
    // 1 candidate -> String, >1 -> Array, 0 -> None (null/absent envelope)
}
```

Before-fix failure evidence (current tree at `5d57b89`, new tests, `cargo
test --locked --test lib substrate_008`): 10 failed, 16 passed. Failing for
the intended reasons: `html_single` (null vs `markdown_structure`),
`base64_single` (null vs `text_inspect`), `instruction_single` (null vs
`text_inspect`), `hidden_markdown_array` (single vs array),
`hidden_instruction_duplicate_array` (single vs duplicate array),
`hidden_base64_duplicate_array` (single vs duplicate),
`three_element_ordering` (single vs three-element),
`hidden_terminal_array` (single vs array), `terminal_markdown_array`
(single vs array), plus `four_historical_edges_remain_absent` (missing
`prompt_wire_recommendations` delegation). After the fix: 26/26 pass.

WP-B (`src/text/inspect_prompt.rs` + `src/tools/text.rs`, commit `475fc19`):

- New `pub(crate) fn prompt_wire_recommendations(findings: &[Value]) ->
  Vec<String>` implements the five historical conditions in fixed order on
  the already-computed findings, preserving duplicates, with no
  `ToolResponse`/registry/profile/schema/serde-envelope dependencies.
  Explicit comment distinguishes it from the unchanged typed preferred
  `_pi_recommend_next_tool() -> Option<String>`.
- `PromptInspectResult` fields/types unchanged; `recommended_next_tool`
  behavior unchanged (proven by
  `substrate_008_typed_preferred_recommendation_unchanged`: clean→None,
  hidden→`text_inspect`, ANSI→`text_transform`,
  markdown→`markdown_structure`, HTML/base64/instruction→None).
- `prompt_input_inspect_tool` obtains `wire_candidates` via the helper and
  serializes `0→Null`, `1→String`, `>1→Array`; the same `Value` is used for
  the result field and the envelope (`with_recommended_next_tool` only when
  non-null). No finding-code table in the adapter; no `_pi_*` restoration.
- All other prompt fields (findings, risk_score, summary via
  `prompt_inspect_wire_summary`, machine codes `PROMPT_HIDDEN_CONTENT` /
  `PROMPT_HAS_FLAGS`, limits, error handling) unchanged.

WP-C (generic guard, `tests/mcp/test_substrate_008.rs`):

- `tools_dir_sources()` reads `src/tools/*.rs` from the crate root at test
  time (test-only filesystem access; production code has none).
- `discover_handlers_in_source()` finds `pub fn <name> -> ToolResponse` in
  code (comment/string/raw-string/byte-string/char-literal aware, with
  lifetime vs char-literal disambiguation and nested block comments).
- `extract_handler_body()` + `calls_handler()` detect unqualified
  `callee(` calls, excluding `::`-qualified typed-core calls, definitions,
  comments, normal/raw/byte strings, char literals, and longer identifiers.
- Live scan: 24 files, 86 handlers, zero edges. Synthetic self-tests (6):
  same-file, cross-file, qualified-allowed, comments+strings ignored,
  raw-strings+char-literals ignored, longer-identifier not mistaken.
- Injection check: temporarily adding `let _ = json_compare(args);` inside
  `structured_data_compare` makes the generic guard fail with
  `["structured_data_compare (src/tools/json.rs) -> json_compare"]`;
  reverted before commit.
- Four historical edges remain absent (supplemental); 007 guard still green.

No registry, profile, audience, surface, protocol, DTO-schema, CLI,
dependency, MSRV, or machine-code change (`generate-docs --check` clean; 86
tools / 23 categories unchanged; `Cargo.toml`/`Cargo.lock` untouched;
`git diff` empty on `src/mcp/specs/`, `src/mcp/schemas/`, `generated/`, and
the `architecture/mcp-server.md` profile block).

## 4. Verification executed

### Commands run

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test lib substrate_008
cargo test --locked --test lib prompt_input_inspect
cargo test --locked --test lib adapter_layering
cargo test --locked --test lib test_inspect_prompt
cargo test --locked --test lib test_tool_coverage
cargo test --locked --lib tool_registration_tables_are_in_sync
cargo test --locked --test lib json_compare
cargo test --locked --test lib structured_data_compare
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo build --locked && cargo test --locked --test lib parity
```

### Results

- `cargo fmt --check`: pass (after `cargo fmt` normalization of the new
  test scanner).
- `generate-docs --check`: pass (no ToolSpec/profile/exposure drift).
- `clippy -D warnings`: pass locally (two test-scanner lints fixed:
  `unnecessary_to_owned`, `identity_op`); remote CI is the control for
  toolchain skew.
- Focused suites: `substrate_008` 26/26 pass (10 failed before the fix for
  the intended reasons); `prompt_input_inspect` 30 passed; `adapter_layering`
  1 passed; `test_inspect_prompt` 24 passed; `test_tool_coverage` 99 passed;
  `tool_registration_tables_are_in_sync` 1 passed (lib unit);
  `json_compare` 40 passed; `structured_data_compare` 35 passed.
- Full suite (`--skip parity`, `--test-threads=4`): lib 673 + main 40 +
  generate-docs 14 + integration 3117 + context-isolation 51 + doc 11 — all
  pass, 0 failed (integration 3091 → 3117 = +26 new 008 tests).
- Parity (`../eggcalc` available): 381 passed, 0 failed, 40 ignored; full
  parity green with no non-listed regressions.
  `tests/fixtures/accepted_parity_failures.txt` untouched.
- `cargo test --locked --doc`: 11 passed, 0 failed.
- MSRV: `rust-version` 1.89.0 untouched; no new dependency
  (`Cargo.toml`/`Cargo.lock` untouched, so no `cargo-deny` trigger).
- Remote CI: `36486287769` (ordinary correctness job green after push).

## 5. Invariant review

- One crate, tracked `Cargo.lock` (no dependency delta), MSRV 1.89.0,
  `--locked` gates throughout: holds.
- 86-tool/23-category registry, order, ToolSpecs, schemas, profiles,
  audiences, exposure, machine codes, direct/discovery behavior: unchanged
  (`generate-docs --check` + registry-sync test + empty diff on
  specs/schemas/generated/profile-block paths).
- Deterministic cores/services independent of `ToolResponse`, registry
  policy, and schema validation: holds (`prompt_wire_recommendations` takes
  only `&[Value]` findings; no new imports of response/registry types in
  `text/`).
- Typed-first ownership (007): preserved and strengthened (adapter delegates
  to typed helper; no `_pi_*` restoration; generic guard proves zero
  handler-to-handler composition repository-wide).
- Unicode 18 / UTS #39 behavior closed in 003–006: untouched (no
  confusables/property/generator changes).
- Python-parity accepted C1–C6 baseline: untouched file; only listed gaps
  observed (40 ignored, no new failures).
- Heuristic-only YAML, TOML/JSON/shell semantics: preserved (untouched
  paths; BUG-002 and BUG-006 pins green).
- Bounds/cancellation/truncation: unchanged; recommendation projection is
  O(findings) bounded by `MAX_FINDINGS`; no new locks, mutable globals,
  env/clock/net/fs reads in production paths (filesystem reads are
  test-only in the guard).
- 003/004/005/006/007 closure records: untouched (immutable history).

## 6. Failure and recovery review

- Oversize input: adapter bounds checks run before typed-core delegation;
  unchanged.
- Empty findings → null result and no envelope recommendation; single →
  string in both; multi → array in both (same `Value` cloned).
- Unknown checks / bad `phrase_patterns`: rejection stage unchanged (007
  pins still green).
- `max_diffs = 0` (BUG-002): typed + MCP pins green; no change beyond
  proving the fix remains.
- `TYPE_MISMATCH` (BUG-006): still dead by construction; 007 pin green.
- Cancellation/contention: no new shared state; projections are pure and
  stage-local; determinism asserted by new fixtures (stable order,
  duplicates preserved) plus existing determinism/concurrency suites
  (green in the full gate).
- Load flakes: none observed in this milestone's runs (full gate green
  first try; parity green first try).

## 7. Migration and compatibility review

- No consumer migration: MCP client config, direct/discovery surfaces,
  `ToolRegistry` calls, typed preflight wrappers, raw `tools::*` imports,
  and typed `text::*` imports remain valid unchanged.
- Intended wire restoration (CHANGELOG-recorded): MCP/tool clients regain
  the pre-007 null/string/array behavior already permitted by the
  `["string","array"]` schema. Rust users of `PromptInspectResult` keep the
  current `Option<String>` API and behavior. No tool name, field name,
  schema type, machine code, profile, audience, or protocol change. No
  recommendation array enters the typed Rust DTO.
- Finding-code deltas vs pre-007 executable (accepted 007 scope, not 008
  regressions): ESC/ C0 controls are `TERMINAL_CONTROL`/info (were
  `HIDDEN_CHAR`/warn), BIDI ownership fixed, `BASE64_BLOB`/`LONG_LINE` codes
  fixed, base64 threshold 40+entropy (was 64+case/digit), TERMINAL details
  no longer share the HIDDEN dedup key. The §3 matrix records the exact
  old/new outputs per input; the wire projection is correct for current
  findings in every case.
- Rollback: reverting `475fc19` restores `5d57b89` behavior exactly (no
  generated-file or lockfile involvement).

## 8. Determinism and bounded-execution review

- Prompt core: pure function of input + compiled regexes; dedup/truncation
  ordering unchanged; wire projection is a fixed five-condition set
  membership over retained findings, O(n) with no allocation beyond the
  bounded recommendation list (≤5 entries).
- Guard scanner: test-only; reads `src/tools/*.rs` at test time; no
  production filesystem access; no new parser dependency.
- No clock, TZ, env, locale, or network in touched paths; limits and
  `limits_applied` behavior unchanged.
- Result/envelope serialization is deterministic (fixed order, no dedup,
  `serde_json` `preserve_order`).

## 9. Documentation and operations

- `architecture/tools.md`: zero-composition section now cites the generic
  86-handler guard (007 four-edge test retained as diagnostics);
  `prompt_input_inspect_tool` documents typed single vs wire
  null/string/array with shared result/envelope value.
- `architecture/text-library.md`: `recommended_next_tool` distinguishes the
  typed preferred mapping from the five-condition wire list with
  null/string/array serialization.
- `src/mcp/schemas/text.rs` `prompt_input_inspect_output`: unchanged
  (`["string","array"]` already correct; description already plural).
- `generated/tool-cards.md` + `architecture/mcp-server.md` profile block:
  untouched (`--check` clean).
- `.opencode/skills/mcp-tools/SKILL.md`: no change needed (zero-reuse
  statement already closed by 007; generic guard is test evidence, not a
  new composition rule).
- `CHANGELOG.md`: `### Fixed (Prompt wire compatibility and
  layering-guard corrective, substrate Milestone 008)` entry.
- Static guards: `substrate_008_generic_handler_graph_has_no_handler_to_handler_edges`
  (24 files, 86 handlers, zero edges) + 6 scanner self-tests +
  `substrate_008_four_historical_edges_remain_absent` (4 edges + 15
  `_pi_*` + wire-helper delegation).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Full-suite wall time (~512s integration) reflects the existing 3k-test corpus, not new 008 cost (+26 tests) | None on the milestone | None; non-gating bench remains maintainer-run per `architecture/performance.md` |
| low | ANSI full-checks input has different first-element history (old array `["text_inspect","text_transform"]` vs current single `"text_transform"`) due to the accepted 007 C0-ownership fix | Documented in §3/§7; no wire-projection defect | None for 008; future finding-code changes must re-run the §3 matrix |

No critical, high, or medium findings remain.

## 11. Roadmap disposition

Milestone 008 closed. The deterministic-tool-substrate workstream returns
to guard-only status with no open milestone. No future plan is unblocked
beyond the workstream itself returning to guard-only: MCP 03c Parts D-F
remain blocked on provider credentials/budget and distribution M005 remains
blocked on Eggpack ADR-0005 / native-qualification resolution (Build M006 if
Option A is accepted) — both independent of this corrective and preserved
as blocked. The 007 deferred inventory (typed `ConfigFacts`,
dependency-facts normalization, catalog/runtime ADR-gated extraction,
large-module decomposition, `tools/helpers.rs` drain-down) remains deferred
and is not opened by this closure. Future work, if any, needs a new
implementation plan.

## 12. Registry updates

- `plans/registry.md`: 008 → closed with this closure record; substrate
  current milestone → guard-only (no open milestone); dependency note updated.
- `plans/subsystems/deterministic-tool-substrate-roadmap.md`: status header →
  active (guard workstream); milestone 008 row → closed with closure link;
  dependency graph annotated closed.
- `plans/implementation/deterministic-tool-substrate/008-*.md`: status header
  → `closed` (implemented in `475fc19`, closed here).
- 003/004/005/006/007 implementation and closure files: untouched (immutable
  history).
