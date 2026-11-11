//! A source draws the same whatever its line endings, and with or without a byte order mark
//! (LANGS-9). Every fixture under `tests/fixtures/` is drawn each way.

use std::path::{Path, PathBuf};

use sirene_core::ascii::Options;
use sirene_core::diagram::{self, Lang};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, out);
        } else if path
            .extension()
            .is_some_and(|x| x == "mmd" || x == "dot" || x == "gv")
        {
            out.push(path);
        }
    }
}

/// The text drawing of `src` and its diagnostics' messages.
fn drawn(src: &str, lang: Option<Lang>) -> (String, Vec<String>) {
    let l = diagram::lower_as(src, lang);
    let messages = l.diagnostics.iter().map(|d| d.message.clone()).collect();
    (l.diagram.ascii(&Options::default()), messages)
}

fn fixtures() -> Vec<(PathBuf, String)> {
    let mut paths = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        &mut paths,
    );
    assert!(paths.len() > 100, "{}", paths.len());
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).unwrap();
            assert!(!src.contains('\r'), "{}", p.display());
            (p, src)
        })
        .collect()
}

#[track_caller]
fn same(path: &Path, what: &str, src: &str, changed: &str) {
    let lang = Lang::of_path(path);
    let (expected, got) = (drawn(src, lang), drawn(changed, lang));
    assert!(
        expected == got,
        "{} draws differently {what}:\n{}\n{:?}\nbut was:\n{}\n{:?}",
        path.display(),
        got.0,
        got.1,
        expected.0,
        expected.1
    );
}

#[test]
fn crlf_line_endings() {
    for (path, src) in fixtures() {
        same(
            &path,
            "with CRLF line endings",
            &src,
            &src.replace('\n', "\r\n"),
        );
    }
}

#[test]
fn byte_order_mark() {
    for (path, src) in fixtures() {
        same(
            &path,
            "after a byte order mark",
            &src,
            &format!("\u{feff}{src}"),
        );
        // Detected from the text alone, too.
        let (expected, got) = (drawn(&src, None), drawn(&format!("\u{feff}{src}"), None));
        assert!(
            expected == got,
            "{} is detected differently",
            path.display()
        );
    }
}

/// A byte order mark is kept in the tree, before frontmatter too.
#[test]
fn byte_order_mark_trees() {
    use sirene_core::{block, c4, class, dot, flowchart, railroad, state, zenuml};
    let fm = "\u{feff}---\r\ntitle: t\r\n---\r\n";
    type Text = fn(&str) -> String;
    let cases: [(Text, String); 8] = [
        (
            |s| flowchart::parse(s).syntax().to_string(),
            format!("{fm}flowchart LR\r\nA\r\n"),
        ),
        (
            |s| state::parse(s).syntax().to_string(),
            format!("{fm}stateDiagram-v2\r\nA\r\n"),
        ),
        (
            |s| class::parse(s).syntax().to_string(),
            format!("{fm}classDiagram\r\nclass A\r\n"),
        ),
        (
            |s| c4::parse(s).syntax().to_string(),
            format!("{fm}C4Context\r\n"),
        ),
        (
            |s| zenuml::parse(s).syntax().to_string(),
            format!("{fm}zenuml\r\nA.b()\r\n"),
        ),
        (
            |s| block::parse(s).syntax().to_string(),
            format!("{fm}block-beta\r\na\r\n"),
        ),
        (
            |s| railroad::parse(s).syntax().to_string(),
            format!("{fm}railroad-beta\r\n"),
        ),
        (
            |s| dot::parse(s).syntax().to_string(),
            "\u{feff}digraph { a }\r\n".into(),
        ),
    ];
    for (text, src) in cases {
        assert_eq!(text(&src), src);
    }
    let l = diagram::lower(&format!("{fm}flowchart LR\r\nA\r\n"));
    assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
}

#[test]
fn no_final_newline() {
    for (path, src) in fixtures() {
        same(
            &path,
            "without a final newline",
            &src,
            src.trim_end_matches('\n'),
        );
    }
}
