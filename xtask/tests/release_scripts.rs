/*!
Coverage for the release shell scripts in `bin/` (WORKSP-8, WORKSP-9).

None of them runs in a normal build, and most only run during a release, where
a mistake is expensive and hard to reverse: `check-versions.sh` stops a
half-bumped workspace from publishing, `publish.sh`'s exit status decides
whether a release reports green, and the formula, changelog, and version
scripts write what the release commits and publishes. `install.sh` runs on
users' machines.

The `check-versions.sh` failure cases hand the script a crafted `cargo metadata`
document, which is why it accepts one as an argument. Commands that would reach
the network (`cargo publish`, `curl`, `gh`) are stubbed by fake scripts placed
first on `PATH`.
*/

#![cfg(unix)]

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

use tempfile::TempDir;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is in the workspace root")
        .to_path_buf()
}

/// Scratch files live in the repo's gitignored `local/` directory.
fn scratch_dir() -> TempDir {
    let root = repo_root().join("local");
    std::fs::create_dir_all(&root).expect("create local/ scratch root");
    TempDir::new_in(root).expect("create scratch dir")
}

/// Serializes writing a stub executable against forking a child. A child
/// forked while another test thread holds a stub open for writing inherits that
/// fd until it execs, and exec'ing the stub in that window fails with ETXTBSY,
/// so the shell would skip the stub and run the real command.
static EXEC_LOCK: Mutex<()> = Mutex::new(());

