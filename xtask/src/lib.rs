//! Packaging tasks: the macOS app bundle, its icon, and the Homebrew cask (`spec/macos-packaging.md`).

use std::ffi::OsString;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use icns::{IconFamily, IconType, Image};
use plist::{Dictionary, Value};
use resvg::{tiny_skia, usvg};

/// The app's display name and bundle directory stem.
pub const APP: &str = "Sirene";
/// The bundle identifier.
pub const BUNDLE_ID: &str = "dev.sirene.Sirene";
/// The type identifier declared for Mermaid files.
pub const MERMAID_UTI: &str = "dev.sirene.mermaid";
/// The workspace version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The app icon's source.
pub const ICON_SVG: &str = include_str!("../../assets/icon.svg");

/// Icon slots written to the `.icns`, with their pixel sizes.
pub const ICON_SLOTS: [(IconType, u32); 10] = [
    (IconType::RGBA32_16x16, 16),
    (IconType::RGBA32_16x16_2x, 32),
    (IconType::RGBA32_32x32, 32),
    (IconType::RGBA32_32x32_2x, 64),
    (IconType::RGBA32_128x128, 128),
    (IconType::RGBA32_128x128_2x, 256),
    (IconType::RGBA32_256x256, 256),
    (IconType::RGBA32_256x256_2x, 512),
    (IconType::RGBA32_512x512, 512),
    (IconType::RGBA32_512x512_2x, 1024),
];

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Packaging tasks for sirene")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Write Sirene.app around a built `sirene` binary.
    Bundle {
        /// The `sirene` binary to bundle.
        #[arg(long)]
        binary: PathBuf,
        /// Directory to write Sirene.app into.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Write sirene.icns and sirene.png from assets/icon.svg.
    Icon {
        /// Directory to write the icon files into.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print a Homebrew cask for a zip of Sirene.app.
    Cask {
        /// Where the zip is downloaded from.
        #[arg(long)]
        url: String,
        /// SHA-256 of the zip, in hex.
        #[arg(long)]
        sha256: String,
        /// The project's home page.
        #[arg(long)]
        homepage: String,
    },
}

/// Runs the command line `args` (including the program name), printing to `out`.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut impl Write) -> Result<(), String> {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) if !e.use_stderr() => {
            return write!(out, "{}", e.render()).map_err(|e| e.to_string());
        }
        Err(e) => return Err(e.render().to_string()),
    };
    let default_out = || Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/bundle");
    match cli.command {
        Command::Bundle { binary, out: dir } => {
            let app = bundle(&binary, &dir.unwrap_or_else(default_out))?;
            writeln!(out, "{}", app.display()).map_err(|e| e.to_string())
        }
        Command::Icon { out: dir } => {
            let dir = dir.unwrap_or_else(default_out);
            write_icon(&dir)?;
            writeln!(out, "{}", dir.join("sirene.icns").display()).map_err(|e| e.to_string())
        }
        Command::Cask {
            url,
            sha256,
            homepage,
        } => write!(out, "{}", cask(&url, &sha256, &homepage)?).map_err(|e| e.to_string()),
    }
}

/// The bundle's `Info.plist` (MACOSP-2, MACOSP-3).
pub fn info_plist() -> Value {
    let mut uti = Dictionary::new();
    uti.insert("UTTypeIdentifier".into(), MERMAID_UTI.into());
    uti.insert("UTTypeDescription".into(), "Mermaid diagram".into());
    uti.insert(
        "UTTypeConformsTo".into(),
        Value::Array(vec!["public.plain-text".into()]),
    );
    let mut tags = Dictionary::new();
    tags.insert(
        "public.filename-extension".into(),
        Value::Array(vec!["mmd".into(), "mermaid".into()]),
    );
    uti.insert("UTTypeTagSpecification".into(), tags.into());

    let mut doc = Dictionary::new();
    doc.insert("CFBundleTypeName".into(), "Mermaid diagram".into());
    doc.insert("CFBundleTypeRole".into(), "Editor".into());
    doc.insert("LSHandlerRank".into(), "Default".into());
    doc.insert(
        "LSItemContentTypes".into(),
        Value::Array(vec![MERMAID_UTI.into()]),
    );

    let mut d = Dictionary::new();
    for (key, value) in [
        ("CFBundleIdentifier", BUNDLE_ID),
        ("CFBundleName", APP),
        ("CFBundleDisplayName", APP),
        ("CFBundleExecutable", "sirene"),
        ("CFBundleIconFile", "sirene"),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleInfoDictionaryVersion", "6.0"),
        ("CFBundleShortVersionString", VERSION),
        ("CFBundleVersion", VERSION),
        ("LSMinimumSystemVersion", "10.15.7"),
        (
            "LSApplicationCategoryType",
            "public.app-category.developer-tools",
        ),
    ] {
        d.insert(key.into(), value.into());
    }
    d.insert("NSHighResolutionCapable".into(), true.into());
    d.insert(
        "CFBundleDocumentTypes".into(),
        Value::Array(vec![doc.into()]),
    );
    d.insert(
        "UTImportedTypeDeclarations".into(),
        Value::Array(vec![uti.into()]),
    );
    d.into()
}

