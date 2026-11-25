# Linux Packaging

The `.rpm` and the AppImage of each release, both around the `x86_64-unknown-linux-gnu` and
`aarch64-unknown-linux-gnu` binaries of WORKSP-9 (with the GUI).
`bin/linux-package.sh` runs the packaging commands used by CI and the release workflow.
AppDir tests live in `xtask/tests/xtask.rs` and `xtask/src/linux.rs`.

## LINUXP-1 AppDir

`cargo xtask linux --binary PATH [--out DIR]` writes `DIR/AppDir` (default `DIR` is
`target/linux`), replacing any existing tree once `PATH` has been read.

| Path | Contents |
|------|----------|
| `usr/bin/sirene` | a copy of `PATH`, mode 0755 |
| `usr/share/applications/sirene.desktop` | LINUXP-2 |
| `usr/share/mime/packages/sirene.xml` | LINUXP-3 |
| `usr/share/icons/hicolor/scalable/apps/sirene.svg` | `assets/icon.svg` (MACOSP-4) |
| `usr/share/icons/hicolor/NxN/apps/sirene.png` | the icon at N = 32, 48, 128, 256, 512 pixels |
| `usr/share/licenses/sirene/` | `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES` |
| `sirene.desktop`, `sirene.png` | the AppImage desktop entry and the icon at 256 pixels |
| `AppRun`, `.DirIcon` | symlinks to `usr/bin/sirene` and `sirene.png` |
| `.sirene-appimage` | marker used by the updater (UPDATE-4) |

The root entries are what an AppImage needs (LINUXP-5); the `usr/` tree is the RPM's payload
(LINUXP-4).

## LINUXP-2 Desktop entry

Both desktop entries describe an `Application` named `Sirene` with `Icon=sirene`, category
`Development`, and `MimeType` listing the types of LINUXP-3 and `text/markdown` (WYSIWY-20).
The installed RPM entry uses `Exec=/usr/bin/sirene %F` so it launches the packaged editor even
when another `sirene` is first on `PATH`. The root AppImage entry uses `Exec=sirene %F` for
AppImage integration tools. Each file opens in a window (CLI-1).

## LINUXP-3 MIME types

`sirene.xml` is a shared-mime-info package declaring, each a subclass of `text/plain`, the file
types of MACOSP-3 with the same descriptions and extensions (`xtask::DOCUMENT_TYPES`):
`text/x-mermaid`, `text/vnd.graphviz`, and `text/x-plantuml` (`xtask::linux::MIME_TYPES`).
The Visio type, `application/vnd.ms-visio.drawing` with glob `*.vsdx`, is a subclass of
`application/zip` instead (VISIO-12).

## LINUXP-4 RPM

`[package.metadata.generate-rpm]` in `crates/sirene/Cargo.toml` maps the files staged under
`target/linux/AppDir/usr/`, including the selected executable, to the same paths under
`/usr/`. The licenses and notices are regular package files, so `dnf` retains them with
`tsflags=nodocs`. `cargo generate-rpm -p crates/sirene --arch ARCH --auto-req find-requires`
writes `sirene-X.Y.Z-1.ARCH.rpm`.

RPM's `find-requires` derives dependencies from the executable's linked libraries, including
its glibc 2.39 floor. The metadata also declares `libvulkan.so.1` and
`libwayland-client.so.0`, which gpui loads at run time. Use `find-requires` from the `rpm`
package because cargo-generate-rpm's built-in scanner can emit glibc `[WEAK]` symbol
requirements that packages do not provide.

A stable version packages as is. For a `-dev` build, use
`--set-metadata 'version = "X.Y.Z~dev"'` to produce an RPM-compatible version. Install with
`sudo dnf install ./sirene-X.Y.Z-1.ARCH.rpm` or the release asset URL, and pass the next
release's RPM to `sudo dnf upgrade` to update (UPDATE-4).

## LINUXP-5 AppImage

`bin/appimage.sh ARCH APPDIR OUT` builds a type 2 AppImage for `x86_64` or `aarch64`.
It checks the SHA-256 of each source archive and builds the upstream 20251108 runtime source
with patched libfuse 3.15.0, squashfuse, and zstd in a pinned Alpine Docker image. The runtime
is followed by a zstd SquashFS of APPDIR (LINUXP-1), with mode 0755. The build needs Docker,
`curl`, and `mksquashfs`.

The image carries the runtime license texts under `usr/share/licenses/appimage-runtime/`. The
adjacent `*.AppImage-runtime-source.tar.gz` contains the corresponding source archives, runtime
object and static application libraries, licenses, and instructions to rebuild the runtime
or relink it with a modified libfuse. The Sirene executable uses the host's glibc 2.39 or later,
X11 or Wayland, and Vulkan libraries. Use FUSE to mount the image or `--appimage-extract-and-run`
to launch from a temporary extraction.

## LINUXP-6 Release build

The `linux` job of `.github/workflows/release.yml` (WORKSP-8) runs on native Ubuntu 24.04
runners for `x86_64` and `aarch64` after `build`. It extracts `sirene` from the GNU tarball and
calls `bin/linux-package.sh ARCH VERSION BINARY OUT`. This writes the AppDir (LINUXP-1) and
produces a versioned RPM (LINUXP-4), AppImage (LINUXP-5), and AppImage runtime source archive.
The `release` job attaches the attested artifacts and lists their hashes in `SHA256SUMS`.
The Linux packages use attestations for provenance and their own update process (UPDATE-4).

## LINUXP-7 Package verification

`.github/workflows/linux-packaging.yml` runs on both native architectures on pull requests
and pushes to branches. It builds a stable-version GNU binary, packs and extracts it with the
same archive script as the release, and calls the release packaging script, then runs `bin/linux-package-smoke.sh ARCH VERSION BINARY DIST`. The checks compare each packaged
executable to the build input, render a diagram from each package, exercise AppImage
extraction and the updater guard, relink the AppImage runtime from the shipped materials, and
install, upgrade, and remove the RPM on Fedora 43. A final job downloads both architectures’
artifacts and uses the release checksum script to validate the six package names and checksums,
with fixture assets representing the other release jobs. This does not publish a release or test
signing and attestations.
The RPM check also verifies that licenses survive `nodocs` and validates the launcher, icon
file, and Mermaid MIME registration.

The menu icon, window startup, and opening files from a live desktop still need verification
for the RPM and for an AppImage registered with an integration tool such as AppImageLauncher.
