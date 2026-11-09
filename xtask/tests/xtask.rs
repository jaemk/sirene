use std::ffi::OsString;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use assert_cmd::Command;
use icns::{IconFamily, Image};
use plist::Value;
use predicates::prelude::*;

fn run(args: &[&str]) -> Result<String, String> {
    let mut out = Vec::new();
    let args = std::iter::once("xtask")
        .chain(args.iter().copied())
        .map(OsString::from);
    xtask::run(args, &mut out).map(|()| String::from_utf8(out).unwrap())
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn check_icns(bytes: &[u8]) {
    let family = IconFamily::read(Cursor::new(bytes)).unwrap();
    assert_eq!(family.available_icons().len(), xtask::ICON_SLOTS.len());
    for (slot, size) in xtask::ICON_SLOTS {
        let image = family.get_icon_with_type(slot).unwrap();
        assert_eq!((image.width(), image.height()), (size, size), "{slot:?}");
    }
}

#[test]
fn bundle_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let binary = tmp.path().join("sirene-built");
    fs::write(&binary, b"\x7fELF fake").unwrap();
    let out = tmp.path().join("out");

    let printed = run(&["bundle", "--binary", path(&binary), "--out", path(&out)]).unwrap();
    let app = out.join("Sirene.app");
    assert_eq!(printed, format!("{}\n", app.display()));

    let contents = app.join("Contents");
    let exe = contents.join("MacOS/sirene");
    assert_eq!(fs::read(&exe).unwrap(), b"\x7fELF fake");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&exe).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    assert_eq!(fs::read(contents.join("PkgInfo")).unwrap(), b"APPL????");
    check_icns(&fs::read(contents.join("Resources/sirene.icns")).unwrap());

    let plist = Value::from_file(contents.join("Info.plist")).unwrap();
    assert_eq!(plist, xtask::info_plist());
    let text = fs::read_to_string(contents.join("Info.plist")).unwrap();
    assert!(text.starts_with("<?xml"), "{text}");

    // A second run replaces the bundle.
    fs::write(contents.join("stale"), b"").unwrap();
    fs::write(&binary, b"new").unwrap();
    run(&["bundle", "--binary", path(&binary), "--out", path(&out)]).unwrap();
    assert!(!contents.join("stale").exists());
    assert_eq!(fs::read(&exe).unwrap(), b"new");

    // The binary may come from the bundle being replaced.
    run(&["bundle", "--binary", path(&exe), "--out", path(&out)]).unwrap();
    assert_eq!(fs::read(&exe).unwrap(), b"new");

    // A missing binary leaves the existing bundle alone.
    let missing = tmp.path().join("missing");
    run(&["bundle", "--binary", path(&missing), "--out", path(&out)]).unwrap_err();
    assert_eq!(fs::read(&exe).unwrap(), b"new");
}

#[test]
fn bundle_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");
    let e = run(&[
        "bundle",
        "--binary",
        path(&missing),
        "--out",
        path(tmp.path()),
    ])
    .unwrap_err();
    assert!(e.starts_with(&format!("{}: ", missing.display())), "{e}");

    // The output directory is a file.
    let file = tmp.path().join("file");
    fs::write(&file, b"").unwrap();
    let e = run(&["bundle", "--binary", path(&file), "--out", path(&file)]).unwrap_err();
    assert!(e.contains("file/Sirene.app/Contents/MacOS: "), "{e}");
    let e = run(&["icon", "--out", path(&file)]).unwrap_err();
    assert!(e.starts_with(&format!("{}: ", file.display())), "{e}");
}

#[test]
fn info_plist() {
    let plist = xtask::info_plist();
    let d = plist.as_dictionary().unwrap();
    let s = |k: &str| d[k].as_string().unwrap().to_owned();
    assert_eq!(s("CFBundleIdentifier"), "io.github.jaemk.sirene");
    assert_eq!(s("CFBundleName"), "Sirene");
    assert_eq!(s("CFBundleDisplayName"), "Sirene");
    assert_eq!(s("CFBundleExecutable"), "sirene");
    assert_eq!(s("CFBundleIconFile"), "sirene");
    assert_eq!(s("CFBundlePackageType"), "APPL");
    let version = env!("CARGO_PKG_VERSION").split('-').next().unwrap();
    assert_eq!(s("CFBundleShortVersionString"), version);
    assert_eq!(s("CFBundleVersion"), version);
    assert_eq!(s("LSMinimumSystemVersion"), "10.15.7");
    assert_eq!(
        s("LSApplicationCategoryType"),
        "public.app-category.developer-tools"
    );
    assert_eq!(d["NSHighResolutionCapable"].as_boolean(), Some(true));

    let docs = d["CFBundleDocumentTypes"].as_array().unwrap();
    assert_eq!(docs.len(), 1);
    let doc = docs[0].as_dictionary().unwrap();
    assert_eq!(doc["CFBundleTypeRole"].as_string(), Some("Editor"));
    assert_eq!(doc["LSHandlerRank"].as_string(), Some("Default"));
    assert_eq!(
        doc["LSItemContentTypes"],
        Value::Array(vec!["io.github.jaemk.sirene.mermaid".into()])
    );

    let utis = d["UTImportedTypeDeclarations"].as_array().unwrap();
    assert_eq!(utis.len(), 1);
    let uti = utis[0].as_dictionary().unwrap();
    assert_eq!(
        uti["UTTypeIdentifier"].as_string(),
        Some("io.github.jaemk.sirene.mermaid")
    );
    assert_eq!(
        uti["UTTypeDescription"].as_string(),
        Some("Mermaid diagram")
    );
    assert_eq!(
        uti["UTTypeConformsTo"],
        Value::Array(vec!["public.plain-text".into()])
    );
    let tags = uti["UTTypeTagSpecification"].as_dictionary().unwrap();
    assert_eq!(
        tags["public.filename-extension"],
        Value::Array(vec!["mmd".into(), "mermaid".into()])
    );
}

