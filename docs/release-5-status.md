# Release 5 Status Note

**Date:** 2026-07-28 UTC
**Final verification baseline:** `75ea50369510d98617741d4025fc626a0983b2e0` (corrective pass on `3e5b41c`)
**Plan:** `plans/2026-07-18-release-5-fuzzing-property-testing-plan.md`
**Corrective pass:** `plans/2026-07-28-calculator-normalization-backtrack-limit-corrective-pass.md`

## Fuzz targets

All 12 targets build against bounded input. The tracked regression corpus has
77 seeds, excluding `.gitkeep` placeholders:

| Target | Category | Corpus seeds |
|--------|----------|--------------|
| calculator_expression | math | 11 |
| calculator_normalization | math | 4 |
| unified_diff | patch | 7 |
| shell_tokenization | shell | 7 |
| shell_quoting | shell | 3 |
| regex_classification | regex | 6 |
| regex_execution | regex | 5 |
| json_pointer | json | 7 |
| toml_config | config | 6 |
| unicode_inspection | unicode | 8 |
| markdown_fences | markdown | 6 |
| glob_matching | path | 7 |
| **Total** | | **77** |

The fuzz toolchain used for the final local builds was pinned to
`nightly-2026-05-07` (`rustc 1.97.0-nightly (365c0e1d7 2026-05-06)`) and
cargo-fuzz `0.13.2`.

## Property tests

47 property tests across 9 modules pass in the ordinary test suite. They cover
round-trip, determinism, symmetry, transaction, and span-validity properties;
the former vacuous no-panic checks were removed or strengthened.

## CI configuration

- **PR smoke fuzzing:** `fuzz-pr.yml` builds all targets and runs bounded high-value targets with concurrency cancellation.
- **Scheduled/manual extended fuzzing:** `fuzz-scheduled.yml` runs all 12 targets in a matrix with per-target timeouts.
- **Sanitizers:** the same workflow runs a 7-target AddressSanitizer matrix.

## Final workflow evidence

Extended fuzz/sanitizer run on CODE_SHA `3e5b41c`:

