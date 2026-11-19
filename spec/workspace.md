# Workspace

Cargo workspace layout, toolchain, CI, and the coverage gate.

## WORKSP-1

The repository root is a virtual Cargo workspace. The three published crates live under
`crates/`:

- `sirene-core`: parsing, diagram model, layout, ASCII and display-list drawing, SVG. No GUI or
  terminal dependencies.
- `sirene-gui`: the GPUI Kit editor (`gui-shell.md`, `wysiwyg-editing.md`). Only paints
  display lists; no layout or drawing logic.
- `sirene`: the `sirene` binary (CLI entrypoint and GUI launcher).

Two more members are not published: `xtask/` holds packaging and release-signing tasks, and
`wasm/` (`sirene-wasm`) exports `sirene-core` rendering to JavaScript on WebAssembly
(WORKSP-11).

## WORKSP-2

Shared package metadata, lints, and dependency versions are declared once in the root
`Cargo.toml` (`[workspace.package]`, `[workspace.lints]`, `[workspace.dependencies]`) and
inherited by members.

## WORKSP-3

`unsafe_code` is forbidden workspace-wide, except in `sirene-wasm`, which denies it and allows it
on each `#[unsafe(no_mangle)]` export alone; the exports' bodies are safe code. Clippy warnings
fail CI.

## WORKSP-4

`rust-toolchain.toml` pins the development toolchain to Rust `1.99.1`, with `rustfmt`, `clippy`,
and `llvm-tools-preview`. This is separate from the workspace's minimum supported Rust version,
`rust-version = "1.92"`, the newest minimum any locked dependency declares (from GPUI Kit's
tree).

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
`main`, to re-run a release that failed partway; every step is idempotent; any other ref skips;
a re-run of a version whose `vX.Y.Z` tag exists fails in `decide` unless `main` is at the
tagged commit):

1. `decide`: `bin/check-versions.sh` requires every dependency between publishable members to pin
   exactly that member's version, and no stable crate to depend on a pre-release. A `-dev`
   version, or one whose `vX.Y.Z` GitHub release exists, ends the run there. A release also
   needs its changelog section (`bin/changelog-section.sh`).
2. `ci` (Linux) and `ci-macos` run `scripts/ci.sh`; `audit` runs the RustSec scan, with the
   advisories `.cargo/audit.toml` ignores and why.
3. `build`, after all three: one `sirene-X.Y.Z-TARGET.tar.gz` (holding `sirene`) per target,
   built with `cargo auditable` and the pinned toolchain (WORKSP-9). The macOS and gnu legs also
   run `cargo package --workspace`, which build-checks each crate against the packaged crates
   before it. `sign`, in the `signing` environment, builds `xtask` in a step without the key,
   then signs each tarball with `target/release/xtask sign` (UPDATE-5; the key is never in the
   environment of a compile) and gives the signed tarballs a build provenance attestation. `app` builds the
   universal `Sirene.app` zip and cask (MACOSP-6). `wasm` builds `sirene-wasm-X.Y.Z.tar.gz`
   (WORKSP-11) and attests it; it is not signed, as nothing updates from it. It also packs the
   `@jaemk/sirene` npm package from the same files (`bin/npm-package.sh`).
4. `release`: creates the `vX.Y.Z` tag on the released commit and its GitHub release, with the
   changelog section as notes and the tarballs, the zip, the cask, and `SHA256SUMS` (of the
   tarballs and zip) as assets.
5. `publish`, in the `release` environment (approval required): `bin/publish.sh` publishes
   `sirene-core`, `sirene-gui`, then `sirene` with `--no-verify` (step 3 verified them, and a
   build here would run every build script with the token in its environment), treating an
   already-published version as a skip and any other failure as fatal. Then one commit to
   `main` writes `Formula/sirene.rb` (`bin/update-formula.sh`), `Casks/sirene.rb`, and, if
   `main` is still on `X.Y.Z`, the next `-dev` version (`bin/bump-dev.sh`). Last,
   `bin/npm-publish.sh` publishes `@jaemk/sirene` `X.Y.Z` with provenance, skipping a version
   already on npm.

crates.io authentication is trusted publishing (`rust-lang/crates-io-auth-action`) only, and so
is npm's: the `@jaemk/sirene` trusted publisher names `release.yml` and the `release`
environment, the package disallows token publishing, and no npm token exists.
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
The verified executable is copied to an exclusively created temporary file in the install
directory, made executable, then renamed over `sirene`. Failed staging leaves the old binary
unchanged; the temporary file is cleaned up. A destination symlink is replaced without writing
through it. A directory destination, including a symlink to a directory, is refused rather than
installing a nested `sirene/sirene`.

