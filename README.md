# sirene

A local WYSIWYG editor and viewer for Mermaid flowcharts, with SVG, PNG, and terminal ASCII
output. Only flowcharts (`flowchart` and `graph`) are drawn; other Mermaid diagram types are
reported as unsupported.

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
sirene png diagram.mmd       # write diagram.png (--quality draft|normal|fine, --scale N, --theme, -o FILE)
sirene png --watch docs/     # write a .png beside each .mmd under docs/, again on each change
```

`-v`/`--verbose`, before or after the command, prints each step (reading, parsing, layout,
rendering, writing, and in the GUI opening, reloading, and saving files) with timings to stderr.
`svg`, `png`, and `ascii` never open a window. `--no-default-features` builds a CLI-only
binary that does not link the GUI. PNG text uses the
embedded Noto Sans faces (`crates/sirene-core/fonts`, SIL OFL 1.1), so images are the same on every
machine.

### The editor

Each window shows the source beside a live drawing. Below, secondary is Cmd on macOS and Ctrl
elsewhere; every action is also in the menus (the menu bar on macOS, the top of each window
elsewhere). The full key table is in `spec/gui-shell.md` (GUISHE-5).

- Click the drawing to give it the keyboard (it shows an accent frame); secondary-e shows or
  hides the source pane, and the divider between them drags.
- Click to select (shift-click for more), double-click or enter to edit a label, drag from one
  node to another to connect them, secondary-shift-n to add a node, backspace to delete. Each
  change is written to the source as an ordinary text edit, and secondary-z undoes it.
- Right-click a node or edge for its menu: change a node's shape, an edge's line, arrowheads, or
  length, reverse an edge, add a node before or after, or connect to or from any other node.
- The shape bar at the top left of the drawing adds a node of its shape, linked from the selected
  node. Double-click empty space to add a node; drag from a node to empty space to add a linked
  one.
- Layout hints, from the right-click menu: the direction of the diagram or a subgraph, Group
  (secondary-g) and Ungroup, Move To a subgraph (or drag a node onto one), Keep Together (an
  invisible `~~~` edge), and Move Earlier or Later (alt-up, alt-down) to reorder statements.
- View > Show or Hide Styles (secondary-i) opens a panel of the document's classes: check one
  to give it to the selected nodes, or edit a class's fill, stroke, text color, width, and
  dashes. It writes `classDef` and `class` statements.
- Scroll or drag empty space to pan, secondary-scroll to zoom; the arrow keys pan (shift for
  further), tab steps through nodes, secondary-8 zooms to the selection, secondary-9 fits the
  window, secondary-0 is 100%.
- File > Export writes PNG or SVG, and secondary-shift-c copies the drawing as an image.
- View > Appearance overrides the system's light or dark mode. It, File > Open Recent, and the
  window and source pane sizes are kept in `~/.config/sirene/settings`
  (`~/Library/Application Support/sirene/settings` on macOS).

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
