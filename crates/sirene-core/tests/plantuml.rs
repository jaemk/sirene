//! PlantUML sequence diagrams (PSEQ-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the sequence model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lang, Lowered};
use sirene_core::flowchart::{Flowchart, Shape, Stroke};
use sirene_core::plantuml;
use sirene_core::scene::{Approx, Theme};
use sirene_core::sequence::{BlockKind, Event, Head, LineStyle, ParticipantKind, Sequence};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plantuml");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "puml"))
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
            assert_eq!(plantuml::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::PlantUml, "{name}");
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

fn diags(src: &str) -> Vec<(Severity, String)> {
    diagram::lower(src)
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect()
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
                    "{}{}{}:{}",
                    id(m.from),
                    if m.arrow.line == LineStyle::Dotted {
                        "~>"
                    } else {
                        ">"
                    },
                    id(m.to),
                    m.text.join(" ")
                ) + if m.activate { " *" } else { "" }
                    + if m.deactivate { " -" } else { "" }
            }
            Event::Activate(p) => format!("activate {}", id(*p)),
            Event::Deactivate(p) => format!("deactivate {}", id(*p)),
            Event::Create(p) => format!("create {}", id(*p)),
            Event::Destroy(p) => format!("destroy {}", id(*p)),
            Event::Start(k, l) => format!("{} {l}", k.keyword()),
            Event::Divider(l) => format!("divider {l}"),
            Event::End => "end".to_owned(),
            Event::Separator(t) => format!("separator {t}"),
            Event::Delay(t) => format!("delay {}", t.as_deref().unwrap_or("")),
            Event::Space(n) => format!("space {n:?}"),
            other => format!("{other:?}"),
        })
        .collect()
}

fn ids(s: &Sequence) -> Vec<&str> {
    s.participants.iter().map(|p| p.id.as_str()).collect()
}

/// PSEQ-1: text before `@startuml` and a second document are ignored; a missing `@enduml` is a
/// warning; other `@start` kinds and other diagram kinds are errors.
#[test]
fn documents() {
    let (s, d) = lowered("preamble\n@startuml name\nA -> B\n@enduml\n@startuml\nC -> D\n@enduml\n");
    assert_eq!(ids(&s), ["A", "B"]);
    assert_eq!(
        d,
        [(
            Severity::Warning,
            "only the first diagram in a file is drawn".to_owned()
        )]
    );
    let (s, d) = lowered("@startuml\nA -> B\n");
    assert_eq!(s.messages().count(), 1);
    assert_eq!(
        d,
        [(
            Severity::Warning,
            "missing `@enduml`; the diagram runs to the end of the file".to_owned()
        )]
    );
    let d = diags("@startsalt\n{}\n@endsalt\n");
    assert_eq!(d[0].0, Severity::Error);
    assert!(d[0].1.contains("`@startsalt`"), "{d:?}");
    // Every kind detected inside `@startuml` is drawn, timing diagrams among them.
    assert!(diags("@startuml\nrobust W\n@enduml\n").is_empty());
    // Keywords match in any case; comments are skipped.
    let (s, d) = lowered("@startuml\n' c\nACTOR A\n/' x\ny '/\nA -> A : self\n@enduml\n");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(s.participants[0].kind, ParticipantKind::Actor);
}

