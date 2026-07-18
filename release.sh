#!/bin/bash
set -e
cd "$(dirname "$0")"

echo "=== Regenerating confusables data from Unicode.org ==="
python3 scripts/generate_confusables.py

echo "=== Regenerating docs from ToolSpec registry ==="
cargo run --bin generate-docs

echo "=== Checking formatting ==="
cargo fmt --all -- --check

echo "=== Running clippy ==="
cargo clippy --all-targets --all-features -- -D warnings

echo "=== Running unit tests ==="
cargo test --locked --all-features --lib

echo "=== Running binary tests ==="
cargo test --locked --all-features --bins

echo "=== Running integration tests (parity excluded) ==="
cargo test --locked --all-features --tests -- --skip parity

echo "=== Running doc tests ==="
cargo test --locked --doc

echo "=== Checking generated docs freshness ==="
cargo run --locked --bin generate-docs -- --check

echo "=== Checking crates.io package ==="
cargo package --locked --verbose

echo "=== Running cargo-deny checks ==="
cargo deny check advisories bans licenses sources