/// Renders the icon (MACOSP-4) as a square PNG of `size` pixels.
pub fn icon_png(size: u32) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(ICON_SVG, &usvg::Options::default())
        .map_err(|e| format!("cannot parse the icon: {e}"))?;
    let mut pixmap =
        tiny_skia::Pixmap::new(size, size).ok_or_else(|| format!("bad icon size {size}"))?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

/// The icon (MACOSP-4) as an `.icns` file.
pub fn icns() -> Result<Vec<u8>, String> {
    let mut family = IconFamily::new();
    for (slot, size) in ICON_SLOTS {
        let image = Image::read_png(Cursor::new(icon_png(size)?)).map_err(|e| e.to_string())?;
        family
            .add_icon_with_type(&image, slot)
            .map_err(|e| e.to_string())?;
    }
    let mut bytes = Vec::new();
    family.write(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Writes `sirene.icns` and a 1024 pixel `sirene.png` into `dir`.
pub fn write_icon(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| context(dir, e))?;
    write(&dir.join("sirene.icns"), &icns()?)?;
    write(&dir.join("sirene.png"), &icon_png(1024)?)
}

/// Writes `dir/Sirene.app` around `binary` (MACOSP-1) and returns its path.
pub fn bundle(binary: &Path, dir: &Path) -> Result<PathBuf, String> {
    let app = dir.join(format!("{APP}.app"));
    if app.exists() {
        fs::remove_dir_all(&app).map_err(|e| context(&app, e))?;
    }
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    for d in [&macos, &resources] {
        fs::create_dir_all(d).map_err(|e| context(d, e))?;
    }
    let exe = macos.join("sirene");
    fs::copy(binary, &exe).map_err(|e| context(binary, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))
            .map_err(|e| context(&exe, e))?;
    }
    let mut plist = Vec::new();
    info_plist()
        .to_writer_xml(&mut plist)
        .map_err(|e| e.to_string())?;
    write(&contents.join("Info.plist"), &plist)?;
    write(&contents.join("PkgInfo"), b"APPL????")?;
    write(&resources.join("sirene.icns"), &icns()?)?;
    Ok(app)
}

/// A Homebrew cask for a zip of the bundle (MACOSP-5).
pub fn cask(url: &str, sha256: &str, homepage: &str) -> Result<String, String> {
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not a SHA-256 checksum: {sha256:?}"));
    }
    for (name, s) in [("url", url), ("homepage", homepage)] {
        if s.contains(['"', '\\', '#']) || s.chars().any(char::is_control) {
            return Err(format!("{name} cannot be quoted in a cask: {s:?}"));
        }
    }
    let sha256 = sha256.to_ascii_lowercase();
    Ok(format!(
        r##"cask "sirene" do
  version "{VERSION}"
  sha256 "{sha256}"

  url "{url}"
  name "{APP}"
  desc "Mermaid diagram editor and viewer"
  homepage "{homepage}"

  depends_on macos: ">= :catalina"

  app "{APP}.app"
  binary "#{{appdir}}/{APP}.app/Contents/MacOS/sirene"
end
"##
    ))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| context(path, e))
}

fn context(path: &Path, e: io::Error) -> String {
    format!("{}: {e}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_are_validated() {
        let ok = "AB".repeat(32);
        let c = cask("https://x/y.zip", &ok, "https://x").unwrap();
        assert!(
            c.contains(&format!("sha256 \"{}\"", "ab".repeat(32))),
            "{c}"
        );
        assert!(cask("u", "ab", "h").is_err());
        assert!(cask("u", &"g".repeat(64), "h").is_err());
    }

    #[test]
    fn urls_must_quote() {
        let ok = "0".repeat(64);
        for bad in ["a\"b", "a\\b", "a#{b}", "a\nb"] {
            let e = cask(bad, &ok, "h").unwrap_err();
            assert!(e.starts_with("url "), "{e}");
            let e = cask("u", &ok, bad).unwrap_err();
            assert!(e.starts_with("homepage "), "{e}");
        }
    }

    #[test]
    fn bad_icon_size() {
        assert_eq!(icon_png(0).unwrap_err(), "bad icon size 0");
    }
}
