//! Lowers a class diagram syntax tree to the graph model (MCLASS-2).

use std::collections::HashMap;

use rowan::TextRange;

use super::parser::opens;
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::flowchart::{
    Arrow, ClassDef, Click, Direction, Edge, Flowchart, Label, Node, Note, NoteSide, Row, Shape,
    Stroke, Style, Subgraph,
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
    let mut depth = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '~' {
            out.push(c);
        } else if opens(depth, chars.peek().copied()) {
            out.push('<');
            depth += 1;
        } else {
            out.push('>');
            depth -= 1;
        }
    }
    out
}

/// The pieces after a class id in a `click` line: `"strings"` (unquoted) and bare words.
fn click_pieces(s: &str) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    let mut r = s.trim_start();
    while !r.is_empty() {
        if let Some(body) = r.strip_prefix('"') {
            let end = body.find('"').unwrap_or(body.len());
            out.push((true, &body[..end]));
            r = body.get(end + 1..).unwrap_or("");
        } else {
            let end = r.find(char::is_whitespace).unwrap_or(r.len());
            out.push((false, &r[..end]));
            r = &r[end..];
        }
        r = r.trim_start();
    }
    out
}

/// The class id, action, and tooltip of a `click`, `link`, or `callback` line (MCLASS-6).
fn parse_click(kw: &str, s: &str) -> Option<(String, Click, Option<String>)> {
    let s = s.trim();
    let (id, rest) = match s.strip_prefix('`') {
        Some(t) => {
            let end = t.find('`')?;
            (&t[..end], &t[end + 1..])
        }
        None => s.split_at(s.find(char::is_whitespace).unwrap_or(s.len())),
    };
    if id.is_empty() {
        return None;
    }
    let rest = rest.trim_start();
    let first_word = rest.split_whitespace().next().unwrap_or("");
    let link = |rest: &str| {
        let pieces = click_pieces(rest);
        let (true, url) = *pieces.first()? else {
            return None;
        };
        let tooltip = pieces.iter().skip(1).find(|p| p.0).map(|p| p.1.to_owned());
        let target = pieces
            .iter()
            .skip(1)
            .find(|p| !p.0 && p.1.starts_with('_'))
            .map(|p| p.1.to_owned());
        Some((
            Click::Link {
                url: url.to_owned(),
                target,
            },
            tooltip,
        ))
    };
    let (click, tooltip) = match kw {
        "link" => link(rest)?,
        "callback" => {
            let pieces = click_pieces(rest);
            let (true, name) = *pieces.first()? else {
                return None;
            };
            let tooltip = pieces.get(1).filter(|p| p.0).map(|p| p.1.to_owned());
            let click = Click::Callback {
                name: name.to_owned(),
                args: Vec::new(),
            };
            (click, tooltip)
        }
        _ if first_word.eq_ignore_ascii_case("href") => link(&rest[4..])?,
        _ => {
            // `call fn(args)` or a bare `fn`, then an optional tooltip.
            let call = rest
                .get(..4)
                .is_some_and(|w| w.eq_ignore_ascii_case("call"))
                && first_word.len() == 4;
            let r = if call { rest[4..].trim_start() } else { rest };
            let name_end = r
                .find(|c: char| c == '(' || c.is_whitespace())
                .unwrap_or(r.len());
            let (name, mut after) = r.split_at(name_end);
            if name.is_empty() {
                return None;
            }
            let mut args = Vec::new();
            if let Some(inner) = after.strip_prefix('(') {
                let close = inner.rfind(')')?;
                args = text::callback_args(&inner[..close]);
                after = &inner[close + 1..];
            }
            let tooltip = click_pieces(after)
                .into_iter()
                .find(|p| p.0)
                .map(|p| p.1.to_owned());
            let click = Click::Callback {
                name: name.to_owned(),
                args,
            };
            (click, tooltip)
        }
    };
    Some((id.to_owned(), click, tooltip))
}

/// A class id without the backticks around it.
fn unticked(id: &str) -> &str {
    id.strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .unwrap_or(id)
}

