# Deterministic Tool Substrate Milestone 007 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/deterministic-tool-substrate/007-typed-core-ownership-and-adapter-deduplication.md`

Source subsystem roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7` (Milestone 7)

Repository baseline reviewed: `38aa6da3e8745ed4599362e2ecb0837bc20fca90`
(plan baseline; the working tree was clean at handoff; four unrelated
remote planning commits landed before implementation began — the
implementation rebased onto `c582d72` with no conflicts)

Implementation commits or pull requests:

- `237afb5` — Substrate milestone 007: typed-core ownership and adapter
  deduplication (work packages A–F: substrate_007 fixtures + layering
  guard, typed prompt-core ownership with documented finding codes,
  structured_data_compare/config_preflight/command_preflight typed
  delegation with private projections, BUG-002 typed-core fix,
  documentation/CHANGELOG reconciliation)
- `7176857` — Substrate 007 follow-up: satisfy manual_filter/question_mark
  lints in the layering-guard test helper (remote CI clippy 1.98 finding;
  no behavior change)

Milestone 003/004/005/006 closure references (immutable history):

- `plans/closure/deterministic-tool-substrate/003-status.md`
  (implementation `3d67807`, closure `24073e9`)
- `plans/closure/deterministic-tool-substrate/004-status.md`
  (implementation `2e3860c`, closure `b7fcc00`)
- `plans/closure/deterministic-tool-substrate/005-status.md`
  (implementation `cb0d743`, closure `1f0816e`)
- `plans/closure/deterministic-tool-substrate/006-status.md`
  (implementation `83de61a`, closure `79a4ff5`)

## 1. Executive finding

The repository's documented typed-first ownership invariant is restored at
the remaining adapter boundaries. `src/text/inspect_prompt.rs` is the sole
semantic implementation of prompt-input inspection (the parallel
adapter-local `_pi_*` family in `src/tools/text.rs` is deleted and
`prompt_input_inspect_tool` is an input-validation/wire-projection
adapter); the four remaining same-module handler-to-handler compositions
are replaced by typed-core calls with private typed-result-to-wire
projection helpers; and a source-architecture guard
(`adapter_layering_has_no_handler_to_handler_composition`) enforces the
zero-reuse rule. No ToolSpec, schema, profile, audience, exposure,
machine-code, public-API, or dependency change occurred
(`generate-docs --check` clean; `Cargo.toml`/`Cargo.lock` untouched). The
ordered merge gate, all focused suites, milestone-area parity, and remote
ordinary CI are green. No critical, high, or medium findings remain; three
low findings (load-flake posture, remote-CI lint follow-up, accepted-gap
staleness note) are recorded in §10.

## 2. Requirement-to-evidence matrix

| Requirement (work package) | Evidence | Result | Notes |
|---|---|---|---|
| A: prompt_input_inspect differential fixtures (clean, hidden, bidi, ANSI, terminal, phrase, checks subset, custom patterns, CRLF numbering, dedup/next-tool) | `tests/mcp/test_substrate_007.rs` `substrate_007_prompt_input_inspect_*` (11 tests) + `substrate_007_prompt_input_inspect_unknown_check_rejected` | pass | 11/11 pass on final code; the guard failed at baseline for the intended composition sites |
| A: structured_data_compare wire pins (equal, differing, nested/array, invalid A/B, max_diffs, shape subresults, BUG-006) | `substrate_007_structured_data_compare_*` (8 tests) | pass | Includes dead-TYPE_MISMATCH compatibility pin |
| A: config_preflight TOML pins (flat, nested, malformed, no-shape-on-invalid) | `substrate_007_config_preflight_*` (4 tests) | pass | |
| A: command_preflight parse-stage pins (argv, quotes, pipe/redirect, unbalanced, risky, platforms) | `substrate_007_command_preflight_*` (6 tests) | pass | Unbalanced-quote expectations encode the preserved `SHELL_RISK` primary-code behavior (see §6) |
| A: layering guard rejects 4 handler calls + `_pi_*` family | `adapter_layering_has_no_handler_to_handler_composition` | pass | Failed at baseline with the intended first violation (`structured_data_compare` → `json_compare`); green after B–E |
| B: typed prompt inspection authoritative; duplicate helpers removed | `src/tools/text.rs`: 0 `fn _pi_*` definitions; `prompt_input_inspect_tool` calls `crate::text::inspect_prompt::prompt_input_inspect()` exactly once; `PromptInspectResult` projected via `prompt_inspect_wire_summary` | pass | Path (1) of the plan's compatibility rule: the adapter was the stale path (wrong codes/severities); typed core corrected to the documented contract |
| B: typed finding codes match `architecture/tools.md` | `LONG_LINE` (was `LONG_MINIFIED_LINE`), `BASE64_BLOB` (was `BASE64_LIKE_BLOB`), `TERMINAL_CONTROL` at info (was warn), C0/C1 controls owned by `terminal_controls` (removed from `unicode_hidden`) | pass | `text-library.md` detection table reconciled; wire `BASE64_BLOB`/`LONG_LINE` cells already matched |
| C: structured_data_compare uses typed JSON cores | `crate::text::validate::json_compare()` + `json_shape()` with `json_shape_to_wire()` projection; no `json_compare(`/`json_shape_tool(` calls in body | pass | Guard-verified; BUG-002 `max_diffs = 0` fix in typed core (§3) |
| D: config_preflight uses typed TOML core | `crate::text::toml::toml_shape(text, 100)` with `toml_shape_to_wire()`; no `toml_shape_tool(` call in body | pass | Guard-verified; YAML behavior untouched |
| E: command_preflight uses typed shell core | `crate::text::shell::shell_split(command, "posix", true)` with `shell_split_to_wire()`; no `shell_split(` handler call in body | pass | Guard-verified; `SHELL_RISK` primary preserved (§6) |
| F: docs reconciled; generated facts unchanged | `architecture/tools.md` zero-reuse section; `plans/000` §4.2 + §7.3; `plans/001` JSON-adapter wording; `text-library.md` severity/ownership cells; skill sentence; CHANGELOG entry; `generate-docs --check` clean | pass | Canonical edits are contradiction/factual reconciliation per `plans/003` §2.1 |
| G: focused suites + merge gate + parity + CI | §4 below | pass | Full gate 673+40+14+3091+51+11, 0 failed; parity §4; remote CI run recorded |

## 3. Production implementation evidence

WP-B prompt core (`src/text/inspect_prompt.rs`):

- `_pi_find_unicode_hidden` no longer matches C0/C1 controls (`0x00..=0x08
  | 0x0E..=0x1F`, `0x7F`, `0x80..=0x9F` removed): per
  `architecture/tools.md`, control characters belong to
  `terminal_controls`, and `unicode_hidden` covers zero-width
  spaces/joiners, marks, separators, invisible formats, variation
  selectors, specials, and BOM.
- `_pi_find_terminal_controls` severity `warn` → `info` (documented
  severity).
- `_pi_find_base64_like_blobs` code `BASE64_LIKE_BLOB` → `BASE64_BLOB`
  (documented code).
- `_pi_find_long_minified_lines` code `LONG_MINIFIED_LINE` → `LONG_LINE`
  with message `Very long line N (M chars)` and span
  `{line, char_start, char_end}` (cumulative char offsets with CRLF/LF
  accounting; line number retained). A first-pass cumulative-offset bug
  found during implementation (per-line-relative byte index) was fixed
  with a cumulative `byte_offset` walk and locked by
  `test_prompt_inspect_long_line_mid_text_cumulative_offsets`.
- `prompt_input_inspect()` signature and `PromptInspectResult` shape
  unchanged; `services/security.rs` Stage 4 consumes the same fields.

WP-B adapter (`src/tools/text.rs`, 3798 → ~3260 lines):

- Deleted the entire adapter-local semantic family (15 helpers:
  `_pi_char_span`, `_pi_hidden_char_display`, `_pi_hidden_char_category`,
  `_pi_find_unicode_hidden`, `_pi_find_bidi`, `_pi_find_html_comments`,
  `_pi_find_markdown_links`, `_pi_python_repr`, `_pi_find_ansi_escapes`,
  `_pi_find_terminal_controls`, `_pi_find_base64_like_blobs`,
  `_pi_find_long_minified_lines`, `_pi_find_instruction_phrases`,
  `_pi_compute_risk_score`, `_pi_build_summary`, `_pi_recommend_next_tool`)
  plus the `DEFAULT_INSTRUCTION_RE`/`DEFAULT_INSTRUCTION_PHRASES` statics
  and the now-unused regex/primitive imports.
- `prompt_input_inspect_tool` keeps argument validation, bounds checks,
  unknown-check rejection, `phrase_patterns` parsing,
  `PROMPT_HIDDEN_CONTENT`/`PROMPT_HAS_FLAGS` machine-code routing,
  envelope findings, and `recommended_next_tool` (single string per the
  architecture contract — the adapter's former multi-recommendation array
  is gone); semantic facts come from exactly one
  `prompt_input_inspect()` call; the wire `summary` is built by the pure
  projection `prompt_inspect_wire_summary()`.

WP-C (`src/tools/json.rs` + `src/text/validate.rs`):

- `structured_data_compare` calls `crate::text::validate::json_compare(a,
  b, ignore_object_order, ignore_array_order, false, false, false,
  max_diffs)` and projects `JsonCompareResult` to the existing
  `VALUE_DIFF` findings + `json_compare` (`equal`/`diff_count`) subresult;
  typed-core `Err` maps to the existing `COMPARE_ERROR` finding.
- Both shape calls use `crate::text::validate::json_shape(a/b, 4, 100, 5)`
  (the standalone adapter defaults) projected through the private
  `json_shape_to_wire()` (`valid`/`shape`/`truncated`/`summary`); the
  `TYPE_MISMATCH` block is preserved byte-for-byte and stays dead
  (BUG-006: no top-level `type` field).
- Typed-core BUG-002 fix: `compare_values` threads a `not_equal` flag set
  when the diff budget is exhausted but `a_val != b_val`, so
  `equal = diffs.is_empty() && !not_equal` stays correct for
  `max_diffs = 0`. Locked by
  `test_json_compare_max_diffs_zero_still_reports_unequal` (typed) and the
  pre-existing MCP `test_structured_data_compare_max_diffs_zero_reports_unequal`.
- During implementation review, an accidental `b_key` → `a_key` token
  change in the positional-compare recursion was caught and reverted
  before commit; locked by
  `test_json_compare_positional_casefold_key_rename_no_panic`
  (`ignore_object_order = false` + casefold + case-only key rename must
  not `unwrap()` on `None`).

WP-D (`src/tools/config.rs`):

- `config_preflight` TOML branch calls
  `crate::text::toml::toml_shape(text, 100)` (standalone default
  `max_tables`) projected through private `toml_shape_to_wire()`
  (`valid`/`top_level_keys`/`tables`/`truncated`/`summary`); on typed-core
  error no subresult is inserted, matching the prior handler-error path.
  JSON/dotenv/INI/cargo branches untouched; YAML exclusion untouched.

WP-E (`src/tools/shell.rs`):

- `command_preflight` parse stage calls
  `crate::text::shell::shell_split(command, "posix", true)` projected
  through private `shell_split_to_wire()` (`argv` + full `features` map);
  the prior handler-error branch (dead code — the handler always returns
  `ok: true` with `parse_ok` in the body) is removed, and parse-error
  info stays in the `shell_split` subresult while the policy engine's
  `RISKY_SHELL_FEATURE` keeps driving the wire-level `SHELL_RISK`
  primary code for unbalanced quotes (see §6).

No registry, profile, audience, surface, protocol, DTO-schema, CLI,
dependency, MSRV, or machine-code change (`generate-docs --check` clean;
86 tools / 23 categories unchanged; `Cargo.toml`/`Cargo.lock` untouched;
`git diff` empty on `src/mcp/specs/`, `src/mcp/schemas/`, `generated/`,
and the `architecture/mcp-server.md` profile block).

## 4. Verification executed

### Commands run

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo test --locked --all-features --lib
cargo test --locked --lib tool_registration_tables_are_in_sync
cargo test --locked --test lib prompt_input_inspect
cargo test --locked --test lib structured_data_compare
cargo test --locked --test lib config_preflight
cargo test --locked --test lib command_preflight
cargo test --locked --test lib shell_split
cargo test --locked --test lib toml_shape
cargo test --locked --test lib json_compare
cargo test --locked --test lib test_inspect_prompt
cargo test --locked --test lib adapter_layering
cargo test --locked --test lib test_preflight_wrappers test_route_contracts test_machine_codes
cargo test --locked --test lib test_validate
cargo build --locked && cargo test --locked --test lib parity
cargo test --locked --test lib parity -- structured_data_compare config_preflight json_compare json_shape toml_shape shell_split command_preflight
```

### Results

- `cargo fmt --check`: pass.
- `generate-docs --check`: pass (no ToolSpec/profile/exposure drift).
- `clippy -D warnings`: pass locally; remote CI (clippy 1.98) flagged one
  `manual_filter` lint in the new test helper, fixed in `7176857`
  (filter/`?` rewrite, also satisfying the local `question_mark` lint);
  remote re-run green (see CI below).
- Full suite (`--skip parity`, `--test-threads=4`): lib 673 + misc 40/14
  + integration 3091 + context-isolation 51 + doc 11 — all pass, 0 failed
  (integration 3058 → 3091 = +30 substrate_007 tests +2 typed regression
  tests +1 prompt cumulative-offset test).
- Focused suites: prompt_input_inspect 30, structured_data_compare 35,
  config_preflight 30, command_preflight 33, shell_split 52, toml_shape 19,
  json_compare 38, test_inspect_prompt 24, substrate_007 layering+fixtures
  30, `tool_registration_tables_are_in_sync` 1, preflight/route/machine-code
  104, test_validate 167 — all pass.
- Parity (`../eggcalc` available): milestone-area filter 589 passed, 0
  failed, 40 ignored; full parity 374–380 passed with only unrelated
  `math_eval` MCP-subprocess TIMEOUT flakes under parallel load (each
  passes in isolation; identical load behavior on baseline) plus the 37
  accepted C1–C6 gaps. `tests/fixtures/accepted_parity_failures.txt`
  untouched. The accepted C6 gap
  `test_bug006_prompt_inspect_vt_ff_detected` fails identically at
  baseline and after (`Expected text content` — HarnessOnly audience
  rejection in the parity subprocess, unrelated to finding content).
- `cargo test --locked --doc`: 11 passed, 0 failed.
- MSRV: `rust-version` 1.89.0 untouched; no new dependency
  (`Cargo.toml`/`Cargo.lock` untouched, so no `cargo-deny` trigger).
- Remote CI: `36468271519` (ordinary correctness job green after push;
  maintenance-lane MSRV/deny/platform coverage per repo policy). A prior
  run `36466006064` failed in 48s on the `manual_filter` lint above;
  fixed in `7176857`, re-run green.

## 5. Invariant review

- One crate, tracked `Cargo.lock` (no dependency delta), MSRV 1.89.0,
  `--locked` gates throughout: holds.
- 86-tool/23-category registry, order, ToolSpecs, schemas, profiles,
  audiences, exposure, machine codes, direct/discovery behavior: unchanged
  (`generate-docs --check` + registry-sync test + empty diff on
  specs/schemas/generated/profile-block paths).
- Deterministic cores/services independent of `ToolResponse`, registry
  policy, and schema validation: holds (projections are pure functions of
  typed results; no new imports of response/registry types in `text/`).
- Unicode 18 / UTS #39 behavior closed in 003–006: untouched (no
  confusables/property/generator changes).
- Python-parity accepted C1–C6 baseline: untouched file; only listed gaps
  plus unrelated load flakes observed.
- Heuristic-only YAML, TOML/JSON/shell semantics: preserved (YAML paths
  untouched; TOML/shell/JSON suites green unmodified).
- Bounds/cancellation/truncation: unchanged; composite `BudgetContext`
  checkpoints preserved; `MAX_TEXT_LENGTH`/`MAX_LIST_ITEMS` adapter checks
  preserved; truncation/`limits_applied` owned by existing layers.
- No new locks, mutable globals, env/clock/net/fs reads in touched paths:
  holds (pure projections; `LazyLock` regexes removed from the adapter
  along with the deleted family).
- 003/004/005/006 closure records: untouched (immutable history).

## 6. Failure and recovery review

- Oversize input: adapter bounds checks run before typed-core delegation
  in all four adapters; typed-core `Err` (oversize) maps to the pre-existing
  error/finding paths (`COMPARE_ERROR`; no shape subresult; prompt
  `INPUT_TOO_LARGE`).
- `max_diffs = 0` (BUG-002): typed core now reports `equal = false` with
  empty diffs + `truncated`; composite findings/summary unchanged in shape.
- Unbalanced-quote parse handling: the removed handler-error branch was
  dead (handler always `ok: true`); typed `parse_ok` info is captured in
  the `shell_split` subresult while `SHELL_RISK` stays the wire primary
  code — verified by `test_primary_code_parse_error_over_risk` (pre-existing)
  and the new `substrate_007_command_preflight_unbalanced_quotes` pin.
- `TYPE_MISMATCH` (BUG-006): still dead by construction; pin asserts absence.
- Unknown checks / bad `phrase_patterns` / unsupported platform: rejection
  stage and error shapes unchanged (pinned by new adapter tests).
- Cancellation/contention: no new shared state; projections are pure and
  stage-local; determinism asserted by existing + new fixtures.
- Accidental-regression catch: the `b_key`→`a_key` token slip in the
  positional-compare recursion was caught in self-review before commit and
  is now locked by
  `test_json_compare_positional_casefold_key_rename_no_panic`.
- Load flakes: 8 unrelated `math_eval` MCP-subprocess tests failed once
  under full parallel load (timeouts) and pass in isolation and on
  re-run; same class as the documented blocking-pool sensitivity, not a
  product regression. Recorded here rather than hidden.

## 7. Migration and compatibility review

- No consumer migration: MCP client config, direct/discovery surfaces,
  `ToolRegistry` calls, typed preflight wrappers, raw `tools::*` imports,
  and typed `text::*` imports remain valid unchanged.
- Intended wire refinements (documented-contract corrections, CHANGELOG-recorded):
  bidi controls now surface as `BIDI_CONTROL` (were deduplicated to
  `HIDDEN_CHAR`); C0/C1 controls surface as `TERMINAL_CONTROL`/info (were
  `HIDDEN_CHAR`/warn); long lines use code `LONG_LINE` with
  `{line, char_start, char_end}` spans; base64 findings use `BASE64_BLOB`;
  `recommended_next_tool` is a single string (the adapter's former
  multi-recommendation array is gone). All match
  `architecture/tools.md`'s long-documented contract; no existing MCP
  test asserted the stale shapes (verified: full suite green unmodified
  except the two intentionally-updated typed tests).
- Parity: zero non-listed failures; accepted baseline untouched.
- Rollback: reverting `237afb5` + `7176857` restores baseline behavior
  exactly (no generated-file or lockfile involvement).

## 8. Determinism and bounded-execution review

- Prompt core: pure function of input + compiled regexes; dedup/truncation
  ordering unchanged; long-line char offsets now cumulative and
  CRLF-aware (pinned for CRLF + mid-text cases).
- JSON/TOML/shell cores: pure; no new loops beyond the existing bounded
  traversals; `not_equal` tracking is O(1) per node.
- No clock, TZ, env, locale, or network in touched paths; limits and
  `limits_applied` behavior unchanged.

## 9. Documentation and operations

- `architecture/tools.md`: zero-composition section replaces the
  four-reuse table (three-vs-four inconsistency eliminated); composite
  pipeline prose updated (structured_data_compare, command_preflight,
  config_preflight TOML); prompt-core ownership noted.
- `architecture/text-library.md`: `unicode_hidden`/`terminal_controls`
  detection cells corrected to the typed core.
- `docs/library-api.md`: no prompt section exists; no change needed.
  `generated/tool-cards.md` + `architecture/mcp-server.md` profile block:
  untouched (`--check` clean).
- `plans/000-long-term-specification.md` §4.2 + §7.3 and
  `plans/001-terminology-and-domain-model.md` JSON-adapter wording:
  exception-count language replaced with the zero-reuse rule (+ 007
  closure link). Factual reconciliation per `plans/003` §2.1; no new
  architecture.
- `.opencode/skills/mcp-tools/SKILL.md` (symlinked from
  `.agents/skills/`): stale "three remaining reuses" sentence replaced
  with the closed state.
- `CHANGELOG.md`: `### Fixed (Typed-core ownership and adapter
  deduplication, substrate Milestone 007)` maintenance entry.
- Static guards: `adapter_layering_has_no_handler_to_handler_composition`
  (4 forbidden handler edges + 15 forbidden `_pi_*` definitions,
  comment/string-literal aware, module-qualified typed calls allowed).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Full-suite load flakes: 8 unrelated `math_eval` MCP-subprocess tests timed out once under `--test-threads=4` parallel load | None on the milestone; all pass in isolation and on clean re-run; same sensitivity class as the documented blocking-pool note | None for 007; consider tracking subprocess-test load budget separately if it recurs |
| low | Remote-CI posture: first 007 run failed fast on a new-clippy lint; fixed and re-run green | Standard corrective loop; closure evidence gathered locally plus the green remote run above | None; toolchain-skew note (local clippy predates CI's 1.98) |
| low | Accepted C6 gap `test_bug006_prompt_inspect_vt_ff_detected` still fails for its documented harness-audience reason | Unchanged by this milestone; finding-content path for VT/FF moved from `HIDDEN_CHAR` to `TERMINAL_CONTROL`/info per the documented contract | None for 007; a future parity-harness fix (Harness audience in the parity subprocess) may revisit the C6 deferrals |

No critical, high, or medium findings remain.

## 11. Roadmap disposition

Milestone 007 closed. The deterministic-tool-substrate workstream returns
to guard-only status with no open milestone. No follow-up corrective is
required: 007 has no hard/interface dependents, so no future plan is
unblocked beyond the workstream itself returning to guard-only (MCP 03c
Parts D-F remain blocked on provider credentials/budget; distribution
M005 remains blocked on Eggpack CI M003d + Build M005 and is now further
gated by Eggpack ADR-0005 per the unrelated remote planning commits —
both independent of this milestone; the plan's §19 instruction to preserve
their blockers/status is honored). The §17 deferred inventory (typed
`ConfigFacts`, dependency-facts normalization, catalog/runtime ADR-gated
extraction, large-module decomposition, `tools/helpers.rs` drain-down)
remains deferred and is not opened by this closure. Future work, if any,
needs a new implementation plan.

## 12. Registry updates

- `plans/registry.md`: 007 → closed with this closure record; substrate
  current milestone → guard-only (no open milestone); dependency note updated.
- `plans/subsystems/deterministic-tool-substrate-roadmap.md`: status header →
  active (guard workstream); milestone 007 row → closed with closure link;
  dependency graph annotated closed.
- `plans/implementation/deterministic-tool-substrate/007-*.md`: status header
  → `closed` (implemented in `237afb5` + `7176857`, closed here).
- 003/004/005/006 implementation and closure files: untouched (immutable history).
