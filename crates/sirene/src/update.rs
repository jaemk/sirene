//! `sirene update`: replace the running binary with a release (UPDATE-1 to UPDATE-6).
//!
//! The download, checksum, extraction, and swap are the `self_update` crate's, through its
//! GitHub backend. What lives here is everything around it: the target triple, the version
//! decision, package-manager refusals, the prompt, and the provenance check. The decisions are
//! pure functions, tested without the network.

use std::io::{IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use self_update::backends::github;

const OWNER: &str = "jaemk";
const NAME: &str = "sirene";
const REPO: &str = "jaemk/sirene";
/// The release asset holding each asset's SHA-256 (UPDATE-5).
const SUMS: &str = "SHA256SUMS";
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
pub fn decide(current: &str, target: &str) -> Result<Decision, String> {
    let parse =
        |v: &str| semver::Version::parse(v).map_err(|e| format!("'{v}' is not a version: {e}"));
    let (current, target) = (parse(current)?, parse(target)?);
    Ok(match target.cmp(&current) {
        std::cmp::Ordering::Equal => Decision::UpToDate,
        std::cmp::Ordering::Greater => Decision::Update,
        std::cmp::Ordering::Less => Decision::Older,
    })
}

/// A `--to` value without its one leading `v`, if it is a plain version (UPDATE-6). It is
/// interpolated into a request path, so nothing that could leave a path segment gets through.
pub fn version_arg(raw: &str) -> Result<String, String> {
    let v = raw.strip_prefix('v').unwrap_or(raw);
    let plain = !v.is_empty()
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    if plain && semver::Version::parse(v).is_ok() {
        Ok(v.to_owned())
    } else {
        Err(format!("--to '{raw}' is not a version like 0.2.0"))
    }
}

/// A package manager that installed the running binary (UPDATE-4).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Owner {
    Homebrew,
    Cask,
    Cargo,
}