/// PSEQ-2: kinds, aliases in both orders, implicit participants, and `order`.
#[test]
fn participants() {
    let (s, d) = lowered(
        "@startuml\nqueue Q order 20\nparticipant \"Long name\" as L\nparticipant M as \"Other\"\n\
         collections C order 10\nL -> N\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    // Unordered participants follow the ordered one declared before them.
    assert_eq!(ids(&s), ["C", "N", "Q", "L", "M"]);
    assert_eq!(s.participants[0].kind, ParticipantKind::Collections);
    assert_eq!(s.participants[1].kind, ParticipantKind::Participant);
    assert_eq!(s.participants[2].kind, ParticipantKind::Queue);
    assert_eq!(s.participants[3].label, ["Long name"]);
    assert_eq!(s.participants[4].label, ["Other"]);
}

/// PSEQ-3: arrow lines and heads, reversed and two-headed arrows, and the edges.
#[test]
fn messages() {
    let (s, d) = lowered(
        "@startuml\nA -> B : a\nA --> B : b\nA ->> B : c\nA ->x B : d\nA <- B : e\n\
         A <-> B : f\n[-> A : g\nA ->] : h\n?-> B : i\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(ids(&s), ["[", "A", "B", "]"]);
    assert_eq!(s.participants[0].kind, ParticipantKind::Hidden);
    assert_eq!(s.participants[3].kind, ParticipantKind::Hidden);
    assert_eq!(
        events(&s),
        [
            "A>B:a", "A~>B:b", "A>B:c", "A>B:d", "B>A:e", "A>B:f", "[>A:g", "A>]:h", "[>B:i",
        ]
    );
    let arrows: Vec<(Head, bool)> = s.messages().map(|m| (m.arrow.head, m.arrow.both)).collect();
    assert_eq!(arrows[0], (Head::Filled, false));
    assert_eq!(arrows[2], (Head::Open, false));
    assert_eq!(arrows[3], (Head::Cross, false));
    assert_eq!(arrows[5], (Head::Filled, true));
}

/// PSEQ-3: activation modifiers, `return`, explicit activations, creation and destruction.
#[test]
fn activations() {
    let (s, d) = lowered(
        "@startuml\nA -> B ++ : call\nB -> C ++ : inner\nreturn r1\nreturn r2\n\
         create D\nA -> D : new\nA -> D !! : bye\nA -> E ** : make\nactivate E\ndeactivate E\n\
         destroy E\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        events(&s),
        [
            "A>B:call *",
            "B>C:inner *",
            "C~>B:r1 -",
            "B~>A:r2 -",
            "create D",
            "A>D:new",
            "destroy D",
            "A>D:bye",
            "create E",
            "A>E:make",
            "activate E",
            "deactivate E",
            "destroy E",
        ]
    );
}

/// PSEQ-3: `autonumber` with a start and step, stopped and resumed.
#[test]
fn autonumber() {
    let (s, d) = lowered(
        "@startuml\nautonumber 10 5\nA -> B\nA -> B\nautonumber stop\nA -> B\n\
         autonumber resume\nA -> B\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    let numbers: Vec<Option<u32>> = s.messages().map(|m| m.number).collect();
    assert_eq!(numbers, [Some(10), Some(15), None, Some(20)]);
}

/// PSEQ-4: frames, `group` labels, separators, delays, spaces, and refs.
#[test]
fn frames() {
    let (s, d) = lowered(
        "@startuml\nA -> B\nalt one\nA -> B\nelse two\nend\ngroup retry [3 times]\nend\n\
         == sep ==\n...\n... later ...\n|||\n||45||\nref over A, B : r\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        events(&s),
        [
            "A>B:",
            "alt one",
            "A>B:",
            "divider two",
            "end",
            "group retry [3 times]",
            "end",
            "separator sep",
            "delay ",
            "delay later",
            "space None",
            "space Some(45)",
            "Ref { participants: [0, 1], text: [\"r\"] }",
        ]
    );
    assert_eq!(
        BlockKind::Group.tab("retry [3 times]"),
        ("retry".to_owned(), "[3 times]".to_owned())
    );
}

/// PSEQ-4: note placements, including beside the previous message and across.
#[test]
fn notes() {
    let (s, d) = lowered(
        "@startuml\nA -> B : m\nnote left : l\nnote right : r\nnote over A, B\n  x\n  y\n\
         end note\nnote across : all\nhide footbox\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert!(s.hide_footbox);
    let notes: Vec<String> = s
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Note(n) => Some(format!(
                "{:?} {:?} {}",
                n.side,
                n.participants,
                n.text.join("|")
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        notes,
        [
            "Left [0] l",
            "Right [1] r",
            "Over [0, 1] x|y",
            "Over [0, 1] all"
        ]
    );
}

/// PSEQ-6: what is not drawn is a warning, files from elsewhere and unknown lines are errors.
#[test]
fn errors() {
    let (s, d) = lowered(
        "@startuml\n!include x.puml\n!include_many y.puml\n!include_once z.puml\n!define X\n\
         !$a = 1\nskinparam a b\nparticipant A #red\n\
         A -[#blue]> B\nA ->o B\nreturn x\nnewpage\nhello there\n@enduml\n",
    );
    let messages: Vec<&str> = d.iter().map(|(_, m)| m.as_str()).collect();
    assert_eq!(
        messages,
        [
            "`!include` reads other files and is not supported",
            "`!include_many` reads other files and is not supported",
            "`!include_once` reads other files and is not supported",
            "`!define` is not supported; ignoring the line",
            "`!$a` is not supported; ignoring the line",
            "`skinparam` is not drawn yet; ignoring it",
            "participant colors are not drawn; ignoring `#red`",
            "arrow styles are not drawn; drawing a plain arrow",
            "`o` arrow ends are drawn as filled heads",
            "`return` needs a participant activated by a message",
            "`newpage` is not drawn yet; ignoring it",
            "expected a message such as `A -> B : text`, or a statement such as `participant`, \
             `note`, or `alt`",
        ],
        "{d:?}"
    );
    assert_eq!(s.messages().count(), 2);
}

/// PSEQ-7: the `@start` line, extensions, Markdown names, and `--lang` choose PlantUML.
#[test]
fn choosing() {
    assert!(plantuml::is_plantuml("\u{feff}\n  @startuml\n"));
    assert!(!plantuml::is_plantuml("sequenceDiagram\n"));
    assert_eq!(Kind::of("@startuml\nA -> B\n@enduml\n"), Kind::PlantUml);
    for ext in ["a.puml", "a.plantuml", "a.pu", "a.iuml"] {
        assert_eq!(Lang::of_path(Path::new(ext)), Some(Lang::PlantUml), "{ext}");
    }
    assert_eq!(Lang::of_info("plantuml"), Some(Lang::PlantUml));
    assert_eq!(Lang::of_info("puml"), Some(Lang::PlantUml));
    let lowered = diagram::lower_as("A -> B\n", Some(Lang::PlantUml));
    assert!(matches!(lowered.diagram, Diagram::Sequence(_)));
}

fn graph(src: &str) -> (Flowchart, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = diags(src);
    match lowered.diagram {
        Diagram::Graph(g) => (g, d),
        other => panic!("not a graph: {other:?}"),
    }
}

/// Edges as `from>to:label` with node labels for ids.
fn flow(g: &Flowchart) -> Vec<String> {
    let name = |id: &str| g.node(id).map_or(id.to_owned(), |n| n.label.text.clone());
    g.edges
        .iter()
        .map(|e| {
            let label = e
                .label
                .as_ref()
                .map_or(String::new(), |l| format!(":{}", l.text));
            format!("{}>{}{label}", name(&e.from), name(&e.to))
        })
        .collect()
}

/// PACT-2..6: actions, decisions, loops, and arrows lower to a flowchart.
#[test]
fn activities() {
    let (g, d) = graph(
        "@startuml\nstart\n:a;\nif (c?) then (yes)\n:b;\nelse (no)\n:x;\nendif\n\
         while (w?) is (more)\n:l;\nendwhile (done)\n-[#red,dashed]-> go;\n:z>\nstop\n@enduml\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        flow(&g),
        [
            ">a",
            "a>c?",
            "c?>b:yes",
            "c?>x:no",
            "b>w?",
            "x>w?",
            "w?>l:more",
            "l>w?",
            "w?>z:go",
            "z>",
        ]
    );
    let shape = |label: &str| {
        g.nodes
            .iter()
            .find(|n| n.label.text == label)
            .map(|n| n.shape)
    };
    assert_eq!(shape("a"), Some(Shape::Rounded));
    assert_eq!(shape("c?"), Some(Shape::Diamond));
    assert_eq!(shape("z"), Some(Shape::LeanRight));
    let last = g
        .edges
        .iter()
        .find(|e| g.node(&e.to).is_some_and(|n| n.label.text == "z"));
    assert_eq!(last.map(|e| e.stroke), Some(Stroke::Dotted));
}

/// PACT-7, PACT-9: lanes and partitions become subgraphs; misuse is reported.
#[test]
fn activity_lanes_and_errors() {
    let (g, d) = graph("@startuml\n|A|\nstart\n|B|\npartition P {\n:x;\n}\nstop\n@enduml\n");
    assert!(d.is_empty(), "{d:?}");
    assert!(g.lanes);
    let titles: Vec<&str> = g.subgraphs.iter().map(|s| s.title.text.as_str()).collect();
    assert!(
        titles.contains(&"A") && titles.contains(&"B") && titles.contains(&"P"),
        "{titles:?}"
    );
    let (_, d) = graph("@startuml\nstart\nendif\nwhile (x)\n:a;\n@enduml\n");
    assert_eq!(d.len(), 2, "{d:?}");
    assert!(d.iter().all(|(s, _)| *s == Severity::Error), "{d:?}");
}
