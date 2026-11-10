//! Mermaid mindmaps (MMIND-*): the fixture corpus with its diagnostics and drawings (LANGS-6),
//! and the tree each source builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::mindmap::{self, Mindmap, Shape};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Segment, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mindmap");
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
            assert_eq!(mindmap::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Mindmap, "{name}");
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
                assert!(s.elements.iter().all(|e| e.owner == Owner::Diagram));
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

fn lowered(src: &str) -> (Mindmap, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Mindmap(m) => (m, d),
        other => panic!("not a mindmap: {other:?}"),
    }
}

fn mindmap(src: &str) -> Mindmap {
    let (m, d) = lowered(src);
    assert!(d.is_empty(), "{d:?}\n{src}");
    m
}

fn labels(m: &Mindmap, of: &[usize]) -> Vec<String> {
    of.iter().map(|&i| m.nodes[i].label.text.clone()).collect()
}

/// MMIND-2: indentation builds the tree; a node's parent is the nearest node above it that is
/// indented less.
#[test]
fn tree() {
    let m = mindmap("mindmap\n  root\n    a\n      a1\n     a2\n    b\n   c\n      c1\n");
    assert_eq!(m.nodes.len(), 7);
    assert_eq!(labels(&m, &m.nodes[0].children), ["a", "b", "c"]);
    // `a2` is indented less than `a1` but more than `a`.
    assert_eq!(labels(&m, &m.nodes[1].children), ["a1", "a2"]);
    let c = m.nodes.iter().position(|n| n.label.text == "c").unwrap();
    assert_eq!(labels(&m, &m.nodes[c].children), ["c1"]);
    let depths: Vec<usize> = m.nodes.iter().map(|n| n.depth).collect();
    assert_eq!(depths, [0, 1, 2, 2, 1, 1, 2]);
    // Each branch of the root is a section; its nodes share it.
    let sections: Vec<Option<usize>> = m.nodes.iter().map(|n| n.section).collect();
    assert_eq!(
        sections,
        [None, Some(0), Some(0), Some(0), Some(1), Some(2), Some(2)]
    );
    let order: Vec<usize> = m.descendants(0).iter().map(|&(i, _)| i).collect();
    assert_eq!(order, [1, 2, 3, 4, 5, 6]);
    assert_eq!(m.descendants(1), [(2, 1), (3, 1)]);
    // Tabs count as one column each, as in Mermaid.
    let m = mindmap("mindmap\n\troot\n\t\ta\n");
    assert_eq!(m.nodes[1].parent, Some(0));
    assert!(mindmap("mindmap\n").nodes.is_empty());
}

/// MMIND-2: shapes, ids, labels, icons, and classes.
#[test]
fn nodes() {
    let m = mindmap(
        "---\ntitle: T\n---\nmindmap\n  root((Root))\n    sq[Square]\n    :::urgent  large\n    rd(Rounded)\n    cl)Cloud(\n    bg))Bang((\n    hx{{Hex}}\n    Plain words\n    [No id]\n    e[]\n    md[\"`**bold** and\n    *italic*`\"]\n    q[\"a (b) [c]\"]\n    br[one<br/>two]\n",
    );
    assert_eq!(m.title.as_deref(), Some("T"));
    let shapes: Vec<Shape> = m.nodes.iter().map(|n| n.shape).collect();
    assert_eq!(
        shapes,
        [
            Shape::Circle,
            Shape::Square,
            Shape::Rounded,
            Shape::Cloud,
            Shape::Bang,
            Shape::Hexagon,
            Shape::Default,
            Shape::Square,
            Shape::Square,
            Shape::Square,
            Shape::Square,
            Shape::Square,
        ]
    );
    let ids: Vec<Option<&str>> = m.nodes.iter().map(|n| n.id.as_deref()).collect();
    assert_eq!(ids[0], Some("root"));
    assert_eq!(ids[6], Some("Plain words"));
    assert_eq!(ids[7], None);
    let all: Vec<usize> = (0..m.nodes.len()).collect();
    assert_eq!(
        labels(&m, &all),
        [
            "Root",
            "Square",
            "Rounded",
            "Cloud",
            "Bang",
            "Hex",
            "Plain words",
            "No id",
            // An empty shape shows its id.
            "e",
            "**bold** and\n*italic*",
            "a (b) [c]",
            "one\ntwo",
        ]
    );
    assert!(m.nodes[9].label.markdown && !m.nodes[10].label.markdown);
    assert_eq!(m.nodes[1].classes, ["urgent", "large"]);
    assert!(m.nodes[2].classes.is_empty());

    let (m, d) = lowered("mindmap\n  root\n  ::icon(fa fa-book)\n");
    assert_eq!(m.nodes[0].icon.as_deref(), Some("fa fa-book"));
    assert_eq!(
        d,
        [(
            Severity::Warning,
            "icons are not drawn; ignoring `fa fa-book`".to_owned()
        )]
    );
}

