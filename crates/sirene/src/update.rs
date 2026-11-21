//! `sirene update`: replace the running binary with a release (UPDATE-1 to UPDATE-6).
//!
//! The download, checksum, signature check, extraction, and swap are the `self_update` crate's,
//! through its GitHub backend. What lives here is everything around it: the target triple, the
//! version decision, package-manager refusals, the prompt, and the signing key. The decisions
//! are pure functions, tested without the network.

use sirene_core::{Error, Result};
use std::fmt;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use self_update::backends::github;

const OWNER: &str = "jaemk";
const NAME: &str = "sirene";
/// The release asset holding each asset's SHA-256 (UPDATE-5).
const SUMS: &str = "SHA256SUMS";
/// The zipsign keys a release archive must be signed with (UPDATE-5). The release workflow signs
/// with the matching key pair; `cargo xtask gen-key` writes a new one. During a key rotation this
/// lists the old key and the new one.
const KEYS: [self_update::VerifyingKey; 1] = [*include_bytes!("../release.pub")];
/// The release lookup's budget (UPDATE-6).
const LOOKUP: Duration = Duration::from_secs(15);
/// The download's budget, as `bin/install.sh` gives curl (UPDATE-6).
const DOWNLOAD: Duration = Duration::from_secs(600);

/// `sirene update`'s options.
pub struct Args {
    pub check: bool,
    pub yes: bool,
    pub to: Option<String>,
}

/// The release target for an OS, architecture, and C library (UPDATE-2), or `None` for a
/// platform with no release.
pub fn target(os: &str, arch: &str, musl: bool) -> Option<String> {
    let arch = match arch {
        "x86_64" | "aarch64" => arch,
        _ => return None,
    };
    match os {
        "macos" => Some(format!("{arch}-apple-darwin")),
        "linux" => Some(format!(
            "{arch}-unknown-linux-{}",
            if musl { "musl" } else { "gnu" }
        )),
        _ => None,
    }
}

/// The running build's release target.
fn this_target() -> Option<String> {
    target(
        std::env::consts::OS,
        std::env::consts::ARCH,
        cfg!(target_env = "musl"),
    )
}

/// How a target version relates to the running one (UPDATE-3).
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    UpToDate,
    Update,
    /// The target is older than the running version.
    Older,
}

/// Compares `current` and `target` by semver precedence.
pub fn decide(current: &str, target: &str) -> Result<Decision> {
    let parse = |v: &str| {
        semver::Version::parse(v)
            .map_err(|e| Error::with_source(format!("'{v}' is not a version"), e))
    };
    let (current, target) = (parse(current)?, parse(target)?);
    Ok(match target.cmp_precedence(&current) {
        std::cmp::Ordering::Equal => Decision::UpToDate,
        std::cmp::Ordering::Greater => Decision::Update,
        std::cmp::Ordering::Less => Decision::Older,
    })
}

/// A `--to` value without its one leading `v`, if it is a plain version (UPDATE-6). It is
/// interpolated into a request path, so nothing that could leave a path segment gets through.
pub fn version_arg(raw: &str) -> Result<String> {
    let v = raw.strip_prefix('v').unwrap_or(raw);
    let plain = !v.is_empty()
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    if !plain {
        return Err(Error::new(format!(
            "--to '{raw}' is not a version like 0.2.0"
        )));
    }
    semver::Version::parse(v)
        .map_err(|e| Error::with_source(format!("--to '{raw}' is not a version like 0.2.0"), e))?;
    Ok(v.to_owned())
}

/// A package manager that installed the running binary (UPDATE-4).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Owner {
    Homebrew,
    Cask,
    Cargo,
    /// The system's package manager: the release `.rpm` installs into `/usr/bin`.
    System,
    /// An AppImage's mounted or extracted root.
    AppImage,
}

impl Owner {
    /// Who installed the binary and how to update it, after "this sirene ".
    pub fn hint(self) -> &'static str {
        match self {
            Owner::Homebrew => "was installed by Homebrew; update it with `brew upgrade sirene`",
            Owner::Cask => {
                "was installed by a Homebrew cask; update it with `brew upgrade --cask sirene`"
            }
            Owner::Cargo => "was installed by Cargo; update it with `cargo install sirene`",
            Owner::System => {
                "was installed by the system's package manager; update it with that, e.g. \
                 `sudo dnf upgrade ./sirene-VERSION-1.ARCH.rpm` with the new release's .rpm"
            }
            Owner::AppImage => {
                "runs from an AppImage; replace it with the new release's AppImage from \
                 https://github.com/jaemk/sirene/releases"
            }
        }
    }
}

