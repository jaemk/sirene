//! Property tests for PlantUML sequence diagrams: the tree holds any source exactly (PSEQ-1),
//! and any source lowers to a well-formed sequence model that draws.

use proptest::prelude::*;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::Diagram;
use sirene_core::scene::{Approx, Theme};
use sirene_core::sequence::{Event, Sequence};
use sirene_core::{TextSize, plantuml};

/// A sequence diagram's participant indices are in range, a graph's edges join its nodes, and
/// the diagram draws in both renderers.
fn check(d: &Diagram) -> Result<(), TestCaseError> {
    match d {
        Diagram::Sequence(s) => check_sequence(s)?,
        Diagram::Graph(g) => {
            for e in &g.edges {
                let known =
                    |id: &str| g.node(id).is_some() || g.subgraphs.iter().any(|s| s.id == id);
                prop_assert!(known(&e.from) && known(&e.to), "{e:?}");
            }
        }
        _ => {}
    }
    for charset in [Charset::Unicode, Charset::Ascii] {
        d.ascii(&Options { charset });
    }
    d.scene(&Approx, &Theme::LIGHT);
    Ok(())
}

fn check_sequence(s: &Sequence) -> Result<(), TestCaseError> {
    let n = s.participants.len();
    for e in &s.events {
        let indices: Vec<usize> = match e {
            Event::Message(m) => vec![m.from, m.to],
            Event::Note(note) => note.participants.clone(),
            Event::Ref { participants, .. } => participants.clone(),
            Event::Activate(p) | Event::Deactivate(p) | Event::Create(p) | Event::Destroy(p) => {
                vec![*p]
            }
            _ => vec![],
        };
        for p in indices {
            prop_assert!(p < n, "{p} >= {n}: {e:?}");
        }
    }
    Ok(())
}

/// Pieces biased toward PlantUML syntax, so the parser's and the lowering's branches get
/// exercised.
fn puml_ish() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    let pieces = prop::sample::select(vec![
        "@startuml", "@enduml", "@startmindmap", "\n", "\r\n", " ", " ", "\t", "'", "/'", "'/",
        "participant ", "actor ", "database ", "queue ", "as ", "order ", "1", "-5", "\"", "\"x\"",
        "A", "B", "C", "[", "]", "?", "->", "-->", "<-", "<->", "->>", "->x", "x->", "o->", "->o",
        "-[#red]>", "-\\", "//", "++", "--", "**", "!!", ":", " : text", "note ", "hnote ",
        "left ", "right ", "over ", "of ", "across", "end note", "ref ", "end ref", "alt",
        "else", "opt", "loop", "group ", "[x]", "end", "box ", "end box", "==", "...", "|||",
        "||40||", "||99999999999||", "return", "activate ", "deactivate ", "create ",
        "destroy ", "autonumber", " stop", " resume", "hide footbox", "title", "end title",
        "legend", "endlegend", "skinparam", "{", "}", "!include", "!define", "#red", "<<s>>",
        "class", "é", "🐟", "\\n", "start", "stop", ":", ";", ":act;", "if (", ") then (",
        "elseif", "endif", "while", "endwhile", "repeat", "repeat while", "backward",
        "fork", "fork again", "end fork", "split", "end split", "switch", "case", "endswitch",
        "partition P {", "|Lane|", "-> lbl;", "#red:", "detach", "kill", "break", "(A)",
    ]);
    prop::collection::vec(pieces, 0..64).prop_map(|v| v.concat())
}

fn name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["A", "B", "C", "\"Long name\"", "é", "x1"])
}

fn arrow() -> impl Strategy<Value = &'static str> {
    #[rustfmt::skip]
    let arrows = prop::sample::select(vec![
        "->", "-->", "->>", "-->>", "-\\", "-//", "->x", "x->", "<-", "<--", "<->", "<-->",
        "-x",
    ]);
    arrows
}

