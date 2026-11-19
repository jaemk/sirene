# Linux Packaging

The `.rpm` and the AppImage of each release, both around the `x86_64-unknown-linux-gnu` and
`aarch64-unknown-linux-gnu` binaries of WORKSP-9 (with the GUI). Built by the `xtask` crate
(`cargo xtask linux`), `cargo generate-rpm`, and `bin/appimage.sh`; tests:
`xtask/tests/xtask.rs` and the unit tests in `xtask/src/linux.rs`.

## LINUXP-1 AppDir

`cargo xtask linux --binary PATH [--out DIR]` writes `DIR/AppDir` (default `DIR` is
`target/linux`), replacing any existing tree once `PATH` has been read:

| Path | Contents |
|------|----------|
| `usr/bin/sirene` | a copy of `PATH`, mode 0755 |
| `usr/share/applications/sirene.desktop` | LINUXP-2 |
| `usr/share/mime/packages/sirene.xml` | LINUXP-3 |
| `usr/share/icons/hicolor/scalable/apps/sirene.svg` | `assets/icon.svg` (MACOSP-4) |
| `usr/share/icons/hicolor/NxN/apps/sirene.png` | the icon at N = 32, 48, 128, 256, 512 pixels |
| `usr/share/licenses/sirene/` | `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES` |
| `sirene.desktop`, `sirene.png` | the desktop entry, and the icon at 256 pixels |
| `AppRun`, `.DirIcon` | symlinks to `usr/bin/sirene` and `sirene.png` |

The root entries are what an AppImage needs (LINUXP-5); the `usr/` tree is the RPM's payload
(LINUXP-4).

## LINUXP-2 Desktop entry

`sirene.desktop` is an `Application` named `Sirene` with `Exec=sirene %F` (CLI-1 opens each file
in a window), `Icon=sirene`, category `Development`, and `MimeType` listing the types
of LINUXP-3 and `text/markdown` (WYSIWY-20), so file managers offer the app for those files.

## LINUXP-3 MIME types

`sirene.xml` is a shared-mime-info package declaring, each a subclass of `text/plain`, the file
types of MACOSP-3 with the same descriptions and extensions (`xtask::DOCUMENT_TYPES`):
`text/x-mermaid`, `text/vnd.graphviz`, and `text/x-plantuml` (`xtask::linux::MIME_TYPES`).

## LINUXP-4 RPM

`[package.metadata.generate-rpm]` in `crates/sirene/Cargo.toml` lists the RPM's files:
`target/release/sirene` as `/usr/bin/sirene`, and each file under `target/linux/AppDir/usr/` at
the same path under `/usr/`, the licenses marked as documentation. `cargo generate-rpm -p
crates/sirene --arch ARCH --auto-req find-requires` writes `sirene-X.Y.Z-1.ARCH.rpm`. Its
requirements come from the binary's linked libraries, through RPM's own `find-requires` (the
`rpm` package; cargo-generate-rpm's builtin scan writes glibc `[WEAK]` symbol versions no
package provides), which also pins the glibc version the
binary was built against (Ubuntu 24.04's), so `dnf` refuses a system that cannot run it, plus
`libvulkan.so.1` and `libwayland-client.so.0`, which gpui loads at run time. RPM versions
cannot contain `-`, so only a stable version packages as is; a `-dev` build needs
`--set-metadata 'version = "X.Y.Z~dev"'`. Install
it with `sudo dnf install ./sirene-X.Y.Z-1.ARCH.rpm`, or the release asset's URL. There is no
package repository: an update installs the new release's `.rpm` the same way (UPDATE-4).

## LINUXP-5 AppImage

`bin/appimage.sh ARCH APPDIR OUT` writes the AppImage `OUT`: the AppImage type 2 runtime of
release `20251108` of <https://github.com/AppImage/type2-runtime> for ARCH (`x86_64` or
`aarch64`), checked against its pinned SHA-256, followed by a zstd squashfs of APPDIR
(LINUXP-1), mode 0755. It needs `curl`, `sha256sum`, and `mksquashfs`. The image bundles no
libraries: like the `.rpm`, it uses the system's glibc, X11 or Wayland, and Vulkan libraries.
Running it needs FUSE, or `--appimage-extract-and-run`.

## LINUXP-6 Release build

The `linux` job of `.github/workflows/release.yml` (WORKSP-8) runs for `x86_64` and `aarch64` on
Ubuntu 24.04 runners after `build`: it unpacks `sirene` from that build's unsigned gnu tarball,
writes the AppDir (LINUXP-1), the `.rpm` (LINUXP-4), and `Sirene-X.Y.Z-ARCH.AppImage`
(LINUXP-5), and gives both a build provenance attestation. The `release` job attaches them to
the GitHub release and lists them in `SHA256SUMS`. They are not zipsign-signed: nothing updates
from them.

## LINUXP-7 Unverified

Not yet checked on a desktop: the menu entry and icon, and a file manager offering the app for
the declared types, from the `.rpm` and from an AppImage integrated with a tool such as
AppImageLauncher.
