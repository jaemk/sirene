//! Grammar-aware losslessness and recovery checks (LANGS-11).

use proptest::prelude::*;
use sirene_core::diagram::{self, Diagram};
use sirene_core::syntax::{SyntaxKind, SyntaxNode};
use sirene_core::theme::ThemeConfig;
use sirene_core::{Diagnostic, journey, pie, xychart};

#[path = "common/parser_registry.rs"]
#[allow(dead_code)]
mod parser_registry;

type Parse = fn(&str) -> (SyntaxNode, Vec<Diagnostic>);

fn parse_pie(s: &str) -> (SyntaxNode, Vec<Diagnostic>) {
    let p = pie::parse(s);
    (p.syntax(), p.diagnostics().to_vec())
}
fn parse_journey(s: &str) -> (SyntaxNode, Vec<Diagnostic>) {
    let p = journey::parse(s);
    (p.syntax(), p.diagnostics().to_vec())
}
fn parse_xychart(s: &str) -> (SyntaxNode, Vec<Diagnostic>) {
    let p = xychart::parse(s);
    (p.syntax(), p.diagnostics().to_vec())
}

proptest! {
    #[test]
    fn generated_statements_recover_at_the_following_title(
        label in "[a-zA-Z]{1,12}", value in 0_u16..1000, separator in prop_oneof![Just("\n"), Just("\r\n")]
    ) {
        let cases: [(Parse, String, String, &str); 3] = [
            (parse_pie, format!("pie{separator}\"{label}\": {value}{separator}title Later\n"),
             format!("pie{separator}\"{label}\": nope{separator}title Later\n"), "expected a number"),
            (parse_journey, format!("journey{separator}{label}: {value}: Me{separator}title Later\n"),
             format!("journey{separator}{label}{separator}title Later\n"), "expected `:` and a score after the task name"),
            (parse_xychart, format!("xychart-beta{separator}bar [{value}, 2]{separator}title Later\n"),
             format!("xychart-beta{separator}bar [{value}, nope]{separator}title Later\n"), "expected a number"),
        ];
        for (parse, valid, malformed, message) in cases {
            let (tree, diagnostics) = parse(&valid);
            prop_assert_eq!(tree.to_string(), valid);
            prop_assert!(diagnostics.is_empty(), "{:?}", diagnostics);
            let (tree, diagnostics) = parse(&malformed);
            prop_assert_eq!(tree.to_string(), malformed.as_str());
            prop_assert!(diagnostics.iter().any(|d| d.message == message), "{:?}", diagnostics);
            let later = malformed.find("title Later").unwrap();
            prop_assert!(tree.descendants().any(|n| n.kind() == SyntaxKind::TITLE_STMT
                && u32::from(n.text_range().start()) as usize == later));
        }
    }

    #[test]
    fn generated_eventmodel_resolves_frame_data(
        event in "[A-Z][a-z]{1,8}",
        key in "[a-z]{1,8}",
        value in 0_u16..1000,
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
    ) {
        let src = [
            "eventmodeling", "tf 01 ui Cart", "tf 02 cmd Add [[D]]",
            &format!("tf 03 evt Cart.{event} {{ saved: true }}"),
            "data D {", &format!("  {key}: {value}"), "}", "",
        ].join(nl);
        let lowered = diagram::lower(&src);
        prop_assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let Diagram::EventModel(model) = lowered.diagram else {
            return Err(proptest::test_runner::TestCaseError::fail("expected an event model"));
        };
        prop_assert_eq!(model.frames.len(), 3);
        prop_assert_eq!(&model.frames[1].data, &[format!("{key}: {value}")]);
        prop_assert_eq!(&model.frames[2].name, &event);
        prop_assert_eq!(model.relations.len(), 2);
    }

    #[test]
    fn generated_wardley_coordinates_keep_axis_order(
        component in "[A-Z][a-z]{1,8}",
        visibility in 2_u8..99,
        evolution in 2_u8..99,
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
    ) {
        let src = [
            "wardley-beta", "size [400, 300]", "anchor User [90, 50]",
            &format!("component \"{component}\" [{visibility}, {evolution}] label [5, -5] (buy) (inertia)"),
            &format!("User -> {component}"), "",
        ].join(nl);
        let lowered = diagram::lower(&src);
        prop_assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let Diagram::Wardley(map) = lowered.diagram else {
            return Err(proptest::test_runner::TestCaseError::fail("expected a Wardley map"));
        };
        prop_assert_eq!(map.nodes.len(), 2);
        let node = &map.nodes[1];
        prop_assert_eq!(&node.label, &component);
        prop_assert_eq!((node.x, node.y), (f64::from(evolution), f64::from(visibility)));
        prop_assert_eq!(map.links.len(), 1);
    }
}

