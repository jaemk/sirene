# macOS Packaging

The app bundle, its document types and icon, the Homebrew cask, and the release build. Built by
the `xtask` crate (`cargo xtask ...`); tests: `xtask/tests/xtask.rs` and the unit tests in
`xtask/src`.

## MACOSP-1 Bundle

`cargo xtask bundle --binary PATH [--out DIR]` writes `DIR/Sirene.app` (default `DIR` is
`target/bundle`), replacing any existing bundle:

| Path | Contents |
|------|----------|
| `Contents/Info.plist` | MACOSP-2 |
| `Contents/PkgInfo` | `APPL????` |
| `Contents/MacOS/sirene` | a copy of `PATH`, mode 0755 |
| `Contents/Resources/sirene.icns` | MACOSP-4 |

The bundle runs the same `sirene` binary as the CLI; with no arguments it opens a window
(GUISHE-1).

## MACOSP-2 Info.plist

An XML property list with `CFBundleIdentifier` `io.github.jaemk.sirene`, `CFBundleName` and
`CFBundleDisplayName` `Sirene`, `CFBundleExecutable` `sirene`, `CFBundleIconFile` `sirene`,
`CFBundlePackageType` `APPL`, `CFBundleShortVersionString` and `CFBundleVersion` set to the
workspace version, `LSMinimumSystemVersion` `10.15.7`, `NSHighResolutionCapable` true,
`LSApplicationCategoryType` `public.app-category.developer-tools`, and the document types of
MACOSP-3.

## MACOSP-3 Document types

`UTImportedTypeDeclarations` declares `io.github.jaemk.sirene.mermaid` ("Mermaid diagram"), conforming to
`public.plain-text`, for the extensions `mmd` and `mermaid`. `CFBundleDocumentTypes` lists that
type with role `Editor` and handler rank `Default`, so Finder offers the app for those files and
opens them through GUISHE-1.

## MACOSP-4 Icon

`assets/icon.svg` is the app icon: a mermaid tail fluke on a rounded square. `cargo xtask icon
[--out DIR]` renders it to `DIR/sirene.icns` (default `target/bundle`), with PNG images in the
ten iconset slots (16, 32, 128, 256 and 512 points at 1x and 2x: `icp4`, `ic11`, `icp5`, `ic12`,
`ic07`, `ic13`, `ic08`, `ic14`, `ic09`, `ic10`), and to a 1024 pixel `DIR/sirene.png`. The bundle
(MACOSP-1) renders the `.icns` the same way.

## MACOSP-5 Homebrew cask

`cargo xtask cask --url URL --sha256 HEX --homepage URL` prints a Homebrew cask `sirene` for a
zip of the bundle at `URL`: a header saying it is generated, the workspace version, the
checksum, `app "Sirene.app"`, and `binary` linking `sirene` on the PATH from inside the bundle.
A checksum that is not 64 hex digits, or a value containing `"`, `\`, `#`, or a control
character, is an error.

The repository is its own Homebrew tap: `Casks/sirene.rb` is the cask for the latest release,
byte-for-byte what `cargo xtask cask` prints for that release's zip URL, checksum, and version.
After each release the `publish` job (MACOSP-6) commits the new cask to `main`:

```
brew tap jaemk/sirene https://github.com/jaemk/sirene
brew trust jaemk/sirene
brew install --cask sirene
```

## MACOSP-6 Release build

The `macos` job of `.github/workflows/release.yml` runs on a macOS runner before the crates.io
publish (WORKSP-8), which waits for it: it builds `sirene` for `aarch64-apple-darwin` and
`x86_64-apple-darwin`, joins them with `lipo`, bundles them (MACOSP-1), zips the bundle with
`ditto`, and prints the cask (MACOSP-5) for the zip's `vX.Y.Z` release URL. The zip and cask are
kept as a workflow artifact, and the `publish` job attaches them to the `vX.Y.Z` GitHub release
after creating it, then commits the cask to `Casks/sirene.rb` on `main` (MACOSP-5) unless it
is already current. The bundle is not signed or notarized, so Gatekeeper asks before the first
launch.

## MACOSP-7 Unverified on macOS

Not yet checked on a Mac: Finder "Open With" and drops on the Dock icon reaching the app's open
handler, the icon in Finder and the Dock, and `brew install --cask sirene` from the tap.
