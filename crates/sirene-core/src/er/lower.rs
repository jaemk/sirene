//! Lowers an ER diagram syntax tree to the graph model (MER-2).

use std::collections::HashMap;

use rowan::TextRange;

use super::parser::WORDS;
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::flowchart::{
    Arrow, ClassDef, Direction, Edge, Flowchart, Label, Node, Row, Shape, Stroke, Style,
};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let mut l = Lowerer::default();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(t) = el.as_token()
        {
            l.chart.title = text::frontmatter_title(t.text());
        }
    }
    for stmt in root.children() {
        match stmt.kind() {
            DIRECTION_STMT => {
                if let Some(d) = text_of(&stmt, DIR).and_then(|d| Direction::from_text(&d)) {
                    l.chart.direction = d;
                }
            }
            ENTITY => l.entity(&stmt),
            RELATION_STMT => l.relation(&stmt),
            CLASSDEF_STMT => l.class_def(&stmt),
            CLASS_STMT => l.class(&stmt),
            STYLE_STMT => l.style(&stmt),
            ACC_TITLE_STMT => l.chart.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => l.chart.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    l.finish()
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|n| n.kind() == kind)
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}

/// A name token's text: quotes removed from a quoted one.
fn name(t: &SyntaxToken) -> String {
    match t.kind() {
        STRING => text::unquote(t.text()).to_owned(),
        _ => t.text().to_owned(),
    }
}

/// The marker of a cardinality symbol, read the same from either side (`|o` and `o|`).
fn marker(symbol: &str) -> Arrow {
    match symbol {
        "|o" | "o|" => Arrow::ZeroOne,
        "}o" | "o{" => Arrow::ZeroMany,
        "}|" | "|{" => Arrow::OneMany,
        _ => Arrow::One,
    }
}

/// The two ends and line of a relationship token, symbols or words (MER-1).
pub(crate) fn relationship(rel: &str) -> (Arrow, Stroke, Arrow) {
    if rel.len() == 6 && rel.is_ascii() && !rel.contains(' ') {
        let stroke = if &rel[2..4] == "--" {
            Stroke::Normal
        } else {
            Stroke::Dotted
        };
        return (marker(&rel[..2]), stroke, marker(&rel[4..]));
    }
    let card = |s: &str| -> Arrow {
        WORDS
            .iter()
            .find(|(w, _)| s.trim() == *w)
            .map_or(Arrow::One, |(_, sym)| marker(sym))
    };
    let (sep, stroke) = if rel.contains(" optionally to ") {
        (" optionally to ", Stroke::Dotted)
    } else {
        (" to ", Stroke::Normal)
    };
    let (a, b) = rel.split_once(sep).unwrap_or((rel, ""));
    (card(a), stroke, card(b))
}

#[derive(Default)]
struct Lowerer {
    chart: Flowchart,
    diagnostics: Vec<Diagnostic>,
    node_index: HashMap<String, usize>,
    /// `:::`, `class`, and `style`, applied once every entity exists.
    styling: Vec<(String, Styling, TextRange)>,
}

enum Styling {
    Class(String),
    Styles(Vec<Style>),
}

impl Lowerer {
    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    /// The entity an `ENTITY_REF` names, created if new, with its alias and `:::` class.
    fn reference(&mut self, r: &SyntaxNode) -> Option<usize> {
        let names: Vec<SyntaxToken> = tokens(r)
            .filter(|t| matches!(t.kind(), IDENT | STRING))
            .collect();
        let id = name(names.first()?);
        let i = match self.node_index.get(&id) {
            Some(&i) => i,
            None => {
                let i = self.chart.nodes.len();
                self.chart.nodes.push(Node {
                    id: id.clone(),
                    label: Label::plain(&id),
                    shape: Shape::Rect,
                    sections: Vec::new(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                    click: None,
                    tooltip: None,
                    metadata: Vec::new(),
                    mentions: Vec::new(),
                });
                self.node_index.insert(id.clone(), i);
                i
            }
        };
        self.chart.nodes[i].mentions.push(r.text_range());
        let colons = token(r, TRIPLE_COLON).map(|t| t.text_range().start());
        for t in names.iter().skip(1) {
            match colons {
                Some(at) if t.text_range().start() > at => self.styling.push((
                    id.clone(),
                    Styling::Class(t.text().to_owned()),
                    r.text_range(),
                )),
                _ => self.chart.nodes[i].label = Label::plain(name(t)),
            }
        }
        Some(i)
    }

    /// MER-2: an entity's attributes are one compartment of rows: type, name, keys, comment.
    fn entity(&mut self, stmt: &SyntaxNode) {
        let Some(i) = child(stmt, ENTITY_REF).and_then(|r| self.reference(&r)) else {
            return;
        };
        for a in stmt.children().filter(|n| n.kind() == ATTRIBUTE) {
            let words: Vec<String> = tokens(&a)
                .filter(|t| t.kind() == IDENT)
                .map(|t| t.text().to_owned())
                .collect();
            let [ty, field] = words.as_slice() else {
                continue;
            };
            let ty = generic(ty);
            let keys = text_of(&a, KEYS)
                .map(|k| k.split(',').map(str::trim).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            let comment = token(&a, STRING)
                .map(|s| text::unquote(s.text()).to_owned())
                .unwrap_or_default();
            let node = &mut self.chart.nodes[i];
            if node.sections.is_empty() {
                node.sections.push(Vec::new());
            }
            node.sections[0].push(Row {
                cells: vec![ty, field.clone(), keys, comment],
                ..Row::default()
            });
        }
    }

    fn relation(&mut self, stmt: &SyntaxNode) {
        let refs: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == ENTITY_REF).collect();
        // Every entity named is drawn, even when the statement is incomplete.
        let ends: Vec<Option<usize>> = refs.iter().map(|r| self.reference(r)).collect();
        let (Some(rel), &[Some(a), Some(b)]) = (token(stmt, RELATION), ends.as_slice()) else {
            return;
        };
        let (start, stroke, end) = relationship(rel.text());
        let label = text_of(stmt, TEXT)
            .map(|t| {
                if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
                    t[1..t.len() - 1].to_owned()
                } else {
                    t
                }
            })
            .filter(|t| !t.is_empty())
            .map(|t| text::label(&t, false));
        self.chart.edges.push(Edge {
            id: None,
            from: self.chart.nodes[a].id.clone(),
            to: self.chart.nodes[b].id.clone(),
            stroke,
            start: Some(start),
            end: Some(end),
            length: 1,
            constraint: true,
            label,
            start_label: None,
            end_label: None,
            classes: Vec::new(),
            styles: Vec::new(),
            curve: None,
            animate: false,
            animation: None,
            metadata: Vec::new(),
            range: stmt.text_range(),
        });
    }