## WORKSP-10 Library rendering

`sirene-core` renders a diagram without the CLI. `source::read` reads a file as UTF-8 of at
most 64 MiB (`source::MAX_INPUT`), naming UTF-16 or the first invalid byte otherwise; the CLI
and the editor read files and stdin through it. `diagram::lower_as` reads a source, with
`Lang::of_path` picking DOT or PlantUML from a file extension. `Lowered::ascii`, `Lowered::svg`,
and `Lowered::png` (feature `png`) apply the diagram's own theme settings over the given theme
and return what `sirene ascii`, `sirene svg`, and `sirene png` write for the same file and
options; the CLI calls them. `markdown::mermaid_blocks` finds a Markdown file's diagram blocks,
each lowered with `lower_as(&block.code, Some(block.lang))`. Choosing output paths and
Markdown image links stay in the CLI.

## WORKSP-11 WebAssembly

`sirene-core`, with or without `png`, builds for `wasm32-unknown-unknown` and renders there.
That target has no clock: phase timings and layout timing logs are skipped, and a gantt chart
read through `diagram::lower` is read at the Unix epoch (`gantt::Parse::lower_at` takes the
date).

`sirene-wasm` builds `sirene-core` for the target as a module with no imports. JavaScript writes
UTF-8 source into its memory (`input(len)` returns where), calls `render(format, lang, theme)`
(SVG, ASCII, or PNG with feature `png`; detected, Mermaid, DOT, or PlantUML; light or dark),
and reads the drawing or an error message, and the diagnostics as a JSON array with 1-based
line and column (`output_ptr`/`output_len`, `diagnostics_ptr`/`diagnostics_len`). A panic traps;
`docs/assets/sirene-wasm.js`, the loader, then starts a new instance for the next call. The
`wasm` cargo profile builds it small, without `png`, for the docs site and releases.

The loader is a plain script that sets `globalThis.SireneWasm`, and `module.exports` under
CommonJS. `load` compiles the module from
its bytes, a fetch `Response`, or a promise of either (a failed fetch is an error naming the URL
and status). `render(source, {format, lang, theme})` returns `{ok, output, error,
diagnostics}`. `renderAll(scope, {theme})` replaces each `<pre><code class="language-X">` (X
one of `mermaid`, `plantuml`, `puml`, `dot`, `graphviz`), `<pre class="mermaid">`, and
`<div class="mermaid">` under `scope` with an `<img class="sirene-diagram">` of its SVG as a
`data:` URL, theme `light` by default; a block it cannot draw stays, with the error in
`data-sirene-error`. It returns `{drawn, failed}`.

Each release attaches `sirene-wasm-X.Y.Z.tar.gz` (WORKSP-8): a `sirene-wasm-X.Y.Z/` directory
with `sirene.wasm` (the `wasm` profile), `sirene-wasm.js`, `LICENSE-MIT`, `LICENSE-APACHE`, and
`THIRD-PARTY-NOTICES`. `examples/web/index.html` is a page that draws its blocks with them.
The `@jaemk/sirene` npm package (`wasm/npm/`, version `X.Y.Z`) holds the same five files, with
`sirene-wasm.js` as its main entry and `sirene.wasm` exported as `@jaemk/sirene/sirene.wasm`.

CI's `wasm` job builds `sirene-wasm` with `png` for the target and runs `scripts/wasm-test.mjs`
under Node: through the loader, it renders each file in `examples/` and a gantt chart as SVG,
ASCII, and PNG, requires no error diagnostics, and checks a diagnostic's line. Any failure fails
the job.

The docs site (`.github/workflows/pages.yml`) serves the module as `wasm/sirene.wasm`.
`docs/assets/custom.js` fetches it on first use: an `edit` button on each sample `samples.py`
draws swaps the code block for an editor and redraws the drawing and `sirene ascii` text as it
changes, showing the first diagnostics; the playground page does the same for Mermaid,
PlantUML, or DOT, keeping the language and source in the URL fragment (`#lang:base64url`).

## WORKSP-12 API compatibility

CI's `semver` job runs `cargo semver-checks` on `sirene-core` with all features, against its
latest crates.io release, on pull requests and pushes to `main`. A change that breaks the API
bumps the workspace `-dev` version to the next minor (`0.5.1-dev` to `0.6.0-dev`; a major once
past 1.0) in the same change, which the check accepts. `sirene-gui` and `sirene` are not checked:
the editor's API serves the binary only.
