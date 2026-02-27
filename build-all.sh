#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"

echo "=== cargo build --workspace (${TARGET}, debug) ==="
cargo build --workspace --target "${TARGET}"

echo ""
echo "=== cargo build --workspace (${TARGET}, release) ==="
cargo build --workspace --target "${TARGET}" --release

echo ""
echo "Build succeeded (debug + release)."
