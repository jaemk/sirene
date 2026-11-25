//! TeX files and the formulas of Markdown files through the CLI (LEMB-3): `svg`, `ascii`, and
//! `check` read a `.tex` file as one equation and a Markdown file's formulas as blocks beside its
//! diagrams.

use assert_cmd::Command;
use predicates::prelude::*;

fn sirene() -> Command {
    let mut command = Command::cargo_bin("sirene").expect("sirene binary is built");
    command.env_remove("SIRENE_LOG").env_remove("RUST_LOG");
    command
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// `sirene svg PATH`'s output, which may be a failed run's.
fn svg_of(path: &std::path::Path) -> String {
    let out = sirene().arg("svg").arg(path).output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

const DOCUMENT_ERROR: &str = "error: full LaTeX documents are not supported; give one formula";

#[test]
fn a_tex_file_is_one_equation() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    for name in ["a.tex", "b.latex", "C.TEX"] {
        std::fs::write(dir.join(name), "$$ \\frac{a}{b} + x^2 $$\n").unwrap();
    }
    sirene()
        .arg("svg")
        .arg(dir.join("a.tex"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("<svg"));
    let svg = svg_of(&dir.join("a.tex"));
    assert!(svg.contains("<path") && !svg.contains("<text"), "{svg}");
    for name in ["b.latex", "C.TEX"] {
        assert_eq!(svg_of(&dir.join(name)), svg, "{name}");
    }
    // To a file.
    sirene()
        .arg("svg")
        .arg(dir.join("a.tex"))
        .arg("-o")
        .arg(dir.join("a.svg"))
        .assert()
        .success();
    assert_eq!(read(&dir.join("a.svg")), svg);
    // Text, and the formula is not drawn as its source.
    sirene()
        .arg("ascii")
        .arg(dir.join("a.tex"))
        .assert()
        .success()
        .stdout(predicate::str::contains("a").and(predicate::str::contains("frac").not()))
        .stderr("");
    sirene()
        .arg("check")
        .arg(dir.join("a.tex"))
        .assert()
        .success()
        .stderr("");
    sirene()
        .args(["png", "--quality", "draft"])
        .arg(dir.join("a.tex"))
        .assert()
        .success();
    assert!(dir.join("a.png").exists());
}

#[test]
fn every_wrapper_is_read() {
    let tmp = tempfile::tempdir().unwrap();
    let bare = {
        let path = tmp.path().join("bare.tex");
        std::fs::write(&path, "x^2\n").unwrap();
        svg_of(&path)
    };
    for (i, src) in [
        "$$x^2$$\n",
        "$x^2$\n",
        "\\[x^2\\]\n",
        "\\(x^2\\)\n",
        "\\begin{math}x^2\\end{math}\n",
        "\\begin{displaymath}\n  x^2\n\\end{displaymath}\n",
        "% a comment\n$$ x^2 $$ % another\n",
        "\\documentclass{article}\n\\begin{document}\n\\[ x^2 \\]\n\\end{document}\n",
    ]
    .iter()
    .enumerate()
    {
        let path = tmp.path().join(format!("w{i}.tex"));
        std::fs::write(&path, src).unwrap();
        sirene()
            .arg("check")
            .arg(&path)
            .assert()
            .success()
            .stderr("");
        assert_eq!(svg_of(&path), bare, "{src:?}");
    }
}

#[test]
fn errors_are_reported_where_they_are() {
    let tmp = tempfile::tempdir().unwrap();
    let tex = tmp.path().join("bad.tex");
    std::fs::write(&tex, "% c\n$$\n  1 + \\frobnicate\n$$\n").unwrap();
    sirene()
        .arg("check")
        .arg(&tex)
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad.tex:3:7: error:"));
    // The source is drawn and the one error reported.
    sirene()
        .arg("svg")
        .arg(&tex)
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad.tex:3:7: error:"))
        .stdout(predicate::str::contains("frobnicate"));
    sirene()
        .arg("ascii")
        .arg(&tex)
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad.tex:3:7: error:"))
        .stdout(predicate::str::contains("frobnicate"));

    // A full document is refused on its first line.
    let doc = tmp.path().join("doc.tex");
    std::fs::write(
        &doc,
        "\\documentclass{article}\n\\begin{document}\nHello $x$.\n\\end{document}\n",
    )
    .unwrap();
    sirene()
        .arg("check")
        .arg(&doc)
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!(
            "doc.tex:1:1: {DOCUMENT_ERROR}"
        )));
    // `--json` has the same position.
    sirene()
        .args(["check", "--json"])
        .arg(&doc)
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("\"line\":1,\"column\":1").and(predicate::str::contains(
                "full LaTeX documents are not supported",
            )),
        );
}

#[test]
fn lang_latex_reads_stdin_and_overrides_the_extension() {
    for lang in ["latex", "tex"] {
        sirene()
            .args(["--lang", lang, "svg", "-o", "-"])
            .write_stdin("\\sqrt{2}\n")
            .assert()
            .success()
            .stdout(predicate::str::contains("<path"));
        sirene()
            .args(["check", "--lang", lang, "-"])
            .write_stdin("\\sqrt{2}\n")
            .assert()
            .success();
    }
    sirene()
        .args(["--lang", "latex", "check", "-"])
        .write_stdin("\\frobnicate\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("1:1: error:"));
    // Without it, stdin is detected, and TeX is not detected.
    sirene()
        .args(["svg", "-o", "-"])
        .write_stdin("x^2\n")
        .assert()
        .stdout(predicate::str::contains("<text"));
    // `--lang` wins over a `.tex` name.
    let tmp = tempfile::tempdir().unwrap();
    let tex = tmp.path().join("a.tex");
    std::fs::write(&tex, "flowchart LR\n  P --> Q\n").unwrap();
    sirene()
        .args(["--lang", "mermaid", "svg"])
        .arg(&tex)
        .assert()
        .success()
        .stdout(predicate::str::contains(">P</text>"));
}

#[test]
fn directories_do_not_pick_up_tex_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("in");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("paper.tex"), "\\documentclass{article}\n").unwrap();
    sirene()
        .arg("check")
        .arg(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no .mmd, .mermaid"));
    std::fs::write(dir.join("a.mmd"), "flowchart\n  A\n").unwrap();
    sirene()
        .arg("check")
        .arg(&dir)
        .assert()
        .success()
        .stderr("");
    let out = tmp.path().join("out");
    sirene()
        .arg("svg")
        .arg(&dir)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(out.join("a.svg").exists());
    assert!(!out.join("paper.svg").exists());
}