fn exec_lock() -> std::sync::MutexGuard<'static, ()> {
    EXEC_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Command::output` with the fork taken under [`EXEC_LOCK`].
fn output(cmd: &mut Command) -> Output {
    let child = {
        let _guard = exec_lock();
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn")
    };
    child.wait_with_output().expect("wait")
}

fn write_exec(path: &Path, content: &str) {
    let _guard = exec_lock();
    std::fs::write(path, content).expect("write stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make stub executable");
}

/// `PATH` with `bin` in front of the inherited one.
fn path_with(bin: &Path) -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{path}", bin.display())
}

fn run(script: &str, args: &[&str]) -> Output {
    output(
        Command::new("bash")
            .arg(repo_root().join("bin").join(script))
            .args(args)
            .current_dir(repo_root()),
    )
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The SHA-256 of `path` in hex, from `sha256sum` or, on macOS, `shasum`.
fn sha256(path: &Path) -> String {
    let out = output(Command::new("shasum").args(["-a", "256"]).arg(path));
    let out = if out.status.success() {
        out
    } else {
        output(Command::new("sha256sum").arg(path))
    };
    assert!(out.status.success(), "{}", stderr(&out));
    stdout(&out).split_whitespace().next().unwrap().to_owned()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A `cargo metadata --no-deps` document with the fields the script reads:
/// `sirene` depends on `sirene-core` and `sirene-gui`, which depends on
/// `sirene-core`; `xtask` is unpublished.
fn metadata(versions: [&str; 3], sirene_pins: [&str; 2], gui_pin: &str) -> String {
    let [sirene, core, gui] = versions;
    let [core_pin, gui_from_root] = sirene_pins;
    format!(
        r#"{{"packages":[
        {{"name":"sirene","version":"{sirene}","publish":null,"dependencies":[
            {{"name":"clap","req":"^4","kind":null}},
            {{"name":"sirene-core","req":"{core_pin}","kind":null}},
            {{"name":"sirene-gui","req":"{gui_from_root}","kind":null}}]}},
        {{"name":"sirene-core","version":"{core}","publish":null,"dependencies":[]}},
        {{"name":"sirene-gui","version":"{gui}","publish":null,"dependencies":[
            {{"name":"sirene-core","req":"{gui_pin}","kind":null}}]}},
        {{"name":"xtask","version":"9.9.9","publish":[],"dependencies":[
            {{"name":"sirene-core","req":"^0.0.1","kind":null}}]}}
        ]}}"#
    )
}

fn check_versions_with(contents: &str) -> Output {
    let dir = scratch_dir();
    let path = dir.path().join("metadata.json");
    std::fs::write(&path, contents).expect("write metadata fixture");
    run("check-versions.sh", &[path.to_str().expect("utf-8 path")])
}

/// The real workspace must always satisfy its own release guard.
#[test]
fn the_committed_workspace_versions_agree() {
    let out = run("check-versions.sh", &[]);
    assert!(
        out.status.success(),
        "bin/check-versions.sh must pass against the committed workspace:\n{}",
        stderr(&out)
    );
    assert!(stdout(&out).contains("Workspace versions agree (sirene "));
}

/// Consistent versions pass, and an unpublished member's stale pin is ignored.
#[test]
fn consistent_versions_pass() {
    let out = check_versions_with(&metadata(
        ["0.2.0", "0.2.0", "0.2.0"],
        ["^0.2.0", "^0.2.0"],
        "^0.2.0",
    ));
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "Workspace versions agree (sirene 0.2.0).\n");
}

/// A crate bumped but a dependent's pin left behind.
#[test]
fn a_stale_pin_fails_the_check() {
    let out = check_versions_with(&metadata(
        ["0.2.0", "0.2.0", "0.2.0"],
        ["^0.2.0", "^0.2.0"],
        "^0.1.0",
    ));
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains(
            "sirene-gui depends on sirene-core ^0.1.0, but the workspace builds sirene-core 0.2.0"
        ),
        "{err}"
    );
    assert!(err.contains("refusing to publish"), "{err}");
}

/// A pin bumped but the crate's own version left behind.
#[test]
fn an_unbumped_crate_fails_the_check() {
    let out = check_versions_with(&metadata(
        ["0.2.0", "0.2.0", "0.1.0"],
        ["^0.2.0", "^0.2.0"],
        "^0.2.0",
    ));
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains(
            "sirene depends on sirene-gui ^0.2.0, but the workspace builds sirene-gui 0.1.0"
        ),
        "{}",
        stderr(&out)
    );
}

/// A stable release must not depend on a pre-release.
#[test]
fn a_stable_crate_on_a_prerelease_fails_the_check() {
    let out = check_versions_with(&metadata(
        ["0.2.0", "0.2.0-rc.1", "0.2.0"],
        ["^0.2.0-rc.1", "^0.2.0"],
        "^0.2.0-rc.1",
    ));
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("Pre-release dependency: sirene 0.2.0"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_prerelease_may_depend_on_prereleases() {
    let out = check_versions_with(&metadata(
        ["0.2.0-rc.1", "0.2.0-rc.1", "0.2.0-rc.1"],
        ["^0.2.0-rc.1", "^0.2.0-rc.1"],
        "^0.2.0-rc.1",
    ));
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn a_missing_root_fails_the_check() {
    let out = check_versions_with(r#"{"packages":[]}"#);
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out),
        "Could not determine the sirene version from cargo metadata.\n"
    );
}

/// Put a fake `cargo` first on `PATH` that echoes its arguments, prints
/// `output`, and exits with `code`, then run `publish.sh`, which calls a bare
/// `cargo publish` per crate.
fn publish_with_stub_cargo(output_text: &str, code: i32) -> Output {
    let dir = scratch_dir();
    let bin = dir.path().join("stub-bin");
    std::fs::create_dir_all(&bin).expect("create stub bin dir");
    write_exec(
        &bin.join("cargo"),
        &format!(
            "#!/bin/bash\necho \"cargo $*\"\ncat <<'STUB_EOF'\n{output_text}\nSTUB_EOF\nexit {code}\n"
        ),
    );
    output(
        Command::new("bash")
            .arg(repo_root().join("bin").join("publish.sh"))
            .current_dir(repo_root())
            .env("PATH", path_with(&bin)),
    )
}

#[test]
fn publish_reports_success() {
    let out = publish_with_stub_cargo("Uploaded", 0);
    assert!(out.status.success(), "{}", stderr(&out));
    let log = stdout(&out);
    for dir in ["crates/sirene-core", "crates/sirene-gui", "crates/sirene"] {
        assert!(
            log.contains(&format!("Published crate in {dir}\n")),
            "{log}"
        );
    }
    // Dependency order.
    let core = log.find("crates/sirene-core...").unwrap();
    let gui = log.find("crates/sirene-gui...").unwrap();
    let root = log.find("crates/sirene...").unwrap();
    assert!(core < gui && gui < root, "{log}");
    assert!(
        log.contains("3 published, 0 already current, 0 failed"),
        "{log}"
    );
    // The token is in the environment, so nothing may be built.
    assert_eq!(
        log.matches("cargo publish --locked --no-verify\n").count(),
        3,
        "{log}"
    );
}

/// Cargo's pre-flight check against the index produces this wording.
#[test]
fn publish_treats_an_index_conflict_as_a_benign_skip() {
    let out = publish_with_stub_cargo(
        "error: crate version `0.1.0` already exists on crates.io index",
        1,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("3 already current"));
}

/// crates.io rejects the upload itself with this wording on a re-run after a
/// partial publish.
#[test]
fn publish_treats_a_server_side_upload_conflict_as_a_benign_skip() {
    let out = publish_with_stub_cargo(
        "error: failed to publish to registry at https://crates.io\n\nCaused by:\n  the remote server responded with an error: crate version `0.1.0` is already uploaded",
        1,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("3 already current"));
}

/// Every other failure still fails the release.
#[test]
fn publish_fails_the_release_on_any_other_error() {
    let out = publish_with_stub_cargo("error: failed to verify package tarball", 1);
    assert!(!out.status.success());
    assert!(stdout(&out).contains("3 failed"));
    assert!(stderr(&out).contains("Release failed"));
}

const CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n## [1.2.0] - 2026-01-02\n\n\n- one\n\n- two\n\n\n## [1.1.0] - 2026-01-01\n\n- old\n";

fn changelog_section(version: &str) -> Output {
    let dir = scratch_dir();
    let path = dir.path().join("CHANGELOG.md");
    std::fs::write(&path, CHANGELOG).unwrap();
    run("changelog-section.sh", &[version, path.to_str().unwrap()])
}

#[test]
fn changelog_section_is_trimmed_and_stops_at_the_next_heading() {
    let out = changelog_section("1.2.0");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "- one\n\n- two\n");
    let out = changelog_section("1.1.0");
    assert_eq!(stdout(&out), "- old\n");
}

/// `1.2` must not match `1.2.0`, and a missing version fails the release.
#[test]
fn changelog_section_requires_the_exact_version() {
    for version in ["1.2", "9.9.9"] {
        let out = changelog_section(version);
        assert!(!out.status.success(), "{version}: {}", stdout(&out));
        assert_eq!(stdout(&out), "", "{version}");
    }
}

/// The workspace version `decide` would release has its notes written.
#[test]
fn the_changelog_has_a_section_for_a_stable_workspace_version() {
    let version = env!("CARGO_PKG_VERSION");
    if version.contains('-') {
        return;
    }
    let out = run("changelog-section.sh", &[version]);
    assert!(
        out.status.success(),
        "CHANGELOG.md needs a `## [{version}]` section before this version can be released"
    );
    assert!(!stdout(&out).trim().is_empty());
}

