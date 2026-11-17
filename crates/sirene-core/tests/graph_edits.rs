//! Property tests for the drawing edits of sequence, state, class, ER, and requirement diagrams
//! (WYSIWY-21 to WYSIWY-25): random edits on random diagrams keep the source parsing without new
//! diagnostics, do what they say, and undo exactly.

use proptest::prelude::*;
use sirene_core::flowchart::edit::TextEdit;
use sirene_core::flowchart::{Arrow, Lowered};
use sirene_core::{class, er, requirement, sequence, state};

#[derive(Debug, Clone, Copy)]
enum Kind {
    Sequence,
    State,
    Class,
    Er,
    Requirement,
}

/// What an edit is checked against: the node (or participant) ids, the number of edges (or
/// messages), and the diagnostics.
struct Summary {
    ids: Vec<String>,
    edges: usize,
    diagnostics: Vec<String>,
}

impl Summary {
    fn has(&self, id: &str) -> bool {
        self.ids.iter().any(|i| i == id)
    }
}

fn summary(l: Lowered) -> Summary {
    Summary {
        ids: l.flowchart.nodes.into_iter().map(|n| n.id).collect(),
        edges: l.flowchart.edges.len(),
        diagnostics: l.diagnostics.into_iter().map(|d| d.message).collect(),
    }
}

impl Kind {
    fn lower(self, src: &str) -> Summary {
        match self {
            Kind::Sequence => {
                let (s, diagnostics) = sequence::parse(src).lower();
                Summary {
                    ids: s.participants.iter().map(|p| p.id.clone()).collect(),
                    edges: s.messages().count(),
                    diagnostics: diagnostics.into_iter().map(|d| d.message).collect(),
                }
            }
            Kind::State => summary(state::parse(src).lower()),
            Kind::Class => summary(class::parse(src).lower()),
            Kind::Er => summary(er::parse(src).lower()),
            Kind::Requirement => summary(requirement::parse(src).lower()),
        }
    }

    fn add(self, src: &str, label: &str) -> (String, TextEdit) {
        match self {
            Kind::Sequence => sequence::edit::add_participant(src, label),
            Kind::State => state::edit::add_state(src, label),
            Kind::Class => class::edit::add_class(src, label),
            Kind::Er => er::edit::add_entity(src, label),
            Kind::Requirement => requirement::edit::add_requirement(src, label),
        }
    }

    fn connect(self, src: &str, from: &str, to: &str) -> Option<TextEdit> {
        match self {
            Kind::Sequence => Some(sequence::edit::add_message(src, from, to, "")),
            Kind::State => state::edit::add_transition(src, from, to),
            Kind::Class => class::edit::add_relation(src, from, to),
            Kind::Er => er::edit::add_relationship(src, from, to),
            Kind::Requirement => requirement::edit::add_relationship(src, from, to),
        }
    }

    fn rename_node(self, src: &str, id: &str, label: &str) -> Option<TextEdit> {
        match self {
            Kind::Sequence => sequence::edit::set_participant_label(src, id, label),
            Kind::State => state::edit::set_state_label(src, id, label),
            Kind::Class => class::edit::set_class_label(src, id, label),
            Kind::Er => er::edit::set_entity_label(src, id, label),
            Kind::Requirement => requirement::edit::rename(src, id, label),
        }
    }

    fn rename_edge(self, src: &str, edge: usize, label: &str) -> Option<TextEdit> {
        match self {
            Kind::Sequence => sequence::edit::set_message_text(src, edge, label),
            Kind::State => state::edit::set_transition_label(src, edge, label),
            Kind::Class => class::edit::set_relation_label(src, edge, label),
            Kind::Er => er::edit::set_relationship_label(src, edge, label),
            Kind::Requirement => None,
        }
    }

    fn delete(self, src: &str, ids: &[&str], edges: &[usize]) -> Option<TextEdit> {
        match self {
            Kind::Sequence => sequence::edit::delete(src, ids, edges),
            Kind::State => state::edit::delete(src, ids, edges),
            Kind::Class => class::edit::delete(src, ids, edges),
            Kind::Er => er::edit::delete(src, ids, edges),
            Kind::Requirement => requirement::edit::delete(src, ids, edges),
        }
    }

