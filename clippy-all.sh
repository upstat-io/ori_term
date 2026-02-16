#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"

echo "=== cargo clippy --workspace (${TARGET}) ==="
cargo clippy --workspace --target "${TARGET}" -- -D warnings

echo ""
echo "=== cargo clippy --workspace (host) ==="
cargo clippy --workspace -- -D warnings

echo ""
echo "All clippy checks passed."
