#!/bin/bash
set -e
cd "$(dirname "$0")"

echo "=== Regenerating confusables data from Unicode.org ==="
python3 scripts/generate_confusables.py

echo "=== Building release ==="
cargo build --release
