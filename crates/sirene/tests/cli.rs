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

fn fixture(name: &str) -> String {
    format!(
        "{}/../sirene-core/tests/fixtures/flowchart/valid/{name}.mmd",
        env!("CARGO_MANIFEST_DIR")
    )
}

const AB: &str = "\
┌───┐
│ A │
└─┬─┘
  │
  ▼
┌───┐
│ B │
└───┘
";

#[test]
fn ascii_reads_stdin() {
    sirene()
        .arg("ascii")
        .write_stdin("flowchart TD\nA --> B\n")
        .assert()
        .success()
        .stdout(AB)
        .stderr("");
    sirene()
        .args(["ascii", "-"])
        .write_stdin("flowchart TD\nA --> B\n")
        .assert()
        .success()
        .stdout(AB);
}

/// The example in the README.
#[test]
fn readme_example() {
    sirene()
        .arg("ascii")
        .write_stdin("flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n")
        .assert()
        .success()
        .stdout(concat!(
            "┌───────┐    ╱─────╲       ┌──────┐\n",
            "│ Start ├───►│ Ok? ├──yes─►│ Done │\n",
            "└───────┘    ╲─────╱       └──────┘\n",
        ));
}

#[test]
fn ascii_reads_a_file() {
    sirene()
        .args(["ascii", &fixture("accessibility")])
        .assert()
        .success()
        .stdout("┌───┐    ┌───┐\n│ A ├───►│ B │\n└───┘    └───┘\n");
}

#[test]
fn ascii_charset() {
    sirene()
        .args(["ascii", "--charset", "ascii", &fixture("accessibility")])
        .assert()
        .success()
        .stdout("+---+    +---+\n| A +--->| B |\n+---+    +---+\n");
    sirene()
        .args(["ascii", "--charset", "ebcdic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ebcdic"));
}

#[test]
fn ascii_reports_warnings() {
    sirene()
        .arg("ascii")
        .write_stdin("flowchart LR\nclick Z cb\n")
        .assert()
        .success()
        .stdout("")
        .stderr("<stdin>:2:7: warning: `click` refers to unknown node `Z`\n");
}

#[test]
fn ascii_reports_errors_and_still_renders() {
    sirene()
        .arg("ascii")
        .write_stdin("flowchart TD\nA --> B\nlinkStyle 5 color:red\n")
        .assert()
        .failure()
        .stdout(AB)
        .stderr(predicate::str::starts_with("<stdin>:3:"))
        .stderr(predicate::str::contains(": error: "));
}

#[test]
fn ascii_read_failures() {
    sirene()
        .args(["ascii", "does/not/exist.mmd"])
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::starts_with(
            "error: cannot read does/not/exist.mmd: ",
        ));
    sirene()
        .arg("ascii")
        .write_stdin(vec![0xff, 0xfe])
        .assert()
        .failure()
        .stderr(predicate::str::starts_with("error: cannot read <stdin>: "));
}

#[test]
fn rejects_unknown_flag() {
    sirene()
        .arg("--bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bogus"));
}
