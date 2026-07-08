# Release Checklist

## Pre-release

1. Ensure the working tree is clean (`git status` shows no uncommitted changes).

2. Verify the version in `Cargo.toml` matches the intended release version.

3. Update `CHANGELOG.md` with an entry for the release.

## Verification

Run the full verification pipeline before tagging:

```sh
./release.sh
```

This runs, in order:

1. Regenerate confusable-character data
2. `cargo fmt --all -- --check`
3. `cargo clippy --all-targets --all-features -- -D warnings`
4. `cargo test --all-features` (unit + integration tests)
5. `cargo run --bin generate-docs -- --check` (generated docs freshness)
6. `cargo package --verbose`

### Verification order (manual)

If you need to run steps individually:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

### Parity tests (local only)

Parity tests are excluded from CI (Python `eggcalc` is not available in CI). Run locally after the main suite passes:

```sh
cargo build
cargo test --test lib parity
```

See `docs/parity.md` for the full parity framework, accepted failures, and known gaps.

## Generated docs

Regenerate docs from the `ToolSpec` registry after any tool changes:

```sh
cargo run --bin generate-docs
```

This updates:
- README tool tables (auto-generated section between `<!-- BEGIN GENERATED -->` markers)
- Architecture profile references
- `generated/tool-cards.md`

Verify freshness with:

```sh
cargo run --bin generate-docs -- --check
```

## Package content check

Verify the crates.io package contents are correct:

```sh
cargo package --verbose
```

This produces a `.crate` file and lists its contents. Verify no unintended files are included and no required files are missing.

## Publish

After all gates pass:

```sh
cargo publish
```

For a dry run without publishing:

```sh
cargo publish --dry-run
```

## Post-release

1. Create a git tag for the release version:
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

2. Verify the crate appears on [crates.io](https://crates.io/crates/eggsact).

3. Update `Cargo.toml` version to the next development version if needed.

## CI

GitHub Actions runs on push/PR to `main` (plus manual `workflow_dispatch`):

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features --lib` (unit tests)
- `cargo test --all-features --bins` (binary tests)
- `cargo test --all-features --tests -- --skip parity` (integration tests)
- `cargo run --bin generate-docs -- --check` (generated docs freshness)
- `cargo package --verbose` (after all checks pass)

Parity tests are not run in CI. They must be validated locally before release.