proptest! {
    #[test]
    fn valid_multiline_directive_keeps_dispatch_and_theme(
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        theme in prop::sample::select(vec!["dark", "forest", "default"]),
        label in "[a-z]{1,10}",
    ) {
        // A complete directive may contain line breaks before the diagram header.
        let src = format!("%%{{init: {{{nl}ignored: [1, 2],{nl}theme: '{theme}'{nl}}}}}%%{nl}pie{nl}\"{label}\": 2{nl}");
        let lowered = diagram::lower(&src);
        prop_assert_eq!(lowered.kind, diagram::Kind::of(&src));
        prop_assert_eq!(lowered.kind, diagram::Kind::Pie);
        prop_assert!(matches!(lowered.diagram, Diagram::Pie(_)));
        prop_assert_eq!(ThemeConfig::read(&src), lowered.theme.clone());
        prop_assert_eq!(lowered.theme.name.as_ref().unwrap().0.as_str(), theme);
    }

    #[test]
    fn malformed_directive_does_not_hide_following_statement(
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        label in "[a-z]{1,10}",
    ) {
        let valid = format!("flowchart TD{nl}%%{{init: {{theme: 'forest', unused: [1, 2]}}}}%%{nl}A[\"{label}\"]{nl}");
        let broken = valid.replace("[1, 2]", "[1 2]");
        let good = diagram::lower(&valid);
        prop_assert!(good.diagnostics.is_empty(), "{:?}", good.diagnostics);
        prop_assert_eq!(good.theme.name.as_ref().unwrap().0.as_str(), "forest");
        let lowered = diagram::lower(&broken);
        prop_assert_eq!(lowered.kind, diagram::Kind::of(&broken));
        prop_assert_eq!(lowered.theme.unread.len(), 1);
        let Diagram::Graph(graph) = lowered.diagram else {
            return Err(proptest::test_runner::TestCaseError::fail("expected a graph"));
        };
        prop_assert!(graph.nodes.iter().any(|node| node.id == "A"));
    }

    #[test]
    fn deeper_grammars_recover_after_one_bad_token(
        nl in prop::sample::select(vec!["\n", "\r\n"]),
        name in "[A-Z][a-z]{1,8}",
    ) {
        let cases = [
            ("wardley", "wardley-beta", format!("pipeline {name} {{\n  component Part [0.5] label [5, -5]\n}}"), format!("pipeline {name} {{\n  component Part [oops]\n}}")),
            ("eventmodel", "eventmodeling", format!("gwt 01\n  given evt {name}\n  when cmd Add\n  then evt Done"), format!("gwt 01\n  given invalid {name}\n  when cmd Add\n  then evt Done")),
            ("railroad", "railroad-ebnf-beta", format!("rule = ('{name}' | [other]) {{item}} ;"), format!("rule = ('{name}' | [other]) {{item}} ! ;")),
        ];
        for (parser_name, header, valid, broken) in cases {
            let parse = parser_registry::PARSERS.iter().find(|p| p.name == parser_name).unwrap().parse;
            let valid = format!("{header}\n{valid}\ntitle Later\n").replace('\n', nl);
            let broken = format!("{header}\n{broken}\ntitle Later\n").replace('\n', nl);
            let (_, diagnostics) = parse(&valid);
            prop_assert!(diagnostics.is_empty(), "{parser_name}: {diagnostics:?}");
            let (tree, diagnostics) = parse(&broken);
            prop_assert!(!diagnostics.is_empty(), "{parser_name}");
            prop_assert_eq!(tree.to_string(), broken.as_str());
            let later = broken.find("title Later").unwrap();
            prop_assert!(tree.descendants().any(|n| n.kind() == SyntaxKind::TITLE_STMT
                && usize::from(n.text_range().start()) == later), "{parser_name}: {tree:#?}");
        }
    }
}