/// A member line: whether it is a method, and its row (MCLASS-1). `$` marks a static member and
/// `*` an abstract one, at the end or right after a method's `)`.
pub(crate) fn member(raw: &str) -> (bool, Row) {
    let mut t = raw.trim().to_owned();
    let (mut underline, mut italic) = (false, false);
    let mut take = |at: usize, t: &mut String| match t.get(at..).and_then(|s| s.chars().next()) {
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
    // A leading `~` is package visibility, not a generic.
    let t = t.trim();
    let cell = match t.strip_prefix('~') {
        Some(rest) => format!("~{}", generics(rest)),
        None => generics(t),
    };
    let row = Row {
        cells: vec![cell],
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
    /// The index in `namespaces` of each namespace id.
    namespace_index: HashMap<String, usize>,
    /// The namespace each class was last placed in by `join`.
    home: HashMap<String, usize>,
    /// The index in `chart.classes` of each `classDef` name.
    class_index: HashMap<String, usize>,
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

    /// The node for class `id`, created if new.
    fn class(&mut self, id: &str) -> usize {
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
        i
    }

    /// Moves class `id` into namespace `n`, out of any other: a `class` declaration in a
    /// namespace places it there, wherever the class was first mentioned.
    fn join(&mut self, id: &str, n: usize) {
        if let Some(old) = self.home.insert(id.to_owned(), n) {
            self.namespaces[old].members.retain(|m| m != id);
        }
        self.namespaces[n].members.push(id.to_owned());
    }

    /// The class a `CLASS_REF` names, with its generics and `:::` class.
    fn reference(&mut self, r: &SyntaxNode) -> Option<usize> {
        let id = unticked(token(r, IDENT)?.text()).to_owned();
        let i = self.class(&id);
        self.chart.nodes[i].mentions.push(r.text_range());
        // Generics replace the label only while it is the id, not one from `class A["Label"]`.
        let label = &self.chart.nodes[i].label.text;
        let named = label == &id || label.starts_with(&format!("{id}<"));
        if let Some(g) = token(r, GENERIC)
            && named
        {
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
            if namespace.is_some() && matches!(stmt.kind(), RELATION_STMT | MEMBER_STMT) {
                self.warning(
                    stmt.text_range(),
                    "a `namespace` holds `class` declarations, notes, and namespaces; this \
                     line belongs outside it",
                );
            }
            match stmt.kind() {
                DIRECTION_STMT => {
                    if let Some(d) = text_of(&stmt, DIR).and_then(|d| Direction::from_text(&d)) {
                        self.chart.direction = d;
                    }
                }
                CLASS_DECL => self.class_decl(&stmt, namespace),
                MEMBER_STMT => {
                    if let Some(i) = child(&stmt, CLASS_REF).and_then(|r| self.reference(&r)) {
                        let raw = text_of(&stmt, MEMBER).unwrap_or_default();
                        self.add_member(i, &raw);
                    }
                }
                ANNOTATION_STMT => {
                    let i = child(&stmt, CLASS_REF).and_then(|r| self.reference(&r));
                    if let (Some(i), Some(a)) = (i, text_of(&stmt, STEREOTYPE)) {
                        self.classes[i].annotation = Some(a);
                    }
                }
                RELATION_STMT => self.relation(&stmt),
                NAMESPACE => self.namespace(&stmt, namespace),
                NOTE_STMT => self.note(&stmt),
                CLASSDEF_STMT => self.class_def(&stmt),
                CSSCLASS_STMT => self.css_class(&stmt),
                STYLE_STMT => self.style(&stmt),
                ACC_TITLE_STMT => self.chart.acc_title = text_of(&stmt, ACC_TEXT),
                ACC_DESCR_STMT => self.chart.acc_descr = text_of(&stmt, ACC_TEXT),
                CLICK_STMT => self.click(&stmt),
                _ => {}
            }
        }
    }

    /// `link A "url"`, `click A href "url"`, `callback A "fn"`, `click A call fn(args)`, or
    /// `click A fn`, each with an optional tooltip string (and a link target for links).
    fn click(&mut self, stmt: &SyntaxNode) {
        let kw = text_of(stmt, CLICK_KW).unwrap_or_default();
        let rest = text_of(stmt, TEXT).unwrap_or_default();
        let Some((id, click, tooltip)) = parse_click(&kw.to_ascii_lowercase(), &rest) else {
            self.warning(
                stmt.text_range(),
                format!("expected a class and a link or callback after `{kw}`; ignoring it"),
            );
            return;
        };
        match self.node_index.get(&id) {
            Some(&i) => {
                let node = &mut self.chart.nodes[i];
                node.click = Some(click);
                if tooltip.is_some() {
                    node.tooltip = tooltip;
                }
            }
            None => self.warning(
                stmt.text_range(),
                format!("`{kw}` refers to unknown class `{id}`"),
            ),
        }
    }

    fn class_decl(&mut self, stmt: &SyntaxNode, namespace: Option<usize>) {
        let Some(i) = child(stmt, CLASS_REF).and_then(|r| self.reference(&r)) else {
            return;
        };
        if let Some(n) = namespace {
            let id = self.chart.nodes[i].id.clone();
            self.join(&id, n);
        }
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

    fn relation(&mut self, stmt: &SyntaxNode) {
        let refs: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == CLASS_REF).collect();
        let (Some(rel), [from, to]) = (token(stmt, RELATION), refs.as_slice()) else {
            return;
        };
        let (Some(a), Some(b)) = (self.reference(from), self.reference(to)) else {
            return;
        };
        let rel = rel.text();
        let line_at = rel.find("--").or_else(|| rel.find("..")).unwrap_or(0);
        let marker = |s: &str| match s {
            "<|" | "|>" => Some(Arrow::Triangle),
            "*" => Some(Arrow::Diamond),
            "o" => Some(Arrow::HollowDiamond),
            "<" | ">" => Some(Arrow::Point),
            "()" => Some(Arrow::Circle),
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
            ports: [None; 2],
            rows: [None; 2],
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
        let k = match self.namespace_index.get(&id) {
            Some(&k) => k,
            None => {
                self.namespace_index
                    .insert(id.clone(), self.namespaces.len());
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
        let Some(target) = text_of(stmt, IDENT).map(|t| unticked(&t).to_owned()) else {
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
        self.class(&target);
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
            match self.class_index.get(name) {
                Some(&i) => self.chart.classes[i].styles.extend(styles.iter().cloned()),
                None => {
                    self.class_index
                        .insert(name.to_owned(), self.chart.classes.len());
                    self.chart.classes.push(ClassDef {
                        name: name.to_owned(),
                        styles: styles.clone(),
                    });
                }
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
        assert_eq!(m("~int x"), (false, "~int x".into(), false, false));
        assert_eq!(
            m("~List~int~ items"),
            (false, "~List<int> items".into(), false, false)
        );
        assert_eq!(m("~run()"), (true, "~run()".into(), false, false));
        assert_eq!(m("é"), (false, "é".into(), false, false));
        assert_eq!(m("run()é"), (true, "run()é".into(), false, false));
        assert_eq!(m("  "), (false, String::new(), false, false));
    }
}
