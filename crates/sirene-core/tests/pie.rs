//! Mermaid pie charts (MPIE-*): the fixture corpus with its diagnostics and drawings (LANGS-6),
//! and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::pie::{self, Pie, percent};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pie");
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
            assert_eq!(pie::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Pie, "{name}");
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

fn pie(src: &str) -> Pie {
    let lowered = diagram::lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    match lowered.diagram {
        Diagram::Pie(p) => p,
        other => panic!("not a pie chart: {other:?}"),
    }
}

/// MPIE-1, MPIE-2: the header, title, and slices.
#[test]
fn slices() {
    let p = pie("pie showData title T\n  \"a\" : 1\n  'b':2.5\n  \"c\" : 2.5\n  \"d\" : 0\n");
    assert!(p.show_data);
    assert_eq!(p.title.as_deref(), Some("T"));
    let labels: Vec<&str> = p.slices.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["a", "b", "c", "d"]);
    assert_eq!(p.total(), 6.0);
    // Largest first, ties in source order.
    let ordered: Vec<&str> = p.ordered().iter().map(|s| s.label.as_str()).collect();
    assert_eq!(ordered, ["b", "c", "a", "d"]);
    assert_eq!(percent(2.5, 6.0), 42);
    assert_eq!(percent(1.0, 0.0), 0);
    // A `title` statement wins over frontmatter.
    let p = pie("---\ntitle: front\n---\npie\n  title inner\n  accTitle: at\n");
    assert_eq!(p.title.as_deref(), Some("inner"));
    assert_eq!(p.acc_title.as_deref(), Some("at"));
    assert!(!p.show_data);
    let p = pie("---\ntitle: front\n---\npie\n");
    assert_eq!(p.title.as_deref(), Some("front"));
}

/// MPIE-2: a negative value is an error, a repeated label a warning; both are skipped.
#[test]
fn errors() {
    let lowered = diagram::lower("pie\n  \"a\" : 1\n  \"a\" : 2\n  \"b\" : -1\n");
    let got: Vec<(Severity, &str)> = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (
                Severity::Warning,
                "`a` is already a slice; skipping this one"
            ),
            (
                Severity::Error,
                "`b` has a negative value; a slice must be 0 or more"
            ),
        ]
    );
    let Diagram::Pie(p) = lowered.diagram else {
        panic!("not a pie chart")
    };
    assert_eq!(p.slices.len(), 1);
    assert_eq!(p.slices[0].value, 1.0);
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
        errors("pie foo\n"),
        ["expected a slice (`\"label\" : value`) or `title`"]
    );
    assert_eq!(errors("pie\n  'a : 1\n"), ["unclosed string: expected `'`"]);
    assert!(errors("pie\n").is_empty());
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

fn filled_paths(s: &Scene) -> usize {
    s.elements
        .iter()
        .filter(|e| matches!(&e.primitive, Primitive::Path { fill: Some(_), .. }))
        .count()
}

/// MPIE-3: slices with percentages, a legend to the right, and the title above.
#[test]
fn drawing() {
    let src = "pie showData title Pets\n  \"Cats\" : 1\n  \"Dogs\" : 3\n  \"Fish\" : 0.001\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| {
        t.iter()
            .find(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .unwrap_or_else(|| panic!("no {want} in {t:?}"))
    };
    let (title, dogs, cats) = (at("Pets"), at("Dogs [3]"), at("Cats [1]"));
    let (big, small) = (at("75%"), at("25%"));
    // The 75% slice starts at twelve o'clock and runs clockwise, so its middle is lower right
    // and the 25% slice's upper left.
    assert!(big.x > small.x && big.y > small.y, "{big:?} {small:?}");
    assert!(title.y < big.y.min(small.y));
    assert!(dogs.y < cats.y && dogs.x > big.x.max(small.x));
    // The 0% slice is not drawn but is in the legend: two slices and three swatches.
    assert!(t.iter().all(|(text, _)| text != "0%"));
    assert!(t.iter().any(|(text, _)| text == "Fish [0.001]"));
    assert_eq!(filled_paths(&s), 5);
    // Every value zero: only the outline and the swatches.
    let s = diagram::lower("pie\n  \"a\" : 0\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert_eq!(filled_paths(&s), 1);
}
