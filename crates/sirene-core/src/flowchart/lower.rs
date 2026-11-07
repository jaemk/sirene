//! Lowers the flowchart syntax tree to the semantic model (FLOWMO-*).

use std::collections::HashMap;

use rowan::TextRange;

use super::model::{
    Arrow, ClassDef, Click, Direction, Edge, Flowchart, Label, Metadata, Node, Shape, Stroke,
    Style, Subgraph,
};
use super::text;
use crate::diagnostic::Diagnostic;
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
    if let Some(dir) = child(root, HEADER).and_then(|h| token(&h, DIR)) {
        l.chart.direction = Direction::from_text(dir.text()).unwrap_or_default();
    }
    l.statements(root, false);
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

fn push_unique(list: &mut Vec<String>, id: String) {
    if !list.contains(&id) {
        list.push(id);
    }
}

fn merge_metadata(into: &mut Metadata, entries: Metadata) {
    for (k, v) in entries {
        match into.iter_mut().find(|(key, _)| *key == k) {
            Some(entry) => entry.1 = v,
            None => into.push((k, v)),
        }
    }
}

/// The label of a `LABEL` node (FLOWMO-6).
fn label_of(node: &SyntaxNode) -> Label {
    tokens(node)
        .find_map(|t| match t.kind() {
            STRING => Some(text::label(text::unquote(t.text()), false)),
            MD_STRING => Some(text::label(text::unquote(t.text()), true)),
            TEXT => Some(text::label(t.text(), false)),
            _ => None,
        })
        .unwrap_or_default()
}

/// Stroke, arrowheads, and length of an edge (FLOWMO-8), or `None` when the parser could not
/// complete it.
fn edge_style(edge: &SyntaxNode) -> Option<(Stroke, Option<Arrow>, Option<Arrow>, u8)> {
    let open = token(edge, STROKE_OPEN);
    let stroke = token(edge, STROKE)?;
    let marker_text = open.as_ref().unwrap_or(&stroke).text().to_owned();
    let start = match marker_text.as_bytes()[0] {
        b'<' => Some(Arrow::Point),
        b'x' => Some(Arrow::Cross),
        b'o' => Some(Arrow::Circle),
        _ => None,
    };
    let s = stroke.text();
    let line = if open.is_none() && start.is_some() {
        &s[1..]
    } else {
        s
    };
    let end = match line.as_bytes()[line.len() - 1] {
        b'>' => Some(Arrow::Point),
        b'x' => Some(Arrow::Cross),
        b'o' => Some(Arrow::Circle),
        _ => None,
    };
    let dots = line.bytes().filter(|&b| b == b'.').count();
    let (kind, length) = if line.starts_with('~') {
        (Stroke::Invisible, line.len() - 2)
    } else if dots > 0 {
        (Stroke::Dotted, dots)
    } else if line.starts_with('=') {
        (Stroke::Thick, line.len() - 2)
    } else {
        (Stroke::Normal, line.len() - 2)
    };
    let length = u8::try_from(length.clamp(1, 10)).expect("clamped");
    if kind == Stroke::Invisible {
        return Some((kind, None, None, length));
    }
    Some((kind, start, end, length))
}

#[derive(Default)]
struct Lowerer {
    chart: Flowchart,
    diagnostics: Vec<Diagnostic>,
    node_index: HashMap<String, usize>,
    edge_ids: HashMap<String, usize>,
    /// Subgraphs in the order their `end`s appear (FLOWMO-10).
    subgraphs: Vec<Subgraph>,
    subgraph_count: usize,
}