#[test]
fn icon_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("icons");
    let printed = run(&["icon", "--out", path(&dir)]).unwrap();
    assert_eq!(printed, format!("{}\n", dir.join("sirene.icns").display()));
    check_icns(&fs::read(dir.join("sirene.icns")).unwrap());
    let png = Image::read_png(Cursor::new(fs::read(dir.join("sirene.png")).unwrap())).unwrap();
    assert_eq!((png.width(), png.height()), (1024, 1024));
    // Transparent outside the rounded square, opaque at its center.
    let px = |x: u32, y: u32| png.data()[((y * 1024 + x) * 4 + 3) as usize];
    assert_eq!(px(10, 10), 0);
    assert_eq!(px(512, 200), 255);
}

#[test]
fn icon_default_dir() {
    let printed = run(&["icon"]).unwrap();
    assert!(
        printed.ends_with("target/bundle/sirene.icns\n"),
        "{printed}"
    );
}

#[test]
fn cask() {
    let sha = "0123456789abcdef".repeat(4);
    let printed = run(&[
        "cask",
        "--url",
        "https://example.com/Sirene.zip",
        "--sha256",
        &sha,
        "--homepage",
        "https://example.com",
    ])
    .unwrap();
    assert_eq!(
        printed,
        format!(
            r##"# Generated by `cargo xtask cask` and committed by .github/workflows/release.yml
# on each release; do not edit by hand.
cask "sirene" do
  version "{}"
  sha256 "{sha}"

  url "https://example.com/Sirene.zip"
  name "Sirene"
  desc "Mermaid diagram editor and viewer"
  homepage "https://example.com"

  app "Sirene.app"
  binary "#{{appdir}}/Sirene.app/Contents/MacOS/sirene"
end
"##,
            env!("CARGO_PKG_VERSION")
        )
    );
    let e = run(&["cask", "--url", "u", "--sha256", "nope", "--homepage", "h"]).unwrap_err();
    assert_eq!(e, "not a SHA-256 checksum: \"nope\"");
}

/// The value of `key "..."` on its own line in a cask.
fn cask_field<'a>(cask: &'a str, key: &str) -> &'a str {
    let prefix = format!("  {key} \"");
    cask.lines()
        .find_map(|l| l.strip_prefix(&prefix)?.strip_suffix('"'))
        .unwrap_or_else(|| panic!("no {key} in the cask"))
}

/// The committed tap cask is exactly what `cargo xtask cask` prints for its
/// release, so the workflow's update of it is a version and checksum diff.
#[test]
fn committed_cask() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Casks/sirene.rb");
    let committed = fs::read_to_string(path).unwrap();
    let version = cask_field(&committed, "version");
    let url = format!(
        "https://github.com/jaemk/sirene/releases/download/v{version}/Sirene-v{version}.zip"
    );
    let regenerated = xtask::cask(
        version,
        &url,
        cask_field(&committed, "sha256"),
        "https://github.com/jaemk/sirene",
    )
    .unwrap();
    assert_eq!(committed, regenerated);
}

#[test]
fn usage() {
    assert!(
        run(&["--help"])
            .unwrap()
            .contains("Packaging tasks for sirene")
    );
    let e = run(&["frobnicate"]).unwrap_err();
    assert!(e.contains("frobnicate"), "{e}");
}

#[test]
fn binary_exit_codes() {
    Command::cargo_bin("xtask")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("bundle"));
    Command::cargo_bin("xtask")
        .unwrap()
        .args(["cask", "--url", "u", "--sha256", "x", "--homepage", "h"])
        .assert()
        .failure()
        .stderr("not a SHA-256 checksum: \"x\"\n");
}
