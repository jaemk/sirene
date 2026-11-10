//! Mermaid venn diagrams (MVENN-*): sets drawn as circles sized and overlapped by their sizes.

pub mod layout;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxElement, SyntaxNode};

/// A set (one name) or an overlap of sets (two or more).
#[derive(Debug, Clone, PartialEq)]
pub struct Subset {
    /// The set names, sorted.
    pub sets: Vec<String>,
    /// The size: the given one, or 10 divided by the square of the number of sets.
    pub size: f64,
    pub label: Option<String>,
}

/// A `text` node placed inside a set or overlap.
#[derive(Debug, Clone, PartialEq)]
pub struct TextNode {
    /// The sets of the region it sits in, sorted.
    pub sets: Vec<String>,
    pub id: String,
    pub label: Option<String>,
}

impl TextNode {
    /// The label, or the id when there is none.
    pub fn text(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }
}

/// A `style` statement: CSS-like properties for sets, overlaps, or text nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// The ids it targets, sorted: one set or text id, or the sets of an overlap.
    pub targets: Vec<String>,
    /// Property names and values in order, e.g. `("fill", "#f00")`.
    pub properties: Vec<(String, String)>,
}

/// A venn diagram (MVENN-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Venn {
    pub title: Option<String>,
    /// Sets and overlaps in source order.
    pub subsets: Vec<Subset>,
    pub texts: Vec<TextNode>,
    pub styles: Vec<Style>,
}

impl Venn {
    /// The last value of `property` styled on `targets` (sorted ids), if any.
    pub fn style(&self, targets: &[String], property: &str) -> Option<&str> {
        self.styles
            .iter()
            .filter(|s| s.targets == targets)
            .flat_map(|s| &s.properties)
            .filter(|(k, _)| k == property)
            .map(|(_, v)| v.as_str())
            .next_back()
    }

    /// The regions to draw: the subsets, then each pair inside a union of three or more sets
    /// that is not declared, sized a quarter of the smaller set, so the union has room (MVENN-3).
    pub fn regions(&self) -> Vec<Subset> {
        let mut out = self.subsets.clone();
        let size = |set: &String| {
            self.subsets
                .iter()
                .rev()
                .find(|s| s.sets.len() == 1 && &s.sets[0] == set)
                .map(|s| s.size)
        };
        for s in &self.subsets {
            if s.sets.len() < 3 {
                continue;
            }
            for (i, a) in s.sets.iter().enumerate() {
                for b in &s.sets[i + 1..] {
                    let pair = vec![a.clone(), b.clone()];
                    if out.iter().any(|o| o.sets == pair) {
                        continue;
                    }
                    let size = match (size(a), size(b)) {
                        (Some(x), Some(y)) => x.min(y) / 4.0,
                        _ => 2.5,
                    };
                    out.push(Subset {
                        sets: pair,
                        size,
                        label: None,
                    });
                }
            }
        }
        out
    }

    /// The name a set is shown by: its label, or its id.
    pub fn name<'a>(&'a self, set: &'a str) -> &'a str {
        self.subsets
            .iter()
            .rev()
            .find(|s| s.sets.len() == 1 && s.sets[0] == set)
            .and_then(|s| s.label.as_deref())
            .unwrap_or(set)
    }
}

/// The result of parsing venn source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

/// `text` trimmed, without surrounding double quotes.
fn unquote(text: &str) -> String {
    let t = text.trim();
    t.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(t)
        .to_owned()
}

/// The ids in a statement's `ID_LIST`, unquoted.
fn ids(node: &SyntaxNode) -> Vec<String> {
    node.children()
        .find(|n| n.kind() == ID_LIST)
        .into_iter()
        .flat_map(|l| l.children_with_tokens())
        .filter_map(SyntaxElement::into_token)
        .filter(|t| matches!(t.kind(), IDENT | STRING))
        .map(|t| unquote(t.text()))
        .collect()
}

/// The text of a `LABEL` child, unquoted and trimmed.
fn label(node: &SyntaxNode) -> Option<String> {
    let l = node.children().find(|n| n.kind() == LABEL)?;
    l.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|t| matches!(t.kind(), STRING | TEXT))
        .map(|t| unquote(t.text()))
}

