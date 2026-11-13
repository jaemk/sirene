//! ZenUML sequence diagrams (MZEN-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the sequence model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::scene::{Approx, Primitive, Theme};
use sirene_core::sequence::{BlockKind, Event, Head, LineStyle, ParticipantKind, Sequence};
use sirene_core::zenuml;

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zenuml");
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
            assert_eq!(zenuml::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::ZenUml, "{name}");
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

fn lowered(src: &str) -> (Sequence, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Sequence(s) => (s, d),
        other => panic!("not a sequence diagram: {other:?}"),
    }
}

/// A compact view of the events: messages as `from>to:text` with `~` for a dotted line and
/// `*` for an activation, and the other events by name.
fn events(s: &Sequence) -> Vec<String> {
    let id = |p: usize| s.participants[p].id.clone();
    s.events
        .iter()
        .map(|e| match e {
            Event::Message(m) => {
                format!(
                    "{}{}{}{}:{}",
                    if m.from == m.to { "self " } else { "" },
                    id(m.from),
                    if m.arrow.line == LineStyle::Dotted {
                        "~>"
                    } else {
                        ">"
                    },
                    id(m.to),
                    m.text.join(" ")
                ) + if m.activate { " *" } else { "" }
            }
            Event::Deactivate(p) => format!("deactivate {}", id(*p)),
            Event::Create(p) => format!("create {}", id(*p)),
            Event::Start(k, l) => format!("{} {l}", k.keyword()),
            Event::Divider(l) => format!("divider {l}"),
            Event::End => "end".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// MZEN-1, MZEN-2: participants with annotators and aliases, async messages and replies, sync
/// calls with bodies and replies, creations, and the implicit starter.
#[test]
fn model() {
    let (s, d) = lowered(
        "zenuml\ntitle T\n@Actor A\nB as Bob\nA->B: hi\n@return\nB->A: ok\n\
         x = B.get(1) {\n  B.check()\n  new C()\n}\nB.put() {\n  return done\n}\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(s.title.as_deref(), Some("T"));
    let ids: Vec<&str> = s.participants.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["", "A", "B", "C"]);
    assert_eq!(s.participants[0].kind, ParticipantKind::Hidden);
    assert_eq!(s.participants[1].kind, ParticipantKind::Actor);
    assert_eq!(s.participants[2].label, ["Bob"]);
    assert_eq!(
        events(&s),
        [
            "A>B:hi",
            "B~>A:ok",
            ">B:get(1) *",
            "self B>B:check() *",
            "deactivate B",
            "create C",
            "B>C:<<create>> *",
            "deactivate C",
            "B~>:x",
            ">B:put() *",
            "B~>:done",
            "deactivate B",
        ]
    );
    let heads: Vec<Head> = s.messages().map(|m| m.arrow.head).collect();
    assert_eq!(heads[0], Head::Open);
    assert_eq!(heads[2], Head::Filled);
    // The reply to an assignment ends the activation.
    assert!(s.messages().nth(5).is_some_and(|m| m.deactivate));
}

/// MZEN-2: without calls from the starter there is no hidden participant; `@Starter` names it.
#[test]
fn starters() {
    let (s, _) = lowered("zenuml\nA->B: hi\n");
    assert_eq!(s.participants.len(), 2);
    assert_eq!(events(&s), ["A>B:hi"]);
    let (s, _) = lowered("zenuml\n@Starter(Client)\nA.m()\nC->A.n()\n");
    let ids: Vec<&str> = s.participants.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["Client", "A", "C"]);
    assert_eq!(
        events(&s),
        [
            "Client>A:m() *",
            "deactivate A",
            "C>A:n() *",
            "deactivate A"
        ]
    );
}

/// MZEN-2: fragments become blocks, with `else` and `catch` / `finally` dividers.
#[test]
fn fragments() {
    let (s, d) = lowered(
        "zenuml\nA->B: x\nif(a) {\n  A->B: 1\n} else if(b) {\n} else {\n}\nforEach(i) {\n}\n\
         opt {\n}\npar {\n}\ntry {\n} catch(E) {\n} finally {\n}\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        events(&s),
        [
            "A>B:x",
            "alt a",
            "A>B:1",
            "divider b",
            "divider ",
            "end",
            "loop i",
            "end",
            "opt ",
            "end",
            "par ",
            "end",
            "try ",
            "divider catch E",
            "divider finally",
            "end",
        ]
    );
    assert_eq!(BlockKind::Try.keyword(), "try");
}

/// MZEN-1: the next keyword may start the line after a branch's `}`.
#[test]
fn next_keyword_on_later_line() {
    let (s, d) = lowered(
        "zenuml\nif(a) {\n  A->B: 1\n}\nelse if(b) {\n}\n\n  else {\n}\n\
         try {\n}\ncatch(E) {\n}\nfinally {\n}\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        events(&s),
        [
            "alt a",
            "A>B:1",
            "divider b",
            "divider ",
            "end",
            "try ",
            "divider catch E",
            "divider finally",
            "end",
        ]
    );
}

/// MZEN-2: what is drawn as near as it can be, with a diagnostic.
#[test]
fn errors() {
    let (s, d) = lowered("zenuml\n@EC2 S\nreturn x\n@return\nA.m()\nA.n(\nif(x)\n");
    let w = |m: &str| (Severity::Warning, m.to_owned());
    let e = |m: &str| (Severity::Error, m.to_owned());
    assert_eq!(
        d,
        [
            w("`@EC2` is not drawn; drawing a participant"),
            w("`return` outside a call; ignoring it"),
            w("`@return` is for the next message such as `A->B: x`; ignoring it"),
            e("expected `)` to close the arguments"),
            e("expected `{`"),
        ]
    );
    assert_eq!(s.participants[1].kind, ParticipantKind::Participant);
    assert_eq!(events(&s), [">A:m() *", "deactivate A"]);
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
        errors("zenuml\nA.m() {\n  B.n()"),
        ["unclosed block: expected `}`"]
    );
    // The statement before an unclosed block's end is still drawn.
    let (s, _) = lowered("zenuml\nA.m() {\n  B.n()");
    assert_eq!(s.messages().count(), 2);
    assert!(errors("zenuml\n").is_empty());
}

/// MZEN-3: the starter has no header, and its calls come in from the left.
#[test]
fn drawing() {
    let src = "zenuml\nA.m()\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let texts: Vec<&str> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts.iter().filter(|t| **t == "A").count(), 2);
    assert!(texts.contains(&"m()"));
    let ascii = diagram::lower(src).diagram.ascii(&Options {
        charset: Charset::Ascii,
    });
    assert!(
        ascii
            .lines()
            .any(|l| l.starts_with("--") && l.contains('>')),
        "{ascii}"
    );
}
