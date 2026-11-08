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

/// `--verbose` (before or after the command) reports each step on stderr and leaves stdout
/// alone; without it nothing extra is printed.
#[test]
fn verbose_reports_each_step() {
    for args in [["-v", "ascii"], ["ascii", "--verbose"]] {
        let out = sirene()
            .args(args)
            .write_stdin("flowchart TD\nA --> B\n")
            .assert()
            .success()
            .stdout(AB)
            .get_output()
            .stderr
            .clone();
        let err = String::from_utf8(out).unwrap();
        for step in [
            "sirene: sirene ",
            "reading <stdin>",
            "read <stdin>: 21 bytes",
            "sirene_core::flowchart: parsed 21 bytes, 2 lines",
            "lowered 2 nodes, 1 edges",
            "sirene_core::layout: laid out 2 nodes",
            "rendered ",
            "writing <stdout>",
            "exit with success",
        ] {
            assert!(err.contains(step), "missing {step:?} in\n{err}");
        }
        assert!(err.lines().all(|l| l.starts_with('[')), "{err}");
    }

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("d.mmd");
    std::fs::write(&src, "flowchart\nA --> B\n").unwrap();
    let out = sirene()
        .args(["png", "-v"])
        .arg(&src)
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();
    let err = String::from_utf8(out).unwrap();
    for step in [
        "no --output; writing next to the input as",
        "drew a ",
        "sirene_core::scene::png: encoded a ",
        "d.png",
    ] {
        assert!(err.contains(step), "missing {step:?} in\n{err}");
    }

    sirene()
        .args(["-v", "ascii", "missing.mmd"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("reading missing.mmd"))
        .stderr(predicate::str::contains("error: cannot read missing.mmd"))
        .stderr(predicate::str::contains("exit with failure"));
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
fn svg_output() {
    let out = sirene()
        .args(["svg", &fixture("accessibility")])
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let svg = String::from_utf8(out).unwrap();
    assert!(
        svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
        "{svg}"
    );
    assert!(svg.contains(">A</text>"), "{svg}");
    assert!(svg.contains("fill=\"#ffffff\""), "{svg}");
    assert!(svg.ends_with("</svg>\n"), "{svg}");
    sirene()
        .args(["svg", "--theme", "dark", "-"])
        .write_stdin("flowchart\nA\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fill=\"#1e1e1e\""));
    sirene()
        .args(["svg", "-"])
        .write_stdin("flowchart\nlinkStyle 3 color:red\n")
        .assert()
        .failure()
        .stdout(predicate::str::starts_with("<svg"))
        .stderr(predicate::str::contains(": error: "));
}

/// The width and height in a PNG's header.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
    assert_eq!(&bytes[12..16], b"IHDR");
    let n = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
    (n(16), n(20))
}

#[test]
fn svg_to_a_file() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out.svg");
    sirene()
        .args([
            "svg",
            &fixture("accessibility"),
            "-o",
            out.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout("");
    let svg = std::fs::read_to_string(&out).unwrap();
    assert!(
        svg.starts_with("<svg ") && svg.ends_with("</svg>\n"),
        "{svg}"
    );
}

#[test]
fn png_next_to_the_input() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("diagram.mmd");
    std::fs::write(&src, "flowchart LR\nA[Hello] --> B[World]\n").unwrap();
    sirene()
        .args(["png", src.to_str().unwrap()])
        .assert()
        .success()
        .stdout("")
        .stderr("");
    let (w4, h4) = png_size(&std::fs::read(tmp.path().join("diagram.png")).unwrap());

    let out = tmp.path().join("small.png");
    sirene()
        .args([
            "png",
            src.to_str().unwrap(),
            "--scale",
            "1",
            "--theme",
            "dark",
        ])
        .args(["--output", out.to_str().unwrap()])
        .assert()
        .success();
    let (w1, h1) = png_size(&std::fs::read(&out).unwrap());
    assert!(w1 > 50 && h1 > 20, "{w1}x{h1}");
    // The default is fine quality, at 4 pixels per unit.
    assert!(
        w4.abs_diff(4 * w1) <= 3 && h4.abs_diff(4 * h1) <= 3,
        "{w4}x{h4} vs {w1}x{h1}"
    );
}

/// `--quality` sets the scale unless `--scale` is given.
#[test]
fn png_quality() {
    let size = |args: &[&str]| {
        let out = sirene()
            .arg("png")
            .args(args)
            .write_stdin("flowchart LR\nA[Hello] --> B[World]\n")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        png_size(&out)
    };
    let (w1, _) = size(&["--quality", "draft"]);
    for (args, n) in [
        (&["--quality", "normal"][..], 2),
        (&["--quality", "fine"], 4),
        (&[], 4),
        (&["--quality", "draft", "--scale", "3"], 3),
    ] {
        let (w, _) = size(args);
        assert!(w.abs_diff(n * w1) <= n, "{args:?}: {w} vs {w1}");
    }
    sirene()
        .args(["png", "--quality", "best", "-"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("best"));
}

#[test]
fn png_from_stdin() {
    let out = sirene()
        .arg("png")
        .write_stdin("flowchart TD\nA --> B\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    png_size(&out);
    let out = sirene()
        .args(["png", "-"])
        .write_stdin("flowchart TD\nA --> B\nlinkStyle 5 color:red\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(": error: "))
        .get_output()
        .stdout
        .clone();
    png_size(&out);
}

#[test]
fn png_failures() {
    sirene()
        .args(["png", "--scale", "0", "-"])
        .write_stdin("flowchart\nA\n")
        .assert()
        .failure()
        .stdout("")
        .stderr("error: scale must be a positive number, not 0\n");
    sirene()
        .args(["png", "-o", "does/not/exist.png", "-"])
        .write_stdin("flowchart\nA\n")
        .assert()
        .failure()
        .stderr(predicate::str::starts_with(
            "error: cannot write does/not/exist.png: ",
        ));
    sirene()
        .args(["png", "does/not/exist.mmd"])
        .assert()
        .failure()
        .stderr(predicate::str::starts_with(
            "error: cannot read does/not/exist.mmd: ",
        ));
}

/// Output never replaces its own source: PNG by default or with `--output`,
/// SVG with `--output`.
#[test]
fn output_does_not_overwrite_the_input() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("diagram.png");
    let text = "flowchart\nA\n";
    std::fs::write(&src, text).unwrap();
    let message = format!(
        "error: refusing to overwrite the input {}; use --output\n",
        src.display()
    );
    let src = src.to_str().unwrap();
    let other = tmp.path().join(".").join("diagram.png");
    let other = other.to_str().unwrap();
    for args in [
        vec!["png", src],
        vec!["png", src, "-o", other],
        vec!["svg", src, "-o", other],
    ] {
        sirene()
            .args(&args)
            .assert()
            .failure()
            .stdout("")
            .stderr(message.clone());
    }
    assert_eq!(std::fs::read_to_string(src).unwrap(), text);
}

#[cfg(not(feature = "gui"))]
#[test]
fn no_gui() {
    sirene()
        .arg("diagram.mmd")
        .assert()
        .failure()
        .stderr("error: built without the GUI; see sirene --help\n");
}

#[test]
fn rejects_unknown_flag() {
    sirene()
        .arg("--bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bogus"));
}
