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

CI (GitHub Actions) runs on pushes to `main` and on pull requests, on both `ubuntu-24.04` (pinned, as
in the release workflow) and `macos-latest`. On Linux, gpui needs the XCB, xkbcommon, Wayland, fontconfig, and Vulkan
development packages installed first. An `audit` job runs `cargo audit` against `Cargo.lock` on
the same triggers and weekly, with the ignores in `.cargo/audit.toml`.

## WORKSP-8 Releases

Merging a version bump to `main` is the release. Between releases `main` carries the next
patch's `-dev` version. To release, set the version (`[workspace.package]` and the
`[workspace.dependencies]` pins) to a stable `X.Y.Z` and add a `## [X.Y.Z]` section to
`CHANGELOG.md`. `.github/workflows/release.yml` runs on every push to `main` (and by hand from
`main`, to re-run a release that failed partway; every step is idempotent; any other ref skips):

1. `decide`: `bin/check-versions.sh` requires every dependency between publishable members to pin
   exactly that member's version, and no stable crate to depend on a pre-release. A `-dev`
   version, or one whose `vX.Y.Z` GitHub release exists, ends the run there. A release also
   needs its changelog section (`bin/changelog-section.sh`).
2. `ci` (Linux) and `ci-macos` run `scripts/ci.sh`; `audit` runs the RustSec scan, with the
   advisories `.cargo/audit.toml` ignores and why.
3. `build`, after all three: one `sirene-X.Y.Z-TARGET.tar.gz` (holding `sirene`) per target,
   built with `cargo auditable` and the pinned toolchain (WORKSP-9). The macOS and gnu legs also
   run `cargo package --workspace`, which build-checks each crate against the packaged crates
   before it. `sign`, in the `signing` environment, signs each tarball with `cargo xtask sign`
   (UPDATE-5) and gives the signed tarballs a build provenance attestation. `app` builds the
   universal `Sirene.app` zip and cask (MACOSP-6).
4. `release`: creates the `vX.Y.Z` tag on the released commit and its GitHub release, with the
   changelog section as notes and the tarballs, the zip, the cask, and `SHA256SUMS` (of the
   tarballs and zip) as assets.
5. `publish`, in the `release` environment (approval required): `bin/publish.sh` publishes
   `sirene-core`, `sirene-gui`, then `sirene` with `--no-verify` (step 3 verified them, and a
   build here would run every build script with the token in its environment), treating an
   already-published version as a skip and any other failure as fatal. Then one commit to
   `main` writes `Formula/sirene.rb` (`bin/update-formula.sh`), `Casks/sirene.rb`, and, if
   `main` is still on `X.Y.Z`, the next `-dev` version (`bin/bump-dev.sh`).

crates.io authentication is trusted publishing (`rust-lang/crates-io-auth-action`) only.
Tests: `xtask/tests/release_scripts.rs`.

## WORKSP-9 Binaries

Each release carries `sirene` for `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`aarch64-unknown-linux-gnu`, and `x86_64-unknown-linux-gnu`, which include the GUI, and for
`aarch64-unknown-linux-musl` and `x86_64-unknown-linux-musl`, which are statically linked and
built without it (`--no-default-features`, CLI-7).

The repository is a Homebrew tap. `Formula/sirene.rb` installs the macOS and gnu binaries;
`Casks/sirene.rb` installs `Sirene.app` (MACOSP-5).

`bin/install.sh` installs the binary for the host into `SIRENE_INSTALL_DIR` (default
`~/.local/bin`): the latest release unless `SIRENE_VERSION` is set (a leading `v` is dropped),
and the musl build on Linux unless `SIRENE_TARGET` names another. If a release has no musl
build and `SIRENE_TARGET` is unset, Linux falls back once to the gnu build. It refuses an unsupported OS or architecture before any request, and a tarball
whose checksum is not in `SHA256SUMS` or does not match. When `gh` is present it checks the
build provenance attestation, reporting but not failing on a mismatch.