    fn styles(&mut self, t: &SyntaxToken) -> Vec<Style> {
        let (styles, invalid) = text::styles(t.text());
        for bad in invalid {
            self.warning(
                t.text_range(),
                format!("ignoring style declaration `{bad}` without `:`"),
            );
        }
        styles
    }

    fn class_def(&mut self, stmt: &SyntaxNode) {
        let (Some(names), Some(style)) = (child(stmt, ID_LIST), token(stmt, STYLE_TEXT)) else {
            return;
        };
        let styles = self.styles(&style);
        for n in tokens(&names).filter(|t| t.kind() == IDENT) {
            let n = n.text();
            match self.chart.classes.iter_mut().find(|c| c.name == n) {
                Some(class) => class.styles.extend(styles.iter().cloned()),
                None => self.chart.classes.push(ClassDef {
                    name: n.to_owned(),
                    styles: styles.clone(),
                }),
            }
        }
    }

    fn class(&mut self, stmt: &SyntaxNode) {
        let (Some(ids), Some(class)) = (child(stmt, ID_LIST), token(stmt, IDENT)) else {
            return;
        };
        for t in tokens(&ids).filter(|t| t.kind() == IDENT) {
            self.styling.push((
                t.text().to_owned(),
                Styling::Class(class.text().to_owned()),
                t.text_range(),
            ));
        }
    }

    fn style(&mut self, stmt: &SyntaxNode) {
        let (Some(ids), Some(style)) = (child(stmt, ID_LIST), token(stmt, STYLE_TEXT)) else {
            return;
        };
        let styles = self.styles(&style);
        for t in tokens(&ids).filter(|t| t.kind() == IDENT) {
            self.styling.push((
                t.text().to_owned(),
                Styling::Styles(styles.clone()),
                t.text_range(),
            ));
        }
    }

    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        for (id, styling, range) in std::mem::take(&mut self.styling) {
            let Some(&i) = self.node_index.get(&id) else {
                self.warning(range, format!("unknown entity `{id}`"));
                continue;
            };
            let n = &mut self.chart.nodes[i];
            match styling {
                Styling::Class(c) => n.classes.push(c),
                Styling::Styles(s) => n.styles.extend(s),
            }
        }
        (self.chart, self.diagnostics)
    }
}

/// `~T~` generics in a type as `<T>`.
fn generic(s: &str) -> String {
    let mut out = String::new();
    let mut open = false;
    for c in s.chars() {
        if c == '~' {
            out.push(if open { '>' } else { '<' });
            open = !open;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relationships() {
        use Arrow::*;
        assert_eq!(relationship("||--o{"), (One, Stroke::Normal, ZeroMany));
        assert_eq!(relationship("}|..|{"), (OneMany, Stroke::Dotted, OneMany));
        assert_eq!(relationship("|o.-o|"), (ZeroOne, Stroke::Dotted, ZeroOne));
        assert_eq!(
            relationship("one or more to zero or many"),
            (OneMany, Stroke::Normal, ZeroMany)
        );
        assert_eq!(
            relationship("1 optionally to many(1)"),
            (One, Stroke::Dotted, OneMany)
        );
    }

    #[test]
    fn generics() {
        assert_eq!(generic("List~int~"), "List<int>");
        assert_eq!(generic("string"), "string");
    }
}