/// The release matrix's `target:` entries.
fn release_matrix_targets() -> BTreeSet<String> {
    read(&repo_root().join(".github/workflows/release.yml"))
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- target:"))
        .map(|t| t.trim().to_owned())
        .collect()
}

/// The tarball targets that `update-formula.sh` names.
const FORMULA_TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
];

#[test]
fn update_formula_writes_each_tarball_checksum() {
    let dir = scratch_dir();
    let dist = dir.path().join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    for target in FORMULA_TARGETS {
        std::fs::write(dist.join(format!("sirene-1.2.3-{target}.tar.gz")), target).unwrap();
    }
    let formula = dir.path().join("Formula/sirene.rb");
    let out = run(
        "update-formula.sh",
        &["1.2.3", dist.to_str().unwrap(), formula.to_str().unwrap()],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        format!("wrote {} for v1.2.3\n", formula.display())
    );
    let text = read(&formula);
    assert!(text.contains("class Sirene < Formula\n"), "{text}");
    assert!(text.contains("  version \"1.2.3\"\n"), "{text}");
    assert!(text.contains("    bin.install \"sirene\"\n"), "{text}");
    for target in FORMULA_TARGETS {
        let sum = sha256(&dist.join(format!("sirene-1.2.3-{target}.tar.gz")));
        let url = format!(
            "      url \"https://github.com/jaemk/sirene/releases/download/v1.2.3/sirene-1.2.3-{target}.tar.gz\"\n      sha256 \"{sum}\"\n"
        );
        assert!(text.contains(&url), "{target}:\n{text}");
    }
}

