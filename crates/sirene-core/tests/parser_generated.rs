//! Grammar-aware losslessness and recovery checks (LANGS-11).

use proptest::prelude::*;
use sirene_core::syntax::{SyntaxKind, SyntaxNode};
use sirene_core::{Diagnostic, journey, pie, xychart};

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
}
