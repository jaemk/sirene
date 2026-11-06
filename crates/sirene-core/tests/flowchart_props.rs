//! Property tests for the flowchart parser: round-trip on arbitrary input (FLOWCH-1) and
//! diagnostic-free parsing of generated valid documents.

use proptest::prelude::*;
use sirene_core::ascii::{self, Charset, Options};
use sirene_core::flowchart::{self, Flowchart};
use sirene_core::{Severity, TextSize};

/// Rendering never panics and ends every line with a newline (ASCIIR-1).
fn check_render(c: &Flowchart) -> Result<(), TestCaseError> {
    for charset in [Charset::Unicode, Charset::Ascii] {
        let out = ascii::render(c, &Options { charset });
        prop_assert!(out.is_empty() || out.ends_with('\n'));
        prop_assert!(
            !out.lines().any(|l| l.ends_with(' ')),
            "trailing space in\n{}",
            out
        );
    }
    Ok(())
}

/// Strings biased toward Mermaid syntax characters so the parser's branches get exercised.
fn mermaid_ish() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    let pieces = prop::sample::select(vec![
        "flowchart", "graph", " LR", " TD", "\n", "\r\n", ";", " ", "\t", "A", "B", "end",
        "subgraph ", "direction ", "classDef ", "class ", "style ", "linkStyle ", "click ",
        "accTitle", "accDescr", ":", "{", "}", "[", "]", "(", ")", "|", "\"", "`", "-->", "---",
        "--", "-.", ".->", "==>", "==", "~~~", "<", ">", "x", "o", "&", ":::", "@{", "@", ",",
        "%%", "%%{", "}%%", "---\n", "#quot;", "/", "\\", "-", ".", "=", "0", "1", "é", "🐟",
        "href ", "call ", "default ", "interpolate ",
    ]);
    prop::collection::vec(pieces, 0..64).prop_map(|v| v.concat())
}

#[derive(Debug, Clone)]
struct Node {
    id: String,
    shape: Option<(&'static str, &'static str)>,
    label: String,
}

fn node() -> impl Strategy<Value = Node> {
    let shapes = prop::sample::select(vec![
        ("[", "]"),
        ("(", ")"),
        ("([", "])"),
        ("[[", "]]"),
        ("[(", ")]"),
        ("((", "))"),
        ("(((", ")))"),
        (">", "]"),
        ("{", "}"),
        ("{{", "}}"),
        ("[/", "/]"),
        ("[\\", "\\]"),
        ("[/", "\\]"),
        ("[\\", "/]"),
    ]);
    (
        "[a-z][a-z0-9_]{0,6}",
        prop::option::of(shapes),
        "[A-Za-z0-9 ]{1,12}",
    )
        .prop_filter("keyword id", |(id, _, _)| {
            !["end", "class", "style", "click", "subgraph", "direction"].contains(&id.as_str())
        })
        .prop_map(|(id, shape, label)| Node { id, shape, label })
}

impl Node {
    fn render(&self) -> String {
        match self.shape {
            Some((open, close)) => format!("{}{open}{}{close}", self.id, self.label),
            None => self.id.clone(),
        }
    }
}

fn edge() -> impl Strategy<Value = String> {
    let strokes = prop::sample::select(vec![
        "-->", "---", "--x", "--o", "<-->", "==>", "===", "-.->", "-.-", "~~~", "---->",
    ]);
    let labelled = prop::sample::select(vec![("-- ", " -->"), ("== ", " ==>"), ("-. ", " .->")]);
    prop_oneof![
        strokes.clone().prop_map(|s| format!(" {s} ")),
        (strokes, "[A-Za-z ]{1,10}").prop_map(|(s, l)| format!(" {s}|{l}| ")),
        (labelled, "[A-Za-z]{1,10}").prop_map(|((o, c), l)| format!(" {o}{l}{c} ")),
    ]
}

