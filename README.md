# sirene

A local WYSIWYG Mermaid diagram editor and viewer, with terminal ASCII output.

See `spec/README.md` for the feature status table.

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

`svg`, `png`, and `ascii` never open a window. `cargo install --path crates/sirene
--no-default-features` builds a CLI-only binary that does not link the GUI. PNG text uses the
embedded Noto Sans (`crates/sirene-core/fonts`, SIL OFL 1.1), so images are the same on every
machine.

On the drawing: click to select (shift-click for more), double-click or enter to edit a label,
drag from one node to another to connect them, secondary-shift-n to add a node, backspace to
delete. Each change is written to the source as an ordinary text edit.

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

Bump `version` in the root `Cargo.toml` (and the matching `[workspace.dependencies]` pins) and
merge to `main`. `.github/workflows/release.yml` sees the new version is not on crates.io and,
after approval in the `release` environment, publishes `sirene-core`, `sirene-gui`, and
`sirene`, tags `vX.Y.Z`, creates the GitHub release, and attaches a universal `Sirene.app` zip
and a Homebrew cask. The app is not signed. See `spec/workspace.md` (WORKSP-8) and
`spec/macos-packaging.md`.
