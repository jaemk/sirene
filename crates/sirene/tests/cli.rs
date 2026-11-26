use assert_cmd::Command;
use predicates::prelude::*;

fn sirene() -> Command {
    let mut command = Command::cargo_bin("sirene").expect("sirene binary is built");
    command.env_remove("SIRENE_LOG").env_remove("RUST_LOG");
    command
}

/// ERRORH-6/7: filters are configurable without altering the rendered output.
#[test]
fn logging_environment_and_verbose_precedence() {
    let source = "flowchart TD\nA --> B\n";
    sirene()
        .arg("text")
        .env("SIRENE_LOG", "debug")
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains("DEBUG sirene: sirene "));
    sirene()
        .arg("text")
        .env("RUST_LOG", "sirene=debug")
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains("reading <stdin>"));
    for quiet in ["", "off"] {
        sirene()
            .arg("text")
            .env("SIRENE_LOG", quiet)
            .env("RUST_LOG", "trace")
            .write_stdin(source)
            .assert()
            .success()
            .stdout(AB)
            .stderr("");
    }
    sirene()
        .args(["-v", "text"])
        .env("SIRENE_LOG", "off,sirene_core=off")
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains("reading <stdin>"))
        .stderr(predicate::str::contains("sirene_core").not());
    sirene()
        .args(["text", "-vv"])
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains("TRACE sirene: lowering"));
    sirene()
        .arg("text")
        .env("SIRENE_LOG", "off,sirene_core::flowchart=debug")
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains("sirene_core::flowchart: parsed"))
        .stderr(predicate::str::contains("reading <stdin>").not());
}

/// ERRORH-8: phase timings print with their target enabled and stay out of other trace output.
#[test]
fn phase_timings_follow_their_target() {
    let source = "flowchart TD\nA --> B\n";
    sirene()
        .arg("text")
        .env("SIRENE_LOG", "off,sirene_core::profile=trace")
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains(
            "sirene_core::profile: phase layout.order ",
        ))
        .stderr(predicate::str::contains(
            "sirene_core::profile: phase ascii ",
        ))
        .stderr(predicate::str::contains("reading <stdin>").not());
    sirene()
        .arg("text")
        .env("SIRENE_LOG", "trace,sirene_core::profile=off")
        .write_stdin(source)
        .assert()
        .success()
        .stdout(AB)
        .stderr(predicate::str::contains("TRACE sirene: lowering"))
        .stderr(predicate::str::contains("sirene_core::profile").not());
}

#[test]
fn logging_rejects_invalid_configuration_without_echoing_it() {
    sirene()
        .arg("ascii")
        .env("SIRENE_LOG", "sirene=secret-value")
        .write_stdin("flowchart\nA\n")
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("invalid log level"))
        .stderr(predicate::str::contains("secret-value").not());
}

/// ERRORH-4: an unreadable entry fails a directory command while readable diagrams proceed.
#[cfg(unix)]
#[test]
fn directory_entry_failures_fail_checks_and_batch_conversion() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("ok.mmd");
    let broken = dir.path().join("broken.mmd");
    std::fs::write(&good, "flowchart\nA\n").unwrap();
    symlink(dir.path().join("missing.mmd"), &broken).unwrap();
    // Hidden entries are intentionally excluded, including inaccessible symlink targets.
    symlink(
        dir.path().join("missing.mmd"),
        dir.path().join(".hidden.mmd"),
    )
    .unwrap();
    for command in ["check", "svg", "png"] {
        sirene()
            .arg(command)
            .arg(dir.path())
            .assert()
            .code(1)
            .stderr(predicate::str::contains(format!(
                "cannot inspect {}",
                broken.display()
            )))
            .stderr(predicate::str::contains(".hidden.mmd").not());
    }
    assert!(good.with_extension("svg").is_file());
    assert!(good.with_extension("png").is_file());
}

/// ERRORH-4: unrelated dangling links do not prevent checking or rendering diagrams.
#[cfg(unix)]
#[test]
fn directory_scan_ignores_unrelated_dangling_symlinks() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("ok.mmd"), "flowchart\nA\n").unwrap();
    symlink("missing", dir.path().join("README")).unwrap();
    for command in ["check", "svg"] {
        sirene().arg(command).arg(dir.path()).assert().success();
    }
    assert!(dir.path().join("ok.svg").is_file());
}

/// ERRORH-3: OS write errors produce failure rather than a panic exit.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn failing_terminal_streams_do_not_panic() {
    use std::process::{Command as Process, Stdio};

    let binary = assert_cmd::cargo::cargo_bin!("sirene");
    let current = env!("CARGO_PKG_VERSION");
    let output = Process::new(binary)
        .args(["update", "--check", "--to", current])
        .env_remove("SIRENE_LOG")
        .env_remove("RUST_LOG")
        .stdout(
            std::fs::File::options()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot write <stdout>"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));

    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty.md");
    std::fs::write(&empty, "# No diagrams\n").unwrap();
    let output = Process::new(binary)
        .arg("check")
        .arg(empty)
        .env_remove("SIRENE_LOG")
        .env_remove("RUST_LOG")
        .stderr(
            std::fs::File::options()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
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
            "Mermaid, PlantUML, and Graphviz DOT diagram editor and viewer",
        ));
}

