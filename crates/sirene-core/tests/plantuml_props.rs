//! Property tests for PlantUML sequence diagrams: the tree holds any source exactly (PSEQ-1),
//! and any source lowers to a well-formed sequence model that draws.

use proptest::prelude::*;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::Diagram;
use sirene_core::scene::{Approx, Theme};
use sirene_core::sequence::{Event, Sequence};
use sirene_core::{TextSize, plantuml};

/// Every participant index in the events is in range, and the diagram draws in both renderers.
fn check(s: &Sequence) -> Result<(), TestCaseError> {
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
    let d = Diagram::Sequence(s.clone());
    for charset in [Charset::Unicode, Charset::Ascii] {
        d.ascii(&Options { charset });
    }
    d.scene(&Approx, &Theme::LIGHT);
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
        "class", "é", "🐟", "\\n",
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