fn statement() -> impl Strategy<Value = String> {
    prop_oneof![
        (node(), prop::collection::vec((edge(), node()), 0..4)).prop_map(|(first, rest)| {
            let mut s = first.render();
            for (e, n) in rest {
                s.push_str(&e);
                s.push_str(&n.render());
            }
            s
        }),
        "[a-z]{1,6}".prop_map(|c| format!("classDef {c} fill:#f96,stroke:#333")),
        "[a-z]{1,6}".prop_map(|c| format!("style {c} fill:#f96")),
        prop::sample::select(vec!["TB", "TD", "BT", "RL", "LR"])
            .prop_map(|d| format!("direction {d}")),
    ]
}

fn document() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!["flowchart", "graph"]),
        prop::sample::select(vec!["", " LR", " TD", " BT", " RL", " TB"]),
        prop::collection::vec(statement(), 0..12),
        prop::collection::vec(statement(), 0..4),
    )
        .prop_map(|(kw, dir, top, nested)| {
            let mut s = format!("{kw}{dir}\n");
            for stmt in top {
                s.push_str("    ");
                s.push_str(&stmt);
                s.push('\n');
            }
            if !nested.is_empty() {
                s.push_str("    subgraph group\n");
                for stmt in nested {
                    s.push_str("        ");
                    s.push_str(&stmt);
                    s.push('\n');
                }
                s.push_str("    end\n");
            }
            s
        })
}

/// Structural invariants of any lowered model (FLOWMO-4, FLOWMO-8, FLOWMO-10).
fn check_model(c: &Flowchart) -> Result<(), TestCaseError> {
    let mut ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.extend(c.subgraphs.iter().map(|s| s.id.as_str()));
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    prop_assert_eq!(ids.len(), count, "node and subgraph ids must be unique");
    let known = |id: &str| ids.binary_search(&id).is_ok();
    for e in &c.edges {
        prop_assert!(known(&e.from) && known(&e.to), "dangling edge {:?}", e);
        prop_assert!((1..=10).contains(&e.length));
    }
    let mut members = Vec::new();
    for s in &c.subgraphs {
        for m in &s.members {
            prop_assert!(known(m), "unknown member {} of {}", m, s.id);
            prop_assert_ne!(m, &s.id);
            members.push(m.as_str());
        }
    }
    let count = members.len();
    members.sort_unstable();
    members.dedup();
    prop_assert_eq!(members.len(), count, "a member belongs to one subgraph");
    for s in &c.subgraphs {
        let mut current = c.parent(&s.id);
        let mut depth = 0;
        while let Some(p) = current {
            prop_assert_ne!(&p.id, &s.id, "subgraph nesting has a cycle");
            depth += 1;
            prop_assert!(depth <= c.subgraphs.len(), "subgraph nesting has a cycle");
            current = c.parent(&p.id);
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn round_trips_arbitrary_text(src in any::<String>()) {
        let parse = flowchart::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src);
        check_model(&parse.lower().flowchart)?;
    }

    #[test]
    fn round_trips_mermaid_like_text(src in mermaid_ish()) {
        let parse = flowchart::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        for d in parse.diagnostics() {
            prop_assert!(usize::from(d.range.end()) <= src.len());
        }
        let lowered = parse.lower();
        check_model(&lowered.flowchart)?;
        check_render(&lowered.flowchart)?;
        for d in &lowered.diagnostics {
            prop_assert!(d.range.end() <= TextSize::of(src.as_str()));
        }
    }

    #[test]
    fn generated_documents_parse_cleanly(src in document()) {
        let parse = flowchart::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        prop_assert!(
            parse.diagnostics().is_empty(),
            "{:?}\n{}",
            parse.diagnostics(),
            src
        );
        // Generated `style` and top-level `direction` statements may draw warnings.
        let lowered = parse.lower();
        prop_assert!(
            lowered.diagnostics.iter().all(|d| d.severity == Severity::Warning),
            "{:?}\n{}",
            lowered.diagnostics,
            src
        );
        check_model(&lowered.flowchart)?;
        check_render(&lowered.flowchart)?;
    }
}
