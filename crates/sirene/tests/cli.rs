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

/// A directory renders each `.mmd` and `.mermaid` file under it, beside it or under `--output`.
#[test]
fn directories() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("in");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::create_dir_all(dir.join(".hidden")).unwrap();
    std::fs::write(dir.join("a.mmd"), "flowchart\nA --> B\n").unwrap();
    std::fs::write(dir.join("sub/b.mermaid"), "flowchart\nC\n").unwrap();
    std::fs::write(dir.join(".hidden/c.mmd"), "flowchart\nD\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "not a diagram").unwrap();

    sirene()
        .args(["png", "--quality", "draft"])
        .arg(&dir)
        .assert()
        .success()
        .stdout("")
        .stderr("");
    png_size(&std::fs::read(dir.join("a.png")).unwrap());
    png_size(&std::fs::read(dir.join("sub/b.png")).unwrap());
    assert!(!dir.join(".hidden/c.png").exists());
    assert!(!dir.join("notes.png").exists());

    let out = tmp.path().join("out");
    sirene()
        .arg("svg")
        .arg(&dir)
        .arg("-o")
        .arg(&out)
        .assert()
        .success()
        .stdout("");
    for svg in ["a.svg", "sub/b.svg"] {
        let text = std::fs::read_to_string(out.join(svg)).unwrap();
        assert!(text.starts_with("<svg"), "{svg}");
    }

    // An error in one file fails the run but still writes the others.
    std::fs::write(dir.join("a.mmd"), "flowchart\nA\nlinkStyle 4 color:red\n").unwrap();
    sirene()
        .arg("svg")
        .arg(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("a.mmd:3:"));
    assert!(dir.join("sub/b.svg").exists());

    let empty = tmp.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    sirene()
        .arg("svg")
        .arg(&empty)
        .assert()
        .failure()
        .stderr(format!(
            "error: no .mmd or .mermaid files in {}\n",
            empty.display()
        ));
    sirene()
        .arg("ascii")
        .arg(&dir)
        .assert()
        .failure()
        .stderr(format!(
            "error: {} is a directory; ascii reads one file\n",
            dir.display()
        ));
}

#[test]
fn watch_needs_a_path() {
    for cmd in ["ascii", "svg", "png"] {
        sirene()
            .args([cmd, "--watch", "-"])
            .write_stdin("flowchart\nA\n")
            .assert()
            .failure()
            .stderr("error: --watch needs a FILE or directory, not stdin\n");
    }
}

/// Kills the child when dropped, so a failed assertion does not leave it running.
struct Running(std::process::Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Waits up to 30 seconds for `ok`.
fn eventually(what: &str, mut ok: impl FnMut() -> bool) {
    let start = std::time::Instant::now();
    while !ok() {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "timed out waiting for {what}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// `--watch` renders at start, then again whenever a file changes or appears, until killed.
#[test]
fn watch_renders_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("a.mmd"), "flowchart\nA\n").unwrap();
    let child = std::process::Command::new(assert_cmd::cargo::cargo_bin("sirene"))
        .args(["svg", "--watch"])
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = Running(child);
    let svg = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();

    eventually("a.svg", || svg("a.svg").contains(">A</text>"));
    std::fs::write(dir.join("a.mmd"), "flowchart\nA --> Changed\n").unwrap();
    eventually("a.svg to change", || {
        svg("a.svg").contains(">Changed</text>")
    });
    std::fs::create_dir(dir.join("new")).unwrap();
    std::fs::write(dir.join("new/b.mmd"), "flowchart\nB\n").unwrap();
    eventually("new/b.svg", || svg("new/b.svg").contains(">B</text>"));
    // A file with errors is still written, and the error reported.
    std::fs::write(
        dir.join("a.mmd"),
        "flowchart\nBroken\nlinkStyle 4 color:red\n",
    )
    .unwrap();
    eventually("a.svg to change again", || {
        svg("a.svg").contains(">Broken</text>")
    });
    assert!(child.0.try_wait().unwrap().is_none(), "watch exited");

    let _ = child.0.kill();
    let mut err = String::new();
    std::io::Read::read_to_string(child.0.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(
        err.starts_with(&format!("watching {} (Ctrl-C to stop)\n", dir.display())),
        "{err}"
    );
    let a = dir.join("a.svg");
    let wrote_a = format!("wrote {}\n", a.display());
    assert!(err.matches(&wrote_a).count() >= 3, "{err}");
    assert!(
        err.contains(&format!("wrote {}\n", dir.join("new/b.svg").display())),
        "{err}"
    );
    assert!(err.contains("a.mmd:3:"), "{err}");
}

/// Watching one file prints it again on each change.
#[test]
fn watch_a_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("d.mmd");
    let out = tmp.path().join("d.txt");
    std::fs::write(&src, "flowchart\nOne\n").unwrap();
    let child = std::process::Command::new(assert_cmd::cargo::cargo_bin("sirene"))
        .args(["ascii", "-w"])
        .arg(&src)
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let _child = Running(child);
    let text = || std::fs::read_to_string(&out).unwrap_or_default();
    eventually("the first drawing", || text().contains("One"));
    std::fs::write(&src, "flowchart\nTwo\n").unwrap();
    eventually("the second drawing", || text().contains("Two"));
    // A missing file is reported, and drawn again once it is back.
    std::fs::remove_file(&src).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(700));
    std::fs::write(&src, "flowchart\nThree\n").unwrap();
    eventually("the third drawing", || text().contains("Three"));
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