/// `--verbose` (before or after the command) reports each step on stderr and leaves stdout
/// alone; without it nothing extra is printed.
#[test]
fn verbose_reports_each_step() {
    for args in [["-v", "text"], ["text", "--verbose"]] {
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
            "sirene: read as a flowchart",
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
        .args(["-v", "text", "missing.mmd"])
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
        .arg("text")
        .write_stdin("flowchart TD\nA --> B\n")
        .assert()
        .success()
        .stdout(AB)
        .stderr("");
    sirene()
        .args(["text", "-"])
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
            "+-------+    /-----\\       +------+\n",
            "| Start +--->| Ok? +--yes->| Done |\n",
            "+-------+    \\-----/       +------+\n",
        ));
    sirene()
        .arg("text")
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
        .args(["text", &fixture("accessibility")])
        .assert()
        .success()
        .stdout("┌───┐    ┌───┐\n│ A ├───►│ B │\n└───┘    └───┘\n");
}

/// `ascii` draws plain ASCII and `text` box-drawing characters; `--charset` overrides either.
#[test]
fn ascii_and_text_default_charsets() {
    let ascii = "+---+    +---+\n| A +--->| B |\n+---+    +---+\n";
    let unicode = "┌───┐    ┌───┐\n│ A ├───►│ B │\n└───┘    └───┘\n";
    let source = "flowchart LR\nA --> B\n";
    for (args, want) in [
        (&["ascii"][..], ascii),
        (&["ascii", "--charset", "unicode"], unicode),
        (&["text"], unicode),
        (&["text", "--charset", "ascii"], ascii),
    ] {
        sirene()
            .args(args)
            .write_stdin(source)
            .assert()
            .success()
            .stdout(want)
            .stderr("");
    }
    sirene()
        .args(["text", "--charset", "ebcdic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ebcdic"));
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

fn ascii_stdout(charset: &str, source: &str) -> String {
    let out = sirene()
        .args(["ascii", "--charset", charset])
        .write_stdin(source)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(out).expect("utf-8 output")
}

#[test]
fn ascii_charset_sets_formulas() {
    let sources = [
        "flowchart LR\nA[\"$$\\sqrt{x^2} \\le \\alpha$$\"] --> B\n",
        "sequenceDiagram\nA->>B: $$\\sqrt{x^2} \\le \\alpha$$\n",
    ];
    for source in sources {
        let ascii = ascii_stdout("ascii", source);
        assert!(ascii.is_ascii(), "{ascii}");
        assert!(ascii.contains("\\/x^2 <= alpha"), "{ascii}");
        let unicode = ascii_stdout("unicode", source);
        assert!(unicode.contains("\\/x^2 <= \u{3b1}"), "{unicode}");
        assert!(!unicode.contains('\u{221a}'), "{unicode}");
        let rich = ascii_stdout("unicode-math", source);
        assert!(rich.contains("\u{221a}x\u{b2} \u{2264} \u{3b1}"), "{rich}");
    }
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
        .arg("text")
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
        .code(1)
        .stderr("error: cannot read <stdin>: the text is UTF-16; sirene reads UTF-8\n");
    sirene()
        .arg("ascii")
        .write_stdin(vec![b'p', b'i', b'e', 0xe9])
        .assert()
        .code(1)
        .stderr(
            "error: cannot read <stdin>: the text is not UTF-8 (an invalid byte at offset 3): incomplete utf-8 byte sequence from index 3\n",
        );
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

/// A diagram's own theme and theme variables win over `--theme` (MTHEME-1, MTHEME-3).
#[test]
fn diagram_themes() {
    sirene()
        .args(["svg", "--theme", "dark", "-"])
        .write_stdin("%%{init: {'theme': 'forest'}}%%\nflowchart\nA\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fill=\"#cde498\""))
        .stderr("");
    sirene()
        .args(["svg", "-"])
        .write_stdin(
            "---\nconfig:\n  themeVariables:\n    lineColor: '#ff0000'\n---\nflowchart\nA --> B\n",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("stroke=\"#ff0000\""));
    sirene()
        .args(["svg", "-"])
        .write_stdin("%%{init: {'theme': 'sunset'}}%%\nflowchart\nA\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("not a Mermaid theme"));
    // Chart colors follow the theme too (MTHEME-4).
    sirene()
        .args(["svg", "-"])
        .write_stdin("%%{init: {'theme': 'forest'}}%%\npie\n\"a\" : 2\n\"b\" : 1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fill=\"#cde498\""))
        .stdout(predicate::str::contains("fill=\"#cdffb2\""));
}

/// The CLI writes what `sirene_core` alone renders from the file (WORKSP-10).
#[test]
fn output_matches_the_library() {
    use sirene_core::ascii::Options;
    use sirene_core::diagram::{self, Lang};
    use sirene_core::scene::{Theme, png::Quality};
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for name in [
        "architecture.mmd",
        "build-graph.dot",
        "checkout.puml",
        "ci-pipeline.mmd",
        "fulfillment.puml",
        "login.mmd",
        "order-lifecycle.mmd",
        "request-timing.puml",
        "shapes.mmd",
        "shop-model.puml",
        "styling.mmd",
    ] {
        let path = examples.join(name);
        let src = std::fs::read_to_string(&path).unwrap();
        let lowered = diagram::lower_as(&src, Lang::of_path(&path));
        let file = path.to_str().unwrap();
        let out = |args: &[&str]| {
            let output = sirene().args(args).output().unwrap();
            assert!(output.status.success(), "{name}: {args:?}");
            output.stdout
        };
        assert_eq!(
            out(&["text", file]),
            lowered.ascii(&Options::UNICODE).unwrap().into_bytes(),
            "{name}"
        );
        assert_eq!(
            out(&["svg", "--theme", "dark", file, "-o", "-"]),
            lowered.svg(&Theme::DARK).unwrap().into_bytes(),
            "{name}"
        );
        assert_eq!(
            out(&["png", "--quality", "draft", file, "-o", "-"]),
            lowered.png(&Theme::LIGHT, Quality::Draft, None).unwrap(),
            "{name}"
        );
    }
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

/// `-o -` is stdout, not a file named `-`; it is refused for a Markdown FILE (CLI-5, CLI-6).
#[test]
fn output_dash_is_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("d.mmd");
    std::fs::write(&src, "flowchart\nA\n").unwrap();
    sirene()
        .current_dir(tmp.path())
        .args(["svg", "d.mmd", "-o", "-"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("<svg"));
    let out = sirene()
        .current_dir(tmp.path())
        .args(["png", "d.mmd", "-o", "-"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    png_size(&out);
    assert!(!tmp.path().join("-").exists());
    assert!(!tmp.path().join("d.png").exists());
    std::fs::write(tmp.path().join("g.md"), "```mermaid\nflowchart\nA\n```\n").unwrap();
    sirene()
        .current_dir(tmp.path())
        .args(["svg", "g.md", "-o", "-"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("`--output -` writes one diagram"));
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
            "error: no .mmd, .mermaid, .dot, .gv, .puml, .plantuml, .pu, .iuml, or .vsdx files in {}\n",
            empty.display()
        ));
    sirene()
        .arg("ascii")
        .arg(&dir)
        .assert()
        .failure()
        .stderr(format!(
            "error: {} is a directory; ascii and text read one file\n",
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
fn batch_protects_symlinked_sources_and_resolves_output_directories() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("input");
    std::fs::create_dir(&dir).unwrap();
    let source = "flowchart\nA --> B\n";
    std::fs::write(dir.join("a.svg"), source).unwrap();
    symlink("a.svg", dir.join("a.mmd")).unwrap();
    symlink("a.svg", dir.join("b.mmd")).unwrap();
    std::fs::write(dir.join("c.mmd"), source).unwrap();
    sirene()
        .arg("svg")
        .arg(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("would overwrite input"));
    assert_eq!(std::fs::read_to_string(dir.join("a.svg")).unwrap(), source);
    assert!(dir.join("b.svg").exists());
    assert!(dir.join("c.svg").exists());

    // The output can instead alias a different input's symlink target.
    std::fs::remove_file(dir.join("a.mmd")).unwrap();
    std::fs::write(dir.join("a.mmd"), source).unwrap();
    sirene()
        .arg("svg")
        .arg(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("would overwrite input"));
    assert_eq!(std::fs::read_to_string(dir.join("a.svg")).unwrap(), source);

    let out = tmp.path().join("out");
    let shared = tmp.path().join("shared");
    std::fs::create_dir(&out).unwrap();
    std::fs::create_dir(&shared).unwrap();
    for name in ["x", "y"] {
        std::fs::create_dir(dir.join(name)).unwrap();
        std::fs::write(dir.join(name).join("diagram.mmd"), source).unwrap();
        symlink(&shared, out.join(name)).unwrap();
    }
    sirene()
        .arg("svg")
        .arg(&dir)
        .arg("-o")
        .arg(&out)
        .assert()
        .failure()
        .stderr(predicate::str::contains("also writes"));
    assert!(shared.join("diagram.svg").exists());
}

#[cfg(unix)]
#[test]
fn batch_replaces_output_symlink_without_touching_source() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let source = "flowchart\nA\n";
    std::fs::write(dir.join("a.mmd"), source).unwrap();
    symlink("a.mmd", dir.join("a.svg")).unwrap();
    sirene().arg("svg").arg(dir).assert().success();
    assert_eq!(std::fs::read_to_string(dir.join("a.mmd")).unwrap(), source);
    assert!(
        !std::fs::symlink_metadata(dir.join("a.svg"))
            .unwrap()
            .is_symlink()
    );
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
            "error: no .mmd, .mermaid, .dot, .gv, .puml, .plantuml, .pu, .iuml, or .vsdx files in",
        ));
    // Stdin.
    sirene()
        .args(["check", "-"])
        .write_stdin("flowchart\nA\n")
        .assert()
        .success()
        .stderr("");
    // Markdown: positions are in the Markdown file.
    let md = dir.join("doc.md");
    std::fs::write(
        &md,
        "# Doc\n\n```mermaid\nflowchart TD\n  A -->\n```\n\n- item\n\n  ```mermaid\n  agentflow-beta\n  a ==> b\n  ```\n",
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
                predicate::str::contains("doc.md:12:5: error: `==>` is not an agentflow edge"),
            ),
        );
    sirene()
        .arg("check")
        .arg(dir.join("missing.md"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("error: cannot read"));
    // Lines indented less than the fence are placed by their own indent.
    std::fs::write(&md, "   ```mermaid\n   flowchart TD\n A -->\n```\n").unwrap();
    sirene()
        .arg("check")
        .arg(&md)
        .assert()
        .failure()
        .stderr(predicate::str::contains("doc.md:3:7: error"));
    std::fs::write(&md, "# Doc\n").unwrap();
    sirene()
        .arg("check")
        .arg(&md)
        .assert()
        .success()
        .stderr(predicate::str::contains("doc.md: no diagram blocks"));
    // `check` needs a file.
    sirene().arg("check").assert().failure();
}

/// `--link` with `-o` outside the Markdown file's directory links the images from that
/// directory, not from the working directory.
#[test]
fn markdown_links_from_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::create_dir(dir.join("docs")).unwrap();
    std::fs::write(
        dir.join("docs/guide.md"),
        "```mermaid\nflowchart LR\n  A --> B\n```\n",
    )
    .unwrap();
    sirene()
        .current_dir(dir)
        .args(["svg", "docs/guide.md", "-o", "img", "--link"])
        .assert()
        .success();
    assert!(dir.join("img/guide-1.svg").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("docs/guide.md")).unwrap(),
        "```mermaid\nflowchart LR\n  A --> B\n```\n![diagram 1](../img/guide-1.svg)\n"
    );
}