#[test]
fn update_formula_fails_on_a_missing_tarball() {
    let dir = scratch_dir();
    let formula = dir.path().join("sirene.rb");
    let out = run(
        "update-formula.sh",
        &[
            "1.2.3",
            dir.path().to_str().unwrap(),
            formula.to_str().unwrap(),
        ],
    );
    assert!(!out.status.success());
    assert!(!formula.exists());
}

/// Every tarball the formula and the installer name is built by the release.
#[test]
fn the_release_matrix_builds_every_named_tarball() {
    let matrix = release_matrix_targets();
    let installer: BTreeSet<String> = ["x86_64", "aarch64"]
        .iter()
        .flat_map(|arch| {
            ["unknown-linux-musl", "unknown-linux-gnu", "apple-darwin"]
                .map(|os| format!("{arch}-{os}"))
        })
        .collect();
    let formula: BTreeSet<String> = FORMULA_TARGETS.map(String::from).into();
    let missing: Vec<_> = installer
        .union(&formula)
        .filter(|t| !matrix.contains(*t))
        .collect();
    assert!(
        missing.is_empty(),
        "not built: {missing:?}; matrix {matrix:?}"
    );
    let script = read(&repo_root().join("bin/update-formula.sh"));
    for target in FORMULA_TARGETS {
        assert!(script.contains(&format!("sha {target}")), "{target}");
    }
}

const MANIFEST: &str = r#"[workspace.package]
version = "1.2.3"
edition = "2024"

[workspace.dependencies]
sirene-core = { path = "crates/sirene-core", version = "1.2.3" }
sirene-gui = { path = "crates/sirene-gui", version = "1.2.3" }
other = { version = "1.2.3" }
"#;

fn bump_dev(version: &str) -> (Output, String) {
    let dir = scratch_dir();
    let path = dir.path().join("Cargo.toml");
    std::fs::write(&path, MANIFEST).unwrap();
    let out = run("bump-dev.sh", &[version, path.to_str().unwrap()]);
    (out, read(&path))
}

#[test]
fn bump_dev_moves_the_workspace_to_the_next_patch() {
    let (out, text) = bump_dev("1.2.3");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "1.2.4-dev\n");
    assert_eq!(
        text,
        MANIFEST
            .replacen("version = \"1.2.3\"\n", "version = \"1.2.4-dev\"\n", 1)
            .replace("\", version = \"1.2.3\" }", "\", version = \"1.2.4-dev\" }"),
        "only the workspace version and the member pins change"
    );
    assert!(text.contains("other = { version = \"1.2.3\" }"), "{text}");
}

/// A main that has already moved on is left alone.
#[test]
fn bump_dev_leaves_another_version_alone() {
    for version in ["1.2.2", "1.2"] {
        let (out, text) = bump_dev(version);
        assert!(out.status.success(), "{}", stderr(&out));
        assert_eq!(stdout(&out), "");
        assert_eq!(text, MANIFEST);
    }
}

/// A fixture for `install.sh`: stub `uname`, `curl`, and `gh` in `bin/`, the
/// release assets the stub `curl` serves in `assets/`, and a log of the URLs it
/// was asked for.
struct Install {
    dir: TempDir,
}

