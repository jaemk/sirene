//! Mermaid block diagrams (MBLOCK-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model and grid each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::block::{self, Block, CellKind, Heading};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Shape, Stroke};
use sirene_core::scene::{Approx, Owner, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/block");
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

fn diagnostics(src: &str, lowered: &Lowered) -> String {
    let mut out = String::new();
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(out, "{line}:{col}: {severity}: {}", d.message).unwrap();
    }
    out
}

/// Each fixture's diagnostics and drawing; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            assert_eq!(block::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Block, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                assert!(!s.elements.is_empty(), "{name}");
            }
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let out = diagram::lower(&src).diagram.ascii(&Options {
            charset: Charset::Ascii,
        });
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn lowered(src: &str) -> (Block, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Block(b) => (b, d),
        other => panic!("not a block diagram: {other:?}"),
    }
}

/// MBLOCK-1, MBLOCK-2: blocks fill their group's grid in order; `space` and widths take
/// columns; `block` ... `end` nests a group; a block arrow points where it says.
#[test]
fn model() {
    let (b, d) = lowered(
        "---\ntitle: T\nconfig:\n  block:\n    padding: 4\n---\nblock-beta\naccTitle: at\ncolumns 3\na[\"A\"] b:2\nspace\nblock:g[\"G\"]\n  columns auto\n  c((\"C\"))\nend\nd<[\"go\"]>(left, up)\na --> b\nb -- \"x\" --- c\nclassDef k fill:#f00\nclass a,g k\nstyle d stroke:#00f\n",
    );
    assert!(d.is_empty(), "{d:?}");
    let c = &b.chart;
    assert_eq!(c.title.as_deref(), Some("T"));
    assert_eq!(c.acc_title.as_deref(), Some("at"));
    assert_eq!(b.padding, 4.0);
    let ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c", "d"]);
    assert_eq!(c.nodes[0].label.text, "A");
    assert_eq!(c.nodes[1].label.text, "b");
    assert_eq!(c.nodes[2].shape, Shape::Circle);
    let root: Vec<(CellKind, usize)> = b.groups[0]
        .cells
        .iter()
        .map(|c| (c.kind, c.width))
        .collect();
    assert_eq!(
        root,
        [
            (CellKind::Node(0), 1),
            (CellKind::Node(1), 2),
            (CellKind::Space, 1),
            (CellKind::Group(1), 1),
            (CellKind::Node(3), 1),
        ]
    );
    assert_eq!(b.groups[0].columns, Some(3));
    assert_eq!(b.groups[1].columns, None);
    assert_eq!(b.groups[1].subgraph, Some(0));
    assert_eq!(c.subgraphs[0].id, "g");
    assert_eq!(c.subgraphs[0].title.text, "G");
    assert_eq!(b.arrows[3], Some(vec![Heading::Left, Heading::Up]));
    assert_eq!(b.arrows[0], None);
    let edges: Vec<(&str, &str, Option<Arrow>, Stroke)> = c
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str(), e.end, e.stroke))
        .collect();
    assert_eq!(
        edges,
        [
            ("a", "b", Some(Arrow::Point), Stroke::Normal),
            ("b", "c", None, Stroke::Normal),
        ]
    );
    assert_eq!(c.edges[1].label.as_ref().unwrap().text, "x");
    assert_eq!(c.nodes[0].classes, ["k"]);
    assert_eq!(c.subgraphs[0].classes, ["k"]);
    assert_eq!(c.nodes[3].styles.len(), 1);
}

/// MBLOCK-2: a later mention of a block changes its label, not its place; `space:N` is N
/// columns.
#[test]
fn mentions_and_spaces() {
    let (b, d) = lowered("block-beta\na\nspace:3\na[\"Again\"]\n");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(b.chart.nodes.len(), 1);
    assert_eq!(b.chart.nodes[0].label.text, "Again");
    assert_eq!(b.groups[0].cells.len(), 4);
}

/// MBLOCK-2: bad counts and widths are drawn as near as they can be, with a diagnostic.
#[test]
fn errors() {
    let (b, d) = lowered("block-beta\ncolumns 0\na:0\nblock\n  columns 2\n  c:5\nend\nclass q k\n");
    let e = |m: &str| (Severity::Error, m.to_owned());
    let w = |m: &str| (Severity::Warning, m.to_owned());
    assert_eq!(
        d,
        [
            e("a group has at least 1 column; ignoring this"),
            e("`a` must be at least 1 column wide; drawing it 1 wide"),
            w("5 columns is wider than the 2 columns of its group; drawing it 2 wide"),
            w("no block `q`; ignoring it"),
        ]
    );
    assert_eq!(b.groups[0].columns, None);
    assert_eq!(b.groups[0].cells[0].width, 1);
    assert_eq!(b.groups[1].cells[0].width, 2);
    assert_eq!(b.chart.subgraphs[0].id, "block-1");
}

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    };
    assert_eq!(
        errors("block-beta\nblock:g\na\n"),
        ["unclosed `block`: expected `end`"]
    );
    assert_eq!(
        errors("block-beta\nend\n"),
        ["`end` without a `block` to close"]
    );
    assert_eq!(
        errors("block-beta\ncolumns x\n"),
        ["expected a number of columns such as `3`, or `auto`"]
    );
    assert_eq!(
        errors("block-beta\na<[\"x\"]>\n"),
        ["expected the arrow's direction, such as `(right)`"]
    );
    assert_eq!(
        errors("block-beta\na -->\n"),
        ["expected a block after the edge"]
    );
    assert!(errors("block-beta\n").is_empty());
    assert!(errors("block\na\n").is_empty());
}

/// MBLOCK-3: blocks in a row sit side by side, rows stack, a group holds its blocks, and a
/// block arrow is drawn as an outline.
#[test]
fn drawing() {
    let src = "block-beta\ncolumns 2\na b\nblock:g\n  c\nend\nd<[\"go\"]>(right)\na --> b\n";
    let lowered = diagram::lower(src);
    let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
    let center = |want: &str| {
        s.elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == want => Some(*center),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no text {want}"))
    };
    let (a, b, c, d) = (center("a"), center("b"), center("c"), center("go"));
    assert!((a.y - b.y).abs() < 0.5 && a.x < b.x);
    assert!(c.y > a.y && (c.x - a.x).abs() < 0.5);
    assert!((d.y - c.y).abs() < 0.5 && d.x > c.x);
    // The arrow's outline belongs to its block.
    let Diagram::Block(blk) = &lowered.diagram else {
        unreachable!()
    };
    let arrow = blk.chart.nodes.iter().position(|n| n.id == "d").unwrap();
    assert!(s.elements.iter().any(|e| e.owner == Owner::Node(arrow)
        && matches!(&e.primitive, Primitive::Path { segments, .. } if segments.len() >= 7)));
    assert!(s.elements.iter().any(|e| matches!(e.owner, Owner::Edge(_))));
}