/// Rendering must not delete a file whose name happens to match the former staging path.
#[test]
fn output_keeps_unrelated_staging_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("diagram.mmd"), "flowchart LR\nA --> B\n").unwrap();
    let staging = dir.join(".out.svg.sirene-tmp");
    std::fs::write(&staging, b"keep this file").unwrap();
    sirene()
        .current_dir(dir)
        .args(["svg", "diagram.mmd", "-o", "out.svg"])
        .assert()
        .success();
    assert_eq!(std::fs::read(&staging).unwrap(), b"keep this file");
    assert!(
        std::fs::read_to_string(dir.join("out.svg"))
            .unwrap()
            .starts_with("<svg")
    );
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 3);
}

/// A generated link must refer to the image's literal filename, not a URI fragment or query.
#[test]
fn markdown_links_encode_filename_delimiters() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let source = "```mermaid\nflowchart LR\nA --> B\n```\n";
    let names = [
        ("guide#v2", "guide%23v2"),
        ("guide%20", "guide%2520"),
        #[cfg(unix)]
        ("guide?x", "guide%3Fx"),
    ];
    for (stem, encoded) in names {
        let md = dir.join(format!("{stem}.md"));
        std::fs::write(&md, source).unwrap();
        sirene()
            .current_dir(dir)
            .arg("svg")
            .arg(&md)
            .arg("--link")
            .assert()
            .success();
        assert!(dir.join(format!("{stem}-1.svg")).is_file());
        let linked = format!("{source}![diagram 1]({encoded}-1.svg)\n");
        assert_eq!(std::fs::read_to_string(&md).unwrap(), linked);
        // Regenerating the encoded link is idempotent too.
        sirene()
            .arg("svg")
            .arg(&md)
            .arg("--link")
            .assert()
            .success();
        assert_eq!(std::fs::read_to_string(&md).unwrap(), linked);
    }
}

