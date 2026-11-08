# Changelog

All notable changes to `sirene` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-11-07

### Added

- Release binaries for macOS (aarch64, x86_64) and Linux (aarch64, x86_64; gnu
  with the GUI, musl without), with `SHA256SUMS` and build provenance
  attestations.
- `bin/install.sh`, a one-line installer for the release binaries.
- Homebrew: this repository is a tap, with a `sirene` formula for the binaries
  and a `sirene` cask for `Sirene.app`.

### Changed

- The macOS bundle identifier is `io.github.jaemk.sirene`, and the Mermaid
  document type is `io.github.jaemk.sirene.mermaid`.

## [0.1.0] - 2026-11-07

### Added

- Flowchart parsing, layout, and rendering to ASCII, SVG, and PNG
  (`sirene ascii`, `sirene svg`, `sirene png`).
- A WYSIWYG editor: the source beside a live drawing, with edits on the drawing
  written back to the source.
- `Sirene.app` for macOS and a Homebrew cask.
