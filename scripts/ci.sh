#!/usr/bin/env bash
# Full local check; CI runs this same script.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo llvm-cov --workspace --locked --fail-under-lines 95