/// `ascii` on a Markdown file prints each diagram block, a blank line between them, with
/// diagnostics at Markdown positions.
#[test]
fn ascii_width() {
    // "+---+" is 5 columns.
    let src = "flowchart LR\n  A\n";
    sirene()
        .args(["ascii", "--charset", "ascii", "--width", "5"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("+---+\n| A |\n+---+\n")
        .stderr("");
    sirene()
        .args(["ascii", "--charset", "ascii", "--width", "4"])
        .write_stdin(src)
        .assert()
        .code(1)
        .stdout("+---+\n| A |\n+---+\n")
        .stderr("error: <stdin>: the drawing is 5 columns wide, over --width 4\n");

    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(
        &md,
        "```mermaid\nflowchart LR\n  A\n```\n\n```mermaid\nflowchart LR\n  Long\n```\n",
    )
    .unwrap();
    sirene()
        .args(["ascii", "--charset", "ascii", "--width", "5"])
        .arg(&md)
        .assert()
        .code(1)
        .stdout("+---+\n| A |\n+---+\n\n+------+\n| Long |\n+------+\n")
        .stderr(format!(
            "error: {} block 2: the drawing is 8 columns wide, over --width 5\n",
            md.display()
        ));
}

#[test]
fn check_json() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good.mmd");
    std::fs::write(&good, "flowchart LR\n  A --> B\n").unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(&md, "# \"Doc\"\n").unwrap();
    let missing = tmp.path().join("missing.mmd");
    let out = sirene()
        .args(["check", "--json", "-"])
        .arg(&good)
        .arg(&md)
        .arg(&missing)
        .write_stdin("digraph { b -> }\n")
        .assert()
        .code(1)
        .stderr("")
        .get_output()
        .clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 5, "{stdout}");
    assert_eq!(lines[0], "[");
    assert!(
        lines[1].starts_with(
            r#"  {"file":"<stdin>","line":1,"column":16,"severity":"error","message":"#
        ),
        "{stdout}"
    );
    let json_path = |p: &std::path::Path| p.display().to_string().replace('\\', "\\\\");
    assert_eq!(
        lines[2],
        format!(
            r#"  {{"file":"{}","line":null,"column":null,"severity":"warning","message":"no diagram blocks"}},"#,
            json_path(&md)
        )
    );
    assert!(
        lines[3].starts_with(&format!(
            r#"  {{"file":null,"line":null,"column":null,"severity":"error","message":"cannot read {}"#,
            json_path(&missing)
        )),
        "{stdout}"
    );
    assert_eq!(lines[4], "]");

    sirene()
        .args(["check", "--json"])
        .arg(&good)
        .assert()
        .success()
        .stdout("[]\n")
        .stderr("");
}

#[test]
fn markdown_stdin() {
    let md = "# Doc\n\n```mermaid\nflowchart LR\n  A\n```\n\n```dot\ndigraph { b -> }\n```\n";
    let out = sirene()
        .args(["ascii", "--markdown", "--charset", "ascii"])
        .write_stdin(md)
        .assert()
        .failure()
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "+---+\n| A |\n+---+\n\n.---.\n( b )\n'---'\n"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.starts_with("<stdin>:9:"), "{stderr}");

    // `check` reads `-` and a file not named `.md` as Markdown.
    let out = sirene()
        .args(["check", "--markdown", "-"])
        .write_stdin(md)
        .assert()
        .failure()
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.starts_with("<stdin>:9:"), "{stderr}");
    let tmp = tempfile::tempdir().unwrap();
    let notes = tmp.path().join("notes.txt");
    std::fs::write(&notes, "```mermaid\nflowchart LR\n  A\n```\n").unwrap();
    sirene()
        .args(["check", "--markdown"])
        .arg(&notes)
        .assert()
        .success()
        .stderr("");
    sirene()
        .args(["ascii", "--markdown", "--watch"])
        .assert()
        .failure()
        .stderr("error: --watch needs a FILE or directory, not stdin\n");
}

