//! Mermaid user journeys (MJOUR-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::journey::{self, Journey};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/journey");
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
            assert_eq!(journey::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Journey, "{name}");
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

fn lowered(src: &str) -> (Journey, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Journey(j) => (j, d),
        other => panic!("not a journey: {other:?}"),
    }
}

fn journey(src: &str) -> Journey {
    let (j, d) = lowered(src);
    assert!(d.is_empty(), "{d:?}\n{src}");
    j
}

/// MJOUR-1, MJOUR-2: the title, sections, and tasks with their scores and actors.
#[test]
fn tasks() {
    let j = journey(
        "journey\n  title Day\n  accTitle: at\n  accDescr: ad\n  Wake: 2\n  section Work\n    Tea : 5 : Me, Cat\n    Mail: 2.5:Me\n  section Home\n    Sit: 4: Cat, Dog\n",
    );
    assert_eq!(j.title.as_deref(), Some("Day"));
    assert_eq!(j.acc_title.as_deref(), Some("at"));
    assert_eq!(j.acc_descr.as_deref(), Some("ad"));
    let names: Vec<Option<&str>> = j.sections.iter().map(|s| s.name.as_deref()).collect();
    assert_eq!(names, [None, Some("Work"), Some("Home")]);
    let tasks: Vec<(usize, &str, f64, Vec<&str>)> = j
        .tasks()
        .map(|(s, t)| {
            (
                s,
                t.name.as_str(),
                t.score,
                t.actors.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        tasks,
        [
            (0, "Wake", 2.0, vec![]),
            (1, "Tea", 5.0, vec!["Me", "Cat"]),
            (1, "Mail", 2.5, vec!["Me"]),
            (2, "Sit", 4.0, vec!["Cat", "Dog"]),
        ]
    );
    assert_eq!(j.actors(), ["Me", "Cat", "Dog"]);
    // Frontmatter gives the title when there is no `title`.
    let j = journey("---\ntitle: Front\n---\njourney\n  a: 3\n");
    assert_eq!(j.title.as_deref(), Some("Front"));
}

/// MJOUR-2: a missing or bad score is an error and the task is skipped; a score out of range
/// is clamped, and data after the actors is ignored, each with a warning.
#[test]
fn errors() {
    let (j, d) = lowered("journey\n  a: x: Me\n  b:\n  c: 0\n  d: 3: Me: more\n  section\n");
    assert_eq!(
        d,
        [
            (
                Severity::Error,
                "`x` is not a score; expected a number from 1 to 5".to_owned()
            ),
            (
                Severity::Error,
                "`b` has no score; expected a number from 1 to 5".to_owned()
            ),
            (
                Severity::Warning,
                "`0` is outside the scores 1 to 5; drawing it as 1".to_owned()
            ),
            (
                Severity::Warning,
                "ignoring `: more` after the actors".to_owned()
            ),
            (Severity::Error, "expected a section name".to_owned()),
        ]
    );
    let scores: Vec<f64> = j.tasks().map(|(_, t)| t.score).collect();
    assert_eq!(scores, [1.0, 3.0]);
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
        errors("journey\n  Make tea\n"),
        ["expected `:` and a score after the task name"]
    );
    assert_eq!(
        errors("journey\n  : 5\n"),
        ["expected a task (`name: score: actors`), `section`, or `title`"]
    );
    assert!(errors("journey\n").is_empty());
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

/// MJOUR-3: the legend at the left, sections above their tasks, and faces lower for lower
/// scores.
#[test]
fn drawing() {
    let src = "journey\n  title Day\n  section Work\n    Tea: 5: Me\n    Mail: 1: Me, Cat\n  section Home\n    Sit: 3: Cat\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| {
        t.iter()
            .find(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .unwrap_or_else(|| panic!("no {want} in {t:?}"))
    };
    let (title, me, cat) = (at("Day"), at("Me"), at("Cat"));
    let (work, home, tea, mail, sit) = (at("Work"), at("Home"), at("Tea"), at("Mail"), at("Sit"));
    assert!(title.y < work.y && work.y < tea.y);
    assert!(me.x < tea.x && me.y < cat.y);
    assert!(tea.x < mail.x && mail.x < sit.x && (tea.y - sit.y).abs() < 0.01);
    // A section spans its tasks.
    assert!((work.x - (tea.x + mail.x) / 2.0).abs() < 0.01);
    assert!((home.x - sit.x).abs() < 0.01);
    // Faces, by their cream fill: one per task, lower for lower scores.
    let faces: Vec<Pt> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Path {
                segments,
                fill: Some(f),
                ..
            } if (f.r, f.g, f.b) == (0xff, 0xf8, 0xdc) => match segments.first() {
                Some(sirene_core::scene::Segment::Move(p)) => Some(*p),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(faces.len(), 3);
    assert!(faces[0].y < faces[2].y && faces[2].y < faces[1].y);
    for (_, p) in &t {
        assert!(p.x > 0.0 && p.x < s.width && p.y > 0.0 && p.y < s.height);
    }
    // No tasks: only the title.
    let s = diagram::lower("journey\n  title T\n")
        .diagram
        .scene(&Approx, &Theme::DARK);
    assert_eq!(texts(&s).len(), 1);
}
