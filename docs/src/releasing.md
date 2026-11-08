# Releasing

`main` carries the next `-dev` version between releases. To release `X.Y.Z`:

1. Set `version` in the root `Cargo.toml`, and the matching `[workspace.dependencies]` pins, to
   `X.Y.Z`.
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md`.
3. Merge to `main`.

`.github/workflows/release.yml` then runs CI and the RustSec audit, builds the binaries and
`Sirene.app`, and creates the `vX.Y.Z` release with them and `SHA256SUMS`. After approval in the
`release` environment it publishes `sirene-core`, `sirene-gui`, and `sirene` to crates.io and
commits the Homebrew formula, the cask, and the next `-dev` version to `main`.

See `spec/workspace.md` (WORKSP-8, WORKSP-9) and `spec/macos-packaging.md` for the details.
