# Parser fuzzing (LANGS-11)

`parser` is a libFuzzer target. Its first input byte selects one of the 34 public diagram
parsers; the rest is decoded as UTF-8 with invalid sequences replaced. It prepends that parser's
header, checks that its syntax tree retains the source (accounting for lone-CR normalization in
flowchart and PlantUML), checks all diagnostic ranges and UTF-8 boundaries, and invokes diagram
detection and lowering. The target rejects inputs over 4096 bytes. The corpus has a seed for
each parser and lone-CR cases. It does not exercise rendering or input bytes before UTF-8 decoding.

CI's `fuzz` job (`.github/workflows/ci.yml`) runs the same smoke test on every pull request.

Install [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and a Rust nightly toolchain, then
from the repository root run a bounded smoke test on a disposable corpus copy:

```sh
mkdir -p /tmp/sirene-fuzz-smoke
cp fuzz/corpus/parser/* /tmp/sirene-fuzz-smoke/
cargo +nightly fuzz run parser /tmp/sirene-fuzz-smoke -- -runs=1000 -seed=1 -max_len=4096
```

Use a fresh corpus directory for each seeded run. For a longer campaign, omit `-runs` and use a
persistent corpus directory; libFuzzer adds new inputs there. Failures are saved in
`fuzz/artifacts/parser/`. Reproduce with `cargo +nightly fuzz run parser PATH_TO_ARTIFACT`, and
turn confirmed issues into ordinary regression tests. `cargo fuzz` enables coverage
instrumentation and sanitizers; this target is separate from the regular workspace tests.

If cargo-fuzz is unavailable, on x86-64 Linux with nightly and cached dependencies this command
runs the target with coverage instrumentation (but without cargo-fuzz's sanitizers):

```sh
RUSTFLAGS='-Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 -Cllvm-args=-sanitizer-coverage-inline-8bit-counters -Cllvm-args=-sanitizer-coverage-pc-table' \
  cargo +nightly run --offline --manifest-path fuzz/Cargo.toml --target x86_64-unknown-linux-gnu --bin parser -- \
  /tmp/sirene-fuzz-smoke -runs=1000 -seed=1 -max_len=4096 -artifact_prefix=/tmp/
```
