//! Mermaid sequence diagrams (MSEQ-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};
use sirene_core::sequence::{
    self, BlockKind, Event, Head, LineStyle, NoteSide, ParticipantKind, Sequence,
};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sequence");
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
            assert_eq!(sequence::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Sequence, "{name}");
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
                assert!(s.width > 0.0 && s.height > 0.0);
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

fn sequence(src: &str) -> Sequence {
    let lowered = diagram::lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    match lowered.diagram {
        Diagram::Sequence(s) => s,
        other => panic!("not a sequence diagram: {other:?}"),
    }
}

fn errors(src: &str) -> Vec<(Severity, String)> {
    diagram::lower(src)
        .diagnostics
        .into_iter()
        .map(|d| (d.severity, d.message))
        .collect()
}

/// MSEQ-1, MSEQ-2: participants in order of declaration or first mention, with aliases and
/// kinds.
#[test]
fn participants() {
    let s = sequence(
        "sequenceDiagram\n  participant B as Bob\n  actor A\n  participant D@{ \"type\": \"database\" }\n  \
         participant E@{ \"type\": \"entity\", \"alias\": \"Things\" }\n  C->>B: hi\n  A->>C: x<br>y\n",
    );
    let ids: Vec<&str> = s.participants.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["B", "A", "D", "E", "C"]);
    assert_eq!(s.participants[0].label, ["Bob"]);
    assert_eq!(s.participants[1].kind, ParticipantKind::Actor);
    assert_eq!(s.participants[2].kind, ParticipantKind::Database);
    assert_eq!(s.participants[3].kind, ParticipantKind::Entity);
    assert_eq!(s.participants[3].label, ["Things"]);
    assert_eq!(s.participants[4].kind, ParticipantKind::Participant);
    let m: Vec<_> = s.messages().collect();
    assert_eq!((m[0].from, m[0].to), (4, 0));
    assert_eq!(m[1].text, ["x", "y"]);
    // Ids may hold spaces and dashes.
    let s = sequence("sequenceDiagram\n  Web Server->>my-db: q\n");
    let ids: Vec<&str> = s.participants.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["Web Server", "my-db"]);
}

/// MSEQ-1: each arrow's line and head.
#[test]
fn arrows() {
    let s = sequence("sequenceDiagram\n  A-->>B: a\n  A-xB: b\n  A<<->>B: c\n  A--)B: d\n");
    let arrows: Vec<_> = s
        .messages()
        .map(|m| (m.arrow.line, m.arrow.head, m.arrow.both))
        .collect();
    assert_eq!(
        arrows,
        [
            (LineStyle::Dotted, Head::Filled, false),
            (LineStyle::Solid, Head::Cross, false),
            (LineStyle::Solid, Head::Filled, true),
            (LineStyle::Dotted, Head::Open, false),
        ]
    );
}

/// MSEQ-2: activations by `+`/`-` and by statement; unbalanced ones are diagnostics.
#[test]
fn activations() {
    let s = sequence("sequenceDiagram\n  A->>+B: a\n  B-->>-A: b\n  activate A\n  deactivate A\n");
    let m: Vec<_> = s.messages().collect();
    assert!(m[0].activate && !m[0].deactivate);
    assert!(m[1].deactivate && !m[1].activate);
    assert!(s.events.contains(&Event::Activate(0)));
    assert!(s.events.contains(&Event::Deactivate(0)));
    assert_eq!(
        errors("sequenceDiagram\n  B-->>-A: b\n  deactivate A\n  activate C\n"),
        [
            (Severity::Error, "`B` is not active".to_owned()),
            (Severity::Error, "`A` is not active".to_owned()),
            (
                Severity::Warning,
                "`C` is still active at the end".to_owned()
            ),
        ]
    );
}

/// MSEQ-1, MSEQ-2: blocks nest, dividers belong to their block, and every block is closed.
#[test]
fn blocks() {
    let s = sequence(
        "sequenceDiagram\n  alt yes\n  A->>B: 1\n  else no\n  loop\n  B->>A: 2\n  end\n  end\n",
    );
    assert_eq!(
        s.events
            .iter()
            .filter(|e| !matches!(e, Event::Message(_)))
            .collect::<Vec<_>>(),
        [
            &Event::Start(BlockKind::Alt, "yes".to_owned()),
            &Event::Divider("no".to_owned()),
            &Event::Start(BlockKind::Loop, String::new()),
            &Event::End,
            &Event::End,
        ]
    );
    assert_eq!(
        errors("sequenceDiagram\n  loop x\n  and y\n  end\n  end\n  opt z\n"),
        [
            (
                Severity::Error,
                "`and` belongs in an `par` block".to_owned()
            ),
            (
                Severity::Error,
                "`end` without a block or `box` to close".to_owned()
            ),
            (Severity::Error, "unclosed `opt`: expected `end`".to_owned()),
        ]
    );
    let d = diagram::lower("sequenceDiagram\n  opt z\n");
    let Diagram::Sequence(s) = d.diagram else {
        panic!()
    };
    assert_eq!(s.events.last(), Some(&Event::End));
}

