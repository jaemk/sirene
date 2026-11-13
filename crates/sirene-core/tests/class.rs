//! Mermaid class diagrams (MCLASS-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Direction, Flowchart, NoteSide, Row, Shape, Stroke};
use sirene_core::scene::{self, Approx, Primitive, Theme};
use sirene_core::{Severity, class};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/class");
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
            assert_eq!(class::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Class, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
            assert!(!s.elements.is_empty(), "{name}");
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let chart = diagram::lower(&src).diagram.into_graph();
        let out = ascii::render(
            &chart,
            &Options {
                charset: Charset::Ascii,
            },
        );
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn chart(src: &str) -> Flowchart {
    let lowered = diagram::lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    lowered.diagram.into_graph()
}

fn rows(r: &[Row]) -> Vec<&str> {
    r.iter().map(|r| r.cells[0].as_str()).collect()
}

/// MCLASS-1, MCLASS-2: members from a body and from `A : member`, split into attributes and
/// methods, with static members underlined and abstract ones italic.
#[test]
fn members_and_sections() {
    let c = chart(
        "classDiagram\n  class A {\n    +int id\n    +run() bool\n    <<interface>>\n  }\n  \
         A : -String name$\n  A : +stop()*\n  class B\n  class C {\n    go()\n  }\n",
    );
    let a = c.node("A").unwrap();
    assert_eq!(a.label.text, "«interface»\nA");
    assert_eq!(a.sections.len(), 2);
    assert_eq!(rows(&a.sections[0]), ["+int id", "-String name"]);
    assert_eq!(rows(&a.sections[1]), ["+run() bool", "+stop()"]);
    assert!(a.sections[0][1].underline && !a.sections[0][1].italic);
    assert!(a.sections[1][1].italic && !a.sections[1][1].underline);
    // No members: a plain box; only methods: an empty attribute section first.
    assert!(c.node("B").unwrap().sections.is_empty());
    let cc = c.node("C").unwrap();
    assert_eq!(cc.sections.len(), 2);
    assert!(cc.sections[0].is_empty());
    assert_eq!(rows(&cc.sections[1]), ["go()"]);
}

#[test]
fn labels_and_generics() {
    let c = chart(
        "classDiagram\n  class List~T~\n  class A[\"An A\"]\n  class Map~K, V~ {\n    +get(K k) V\n    \
         +List~V~ values\n  }\n",
    );
    assert_eq!(c.node("List").unwrap().label.text, "List<T>");
    assert_eq!(c.node("A").unwrap().label.text, "An A");
    let m = c.node("Map").unwrap();
    assert_eq!(m.label.text, "Map<K, V>");
    assert_eq!(rows(&m.sections[0]), ["+List<V> values"]);
}

/// MCLASS-2: each relation's markers, line, labels, and cardinalities.
#[test]
fn relations() {
    let c = chart(
        "classDiagram\n  A <|-- B\n  C *-- D\n  E o-- F\n  G --> H\n  I -- J\n  K ..> L\n  \
         M ..|> N\n  O .. P\n  Q <|--|> R\n  S \"1\" --> \"0..*\" T : owns\n",
    );
    let got: Vec<_> = c.edges.iter().map(|e| (e.start, e.end, e.stroke)).collect();
    use Arrow::*;
    use Stroke::{Dotted, Normal};
    assert_eq!(
        got,
        [
            (Some(Triangle), None, Normal),
            (Some(Diamond), None, Normal),
            (Some(HollowDiamond), None, Normal),
            (None, Some(Point), Normal),
            (None, None, Normal),
            (None, Some(Point), Dotted),
            (None, Some(Triangle), Dotted),
            (None, None, Dotted),
            (Some(Triangle), Some(Triangle), Normal),
            (None, Some(Point), Normal),
        ]
    );
    let e = &c.edges[9];
    assert_eq!((e.from.as_str(), e.to.as_str()), ("S", "T"));
    assert_eq!(e.start_label.as_ref().unwrap().text, "1");
    assert_eq!(e.end_label.as_ref().unwrap().text, "0..*");
    assert_eq!(e.label.as_ref().unwrap().text, "owns");
    assert!(c.edges[0].start_label.is_none() && c.edges[0].label.is_none());
}

/// MCLASS-2: a relation's second class may follow its end with no space.
#[test]
fn relations_without_spaces() {
    let c = chart("classDiagram\n  Duck-->Egg\n  A --|>B\n  C..>D\n  E--*F\n  G --oval\n");
    use Arrow::*;
    let got: Vec<_> = c
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str(), e.start, e.end))
        .collect();
    assert_eq!(
        got,
        [
            ("Duck", "Egg", None, Some(Point)),
            ("A", "B", None, Some(Triangle)),
            ("C", "D", None, Some(Point)),
            ("E", "F", None, Some(Diamond)),
            ("G", "oval", None, None),
        ]
    );
}

/// MCLASS-2: lollipop interfaces draw a circle at the `()` end.
#[test]
fn lollipops() {
    let c = chart("classDiagram\n  Class01 --() bar\n  foo ()-- Class01\n");
    assert_eq!(c.edges.len(), 2);
    assert_eq!(
        (c.edges[0].start, c.edges[0].end),
        (None, Some(Arrow::Circle))
    );
    assert_eq!(
        (c.edges[1].start, c.edges[1].end),
        (Some(Arrow::Circle), None)
    );
    assert_eq!(
        (c.edges[1].from.as_str(), c.edges[1].to.as_str()),
        ("foo", "Class01")
    );
}

