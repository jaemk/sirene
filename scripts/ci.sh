#!/usr/bin/env bash
# Full local check; CI runs this same script.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --package sirene --no-default-features --locked
cargo llvm-cov --workspace --locked --fail-under-lines 95
# CI scans in its own `audit` job; run the scan here when it is installed.
if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit
else
    echo "cargo-audit not installed; skipping the advisory scan"
fi
