#!/usr/bin/env bash
set -euo pipefail

# Match CI: promote all warnings to errors so local test catches the same
# lint failures that RUSTFLAGS="-D warnings" catches in GitHub Actions.
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings"

echo "=== cargo test --workspace --features oriterm/gpu-tests ==="
cargo test --workspace --features oriterm/gpu-tests

echo ""
echo "All tests passed."
