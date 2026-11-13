//! Lowers a requirement diagram syntax tree to the graph model (MREQ-2).

use std::collections::{HashMap, HashSet};

use rowan::TextRange;

use super::parser::KINDS;
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
            REQUIREMENT => l.definition(&stmt),
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

/// A name or value token's text: quotes removed from a quoted one, trimmed.
fn value(t: &SyntaxToken) -> String {
    match t.kind() {
        STRING => text::unquote(t.text()).trim().to_owned(),
        _ => t.text().trim().to_owned(),
    }
}

/// The stereotype shown above a node's name, by its lowercase keyword.
pub(crate) fn stereotype(kind: &str) -> &'static str {
    match kind {
        "functionalrequirement" => "Functional Requirement",
        "interfacerequirement" => "Interface Requirement",
        "performancerequirement" => "Performance Requirement",
        "physicalrequirement" => "Physical Requirement",
        "designconstraint" => "Design Constraint",
        "element" => "Element",
        _ => "Requirement",
    }
}

/// A field's row label and its value as shown, or why it is ignored.
fn field(element: bool, key: &str, v: &str) -> Result<(usize, &'static str, String), String> {
    let key = key.to_ascii_lowercase();
    let pick = |allowed: &[&'static str]| {
        allowed
            .iter()
            .find(|a| a.eq_ignore_ascii_case(v))
            .map(|a| a.to_string())
    };
    match (element, key.as_str()) {
        (false, "id") => Ok((0, "Id:", v.to_owned())),
        (false, "text") => Ok((1, "Text:", v.to_owned())),
        (false, "risk") => pick(&["Low", "Medium", "High"])
            .map(|r| (2, "Risk:", r))
            .ok_or_else(|| format!("ignoring risk `{v}`: expected `Low`, `Medium`, or `High`")),
        (false, "verifymethod") => pick(&["Analysis", "Inspection", "Test", "Demonstration"])
            .map(|m| (3, "Verification:", m))
            .ok_or_else(|| {
                format!(
                    "ignoring verify method `{v}`: expected `Analysis`, `Inspection`, `Test`, or \
                     `Demonstration`"
                )
            }),
        (true, "type") => Ok((0, "Type:", v.to_owned())),
        (true, "docref") => Ok((1, "Doc Ref:", v.to_owned())),
        (false, _) => Err(format!(
            "ignoring `{key}`: a requirement's fields are `id`, `text`, `risk`, and `verifymethod`"
        )),
        (true, _) => Err(format!(
            "ignoring `{key}`: an element's fields are `type` and `docref`"
        )),
    }
}

