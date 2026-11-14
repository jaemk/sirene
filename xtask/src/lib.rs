//! Packaging tasks: the macOS app bundle, its icon, and the Homebrew cask (`spec/macos-packaging.md`),
//! and release archive signing (`spec/self-update.md`).

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
pub const BUNDLE_ID: &str = "io.github.jaemk.sirene";
/// The type identifier declared for Mermaid files.
pub const MERMAID_UTI: &str = "io.github.jaemk.sirene.mermaid";
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
    /// Write a new release signing key pair (release.key) and its verifying key (release.pub).
    GenKey {
        /// Directory to write the keys into.
        #[arg(long)]
        out: PathBuf,
    },
    /// Sign release archives in place for `sirene update` (UPDATE-5).
    Sign {
        /// The 64-byte signing key pair `gen-key` wrote.
        #[arg(long)]
        key: PathBuf,
        /// The verifying key `sirene update` embeds; `key` must be its pair.
        #[arg(long, default_value = "crates/sirene/release.pub")]
        public: PathBuf,
        /// The `.tar.gz` archives to sign.
        #[arg(required = true)]
        archives: Vec<PathBuf>,
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
        } => write!(out, "{}", cask(VERSION, &url, &sha256, &homepage)?).map_err(|e| e.to_string()),
        Command::GenKey { out: dir } => {
            let (private, public) = gen_key(&dir)?;
            writeln!(out, "{}\n{}", private.display(), public.display()).map_err(|e| e.to_string())
        }
        Command::Sign {
            key,
            public,
            archives,
        } => {
            let key = signing_key(&key, &public)?;
            for a in &archives {
                sign(&key, a)?;
                writeln!(out, "signed {}", a.display()).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
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
        ("CFBundleShortVersionString", bundle_version()),
        ("CFBundleVersion", bundle_version()),
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

/// The workspace version without its pre-release suffix (`0.1.2-dev` ->
/// `0.1.2`): bundle versions are period-separated integers.
pub fn bundle_version() -> &'static str {
    version_core(VERSION)
}

fn version_core(version: &str) -> &str {
    version.split_once('-').map_or(version, |(core, _)| core)
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

/// sirene's licenses and the third-party notices, copied into the bundle's `Resources`.
pub const LICENSES: [(&str, &str); 3] = [
    ("LICENSE-MIT", include_str!("../../LICENSE-MIT")),
    ("LICENSE-APACHE", include_str!("../../LICENSE-APACHE")),
    (
        "THIRD-PARTY-NOTICES",
        include_str!("../../THIRD-PARTY-NOTICES"),
    ),
];

/// Writes `dir/Sirene.app` around `binary` (MACOSP-1) and returns its path.
pub fn bundle(binary: &Path, dir: &Path) -> Result<PathBuf, String> {
    // Read the binary before removing the old bundle, which may hold it.
    let exe_bytes = fs::read(binary).map_err(|e| context(binary, e))?;
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
    write(&exe, &exe_bytes)?;
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
    for (name, text) in LICENSES {
        write(&resources.join(name), text.as_bytes())?;
    }
    Ok(app)
}

/// A Homebrew cask for version `version` of a zip of the bundle (MACOSP-5).
pub fn cask(version: &str, url: &str, sha256: &str, homepage: &str) -> Result<String, String> {
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not a SHA-256 checksum: {sha256:?}"));
    }
    for (name, s) in [("version", version), ("url", url), ("homepage", homepage)] {
        if s.contains(['"', '\\', '#']) || s.chars().any(char::is_control) {
            return Err(format!("{name} cannot be quoted in a cask: {s:?}"));
        }
    }
    let sha256 = sha256.to_ascii_lowercase();
    Ok(format!(
        r##"# Generated by `cargo xtask cask` and committed by .github/workflows/release.yml
# on each release; do not edit by hand.
cask "sirene" do
  version "{version}"
  sha256 "{sha256}"

  url "{url}"
  name "{APP}"
  desc "Mermaid diagram editor and viewer"
  homepage "{homepage}"

  app "{APP}.app"
  binary "#{{appdir}}/{APP}.app/Contents/MacOS/sirene"
end
"##
    ))
}

/// Writes a new zipsign key pair: `release.key`, the 64-byte signing key pair for the release
/// workflow's `ZIPSIGN_KEY` secret, and `release.pub`, the 32-byte verifying key `sirene update`
/// embeds. Refuses to overwrite either. Returns the two paths.
pub fn gen_key(dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let (private, public) = (dir.join("release.key"), dir.join("release.pub"));
    for p in [&private, &public] {
        if p.exists() {
            return Err(format!("{} exists; not overwriting it", p.display()));
        }
    }
    // An ed25519 secret key is 32 random bytes.
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("could not read random bytes: {e}"))?;
    let key = zipsign_api::SigningKey::from_bytes(&seed);
    fs::create_dir_all(dir).map_err(|e| context(dir, e))?;
    write_private_new(&private, &key.to_keypair_bytes())?;
    write_new(&public, key.verifying_key().as_bytes())?;
    Ok((private, public))
}

