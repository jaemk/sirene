//! Tree views (MTREEV-*): the fixture corpus and its drawings (LANGS-6), and the model each
//! source builds.

use std::fs;
use std::path::Path;

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::scene::{Approx, Primitive, Theme};
use sirene_core::treeview::{self, TreeView};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/treeview");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .map(|p| {
            let name = p
                .file_stem()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            (name, fs::read_to_string(&p).expect("readable fixture"))
        })
        .collect();
    out.sort();
    out
}

/// Each fixture's drawing as text; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for (name, src) in fixtures("valid") {
        assert_eq!(treeview::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::TreeView, "{name}");
        let lowered = diagram::lower(&src);
        assert!(
            lowered.diagnostics.is_empty(),
            "{name}: {:?}",
            lowered.diagnostics
        );
        for charset in [Charset::Unicode, Charset::Ascii] {
            let out = lowered.diagram.ascii(&Options { charset });
            let kind = match charset {
                Charset::Unicode => "valid",
                Charset::Ascii => "ascii",
            };
            insta::assert_snapshot!(format!("{kind}__{name}"), out, &src);
        }
        for theme in [Theme::LIGHT, Theme::DARK] {
            let s = lowered.diagram.scene(&Approx, &theme);
            assert!(!s.elements.is_empty(), "{name}");
        }
    }
}

fn lowered(src: &str) -> TreeView {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::TreeView(t) => t,
        other => panic!("not a tree view: {other:?}"),
    }
}

/// MTREEV-1, MTREEV-2: names, directories, and annotations.
#[test]
fn model() {
    let t = lowered(
        "treeView-beta\n  a/\n    \"b c\" :::highlight icon(logos:react) ## main  thing\n  d\n",
    );
    assert_eq!(t.entries.len(), 2);
    let a = &t.entries[0];
    assert!(a.directory);
    assert_eq!(a.name, "a");
    let b = &a.children[0];
    assert_eq!(b.name, "b c");
    assert_eq!(b.class.as_deref(), Some("highlight"));
    assert_eq!(b.icon.as_deref(), Some("logos:react"));
    assert_eq!(b.description.as_deref(), Some("main  thing"));
    assert!(!t.entries[1].directory);
}

/// MTREEV-3: a box-drawn tree with an indented line that has no branch.
#[test]
fn problems() {
    let messages: Vec<String> = diagram::lower("treeView-beta\n├── a\n    b\n")
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect();
    assert_eq!(
        messages,
        [
            "expected `├──` or `└──` before an indented line in a tree drawn with box-drawing \
          characters"
        ]
    );
}

/// MTREEV-4: the root `/` first, then a row per entry.
#[test]
fn drawing() {
    let s = diagram::lower("treeView-beta\n  src/\n    a.ts\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    let texts: Vec<&str> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["/", "src", "a.ts"]);
    assert!(s.width > 0.0 && s.height > 0.0);
}
