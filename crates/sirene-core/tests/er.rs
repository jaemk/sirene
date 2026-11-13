//! Mermaid ER diagrams (MER-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Direction, Flowchart, Stroke};
use sirene_core::scene::{self, Approx, Primitive, Theme};
use sirene_core::{Severity, er};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/er");
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
            assert_eq!(er::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Er, "{name}");
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

/// MER-1, MER-2: each relationship's two markers and line, from symbols and from words.
#[test]
fn relationships() {
    let c = chart(
        "erDiagram\n  A |o--o| B : x\n  C ||..|| D\n  E }o.-o{ F\n  G }|-.|{ H\n  \
         I zero or one to one or more J : y\n  K many(0) optionally to only one L\n",
    );
    use Arrow::*;
    use Stroke::{Dotted, Normal};
    let got: Vec<_> = c
        .edges
        .iter()
        .map(|e| (e.start.unwrap(), e.stroke, e.end.unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            (ZeroOne, Normal, ZeroOne),
            (One, Dotted, One),
            (ZeroMany, Dotted, ZeroMany),
            (OneMany, Dotted, OneMany),
            (ZeroOne, Normal, OneMany),
            (ZeroMany, Dotted, One),
        ]
    );
    assert_eq!(c.edges[0].label.as_ref().unwrap().text, "x");
    assert_eq!(c.edges[4].label.as_ref().unwrap().text, "y");
    assert!(c.edges[1].label.is_none());
    let ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"]
    );
}

/// MER-1: either spelling of a cardinality symbol is read on either side.
#[test]
fn mirrored_symbols() {
    let c = chart(
        "erDiagram\n  \"Order Item\" o{--|| ORDER : part\n  A |{--o| B\n  C o|--}| D\n  E |o--|{ F\n  G }o--o{ H\n",
    );
    use Arrow::*;
    let got: Vec<_> = c
        .edges
        .iter()
        .map(|e| (e.start.unwrap(), e.end.unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            (ZeroMany, One),
            (OneMany, ZeroOne),
            (ZeroOne, OneMany),
            (ZeroOne, OneMany),
            (ZeroMany, ZeroMany),
        ]
    );
    assert_eq!(c.edges[0].from, "Order Item");
}

/// MER-2: attributes are one section of rows of type, name, keys, and comment.
#[test]
fn attributes() {
    let c = chart(
        "erDiagram\n  CAR {\n    string reg PK \"plate\"\n    List~int~ ids FK,UK\n    \
         int doors\n  }\n  CAR ||--o{ DRIVER : has\n",
    );
    let car = c.node("CAR").unwrap();
    assert_eq!(car.sections.len(), 1);
    let rows: Vec<Vec<&str>> = car.sections[0]
        .iter()
        .map(|r| r.cells.iter().map(String::as_str).collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec!["string", "reg", "PK", "plate"],
            vec!["List<int>", "ids", "FK, UK", ""],
            vec!["int", "doors", "", ""],
        ]
    );
    assert!(c.node("DRIVER").unwrap().sections.is_empty());
}

/// MER-1: aliases, quoted names, `:::` classes, `class`, `style`, and `direction`.
#[test]
fn names_and_styling() {
    let c = chart(
        "erDiagram\n  direction RL\n  p[Person]\n  \"Big Co\"[\"A company\"] ||--o{ p : employs\n  \
         classDef hot fill:#f00\n  p:::hot\n  class Big hot\n  style p stroke:#000\n  Big\n",
    );
    assert_eq!(c.direction, Direction::RightLeft);
    let p = c.node("p").unwrap();
    assert_eq!(p.label.text, "Person");
    assert_eq!(p.classes, ["hot"]);
    assert_eq!(p.styles[0].property, "stroke");
    assert_eq!(c.node("Big Co").unwrap().label.text, "A company");
    assert_eq!(c.node("Big").unwrap().classes, ["hot"]);
    assert_eq!(c.edges[0].from, "Big Co");
}

/// The markers are drawn at both ends of the line, in pixels and in text.
#[test]
fn drawing() {
    let c = chart("erDiagram\n  A ||--o{ B : has\n  C |o..|{ D\n");
    let s = scene::flowchart(&c, &Approx, &Theme::LIGHT);
    let texts: Vec<&str> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"has"), "{texts:?}");
    let out = ascii::render(&c, &Options::default());
    for want in ["1", "*", "?", "+"] {
        assert!(out.contains(want), "{want}:\n{out}");
    }
}

#[test]
fn diagnostics_messages() {
    let messages = |src: &str| -> Vec<(Severity, String)> {
        diagram::lower(src)
            .diagnostics
            .iter()
            .map(|d| (d.severity, d.message.clone()))
            .collect()
    };
    use Severity::{Error, Warning};
    assert_eq!(
        messages("erDiagram\n  A ||--o{\n"),
        [(
            Error,
            "expected an entity after the relationship".to_owned()
        )]
    );
    assert_eq!(
        messages("erDiagram\n  A |}--o{ B\n"),
        [(Error, "expected a relationship such as `||--o{`".to_owned())]
    );
    assert_eq!(
        messages("erDiagram\n  A {\n    string\n  }\n"),
        [(
            Error,
            "expected an attribute name after its type".to_owned()
        )]
    );
    assert_eq!(
        messages("erDiagram\n  A {\n    string a\n"),
        [(Error, "unclosed attribute block: expected `}`".to_owned())]
    );
    assert_eq!(
        messages("erDiagram\n  style X fill:#f00\n"),
        [(Warning, "unknown entity `X`".to_owned())]
    );
    assert!(messages("erDiagram\n").is_empty());
}