- **Run ID**: `30287151564`
- **URL**: <https://github.com/eggstack/eggsact/actions/runs/30287151564>
- **Head SHA**: `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
- **Conclusion**: success; 19/19 jobs passed (12 fuzz-matrix + 7 fuzz-sanitizers)

Local `cargo fuzz build` and `cargo fuzz build --sanitizer=address` also passed.

## Findings fixed in the final proof pass

- `unified_diff`: zero-count destination hunks now validate ranges without a
  panic.
- Path inspection: short Unicode paths no longer index by byte position.
- Calculator `gcd`/`lcm`: `i64::MIN` now fails closed instead of panicking during
  absolute-value conversion.
- Regex execution: zero-length matches at end-of-input now advance and
  terminate instead of looping forever.
- Fuzz harness invariants were corrected where large-number formatting and
  normalization spacing do not guarantee byte-identical canonical strings.

These fixes were made narrowly in response to minimized fuzz inputs and are
included in the final verification baseline.

## Corrective pass: calculator normalization backtrack limit

A re-dispatch of the extended fuzz workflow on exact CODE_SHA `3e5b41c`
(Run `30306975485`) found a non-deterministic crash in the
`calculator_normalization` fuzz target. The crash input `32E73 33` triggers a
`fancy-regex` `BacktrackLimitExceeded` panic during normalization regex
matching.

**Root cause:** `combine_consecutive_number_words` treated scientific-notation
tokens (e.g. `32e73`) as compound-number components, producing a 74-digit literal
that exhausted the `fancy-regex` backtrack limit when matched against the
715-unit `UNIT_ALT` alternation in `UNIT_SPELLED_RE.replace_all`.

**Fix (commit `75ea503`):**

- All `fancy-regex` `replace_all()`/`replacen()` calls in `src/calc/normalize.rs`
  replaced with a fallible `try_replace_all()` helper using
  `try_replacen(text, 0, replacement)`, propagating runtime errors as
  deterministic `RunError::Internal` instead of panicking.
- `combine_number_run` now excludes scientific-notation tokens (containing `e`)
  from compound-number logic, causing `32E73 33` to normalize to `32E73+33`
  (matching Python/eggcalc parity).
- `binary_word_check` and `preprocess_units` propagate `fancy-regex` errors
  instead of `unwrap()`/`.ok().flatten()`.
- Fuzz target corrected to distinguish production panic, deterministic
  `Ok`/`Err`, and non-determinism via `catch_unwind`.
- Regression tests and a persistent fuzz corpus seed added.

**Verification:** `calc::run("32E73 33")` no longer panics; output matches
Python/eggcalc parity (`32E73+33` → 3.2e74). All final workflows pass on the
new SHA `75ea503`: ordinary CI (12/12), release verification (Full Release
Gate), extended fuzz (19/19), latest-compatible, and Python parity.

## Closure criteria

| Criterion | Evidence | Status |
|-----------|----------|--------|
| Releases 1–3 final correctness closure | `plans/2026-07-18-releases-1-3-final-correctness-plan.md` | Complete |
| Release 4 verification infrastructure green | `docs/release-4-status.md` — all criteria met | Complete |
| Every planned fuzz target builds and runs | 12 targets build; all run in extended fuzz matrix | Complete |
| Persistent corpora committed and seeded | 77 seeds across 12 targets (see table above) | Complete |
| All required surfaces have fuzz coverage | Calculator, diff, shell, regex, JSON, TOML, Unicode, Markdown, glob — all covered | Complete |
| Core properties enforced in ordinary tests | 47 property tests across 9 modules | Complete |
| No untriaged crash/hang/OOM/overflow | Extended fuzz Run `30287151564` on `3e5b41c` — all 19 jobs pass, no new findings; corrective pass `75ea503` closes the `calculator_normalization` backtrack artifact | Complete |
| Fixed findings have regression tests | 4 fuzz-discovered fixes with regression seeds in `fuzz/corpus/` | Complete |
| PR smoke fuzzing active and bounded | `fuzz-pr.yml` — builds all targets, runs bounded high-value targets with concurrency cancellation | Complete |
| Extended fuzzing covers all targets | `fuzz-scheduled.yml` — 12-target matrix with per-target timeouts | Complete |
| Fuzz dependencies excluded from runtime | `fuzz/Cargo.toml` isolated workspace; not in root `Cargo.toml` | Complete |
| Fuzzing documentation current | `docs/fuzzing.md` — reproduce, minimize, fix, promote, security handling | Complete |
| Full CI, cargo-deny, docs, package gates pass | CI run [30367423228](https://github.com/eggstack/eggsact/actions/runs/30367423228) — all 12 jobs success on `75ea503` | Complete |
| Release verification on exact CODE_SHA | Release verification [30373993751](https://github.com/eggstack/eggsact/actions/runs/30373993751) — Full Release Gate success on `75ea503`; provenance artifact records `75ea503` | Complete |
| Extended fuzz and sanitizer matrices pass on corrective SHA | Extended fuzz [30373991584](https://github.com/eggstack/eggsact/actions/runs/30373991584) — 19/19 jobs success (12 fuzz-matrix + 7 fuzz-sanitizers), no crash artifacts | Complete |
| Latest-compatible dependencies pass on corrective SHA | [30373996030](https://github.com/eggstack/eggsact/actions/runs/30373996030) — success on `75ea503` | Complete |
| Python parity passes on corrective SHA | [30373998127](https://github.com/eggstack/eggsact/actions/runs/30373998127) — success on `75ea503`; parity report records eggsact 1.2.0, eggcalc 1.1.6, Python 3.12.13 | Complete |

## Publication status

`eggsact 1.2.0` was published to crates.io on 2026-07-28T19:10:10.018107Z
from detached commit `75ea50369510d98617741d4025fc626a0983b2e0`. Annotated
tag `v1.2.0` dereferences to the same SHA. Current `main` has advanced to
unreleased version `1.2.1`.
