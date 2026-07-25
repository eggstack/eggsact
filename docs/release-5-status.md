# Release 5 Status Note

**Date:** 2026-07-25 UTC
**Final verification baseline:** `06f7a0bd7c1005439e9de229c37cb34d988b42e4`
**Plan:** `plans/2026-07-18-release-5-fuzzing-property-testing-plan.md`

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

The [Fuzz Extended run 30138546987](https://github.com/eggstack/eggsact/actions/runs/30138546987)
passed 19/19 jobs on the exact final SHA. This includes all 12 fuzz-matrix
jobs and all 7 sanitizer jobs:

- sanitizer: `regex_classification`, `calculator_expression`, `shell_tokenization`,
  `glob_matching`, `unicode_inspection`, `json_pointer`, `unified_diff`;
- extended matrix: all 12 targets, including `regex_execution`,
  `calculator_normalization`, `toml_config`, and `markdown_fences`.

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

## Closure criteria

| Criterion | Evidence | Status |
|-----------|----------|--------|
| Releases 1–3 final correctness closure | `plans/2026-07-18-releases-1-3-final-correctness-plan.md` | Complete |
| Release 4 verification infrastructure green | `docs/release-4-status.md` — all criteria met | Complete |
| Every planned fuzz target builds and runs | 12 targets build; all run in extended fuzz matrix | Complete |
| Persistent corpora committed and seeded | 77 seeds across 12 targets (see table above) | Complete |
| All required surfaces have fuzz coverage | Calculator, diff, shell, regex, JSON, TOML, Unicode, Markdown, glob — all covered | Complete |
| Core properties enforced in ordinary tests | 47 property tests across 9 modules | Complete |
| No untriaged crash/hang/OOM/overflow | Fuzz Extended run [30138546987](https://github.com/eggstack/eggsact/actions/runs/30138546987) — 19/19 success, 0 failures | Complete |
| Fixed findings have regression tests | 4 fuzz-discovered fixes with regression seeds in `fuzz/corpus/` | Complete |
| PR smoke fuzzing active and bounded | `fuzz-pr.yml` — builds all targets, runs bounded high-value targets with concurrency cancellation | Complete |
| Extended fuzzing covers all targets | `fuzz-scheduled.yml` — 12-target matrix with per-target timeouts | Complete |
| Fuzz dependencies excluded from runtime | `fuzz/Cargo.toml` isolated workspace; not in root `Cargo.toml` | Complete |
| Fuzzing documentation current | `docs/fuzzing.md` — reproduce, minimize, fix, promote, security handling | Complete |
| Full CI, cargo-deny, docs, package gates pass | CI run [30162970273](https://github.com/eggstack/eggsact/actions/runs/30162970273) — all 12 jobs success | Complete |
| Release verification on exact CODE_SHA | Release verification [30177462182](https://github.com/eggstack/eggsact/actions/runs/30177462182) — all 18 jobs success on `06f7a0b` | Complete |

## Publication status

Actual crates.io publication and annotated tag creation remain direct maintainer actions; this note records proof and release readiness, not publication itself.
