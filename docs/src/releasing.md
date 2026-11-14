# Releasing

## Signing setup

Before releasing, configure the repository's `signing` environment to allow deployments only
from `main`. Its `ZIPSIGN_KEY` secret must contain the base64-encoded 64-byte signing key pair
matching `crates/sirene/release.pub`. The signing task refuses a mismatched key.

`cargo xtask gen-key --out DIR` writes a new `release.key` and `release.pub`. Keep the private
`release.key` outside the repository and never commit it. The checked-in verifying key is
`crates/sirene/release.pub`. For an existing release key, retain that pair; replacing it needs
the [key rotation steps](https://github.com/jaemk/sirene/blob/main/spec/self-update.md#update-5-verification),
so older installations can still update.

## Release steps

`main` carries the next `-dev` version between releases. To release `X.Y.Z`:

1. Set `version` in the root `Cargo.toml`, and the matching `[workspace.dependencies]` pins, to
   `X.Y.Z`.
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md`.
3. Merge to `main`.

`.github/workflows/release.yml` then runs CI and the RustSec audit, builds the binaries and
`Sirene.app`. The `sign` job builds `xtask` before exposing the secret, decodes the key with
`umask 077`, and signs the tarballs before their attestations and `SHA256SUMS` are generated.
The `release` job creates the `vX.Y.Z` release with the signed tarballs, app, and checksums.
After approval in the `release` environment, `publish` publishes `sirene-core`, `sirene-gui`,
and `sirene` to crates.io and commits the Homebrew formula, the cask, and the next `-dev` version
to `main`.

To retry a failed release, dispatch the release workflow from `main`. If the release already
exists, `main` must still be at the commit named by its `vX.Y.Z` tag; the workflow refuses to
rebuild that version from a different commit. Dispatches from other branches skip the release.

See `spec/workspace.md` (WORKSP-8, WORKSP-9) and `spec/macos-packaging.md` for the details.
