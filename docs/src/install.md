# Install

## Install script (Linux and macOS)

```
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/jaemk/sirene/main/bin/install.sh | sh
```

It downloads the release binary for this platform, checks it against the release's
`SHA256SUMS`, and installs it to `~/.local/bin`. It reads these environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `SIRENE_VERSION` | the latest release | the version to install, e.g. `0.2.0` |
| `SIRENE_INSTALL_DIR` | `~/.local/bin` | where to put `sirene` |
| `SIRENE_TARGET` | this platform's, musl on Linux | the release target triple |

On Linux the default is the static musl build, which runs on any distribution but has no GUI.
For the editor, install the glibc build, which needs the system's X11 or Wayland and Vulkan
libraries:

```
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/jaemk/sirene/main/bin/install.sh | SIRENE_TARGET=x86_64-unknown-linux-gnu sh
```

(`aarch64-unknown-linux-gnu` on ARM.)

## Homebrew (macOS and Linux)

```
brew tap jaemk/sirene https://github.com/jaemk/sirene
brew trust jaemk/sirene
brew install sirene          # the binary
brew install --cask sirene   # macOS: Sirene.app, with `sirene` linked onto the PATH
```

The repo is not named `homebrew-sirene`, so the tap needs its clone URL. Install one of the
formula or the cask, not both: each puts `sirene` on the PATH. The app is not signed, so macOS
asks before the first launch.

## Cargo

```
cargo install sirene                          # with the editor
cargo install sirene --no-default-features    # the command line only
```

Building the editor on Linux needs the XCB, xkbcommon, Wayland, fontconfig, and Vulkan
development packages, e.g. on Debian or Ubuntu:

```
sudo apt-get install libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libfontconfig1-dev libfreetype-dev libvulkan-dev
```

A build without the default `gui` feature does not link any of these; `sirene` with no command
then prints `error: built without the GUI; see sirene --help`.

## Release binaries

Each [release](https://github.com/jaemk/sirene/releases) has binaries for Linux (musl and glibc,
x86_64 and aarch64), macOS (x86_64 and aarch64), a universal `Sirene.app` zip, and
`SHA256SUMS`.