/// MSEQ-1: notes, `autonumber`, `box`, `create`, `destroy`, and the title.
#[test]
fn statements() {
    let s = sequence(
        "sequenceDiagram\n  title: T\n  box Purple Team\n  participant A\n  end\n  autonumber 5 2\n  \
         note over A,B: n\n  note right of A: r\n  A->>B: one\n  A->>B: two\n  autonumber off\n  A->>B: three\n  \
         create actor C\n  A->>C: new\n  destroy C\n  C-xA: gone\n",
    );
    assert_eq!(s.title.as_deref(), Some("T"));
    assert_eq!(s.groups.len(), 1);
    assert_eq!(s.groups[0].color.as_deref(), Some("Purple"));
    assert_eq!(s.groups[0].title.as_deref(), Some("Team"));
    assert_eq!(s.participants[0].group, Some(0));
    assert_eq!(s.participants[1].group, None);
    let numbers: Vec<Option<u32>> = s.messages().map(|m| m.number).collect();
    assert_eq!(numbers, [Some(5), Some(7), None, None, None]);
    let notes: Vec<(NoteSide, Vec<usize>)> = s
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Note(n) => Some((n.side, n.participants.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        notes,
        [(NoteSide::Over, vec![0, 1]), (NoteSide::Right, vec![0])]
    );
    assert!(s.events.contains(&Event::Create(2)));
    assert!(s.events.contains(&Event::Destroy(2)));
    assert_eq!(s.participants[2].kind, ParticipantKind::Actor);
    assert_eq!(
        errors("sequenceDiagram\n  A->>B: x\n  create participant B\n"),
        [(
            Severity::Error,
            "`B` already exists; `create` comes before its first mention".to_owned()
        )]
    );
    assert_eq!(
        errors("sequenceDiagram\n  participant X@{ \"type\": \"cloud\" }\n  links A: {}\n"),
        [
            (
                Severity::Warning,
                "unknown participant type `cloud`; drawing a box".to_owned()
            ),
            (
                Severity::Warning,
                "`links` is not supported in sequence diagrams; ignoring it".to_owned()
            ),
        ]
    );
}

fn center(s: &Scene, want: &str) -> Pt {
    s.elements
        .iter()
        .find_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } if text == want => Some(*center),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {want}"))
}

/// MSEQ-3: participants are columns, messages go down in order, and headers repeat at the
/// bottom.
#[test]
fn drawing() {
    let src = "sequenceDiagram\n  Alice->>Bob: first\n  Bob-->>Alice: second\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let (alice, bob) = (center(&s, "Alice"), center(&s, "Bob"));
    let (first, second) = (center(&s, "first"), center(&s, "second"));
    assert!(alice.x < bob.x && (alice.y - bob.y).abs() < 0.1);
    assert!(first.y < second.y && first.y > alice.y);
    assert!(first.x > alice.x && first.x < bob.x);
    let names = s
        .elements
        .iter()
        .filter(|e| matches!(&e.primitive, Primitive::Text { text, .. } if text == "Alice"))
        .count();
    assert_eq!(names, 2);
    // A long label pushes the columns apart.
    let wide = diagram::lower("sequenceDiagram\n  Alice->>Bob: a much longer label than before\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert!(center(&wide, "Bob").x - center(&wide, "Alice").x > bob.x - alice.x);
}

#[test]
fn ascii_drawing() {
    let out = diagram::lower("sequenceDiagram\n  A->>B: hi\n  B-->>A: yo\n")
        .diagram
        .ascii(&Options {
            charset: Charset::Unicode,
        });
    assert_eq!(
        out,
        "┌───┐   ┌───┐\n\
         │ A │   │ B │\n\
         └─┬─┘   └─┬─┘\n\
         \x20 │       │\n\
         \x20 │  hi   │\n\
         \x20 ├──────►│\n\
         \x20 │       │\n\
         \x20 │  yo   │\n\
         \x20 │◄┄┄┄┄┄┄┤\n\
         \x20 │       │\n\
         ┌─┴─┐   ┌─┴─┐\n\
         │ A │   │ B │\n\
         └───┘   └───┘\n"
    );
}