fn statement() -> impl Strategy<Value = String> {
    let kinds = prop::sample::select(vec!["participant", "actor", "boundary", "queue"]);
    let modifier = prop::sample::select(vec!["", " ++", " **", " !!", " ++--"]);
    let side = prop::sample::select(vec!["left", "right"]);
    let simple = prop_oneof![
        (kinds, name()).prop_map(|(k, n)| format!("{k} {n}")),
        (name(), arrow(), name(), modifier)
            .prop_map(|(a, arrow, b, m)| format!("{a} {arrow} {b}{m} : msg")),
        (arrow(), name()).prop_map(|(arrow, b)| format!("[{arrow} {b} : in")),
        (name(), side).prop_map(|(n, s)| format!("note {s} of {n} : n")),
        (name(), name()).prop_map(|(a, b)| format!("note over {a}, {b}\n  x\nend note")),
        name().prop_map(|n| format!("ref over {n} : r")),
        Just("== sep ==".to_owned()),
        Just("... wait ...".to_owned()),
        Just("||30||".to_owned()),
        Just("autonumber".to_owned()),
        Just("hide footbox".to_owned()),
    ];
    simple.prop_recursive(3, 24, 4, |inner| {
        (
            prop::sample::select(vec!["alt a", "opt", "loop 2", "group g [x]", "critical"]),
            prop::collection::vec(inner.clone(), 0..3),
            prop::collection::vec(inner, 0..3),
        )
            .prop_map(|(head, a, b)| {
                format!("{head}\n{}\nelse\n{}\nend", a.join("\n"), b.join("\n"))
            })
    })
}

/// A well-formed diagram.
fn document() -> impl Strategy<Value = String> {
    prop::collection::vec(statement(), 0..10)
        .prop_map(|stmts| format!("@startuml\n{}\n@enduml\n", stmts.join("\n")))
}

/// A well-formed activity statement, with nested blocks (PACT-*).
fn activity_statement() -> impl Strategy<Value = String> {
    let simple = prop_oneof![
        prop::sample::select(vec![
            ":a;",
            ":two\nlines;",
            "#pink:colored|",
            ":send>",
            "-> label;",
            "-[#red,dashed]->",
            "detach",
            "stop",
            "end",
            "kill",
            "(C)",
            "note right: n",
            "note left\nx\nend note",
            "|L1|",
            "|L2|",
        ])
        .prop_map(str::to_owned),
    ];
    simple.prop_recursive(3, 24, 4, |inner| {
        let body = || prop::collection::vec(inner.clone(), 0..3).prop_map(|v| v.join("\n"));
        prop_oneof![
            (body(), body()).prop_map(|(a, b)| format!(
                "if (c) then (yes)\n{a}\nelseif (d) then\n{b}\nelse (no)\n{a}\nendif"
            )),
            body().prop_map(|a| format!("while (w) is (y)\n{a}\nbreak\nendwhile (n)")),
            body().prop_map(|a| format!(
                "repeat\n{a}\nbackward :b;\nrepeat while (r) is (y) not (n)"
            )),
            (body(), body()).prop_map(|(a, b)| format!("fork\n{a}\nfork again\n{b}\nend fork")),
            (body(), body()).prop_map(|(a, b)| format!("split\n{a}\nsplit again\n{b}\nend split")),
            (body(), body())
                .prop_map(|(a, b)| format!("switch (s)\ncase (1)\n{a}\ncase (2)\n{b}\nendswitch")),
            body().prop_map(|a| format!("partition P {{\n{a}\n}}")),
            body().prop_map(|a| format!("group G\n{a}\nend group")),
        ]
    })
}

fn activity() -> impl Strategy<Value = String> {
    prop::collection::vec(activity_statement(), 0..8)
        .prop_map(|stmts| format!("@startuml\nstart\n{}\n@enduml\n", stmts.join("\n")))
}

/// A well-formed class diagram statement (PCLS-*).
fn class_statement() -> impl Strategy<Value = String> {
    let names = || prop::sample::select(vec!["A", "B", "C", "\"Long name\"", "x.y.Z", "Box<T>"]);
    let kinds = prop::sample::select(vec![
        "class",
        "abstract class",
        "interface",
        "enum",
        "object",
        "entity",
        "struct",
        "circle",
        "diamond",
    ]);
    let arrows = prop::sample::select(vec![
        "<|--",
        "*--",
        "o--",
        "-->",
        "..>",
        "..|>",
        "--",
        "..",
        "-up->",
        "-left-",
        "-[#red]->",
        "-[hidden]-",
        "}o--||",
        "---",
        "->",
    ]);
    let members = prop::sample::select(vec![
        "",
        " {\n  -id : int\n  +run()\n}",
        " {\n  a\n  --\n  b()\n  .. t ..\n}",
        " {}",
        " <<S>> #pink",
        " extends B",
        " implements B, C",
    ]);
    let simple = prop_oneof![
        (kinds, names(), members).prop_map(|(k, n, m)| format!("{k} {n}{m}")),
        (names(), arrows, names()).prop_map(|(a, arrow, b)| format!("{a} {arrow} {b}")),
        (names(), names()).prop_map(|(a, b)| format!("{a} \"1\" *-- \"n\" {b} : has >")),
        names().prop_map(|n| format!("{n} : +field : int")),
        names().prop_map(|n| format!("note right of {n} : n")),
        Just("note \"free\" as N".to_owned()),
        Just("hide empty members".to_owned()),
        Just("left to right direction".to_owned()),
    ];
    simple.prop_recursive(2, 16, 4, |inner| {
        (
            prop::sample::select(vec!["package P", "namespace N", "together"]),
            prop::collection::vec(inner, 0..4),
        )
            .prop_map(|(head, body)| format!("{head} {{\n{}\n}}", body.join("\n")))
    })
}

