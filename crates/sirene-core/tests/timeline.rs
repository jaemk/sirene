//! Mermaid timelines (TIMELI-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Theme};
use sirene_core::timeline::{self, Direction, Timeline};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
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
            assert_eq!(timeline::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Timeline, "{name}");
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

fn timeline(src: &str) -> Timeline {
    let lowered = diagram::lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    match lowered.diagram {
        Diagram::Timeline(t) => t,
        other => panic!("not a timeline: {other:?}"),
    }
}

/// TIMELI-1, TIMELI-2: periods with events on one line and on continuation lines.
#[test]
fn periods_and_events() {
    let t = timeline(
        "timeline\n  title T\n  2002 : A : B\n       : C\n  10:30 : stand:up\n  bare\n  x<br>y : z\n",
    );
    assert_eq!(t.title.as_deref(), Some("T"));
    assert_eq!(t.direction, Direction::LeftRight);
    assert_eq!(t.sections.len(), 1);
    assert!(t.sections[0].name.is_none());
    let p = &t.sections[0].periods;
    assert_eq!(p.len(), 4);
    assert_eq!(p[0].label, ["2002"]);
    assert_eq!(p[0].events, [vec!["A"], vec!["B"], vec!["C"]]);
    assert_eq!(p[1].label, ["10:30"]);
    assert_eq!(p[1].events, [vec!["stand:up"]]);
    assert!(p[2].events.is_empty());
    assert_eq!(p[3].label, ["x", "y"]);
    assert!(!t.has_named_sections());
    // A `title` statement wins over frontmatter.
    let t = timeline("---\ntitle: front\n---\ntimeline TD\n  title inner\n");
    assert_eq!(t.title.as_deref(), Some("inner"));
    assert_eq!(t.direction, Direction::TopDown);
    let t = timeline("---\ntitle: front\n---\ntimeline\n");
    assert_eq!(t.title.as_deref(), Some("front"));
}

#[test]
fn sections() {
    let t = timeline(
        "timeline\n  early : e\n  section One\n  a : 1\n  b : 2\n  section Two\n  c : 3\n",
    );
    let names: Vec<Option<Vec<String>>> = t.sections.iter().map(|s| s.name.clone()).collect();
    assert_eq!(
        names,
        [
            None,
            Some(vec!["One".to_owned()]),
            Some(vec!["Two".to_owned()])
        ]
    );
    let counts: Vec<usize> = t.sections.iter().map(|s| s.periods.len()).collect();
    assert_eq!(counts, [1, 2, 1]);
    assert!(t.has_named_sections());
    let sections: Vec<usize> = t.periods().map(|(s, _)| s).collect();
    assert_eq!(sections, [0, 1, 1, 2]);
}

/// TIMELI-2: an event before any period is an error; an empty event is a warning.
#[test]
fn errors() {
    let d = diagram::lower("timeline\n  : orphan\n  a : one :\n").diagnostics;
    let got: Vec<(Severity, &str)> = d.iter().map(|d| (d.severity, d.message.as_str())).collect();
    assert_eq!(
        got,
        [
            (Severity::Error, "an event needs a time period before it"),
            (Severity::Warning, "skipping an empty event"),
        ]
    );
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
    assert_eq!(errors("timeline XY\n"), ["expected the end of the line"]);
    assert!(errors("timeline\n").is_empty());
}

fn center(s: &sirene_core::scene::Scene, want: &str) -> Pt {
    s.elements
        .iter()
        .find_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } if text == want => Some(*center),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {want}"))
}

/// TIMELI-3: periods in a row with events below, sections above; or in a column for `TD`.
#[test]
fn drawing() {
    let src = "timeline\n  section S\n  a : e1 : e2\n  b : f\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let (sec, a, b) = (center(&s, "S"), center(&s, "a"), center(&s, "b"));
    let (e1, e2) = (center(&s, "e1"), center(&s, "e2"));
    assert!(sec.y < a.y && a.x < b.x && (a.y - b.y).abs() < 0.1);
    assert!(a.y < e1.y && e1.y < e2.y && (e1.x - a.x).abs() < 0.1);
    let s = diagram::lower(&src.replace("timeline", "timeline TD"))
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    let (sec, a, b) = (center(&s, "S"), center(&s, "a"), center(&s, "b"));
    let (e1, e2) = (center(&s, "e1"), center(&s, "e2"));
    assert!(sec.x < a.x && a.y < b.y);
    assert!(a.x < e1.x && e1.x < e2.x && (e1.y - a.y).abs() < 0.1);
    // Long text wraps.
    let long = "timeline\n  p : one two three four five six seven eight nine ten eleven twelve\n";
    let s = diagram::lower(long).diagram.scene(&Approx, &Theme::LIGHT);
    let texts = s
        .elements
        .iter()
        .filter(|e| matches!(&e.primitive, Primitive::Text { text, .. } if text.starts_with("one") || text.ends_with("twelve")))
        .count();
    assert_eq!(texts, 2);
}
