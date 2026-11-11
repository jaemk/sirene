//! Mermaid Cynefin diagrams (MCYN-*): items placed in the five domains of the Cynefin
//! framework, with transitions between domains.

mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// One of the five domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Complex,
    Complicated,
    Clear,
    Chaotic,
    Confusion,
}

impl Domain {
    /// The four quadrants, in reading order: top left, top right, bottom left, bottom right.
    pub const QUADRANTS: [Domain; 4] = [
        Domain::Complex,
        Domain::Complicated,
        Domain::Chaotic,
        Domain::Clear,
    ];

    /// Every domain, the quadrants first.
    pub const ALL: [Domain; 5] = [
        Domain::Complex,
        Domain::Complicated,
        Domain::Chaotic,
        Domain::Clear,
        Domain::Confusion,
    ];

    fn parse(s: &str) -> Option<Domain> {
        Some(match s {
            "complex" => Domain::Complex,
            "complicated" => Domain::Complicated,
            "clear" => Domain::Clear,
            "chaotic" => Domain::Chaotic,
            "confusion" => Domain::Confusion,
            _ => return None,
        })
    }

    /// The keyword, e.g. `complex`.
    pub fn keyword(self) -> &'static str {
        match self {
            Domain::Complex => "complex",
            Domain::Complicated => "complicated",
            Domain::Clear => "clear",
            Domain::Chaotic => "chaotic",
            Domain::Confusion => "confusion",
        }
    }

    /// The name drawn on the diagram, e.g. `Complex`.
    pub fn title(self) -> &'static str {
        match self {
            Domain::Complex => "Complex",
            Domain::Complicated => "Complicated",
            Domain::Clear => "Clear",
            Domain::Chaotic => "Chaotic",
            Domain::Confusion => "Confusion",
        }
    }

    /// The decision model's steps, e.g. `["Probe", "Sense", "Respond"]`; none for confusion.
    pub fn model(self) -> &'static [&'static str] {
        match self {
            Domain::Complex => &["Probe", "Sense", "Respond"],
            Domain::Complicated => &["Sense", "Analyse", "Respond"],
            Domain::Clear => &["Sense", "Categorise", "Respond"],
            Domain::Chaotic => &["Act", "Sense", "Respond"],
            Domain::Confusion => &[],
        }
    }

    /// The kind of practice, e.g. `Emergent Practices`.
    pub fn practice(self) -> &'static str {
        match self {
            Domain::Complex => "Emergent Practices",
            Domain::Complicated => "Good Practices",
            Domain::Clear => "Best Practices",
            Domain::Chaotic => "Novel Practices",
            Domain::Confusion => "Disorder",
        }
    }

    fn index(self) -> usize {
        match self {
            Domain::Complex => 0,
            Domain::Complicated => 1,
            Domain::Chaotic => 2,
            Domain::Clear => 3,
            Domain::Confusion => 4,
        }
    }
}

/// A `from --> to` transition.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: Domain,
    pub to: Domain,
    pub label: Option<String>,
}

/// The confusion domain shows at most this many items, then a `+N more` badge.
pub const CONFUSION_ITEMS: usize = 3;

/// A Cynefin diagram (MCYN-2).
#[derive(Debug, Clone, PartialEq)]
pub struct Cynefin {
    pub title: Option<String>,
    /// Items per domain, indexed as [`Domain::ALL`].
    items: [Vec<String>; 5],
    pub transitions: Vec<Transition>,
    /// Whether each domain shows its decision model and practice (`showDomainDescriptions`).
    pub descriptions: bool,
}

impl Default for Cynefin {
    fn default() -> Self {
        Cynefin {
            title: None,
            items: Default::default(),
            transitions: Vec::new(),
            descriptions: true,
        }
    }
}

impl Cynefin {
    /// The items of `domain`, in source order.
    pub fn items(&self, domain: Domain) -> &[String] {
        &self.items[domain.index()]
    }

    /// The items `domain` shows, and how many more it holds (only confusion caps them).
    pub fn shown(&self, domain: Domain) -> (&[String], usize) {
        let items = self.items(domain);
        if domain == Domain::Confusion && items.len() > CONFUSION_ITEMS {
            (&items[..CONFUSION_ITEMS], items.len() - CONFUSION_ITEMS)
        } else {
            (items, 0)
        }
    }
}

/// The result of parsing Cynefin source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

/// A quoted string's text, without its quotes and with `\` escapes resolved.
fn unquote(token: &str) -> String {
    let body = &token[1..token.len() - 1];
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
}

fn strings(node: &SyntaxNode) -> impl Iterator<Item = String> {
    tokens(node)
        .filter(|t| t.kind() == STRING)
        .map(|t| unquote(t.text()))
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Cynefin`] diagram, with the parser's diagnostics and warnings for
    /// redeclared domains and self-loops.
    pub fn lower(&self) -> (Cynefin, Vec<Diagnostic>) {
        let root = self.syntax();
        let mut chart = Cynefin::default();
        let mut diagnostics = self.diagnostics.clone();
        for el in root.children_with_tokens() {
            if el.kind() == FRONTMATTER
                && let Some(tok) = el.as_token()
            {
                chart.title = text::frontmatter_title(tok.text());
                for (k, v) in text::frontmatter_config(tok.text(), "cynefin") {
                    if k == "showDomainDescriptions" {
                        chart.descriptions = v != "false";
                    }
                }
            }
        }
        let mut declared: Vec<Domain> = Vec::new();
        let mut current: Option<Domain> = None;
        for node in root.children() {
            match node.kind() {
                TITLE_STMT => {
                    current = None;
                    if let Some(t) = tokens(&node).find(|t| t.kind() == TEXT) {
                        chart.title = Some(t.text().to_owned());
                    }
                }
                DOMAIN_STMT => {
                    let Some(kw) = tokens(&node).find(|t| t.kind() == CYNEFIN_KW) else {
                        continue;
                    };
                    let Some(domain) = Domain::parse(kw.text()) else {
                        continue;
                    };
                    if declared.contains(&domain) {
                        diagnostics.push(Diagnostic::warning(
                            kw.text_range(),
                            format!(
                                "domain `{}` is declared again; its earlier items are dropped",
                                domain.keyword()
                            ),
                        ));
                    } else {
                        declared.push(domain);
                    }
                    chart.items[domain.index()] = strings(&node).collect();
                    current = Some(domain);
                }
                ITEM_STMT => {
                    if let Some(d) = current {
                        chart.items[d.index()].extend(strings(&node));
                    }
                }
                TRANSITION_STMT => {
                    current = None;
                    let ends: Vec<Domain> = tokens(&node)
                        .filter(|t| t.kind() == CYNEFIN_KW)
                        .filter_map(|t| Domain::parse(t.text()))
                        .collect();
                    let [from, to] = ends[..] else { continue };
                    if from == to {
                        diagnostics.push(Diagnostic::warning(
                            node.text_range(),
                            format!(
                                "a transition from `{}` to itself is not drawn",
                                from.keyword()
                            ),
                        ));
                        continue;
                    }
                    chart.transitions.push(Transition {
                        from,
                        to,
                        label: strings(&node).next().filter(|l| !l.is_empty()),
                    });
                }
                _ => current = None,
            }
        }
        log::debug!("lowered a Cynefin diagram");
        (chart, diagnostics)
    }
}

/// Parses Cynefin source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquoting() {
        assert_eq!(unquote("\"a\\\"b\""), "a\"b");
        assert_eq!(unquote("'x'"), "x");
    }
}
