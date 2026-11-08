/*!
Coverage for the release shell scripts in `bin/` (WORKSP-5).

Neither script runs in a normal build, and both only run during a release, where
a mistake is expensive and hard to reverse: `check-versions.sh` stops a
half-bumped workspace from publishing, and `publish.sh`'s exit status decides
whether a release reports green.

The `check-versions.sh` failure cases hand the script a crafted `cargo metadata`
document, which is why it accepts one as an argument.
*/

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn run(script: &str, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(repo_root().join("bin").join(script))
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("failed to run bin/{script}: {e}"))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
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

/// Put a fake `cargo` first on `PATH` that prints `output` and exits with
/// `code`, then run `publish.sh`, which calls a bare `cargo publish` per crate.
fn publish_with_stub_cargo(output: &str, code: i32) -> Output {
    let dir = scratch_dir();
    let bin = dir.path().join("stub-bin");
    std::fs::create_dir_all(&bin).expect("create stub bin dir");
    let stub = bin.join("cargo");
    std::fs::write(
        &stub,
        format!("#!/bin/bash\ncat <<'STUB_EOF'\n{output}\nSTUB_EOF\nexit {code}\n"),
    )
    .expect("write stub cargo");

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
        .expect("make stub cargo executable");

    let path = std::env::var("PATH").unwrap_or_default();
    Command::new("bash")
        .arg(repo_root().join("bin").join("publish.sh"))
        .current_dir(repo_root())
        .env("PATH", format!("{}:{path}", bin.display()))
        .output()
        .expect("failed to run bin/publish.sh")
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

/// Runs `tag-release.sh` with stub `git` and `gh` that log their arguments.
/// `existing` lists the tags that already have a remote tag and a release.
fn tag_release_with_stubs(existing: &[&str]) -> (Output, String) {
    let dir = scratch_dir();
    let bin = dir.path().join("stub-bin");
    std::fs::create_dir_all(&bin).expect("create stub bin dir");
    let log = dir.path().join("calls.log");
    let known = existing.join(" ");
    let stubs = [
        (
            "git",
            format!(
                r#"#!/bin/bash
echo "git $*" >> "{log}"
case "$1" in
  ls-remote) for t in {known}; do [ "refs/tags/$t" = "$4" ] && printf 'sha\trefs/tags/%s\n' "$t"; done ;;
  rev-parse) exit 1 ;;
esac
exit 0
"#,
                log = log.display()
            ),
        ),
        (
            "gh",
            format!(
                r#"#!/bin/bash
echo "gh $*" >> "{log}"
if [ "$2" = view ]; then
  for t in {known}; do [ "$t" = "$3" ] && exit 0; done
  exit 1
fi
exit 0
"#,
                log = log.display()
            ),
        ),
    ];
    use std::os::unix::fs::PermissionsExt;
    for (name, body) in stubs {
        let path = bin.join(name);
        std::fs::write(&path, body).expect("write stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make stub executable");
    }
    let path = std::env::var("PATH").unwrap_or_default();
    let out = Command::new("bash")
        .arg(repo_root().join("bin").join("tag-release.sh"))
        .current_dir(repo_root())
        .env("PATH", format!("{}:{path}", bin.display()))
        .env_remove("GITHUB_ACTIONS")
        .output()
        .expect("failed to run bin/tag-release.sh");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    (out, calls)
}

/// Every crate gets a tag and release; only the root crate's may be "Latest".
#[test]
fn tag_release_marks_only_the_root_release_latest() {
    let version = env!("CARGO_PKG_VERSION");
    let (out, calls) = tag_release_with_stubs(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    for (tag, latest) in [
        (format!("v{version}"), true),
        (format!("sirene-core-v{version}"), false),
        (format!("sirene-gui-v{version}"), false),
    ] {
        assert!(
            calls.contains(&format!("git tag -a {tag} -m Release {tag}\n")),
            "{calls}"
        );
        assert!(
            calls.contains(&format!("git push origin {tag}\n")),
            "{calls}"
        );
        assert!(
            calls.contains(&format!(
                "gh release create {tag} --generate-notes --title {tag} --latest={latest}\n"
            )),
            "{calls}"
        );
    }
}

/// Tags that already have a tag and release are left alone.
#[test]
fn tag_release_skips_existing_releases() {
    let version = env!("CARGO_PKG_VERSION");
    let root = format!("v{version}");
    let (out, calls) = tag_release_with_stubs(&[&root]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains(&format!("Tag {root} and its GitHub release already exist")),
        "{}",
        stdout(&out)
    );
    assert!(
        !calls.contains(&format!("git push origin {root}\n")),
        "{calls}"
    );
    assert!(
        calls.contains(&format!("git push origin sirene-core-v{version}\n")),
        "{calls}"
    );
}
