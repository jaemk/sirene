//! Mermaid sankey diagrams (MSANKEY-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model and layout each row builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::sankey::{self, Align, LinkColor, Sankey};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sankey");
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
            assert_eq!(sankey::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Sankey, "{name}");
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

fn lowered(src: &str) -> (Sankey, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Sankey(s) => (s, d),
        other => panic!("not a sankey diagram: {other:?}"),
    }
}

fn links(s: &Sankey) -> Vec<(&str, &str, f64)> {
    s.links
        .iter()
        .map(|l| {
            (
                s.nodes[l.source].as_str(),
                s.nodes[l.target].as_str(),
                l.value,
            )
        })
        .collect()
}

/// MSANKEY-1, MSANKEY-2: plain and quoted fields, nodes in order of first use, and the config.
#[test]
fn model() {
    let (s, d) = lowered(
        "---\ntitle: T\nconfig:\n  sankey:\n    width: 300\n    height: 200\n    linkColor: target\n    nodeAlignment: center\n    showValues: false\n    prefix: \"~\"\n    suffix: \" kg\"\n    nodeWidth: 4\n    nodePadding: 7\n---\nsankey\n  a b , \"c, \"\"d\"\"\" , 1.5\nx,a b,2\ny,x,1e3\nz,x,2.5E-1\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(s.title.as_deref(), Some("T"));
    assert_eq!(s.nodes, ["a b", "c, \"d\"", "x", "y", "z"]);
    assert_eq!(
        links(&s),
        [
            ("a b", "c, \"d\"", 1.5),
            ("x", "a b", 2.0),
            ("y", "x", 1000.0),
            ("z", "x", 0.25)
        ]
    );
    assert_eq!((s.width, s.height, s.node_width), (300.0, 200.0, 4.0));
    assert_eq!(
        (&s.link_color, s.align, s.show_values, s.padding()),
        (&LinkColor::Target, Align::Center, false, 7.0)
    );
    assert_eq!(s.format_value(2.0), "~2 kg");
    let (s, _) = lowered("---\nconfig:\n  sankey:\n    linkColor: \"#a1a1a1\"\n---\nsankey\n");
    assert_eq!(s.link_color, LinkColor::Fixed("#a1a1a1".into()));
    assert_eq!(Sankey::default().link_color, LinkColor::Gradient);
}

/// MSANKEY-2: a link that closes a cycle or flows into its own node, and a negative value, are
/// left out; a node only they named is not drawn.
#[test]
fn errors() {
    let (s, d) = lowered(
        "---\nconfig:\n  sankey:\n    nodeAlignment: middle\n---\nsankey\nA,B,10\nB,C,4\nC,A,1\nD,D,2\nE,F,-3\nG,H,1e40\n",
    );
    let e = |m: &str| (Severity::Error, m.to_owned());
    assert_eq!(
        d,
        [
            (
                Severity::Warning,
                "ignoring `nodeAlignment: middle`: not a valid value".to_owned()
            ),
            e(
                "`C` -> `A`: it closes the cycle `C` -> `A` -> `B` -> `C`, which a sankey \
                 diagram cannot draw; ignoring it"
            ),
            e(
                "`D` -> `D`: `D` flows into itself, which a sankey diagram cannot draw; \
                 ignoring it"
            ),
            e("`E` -> `F` has a negative value; ignoring it"),
            e("`G` -> `H` has a value over 1e30; ignoring it"),
        ]
    );
    assert_eq!(s.nodes, ["A", "B", "C"]);
    assert_eq!(links(&s), [("A", "B", 10.0), ("B", "C", 4.0)]);
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
        errors("sankey\nA,B\n"),
        ["expected `,` and the next field: a row is `source,target,value`"]
    );
    assert!(errors("sankey-beta\n").is_empty());
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

/// MSANKEY-3: columns left to right by depth, heights by value, and labels toward the middle.
#[test]
fn drawing() {
    let (s, _) = lowered("sankey\nA,B,30\nA,C,10\nB,D,30\n");
    let l = s.layout();
    let h = |i: usize| l.nodes[i].y1 - l.nodes[i].y0;
    assert_eq!(
        l.nodes.iter().map(|n| n.layer).collect::<Vec<_>>(),
        [0, 1, 2, 2]
    );
    assert!((h(0) - h(1) - h(2)).abs() < 0.01 && (h(1) - 3.0 * h(2)).abs() < 0.01);
    assert!((h(1) - h(3)).abs() < 0.01);
    let scene = diagram::lower("sankey\nA,B,30\nA,C,10\nB,D,30\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    let t = texts(&scene);
    let at = |want: &str| t.iter().find(|(x, _)| x == want).map(|&(_, p)| p).unwrap();
    // A's label is right of its node, D's left of its own; each label carries the node's value.
    assert!(at("A 40").x < at("B 30").x && at("B 30").x < at("D 30").x);
    assert!((at("C 10").y - at("D 30").y).abs() > 10.0);
    assert!(at("A 40").x < scene.width / 2.0 && at("D 30").x < scene.width - 16.0);
}