#[test]
fn markdown_ascii() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(
        &md,
        "# Doc\n\n```mermaid\nflowchart LR\n  A\n```\n\n```dot\ndigraph { b -> }\n```\n",
    )
    .unwrap();
    let out = sirene()
        .args(["ascii", "--charset", "ascii"])
        .arg(&md)
        .assert()
        .failure()
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "+---+\n| A |\n+---+\n\n.---.\n( b )\n'---'\n"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.starts_with(&format!("{}:9:", md.display())),
        "{stderr}"
    );
    std::fs::write(&md, "# Doc\n").unwrap();
    sirene()
        .arg("ascii")
        .arg(&md)
        .assert()
        .success()
        .stdout("")
        .stderr(predicates::str::contains("no diagram blocks"));
}

/// `svg` and `png` on a Markdown file write one image per diagram block and never change the
/// blocks; `--link` adds a link after each, once.
#[test]
fn markdown_images() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let md = dir.join("doc.md");
    let text = "# Doc\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\n```mermaid\nagentflow-beta\n  flow f\n    X\n  end\n```\n\n~~~mermaid\ngraph TD\n  C\n~~~\n";
    std::fs::write(&md, text).unwrap();
    sirene()
        .arg("svg")
        .arg(&md)
        .assert()
        .success()
        .stdout("")
        .stderr(
            predicate::str::contains("doc-1.svg")
                .and(predicate::str::contains("doc-2.svg"))
                .and(predicate::str::contains("doc-3.svg")),
        );
    let svg = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    assert!(svg("doc-1.svg").contains(">A</text>"));
    assert!(svg("doc-2.svg").contains(">X</text>"));
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
        "# Doc\n\n```mermaid\nflowchart LR\n  A --> B\n```\n![diagram 1](img/doc-1.png)\n\n```mermaid\nagentflow-beta\n  flow f\n    X\n  end\n```\n![diagram 2](img/doc-2.png)\n\n~~~mermaid\ngraph TD\n  C\n~~~\n![diagram 3](img/doc-3.png)\n"
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
        .stderr(predicate::str::contains("no diagram blocks"));
    // A directory named like a Markdown file is a directory.
    let named = dir.join("site.md");
    std::fs::create_dir(&named).unwrap();
    std::fs::write(named.join("a.mmd"), "flowchart\n  In\n").unwrap();
    sirene().arg("svg").arg(&named).assert().success();
    assert!(svg("site.md/a.svg").contains(">In</text>"));
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

    // A missing file is reported on the first pass.
    let log = tmp.path().join("log.txt");
    let child = std::process::Command::new(assert_cmd::cargo::cargo_bin("sirene"))
        .args(["svg", "--watch"])
        .arg(tmp.path().join("missing.md"))
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(&log).unwrap())
        .spawn()
        .unwrap();
    let _child = Running(child);
    eventually("the read error", || {
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("error: cannot read")
    });
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

/// DOT draws from stdin, from `.dot` and `.gv` files in a directory, and from `dot` blocks in
/// Markdown (LANGS-1, LANGS-4).
#[test]
fn dot_graphs() {
    sirene()
        .arg("ascii")
        .write_stdin("digraph { rankdir=LR; fetch -> build }\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fetch").and(predicate::str::contains("build")))
        .stderr("");
    sirene()
        .args(["svg", "-"])
        .write_stdin("digraph { bgcolor=navy; a [style=filled, fillcolor=gold] }\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fill=\"#ffd700\""))
        .stdout(predicate::str::contains("fill=\"#000080\""));
    sirene()
        .args(["check", "-"])
        .write_stdin("graph { a -> b }\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("`->` in a `graph`: expected `--`"));

    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.dot"), "digraph { a -> b }\n").unwrap();
    std::fs::write(tmp.path().join("b.gv"), "graph { c -- d }\n").unwrap();
    let out = tmp.path().join("out");
    sirene()
        .arg("svg")
        .arg(tmp.path())
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(out.join("a.svg").is_file());
    assert!(out.join("b.svg").is_file());

    let md = tmp.path().join("doc.md");
    std::fs::write(&md, "# Doc\n\n```dot\ndigraph { x -> y }\n```\n").unwrap();
    sirene().arg("check").arg(&md).assert().success().stderr("");
}

/// The extension, the Markdown block's name, and `--lang` pick the language (DOT-5).
#[test]
fn forced_languages() {
    let not_dot = "flowchart LR\n  a --> b\n";
    let missing_header = "expected `graph` or `digraph`";
    let tmp = tempfile::tempdir().unwrap();
    let gv = tmp.path().join("a.gv");
    std::fs::write(&gv, not_dot).unwrap();
    sirene()
        .arg("check")
        .arg(&gv)
        .assert()
        .failure()
        .stderr(predicate::str::contains(missing_header));
    // `--lang` overrides the extension, before or after the command.
    sirene()
        .args(["check", "--lang", "mermaid"])
        .arg(&gv)
        .assert()
        .success();
    sirene()
        .args(["--lang", "dot", "check", "-"])
        .write_stdin(not_dot)
        .assert()
        .failure()
        .stderr(predicate::str::contains(missing_header));
    sirene()
        .args(["check", "-"])
        .write_stdin(not_dot)
        .assert()
        .success();

    let md = tmp.path().join("doc.md");
    std::fs::write(
        &md,
        "```graphviz\nflowchart LR\n```\n\n```mermaid\nflowchart LR\n```\n",
    )
    .unwrap();
    sirene()
        .arg("check")
        .arg(&md)
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!(
            "doc.md:2:1: error: {missing_header}"
        )));

    // A DOT-looking source in a `.mmd` file is read as Mermaid.
    let mmd = tmp.path().join("b.mmd");
    std::fs::write(&mmd, "digraph { x -> y }\n").unwrap();
    let svg = tmp.path().join("b.svg");
    sirene()
        .arg("svg")
        .arg(&mmd)
        .arg("-o")
        .arg(&svg)
        .assert()
        .failure()
        .stderr(predicate::str::contains("b.mmd:1:1: error:"));
    let detected = tmp.path().join("c.svg");
    sirene()
        .args(["svg", "-o"])
        .arg(&detected)
        .write_stdin("digraph { x -> y }\n")
        .assert()
        .success();
    assert_ne!(
        std::fs::read_to_string(&svg).unwrap(),
        std::fs::read_to_string(&detected).unwrap()
    );
}

