# Development

The workspace has four crates:

| Crate | What |
|---|---|
| `sirene-core` | the parser, model, edits, layout, and SVG, PNG, and ASCII renderers; no GUI |
| `sirene-gui` | the editor, on gpui and GPUI Kit |
| `sirene` | the binary; the `gui` feature (on by default) links `sirene-gui` |
| `xtask` | packaging and release-signing tasks; not published |

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
block with that build and adds the drawing and `sirene text` output below it. A block sirene
reports an error for fails the build; ```` ```mermaid no-render ```` leaves a block as source only.
Untagged blocks are not drawn.

The pages redraw a sample as it is edited, and the [playground](playground.md) draws as you type,
with `sirene-wasm` (`wasm/`), `sirene-core` built for WebAssembly. Without the module they show
that it is not built. To build it into the preview:

```
cargo build --profile wasm -p sirene-wasm --target wasm32-unknown-unknown
mkdir -p docs/src/wasm
cp target/wasm32-unknown-unknown/wasm/sirene_wasm.wasm docs/src/wasm/sirene.wasm
```

`.github/workflows/pages.yml` builds both on pull requests and publishes them on each push to
`main` that changes `docs/`, `examples/`, `crates/`, or `wasm/`.

## Packaging

```
cargo build --release --package sirene
cargo xtask bundle --binary target/release/sirene    # target/bundle/Sirene.app
cargo xtask icon                                     # target/bundle/sirene.{icns,png}
cargo xtask cask --url URL --sha256 HEX --homepage URL
```

On Linux, build the editor on the architecture you are packaging. Install `rpm` for
`find-requires`, `squashfs-tools`, `curl`, and `desktop-file-utils`. The runtime build and the
Fedora smoke test use Docker. Run these commands from the workspace root, with the version
from the root `Cargo.toml` and `aarch64` in place of `x86_64` on ARM.

```bash
cargo install cargo-generate-rpm --version 0.21.0 --locked
cargo build --locked --release --package sirene
bash bin/linux-package.sh x86_64 X.Y.Z-dev target/release/sirene target/linux-dist
bash bin/linux-package-smoke.sh x86_64 X.Y.Z-dev target/release/sirene target/linux-dist
```

The packaging command stages `target/linux/AppDir` and writes the RPM, AppImage, and AppImage
runtime source archive to the output directory. A development version such as `X.Y.Z-dev` uses
`X.Y.Z~dev` in the RPM name and metadata.