/// The signing key pair in `key`, checked against the verifying key in `public`, so a key that
/// `sirene update` would not accept signs nothing.
pub fn signing_key(key: &Path, public: &Path) -> Result<zipsign_api::SigningKey, String> {
    let mut keys = zipsign_api::sign::read_signing_keys([fs::File::open(key)])
        .map_err(|e| format!("{}: {e}", key.display()))?;
    let expected = fs::read(public).map_err(|e| context(public, e))?;
    let key = keys.remove(0);
    if key.verifying_key().as_bytes()[..] != expected[..] {
        return Err(format!(
            "{}: not the key pair of the verifying key in {}",
            key_name(key.verifying_key().as_bytes()),
            public.display()
        ));
    }
    Ok(key)
}

/// A short, non-secret name for a verifying key.
fn key_name(public: &[u8]) -> String {
    let hex: String = public.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("signing key {hex}...")
}

/// Signs the `.tar.gz` `archive` in place with `key`, as `self_update` verifies it: over the
/// archive's bytes, with its file name as the context.
pub fn sign(key: &zipsign_api::SigningKey, archive: &Path) -> Result<(), String> {
    let keys = std::slice::from_ref(key);
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| n.ends_with(".tar.gz"))
        .ok_or_else(|| format!("{}: not a .tar.gz file name", archive.display()))?;
    let bytes = fs::read(archive).map_err(|e| context(archive, e))?;
    let mut signed = Cursor::new(Vec::with_capacity(bytes.len() + 1024));
    zipsign_api::sign::copy_and_sign_tar(
        &mut Cursor::new(bytes),
        &mut signed,
        keys,
        Some(name.as_bytes()),
    )
    .map_err(|e| format!("{}: {e}", archive.display()))?;
    let tmp = archive.with_file_name(format!(".{name}.signing"));
    write(&tmp, signed.get_ref())?;
    fs::rename(&tmp, archive).map_err(|e| context(archive, e))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| context(path, e))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| context(path, e))?;
    f.write_all(bytes).map_err(|e| context(path, e))
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path).map_err(|e| context(path, e))?;
    f.write_all(bytes).map_err(|e| context(path, e))
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
        let c = cask("1.0.0", "https://x/y.zip", &ok, "https://x").unwrap();
        assert!(
            c.contains(&format!("sha256 \"{}\"", "ab".repeat(32))),
            "{c}"
        );
        assert!(cask("1", "u", "ab", "h").is_err());
        assert!(cask("1", "u", &"g".repeat(64), "h").is_err());
    }

    #[test]
    fn values_must_quote() {
        let ok = "0".repeat(64);
        for bad in ["a\"b", "a\\b", "a#{b}", "a\nb"] {
            let e = cask(bad, "u", &ok, "h").unwrap_err();
            assert!(e.starts_with("version "), "{e}");
            let e = cask("1", bad, &ok, "h").unwrap_err();
            assert!(e.starts_with("url "), "{e}");
            let e = cask("1", "u", &ok, bad).unwrap_err();
            assert!(e.starts_with("homepage "), "{e}");
        }
    }

    #[test]
    fn bundle_versions_drop_the_prerelease() {
        assert_eq!(version_core("0.1.2-dev"), "0.1.2");
        assert_eq!(version_core("1.0.0-rc.1"), "1.0.0");
        assert_eq!(version_core("0.1.1"), "0.1.1");
    }

    /// What `sign` writes passes the check `sirene update` runs, under the archive's own name
    /// and only with the key pair's verifying key.
    #[test]
    fn signed_archives_verify() {
        let dir = tempfile::tempdir().unwrap();
        let (private, public_path) = gen_key(dir.path()).unwrap();
        assert!(
            gen_key(dir.path())
                .unwrap_err()
                .ends_with("exists; not overwriting it")
        );
        let public: self_update::VerifyingKey = fs::read(&public_path).unwrap().try_into().unwrap();
        let other_dir = tempfile::tempdir().unwrap();
        let (other_private, other_path) = gen_key(other_dir.path()).unwrap();
        let other: self_update::VerifyingKey = fs::read(&other_path).unwrap().try_into().unwrap();
        // A key pair is checked against the verifying key it must match.
        let e = signing_key(&other_private, &public_path).err().unwrap();
        assert!(e.contains("not the key pair of the verifying key"), "{e}");
        let key = signing_key(&private, &public_path).unwrap();

        let archive = dir
            .path()
            .join("sirene-1.0.0-x86_64-unknown-linux-gnu.tar.gz");
        let mut gz = Vec::new();
        {
            let mut e = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::default());
            e.write_all(b"not really a tar").unwrap();
            e.finish().unwrap();
        }
        write(&archive, &gz).unwrap();
        // Unsigned, it fails.
        assert!(self_update::verify_signature(&archive, &[public]).is_err());
        sign(&key, &archive).unwrap();
        self_update::verify_signature(&archive, &[public]).unwrap();
        assert!(self_update::verify_signature(&archive, &[other]).is_err());
        // The signature covers the name: the same bytes under another name fail.
        let renamed = dir
            .path()
            .join("sirene-1.0.0-aarch64-unknown-linux-gnu.tar.gz");
        fs::copy(&archive, &renamed).unwrap();
        assert!(self_update::verify_signature(&renamed, &[public]).is_err());
        // And the bytes.
        let mut bytes = fs::read(&archive).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        write(&archive, &bytes).unwrap();
        assert!(self_update::verify_signature(&archive, &[public]).is_err());

        let e = sign(&key, &dir.path().join("a.zip")).unwrap_err();
        assert!(e.ends_with("not a .tar.gz file name"), "{e}");
    }

    #[test]
    fn bad_icon_size() {
        assert_eq!(icon_png(0).unwrap_err(), "bad icon size 0");
    }
}