/// PlantUML sequence diagrams are found by extension, `@startuml`, Markdown block name, and
/// `--lang plantuml` (PSEQ-7).
#[test]
fn plantuml_sequences() {
    let src = "@startuml\nAlice -> Bob : hello\n@enduml\n";
    sirene()
        .args(["ascii", "-"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("Alice"));
    // Without `@startuml`, `--lang` reads the whole source as the diagram.
    sirene()
        .args(["--lang", "plantuml", "check", "-"])
        .write_stdin("Alice -> Bob : hello\n")
        .assert()
        .success()
        .stderr("");
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.puml"), src).unwrap();
    std::fs::write(tmp.path().join("b.pu"), "@startsalt\n{\n}\n@endsalt\n").unwrap();
    sirene()
        .arg("check")
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "b.pu:1:1: error: `@startsalt` diagrams are not supported yet",
        ));
    let md = tmp.path().join("doc.md");
    std::fs::write(
        &md,
        format!("# Doc\n\n```plantuml\n{src}```\n\n```puml\nA -> B\n```\n"),
    )
    .unwrap();
    sirene().arg("check").arg(&md).assert().success().stderr("");
}

/// An agentflow diagram draws in text (MAGF-1).
#[test]
fn agentflow_draws() {
    sirene()
        .arg("ascii")
        .write_stdin("agentflow-beta\n  flow f[\"Plan\"]\n    a --> b\n  end\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Plan"))
        .stderr("");
}

/// A ZenUML diagram draws in text and as an image (MZEN-3, LANGS-4).
#[test]
fn zenuml_diagrams() {
    let src = "zenuml\nA.get() {\n  return x\n}\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "     +---+\n     | A |\n     +-+-+\n       |\n get() |\n------>|\n       |\n   \
             x   |\n <.....+\n       |\n     +-+-+\n     | A |\n     +---+\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("get()</text>"));
}

/// An info diagram draws sirene's version in text and as an image (MINFO-3, MINFO-4).
#[test]
fn info_diagrams() {
    let version = format!("sirene v{}", env!("CARGO_PKG_VERSION"));
    sirene()
        .arg("ascii")
        .write_stdin("info\n")
        .assert()
        .success()
        .stdout(format!("{version}\n"));
    sirene()
        .arg("svg")
        .write_stdin("info showInfo\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("{version}</text>")));
}

/// An ishikawa diagram draws in text and as an image (MISH-3, MISH-4).
#[test]
fn ishikawa_diagrams() {
    let src = "ishikawa-beta\n  Late\n  People\n    Short staffed\n  Methods\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("Late\n+-- People\n|   `-- Short staffed\n`-- Methods\n");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("Short staffed</text>"));
}

/// A venn diagram draws in text and as an image (MVENN-4, MVENN-5).
#[test]
fn venn_diagrams() {
    let src = "venn-beta\n  set A[\"Alpha\"]\n    text a1\n  set B\n  union A,B[\"Both\"]\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("Alpha\n  - a1\nB\nAlpha & B: Both\n");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("Both</text>"));
}

/// A Wardley map draws in text and as an image (MWARD-4, MWARD-5).
#[test]
fn wardley_maps() {
    let src = "wardley-beta\ncomponent Tea [0.5, 0.5]\ncomponent Cup [0.4, 0.8]\nTea -> Cup\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("* Tea"))
        .stdout(predicate::str::ends_with("Links:\n  Tea -- Cup\n"));
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("Evolution</text>"));
}

/// A tree view draws in text and as an image (MTREEV-4, MTREEV-5).
#[test]
fn tree_views() {
    let src = "treeView-beta\n  src/\n    a.ts ## entry\n  b\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("/\n|-- src/\n|   `-- a.ts  entry\n`-- b\n");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("entry</text>"));
}

/// A use case diagram draws in text and as an image (MUSE-8).
#[test]
fn use_case_diagrams() {
    let src = "usecase-beta\nactor User\nUser --> Login(\"Sign in\")\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("User").and(predicate::str::contains("Sign in")));
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("Sign in</text>"));
}

/// A railroad diagram draws in text and as an image (MRAIL-3, MRAIL-4).
#[test]
fn railroad_diagrams() {
    let src = "railroad-ebnf-beta\nsign = \"+\" | \"-\" ;\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "           .---.\n\
             sign = *-+-+ + +-+-*\n\
             \x20        | '---' |\n\
             \x20        | .---. |\n\
             \x20        +-+ - +-+\n\
             \x20          '---'\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("sign =</text>"));
}

/// A C4 diagram draws in text and as an image (MC4-4, LANGS-4).
#[test]
fn c4_diagrams() {
    let src = "C4Context\nPerson(a, A)\nSystem(b, B)\nRel(a, b, Uses)\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            ".------------.  .------------.\n\
             | <<person>> |  | <<system>> |\n\
             |     A      |  |     B      |\n\
             '------------'  '------------'\n\
             \n\
             a --> b: Uses\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("&lt;&lt;person&gt;&gt;</text>"));
}

