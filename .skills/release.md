# Skill: Releasing eggsact

Use this when preparing or performing a release.

## Release Process

```bash
./release.sh   # full pipeline
```

The script runs in order:
1. Regenerate confusables data: `python3 scripts/generate_confusables.py`
2. Regenerate docs: `cargo run --bin generate-docs`
3. Check formatting: `cargo fmt --all -- --check`
4. Run clippy: `cargo clippy --all-targets --all-features -- -D warnings`
5. Run all tests: `cargo test`
6. Build release: `cargo build --release`
7. Check crates.io packaging: `cargo package`

## Pre-Release Checklist

- [ ] All tests pass: `cargo test`
- [ ] No formatting issues: `cargo fmt --all -- --check`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Generated docs current: `cargo run --bin generate-docs -- --check`
- [ ] Parity tests pass (when Python `eggcalc` is available at `../eggcalc`): `cargo test --test lib parity`
  - Note: As of 2026-07-04, the Rust parity suite has known gaps documented in `docs/parity.md` (`Verification status` and `Known parity gaps` sections). The 64-of-67 tool subset passes for matching tools; the remaining 53 failures are categorized as test-harness audience bug, tool/output drift, and a 3-tool gap. Closing these gaps is out of scope for release-polish and is tracked for follow-up work.
- [ ] Confusables data regenerated: `python3 scripts/generate_confusables.py`
- [ ] Crate packaging succeeds: `cargo package`
- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated

## Publishing to crates.io

```bash
cargo package
cargo publish
```

`cargo package` must be run on a clean worktree. If regenerating confusables changes
tracked files, commit those generated updates before packaging or publishing.

## Version Location

Version is defined in `Cargo.toml` and referenced in:
- `Cargo.toml` (source of truth)
- `docs/mcp-tools.md` (overview table)
- `CHANGELOG.md`

## CI Pipeline

CI runs on GitHub Actions (`.github/workflows/ci.yml`):
- Check formatting
- Run clippy with warnings denied
- Build on ubuntu-latest
- Run tests (unit + integration)
- Verify generated docs are current
- Run `cargo package`

## Cargo.lock

`Cargo.lock` is gitignored but present. Do not commit it — this is a binary crate convention.
