# sirene

A local WYSIWYG Mermaid diagram editor and viewer, with terminal ASCII output.

See `spec/README.md` for the feature status table.

## Install

Install script (Linux and macOS):

```
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/jaemk/sirene/main/bin/install.sh | sh
```

It installs to `~/.local/bin` (`SIRENE_INSTALL_DIR`) and checks the download against the
release's `SHA256SUMS`. On Linux it installs the static musl build, which has no GUI; for the
GUI build, which needs the system's X11/Wayland and Vulkan libraries, set
`SIRENE_TARGET=x86_64-unknown-linux-gnu` (or `aarch64-...`).

Homebrew (macOS and Linux):

```
brew tap jaemk/sirene https://github.com/jaemk/sirene
brew trust jaemk/sirene
brew install sirene          # the binary
brew install --cask sirene   # macOS: Sirene.app, with `sirene` linked onto the PATH
```

The repo is not named `homebrew-sirene`, so the tap needs its clone URL. Install one of the
formula or the cask, not both: each puts `sirene` on the PATH. The app is not signed, so macOS
asks before the first launch.

With the Rust toolchain: `cargo install sirene` (add `--no-default-features` for the CLI only).
Release binaries are also on the [releases page](https://github.com/jaemk/sirene/releases).

## Usage

```
$ printf 'flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n' | sirene ascii
┌───────┐    ╱─────╲       ┌──────┐
│ Start ├───►│ Ok? ├──yes─►│ Done │
└───────┘    ╲─────╱       └──────┘
```

`sirene ascii FILE` reads a file; `--charset ascii` uses only ASCII characters.

```
sirene diagram.mmd           # edit the source beside a live drawing; follows changes on disk
sirene svg diagram.mmd       # print SVG (--theme light|dark, -o FILE)
sirene png diagram.mmd       # write diagram.png (--scale 2, --theme, -o FILE)
```

`-v`/`--verbose`, before or after the command, prints each step (reading, parsing, layout,
rendering, writing, and in the GUI opening, reloading, and saving files) with timings to stderr.
`svg`, `png`, and `ascii` never open a window. `--no-default-features` builds a CLI-only
binary that does not link the GUI. PNG text uses the
embedded Noto Sans faces (`crates/sirene-core/fonts`, SIL OFL 1.1), so images are the same on every
machine.

On the drawing: click to select (shift-click for more), double-click or enter to edit a label,
drag from one node to another to connect them, secondary-shift-n to add a node, backspace to
delete. Each change is written to the source as an ordinary text edit.

`examples/` has diagrams to try (`sirene examples/architecture.mmd`, or
`cargo run -p sirene -- examples/shapes.mmd` from a checkout); a test keeps them free of
diagnostics.

On Linux, building needs the XCB, xkbcommon, Wayland, fontconfig, and Vulkan development
packages (see `.github/workflows/ci.yml`).

## Development

```
bash scripts/ci.sh
```

Runs fmt, clippy, tests, and the coverage gate (needs `cargo install cargo-llvm-cov`).

## Packaging

```
cargo build --release --package sirene
cargo xtask bundle --binary target/release/sirene    # target/bundle/Sirene.app
cargo xtask icon                                     # target/bundle/sirene.{icns,png}
cargo xtask cask --url URL --sha256 HEX --homepage URL
```

## Releasing

`main` carries the next `-dev` version between releases. To release, set `version` in the root
`Cargo.toml` (and the matching `[workspace.dependencies]` pins) to `X.Y.Z`, add a `## [X.Y.Z]`
section to `CHANGELOG.md`, and merge to `main`. `.github/workflows/release.yml` runs CI and the
RustSec audit, builds the binaries and `Sirene.app`, creates the `vX.Y.Z` release with them,
then, after approval in the `release` environment, publishes `sirene-core`, `sirene-gui`, and
`sirene` and commits the formula, the cask, and the next `-dev` version to `main`. See
`spec/workspace.md` (WORKSP-8, WORKSP-9) and `spec/macos-packaging.md`.
