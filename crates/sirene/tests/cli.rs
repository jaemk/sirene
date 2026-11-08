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

    // -o must name a directory.
    let file = tmp.path().join("file.txt");
    std::fs::write(&file, "").unwrap();
    sirene()
        .arg("svg")
        .arg(&dir)
        .arg("-o")
        .arg(&file)
        .assert()
        .failure()
        .stderr(format!(
            "error: {} is not a directory; a DIR renders into one\n",
            file.display()
        ));
}

#[test]
fn directory_output_collisions() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("a.mermaid"), "flowchart\nA\n").unwrap();
    std::fs::write(dir.join("a.mmd"), "flowchart\nA --> B\n").unwrap();
    std::fs::write(dir.join("b.mmd"), "flowchart\nB\n").unwrap();
    sirene()
        .arg("svg")
        .arg(dir)
        .assert()
        .failure()
        .stderr(format!(
            "error: skipping {}; {} also writes {}\n",
            dir.join("a.mmd").display(),
            dir.join("a.mermaid").display(),
            dir.join("a.svg").display()
        ));
    // a.svg comes from a.mermaid (one node), not a.mmd.
    let svg = std::fs::read_to_string(dir.join("a.svg")).unwrap();
    assert!(!svg.contains(">B<"), "{svg}");
    assert!(dir.join("b.svg").exists());
}

#[cfg(unix)]
#[test]
fn unreadable_directory() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("locked");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads it anyway; there is nothing to check then.
    let readable = std::fs::read_dir(&dir).is_ok();
    let assert = sirene().arg("svg").arg(&dir).assert().failure();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !readable {
        assert.stderr(predicate::str::starts_with(format!(
            "error: cannot read {}: ",
            dir.display()
        )));
    }
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
    // Collect stderr as it comes, so the test can wait for a status line itself.
    let log = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let mut stderr = child.0.stderr.take().unwrap();
    let sink = log.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n @ 1..) = std::io::Read::read(&mut stderr, &mut buf) {
            sink.lock()
                .unwrap()
                .push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    });
    let err = || log.lock().unwrap().clone();
    let svg = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    let wrote_a = format!("wrote {}\n", dir.join("a.svg").display());

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
    // The status line follows the write.
    eventually("the third `wrote a.svg`", || {
        err().matches(&wrote_a).count() >= 3
    });
    assert!(child.0.try_wait().unwrap().is_none(), "watch exited");

    let err = err();
    assert!(
        err.starts_with(&format!("watching {} (Ctrl-C to stop)\n", dir.display())),
        "{err}"
    );
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

/// `check` prints only diagnostics, for files, directories, Markdown blocks, and stdin, and
/// fails when any has an error.
#[test]
fn check() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("good.mmd"), "flowchart\nA --> B\n").unwrap();
    std::fs::write(dir.join("warn.mmd"), "flowchart\nclick Z cb\n").unwrap();
    sirene()
        .arg("check")
        .arg(dir.join("good.mmd"))
        .arg(dir.join("warn.mmd"))
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("warn.mmd:2:7: warning:"));

    // Errors fail the run, and every file is still checked.
    std::fs::write(dir.join("bad.mmd"), "flowchart\nA -->\n").unwrap();
    sirene()
        .arg("check")
        .arg(dir.join("bad.mmd"))
        .arg(dir.join("missing.mmd"))
        .arg(dir.join("warn.mmd"))
        .assert()
        .failure()
        .stdout("")
        .stderr(
            predicate::str::contains("bad.mmd:2:6: error: expected a node after the edge")
                .and(predicate::str::contains("error: cannot read"))
                .and(predicate::str::contains("warn.mmd:2:7: warning:")),
        );
    // A directory: each diagram under it.
    sirene()
        .arg("check")
        .arg(dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad.mmd:2:6: error"));
    let empty = dir.join("empty");
    std::fs::create_dir(&empty).unwrap();
    sirene()
        .arg("check")
        .arg(&empty)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "error: no .mmd or .mermaid files in",
        ));
    // Stdin.
    sirene()
        .args(["check", "-"])
        .write_stdin("flowchart\nA\n")
        .assert()
        .success()
        .stderr("");
    // Markdown: positions are in the Markdown file; other diagram types are skipped.
    let md = dir.join("doc.md");
    std::fs::write(
        &md,
        "# Doc\n\n```mermaid\nflowchart TD\n  A -->\n```\n\n- item\n\n  ```mermaid\n  sequenceDiagram\n  ```\n",
    )
    .unwrap();
    sirene()
        .arg("check")
        .arg(&md)
        .assert()
        .failure()
        .stdout("")
        .stderr(
            predicate::str::contains("doc.md:5:8: error: expected a node after the edge").and(
                predicate::str::contains(
                    "doc.md:11:3: warning: skipping a `sequenceDiagram` diagram",
                ),
            ),
        );
    sirene()
        .arg("check")
        .arg(dir.join("missing.md"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("error: cannot read"));
    // `check` needs a file.
    sirene().arg("check").assert().failure();
}