impl Lowerer {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        // FLOWMO-10: a node with a subgraph's id is that subgraph.
        let mut merged = Vec::new();
        for sg in &mut self.subgraphs {
            if let Some(&i) = self.node_index.get(&sg.id) {
                let node = &self.chart.nodes[i];
                sg.classes.extend(node.classes.iter().cloned());
                sg.styles.extend(node.styles.iter().cloned());
                merged.push(sg.id.clone());
            }
        }
        self.chart.nodes.retain(|n| !merged.contains(&n.id));
        self.subgraphs.sort_by_key(|s| s.range.start());
        self.chart.subgraphs = self.subgraphs;
        (self.chart, self.diagnostics)
    }

    /// Lowers the statements directly under `parent`. Returns the ids of the nodes and
    /// subgraphs they mention, and the last `direction`.
    fn statements(
        &mut self,
        parent: &SyntaxNode,
        nested: bool,
    ) -> (Vec<String>, Option<Direction>) {
        let mut members = Vec::new();
        let mut direction = None;
        for stmt in parent.children() {
            match stmt.kind() {
                VERTEX_STMT => {
                    for id in self.vertex_stmt(&stmt) {
                        push_unique(&mut members, id);
                    }
                }
                SUBGRAPH => {
                    let id = self.subgraph(&stmt);
                    push_unique(&mut members, id);
                }
                DIRECTION_STMT => {
                    let dir = token(&stmt, DIR).and_then(|t| Direction::from_text(t.text()));
                    if nested {
                        direction = dir.or(direction);
                    } else {
                        self.warning(
                            stmt.text_range(),
                            "`direction` only applies inside a subgraph; use the header direction",
                        );
                    }
                }
                CLASSDEF_STMT => self.class_def_stmt(&stmt),
                CLASS_STMT => self.class_stmt(&stmt),
                STYLE_STMT => self.style_stmt(&stmt),
                LINKSTYLE_STMT => self.link_style_stmt(&stmt),
                CLICK_STMT => self.click_stmt(&stmt),
                ACC_TITLE_STMT => self.chart.acc_title = Some(acc_text(&stmt)),
                ACC_DESCR_STMT => self.chart.acc_descr = Some(acc_text(&stmt)),
                _ => {}
            }
        }
        (members, direction)
    }

    // ---- vertex statements ----

    /// FLOWMO-4, FLOWMO-8. Returns the ids of the nodes mentioned.
    fn vertex_stmt(&mut self, stmt: &SyntaxNode) -> Vec<String> {
        let mut members = Vec::new();
        let mut prev: Option<Vec<String>> = None;
        let mut edge = None;
        for part in stmt.children() {
            match part.kind() {
                NODE_GROUP => {
                    let ids: Vec<String> = part
                        .children()
                        .filter(|n| n.kind() == NODE)
                        .filter_map(|n| self.node(&n))
                        .collect();
                    if let (Some(from), Some(e)) = (&prev, edge.take()) {
                        self.edges(from, &ids, &e);
                    }
                    for id in &ids {
                        push_unique(&mut members, id.clone());
                    }
                    prev = Some(ids);
                }
                EDGE => edge = Some(part),
                _ => {}
            }
        }
        members
    }

    /// Applies one node mention. Returns the node id, or `None` when the mention refers to an
    /// edge or subgraph (FLOWMO-7) or is invalid.
    fn node(&mut self, syntax: &SyntaxNode) -> Option<String> {
        let id = token(syntax, IDENT)?.text().to_owned();
        if id == "end" {
            return None;
        }
        let meta = child(syntax, METADATA).map(|m| self.metadata(&m));
        if let Some(&e) = self.edge_ids.get(&id) {
            if let Some(entries) = meta {
                self.edge_metadata(e, entries);
            }
            return None;
        }
        if let Some(entries) = &meta
            && let Some(sg) = self.subgraphs.iter_mut().find(|s| s.id == id)
        {
            merge_metadata(&mut sg.metadata, entries.clone());
            return None;
        }
        let i = self.node_entry(&id);
        self.chart.nodes[i].mentions.push(syntax.text_range());
        if let Some(shape) = child(syntax, SHAPE) {
            let open = token(&shape, SHAPE_OPEN).map(|t| t.text().to_owned());
            let close = token(&shape, SHAPE_CLOSE).map(|t| t.text().to_owned());
            let node = &mut self.chart.nodes[i];
            node.shape = Shape::from_delimiters(
                open.as_deref().unwrap_or("["),
                close.as_deref().unwrap_or(""),
            );
            node.label = child(&shape, LABEL)
                .map(|l| label_of(&l))
                .unwrap_or_default();
        }
        if let Some(class) = child(syntax, CLASS_SHORTHAND).and_then(|c| token(&c, IDENT)) {
            self.chart.nodes[i].classes.push(class.text().to_owned());
        }
        if let (Some(entries), Some(m)) = (meta, child(syntax, METADATA)) {
            self.node_metadata(i, entries, m.text_range());
        }
        Some(id)
    }

    /// The index of node `id`, creating it with its id as label.
    fn node_entry(&mut self, id: &str) -> usize {
        if let Some(&i) = self.node_index.get(id) {
            return i;
        }
        let i = self.chart.nodes.len();
        self.chart.nodes.push(Node {
            id: id.to_owned(),
            label: Label::plain(id),
            shape: Shape::Rect,
            classes: Vec::new(),
            styles: Vec::new(),
            click: None,
            tooltip: None,
            metadata: Vec::new(),
            mentions: Vec::new(),
        });
        self.node_index.insert(id.to_owned(), i);
        i
    }

    /// Parses a `METADATA` node's body, reporting malformed entries.
    fn metadata(&mut self, syntax: &SyntaxNode) -> Metadata {
        let body = token(syntax, META_BODY);
        let (entries, invalid) = text::metadata(body.as_ref().map_or("", |t| t.text()));
        for bad in invalid {
            self.error(
                syntax.text_range(),
                format!("expected `key: value` in metadata, found `{bad}`"),
            );
        }
        entries
    }

    fn node_metadata(&mut self, i: usize, entries: Metadata, range: TextRange) {
        let get = |key: &str| {
            entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let shape = get("shape").map(|name| (name.to_owned(), Shape::from_name(name)));
        let markdown = !matches!(get("labelType"), Some("text" | "string"));
        let label = get("label")
            .filter(|l| !l.is_empty())
            .map(|l| text::label(l, markdown));
        let node = &mut self.chart.nodes[i];
        if let Some(label) = label {
            node.label = label;
        }
        merge_metadata(&mut node.metadata, entries);
        match shape {
            Some((_, Some(s))) => node.shape = s,
            Some((name, None)) => self.error(range, format!("unknown shape `{name}`")),
            None => {}
        }
    }

    fn edge_metadata(&mut self, e: usize, entries: Metadata) {
        let edge = &mut self.chart.edges[e];
        for (k, v) in &entries {
            match k.as_str() {
                "animate" => edge.animate = v == "true",
                "animation" => edge.animation = Some(v.clone()),
                "curve" => edge.curve = Some(v.clone()),
                _ => {}
            }
        }
        merge_metadata(&mut edge.metadata, entries);
    }

    /// FLOWMO-8: one edge per source and target pair.
    fn edges(&mut self, from: &[String], to: &[String], syntax: &SyntaxNode) {
        let Some((stroke, start, end, length)) = edge_style(syntax) else {
            return;
        };
        let label = child(syntax, EDGE_LABEL)
            .and_then(|l| child(&l, LABEL))
            .map(|l| label_of(&l))
            .filter(|l| !l.text.is_empty());
        let mut user_id = child(syntax, EDGE_ID).and_then(|n| {
            let id = token(&n, IDENT)?.text().to_owned();
            if self.edge_ids.contains_key(&id) {
                self.warning(
                    n.text_range(),
                    format!("duplicate edge id `{id}` is ignored"),
                );
                return None;
            }
            Some(id)
        });
        for (i, f) in from.iter().enumerate() {
            for (j, t) in to.iter().enumerate() {
                let id = if i + 1 == from.len() && j == 0 {
                    user_id.take()
                } else {
                    None
                };
                if let Some(id) = &id {
                    self.edge_ids.insert(id.clone(), self.chart.edges.len());
                }
                self.chart.edges.push(Edge {
                    id,
                    from: f.clone(),
                    to: t.clone(),
                    stroke,
                    start,
                    end,
                    length,
                    label: label.clone(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                    curve: None,
                    animate: false,
                    animation: None,
                    metadata: Vec::new(),
                    range: syntax.text_range(),
                });
            }
        }
    }

    // ---- subgraphs ----

    /// FLOWMO-9, FLOWMO-10. Returns the subgraph's id.
    fn subgraph(&mut self, syntax: &SyntaxNode) -> String {
        let header = child(syntax, SUBGRAPH_HEADER).expect("subgraph without a header");
        let (explicit, title) = subgraph_header(&header);
        let (members, direction) = self.statements(syntax, true);
        let id = explicit.unwrap_or_else(|| format!("subGraph{}", self.subgraph_count));
        self.subgraph_count += 1;

        let mut kept = Vec::new();
        for m in &members {
            if *m == id {
                continue;
            }
            if self.nested_in(&id, std::slice::from_ref(m)) {
                self.warning(
                    header.text_range(),
                    format!("subgraph `{m}` cannot be nested in its own descendant `{id}`"),
                );
                continue;
            }
            match self.owner(m) {
                Some(owner) => {
                    if owner != id && !self.nested_in(&owner, &members) {
                        self.warning(
                            header.text_range(),
                            format!("`{m}` already belongs to subgraph `{owner}`"),
                        );
                    }
                }
                None => kept.push(m.clone()),
            }
        }
        match self.subgraphs.iter_mut().find(|s| s.id == id) {
            Some(existing) => {
                for m in kept {
                    push_unique(&mut existing.members, m);
                }
            }
            None => self.subgraphs.push(Subgraph {
                id: id.clone(),
                title,
                direction,
                members: kept,
                classes: Vec::new(),
                styles: Vec::new(),
                metadata: Vec::new(),
                range: syntax.text_range(),
            }),
        }
        id
    }

    /// The id of the resolved subgraph that has `id` as a member.
    fn owner(&self, id: &str) -> Option<String> {
        self.subgraphs
            .iter()
            .find(|s| s.members.iter().any(|m| m == id))
            .map(|s| s.id.clone())
    }

    /// Whether subgraph `id` sits, at any depth, under a subgraph with `members`.
    fn nested_in(&self, id: &str, members: &[String]) -> bool {
        let mut seen = Vec::new();
        let mut current = Some(id.to_owned());
        while let Some(c) = current {
            if members.contains(&c) {
                return true;
            }
            if seen.contains(&c) {
                break;
            }
            current = self.owner(&c);
            seen.push(c);
        }
        false
    }

    // ---- other statements ----

    /// Parses style text, reporting declarations without `:` (FLOWMO-12).
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

    /// FLOWMO-11.
    fn class_def_stmt(&mut self, stmt: &SyntaxNode) {
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

    /// FLOWMO-11.
    fn class_stmt(&mut self, stmt: &SyntaxNode) {
        let (Some(ids), Some(class)) = (child(stmt, ID_LIST), token(stmt, IDENT)) else {
            return;
        };
        let class = class.text().to_owned();
        for t in tokens(&ids).filter(|t| t.kind() == IDENT) {
            let id = t.text();
            let mut found = false;
            if let Some(&i) = self.node_index.get(id) {
                self.chart.nodes[i].classes.push(class.clone());
                found = true;
            }
            if let Some(&e) = self.edge_ids.get(id) {
                self.chart.edges[e].classes.push(class.clone());
                found = true;
            }
            if let Some(sg) = self.subgraphs.iter_mut().find(|s| s.id == id) {
                sg.classes.push(class.clone());
                found = true;
            }
            if !found {
                self.warning(
                    t.text_range(),
                    format!("`class` refers to unknown id `{id}`"),
                );
            }
        }
    }

    /// FLOWMO-12.
    fn style_stmt(&mut self, stmt: &SyntaxNode) {
        let (Some(id), Some(style)) = (token(stmt, IDENT), token(stmt, STYLE_TEXT)) else {
            return;
        };
        let styles = self.styles(&style);
        let name = id.text();
        if let Some(&i) = self.node_index.get(name) {
            self.chart.nodes[i].styles.extend(styles);
        } else if let Some(sg) = self.subgraphs.iter_mut().find(|s| s.id == name) {
            sg.styles.extend(styles);
        } else if let Some(&e) = self.edge_ids.get(name) {
            self.chart.edges[e].styles.extend(styles);
        } else {
            self.warning(
                id.text_range(),
                format!("`style` refers to unknown id `{name}`; creating a node"),
            );
            let i = self.node_entry(name);
            self.chart.nodes[i].styles.extend(styles);
        }
    }

    /// FLOWMO-13.
    fn link_style_stmt(&mut self, stmt: &SyntaxNode) {
        let curve = token(stmt, INTERPOLATE_KW)
            .and(token(stmt, IDENT))
            .map(|t| t.text().to_owned());
        let styles = token(stmt, STYLE_TEXT).map(|t| self.styles(&t));
        if token(stmt, DEFAULT_KW).is_some() {
            if let Some(curve) = curve {
                self.chart.default_curve = Some(curve);
            }
            if let Some(styles) = styles {
                self.chart.default_edge_styles = styles;
            }
            return;
        }
        let Some(list) = child(stmt, ID_LIST) else {
            return;
        };
        for t in tokens(&list).filter(|t| t.kind() == NUMBER) {
            let count = self.chart.edges.len();
            let Some(edge) = t
                .text()
                .parse::<usize>()
                .ok()
                .and_then(|i| self.chart.edges.get_mut(i))
            else {
                self.error(
                    t.text_range(),
                    format!("no edge at index {} (edges so far: {count})", t.text()),
                );
                continue;
            };
            if let Some(curve) = &curve {
                edge.curve = Some(curve.clone());
            }
            if let Some(styles) = &styles {
                edge.styles.clone_from(styles);
            }
        }
    }

    /// FLOWMO-14.
    fn click_stmt(&mut self, stmt: &SyntaxNode) {
        let toks: Vec<SyntaxToken> = tokens(stmt).filter(|t| t.kind() != WHITESPACE).collect();
        let kinds: Vec<SyntaxKind> = toks.iter().map(SyntaxToken::kind).collect();
        let tok_text = |i: usize| toks[i].text().to_owned();
        let string = |i: usize| text::unquote(toks[i].text()).to_owned();
        let after = |i: usize, kind: SyntaxKind| {
            (i..toks.len())
                .find(|&j| kinds[j] == kind)
                .map(|j| toks[j].text().to_owned())
        };
        // kinds[0] is CLICK_KW, kinds[1] the node id.
        let (click, tooltip_from) = match kinds.get(2..) {
            Some([HREF_KW, STRING, ..]) | Some([STRING, ..]) => {
                let url_at = if kinds[2] == HREF_KW { 3 } else { 2 };
                let target = after(url_at + 1, LINK_TARGET);
                (
                    Click::Link {
                        url: string(url_at),
                        target,
                    },
                    url_at + 1,
                )
            }
            Some([CALL_KW, IDENT, ..]) => {
                let args = after(4, CALLBACK_ARGS).unwrap_or_default();
                (
                    Click::Callback {
                        name: tok_text(3),
                        args: text::callback_args(&args),
                    },
                    4,
                )
            }
            Some([IDENT, ..]) => (
                Click::Callback {
                    name: tok_text(2),
                    args: Vec::new(),
                },
                3,
            ),
            _ => return,
        };
        let tooltip = (tooltip_from..toks.len())
            .find(|&j| kinds[j] == STRING)
            .map(string);
        let id = &toks[1];
        match self.node_index.get(id.text()) {
            Some(&i) => {
                let node = &mut self.chart.nodes[i];
                node.click = Some(click);
                if tooltip.is_some() {
                    node.tooltip = tooltip;
                }
            }
            None => self.warning(
                id.text_range(),
                format!("`click` refers to unknown node `{}`", id.text()),
            ),
        }
    }
}

/// Explicit id and title from a subgraph header (FLOWMO-9).
fn subgraph_header(header: &SyntaxNode) -> (Option<String>, Label) {
    if let Some(label) = child(header, LABEL) {
        let raw = tokens(&label)
            .find(|t| matches!(t.kind(), STRING | MD_STRING | TEXT))
            .map(|t| match t.kind() {
                TEXT => t.text().trim().to_owned(),
                _ => text::unquote(t.text()).trim().to_owned(),
            })
            .unwrap_or_default();
        let id = (!raw.is_empty() && !raw.contains(char::is_whitespace)).then_some(raw);
        return (id, label_of(&label));
    }
    let Some(ident) = token(header, IDENT) else {
        return (None, Label::default());
    };
    let id = ident.text().to_owned();
    let title = child(header, SHAPE)
        .map(|shape| {
            child(&shape, LABEL)
                .map(|l| label_of(&l))
                .unwrap_or_default()
        })
        .unwrap_or_else(|| Label::plain(&id));
    (Some(id), title)
}

fn acc_text(stmt: &SyntaxNode) -> String {
    token(stmt, ACC_TEXT).map_or_else(String::new, |t| t.text().trim().to_owned())
}