/// MCLASS-1: a name in backticks may hold spaces and punctuation.
#[test]
fn backtick_names() {
    let c = chart(
        "classDiagram\n  class `Animal Class!`\n  `Animal Class!` --> Dog\n  note for `Animal Class!` \"n\"\n",
    );
    assert!(c.node("Animal Class!").is_some());
    assert_eq!(c.nodes.len(), 2);
    assert_eq!(c.edges[0].from, "Animal Class!");
    assert_eq!(c.notes[0].target, "Animal Class!");
}

/// MCLASS-1: nested generics close as many levels as they open.
#[test]
fn nested_generics() {
    let c = chart(
        "classDiagram\n  class Grid~List~List~int~~~ {\n    +List~List~int~~ grid\n    +Map~string, List~int~~ m\n  }\n",
    );
    let g = c.node("Grid").unwrap();
    assert_eq!(g.label.text, "Grid<List<List<int>>>");
    assert_eq!(
        rows(&g.sections[0]),
        ["+List<List<int>> grid", "+Map<string, List<int>> m"]
    );
}

/// MCLASS-3: the left class of `A <|-- B` ranks above the right one.
#[test]
fn superclasses_rank_above() {
    let c = chart("classDiagram\n  Animal <|-- Duck\n");
    let s = scene::flowchart(&c, &Approx, &Theme::LIGHT);
    let top = |i| s.bounds(scene::Owner::Node(i)).unwrap().0.y;
    assert!(top(0) < top(1));
}

/// Markers and end labels are drawn; static members get an underline.
#[test]
fn drawing() {
    let c = chart(
        "classDiagram\n  A \"1\" *-- \"many\" B\n  C o-- D\n  E <|-- F\n  class G {\n    +make()$\n  }\n",
    );
    let s = scene::flowchart(&c, &Approx, &Theme::LIGHT);
    let texts: Vec<&str> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for want in ["1", "many", "+make()"] {
        assert!(texts.contains(&want), "{want}: {texts:?}");
    }
    // A hollow marker is filled with the background.
    let hollow = s.elements.iter().any(|e| {
        matches!(&e.primitive, Primitive::Path { fill: Some(f), stroke: Some(_), .. }
            if *f == Theme::LIGHT.background)
    });
    assert!(hollow);
    let out = ascii::render(&c, &Options::default());
    for want in ["◆", "◇", "△", "1", "many"] {
        assert!(out.contains(want), "{want}:\n{out}");
    }
}

#[test]
fn namespaces_notes_and_styling() {
    let c = chart(
        "classDiagram\n  direction RL\n  namespace N {\n    class A\n    class B\n  }\n  \
         note for A \"hi\"\n  classDef hot fill:#f00\n  cssClass \"A,B\" hot\n  class C:::hot\n  \
         style C stroke:#000\n",
    );
    assert_eq!(c.direction, Direction::RightLeft);
    assert_eq!(c.subgraph("N").unwrap().members, ["A", "B"]);
    assert_eq!(c.notes[0].target, "A");
    assert_eq!(c.notes[0].side, NoteSide::After);
    assert_eq!(c.notes[0].label.text, "hi");
    for id in ["A", "B", "C"] {
        assert_eq!(c.node(id).unwrap().classes, ["hot"], "{id}");
    }
    assert_eq!(c.node("C").unwrap().styles[0].property, "stroke");
}

/// A note with no class is a node of its own (MCLASS-3).
#[test]
fn floating_notes() {
    let c = chart("classDiagram\n  class A\n  note \"on its own\\nline two\"\n");
    assert_eq!(c.nodes.len(), 2);
    let n = &c.nodes[1];
    assert_eq!(n.shape, Shape::Note);
    assert_eq!(n.label.text, "on its own\nline two");
    assert!(n.sections.is_empty());
    let s = scene::flowchart(&c, &Approx, &Theme::LIGHT);
    let fill = s.elements.iter().find_map(|e| match &e.primitive {
        Primitive::Path { fill, .. } if e.owner == scene::Owner::Node(1) => *fill,
        _ => None,
    });
    assert_eq!(
        fill,
        Some(Theme::LIGHT.cluster_fill),
        "drawn in the note colors"
    );
    let text = ascii::render(
        &c,
        &Options {
            charset: Charset::Unicode,
        },
    );
    assert!(text.contains("on its own") && text.contains('╮'), "{text}");
}

/// MCLASS-4, LANGS-6.
#[test]
fn unsupported_is_a_warning() {
    for (src, want) in [
        (
            "classDiagram\n  click A href \"x\"\n",
            "`click` is not supported",
        ),
        (
            "classDiagram\n  callback A \"f\"\n",
            "`callback` is not supported",
        ),
        ("classDiagram\n  link A \"x\"\n", "`link` is not supported"),
        ("classDiagram\n  style X fill:#f00\n", "unknown class `X`"),
    ] {
        let d = diagram::lower(src).diagnostics;
        assert!(
            d.iter()
                .any(|d| d.severity == Severity::Warning && d.message.contains(want)),
            "{src}: {d:?}"
        );
    }
}

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.clone())
            .collect()
    };
    assert_eq!(
        errors("classDiagram\n  A <|--\n"),
        ["expected a class after the relation"]
    );
    assert_eq!(
        errors("classDiagram\n  class A {\n  +x\n"),
        ["unclosed class body: expected `}`"]
    );
    assert_eq!(
        errors("classDiagram\n  namespace N {\n  class A\n"),
        ["unclosed namespace: expected `}`"]
    );
    assert_eq!(errors("flowchart\n").len(), 0);
    assert_eq!(
        errors("classDiagramX\n").first().map(String::as_str),
        Some("expected `flowchart` or `graph` header")
    );
}