/// An architecture diagram draws in text and as an image (MARCH-4, LANGS-4).
#[test]
fn architecture_diagrams() {
    let src = "architecture-beta\nservice a(server)[A]\nservice b[B]\na:R --> L:b\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "+------------+      +---+\n| A (server) +----->| B |\n+------------+      +---+\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">A</text>"));
}

/// A block diagram draws in text and as an image (MBLOCK-4, LANGS-4).
#[test]
fn block_diagrams() {
    let src = "block-beta\na b\na --> b\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("+---+  +---+\n| a |  | b |\n+---+  +---+\n\na --> b\n");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">a</text>"));
}

/// A treemap draws in text and as an image (MTREE-4, LANGS-4).
#[test]
fn treemaps() {
    let src = "treemap-beta\n\"S\"\n  \"a\": 3\n  \"b\": 1\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(format!(
            "S     4  {}\n+- a  3  {}\n`- b  1  {}\n",
            "#".repeat(24),
            "#".repeat(18),
            "#".repeat(6)
        ));
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">S</text>"));
}

/// A sankey diagram draws in text and as an image (MSANKEY-4, LANGS-4).
#[test]
fn sankey_diagrams() {
    let src = "sankey\nA,B,2\nA,C,1\n";
    let full = "#".repeat(24);
    let half = "#".repeat(12);
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(format!(
            "A  3\n+- B  2  {full}\n`- C  1  {half}\n\nB  2\n\nC  1\n"
        ));
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">A 3</text>"));
}

/// A radar chart draws in text and as an image (MRADAR-4, LANGS-4).
#[test]
fn radar_charts() {
    let src = "radar-beta\n  axis a[\"Speed\"], b\n  curve c[\"Car\"]{4, 2}\n  max 4\n";
    let full = "#".repeat(24);
    let half = format!("{}{}", "#".repeat(12), " ".repeat(12));
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(format!(
            "Speed  Car  {full} 4\nb      Car  {half} 2\n\n0 to 4\n"
        ));
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">Speed</text>"));
}

/// A packet diagram draws in text and as an image (MPKT-4, LANGS-4).
#[test]
fn packet_diagrams() {
    let src =
        "---\nconfig:\n  packet:\n    bitsPerRow: 4\n---\npacket\n  0-2: \"abc\"\n  +3: \"d\"\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("   0\n   0 1 2 3\n  +-----+-+\n0 | abc |d|\n  +---+-+-+\n4 | d |\n  +---+\n");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">abc</text>"));
}

/// A kanban board draws in text and as an image (MKAN-4, LANGS-4).
#[test]
fn kanban_boards() {
    let src =
        "kanban\n  todo[To do]\n    a[Write docs]@{ ticket: T-1, priority: High }\n  done[Done]\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "+- To do ----------+ +- Done -----------+\n\
             | .--------------. | |                  |\n\
             | | Write docs   | | |                  |\n\
             | | ^ High T-1   | | |                  |\n\
             | '--------------' | |                  |\n\
             +------------------+ +------------------+\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains(">Write docs</text>"));
}

/// A requirement diagram draws in text and as an image (MREQ-3, LANGS-4).
#[test]
fn requirement_diagrams() {
    let src = "requirementDiagram\n  element e {\n    type: t\n  }\n  requirement r {\n    id: 1\n  }\n  e - satisfies -> r\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "  +-------------+\n  | <<Element>> |\n  |      e      |\n  +-------------+\n  \
             | Type: t     |\n  +------+------+\n         :\n   <<satisfies>>\n         v\n\
             +-----------------+\n| <<Requirement>> |\n|        r        |\n\
             +-----------------+\n| Id: 1           |\n+-----------------+\n",
        );
    // The SVG keeps the guillemets.
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicate::str::contains("«Element»"));
}

/// An XY chart draws in text and as an image (MXY-4, LANGS-4).
#[test]
fn xy_charts() {
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin("xychart-beta\n  x-axis [a, b]\n  bar [1, 3]\n")
        .assert()
        .success()
        .stdout(
            "   bar 1\na                           1\nb  ######################## 3\n\n1 to 3\n",
        );
    sirene()
        .arg("svg")
        .write_stdin("xychart-beta\n  x-axis [a, b]\n  bar [1, 3]\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(">a</text>"));
}

/// A quadrant chart draws in text and as an image (MQUAD-4, LANGS-4).
#[test]
fn quadrant_charts() {
    sirene()
        .arg("text")
        .write_stdin("quadrantChart\n  quadrant-1 Do\n  A: [0.75, 0.75]\n")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("│         Do         │").and(predicate::str::contains("● A")),
        );
    sirene()
        .arg("svg")
        .write_stdin("quadrantChart\n  A: [0.75, 0.75]\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(">A</text>"));
}

/// A user journey draws in text and as an image (MJOUR-4, LANGS-4).
#[test]
fn journeys() {
    let src = "journey\n  section Work\n    Tea: 5: Me\n    Mail: 2: Me, Cat\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("Work\n  Tea   ***** 5  Me\n  Mail  **... 2  Me, Cat\n")
        .stderr("");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            predicate::str::contains(">Tea</text>").and(predicate::str::contains(">Cat</text>")),
        );
}

/// A mindmap draws in text and as an image (MMIND-4, LANGS-4).
#[test]
fn mindmaps() {
    let src = "mindmap\n  root((Root))\n    a[A]\n      a1\n    b\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("((Root))\n+-- [A]\n|   `-- a1\n`-- b\n")
        .stderr("");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            predicate::str::contains(">Root</text>").and(predicate::str::contains(">a1</text>")),
        );
}

