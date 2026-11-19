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
    xtask::run(args, &mut out)
        .map(|()| String::from_utf8(out).unwrap())
        .map_err(|e| e.report())
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
    let notices = fs::read_to_string(contents.join("Resources/THIRD-PARTY-NOTICES")).unwrap();
    assert!(notices.contains("venn.js"), "{notices}");
    for name in ["LICENSE-MIT", "LICENSE-APACHE"] {
        assert!(contents.join("Resources").join(name).is_file(), "{name}");
    }

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

    // Each language's files open by default, and Markdown as an alternate (MACOSP-3).
    let docs = d["CFBundleDocumentTypes"].as_array().unwrap();
    let docs: Vec<(String, String)> = docs
        .iter()
        .map(|doc| {
            let doc = doc.as_dictionary().unwrap();
            assert_eq!(doc["CFBundleTypeRole"].as_string(), Some("Editor"));
            let types = doc["LSItemContentTypes"].as_array().unwrap();
            assert_eq!(types.len(), 1);
            (
                types[0].as_string().unwrap().to_owned(),
                doc["LSHandlerRank"].as_string().unwrap().to_owned(),
            )
        })
        .collect();
    let owned = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    };
    assert_eq!(
        docs,
        owned(&[
            ("io.github.jaemk.sirene.mermaid", "Default"),
            ("io.github.jaemk.sirene.dot", "Default"),
            ("io.github.jaemk.sirene.plantuml", "Default"),
            ("net.daringfireball.markdown", "Alternate"),
        ])
    );

    let utis = d["UTImportedTypeDeclarations"].as_array().unwrap();
    let utis: Vec<(String, String, Value)> = utis
        .iter()
        .map(|uti| {
            let uti = uti.as_dictionary().unwrap();
            assert_eq!(
                uti["UTTypeConformsTo"],
                Value::Array(vec!["public.plain-text".into()])
            );
            let tags = uti["UTTypeTagSpecification"].as_dictionary().unwrap();
            (
                uti["UTTypeIdentifier"].as_string().unwrap().to_owned(),
                uti["UTTypeDescription"].as_string().unwrap().to_owned(),
                tags["public.filename-extension"].clone(),
            )
        })
        .collect();
    let exts = |e: &[&str]| Value::Array(e.iter().map(|&e| e.into()).collect());
    assert_eq!(
        utis,
        [
            (
                "io.github.jaemk.sirene.mermaid".to_owned(),
                "Mermaid diagram".to_owned(),
                exts(&["mmd", "mermaid"]),
            ),
            (
                "io.github.jaemk.sirene.dot".to_owned(),
                "Graphviz DOT graph".to_owned(),
                exts(&["dot", "gv"]),
            ),
            (
                "io.github.jaemk.sirene.plantuml".to_owned(),
                "PlantUML diagram".to_owned(),
                exts(&["puml", "plantuml", "pu", "iuml"]),
            ),
        ]
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

/// ERRORH-2: release tooling retains both dependency and underlying OS errors.
#[test]
fn signing_key_errors_retain_io_causes() {
    use std::error::Error as _;
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.key");
    let public = dir.path().join("release.pub");
    let error = xtask::signing_key(&missing, &public).err().unwrap();
    let mut cause = error.source();
    let mut found = false;
    while let Some(error) = cause {
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
            found = true;
        }
        cause = error.source();
    }
    assert!(found, "{}", error.report());
    assert!(error.report().contains("missing.key"));
}

/// ERRORH-4: failed output writes return inspectable errors without panicking.
#[test]
fn command_output_errors_retain_io_causes() {
    use std::error::Error as _;
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed reader",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let args = ["xtask", "--help"].map(OsString::from);
    let error = xtask::run(args, &mut Broken).unwrap_err();
    let io = error
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(io.kind(), std::io::ErrorKind::BrokenPipe);
}