/// MMIND-2: a second root is an error and is left out with its decorations; an icon or class
/// before any node is an error.
#[test]
fn errors() {
    let (m, d) = lowered("mindmap\n  :::early\n  root\n    a\n  second\n  ::icon(x)\n    b\n");
    assert_eq!(
        d,
        [
            (
                Severity::Error,
                "`:::` must follow the node it applies to".to_owned()
            ),
            (
                Severity::Error,
                "`second` has no parent: a mindmap has one root, so indent this line under a \
                 node above it"
                    .to_owned()
            ),
        ]
    );
    // `b` joins the root, the nearest node indented less.
    assert_eq!(labels(&m, &m.nodes[0].children), ["a", "b"]);
    assert!(m.nodes.iter().all(|n| n.icon.is_none()));
    let (_, d) = lowered("mindmap\n  ::icon(x)\n  root\n");
    assert_eq!(d[0].1, "`::icon(...)` must follow the node it applies to");
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
    assert_eq!(errors("mindmap\n  a[x\n"), ["unclosed `[`: expected `]`"]);
    assert_eq!(
        errors("mindmap\n  a((x)\n"),
        ["unclosed `((`: expected `))`"]
    );
    assert_eq!(
        errors("mindmap\n  {x\n"),
        ["expected a node, `::icon(...)`, or `:::class`"]
    );
    assert!(errors("mindmap\n").is_empty());
}

fn texts(s: &Scene) -> Vec<(String, Pt)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.clone(), *center)),
            _ => None,
        })
        .collect()
}

/// MMIND-3: the root in the middle, branches split between its sides in order, and edges that
/// thin with depth in their branch's color.
#[test]
fn drawing() {
    let src = "---\ntitle: Map\n---\nmindmap\n  root((Root))\n    a\n      a1\n        a11\n    b\n    c\n    d\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| {
        t.iter()
            .find(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .unwrap_or_else(|| panic!("no {want} in {t:?}"))
    };
    let (title, root, a, a1, a11) = (at("Map"), at("Root"), at("a"), at("a1"), at("a11"));
    let (b, c, d) = (at("b"), at("c"), at("d"));
    assert!(title.y < root.y.min(a.y).min(d.y));
    // `a` holds the most height, so `a` and `b` go right and `c` and `d` left, each side top
    // to bottom in order.
    assert!(a.x > root.x && b.x > root.x && c.x < root.x && d.x < root.x);
    assert!(a.y < b.y && c.y < d.y);
    assert!(a1.x > a.x && a11.x > a1.x);
    // A node is centered on its children.
    assert!((a.y - a11.y).abs() < 0.01);
    // Every node lies inside the drawing.
    for (_, p) in &t {
        assert!(p.x > 0.0 && p.x < s.width && p.y > 0.0 && p.y < s.height);
    }
    // The diagram is read-only, so it owns every element.
    assert!(s.elements.iter().all(|e| e.owner == Owner::Diagram));
    // Edges, in node order from node 1, with their position among the elements.
    let edges: Vec<(usize, f32, u32)> = s
        .elements
        .iter()
        .enumerate()
        .filter_map(|(k, e)| match &e.primitive {
            Primitive::Path {
                segments,
                stroke: Some(st),
                ..
            } if is_edge(segments) => {
                let c = st.color;
                Some((
                    k,
                    st.width,
                    u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b),
                ))
            }
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 6);
    // `a`, `a1`, and `a11` are nodes 1 to 3: thinner with depth, all in `a`'s color; `b`'s
    // differs.
    let (a, a1, a11, b) = (edges[0], edges[1], edges[2], edges[3]);
    assert!(a.1 > a1.1 && a1.1 > a11.1);
    assert!(a.2 == a1.2 && a1.2 == a11.2);
    assert_ne!(a.2, b.2);
    // Edges come before nodes, so nodes cover their ends.
    let first_fill = s
        .elements
        .iter()
        .position(|e| matches!(&e.primitive, Primitive::Path { fill: Some(_), .. }))
        .unwrap();
    assert!(edges.iter().all(|e| e.0 < first_fill));

    // One branch goes right; an empty mindmap is just its margins.
    let s = diagram::lower("mindmap\n  r\n    x\n")
        .diagram
        .scene(&Approx, &Theme::DARK);
    let t = texts(&s);
    assert!(t[1].1.x > t[0].1.x);
    let s = diagram::lower("mindmap\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert!(s.elements.is_empty());
    assert_eq!((s.width, s.height), (32.0, 32.0));
}

fn is_edge(segments: &[Segment]) -> bool {
    segments.iter().any(|s| matches!(s, Segment::Cubic { .. }))
}

/// MMIND-3: every shape draws one filled outline; the default one adds a line under it.
#[test]
fn shapes() {
    let src = "mindmap\n  r[R]\n    a(A)\n    b((B))\n    c)C(\n    d))D((\n    e{{E}}\n    f\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let count =
        |want: fn(&Primitive) -> bool| s.elements.iter().filter(|e| want(&e.primitive)).count();
    assert_eq!(
        count(|p| matches!(p, Primitive::Path { fill: Some(_), .. })),
        7
    );
    // Six edges and one underline.
    assert_eq!(
        count(|p| matches!(p, Primitive::Path { fill: None, segments, .. } if is_edge(segments))),
        6
    );
    assert_eq!(
        count(|p| matches!(p, Primitive::Path { fill: None, segments, .. } if !is_edge(segments))),
        1
    );
}