/// Who installed the executable at `exe` (symlinks already resolved), given Cargo's home.
pub fn owner(exe: &Path, cargo_home: Option<&Path>) -> Option<Owner> {
    let parts: Vec<&std::ffi::OsStr> = exe
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();
    if parts.iter().any(|p| *p == "Cellar") {
        return Some(Owner::Homebrew);
    }
    // The AppImage runtime mounts the image at `.mount_*`, or extracts it to
    // `appimage_extracted_*` without FUSE.
    let appimage = parts.iter().any(|p| {
        p.to_str()
            .is_some_and(|s| s.starts_with(".mount_") || s.starts_with("appimage_extracted_"))
    });
    if appimage {
        return Some(Owner::AppImage);
    }
    if exe.parent() == Some(Path::new("/usr/bin")) {
        return Some(Owner::System);
    }
    let bundled = parts.windows(3).any(|w| {
        w[0].to_str().is_some_and(|s| s.ends_with(".app")) && w[1] == "Contents" && w[2] == "MacOS"
    });
    if bundled {
        return Some(Owner::Cask);
    }
    let bin = cargo_home?.join("bin");
    let bin = bin.canonicalize().unwrap_or(bin);
    (exe.parent() == Some(bin.as_path())).then_some(Owner::Cargo)
}

/// Cargo's home: `$CARGO_HOME`, or `~/.cargo`.
fn cargo_home() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|h| h.join(".cargo")))
}

/// The GitHub updater for `target`, at release `version` or the latest.
fn updater(
    target: &str,
    version: Option<&str>,
    timeout: Duration,
) -> std::result::Result<github::Update, self_update::Error> {
    let mut b = github::Update::configure();
    b.repo_owner(OWNER)
        .repo_name(NAME)
        .bin_name(NAME)
        .target(target)
        .current_version(env!("CARGO_PKG_VERSION"))
        .auth_token_from_env()
        .timeout(timeout)
        .no_confirm(true)
        .show_output(false)
        .show_download_progress(std::io::stderr().is_terminal())
        .check_install_path_writable(true)
        .checksum_from_asset(SUMS)
        .verifying_keys(KEYS);
    if let Some(v) = version {
        b.release_tag(format!("v{v}"));
        let expected = format!("sirene-{v}-{target}.tar.gz");
        b.asset_matcher(move |assets| exact_asset(assets, &expected));
    }
    b.build()
}

/// Select only the archive built for this exact release and target.
fn exact_asset(
    assets: &[self_update::ReleaseAsset],
    expected: &str,
) -> Option<self_update::ReleaseAsset> {
    assets
        .iter()
        .find(|asset| asset.name() == expected)
        .cloned()
}

/// The latest release's version.
fn latest(target: &str) -> Result<String> {
    let up = updater(target, None, LOOKUP)
        .map_err(|e| Error::with_source("cannot configure release lookup", e))?;
    let releases = up
        .get_latest_release()
        .map_err(|e| Error::with_source("cannot fetch releases", e))?;
    let release = releases.latest().ok_or("the release listing is empty")?;
    Ok(release.version().trim_start_matches('v').to_owned())
}

/// Asks `question [y/N]`, preserving prompt, flush, read, and EOF failures.
fn confirm(question: &str, input: &mut impl BufRead, out: &mut impl Write) -> Result<bool> {
    write!(out, "{question} [y/N] ")
        .and_then(|()| out.flush())
        .map_err(|e| Error::with_source("cannot write update confirmation", e))?;
    let mut answer = String::new();
    let read = input
        .read_line(&mut answer)
        .map_err(|e| Error::with_source("cannot read update confirmation", e))?;
    if read == 0 {
        return Err(Error::with_source(
            "cannot read update confirmation",
            std::io::Error::from(std::io::ErrorKind::UnexpectedEof),
        ));
    }
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn say(out: &mut impl Write, args: fmt::Arguments<'_>) -> Result<()> {
    crate::output::line(out, args).map_err(|e| Error::with_source("cannot write <stdout>", e))
}

pub fn run(args: Args) -> ExitCode {
    match run_update(args, &mut std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            diagnostic!("error: {}", e.report());
            ExitCode::FAILURE
        }
    }
}