impl Install {
    fn new(os: &str, arch: &str) -> Install {
        let fx = Install { dir: scratch_dir() };
        for d in ["bin", "assets"] {
            std::fs::create_dir_all(fx.path(d)).unwrap();
        }
        write_exec(
            &fx.path("bin/uname"),
            &format!("#!/bin/sh\ncase \"$1\" in\n-s) echo {os} ;;\n-m) echo {arch} ;;\nesac\n"),
        );
        // Serves assets/<basename> (or the latest-release JSON), else exits
        // 22 like `curl -f` on a 404.
        write_exec(
            &fx.path("bin/curl"),
            &format!(
                r#"#!/bin/sh
url=""; dest=""; prev=""
for a in "$@"; do
  [ "$prev" = "-o" ] && dest="$a"
  case "$a" in https://*) url="$a" ;; esac
  prev="$a"
done
printf '%s\n' "$url" >> "{dir}/urls.log"
case "$url" in
*/releases/latest) echo '{{"tag_name": "v9.9.9"}}' ;;
*) f="{dir}/assets/${{url##*/}}"; [ -f "$f" ] || exit 22; cp "$f" "$dest" ;;
esac
"#,
                dir = fx.dir.path().display()
            ),
        );
        write_exec(&fx.path("bin/gh"), "#!/bin/sh\nexit 1\n");
        fx
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Publishes a tarball holding a `sirene` that contains `payload`.
    fn publish(&self, target: &str, payload: &str) -> String {
        let name = format!("sirene-9.9.9-{target}.tar.gz");
        let staging = self.path(&format!("staging-{target}"));
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("sirene"), payload).unwrap();
        let out = output(
            Command::new("tar")
                .arg("-czf")
                .arg(self.path("assets").join(&name))
                .arg("-C")
                .arg(&staging)
                .arg("sirene"),
        );
        assert!(out.status.success());
        name
    }

    /// Writes SHA256SUMS for the named assets, with their real checksums
    /// unless `digest` is given.
    fn sums(&self, names: &[&str], digest: Option<&str>) {
        let mut body = String::new();
        for name in names {
            let real = sha256(&self.path("assets").join(name));
            let sum = digest.unwrap_or(&real);
            body.push_str(&format!("{sum}  {name}\n"));
        }
        std::fs::write(self.path("assets/SHA256SUMS"), body).unwrap();
    }

    fn run(&self, env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new("sh");
        cmd.arg(repo_root().join("bin/install.sh"))
            .env("PATH", path_with(&self.path("bin")))
            .env("SIRENE_INSTALL_DIR", self.path("install"))
            .env_remove("SIRENE_VERSION")
            .env_remove("SIRENE_TARGET");
        for (k, v) in env {
            cmd.env(k, v);
        }
        output(&mut cmd)
    }

    fn installed(&self) -> Option<String> {
        std::fs::read_to_string(self.path("install/sirene")).ok()
    }

    fn assets_requested(&self) -> Vec<String> {
        std::fs::read_to_string(self.path("urls.log"))
            .unwrap_or_default()
            .lines()
            .filter(|l| l.ends_with(".tar.gz"))
            .map(|l| l.rsplit('/').next().unwrap().to_owned())
            .collect()
    }
}