#[derive(Default)]
struct Lowerer {
    chart: Flowchart,
    diagnostics: Vec<Diagnostic>,
    node_index: HashMap<String, usize>,
    /// Class names to indexes in the chart's classes.
    class_index: HashMap<String, usize>,
    /// Each node's fields by their place in the box, as `(row label, value)`.
    fields: Vec<[Option<(&'static str, String)>; 4]>,
    defined: HashSet<String>,
    /// Names used in relationships but never defined, with where.
    undefined: Vec<(String, TextRange)>,
    /// `:::`, `class`, and `style`, applied once every node exists.
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

    /// The node named `id`, created if new.
    fn node(&mut self, id: &str, range: TextRange) -> usize {
        let i = match self.node_index.get(id) {
            Some(&i) => i,
            None => {
                let i = self.chart.nodes.len();
                self.chart.nodes.push(Node {
                    id: id.to_owned(),
                    label: Label::plain(id),
                    shape: Shape::Rect,
                    sections: Vec::new(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                    click: None,
                    tooltip: None,
                    metadata: Vec::new(),
                    mentions: Vec::new(),
                });
                self.fields.push(Default::default());
                self.node_index.insert(id.to_owned(), i);
                i
            }
        };
        self.chart.nodes[i].mentions.push(range);
        i
    }

    /// The classes in a `:::a,b` under `node`, for `id`.
    fn shorthand(&mut self, node: &SyntaxNode, id: &str) {
        let Some(s) = child(node, CLASS_SHORTHAND) else {
            return;
        };
        for t in s
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == IDENT)
        {
            self.styling.push((
                id.to_owned(),
                Styling::Class(t.text().to_owned()),
                s.text_range(),
            ));
        }
    }

    /// MREQ-2: a requirement or element, its stereotype above its name and its fields in rows.
    fn definition(&mut self, stmt: &SyntaxNode) {
        let (Some(kw), Some(name)) = (
            token(stmt, REQ_KW),
            tokens(stmt).find(|t| matches!(t.kind(), IDENT | STRING)),
        ) else {
            return;
        };
        let kind = kw.text().to_ascii_lowercase();
        let element = kind == "element";
        debug_assert!(KINDS.contains(&kind.as_str()));
        let id = value(&name);
        // As in Mermaid, the first definition of a name is kept.
        if !self.defined.insert(id.clone()) {
            self.warning(
                name.text_range(),
                format!("`{id}` is already defined; this definition is skipped"),
            );
            return;
        }
        let i = self.node(&id, stmt.text_range());
        self.chart.nodes[i].label = Label::plain(format!("«{}»\n{id}", stereotype(&kind)));
        self.shorthand(stmt, &id);
        for f in stmt.children().filter(|n| n.kind() == FIELD) {
            let (Some(key), Some(v)) = (
                token(&f, ATTR_KEY),
                tokens(&f).find(|t| matches!(t.kind(), TEXT | STRING)),
            ) else {
                continue;
            };
            match field(element, key.text(), &value(&v)) {
                Ok((at, label, v)) => self.fields[i][at] = Some((label, v)),
                Err(message) => self.warning(f.text_range(), message),
            }
        }
    }

    /// MREQ-2: `a - type -> b` and `b <- type - a` are both an edge from `a` to `b`.
    fn relation(&mut self, stmt: &SyntaxNode) {
        let names: Vec<SyntaxToken> = tokens(stmt)
            .filter(|t| matches!(t.kind(), IDENT | STRING))
            .collect();
        let (Some(rel), [a, b]) = (token(stmt, RELATION), names.as_slice()) else {
            return;
        };
        let backward = token(stmt, ARROW).is_some_and(|t| t.text() == "<-");
        let (from, to) = if backward { (b, a) } else { (a, b) };
        let (from, to) = (value(from), value(to));
        for (id, t) in [
            (&from, if backward { b } else { a }),
            (&to, if backward { a } else { b }),
        ] {
            self.node(id, t.text_range());
            self.undefined.push((id.clone(), t.text_range()));
        }
        let kind = rel.text().to_ascii_lowercase();
        let contains = kind == "contains";
        self.chart.edges.push(Edge {
            id: None,
            from,
            to,
            stroke: if contains {
                Stroke::Normal
            } else {
                Stroke::Dotted
            },
            start: contains.then_some(Arrow::Circle),
            end: (!contains).then_some(Arrow::Point),
            length: 1,
            constraint: true,
            ports: [None; 2],
            rows: [None; 2],
            label: Some(Label::plain(format!("«{kind}»"))),
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
            match self.class_index.get(n) {
                Some(&i) => self.chart.classes[i].styles.extend(styles.iter().cloned()),
                None => {
                    self.class_index
                        .insert(n.to_owned(), self.chart.classes.len());
                    self.chart.classes.push(ClassDef {
                        name: n.to_owned(),
                        styles: styles.clone(),
                    });
                }
            }
        }
    }

    /// `class a,b name`, or `name:::a,b`.
    fn class(&mut self, stmt: &SyntaxNode) {
        if token(stmt, CLASS_KW).is_none() {
            if let Some(name) = tokens(stmt).find(|t| matches!(t.kind(), IDENT | STRING)) {
                self.shorthand(stmt, &value(&name));
            }
            return;
        }
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
        let mut warned = HashSet::new();
        for (id, range) in std::mem::take(&mut self.undefined) {
            if !self.defined.contains(&id) && warned.insert(id.clone()) {
                self.warning(
                    range,
                    format!("`{id}` is not defined; drawing it as a plain box"),
                );
            }
        }
        for (id, styling, range) in std::mem::take(&mut self.styling) {
            let Some(&i) = self.node_index.get(&id) else {
                self.warning(range, format!("unknown requirement or element `{id}`"));
                continue;
            };
            let n = &mut self.chart.nodes[i];
            match styling {
                Styling::Class(c) => n.classes.push(c),
                Styling::Styles(s) => n.styles.extend(s),
            }
        }
        for (node, fields) in self.chart.nodes.iter_mut().zip(&self.fields) {
            let rows: Vec<Row> = fields
                .iter()
                .flatten()
                .map(|(label, v)| Row {
                    cells: vec![(*label).to_owned(), v.clone()],
                    ..Row::default()
                })
                .collect();
            if !rows.is_empty() {
                node.sections = vec![rows];
            }
        }
        (self.chart, self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields() {
        assert_eq!(
            field(false, "Risk", "high"),
            Ok((2, "Risk:", "High".into()))
        );
        assert_eq!(
            field(false, "verifyMethod", "TEST"),
            Ok((3, "Verification:", "Test".into()))
        );
        assert_eq!(
            field(true, "docRef", "a/b"),
            Ok((1, "Doc Ref:", "a/b".into()))
        );
        assert!(field(false, "risk", "huge").is_err());
        assert!(field(false, "type", "x").is_err());
        assert!(field(true, "id", "x").is_err());
        assert!(field(false, "verifymethod", "guess").is_err());
    }

    #[test]
    fn stereotypes() {
        for k in KINDS {
            assert!(!stereotype(k).is_empty());
        }
        assert_eq!(stereotype("designconstraint"), "Design Constraint");
    }
}
