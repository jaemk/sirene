# Workspace

Cargo workspace layout, toolchain, CI, and the coverage gate.

## WORKSP-1

The repository root is a virtual Cargo workspace. Member crates live under `crates/`:

- `sirene-core`: parsing, diagram model, layout, ASCII and display-list drawing, SVG. No GUI or
  terminal dependencies.
- `sirene-gui`: the GPUI Kit editor (`gui-shell.md`, `wysiwyg-editing.md`). Only paints
  display lists; no layout or drawing logic.
- `sirene`: the `sirene` binary (CLI entrypoint and GUI launcher).

## WORKSP-2

Shared package metadata, lints, and dependency versions are declared once in the root
`Cargo.toml` (`[workspace.package]`, `[workspace.lints]`, `[workspace.dependencies]`) and
inherited by members.

## WORKSP-3

`unsafe_code` is forbidden workspace-wide. Clippy warnings fail CI.

## WORKSP-4

`rust-toolchain.toml` pins the `stable` channel with `rustfmt`, `clippy`, and
`llvm-tools-preview`. The workspace `rust-version` is the newest one any locked dependency
declares (1.92, from GPUI Kit's tree).

## WORKSP-5

`scripts/ci.sh` runs the full local check: `cargo fmt --check`, `cargo clippy` with
`-D warnings` on all targets, `cargo test --workspace`, and the coverage gate. CI runs the same
script.

## WORKSP-6

The coverage gate is `cargo llvm-cov --workspace` with a minimum line coverage of 95%. A build
that drops below the threshold fails.

## WORKSP-7

CI (GitHub Actions) runs on pushes to `main` and on pull requests, on both `ubuntu-latest` and
`macos-latest`. On Linux, gpui needs the XCB, xkbcommon, Wayland, fontconfig, and Vulkan
development packages installed first.

## WORKSP-8 Releases

Merging a version bump to `main` is the release. `.github/workflows/release.yml` runs on every
push to `main` (and by hand):

1. `check`: `bin/check-versions.sh` requires every dependency between publishable members to pin
   exactly that member's version, and no stable crate to depend on a pre-release; then the job
   publishes only when the `sirene` version is not on crates.io.
2. `publish`, in the `release` environment (approval required): `bin/publish.sh` publishes
   `sirene-core`, `sirene-gui`, then `sirene`, treating an already-published version as a skip
   and any other failure as fatal; then `bin/tag-release.sh` tags and creates a GitHub release
   for each crate's current version that lacks one (`vX.Y.Z` for `sirene`, `NAME-vX.Y.Z` for the
   others).
3. `macos`: builds the app for `vX.Y.Z` (MACOSP-6) and attaches it to that release.

crates.io authentication is trusted publishing (`rust-lang/crates-io-auth-action`). A crate's
first version cannot be published that way, so while the `CARGO_REGISTRY_TOKEN` secret is set
the job uses it instead. Tests: `xtask/tests/release_scripts.rs`.
