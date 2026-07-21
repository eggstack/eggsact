# MSRV Evidence

This document records the minimum supported Rust version (MSRV) for the eggsact crate and the evidence supporting the selection.

## Declared MSRV

**Rust 1.89.0** — declared in `Cargo.toml` as `rust-version = "1.89.0"`.

## Rationale

The MSRV is set by the most restrictive requirement in the dependency graph:

- **hashbrown 0.17.1** requires Rust 1.85.0 for its core functionality.
- **Temporary lifetime extension** (`let` expressions in match arms and closures) stabilized in Rust 1.89.0. Test code in `tests/property/` and `tests/mcp/` uses this feature.
- No dependency requires a version higher than 1.89.0.

Therefore 1.89.0 is the lowest version that compiles the library, all binaries, and the full test suite.

## Test commands

The following commands must pass on the declared MSRV:

```bash
cargo +1.89.0 check --locked --all-targets --all-features
cargo +1.89.0 test --locked --all-features --lib
cargo +1.89.0 test --locked --all-features --bins
cargo +1.89.0 test --locked --doc
```

## Test history

| Date | Commit | MSRV | Result |
|------|--------|------|--------|
| 2026-07-21 | 536c380 | 1.89.0 | CI passes (Ubuntu, Windows, macOS) |

## MSRV policy

- MSRV may be raised in a MINOR release with a changelog entry.
- An MSRV increase must be justified by a dependency requirement or language feature need.
- The locked dependency graph must resolve on MSRV.
- CI blocks on MSRV test failures.
- MSRV covers the library, all binaries, and the supported test subset.

See `docs/compatibility-policy.md` for the full compatibility policy.
