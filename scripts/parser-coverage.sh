#!/usr/bin/env bash
# Source coverage for parser properties and replayed fuzz corpora.
set -euo pipefail

cd "$(dirname "$0")/.."

parser_corpus=${1:-fuzz/corpus/parser}
document_corpus=${2:-fuzz/corpus/document}
report_dir="$PWD/target/parser-coverage"
mkdir -p "$report_dir"
export PROPTEST_RNG_SEED=${PROPTEST_RNG_SEED:-8128}

export CARGO_LLVM_COV_TARGET_DIR="$report_dir/properties"
cargo llvm-cov clean --profraw-only --locked
cargo llvm-cov --locked --no-report -p sirene-core \
  --test parser_contract --test parser_generated --test parser_limits_props \
  --test flowchart_props --test dot_props --test mermaid_props --test plantuml_props \
  -- --skip fuzz_selector_mapping_is_stable_and_unique \
  --skip carriage_returns_follow_each_parsers_contract --skip valid_sources_and_recovery
cargo llvm-cov report --locked --json --output-path "$report_dir/properties.json"

export CARGO_LLVM_COV_TARGET_DIR="$report_dir/fuzz-replay"
cargo llvm-cov clean --manifest-path fuzz/Cargo.toml --profraw-only --locked
cargo llvm-cov run --manifest-path fuzz/Cargo.toml --locked --no-report \
  --dep-coverage sirene-core --bin parser \
  -- "$parser_corpus" -runs=0
cargo llvm-cov run --manifest-path fuzz/Cargo.toml --locked --no-report \
  --dep-coverage sirene-core --bin document \
  -- "$document_corpus" -runs=0

# Merge both binaries' profiles and report coverage for the local sirene-core dependency.
llvm_bin="$(rustc --print target-libdir)/../bin"
fuzz_dir="$CARGO_LLVM_COV_TARGET_DIR"
"$llvm_bin/llvm-profdata" merge -sparse "$fuzz_dir"/*.profraw -o "$fuzz_dir/replay.profdata"
"$llvm_bin/llvm-cov" export -instr-profile="$fuzz_dir/replay.profdata" \
  -object "$fuzz_dir/debug/parser" -object "$fuzz_dir/debug/document" \
  -ignore-filename-regex='(^|/)(\.cargo|rustc|\.rustup|fuzz_targets)/' > "$report_dir/fuzz.json"
