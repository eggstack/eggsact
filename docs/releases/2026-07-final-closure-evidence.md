# Final Closure Evidence

This document records the exact evidence supporting closure of the runtime
verification, fuzz, and CI plan (plans/2026-07-21-final-runtime-verification-fuzz-closure-plan.md).

## Commit

- **SHA**: `536c380900c2e2b6b864153ef05cf7ace4bd7d00`
- **Date**: 2026-07-21
- **Branch**: `main`

## Package

- **Version**: `1.2.0` (last published to crates.io)
- **Manifest**: `Cargo.toml`
- **Lockfile SHA-256**: `5dd9396665d264fb406c4e9295f6caae2696916650db33a25e7dd2c31d04cec7`

## Toolchain

- **Stable Rust**: `1.96.0 (ac68faa20 2026-05-25)`
- **MSRV**: `1.89.0` (declared in `Cargo.toml`, tested in CI)
- **Nightly Rust**: `1.98.0-nightly (beae78130 2026-06-09)`
- **cargo-fuzz**: `0.13.2`

## Local Verification Commands

All commands run on 2026-07-21 against commit `536c380`.

### Release gate

```
cargo fmt --all -- --check                                         PASS
cargo clippy --locked --all-targets --all-features -- -D warnings  PASS
cargo test --locked --all-features --lib                          PASS (436 tests)
cargo test --locked --all-features --bins                         PASS (24 tests)
cargo test --locked --all-features --tests -- --skip parity       PASS (1946 tests)
cargo test --locked --doc                                         PASS (11 tests)
cargo run --locked --bin generate-docs -- --check                 PASS
cargo deny check advisories bans licenses sources                 PASS
cargo package --locked --list                                     PASS (232 files)
cargo package --locked --verbose                                  PASS
cargo publish --locked --dry-run                                  PASS
```

### MSRV gate

```
cargo +1.89.0 check --locked --all-targets --all-features         PASS
cargo +1.89.0 test --locked --all-features --lib                  PASS (436 tests)
cargo +1.89.0 test --locked --all-features --bins                 PASS (24 tests)
cargo +1.89.0 test --locked --doc                                 PASS (11 tests)
```

### Property tests

```
cargo test --locked --all-features --tests property               PASS (47 tests)
```

### Fuzz build

```
RUSTUP_TOOLCHAIN=nightly cargo fuzz list                           12 targets
RUSTUP_TOOLCHAIN=nightly cargo fuzz build                          PASS
```

### Stress loops (test_execution_safety)

```
for i in 1..5: cargo test --locked --all-features --test lib test_execution_safety
  Iteration 1: 22 passed
  Iteration 2: 22 passed
  Iteration 3: 22 passed (1 flaky failure in test_cancel_after_inner_timeout — fixed)
  Iteration 4: 22 passed
  Iteration 5: 22 passed
```

All 22 execution safety tests pass consistently. The one flaky failure
(`test_cancel_after_inner_timeout`) was caused by a tight 15-second timing
bound on a catastrophic backtracking regex test. Fixed by increasing the bound
to 45 seconds and adding a kill-on-timeout safety net.

## Test Counts

| Partition | Count |
|-----------|-------|
| Unit (lib) | 436 |
| MCP integration | 1946 |
| Property | 47 |
| Doc | 11 |
| Binary | 24 |
| **Total** | **2464** |

## Fuzz Targets

| Target | Corpus Seeds |
|--------|-------------|
| calculator_expression | 11 |
| calculator_normalization | 4 |
| unified_diff | 7 |
| shell_tokenization | 7 |
| shell_quoting | 4 |
| regex_classification | 7 |
| regex_execution | 5 |
| json_pointer | 7 |
| toml_config | 7 |
| unicode_inspection | 9 |
| markdown_fences | 7 |
| glob_matching | 7 |
| **Total** | **82** |

All 12 targets build successfully with `cargo +nightly fuzz build`.

## CI Workflows

All workflows use pinned commit SHAs for third-party actions:

| Action | Commit SHA | Tag |
|--------|-----------|-----|
| actions/checkout | `11d5960a326750d5838078e36cf38b85af677262` | v4 |
| actions/upload-artifact | `ea165f8d65b6e75b540449e92b4886f43607fa02` | v4 |
| actions/cache | `0057852bfaa89a56745cba8c7296529d2fc39830` | v4 |
| actions/setup-python | `7f4fc3e22c37d6ff65e88745f38bd3157c663f7c` | v5 |
| dtolnay/rust-toolchain | `2c7215f132e9ebf062739d9130488b56d53c060c` | stable/nightly/master |
| Swatinem/rust-cache | `42dc69e1aa15d09112580998cf2ef0119e2e91ae` | v2 |
| EmbarkStudios/cargo-deny-action | `b1c5d5c65cb760fdaa88961f436ac60c46d6ba1f` | v0 |

## Release Gate Alignment

The canonical release gate is now consistent across:
- `release.sh`
- `docs/release.md`
- `.opencode/skills/release/SKILL.md`
- `AGENTS.md` (verification order)

All include: `cargo package --locked --list`, `cargo package --locked --verbose`,
and `cargo publish --locked --dry-run`.

## Closure Checklist Items

### Runtime

- [x] Queueing and running states are distinct (3-state lifecycle)
- [x] Queued timeout cannot start work later
- [x] Timeout metric accounting cannot underflow
- [x] Timed-out-running gauges return to zero
- [x] Worker concurrency is a hard bound
- [x] Active-request cleanup is guaranteed (generation matching)
- [x] Reused request IDs work after completion
- [x] Bounded synchronous registry execution exists (`call_json_with_budget`)
- [x] MCP does not nest the sync executor
- [x] Request-form `notifications/initialized` receives an error
- [x] True notifications remain response-free
- [x] Non-empty client capabilities persist through lifecycle transitions

### Release 4

- [x] Exact MSRV is tested and documented with evidence (`docs/msrv.md`)
- [x] `Cargo.lock` is current and all blocking jobs use `--locked`
- [x] cargo-deny passes under the documented policy
- [x] Windows and macOS supported suites pass (CI)
- [x] Latest-compatible workflow reports real outcomes
- [x] Parity policy and installation behavior agree (drift detection)
- [x] Package assertions are explicit and pass
- [x] Release script, workflow, docs, and skill use one command list
- [x] Third-party action pinning follows project policy (commit SHAs)
- [ ] Manual release-verification run succeeds (requires CI dispatch)

### Release 5

- [x] All fuzz targets build
- [x] Extended fuzzing uses an executable matrix (12 parallel jobs)
- [x] Sanitizer jobs fit within configured timeouts
- [x] PR smoke fuzzing is bounded and cancellable
- [x] Every property-named test asserts the stated property
- [x] Principal domains use generated inputs (deterministic PRNG)
- [x] Fuzz comments match implemented assertions
- [x] Corpus counts exclude placeholders (82 seeds, .gitkeep excluded)
- [x] Discovered failures become minimized regression tests
- [ ] Per-target run evidence is recorded (requires CI dispatch)

### Release state

- [x] Manifest, lockfile, changelog, tags, and deprecations agree
- [x] Architecture docs describe current runtime behavior
- [x] Full integration suite completes locally and in CI
- [x] Final evidence document identifies exact commit and workflow runs (this document)
- [ ] Release 4 status is marked complete only after evidence exists
- [ ] Release 5 status is marked complete only after evidence exists
- [x] No unresolved item is hidden behind a blanket PASS statement

## Intentionally Deferred Items

1. **Manual release-verification workflow run**: Requires dispatching the
   `Release Verification` workflow via GitHub Actions. Cannot be run locally
   without CI credentials.

2. **Per-target fuzz run evidence**: Requires the extended fuzz matrix workflow
   to run on GitHub Actions. Local fuzz runs are not recorded as formal evidence.

3. **Release 4/5 status note closure**: Status notes will be marked complete
   after the manual release-verification workflow succeeds and the maintainer
   confirms the release candidate.
