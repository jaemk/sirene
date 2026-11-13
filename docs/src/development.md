# Development

The workspace has three crates:

| Crate | What |
|---|---|
| `sirene-core` | the parser, model, edits, layout, and SVG, PNG, and ASCII renderers; no GUI |
| `sirene-gui` | the editor, on gpui and GPUI Kit |
| `sirene` | the binary; the `gui` feature (on by default) links `sirene-gui` |

The behavior of each part is specified in [`spec/`](https://github.com/jaemk/sirene/tree/main/spec),
with stable ids (`FLOWCH-7`, `GUISHE-5`) that tests and code comments refer to.
[`spec/README.md`](https://github.com/jaemk/sirene/blob/main/spec/README.md) has the feature
status table.

## Building

```
cargo build                                  # with the editor
cargo build -p sirene --no-default-features  # the command line only
cargo run -p sirene -- examples/shapes.mmd
```

On Linux the editor needs the XCB, xkbcommon, Wayland, fontconfig, and Vulkan development
packages (see [Install](install.md#cargo)).

## Checks

```
bash scripts/ci.sh
```

runs `cargo fmt --check`, clippy with warnings as errors, the tests, and the line coverage gate
(needs `cargo install cargo-llvm-cov`). CI runs the same script.

## This site

The guide is an [mdBook](https://rust-lang.github.io/mdBook/) in `docs/`, and the landing page is
`docs/landing/`. To preview:

```
cargo build -p sirene --no-default-features
mdbook serve docs
```

The `docs/samples.py` preprocessor (Python 3) draws each `mermaid`, `plantuml`, or `dot` code
block with that build and adds the drawing and `sirene ascii` text below it. A block sirene
reports an error for fails the build; ```` ```mermaid no-render ```` leaves a block as source only.
Untagged blocks are not drawn.

`.github/workflows/pages.yml` builds both on pull requests and publishes them on each push to
`main` that changes `docs/`, `examples/`, or `crates/`.

## Packaging

```
cargo build --release --package sirene
cargo xtask bundle --binary target/release/sirene    # target/bundle/Sirene.app
cargo xtask icon                                     # target/bundle/sirene.{icns,png}
cargo xtask cask --url URL --sha256 HEX --homepage URL
```
