//! Shared lossless-tree and recovery contracts for representative parsers (LANGS-10).

use proptest::prelude::*;
use sirene_core::Diagnostic;
use sirene_core::syntax::{SyntaxKind, SyntaxNode};

#[path = "common/parser_registry.rs"]
mod parser_registry;
use parser_registry::PARSERS;

#[test]
fn fuzz_selector_mapping_is_stable_and_unique() {
    let expected = [
        ("flowchart", "flowchart TD", true),
        ("architecture", "architecture-beta", false),
        ("block", "block-beta", false),
        ("c4", "C4Context", false),
        ("class", "classDiagram", false),
        ("cynefin", "cynefin-beta", false),
        ("dot", "digraph {", false),
        ("er", "erDiagram", false),
        ("eventmodel", "eventmodeling", false),
        ("gantt", "gantt", false),
        ("git", "gitGraph", false),
        ("info", "info", false),
        ("ishikawa", "ishikawa-beta", false),
        ("journey", "journey", false),
        ("kanban", "kanban", false),
        ("mindmap", "mindmap", false),
        ("packet", "packet-beta", false),
        ("pie", "pie", false),
        ("plantuml", "@startuml", true),
        ("quadrant", "quadrantChart", false),
        ("radar", "radar-beta", false),
        ("railroad", "railroad-beta", false),
        ("requirement", "requirementDiagram", false),
        ("sankey", "sankey-beta", false),
        ("sequence", "sequenceDiagram", false),
        ("state", "stateDiagram-v2", false),
        ("timeline", "timeline", false),
        ("treemap", "treemap-beta", false),
        ("treeview", "treeView-beta", false),
        ("usecase", "usecase-beta", false),
        ("venn", "venn-beta", false),
        ("wardley", "wardley-beta", false),
        ("xychart", "xychart-beta", false),
        ("zenuml", "zenuml", false),
    ];
    assert_eq!(PARSERS.len(), expected.len());
    let mut names = std::collections::HashSet::new();
    let mut headers = std::collections::HashSet::new();
    for (parser, &(name, header, normalizes)) in PARSERS.iter().zip(&expected) {
        assert_eq!(
            (parser.name, parser.header, parser.normalize_cr),
            (name, header, normalizes)
        );
        assert!(names.insert(parser.name));
        assert!(headers.insert(parser.header));
    }
}

fn check_contract(src: &str, tree: &SyntaxNode, diagnostics: &[Diagnostic]) {
    assert_eq!(tree.to_string(), src);
    for d in diagnostics {
        let start = u32::from(d.range.start()) as usize;
        let end = u32::from(d.range.end()) as usize;
        assert!(start <= end && end <= src.len(), "{d:?} in {src:?}");
        assert!(
            src.is_char_boundary(start) && src.is_char_boundary(end),
            "{d:?} in {src:?}"
        );
    }
}

fn normalized(src: &str) -> String {
    let mut chars = src.chars().peekable();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        out.push(if c == '\r' && chars.peek() != Some(&'\n') {
            '\n'
        } else {
            c
        });
    }
    out
}

#[test]
fn carriage_returns_follow_each_parsers_contract() {
    for parser in PARSERS {
        let (name, parse, header, normalizes) = (
            parser.name,
            parser.parse,
            parser.header,
            parser.normalize_cr,
        );
        let src = format!("{header}\rcafé\r\nnext\r");
        let (tree, diagnostics) = parse(&src);
        let expected = if normalizes { normalized(&src) } else { src };
        assert_eq!(tree.to_string(), expected, "{name}");
        check_contract(&expected, &tree, &diagnostics);
    }
}

#[test]
fn valid_sources_and_recovery() {
    let cases = [
        (
            "pie\n\"café\": 2\ntitle Later\n",
            "pie\n\"café\": nope\ntitle Later\n",
            "expected a number",
        ),
        (
            "journey\ncafé: 2: Me\ntitle Later\n",
            "journey\ncafé\ntitle Later\n",
            "expected `:` and a score after the task name",
        ),
        (
            "timeline TD\ncafé : Event\ntitle Later\n",
            "timeline XY\ncafé : Event\ntitle Later\n",
            "expected the end of the line",
        ),
        (
            "xychart-beta\nbar [1, 2]\ntitle Later\n",
            "xychart-beta\nbar [1, café]\ntitle Later\n",
            "expected a number",
        ),
    ];
    for (name, (valid, malformed, message)) in ["pie", "journey", "timeline", "xychart"]
        .into_iter()
        .zip(cases)
    {
        let parse = PARSERS.iter().find(|p| p.name == name).unwrap().parse;
        let (tree, diagnostics) = parse(valid);
        check_contract(valid, &tree, &diagnostics);
        assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
        let (tree, diagnostics) = parse(malformed);
        check_contract(malformed, &tree, &diagnostics);
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>(),
            [message],
            "{name}"
        );
        let later = malformed.find("title Later").unwrap();
        assert!(
            tree.descendants()
                .any(|node| node.kind() == SyntaxKind::TITLE_STMT
                    && u32::from(node.text_range().start()) as usize == later),
            "{name}: {tree:#?}"
        );
    }
}

proptest! {
    #[test]
    fn arbitrary_unicode_text_is_lossless_and_diagnostics_are_in_bounds(
        body in prop::collection::vec(any::<char>(), 0..100)
    ) {
        let body: String = body.into_iter().collect();
        for parser in PARSERS {
        let (name, parse, header, normalizes) = (parser.name, parser.parse, parser.header, parser.normalize_cr);
            let src = format!("{header}\n{body}");
            let (tree, diagnostics) = parse(&src);
            let expected = if normalizes { normalized(&src) } else { src.clone() };
            assert_eq!(tree.to_string(), expected, "{name}");
            check_contract(&expected, &tree, &diagnostics);
        }
    }
}
