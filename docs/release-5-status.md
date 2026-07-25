# Release 5 Status Note

**Date:** 2026-07-25 UTC  
**Final verification baseline:** `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`

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

## Release closure

- [x] Every planned fuzz target builds against bounded input
- [x] Persistent corpora are committed and seeded with historical regressions
- [x] Calculator, diff, shell, regex, JSON, TOML/config, Unicode, Markdown, and glob/path surfaces have fuzz coverage
- [x] Core properties are enforced in ordinary tests
- [x] Fuzz target assertions match implemented guarantees
- [x] No known crash, hang, OOM, stack overflow, or invariant failure remains untriaged
- [x] PR smoke fuzzing is bounded and cancellable
- [x] Scheduled/manual extended fuzzing uses a matrix strategy with per-target timeouts
- [x] Sanitizer matrix passed on the final SHA
- [x] Fuzz dependencies and artifacts are excluded from normal package/runtime dependencies
- [x] Ordinary CI, cargo-deny, generated docs, package, release, and parity gates pass

Actual crates.io publication remains a direct maintainer action; this note
records proof and release readiness, not publication itself.
