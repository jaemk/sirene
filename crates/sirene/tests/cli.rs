use assert_cmd::Command;
use predicates::prelude::*;

fn sirene() -> Command {
    Command::cargo_bin("sirene").expect("sirene binary is built")
}

#[test]
fn prints_version() {
    sirene()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(concat!(
            "sirene ",
            env!("CARGO_PKG_VERSION")
        )));
}

#[test]
fn prints_help() {
    sirene()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Mermaid diagram editor and viewer",
        ));
}

#[test]
fn runs_without_arguments() {
    sirene().assert().success();
}

#[test]
fn rejects_unknown_flag() {
    sirene()
        .arg("--bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bogus"));
}
