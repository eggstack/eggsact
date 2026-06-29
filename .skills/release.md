# Skill: Releasing eggsact

Use this when preparing or performing a release.

## Release Process

```bash
./release.sh   # full pipeline
```

The script runs in order:
1. Regenerate confusables data: `python3 scripts/generate_confusables.py`
2. Check formatting: `cargo fmt --check`
3. Run clippy: `cargo clippy --all-targets --all-features`
4. Run all tests: `cargo test`
5. Build release: `cargo build --release`

## Pre-Release Checklist

- [ ] All tests pass: `cargo test`
- [ ] No formatting issues: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features`
- [ ] Parity tests pass: `cargo test --test lib parity`
- [ ] Confusables data regenerated: `python3 scripts/generate_confusables.py`
- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated

## Publishing to crates.io

```bash
cargo publish
```

## Version Location

Version is defined in `Cargo.toml` and referenced in:
- `Cargo.toml` (source of truth)
- `docs/mcp-tools.md` (overview table)
- `CHANGELOG.md`

## CI Pipeline

CI runs on GitHub Actions (`.github/workflows/ci.yml`):
- Build on ubuntu-latest
- Run tests (unit + integration)
- Does NOT run `cargo fmt` or `cargo clippy` — those are only enforced via `release.sh`

## Cargo.lock

`Cargo.lock` is gitignored but present. Do not commit it — this is a binary crate convention.