fn run_update(args: Args, out: &mut impl Write) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let triple = this_target().ok_or_else(|| {
        Error::new(format!(
            "no release is built for {} {}; build from source with `cargo install sirene`",
            std::env::consts::OS,
            std::env::consts::ARCH,
        ))
    })?;
    let pinned = args.to.as_deref().map(version_arg).transpose()?;
    let exe = std::env::current_exe()
        .and_then(|e| e.canonicalize())
        .map_err(|e| Error::with_source("cannot locate the running executable", e))?;
    let owner = owner(&exe, cargo_home().as_deref());
    if let (Some(owner), false) = (owner, args.check) {
        return Err(Error::new(format!("this sirene {}", owner.hint())));
    }
    let explicit = pinned.is_some();
    let version = match pinned {
        Some(v) => v,
        None => {
            log::debug!("looking up the latest release");
            latest(&triple).map_err(|e| e.context("could not look up the latest release"))?
        }
    };
    let decision = decide(current, &version)?;
    log::debug!("update decision for {current} -> {version}: {decision:?}");
    let note = |out: &mut _| {
        if let Some(owner) = owner {
            say(out, format_args!("note: this sirene {}", owner.hint()))?;
        }
        Ok(())
    };
    match decision {
        Decision::UpToDate => {
            say(
                out,
                format_args!("sirene {current} is up to date (target {triple})"),
            )?;
            return note(out);
        }
        Decision::Older if explicit => {
            return Err(Error::new(format!(
                "sirene {current} is newer than {version}; not downgrading (target {triple})"
            )));
        }
        Decision::Older => {
            say(
                out,
                format_args!(
                    "sirene {current} is newer than the latest release, {version} (target {triple})"
                ),
            )?;
            return note(out);
        }
        Decision::Update if args.check => {
            if explicit {
                say(
                    out,
                    format_args!(
                        "{version} is newer than sirene {current} (target {triple}); release availability not checked"
                    ),
                )?;
            } else {
                say(
                    out,
                    format_args!("update available: {current} -> {version} (target {triple})"),
                )?;
            }
            return note(out);
        }
        Decision::Update => {}
    }
    if !args.yes {
        if !std::io::stdin().is_terminal() {
            return Err(Error::new(
                "not a terminal; pass --yes to update without asking",
            ));
        }
        if !confirm(
            &format!("update sirene {current} -> {version}?"),
            &mut std::io::stdin().lock(),
            out,
        )? {
            log::debug!("update cancelled");
            say(out, format_args!("not updated"))?;
            return Ok(());
        }
    }
    say(out, format_args!("downloading sirene {version} ({triple})"))?;
    let start = std::time::Instant::now();
    updater(&triple, Some(&version), DOWNLOAD)
        .and_then(|u| u.update())
        .map_err(|e| Error::with_source("update failed; sirene is unchanged", e))?;
    log::debug!("installed sirene {version} in {:.1?}", start.elapsed());
    say(out, format_args!("updated sirene {current} -> {version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ERRORH-5: cancellation, EOF, read errors, and prompt failures are distinct.
    #[test]
    fn confirmation_preserves_io_failures() {
        use std::error::Error as _;
        use std::io::Cursor;

        let mut out = Vec::new();
        assert!(confirm("update?", &mut Cursor::new(b"YES\n"), &mut out).unwrap());
        assert_eq!(out, b"update? [y/N] ");
        assert!(!confirm("update?", &mut Cursor::new(b"no\n"), &mut out).unwrap());
        assert!(!confirm("update?", &mut Cursor::new(b"\n"), &mut out).unwrap());
        let error = confirm("update?", &mut Cursor::new(b""), &mut out).unwrap_err();
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::UnexpectedEof
        );

        struct FailedIo {
            flush: bool,
        }
        impl Write for FailedIo {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.flush {
                    Ok(bytes.len())
                } else {
                    Err(std::io::ErrorKind::BrokenPipe.into())
                }
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
        }
        impl std::io::Read for FailedIo {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::PermissionDenied.into())
            }
        }
        impl BufRead for FailedIo {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                Err(std::io::ErrorKind::PermissionDenied.into())
            }
            fn consume(&mut self, _: usize) {}
        }
        for flush in [false, true] {
            let error = confirm(
                "update?",
                &mut Cursor::new(b"yes\n"),
                &mut FailedIo { flush },
            )
            .unwrap_err();
            assert_eq!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<std::io::Error>()
                    .unwrap()
                    .kind(),
                std::io::ErrorKind::BrokenPipe
            );
            let error = say(&mut FailedIo { flush }, format_args!("status")).unwrap_err();
            assert_eq!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<std::io::Error>()
                    .unwrap()
                    .kind(),
                std::io::ErrorKind::BrokenPipe
            );
        }
        let error = confirm("update?", &mut FailedIo { flush: false }, &mut out).unwrap_err();
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        let error = decide("bad", "1.0.0").unwrap_err();
        assert!(error.source().unwrap().is::<semver::Error>());
        let error = version_arg("0.2").unwrap_err();
        assert!(error.source().unwrap().is::<semver::Error>());
    }

    #[test]
    fn targets() {
        assert_eq!(
            target("macos", "aarch64", false).as_deref(),
            Some("aarch64-apple-darwin")
        );
        assert_eq!(
            target("linux", "x86_64", false).as_deref(),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            target("linux", "aarch64", true).as_deref(),
            Some("aarch64-unknown-linux-musl")
        );
        assert_eq!(target("windows", "x86_64", false), None);
        assert_eq!(target("linux", "riscv64", false), None);
        assert_eq!(
            this_target(),
            target(
                std::env::consts::OS,
                std::env::consts::ARCH,
                cfg!(target_env = "musl")
            )
        );
    }

    #[test]
    fn release_assets_require_exact_version_and_target() {
        let expected = "sirene-1.2.3-x86_64-unknown-linux-musl.tar.gz";
        let asset = |name| self_update::ReleaseAsset::new(name, "https://example.invalid/archive");
        let wrong = [
            asset("sirene-1.2.3-x86_64-unknown-linux-gnu.tar.gz"),
            asset("sirene-1.2.4-x86_64-unknown-linux-musl.tar.gz"),
            asset("other-sirene-1.2.3-x86_64-unknown-linux-musl.tar.gz"),
            asset("sirene-1.2.3-x86_64-unknown-linux-musl.tar.gz.sig"),
        ];
        assert!(exact_asset(&wrong, expected).is_none());
        let mut assets = wrong.to_vec();
        assets.insert(2, asset(expected));
        assert_eq!(exact_asset(&assets, expected).unwrap().name(), expected);
        assets.reverse();
        assert_eq!(exact_asset(&assets, expected).unwrap().name(), expected);
    }

    #[test]
    fn decisions() {
        assert_eq!(decide("0.2.0", "0.2.0").unwrap(), Decision::UpToDate);
        assert_eq!(decide("0.2.0", "0.3.0").unwrap(), Decision::Update);
        assert_eq!(decide("0.2.0", "0.1.1").unwrap(), Decision::Older);
        // A dev build of the next version is newer than the release before it, older than its
        // own release.
        assert_eq!(decide("0.2.1-dev", "0.2.0").unwrap(), Decision::Older);
        assert_eq!(decide("0.2.1-dev", "0.2.1").unwrap(), Decision::Update);
        assert!(decide("0.2.0", "latest").is_err());
        // Build metadata does not affect precedence.
        assert_eq!(decide("0.2.0+a", "0.2.0+b").unwrap(), Decision::UpToDate);
        assert_eq!(
            decide("0.2.0", "0.2.0+build.5").unwrap(),
            Decision::UpToDate
        );
    }

    #[test]
    fn versions() {
        assert_eq!(version_arg("0.2.0").unwrap(), "0.2.0");
        assert_eq!(version_arg("v0.2.0").unwrap(), "0.2.0");
        assert_eq!(version_arg("1.0.0-rc.1").unwrap(), "1.0.0-rc.1");
        for bad in [
            "",
            "v",
            "vv0.2.0",
            "0.2.0+build.1",
            "0.2",
            "../0.2.0",
            "0.2.0/x",
            "0.2.0 x",
            "latest",
        ] {
            assert!(version_arg(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn owners() {
        let cargo = Path::new("/home/u/.cargo");
        let of = |p: &str| owner(Path::new(p), Some(cargo));
        assert_eq!(
            of("/opt/homebrew/Cellar/sirene/0.2.0/bin/sirene"),
            Some(Owner::Homebrew)
        );
        assert_eq!(
            of("/Applications/Sirene.app/Contents/MacOS/sirene"),
            Some(Owner::Cask)
        );
        assert_eq!(of("/home/u/.cargo/bin/sirene"), Some(Owner::Cargo));
        assert_eq!(of("/home/u/.local/bin/sirene"), None);
        assert_eq!(of("/home/u/.cargo/bin/x/sirene"), None);
        assert_eq!(owner(Path::new("/home/u/.cargo/bin/sirene"), None), None);
        assert_eq!(of("/usr/bin/sirene"), Some(Owner::System));
        assert_eq!(of("/usr/local/bin/sirene"), None);
        assert_eq!(
            of("/tmp/.mount_SireneAb12/usr/bin/sirene"),
            Some(Owner::AppImage)
        );
        assert_eq!(
            of("/tmp/appimage_extracted_0123abcd/usr/bin/sirene"),
            Some(Owner::AppImage)
        );
        assert!(Owner::Cask.hint().ends_with("`brew upgrade --cask sirene`"));
        assert!(Owner::System.hint().contains("sudo dnf upgrade"));
        assert!(Owner::AppImage.hint().contains("/jaemk/sirene/releases"));
    }
}