impl Owner {
    /// The command that updates a binary this owner installed.
    pub fn command(self) -> &'static str {
        match self {
            Owner::Homebrew => "brew upgrade sirene",
            Owner::Cask => "brew upgrade --cask sirene",
            Owner::Cargo => "cargo install sirene",
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

/// What `gh attestation verify` said about an archive (UPDATE-5).
#[derive(Debug, PartialEq, Eq)]
pub enum Provenance {
    Verified,
    /// `gh` could not run the check; says nothing about the archive.
    Unchecked(String),
    /// `gh` ran the check and the archive did not verify.
    Rejected(String),
}

/// Whether `gh`'s error means it could not run the check, rather than that the archive failed
/// it. Narrow on purpose: "no attestations found" is also what a substituted archive gets, so it
/// is a rejection.
pub fn gh_could_not_check(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "unknown command",
        "unknown flag",
        "dial tcp",
        "no such host",
        "connection refused",
        "i/o timeout",
        "context deadline exceeded",
        "tls handshake",
        "certificate signed by unknown authority",
        "gh auth login",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

/// Runs `gh attestation verify` on `archive`; `None` when `gh` is not installed.
pub fn provenance(gh: &str, archive: &Path) -> Option<Provenance> {
    let out = match Command::new(gh)
        .arg("attestation")
        .arg("verify")
        .arg(archive)
        .args(["--repo", REPO])
        .output()
    {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(Provenance::Unchecked(e.to_string())),
    };
    if out.status.success() {
        return Some(Provenance::Verified);
    }
    // gh's output reaches the terminal; keep it to printable text.
    let stderr: String = String::from_utf8_lossy(&out.stderr)
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    Some(if gh_could_not_check(&stderr) {
        Provenance::Unchecked(stderr)
    } else {
        Provenance::Rejected(stderr)
    })
}

/// The `self_update` hook run on the downloaded archive, before it is extracted.
fn verify(archive: &Path) -> Result<(), self_update::Error> {
    match provenance("gh", archive) {
        None => Ok(()),
        Some(Provenance::Verified) => {
            println!("build provenance verified");
            Ok(())
        }
        Some(Provenance::Unchecked(why)) => {
            eprintln!("note: build provenance could not be checked ({why}); continuing");
            Ok(())
        }
        Some(Provenance::Rejected(why)) => Err(self_update::Error::archive_verification_rejected(
            format!("build provenance did not verify: {why}"),
        )),
    }
}

/// The GitHub updater for `target`, at release `version` or the latest.
fn updater(
    target: &str,
    version: Option<&str>,
    timeout: Duration,
) -> Result<github::Update, self_update::Error> {
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
        .verify_archive(verify);
    if let Some(v) = version {
        b.release_tag(format!("v{v}"));
    }
    b.build()
}

/// The latest release's version.
fn latest(target: &str) -> Result<String, String> {
    let up = updater(target, None, LOOKUP).map_err(|e| e.to_string())?;
    let releases = up.get_latest_release().map_err(|e| e.to_string())?;
    let release = releases.latest().ok_or("the release listing is empty")?;
    Ok(release.version().trim_start_matches('v').to_owned())
}

/// Asks `question [y/N]` on the terminal.
fn confirm(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}

pub fn run(args: Args) -> ExitCode {
    let current = env!("CARGO_PKG_VERSION");
    let Some(triple) = this_target() else {
        return fail(format!(
            "no release is built for {} {}; build from source with `cargo install sirene`",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    };
    let pinned = match args.to.as_deref().map(version_arg).transpose() {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let exe = std::env::current_exe().and_then(|e| e.canonicalize());
    let owner = exe
        .as_ref()
        .ok()
        .and_then(|e| owner(e, cargo_home().as_deref()));
    if let (Some(owner), false) = (owner, args.check) {
        return fail(format!(
            "this sirene was installed by a package manager; update it with `{}`",
            owner.command()
        ));
    }
    let explicit = pinned.is_some();
    let version = match pinned {
        Some(v) => v,
        None => {
            log::debug!("looking up the latest release");
            match latest(&triple) {
                Ok(v) => v,
                Err(e) => return fail(format!("could not look up the latest release: {e}")),
            }
        }
    };
    let decision = match decide(current, &version) {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    let note = || {
        if let Some(owner) = owner {
            println!("note: update this sirene with `{}`", owner.command());
        }
    };
    match decision {
        Decision::UpToDate => {
            println!("sirene {current} is up to date (target {triple})");
            note();
            return ExitCode::SUCCESS;
        }
        Decision::Older if explicit => {
            return fail(format!(
                "sirene {current} is newer than {version}; not downgrading (target {triple})"
            ));
        }
        Decision::Older => {
            println!(
                "sirene {current} is newer than the latest release, {version} (target {triple})"
            );
            return ExitCode::SUCCESS;
        }
        Decision::Update if args.check => {
            println!("update available: {current} -> {version} (target {triple})");
            note();
            return ExitCode::SUCCESS;
        }
        Decision::Update => {}
    }
    if !args.yes {
        if !std::io::stdin().is_terminal() {
            return fail("not a terminal; pass --yes to update without asking");
        }
        if !confirm(&format!("update sirene {current} -> {version}?")) {
            println!("not updated");
            return ExitCode::SUCCESS;
        }
    }
    println!("downloading sirene {version} ({triple})");
    let swapped = updater(&triple, Some(&version), DOWNLOAD).and_then(|u| u.update());
    match swapped {
        Ok(_) => {
            println!("updated sirene {current} -> {version}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(format!("update failed; sirene is unchanged: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(this_target().is_some());
    }

    #[test]
    fn decisions() {
        assert_eq!(decide("0.2.0", "0.2.0"), Ok(Decision::UpToDate));
        assert_eq!(decide("0.2.0", "0.3.0"), Ok(Decision::Update));
        assert_eq!(decide("0.2.0", "0.1.1"), Ok(Decision::Older));
        // A dev build of the next version is newer than the release before it, older than its
        // own release.
        assert_eq!(decide("0.2.1-dev", "0.2.0"), Ok(Decision::Older));
        assert_eq!(decide("0.2.1-dev", "0.2.1"), Ok(Decision::Update));
        assert!(decide("0.2.0", "latest").is_err());
    }

    #[test]
    fn versions() {
        assert_eq!(version_arg("0.2.0").as_deref(), Ok("0.2.0"));
        assert_eq!(version_arg("v0.2.0").as_deref(), Ok("0.2.0"));
        assert_eq!(version_arg("1.0.0-rc.1").as_deref(), Ok("1.0.0-rc.1"));
        for bad in [
            "", "v", "vv0.2.0", "0.2", "../0.2.0", "0.2.0/x", "0.2.0 x", "latest",
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
        assert_eq!(Owner::Cask.command(), "brew upgrade --cask sirene");
    }

    #[test]
    fn gh_errors() {
        assert!(gh_could_not_check(
            "unknown command \"attestation\" for \"gh\""
        ));
        assert!(gh_could_not_check(
            "Post https://api.github.com: dial tcp: lookup api.github.com: no such host"
        ));
        assert!(gh_could_not_check(
            "To get started with GitHub CLI, please run:  gh auth login"
        ));
        assert!(!gh_could_not_check("Error: no attestations found"));
        assert!(!gh_could_not_check("HTTP 404: Not Found"));
    }

    #[test]
    fn missing_gh_is_skipped() {
        assert_eq!(provenance("sirene-no-such-gh", Path::new("a.tar.gz")), None);
    }

    #[cfg(unix)]
    #[test]
    fn gh_outcomes() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fake = |name: &str, body: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.to_str().unwrap().to_owned()
        };
        let archive = Path::new("sirene.tar.gz");
        // The archive and repo are passed through.
        let args = dir.path().join("args");
        let ok = fake("ok", &format!("echo \"$@\" > {}", args.display()));
        assert_eq!(provenance(&ok, archive), Some(Provenance::Verified));
        assert_eq!(
            std::fs::read_to_string(&args).unwrap(),
            "attestation verify sirene.tar.gz --repo jaemk/sirene\n"
        );
        let offline = fake("offline", "echo 'dial tcp: i/o timeout' >&2; exit 1");
        assert_eq!(
            provenance(&offline, archive),
            Some(Provenance::Unchecked("dial tcp: i/o timeout".into()))
        );
        let none = fake(
            "none",
            "printf 'Error: no attestations found\\033[0m' >&2; exit 1",
        );
        assert_eq!(
            provenance(&none, archive),
            Some(Provenance::Rejected(
                "Error: no attestations found [0m".into()
            ))
        );
    }
}