const MD: &str = "# Doc\n\nInline $x^2$ here.\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\n$$\n\\frac{a}{b}\n$$\n\n```math\ny_1\n```\n";

#[test]
fn markdown_formulas_are_blocks_beside_the_diagrams() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let md = dir.join("doc.md");
    std::fs::write(&md, MD).unwrap();
    sirene()
        .arg("svg")
        .arg(&md)
        .assert()
        .success()
        .stdout("")
        .stderr(
            predicate::str::contains("doc-1.svg")
                .and(predicate::str::contains("doc-2.svg"))
                .and(predicate::str::contains("doc-3.svg"))
                .and(predicate::str::contains("doc-4.svg")),
        );
    // The numbers count every block in source order: formula, diagram, formula, formula.
    let svg = |n: usize| read(&dir.join(format!("doc-{n}.svg")));
    for n in [1, 3, 4] {
        assert!(svg(n).contains("<path") && !svg(n).contains("<text"), "{n}");
    }
    assert!(svg(2).contains(">A</text>"));
    assert!(!dir.join("doc-5.svg").exists());
    // An inline formula is set smaller than the same one in display style.
    let height = |s: &str| -> f32 {
        let at = s.find("height=\"").unwrap() + 8;
        s[at..].split('"').next().unwrap().parse().unwrap()
    };
    let display = tmp.path().join("display.md");
    std::fs::write(&display, "$$\\sum_{i=0}^n i$$\n\n$\\sum_{i=0}^n i$\n").unwrap();
    sirene().arg("svg").arg(&display).assert().success();
    let (d, i) = (
        read(&dir.join("display-1.svg")),
        read(&dir.join("display-2.svg")),
    );
    assert!(height(&d) > height(&i), "{} {}", height(&d), height(&i));
    // The Markdown is left alone.
    assert_eq!(read(&md), MD);
}

#[test]
fn markdown_links_follow_diagram_blocks_only() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(&md, MD).unwrap();
    for _ in 0..2 {
        sirene().args(["svg", "--link"]).arg(&md).assert().success();
    }
    let linked = read(&md);
    assert_eq!(
        linked,
        MD.replace("```\n\n$$\n", "```\n![diagram 2](doc-2.svg)\n\n$$\n")
    );
}

#[test]
fn markdown_ascii_prints_each_block() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(&md, MD).unwrap();
    let out = sirene()
        .args(["ascii", "--charset", "ascii"])
        .arg(&md)
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    // Four blocks, a blank line between each, the diagram among them.
    assert!(out.contains("| A "), "{out}");
    assert!(!out.contains("\\frac") && !out.contains('$'), "{out}");
    let blocks: Vec<&str> = out.split("\n\n").filter(|b| !b.trim().is_empty()).collect();
    assert!(blocks.len() >= 4, "{out}");
    // Only formulas: still one block each, and no "no diagram blocks" warning.
    std::fs::write(&md, "$x$ and $$y$$\n").unwrap();
    sirene()
        .arg("ascii")
        .arg(&md)
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::str::contains("x").and(predicate::str::contains("y")));
}

#[test]
fn markdown_check_positions_errors_in_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(
        &md,
        "# Doc\n\nGood $x$ and bad $a^1^2$.\n\n> $$\n> \\frobnicate\n> $$\n\n- item\n\n  ```tex\n  \\undefined{x}\n  ```\n\n$5 and $6 are prices.\n",
    )
    .unwrap();
    let out = sirene()
        .arg("check")
        .arg(&md)
        .assert()
        .failure()
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr).unwrap();
    let name = md.display();
    assert_eq!(
        stderr.lines().filter(|l| l.contains("error:")).count(),
        3,
        "{stderr}"
    );
    assert!(stderr.contains(&format!("{name}:3:22: error:")), "{stderr}");
    assert!(stderr.contains(&format!("{name}:6:3: error:")), "{stderr}");
    assert!(stderr.contains(&format!("{name}:12:3: error:")), "{stderr}");
    // `svg` and `ascii` report the same places, and still write the images.
    sirene()
        .arg("svg")
        .arg(&md)
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!("{name}:3:22: error:")));
    assert!(tmp.path().join("doc-4.svg").exists());
    assert!(!tmp.path().join("doc-5.svg").exists());
    sirene()
        .arg("ascii")
        .arg(&md)
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!("{name}:6:3: error:")));
    // JSON.
    sirene()
        .args(["check", "--json"])
        .arg(&md)
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"line\":12,\"column\":3"));
}

#[test]
fn markdown_without_formulas_or_diagrams_still_warns() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(&md, "costs $5 or $6, `$x$`\n\n```rust\n$$x$$\n```\n").unwrap();
    sirene()
        .arg("check")
        .arg(&md)
        .assert()
        .success()
        .stderr(predicate::str::contains("no diagram blocks"));
    std::fs::write(&md, "only $x$\n").unwrap();
    sirene().arg("check").arg(&md).assert().success().stderr("");
}
