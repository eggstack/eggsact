#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "release-check: working tree is not clean" >&2
  exit 1
fi

echo "release-check: formatting"
cargo fmt --all -- --check

echo "release-check: generated docs"
cargo run --locked --bin generate-docs -- --check

echo "release-check: clippy"
cargo clippy --locked --all-targets --all-features -- -D warnings

echo "release-check: tests"
cargo test --locked --all-features -- --skip parity --test-threads=4

echo "release-check: doc tests"
cargo test --locked --doc

echo "release-check: cargo-deny"
if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "cargo-deny is required: cargo install cargo-deny --version 0.19.0 --locked" >&2
  exit 1
fi
cargo deny check advisories bans licenses sources

echo "release-check: package"
cargo package --locked

echo "release-check: publish dry run"
cargo publish --locked --dry-run

echo "release-check: passed; no publication was performed"