/// `svg` and `png` on a Markdown file write one image per flowchart block and never change the
/// blocks; `--link` adds a link after each, once.
#[test]
fn markdown_images() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let md = dir.join("doc.md");
    let text = "# Doc\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\n```mermaid\npie\n  \"a\": 1\n```\n\n~~~mermaid\ngraph TD\n  C\n~~~\n";
    std::fs::write(&md, text).unwrap();
    sirene()
        .arg("svg")
        .arg(&md)
        .assert()
        .success()
        .stdout("")
        .stderr(
            predicate::str::contains("doc.md:9:1: warning: skipping a `pie` diagram")
                .and(predicate::str::contains("doc-1.svg"))
                .and(predicate::str::contains("doc-3.svg")),
        );
    let svg = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    assert!(svg("doc-1.svg").contains(">A</text>"));
    assert!(!dir.join("doc-2.svg").exists());
    assert!(svg("doc-3.svg").contains(">C</text>"));
    // Without --link, the Markdown is left alone.
    assert_eq!(std::fs::read_to_string(&md).unwrap(), text);

    // --link adds links after the blocks, and only once.
    for _ in 0..2 {
        sirene()
            .args(["png", "--link", "--quality", "draft"])
            .arg(&md)
            .arg("-o")
            .arg(dir.join("img"))
            .assert()
            .success();
    }
    let linked = std::fs::read_to_string(&md).unwrap();
    assert_eq!(
        linked,
        "# Doc\n\n```mermaid\nflowchart LR\n  A --> B\n```\n![diagram 1](img/doc-1.png)\n\n```mermaid\npie\n  \"a\": 1\n```\n\n~~~mermaid\ngraph TD\n  C\n~~~\n![diagram 3](img/doc-3.png)\n"
    );
    assert!(dir.join("img/doc-1.png").exists());
    // Every mermaid block is still there, unchanged.
    for block in [
        "```mermaid\nflowchart LR\n  A --> B\n```",
        "~~~mermaid\ngraph TD\n  C\n~~~",
    ] {
        assert!(linked.contains(block));
    }

    // Errors are reported at their Markdown positions and fail the run; the image is written.
    std::fs::write(&md, "```mermaid\nflowchart\n  A -->\n```\n").unwrap();
    sirene()
        .arg("svg")
        .arg(&md)
        .assert()
        .failure()
        .stderr(predicate::str::contains("doc.md:3:8: error"));
    assert!(dir.join("doc-1.svg").exists());
    // An output that is a file, a missing file, and no blocks.
    sirene()
        .arg("svg")
        .arg(&md)
        .arg("-o")
        .arg(dir.join("doc-1.svg"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a directory"));
    sirene()
        .arg("svg")
        .arg(dir.join("none.md"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("error: cannot read"));
    std::fs::write(&md, "no diagrams\n").unwrap();
    sirene()
        .arg("svg")
        .arg(&md)
        .assert()
        .success()
        .stderr(predicate::str::contains("no mermaid blocks"));
}

/// Watching a Markdown file renders its blocks again when it changes.
#[test]
fn watch_markdown() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("w.md");
    std::fs::write(&md, "```mermaid\nflowchart\n  One\n```\n").unwrap();
    let child = std::process::Command::new(assert_cmd::cargo::cargo_bin("sirene"))
        .args(["svg", "--watch", "--link"])
        .arg(&md)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let _child = Running(child);
    let svg = || std::fs::read_to_string(tmp.path().join("w-1.svg")).unwrap_or_default();
    eventually("the first image", || svg().contains(">One</text>"));
    eventually("the link", || {
        std::fs::read_to_string(&md)
            .unwrap_or_default()
            .contains("![diagram 1](w-1.svg)")
    });
    std::fs::write(&md, "```mermaid\nflowchart\n  Two\n```\n").unwrap();
    eventually("the second image", || svg().contains(">Two</text>"));
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

/// A mistyped command is an error instead of a window for a file that does not exist (CLI-1).
#[test]
fn suggests_a_misspelled_command() {
    sirene()
        .args(["asci", "diagram.mmd"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(
            "error: no file or command named 'asci'; did you mean 'sirene ascii'? \
             (to open a new file with that name, use './asci')\n",
        );
}

/// Another Mermaid diagram type gets one clear error and no drawing (FLOWCH-16).
#[test]
fn other_diagram_types() {
    sirene()
        .arg("ascii")
        .write_stdin("sequenceDiagram\n  A->>B: hi\n")
        .assert()
        .failure()
        .stderr(
            "<stdin>:1:1: error: `sequenceDiagram` diagrams are not supported; \
             sirene draws flowcharts\n",
        );
}

#[test]
fn rejects_unknown_flag() {
    sirene()
        .arg("--bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bogus"));
}