#[test]
fn install_prefers_the_musl_binary_on_linux() {
    let fx = Install::new("Linux", "x86_64");
    let musl = fx.publish("x86_64-unknown-linux-musl", "MUSL");
    fx.publish("x86_64-unknown-linux-gnu", "GNU");
    fx.sums(&[&musl], None);
    let out = fx.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.installed().as_deref(), Some("MUSL"));
    assert_eq!(fx.assets_requested(), [musl]);
    let mode = std::fs::metadata(fx.path("install/sirene"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
    assert!(
        stdout(&out).contains("installed sirene 9.9.9 to "),
        "{}",
        stdout(&out)
    );
    assert!(stderr(&out).contains("could not be verified (continuing)"));
}

#[test]
fn install_replaces_a_symlink_without_writing_its_target() {
    let fx = Install::new("Linux", "x86_64");
    let musl = fx.publish("x86_64-unknown-linux-musl", "NEW");
    fx.sums(&[&musl], None);
    std::fs::create_dir_all(fx.path("install")).unwrap();
    std::fs::write(fx.path("sentinel"), "KEEP").unwrap();
    std::os::unix::fs::symlink(fx.path("sentinel"), fx.path("install/sirene")).unwrap();
    write_exec(&fx.path("bin/install"), "#!/bin/sh\nexit 1\n");

    let out = fx.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(read(&fx.path("sentinel")), "KEEP");
    assert_eq!(fx.installed().as_deref(), Some("NEW"));
    assert!(
        std::fs::symlink_metadata(fx.path("install/sirene"))
            .unwrap()
            .is_file()
    );
    assert_eq!(
        std::fs::metadata(fx.path("install/sirene"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(std::fs::read_dir(fx.path("install")).unwrap().count(), 1);
}

#[test]
fn install_refuses_a_directory_destination() {
    for linked in [false, true] {
        let fx = Install::new("Linux", "x86_64");
        let musl = fx.publish("x86_64-unknown-linux-musl", "NEW");
        fx.sums(&[&musl], None);
        std::fs::create_dir_all(fx.path("install")).unwrap();
        let directory = if linked {
            fx.path("other")
        } else {
            fx.path("install/sirene")
        };
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("sentinel"), "KEEP").unwrap();
        if linked {
            std::os::unix::fs::symlink(&directory, fx.path("install/sirene")).unwrap();
        }

        let out = fx.run(&[]);
        assert!(!out.status.success());
        assert!(
            stderr(&out).contains("is a directory; refusing to install"),
            "{}",
            stderr(&out)
        );
        assert_eq!(read(&directory.join("sentinel")), "KEEP");
        assert!(!directory.join("sirene").exists());
        assert_eq!(std::fs::read_dir(fx.path("install")).unwrap().count(), 1);
    }
}

#[test]
fn install_failed_staging_preserves_the_old_binary_and_cleans_up() {
    let fx = Install::new("Linux", "x86_64");
    let musl = fx.publish("x86_64-unknown-linux-musl", "NEW");
    fx.sums(&[&musl], None);
    std::fs::create_dir_all(fx.path("install")).unwrap();
    std::fs::write(fx.path("install/sirene"), "OLD").unwrap();
    write_exec(&fx.path("bin/install"), "#!/bin/sh\nexit 1\n");
    write_exec(
        &fx.path("bin/cp"),
        "#!/bin/sh\ncase \"$2\" in */install/.sirene.*) printf partial > \"$2\"; exit 1 ;; esac\nexec /bin/cp \"$@\"\n",
    );

    let out = fx.run(&[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("could not write to"),
        "{}",
        stderr(&out)
    );
    assert_eq!(fx.installed().as_deref(), Some("OLD"));
    assert_eq!(std::fs::read_dir(fx.path("install")).unwrap().count(), 1);
}

#[test]
fn install_falls_back_to_gnu_once_when_musl_is_missing() {
    let fx = Install::new("Linux", "aarch64");
    let gnu = fx.publish("aarch64-unknown-linux-gnu", "GNU");
    fx.sums(&[&gnu], None);
    let out = fx.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.installed().as_deref(), Some("GNU"));
    assert_eq!(
        fx.assets_requested(),
        [
            "sirene-9.9.9-aarch64-unknown-linux-musl.tar.gz",
            gnu.as_str()
        ]
    );
    assert!(stdout(&out).contains("musl asset unavailable for 9.9.9, retrying with gnu"));

    let fx = Install::new("Linux", "x86_64");
    let out = fx.run(&[]);
    assert!(!out.status.success());
    assert_eq!(fx.assets_requested().len(), 2);
    assert!(
        stderr(&out).contains("download failed: "),
        "{}",
        stderr(&out)
    );
}

#[test]
fn install_does_not_fall_back_on_macos() {
    let fx = Install::new("Darwin", "arm64");
    let out = fx.run(&[]);
    assert!(!out.status.success());
    assert_eq!(
        fx.assets_requested(),
        ["sirene-9.9.9-aarch64-apple-darwin.tar.gz"]
    );

    let fx = Install::new("Darwin", "x86_64");
    let mac = fx.publish("x86_64-apple-darwin", "MAC");
    fx.sums(&[&mac], None);
    let out = fx.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.installed().as_deref(), Some("MAC"));
}

/// SIRENE_TARGET picks the gnu (GUI) build, and SIRENE_VERSION, with or
/// without a leading `v`, skips the latest-release lookup.
#[test]
fn install_honors_the_target_and_version_overrides() {
    for version in ["9.9.9", "v9.9.9"] {
        let fx = Install::new("Linux", "x86_64");
        let gnu = fx.publish("x86_64-unknown-linux-gnu", "GNU");
        fx.sums(&[&gnu], None);
        let out = fx.run(&[
            ("SIRENE_TARGET", "x86_64-unknown-linux-gnu"),
            ("SIRENE_VERSION", version),
        ]);
        assert!(out.status.success(), "{}", stderr(&out));
        assert_eq!(fx.installed().as_deref(), Some("GNU"));
        assert_eq!(fx.assets_requested(), [gnu]);
        let urls = read(&fx.path("urls.log"));
        assert!(!urls.contains("releases/latest"), "{urls}");
        assert!(!urls.contains("/vv"), "{urls}");
    }
}

/// An explicit SIRENE_TARGET is never swapped for another build.
#[test]
fn install_does_not_fall_back_from_an_explicit_target() {
    let fx = Install::new("Linux", "x86_64");
    let gnu = fx.publish("x86_64-unknown-linux-gnu", "GNU");
    fx.sums(&[&gnu], None);
    let out = fx.run(&[("SIRENE_TARGET", "x86_64-unknown-linux-musl")]);
    assert!(!out.status.success());
    assert_eq!(
        fx.assets_requested(),
        ["sirene-9.9.9-x86_64-unknown-linux-musl.tar.gz"]
    );
    assert_eq!(fx.installed(), None);
}

/// Only main releases: a dispatch from another branch would publish that
/// branch and commit its formula and cask to main.
#[test]
fn the_release_workflow_runs_only_on_main() {
    let workflow = read(&repo_root().join(".github/workflows/release.yml"));
    assert!(
        workflow.contains(
            "    if: github.repository == 'jaemk/sirene' && github.ref == 'refs/heads/main'\n"
        ),
        "decide must check the ref"
    );
}

/// The release tarballs carry the licenses and third-party notices.
#[test]
fn the_release_tarball_carries_the_notices() {
    let workflow = read(&repo_root().join(".github/workflows/release.yml"));
    let tar = workflow
        .lines()
        .find(|l| l.trim_start().starts_with("tar -czf"))
        .expect("a tar line");
    for name in ["LICENSE-MIT", "LICENSE-APACHE", "THIRD-PARTY-NOTICES"] {
        assert!(tar.contains(name), "{name} missing from: {tar}");
        assert!(repo_root().join(name).is_file(), "{name}");
    }
}

#[test]
fn install_refuses_a_bad_or_missing_checksum() {
    let fx = Install::new("Linux", "x86_64");
    let musl = fx.publish("x86_64-unknown-linux-musl", "MUSL");
    fx.sums(&[&musl], Some(&"0".repeat(64)));
    let out = fx.run(&[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains(&format!("checksum mismatch for {musl}")),
        "{}",
        stderr(&out)
    );
    assert_eq!(fx.installed(), None);

    std::fs::write(fx.path("assets/SHA256SUMS"), "").unwrap();
    let out = fx.run(&[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains(&format!("SHA256SUMS has no entry for {musl}")));
    assert_eq!(fx.installed(), None);
}

#[test]
fn install_refuses_an_unsupported_platform_before_any_request() {
    for (os, arch, message) in [
        ("Plan9", "x86_64", "unsupported OS: Plan9"),
        ("Linux", "riscv64", "unsupported architecture: riscv64"),
    ] {
        let fx = Install::new(os, arch);
        let out = fx.run(&[]);
        assert!(!out.status.success());
        assert!(stderr(&out).contains(message), "{}", stderr(&out));
        assert!(!fx.path("urls.log").exists());
    }
}

/// The committed formula, once a release has written one, is what
/// `update-formula.sh` produces for its version and checksums.
#[test]
fn the_committed_formula_matches_the_script() {
    let committed = repo_root().join("Formula/sirene.rb");
    let Ok(text) = std::fs::read_to_string(&committed) else {
        return;
    };
    let version = text
        .lines()
        .find_map(|l| l.strip_prefix("  version \"")?.strip_suffix('"'))
        .expect("a version line");
    let sums: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("sha256 \"")?.strip_suffix('"'))
        .collect();
    assert_eq!(sums.len(), FORMULA_TARGETS.len(), "{text}");
    // Regenerate with placeholder checksums, then put the committed ones back
    // in order; the rest must be byte-identical.
    let dir = scratch_dir();
    for target in FORMULA_TARGETS {
        std::fs::write(
            dir.path().join(format!("sirene-{version}-{target}.tar.gz")),
            "",
        )
        .unwrap();
    }
    let out_file = dir.path().join("sirene.rb");
    let out = run(
        "update-formula.sh",
        &[
            version,
            dir.path().to_str().unwrap(),
            out_file.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let mut regenerated = read(&out_file);
    for sum in sums {
        regenerated = regenerated.replacen(empty, sum, 1);
    }
    assert_eq!(text, regenerated);
}
