//! Lowers a class diagram syntax tree to the graph model (MCLASS-2).

use std::collections::HashMap;

use rowan::TextRange;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::flowchart::{
    Arrow, ClassDef, Direction, Edge, Flowchart, Label, Node, Note, NoteSide, Row, Shape, Stroke,
    Style, Subgraph,
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
    l.block(root, None);
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

/// `~T~` generics as `<T>`.
fn generics(s: &str) -> String {
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

/// A member line: whether it is a method, and its row (MCLASS-1). `$` marks a static member and
/// `*` an abstract one, at the end or right after a method's `)`.
pub(crate) fn member(raw: &str) -> (bool, Row) {
    let mut t = raw.trim().to_owned();
    let (mut underline, mut italic) = (false, false);
    let mut take = |at: usize, t: &mut String| match t[at..].chars().next() {
        Some('$') => {
            underline = true;
            t.remove(at);
            true
        }
        Some('*') => {
            italic = true;
            t.remove(at);
            true
        }
        _ => false,
    };
    if let Some(close) = t.rfind(')') {
        while take(close + 1, &mut t) {}
    }
    while !t.is_empty() && take(t.len() - 1, &mut t) {}
    let method = t.contains('(');
    let row = Row {
        cells: vec![generics(t.trim())],
        italic,
        underline,
    };
    (method, row)
}

#[derive(Default)]
struct Class {
    annotation: Option<String>,
    attributes: Vec<Row>,
    methods: Vec<Row>,
}

#[derive(Default)]
struct Lowerer {
    chart: Flowchart,
    diagnostics: Vec<Diagnostic>,
    node_index: HashMap<String, usize>,
    classes: Vec<Class>,
    namespaces: Vec<Subgraph>,
    /// `:::`, `cssClass`, and `style`, applied once every class exists.
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

    /// The node for class `id`, created in `namespace` if new.
    fn class(&mut self, id: &str, namespace: Option<usize>) -> usize {
        if let Some(&i) = self.node_index.get(id) {
            return i;
        }
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
        self.classes.push(Class::default());
        self.node_index.insert(id.to_owned(), i);
        if let Some(n) = namespace {
            self.namespaces[n].members.push(id.to_owned());
        }
        i
    }

    /// The class a `CLASS_REF` names, with its generics and `:::` class.
    fn reference(&mut self, r: &SyntaxNode, namespace: Option<usize>) -> Option<usize> {
        let id = token(r, IDENT)?.text().to_owned();
        let i = self.class(&id, namespace);
        self.chart.nodes[i].mentions.push(r.text_range());
        if let Some(g) = token(r, GENERIC) {
            self.chart.nodes[i].label = Label::plain(format!("{id}{}", generics(g.text())));
        }
        let class = tokens(r)
            .skip_while(|t| t.kind() != TRIPLE_COLON)
            .find(|t| t.kind() == IDENT);
        if let Some(c) = class {
            self.styling
                .push((id, Styling::Class(c.text().to_owned()), r.text_range()));
        }
        Some(i)
    }

    fn add_member(&mut self, i: usize, raw: &str) {
        let raw = raw.trim();
        if raw.is_empty() {
            return;
        }
        let (method, row) = member(raw);
        let class = &mut self.classes[i];
        if method {
            class.methods.push(row);
        } else {
            class.attributes.push(row);
        }
    }

    fn block(&mut self, parent: &SyntaxNode, namespace: Option<usize>) {
        for stmt in parent.children() {
            match stmt.kind() {
                DIRECTION_STMT => {
                    if let Some(d) = text_of(&stmt, DIR).and_then(|d| Direction::from_text(&d)) {
                        self.chart.direction = d;
                    }
                }
                CLASS_DECL => self.class_decl(&stmt, namespace),
                MEMBER_STMT => {
                    if let Some(i) =
                        child(&stmt, CLASS_REF).and_then(|r| self.reference(&r, namespace))
                    {
                        let raw = text_of(&stmt, MEMBER).unwrap_or_default();
                        self.add_member(i, &raw);
                    }
                }
                ANNOTATION_STMT => {
                    let i = child(&stmt, CLASS_REF).and_then(|r| self.reference(&r, namespace));
                    if let (Some(i), Some(a)) = (i, text_of(&stmt, STEREOTYPE)) {
                        self.classes[i].annotation = Some(a);
                    }
                }
                RELATION_STMT => self.relation(&stmt, namespace),
                NAMESPACE => self.namespace(&stmt, namespace),
                NOTE_STMT => self.note(&stmt),
                CLASSDEF_STMT => self.class_def(&stmt),
                CSSCLASS_STMT => self.css_class(&stmt),
                STYLE_STMT => self.style(&stmt),
                ACC_TITLE_STMT => self.chart.acc_title = text_of(&stmt, ACC_TEXT),
                ACC_DESCR_STMT => self.chart.acc_descr = text_of(&stmt, ACC_TEXT),
                CLICK_STMT => {
                    let kw = text_of(&stmt, CLICK_KW).unwrap_or_default();
                    self.warning(
                        stmt.text_range(),
                        format!("`{kw}` is not supported in class diagrams; ignoring it"),
                    );
                }
                _ => {}
            }
        }
    }

    fn class_decl(&mut self, stmt: &SyntaxNode, namespace: Option<usize>) {
        let Some(i) = child(stmt, CLASS_REF).and_then(|r| self.reference(&r, namespace)) else {
            return;
        };
        if let Some(s) = token(stmt, STRING) {
            self.chart.nodes[i].label = text::label(text::unquote(s.text()), false);
        }
        for t in tokens(stmt) {
            match t.kind() {
                MEMBER => self.add_member(i, t.text()),
                STEREOTYPE => self.classes[i].annotation = Some(t.text().trim().to_owned()),
                _ => {}
            }
        }
    }

    fn relation(&mut self, stmt: &SyntaxNode, namespace: Option<usize>) {
        let refs: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == CLASS_REF).collect();
        let (Some(rel), [from, to]) = (token(stmt, RELATION), refs.as_slice()) else {
            return;
        };
        let (Some(a), Some(b)) = (
            self.reference(from, namespace),
            self.reference(to, namespace),
        ) else {
            return;
        };
        let rel = rel.text();
        let line_at = rel.find("--").or_else(|| rel.find("..")).unwrap_or(0);
        let marker = |s: &str| match s {
            "<|" | "|>" => Some(Arrow::Triangle),
            "*" => Some(Arrow::Diamond),
            "o" => Some(Arrow::HollowDiamond),
            "<" | ">" => Some(Arrow::Point),
            _ => None,
        };
        let (start, end) = (marker(&rel[..line_at]), marker(&rel[line_at + 2..]));
        let stroke = if rel[line_at..].starts_with("..") {
            Stroke::Dotted
        } else {
            Stroke::Normal
        };
        // Cardinalities: a string before the relation token belongs to the start, after it to
        // the end.
        let rel_at = token(stmt, RELATION).map(|t| t.text_range().start());
        let mut start_label = None;
        let mut end_label = None;
        for s in tokens(stmt).filter(|t| t.kind() == STRING) {
            let l = Some(Label::plain(text::unquote(s.text())));
            if rel_at.is_some_and(|at| s.text_range().start() < at) {
                start_label = l;
            } else {
                end_label = l;
            }
        }
        let label = text_of(stmt, TEXT)
            .filter(|t| !t.is_empty())
            .map(|t| text::label(&t, false));
        self.chart.edges.push(Edge {
            id: None,
            from: self.chart.nodes[a].id.clone(),
            to: self.chart.nodes[b].id.clone(),
            stroke,
            start,
            end,
            length: 1,
            constraint: true,
            label,
            start_label,
            end_label,
            classes: Vec::new(),
            styles: Vec::new(),
            curve: None,
            animate: false,
            animation: None,
            metadata: Vec::new(),
            range: stmt.text_range(),
        });
    }

    fn namespace(&mut self, stmt: &SyntaxNode, parent: Option<usize>) {
        let Some(id) = text_of(stmt, IDENT) else {
            return;
        };
        let k = match self.namespaces.iter().position(|s| s.id == id) {
            Some(k) => k,
            None => {
                self.namespaces.push(Subgraph {
                    id: id.clone(),
                    title: Label::plain(&id),
                    direction: None,
                    members: Vec::new(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                    metadata: Vec::new(),
                    range: stmt.text_range(),
                    region: false,
                });
                if let Some(p) = parent {
                    self.namespaces[p].members.push(id);
                }
                self.namespaces.len() - 1
            }
        };
        self.block(stmt, Some(k));
    }

    fn note(&mut self, stmt: &SyntaxNode) {
        let body = token(stmt, STRING).map(|s| text::unquote(s.text()).replace("\\n", "\n"));
        let Some(body) = body else {
            return;
        };
        let Some(target) = text_of(stmt, IDENT) else {
            // A note on its own is a node that no relation can name: `:` never appears in a
            // class name.
            let id = format!("note:{}", self.chart.nodes.len());
            self.chart.nodes.push(Node {
                id,
                label: text::label(&body, false),
                shape: Shape::Note,
                sections: Vec::new(),
                classes: Vec::new(),
                styles: Vec::new(),
                click: None,
                tooltip: None,
                metadata: Vec::new(),
                mentions: vec![stmt.text_range()],
            });
            self.classes.push(Class::default());
            return;
        };
        self.class(&target, None);
        self.chart.notes.push(Note {
            target,
            side: NoteSide::After,
            label: text::label(&body, false),
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
        for name in tokens(&names).filter(|t| t.kind() == IDENT) {
            let name = name.text();
            match self.chart.classes.iter_mut().find(|c| c.name == name) {
                Some(class) => class.styles.extend(styles.iter().cloned()),
                None => self.chart.classes.push(ClassDef {
                    name: name.to_owned(),
                    styles: styles.clone(),
                }),
            }
        }
    }

    fn css_class(&mut self, stmt: &SyntaxNode) {
        let (Some(list), Some(class)) = (token(stmt, STRING), text_of(stmt, IDENT)) else {
            return;
        };
        for id in text::unquote(list.text()).split(',') {
            self.styling.push((
                id.trim().to_owned(),
                Styling::Class(class.clone()),
                list.text_range(),
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
                self.warning(range, format!("unknown class `{id}`"));
                continue;
            };
            let n = &mut self.chart.nodes[i];
            match styling {
                Styling::Class(c) => n.classes.push(c),
                Styling::Styles(s) => n.styles.extend(s),
            }
        }
        // MCLASS-2: the annotation above the name, then attributes and methods; both sections
        // when either has rows.
        for (node, class) in self.chart.nodes.iter_mut().zip(self.classes) {
            if let Some(a) = class.annotation {
                let name = a.trim_start_matches("<<").trim_end_matches(">>").trim();
                node.label.text = format!("«{name}»\n{}", node.label.text);
            }
            if !class.attributes.is_empty() || !class.methods.is_empty() {
                node.sections = vec![class.attributes, class.methods];
            }
        }
        self.chart.subgraphs = self.namespaces;
        (self.chart, self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members() {
        let m = |s: &str| {
            let (method, row) = member(s);
            (method, row.cells[0].clone(), row.italic, row.underline)
        };
        assert_eq!(m("+int id"), (false, "+int id".into(), false, false));
        assert_eq!(m("+run() bool"), (true, "+run() bool".into(), false, false));
        assert_eq!(
            m("+make()$ Shape"),
            (true, "+make() Shape".into(), false, true)
        );
        assert_eq!(m("draw()*"), (true, "draw()".into(), true, false));
        assert_eq!(m("String id$"), (false, "String id".into(), false, true));
        assert_eq!(
            m("List~int~ items"),
            (false, "List<int> items".into(), false, false)
        );
        assert_eq!(m("  "), (false, String::new(), false, false));
    }
}
