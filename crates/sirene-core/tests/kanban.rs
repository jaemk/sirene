//! Mermaid kanban boards (MKAN-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each line builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::kanban::{self, Kanban, Priority};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kanban");
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
            assert_eq!(kanban::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Kanban, "{name}");
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

fn lowered(src: &str) -> (Kanban, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Kanban(k) => (k, d),
        other => panic!("not a kanban board: {other:?}"),
    }
}

fn board(src: &str) -> Kanban {
    let (k, d) = lowered(src);
    assert!(d.is_empty(), "{d:?}\n{src}");
    k
}

/// MKAN-1, MKAN-2: columns at the first node's indent, cards under them, and card metadata.
#[test]
fn columns_and_cards() {
    let k = board(
        "---\ntitle: Board\nconfig:\n  kanban:\n    ticketBaseUrl: 'https://t/#TICKET#'\n---\nkanban\n  todo[To do]\n    a[First]@{ ticket: T-1, assigned: 'ann', priority: 'very high' }\n    Second\n      deeper[Still a card]\n  [Done]\n    b[Label]@{ label: 'Relabeled' }\n    :::urgent big\n  empty\n",
    );
    assert_eq!(k.title.as_deref(), Some("Board"));
    assert_eq!(k.ticket_url("T-1").as_deref(), Some("https://t/T-1"));
    let cols: Vec<(Option<&str>, &str, usize)> = k
        .columns
        .iter()
        .map(|c| (c.id.as_deref(), c.label.text.as_str(), c.cards.len()))
        .collect();
    assert_eq!(
        cols,
        [
            (Some("todo"), "To do", 3),
            (None, "Done", 1),
            (Some("empty"), "empty", 0)
        ]
    );
    let a = &k.columns[0].cards[0];
    assert_eq!(a.id.as_deref(), Some("a"));
    assert_eq!(a.label.text, "First");
    assert_eq!(
        (a.ticket.as_deref(), a.assigned.as_deref(), a.priority),
        (Some("T-1"), Some("ann"), Some(Priority::VeryHigh))
    );
    assert_eq!(k.columns[0].cards[1].label.text, "Second");
    assert_eq!(k.columns[0].cards[2].label.text, "Still a card");
    let b = &k.columns[1].cards[0];
    assert_eq!(b.label.text, "Relabeled");
    assert_eq!(b.classes, ["urgent", "big"]);
}

/// MKAN-2: a node left of the first column is an error and left out; unknown metadata and
/// priorities, and icons, are warnings.
#[test]
fn errors() {
    let (k, d) = lowered(
        "kanban\n    col\n      a@{ priority: 'Medium', size: 2 }\n      ::icon(x)\n  left\n",
    );
    let w = |s: &str| (Severity::Warning, s.to_owned());
    assert_eq!(
        d,
        [
            w("ignoring priority `Medium`: expected `Very High`, `High`, `Low`, or `Very Low`"),
            w(
                "ignoring `size`: a card's metadata is `assigned`, `ticket`, `priority`, `label`, and `icon`"
            ),
            w("icons are not drawn; ignoring `x`"),
            (
                Severity::Error,
                "`left` is indented less than the first column; indent columns alike and their cards under them"
                    .to_owned()
            ),
        ]
    );
    assert_eq!(k.columns.len(), 1);
    assert_eq!(k.columns[0].cards[0].icon.as_deref(), Some("x"));
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
        errors("kanban\n  a\n    b[x]@{ k: v\n"),
        ["unclosed `@{`: expected `}`"]
    );
    assert_eq!(errors("kanbans\n").len(), 1);
    assert!(errors("kanban\n").is_empty());
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

/// MKAN-3: columns side by side, cards down each under its title, with ticket and assignee.
#[test]
fn drawing() {
    let src = "kanban\n  todo[To do]\n    a[One]@{ ticket: T-1, assigned: ann, priority: High }\n    b[Two]\n  done[Done]\n    c[Three]\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| {
        t.iter()
            .find(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .unwrap_or_else(|| panic!("no {want} in {t:?}"))
    };
    let (todo, done, one, two, three) =
        (at("To do"), at("Done"), at("One"), at("Two"), at("Three"));
    assert!(todo.x < done.x && (todo.y - done.y).abs() < 0.01);
    assert!(todo.y < one.y && one.y < two.y);
    assert!(three.x > two.x && (three.y - one.y).abs() < 0.01);
    let (ticket, ann) = (at("T-1"), at("ann"));
    assert!(ticket.y > one.y && ticket.x < ann.x && (ticket.y - ann.y).abs() < 0.01);
    assert!(ann.y < two.y);
    // The priority stripe.
    let orange = sirene_core::scene::Color::rgb(0xf08c00);
    assert!(s.elements.iter().any(|e| matches!(
        &e.primitive,
        Primitive::Path { fill: Some(f), .. } if *f == orange
    )));
    for (_, p) in &t {
        assert!(p.x > 0.0 && p.x < s.width && p.y > 0.0 && p.y < s.height);
    }
}
