# Workspace

Cargo workspace layout, toolchain, CI, and the coverage gate.

## WORKSP-1

The repository root is a virtual Cargo workspace. Member crates live under `crates/`:

- `sirene-core`: parsing, diagram model, edit API, layout. No GUI or terminal dependencies.
- `sirene`: the `sirene` binary (CLI entrypoint, and later the GUI launcher).

Later crates (`sirene-render`, `sirene-gui`) are added by the specs that introduce them.

## WORKSP-2

Shared package metadata, lints, and dependency versions are declared once in the root
`Cargo.toml` (`[workspace.package]`, `[workspace.lints]`, `[workspace.dependencies]`) and
inherited by members.

## WORKSP-3

`unsafe_code` is forbidden workspace-wide. Clippy warnings fail CI.

## WORKSP-4

`rust-toolchain.toml` pins the `stable` channel with `rustfmt`, `clippy`, and
`llvm-tools-preview`.

## WORKSP-5

`scripts/ci.sh` runs the full local check: `cargo fmt --check`, `cargo clippy` with
`-D warnings` on all targets, `cargo test --workspace`, and the coverage gate. CI runs the same
script.

## WORKSP-6

The coverage gate is `cargo llvm-cov --workspace` with a minimum line coverage of 95%. A build
that drops below the threshold fails.

## WORKSP-7

CI (GitHub Actions) runs on pushes to `main` and on pull requests, on both `ubuntu-latest` and
`macos-latest`.