/// A gantt chart draws in text and as an image (MGANTT-6, LANGS-4).
#[test]
fn gantt_charts() {
    let src = "gantt\n  dateFormat YYYY-MM-DD\n  tickInterval 1day\n  axisFormat %d\n  todayMarker off\n  \
               a : 2014-01-01, 2d\n  b : crit, 1d\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "a    ########################################\n\
             ! b                                          ####################\n\
             \x20    +-------------------+-------------------+-------------------+\n\
             \x20    01                  02                  03                  04\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicates::str::contains(">a</text>"));
    sirene()
        .arg("check")
        .arg("-")
        .write_stdin("gantt\n  a : after b, 1d\n")
        .assert()
        .failure()
        .stderr("<stdin>:2:3: error: no task has the id `b`\n");
}

/// A pie chart draws in text and as an image (MPIE-4, LANGS-4).
#[test]
fn pie_charts() {
    let src = "pie showData\n  \"a\" : 3\n  \"b\" : 1\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "a  ##############################             75%  3\n\
             b  ##########                                 25%  1\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicates::str::contains(">a [3]</text>"));
    sirene()
        .arg("check")
        .arg("-")
        .write_stdin("pie\n  \"a\" : -1\n")
        .assert()
        .failure()
        .stderr("<stdin>:2:9: error: `a` has a negative value; a slice must be 0 or more\n");
}

/// A sequence diagram draws in text and as an image (MSEQ-4, LANGS-4).
#[test]
fn sequence_diagrams() {
    let src = "sequenceDiagram\n  A->>B: hi\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "+---+   +---+\n\
             | A |   | B |\n\
             +-+-+   +-+-+\n\
             \x20 |       |\n\
             \x20 |  hi   |\n\
             \x20 +------>|\n\
             \x20 |       |\n\
             +-+-+   +-+-+\n\
             | A |   | B |\n\
             +---+   +---+\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicates::str::contains(">hi</text>"));
    sirene()
        .arg("check")
        .arg("-")
        .write_stdin("sequenceDiagram\n  loop x\n")
        .assert()
        .failure()
        .stderr("<stdin>:2:3: error: unclosed `loop`: expected `end`\n");
}

/// A git graph draws in text and as an image (MGIT-5, LANGS-4).
#[test]
fn git_graphs() {
    let src = "gitGraph\n  commit id: \"a\"\n  branch dev\n  commit id: \"b\"\n  checkout main\n  \
               merge dev id: \"m\"\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("main\n| dev\n* |  a\n+-*  b\n@-+  m\n");
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicates::str::contains(">dev</text>"));
    sirene()
        .arg("check")
        .arg("-")
        .write_stdin("gitGraph\n  merge main\n")
        .assert()
        .failure()
        .stderr("<stdin>:2:9: error: cannot merge `main` into itself\n");
}

/// A timeline draws in text and as an image (TIMELI-4, LANGS-4).
#[test]
fn timelines() {
    let src = "timeline\n  2002 : A\n  2004 : B\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(
            "+------+ +------+\n\
             | 2002 | | 2004 |\n\
             +---+--+ +---+--+\n\
             ----+--------+---->\n\
             +---+--+ +---+--+\n\
             |  A   | |  B   |\n\
             +------+ +------+\n",
        );
    sirene()
        .arg("svg")
        .write_stdin(src)
        .assert()
        .success()
        .stdout(predicates::str::contains(">2004</text>"));
}

/// A state diagram draws from stdin, a file, and a Markdown block (MSTATE-1, LANGS-4).
#[test]
fn state_diagrams() {
    let src = "stateDiagram-v2\n  [*] --> Still\n  Still --> Moving : push\n  Moving --> [*]\n";
    sirene()
        .args(["ascii", "--charset", "ascii"])
        .write_stdin(src)
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::str::contains("| Still |").and(predicate::str::contains("push")));
    sirene()
        .arg("check")
        .arg("-")
        .write_stdin("stateDiagram-v2\n  click a \"x\"\n")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "<stdin>:2:3: warning: `click` is not supported in state diagrams",
        ));
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(&md, format!("```mermaid\n{src}```\n")).unwrap();
    sirene().arg("svg").arg(&md).assert().success();
    let svg = std::fs::read_to_string(tmp.path().join("doc-1.svg")).unwrap();
    assert!(svg.contains(">Still<"), "{svg}");
}

/// `update` decisions with `--to` make no network request (UPDATE-1, UPDATE-3, UPDATE-6).
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn update() {
    let current = env!("CARGO_PKG_VERSION");
    let target = predicate::str::is_match(
        r"\(target (x86_64|aarch64)-(apple-darwin|unknown-linux-(gnu|musl))\)",
    )
    .unwrap();
    sirene()
        .args(["update", "--check", "--to", "v99.0.0"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(format!(
            "99.0.0 is newer than sirene {current} (target "
        )))
        .stdout(predicate::str::contains("release availability not checked"))
        .stdout(target.clone());
    sirene()
        .args(["update", "--check", "--to", current])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(format!(
            "sirene {current} is up to date"
        )))
        .stdout(target);
    sirene()
        .args(["update", "--check", "--to", "0.0.1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!(
            "sirene {current} is newer than 0.0.1; not downgrading"
        )));
    sirene()
        .args(["update", "--to", "../0.2.0"])
        .assert()
        .failure()
        .stderr("error: --to '../0.2.0' is not a version like 0.2.0\n");
    // Without a terminal, only --yes updates.
    sirene()
        .args(["update", "--to", "99.0.0"])
        .assert()
        .failure()
        .stdout("")
        .stderr("error: not a terminal; pass --yes to update without asking\n");
}

#[test]
fn rejects_unknown_flag() {
    sirene()
        .arg("--bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bogus"));
}
