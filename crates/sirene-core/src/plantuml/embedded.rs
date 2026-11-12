//! PlantUML documents written in another language sirene reads (PEMB-*): `@startdot` holds a
//! Graphviz DOT graph and `@startebnf` EBNF rules, read by the DOT and railroad readers.

use rowan::{TextRange, TextSize};

use super::lines;
use crate::diagnostic::Diagnostic;
use crate::dot;
use crate::flowchart::Flowchart;
use crate::railroad::{self, Railroad};
use crate::syntax::SyntaxNode;

/// A `@startdot` document's graph.
pub(crate) fn lower_dot(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let body = lines::body(root, |_| true);
    let mut l = dot::parse(&body).lower();
    if l.background.is_some()
        && let Some(at) = body.find("bgcolor")
    {
        let at = TextSize::try_from(at).expect("source fits");
        l.diagnostics.push(Diagnostic::warning(
            TextRange::at(at, TextSize::from(7)),
            "`bgcolor` is not drawn in a PlantUML document",
        ));
    }
    (l.flowchart, l.diagnostics)
}

/// The railroad reader's header for EBNF, put before a `@startebnf` document's rules.
const EBNF_HEADER: &str = "railroad-ebnf-beta\n";

/// A `@startebnf` document's rules.
pub(crate) fn lower_ebnf(root: &SyntaxNode) -> (Railroad, Vec<Diagnostic>) {
    let body = lines::body(root, |_| true);
    let (chart, d) = railroad::parse(&format!("{EBNF_HEADER}{body}")).lower();
    let shift = TextSize::try_from(EBNF_HEADER.len()).expect("header fits");
    let d = d
        .into_iter()
        .map(|mut d| {
            let start = d.range.start().checked_sub(shift).unwrap_or_default();
            let end = d.range.end().checked_sub(shift).unwrap_or_default();
            d.range = TextRange::new(start, end.max(start));
            d
        })
        .collect();
    (chart, d)
}