/// `fill:#f00, color: red` as name and value pairs; commas inside parentheses do not split.
fn properties(css: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut push = |part: &str| {
        if let Some((k, v)) = part.split_once(':') {
            let k = k.trim();
            if !k.is_empty() {
                out.push((k.to_owned(), unquote(v)));
            }
        }
    };
    for (i, c) in css.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                push(&css[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    push(&css[start..]);
    out
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Venn`], with the parser's diagnostics and any unknown sets.
    pub fn lower(&self) -> (Venn, Vec<Diagnostic>) {
        let root = self.syntax();
        let mut chart = Venn::default();
        let mut diagnostics = self.diagnostics.clone();
        let mut known: Vec<String> = Vec::new();
        let mut current: Option<Vec<String>> = None;
        for el in root.children_with_tokens() {
            if el.kind() == FRONTMATTER
                && let Some(tok) = el.as_token()
            {
                chart.title = text::frontmatter_title(tok.text());
            }
        }
        for node in root.children() {
            match node.kind() {
                TITLE_STMT => {
                    let t = node
                        .children_with_tokens()
                        .filter_map(SyntaxElement::into_token)
                        .find(|t| t.kind() == TEXT);
                    if let Some(t) = t {
                        chart.title = Some(unquote(t.text()));
                    }
                }
                SET_STMT => {
                    let mut sets = ids(&node);
                    if sets.is_empty() {
                        continue;
                    }
                    let union = node
                        .first_token()
                        .is_some_and(|t| t.text().eq_ignore_ascii_case("union"));
                    if union {
                        if sets.len() < 2 {
                            diagnostics.push(Diagnostic::new(
                                node.text_range(),
                                "a union needs two or more sets",
                            ));
                            continue;
                        }
                        let unknown: Vec<&String> =
                            sets.iter().filter(|s| !known.contains(s)).collect();
                        if !unknown.is_empty() {
                            let names: Vec<String> =
                                unknown.iter().map(|s| format!("`{s}`")).collect();
                            diagnostics.push(Diagnostic::new(
                                node.text_range(),
                                format!(
                                    "unknown set {}: declare it with `set` first",
                                    names.join(", ")
                                ),
                            ));
                            continue;
                        }
                    } else if !known.contains(&sets[0]) {
                        known.push(sets[0].clone());
                    }
                    sets.sort();
                    let size = node
                        .children_with_tokens()
                        .filter_map(SyntaxElement::into_token)
                        .find(|t| t.kind() == NUMBER)
                        .and_then(|t| t.text().trim_start_matches('+').parse::<f64>().ok())
                        .unwrap_or(10.0 / (sets.len() * sets.len()) as f64);
                    current = Some(sets.clone());
                    chart.subsets.push(Subset {
                        sets,
                        size,
                        label: label(&node),
                    });
                }
                TEXT_STMT => {
                    let Some(id) = node
                        .children()
                        .find(|n| n.kind() == NODE)
                        .map(|n| unquote(&n.text().to_string()))
                    else {
                        continue;
                    };
                    let mut sets = ids(&node);
                    if sets.is_empty() {
                        match &current {
                            Some(c) => sets = c.clone(),
                            None => continue,
                        }
                    }
                    sets.sort();
                    chart.texts.push(TextNode {
                        sets,
                        id,
                        label: label(&node),
                    });
                }
                STYLE_STMT => {
                    let mut targets = ids(&node);
                    targets.sort();
                    let css = node
                        .children_with_tokens()
                        .filter_map(SyntaxElement::into_token)
                        .find(|t| t.kind() == STYLE_TEXT)
                        .map(|t| t.text().to_owned())
                        .unwrap_or_default();
                    chart.styles.push(Style {
                        targets,
                        properties: properties(&css),
                    });
                }
                _ => {}
            }
        }
        log::debug!("lowered a venn diagram");
        (chart, diagnostics)
    }
}

/// Parses venn source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_properties() {
        assert_eq!(
            properties("fill:#f00, color: \"red\" ,stroke: rgb(1, 2, 3),bad"),
            [
                ("fill".to_owned(), "#f00".to_owned()),
                ("color".to_owned(), "red".to_owned()),
                ("stroke".to_owned(), "rgb(1, 2, 3)".to_owned()),
            ]
        );
    }
}