/// A well-formed state diagram statement (PSTA-*).
fn state_statement() -> impl Strategy<Value = String> {
    let names = || prop::sample::select(vec!["A", "B", "C", "[*]", "[H]", "\"Long\""]);
    let arrows = prop::sample::select(vec![
        "-->",
        "->",
        "-up->",
        "-left->",
        "-[#red,dotted]->",
        "--->",
    ]);
    let decls = prop::sample::select(vec![
        "state S",
        "state \"Long name\" as L",
        "state S : does x",
        "state c <<choice>>",
        "state f <<fork>>",
        "state j <<join>>",
        "state h <<history>>",
        "state S #pink",
        "S : more",
        "note right of S : n",
        "note \"free\" as N",
        "hide empty description",
    ]);
    let simple = prop_oneof![
        decls.prop_map(str::to_owned),
        (names(), arrows, names()).prop_map(|(a, arrow, b)| format!("{a} {arrow} {b} : ev")),
    ];
    simple.prop_recursive(3, 24, 4, |inner| {
        (
            prop::sample::select(vec!["Outer", "Inner", "\"Long comp\" as LC"]),
            prop::collection::vec(inner.clone(), 0..3),
            prop::collection::vec(inner, 0..3),
            any::<bool>(),
        )
            .prop_map(|(name, a, b, split)| {
                let sep = if split { "\n--\n" } else { "\n" };
                format!("state {name} {{\n{}{sep}{}\n}}", a.join("\n"), b.join("\n"))
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn generated_diagrams_lower_and_draw(src in document()) {
        // The statements are well formed; how they combine (an activation never closed, a
        // creation after a message) may still be a lowering error.
        let parse = plantuml::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        prop_assert!(parse.diagnostics().is_empty(), "{:?}\n{}", parse.diagnostics(), src);
        check(&parse.lower().0)?;
    }

    #[test]
    fn generated_activities_lower_and_draw(src in activity()) {
        let parse = plantuml::parse(&src);
        prop_assert_eq!(parse.kind(), plantuml::Kind::Activity);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        let (diagram, diagnostics) = parse.lower();
        prop_assert!(diagnostics.is_empty(), "{:?}\n{}", diagnostics, src);
        check(&diagram)?;
    }

    #[test]
    fn generated_classes_lower_and_draw(
        stmts in prop::collection::vec(class_statement(), 0..8),
    ) {
        let src = format!("@startuml\nclass Anchor\n{}\n@enduml\n", stmts.join("\n"));
        let parse = plantuml::parse(&src);
        prop_assert_eq!(parse.kind(), plantuml::Kind::Class);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        let (diagram, diagnostics) = parse.lower();
        prop_assert!(diagnostics.is_empty(), "{:?}\n{}", diagnostics, src);
        check(&diagram)?;
    }

    #[test]
    fn generated_states_lower_and_draw(
        stmts in prop::collection::vec(state_statement(), 0..8),
    ) {
        let src = format!("@startuml\nstate Anchor\n{}\n@enduml\n", stmts.join("\n"));
        let parse = plantuml::parse(&src);
        prop_assert_eq!(parse.kind(), plantuml::Kind::State);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        let (diagram, diagnostics) = parse.lower();
        // Transitions may cross concurrent regions, which is a warning; nothing else is.
        let others: Vec<_> = diagnostics
            .iter()
            .filter(|d| !d.message.starts_with("transitions between concurrent regions"))
            .collect();
        prop_assert!(others.is_empty(), "{:?}\n{}", others, src);
        check(&diagram)?;
    }

    #[test]
    fn round_trips_arbitrary_text(src in any::<String>()) {
        let parse = plantuml::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src);
        check(&parse.lower().0)?;
    }

    #[test]
    fn round_trips_puml_like_text(src in puml_ish()) {
        let parse = plantuml::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        let (seq, diagnostics) = parse.lower();
        for d in &diagnostics {
            prop_assert!(d.range.end() <= TextSize::of(src.as_str()));
        }
        check(&seq)?;
    }
}
