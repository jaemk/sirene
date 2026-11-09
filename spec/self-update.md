# Self Update

`sirene update` replaces the running binary with a release from
<https://github.com/jaemk/sirene/releases>, the same artifact `bin/install.sh` installs.
Implemented in `crates/sirene/src/update.rs`. Tests: its unit tests, and `update` in
`crates/sirene/tests/cli.rs`.

## UPDATE-1 Command

`sirene update [--check] [--yes] [--to VERSION]`. With no `--to`, the target is the latest
release (GitHub's `releases/latest`, which is never a pre-release). `--check` prints the
decision and changes nothing. Otherwise sirene asks `[y/N]` before replacing itself, unless
`--yes`; with no terminal on stdin and no `--yes` it refuses. `--to` with `--check` makes no
network request.

## UPDATE-2 Target

The release asset is `sirene-VERSION-TARGET.tar.gz` for the running build's target:
`{x86_64,aarch64}-apple-darwin` on macOS, and `{x86_64,aarch64}-unknown-linux-{gnu,musl}` on
Linux, keeping the build's C library, so a GUI (gnu) binary stays a GUI binary and a static
(musl) one stays static. Other platforms are refused with a pointer to building from source.
Every report names the target.

## UPDATE-3 Decision

Versions compare by semver precedence, so a `-dev` build of the next version is newer than the
release before it.

- The target equals the running version: `sirene X is up to date`, exit 0.
- The target is newer: the update, exit 0 (`--check` says `update available: X -> Y`).
- The target is older: from `releases/latest`, the running build is newer than the latest
  release, exit 0; from `--to`, a refusal to downgrade, exit 1.

## UPDATE-4 Package managers

A binary another tool installed is that tool's to replace. `sirene update` refuses, exit 1, and
names the command, when the running executable (symlinks resolved) is:

- in a Homebrew Cellar (a `Cellar` path component): `brew upgrade sirene`;
- inside an app bundle (`*.app/Contents/MacOS/`): `brew upgrade --cask sirene`;
- in Cargo's bin directory (`$CARGO_HOME/bin`, default `~/.cargo/bin`): `cargo install sirene`.

`--check` still reports, and adds that note.

## UPDATE-5 Verification

Before anything is extracted, the archive's SHA-256 must match its entry in the release's
`SHA256SUMS`; a missing entry or a mismatch aborts with the binary unchanged. When `gh` is on the
PATH, `gh attestation verify ARCHIVE --repo jaemk/sirene` then checks its build provenance
(WORKSP-9): success prints `build provenance verified`; a `gh` that cannot run the check (an old
`gh`, not logged in, no network) prints a note and continues; any other failure, including no
attestation found, aborts. Without `gh` the check is skipped silently, as `bin/install.sh` does.

## UPDATE-6 Inputs

`--to` takes `X.Y.Z` with an optional leading `v` and pre-release or build suffix of ASCII
letters, digits, `.`, and `-`; anything else is refused before any request. `GH_TOKEN` or
`GITHUB_TOKEN`, when set, authenticates the requests. The release lookup times out after 15
seconds and the download after 10 minutes. The install directory must be writable, checked
before the download.