/// ERRORH-4: buffered output can fail only when it is flushed.
#[test]
fn command_output_flush_errors_retain_io_causes() {
    use std::error::Error as _;
    struct FailsOnFlush(Vec<u8>);
    impl std::io::Write for FailsOnFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    for args in [
        vec!["xtask", "--help"],
        vec![
            "xtask",
            "cask",
            "--sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--url",
            "https://example.com",
            "--homepage",
            "https://example.com",
        ],
    ] {
        let mut out = FailsOnFlush(Vec::new());
        let error = xtask::run(args.into_iter().map(OsString::from), &mut out).unwrap_err();
        assert!(!out.0.is_empty());
        let io = error
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap();
        assert_eq!(io.kind(), std::io::ErrorKind::BrokenPipe);
    }
}

#[test]
fn linux_appdir_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let binary = tmp.path().join("sirene-built");
    fs::write(&binary, b"\x7fELF fake").unwrap();
    let out = tmp.path().join("out");
    // A stale tree is replaced.
    fs::create_dir_all(out.join("AppDir/stale")).unwrap();

    let printed = run(&["linux", "--binary", path(&binary), "--out", path(&out)]).unwrap();
    let root = out.join("AppDir");
    assert_eq!(printed, format!("{}\n", root.display()));
    assert!(!root.join("stale").exists());

    let exe = root.join("usr/bin/sirene");
    assert_eq!(fs::read(&exe).unwrap(), b"\x7fELF fake");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&exe).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            fs::read_link(root.join("AppRun")).unwrap(),
            Path::new("usr/bin/sirene")
        );
        assert_eq!(
            fs::read_link(root.join(".DirIcon")).unwrap(),
            Path::new("sirene.png")
        );
        assert_eq!(fs::read(root.join("AppRun")).unwrap(), b"\x7fELF fake");
    }
    let share = root.join("usr/share");
    let desktop = fs::read_to_string(share.join("applications/sirene.desktop")).unwrap();
    assert_eq!(desktop, xtask::linux::desktop_entry());
    assert_eq!(
        fs::read_to_string(root.join("sirene.desktop")).unwrap(),
        desktop
    );
    assert_eq!(
        fs::read_to_string(share.join("mime/packages/sirene.xml")).unwrap(),
        xtask::linux::mime_package()
    );
    assert_eq!(
        fs::read_to_string(share.join("icons/hicolor/scalable/apps/sirene.svg")).unwrap(),
        xtask::ICON_SVG
    );
    assert_eq!(
        fs::read(root.join("sirene.png")).unwrap(),
        xtask::icon_png(256).unwrap()
    );
    for size in xtask::linux::ICON_SIZES {
        let png = share.join(format!("icons/hicolor/{size}x{size}/apps/sirene.png"));
        assert_eq!(
            fs::read(&png).unwrap(),
            xtask::icon_png(size).unwrap(),
            "{size}"
        );
    }
    for (name, text) in xtask::LICENSES {
        let p = share.join("licenses/sirene").join(name);
        assert_eq!(fs::read_to_string(p).unwrap(), text, "{name}");
    }
}

#[test]
fn linux_needs_the_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    let missing = tmp.path().join("missing");
    let e = run(&["linux", "--binary", path(&missing), "--out", path(&out)]).unwrap_err();
    assert!(e.starts_with(path(&missing)), "{e}");
    assert!(!out.exists());
}

/// The RPM metadata ships every file of the AppDir's `usr/` tree (LINUXP-4).
#[test]
fn rpm_assets_cover_the_appdir() {
    let tmp = tempfile::tempdir().unwrap();
    let binary = tmp.path().join("sirene");
    fs::write(&binary, b"bin").unwrap();
    let root = xtask::linux::appdir(&binary, tmp.path()).unwrap();
    let manifest = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/sirene/Cargo.toml"),
    )
    .unwrap();
    let mut files = Vec::new();
    let mut dirs = vec![root.join("usr")];
    while let Some(d) = dirs.pop() {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                files.push(p.strip_prefix(&root).unwrap().to_str().unwrap().to_owned());
            }
        }
    }
    assert_eq!(files.len(), 12);
    for f in files {
        assert!(manifest.contains(&format!("dest = \"/{f}\"")), "{f}");
    }
}
