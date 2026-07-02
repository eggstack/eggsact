# Skill: Releasing eggsact

Use this when preparing or performing a release.

## Release Process

```bash
./release.sh   # full pipeline
```

The script runs in order:
1. Regenerate confusables data: `python3 scripts/generate_confusables.py`
2. Regenerate docs: `cargo run --bin generate-docs`
3. Check formatting: `cargo fmt --check`
4. Run clippy: `cargo clippy --all-targets --all-features`
5. Run all tests: `cargo test`
6. Build release: `cargo build --release`
7. Check crates.io packaging: `cargo package`

## Pre-Release Checklist

- [ ] All tests pass: `cargo test`
- [ ] No formatting issues: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features`
- [ ] Generated docs current: `cargo run --bin generate-docs -- --check`
- [ ] Parity tests pass: `cargo test --test lib parity`
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