    /// The kind's own edge edit, picked by `n`.
    fn set_edge(self, src: &str, edge: usize, n: usize) -> Option<TextEdit> {
        match self {
            Kind::Sequence | Kind::State => None,
            Kind::Class => {
                let kinds = class::edit::RELATION_KINDS;
                class::edit::set_relation_kind(src, edge, kinds[n % kinds.len()].0)
            }
            Kind::Er => {
                let markers = [Arrow::One, Arrow::ZeroOne, Arrow::OneMany, Arrow::ZeroMany];
                let even = n.is_multiple_of(2);
                match n % 5 {
                    4 => er::edit::set_identifying(src, edge, even),
                    m => er::edit::set_cardinality(src, edge, even, markers[m]),
                }
            }
            Kind::Requirement => {
                let types = requirement::edit::RELATIONSHIPS;
                requirement::edit::set_relationship_type(src, edge, types[n % types.len()].1)
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Op {
    Add(String),
    Connect(usize, usize),
    RenameNode(usize, String),
    RenameEdge(usize, String),
    DeleteNode(usize),
    DeleteEdge(usize),
    SetEdge(usize, usize),
}

const IDS: [&str; 5] = ["A", "B", "Cc", "d_1", "E"];

fn label() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 _#\"'<>&;:{}()|*/\\[\\]\\\\.,=`%-]{0,10}"
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        label().prop_map(Op::Add),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Connect(a, b)),
        (any::<usize>(), label()).prop_map(|(i, l)| Op::RenameNode(i, l)),
        (any::<usize>(), label()).prop_map(|(i, l)| Op::RenameEdge(i, l)),
        any::<usize>().prop_map(Op::DeleteNode),
        any::<usize>().prop_map(Op::DeleteEdge),
        (any::<usize>(), any::<usize>()).prop_map(|(i, n)| Op::SetEdge(i, n)),
    ]
}

/// A diagram of `kind` with edges between `IDS`, some labeled, and some extra statements.
fn document(kind: Kind) -> impl Strategy<Value = String> {
    let edge = (0..IDS.len(), 0..IDS.len(), 0..4usize, any::<bool>());
    (
        prop::collection::vec(edge, 0..6),
        prop::collection::vec(0..IDS.len(), 0..3),
    )
        .prop_map(move |(edges, extras)| {
            let mut s = String::from(match kind {
                Kind::Sequence => "sequenceDiagram\n",
                Kind::State => "stateDiagram-v2\n",
                Kind::Class => "classDiagram\n",
                Kind::Er => "erDiagram\n",
                Kind::Requirement => "requirementDiagram\n",
            });
            for &i in &extras {
                let id = IDS[i];
                s += &match kind {
                    Kind::Sequence => format!("    participant {id} as Long {id}\n"),
                    Kind::State => format!("    state \"Long {id}\" as {id}\n"),
                    Kind::Class => format!("    class {id} {{\n        +f()\n    }}\n"),
                    Kind::Er => format!("    {id} {{\n        string name\n    }}\n"),
                    Kind::Requirement => {
                        format!("    requirement {id} {{\n        id: 1\n    }}\n")
                    }
                };
            }
            for (a, b, n, labeled) in edges {
                let (a, b) = (IDS[a], IDS[b]);
                let text = if labeled { " : x" } else { "" };
                s += &match kind {
                    Kind::Sequence => {
                        let arrow = ["->>", "-->>", "-x", "->"][n];
                        format!("    {a}{arrow}{b}: x\n")
                    }
                    Kind::State => format!("    {a} --> {b}{text}\n"),
                    Kind::Class => {
                        let rel = ["<|--", "*--", "-->", "..|>"][n];
                        format!("    {a} {rel} {b}{text}\n")
                    }
                    Kind::Er => {
                        let rel = ["||--o{", "|o..||", "}|--|{", "}o--o|"][n];
                        format!("    {a} {rel} {b} : x\n")
                    }
                    Kind::Requirement => {
                        let rel = ["satisfies", "traces", "contains", "verifies"][n];
                        format!("    {a} - {rel} -> {b}\n")
                    }
                };
            }
            s
        })
}

type Check = Box<dyn Fn(&Summary) -> bool>;

/// Applies `op` to `src` and checks the result. Returns the new source.
fn step(kind: Kind, src: &str, op: &Op) -> Result<String, TestCaseError> {
    let before = kind.lower(src);
    let pick =
        |i: usize| (!before.ids.is_empty()).then(|| before.ids[i % before.ids.len()].clone());
    let edge = |i: usize| (before.edges > 0).then(|| i % before.edges);
    let (n, e) = (before.ids.len(), before.edges);
    let same: Check = Box::new(move |s: &Summary| s.ids.len() == n && s.edges == e);
    let (edit, check): (Option<TextEdit>, Check) = match op {
        Op::Add(label) => {
            let (id, edit) = kind.add(src, label);
            let check = move |s: &Summary| s.ids.len() == n + 1 && s.has(&id);
            (Some(edit), Box::new(check))
        }
        Op::Connect(a, b) => match (pick(*a), pick(*b)) {
            (Some(a), Some(b)) => (
                kind.connect(src, &a, &b),
                Box::new(move |s: &Summary| s.edges == e + 1),
            ),
            _ => (None, same),
        },
        Op::RenameNode(i, label) => match pick(*i) {
            Some(id) => (kind.rename_node(src, &id, label), same),
            None => (None, same),
        },
        Op::RenameEdge(i, label) => match edge(*i) {
            Some(i) => (kind.rename_edge(src, i, label), same),
            None => (None, same),
        },
        Op::DeleteNode(i) => match pick(*i) {
            Some(id) => (
                kind.delete(src, &[&id], &[]),
                Box::new(move |s: &Summary| !s.has(&id)),
            ),
            None => (None, same),
        },
        Op::DeleteEdge(i) => match edge(*i) {
            Some(i) => (
                kind.delete(src, &[], &[i]),
                Box::new(move |s: &Summary| s.edges == e - 1),
            ),
            None => (None, same),
        },
        Op::SetEdge(i, k) => match edge(*i) {
            Some(i) => (kind.set_edge(src, i, *k), same),
            None => (None, same),
        },
    };
    let Some(edit) = edit else {
        return Ok(src.to_owned());
    };
    let out = edit.apply(src);
    let after = kind.lower(&out);
    prop_assert!(
        after.diagnostics.len() <= before.diagnostics.len(),
        "{op:?} added diagnostics:\n{src}\n->\n{out}\n{:?}\n->\n{:?}",
        before.diagnostics,
        after.diagnostics
    );
    prop_assert!(
        check(&after),
        "{op:?} did not do its edit:\n{src}\n->\n{out}"
    );
    // Undo: the replaced text put back over the inserted text.
    let undo = TextEdit {
        range: edit.range.start..edit.range.start + edit.text.len(),
        text: src[edit.range.clone()].to_owned(),
    };
    prop_assert_eq!(undo.apply(&out), src);
    Ok(out)
}

fn run(kind: Kind, src: &str, ops: &[Op]) -> Result<(), TestCaseError> {
    let mut src = src.to_owned();
    for op in ops {
        src = step(kind, &src, op)?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn sequence_edits(src in document(Kind::Sequence), ops in prop::collection::vec(op(), 1..8)) {
        run(Kind::Sequence, &src, &ops)?;
    }

    #[test]
    fn state_edits(src in document(Kind::State), ops in prop::collection::vec(op(), 1..8)) {
        run(Kind::State, &src, &ops)?;
    }

    #[test]
    fn class_edits(src in document(Kind::Class), ops in prop::collection::vec(op(), 1..8)) {
        run(Kind::Class, &src, &ops)?;
    }

    #[test]
    fn er_edits(src in document(Kind::Er), ops in prop::collection::vec(op(), 1..8)) {
        run(Kind::Er, &src, &ops)?;
    }

    #[test]
    fn requirement_edits(
        src in document(Kind::Requirement),
        ops in prop::collection::vec(op(), 1..8),
    ) {
        run(Kind::Requirement, &src, &ops)?;
    }
}
