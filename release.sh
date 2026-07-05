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

echo "=== Running tests ==="
cargo test --all-features

echo "=== Checking generated docs freshness ==="
cargo run --bin generate-docs -- --check

echo "=== Checking crates.io package ==="
cargo package --verbose
