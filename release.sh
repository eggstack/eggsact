#!/bin/bash
set -e
cd "$(dirname "$0")"

echo "=== Regenerating confusables data from Unicode.org ==="
python3 scripts/generate_confusables.py

echo "=== Regenerating docs from ToolSpec registry ==="
cargo run --bin generate-docs

echo "=== Checking formatting ==="
cargo fmt --check

echo "=== Running clippy ==="
cargo clippy --all-targets --all-features -- -D warnings

echo "=== Running tests ==="
cargo test

echo "=== Building release ==="
cargo build --release

echo "=== Checking crates.io package ==="
cargo package
