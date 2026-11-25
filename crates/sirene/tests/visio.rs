//! `.vsdx` drawings through the CLI (VISIO-11, VISIO-12): `svg`, `png`, `ascii`, and `check`
//! read a package as bytes, detected by extension or the ZIP magic.

#[path = "../../sirene-core/src/visio/testpkg.rs"]
mod testpkg;

use assert_cmd::Command;
use predicates::prelude::*;
use testpkg::*;

fn sirene() -> Command {
    let mut command = Command::cargo_bin("sirene").expect("sirene binary is built");
    command.env_remove("SIRENE_LOG").env_remove("RUST_LOG");
    command
}

fn one_page() -> Vec<u8> {
    let sh = shape(
        1,
        "",
        &format!("{}{}", xform(1.0, 1.0, 2.0, 1.0), rect_geometry(2.0, 1.0)),
    );
    Builder::new().page(6.0, 4.0, &sh).build()
}

fn two_pages() -> Vec<u8> {
    let sh = shape(
        1,
        "",
        &format!("{}{}", xform(1.0, 1.0, 2.0, 1.0), rect_geometry(2.0, 1.0)),
    );
    Builder::new()
        .page(6.0, 4.0, &sh)
        .page(3.0, 2.0, &sh)
        .build()
}

#[test]
fn svg_draws_a_vsdx_file_by_extension() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.vsdx");
    std::fs::write(&file, one_page()).unwrap();
    sirene()
        .arg("svg")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("<svg").and(predicate::str::contains("<path")))
        .stderr("");
}

#[test]
fn a_package_is_detected_by_its_magic_without_the_extension() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("drawing.bin");
    std::fs::write(&file, one_page()).unwrap();
    sirene()
        .arg("svg")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("<path"));
    // And from stdin, with --lang.
    sirene()
        .args(["--lang", "vsdx", "svg"])
        .write_stdin(one_page())
        .assert()
        .success()
        .stdout(predicate::str::contains("<path"));
}

#[test]
fn png_writes_an_image() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.vsdx");
    std::fs::write(&file, one_page()).unwrap();
    sirene().arg("png").arg(&file).assert().success();
    let png = std::fs::read(tmp.path().join("a.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
}

#[test]
fn ascii_refuses_a_drawing() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.vsdx");
    std::fs::write(&file, one_page()).unwrap();
    sirene()
        .arg("ascii")
        .arg(&file)
        .assert()
        .failure()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "error: a Visio drawing has no text form",
        ));
}

#[test]
fn pages_are_chosen_or_written_as_stem_n() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("deck.vsdx");
    std::fs::write(&file, two_pages()).unwrap();
    // Several pages to stdout is an error.
    sirene()
        .arg("svg")
        .arg(&file)
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::contains("2 pages"));
    // --page picks one.
    let page = |n: &str| {
        let out = sirene()
            .arg("svg")
            .arg(&file)
            .args(["--page", n])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    assert!(page("1").contains("576"));
    assert!(page("2").contains("288"));
    sirene()
        .arg("svg")
        .arg(&file)
        .args(["--page", "3"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("out of range"));
    sirene()
        .arg("svg")
        .arg(&file)
        .args(["--page", "0"])
        .assert()
        .failure();
    // With --output, one file per page.
    let out = tmp.path().join("out.svg");
    sirene()
        .arg("svg")
        .arg(&file)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(tmp.path().join("out-1.svg"))
            .unwrap()
            .contains("576")
    );
    assert!(
        std::fs::read_to_string(tmp.path().join("out-2.svg"))
            .unwrap()
            .contains("288")
    );
    assert!(!out.exists());
}

#[test]
fn a_directory_scan_picks_up_vsdx_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.vsdx"), one_page()).unwrap();
    sirene().arg("svg").arg(tmp.path()).assert().success();
    assert!(tmp.path().join("a.svg").exists());
}

#[test]
fn check_reports_package_errors_without_a_position() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good.vsdx");
    std::fs::write(&good, one_page()).unwrap();
    sirene()
        .arg("check")
        .arg(&good)
        .assert()
        .success()
        .stderr("");
    let bad = tmp.path().join("bad.vsdx");
    std::fs::write(&bad, b"PK\x03\x04 not a package").unwrap();
    sirene()
        .arg("check")
        .arg(&bad)
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad.vsdx: error: "));
    sirene()
        .arg("svg")
        .arg(&bad)
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad.vsdx: error: "));
}
