//! Lowers the flowchart syntax tree to the semantic model (FLOWMO-*).

use std::collections::{HashMap, HashSet};

use rowan::TextRange;

use super::model::{
    Arrow, ClassDef, Click, Direction, Edge, Flowchart, Label, Metadata, Node, Shape, Stroke,
    Style, Subgraph,
};
use super::{agentflow, text};
use crate::diagnostic::Diagnostic;
use crate::lines::MAX_DEPTH;
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
    if let Some(header) = child(root, HEADER) {
        let kw = token(&header, GRAPH_KW);
        l.chart.lanes = kw.as_ref().is_some_and(|t| t.text() == super::SWIMLANE);
        l.chart.agentflow = kw.is_some_and(|t| t.text() == super::AGENTFLOW);
        if let Some(dir) = token(&header, DIR) {
            l.chart.direction = Direction::from_text(dir.text()).unwrap_or_default();
        }
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

/// Ids in first-mention order, without repeats.
#[derive(Default)]
struct Ids {
    list: Vec<String>,
    seen: HashSet<String>,
}

impl Ids {
    fn push(&mut self, id: String) {
        if !self.seen.contains(&id) {
            self.seen.insert(id.clone());
            self.list.push(id);
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.seen.contains(id)
    }
}

/// The most metadata keys an element keeps. Mermaid reads a few; the cap keeps each merge a
/// short scan however often an element's metadata is given.
pub const MAX_META_KEYS: usize = 256;

/// Sets `entries` in `into`. Returns whether a new key was left out past `MAX_META_KEYS`.
#[must_use]
fn merge_metadata(into: &mut Metadata, entries: Metadata) -> bool {
    let mut dropped = false;
    for (k, v) in entries {
        if let Some(entry) = into.iter_mut().find(|(key, _)| *key == k) {
            entry.1 = v;
        } else if into.len() < MAX_META_KEYS {
            into.push((k, v));
        } else {
            dropped = true;
        }
    }
    dropped
}

/// The label of a `LABEL` node (FLOWMO-6).
pub(crate) fn label_of(node: &SyntaxNode) -> Label {
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
pub(crate) fn edge_style(edge: &SyntaxNode) -> Option<(Stroke, Option<Arrow>, Option<Arrow>, u8)> {
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
    /// Index in `subgraphs` by id.
    subgraph_index: HashMap<String, usize>,
    /// The id of the subgraph in `subgraphs` that has each member.
    owners: HashMap<String, String>,
    /// Subgraph levels at and below each subgraph: 1 for one holding only nodes.
    heights: HashMap<String, usize>,
    subgraph_count: usize,
    /// Subgraphs that lost a member to one that listed it first.
    lost: Vec<String>,
    /// Ids in agentflow `global` blocks, which belong to no `flow` (MAGF-2).
    globals: Ids,
    /// The index in `chart.classes` of each `classDef` name.
    class_index: HashMap<String, usize>,
    /// Whether `MAX_EDGES` has been reported.
    too_many_edges: bool,
    /// Bytes of edge label text so far, counting each edge of an `&` product.
    label_bytes: usize,
    /// Whether `MAX_LABEL_BYTES` has been reported.
    too_much_label: bool,
    /// Whether `MAX_META_KEYS` has been reported.
    too_many_keys: bool,
    /// Bytes of style and class text copied by `classDef`, `class` and `linkStyle`.
    style_bytes: usize,
    /// Whether `MAX_STYLE_BYTES` has been reported.
    too_much_style: bool,
}

/// The most style and class text, in bytes, that `classDef`, `class` and `linkStyle` copy onto
/// their targets. A statement listing many targets copies its text once per target.
pub const MAX_STYLE_BYTES: usize = 64 << 20;

/// Bytes counted for each copy besides its text, so copies of short text still add up.
const COPY_OVERHEAD: usize = 32;

/// The bytes counted for copying `styles` onto one target.
fn style_bytes(styles: &[Style]) -> usize {
    styles
        .iter()
        .map(|s| COPY_OVERHEAD + s.property.len() + s.value.len())
        .sum::<usize>()
        + COPY_OVERHEAD
}

/// The most edges a flowchart has.
pub const MAX_EDGES: usize = 100_000;

/// The most edge label text, in bytes, across all edges. One label on every edge of an `&`
/// product counts once per edge, so a short source cannot make gigabytes of labels.
pub const MAX_LABEL_BYTES: usize = 16 << 20;

/// The most subgraphs nested in one another, by blocks or by listing subgraphs as members: as
/// many as the parser reads nested blocks.
pub const MAX_NESTING: usize = MAX_DEPTH + 1;

/// The id of the untitled lane that holds a swimlane diagram's nodes outside every lane.
pub const DEFAULT_LANE: &str = "__swimlane_default__";

impl Lowerer {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        // FLOWMO-10: a node with a subgraph's id is that subgraph.
        let mut merged = HashSet::new();
        for sg in &mut self.subgraphs {
            if let Some(&i) = self.node_index.get(&sg.id) {
                let node = &self.chart.nodes[i];
                sg.classes.extend(node.classes.iter().cloned());
                sg.styles.extend(node.styles.iter().cloned());
                merged.insert(sg.id.clone());
            }
        }
        self.chart.nodes.retain(|n| !merged.contains(&n.id));
        // An edge between a subgraph and one of its own descendants has no route: it is left
        // out of the drawing (LAYOUT-2).
        if !self.chart.lanes {
            let inside = |inner: &str, outer: &str| {
                let mut seen = HashSet::new();
                let mut at = inner;
                while let Some(owner) = self.owners.get(at) {
                    if owner == outer {
                        return true;
                    }
                    if !seen.insert(owner.as_str()) {
                        break;
                    }
                    at = owner;
                }
                false
            };
            let nested: Vec<(TextRange, String)> = self
                .chart
                .edges
                .iter()
                .filter(|e| inside(&e.to, &e.from) || inside(&e.from, &e.to))
                .map(|e| {
                    (
                        e.range,
                        format!(
                            "an edge between a subgraph and a node inside it is not drawn; ignoring `{} --> {}`",
                            e.from, e.to
                        ),
                    )
                })
                .collect();
            for (range, message) in nested {
                self.warning(range, message);
            }
        }
        for sg in &mut self.subgraphs {
            sg.members.retain(|m| !self.globals.contains(m));
        }
        self.subgraphs.sort_by_key(|s| s.range.start());
        self.chart.subgraphs = std::mem::take(&mut self.subgraphs);
        if self.chart.lanes {
            self.lanes();
        }
        (self.chart, self.diagnostics)
    }

    /// Swimlane lanes (MSWIM-2): drops a lane emptied by a node another lane listed first,
    /// warns about subgraphs nested in a lane, and gathers nodes outside every lane into an
    /// untitled last lane.
    fn lanes(&mut self) {
        let chart = &self.chart;
        let members: HashSet<&str> = chart
            .subgraphs
            .iter()
            .flat_map(|s| s.members.iter().map(String::as_str))
            .collect();
        let nested = |id: &str| members.contains(id);
        let ends: HashSet<&str> = chart
            .edges
            .iter()
            .flat_map(|e| [e.from.as_str(), e.to.as_str()])
            .collect();
        let lost: HashSet<&str> = self.lost.iter().map(String::as_str).collect();
        let emptied: HashSet<String> = chart
            .subgraphs
            .iter()
            .filter(|s| {
                s.members.is_empty()
                    && lost.contains(s.id.as_str())
                    && !nested(&s.id)
                    && !ends.contains(s.id.as_str())
            })
            .map(|s| s.id.clone())
            .collect();
        let inner: Vec<(TextRange, String)> = chart
            .subgraphs
            .iter()
            .filter(|s| nested(&s.id))
            .map(|s| {
                (
                    TextRange::at(s.range.start(), "subgraph".len().try_into().unwrap()),
                    s.id.clone(),
                )
            })
            .collect();
        self.chart.subgraphs.retain(|s| !emptied.contains(&s.id));
        for (range, id) in inner {
            self.warning(
                range,
                format!(
                    "subgraph `{id}` is nested in a lane; only top-level subgraphs are lanes, so \
                     it is drawn as a plain box"
                ),
            );
        }
        let chart = &self.chart;
        let members: HashSet<&str> = chart
            .subgraphs
            .iter()
            .flat_map(|s| s.members.iter().map(String::as_str))
            .collect();
        let loose: Vec<String> = chart
            .nodes
            .iter()
            .filter(|n| !members.contains(n.id.as_str()))
            .map(|n| n.id.clone())
            .collect();
        if !loose.is_empty() {
            self.chart.subgraphs.push(Subgraph {
                id: DEFAULT_LANE.to_owned(),
                title: Label::default(),
                direction: None,
                members: loose,
                classes: Vec::new(),
                styles: Vec::new(),
                metadata: Vec::new(),
                range: TextRange::default(),
                region: false,
            });
        }
    }

    /// Lowers the statements directly under `parent`. Returns the ids of the nodes and
    /// subgraphs they mention, and the last `direction`.
    fn statements(&mut self, parent: &SyntaxNode, nested: bool) -> (Ids, Option<Direction>) {
        let mut members = Ids::default();
        let mut direction = None;
        for stmt in parent.children() {
            match stmt.kind() {
                VERTEX_STMT => {
                    for id in self.vertex_stmt(&stmt) {
                        members.push(id);
                    }
                }
                SUBGRAPH => {
                    let id = self.subgraph(&stmt);
                    members.push(id);
                }
                GLOBAL => {
                    let (inner, _) = self.statements(&stmt, false);
                    for id in inner.list {
                        self.globals.push(id);
                    }
                }
                CONNECTOR_STMT => {
                    if let Some(id) = self.connector(&stmt) {
                        members.push(id);
                    }
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
        let mut members = Ids::default();
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
                        members.push(id.clone());
                    }
                    prev = Some(ids);
                }
                EDGE => edge = Some(part),
                _ => {}
            }
        }
        members.list
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
                self.edge_metadata(e, entries, syntax.text_range());
            }
            return None;
        }
        if let Some(entries) = &meta
            && let Some(&s) = self.subgraph_index.get(&id)
        {
            let sg = &mut self.subgraphs[s];
            if merge_metadata(&mut sg.metadata, entries.clone()) {
                self.too_many_keys(syntax.text_range());
            }
            return None;
        }
        let i = self.node_entry(&id);
        self.chart.nodes[i].mentions.push(syntax.text_range());
        if let Some(shape) = child(syntax, SHAPE) {
            let open = token(&shape, SHAPE_OPEN).map(|t| t.text().to_owned());
            let close = token(&shape, SHAPE_CLOSE).map(|t| t.text().to_owned());
            let mut kind = Shape::from_delimiters(
                open.as_deref().unwrap_or("["),
                close.as_deref().unwrap_or(""),
            );
            if self.chart.agentflow {
                kind = self.agentflow_delimited(kind, shape.text_range());
            }
            let node = &mut self.chart.nodes[i];
            node.shape = kind;
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
            shape: if self.chart.agentflow {
                Shape::Rounded
            } else {
                Shape::Rect
            },
            sections: Vec::new(),
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
        let (mut entries, invalid) = text::metadata(body.as_ref().map_or("", |t| t.text()));
        for bad in invalid {
            self.error(
                syntax.text_range(),
                format!("expected `key: value` in metadata, found `{bad}`"),
            );
        }
        if entries.len() > MAX_META_KEYS {
            entries.truncate(MAX_META_KEYS);
            self.too_many_keys(syntax.text_range());
        }
        entries
    }

    /// Warns, once per diagram, that metadata keys past `MAX_META_KEYS` were left out.
    fn too_many_keys(&mut self, range: TextRange) {
        if !self.too_many_keys {
            self.too_many_keys = true;
            self.warning(
                range,
                format!(
                    "an element has more than {MAX_META_KEYS} metadata keys; leaving out the rest"
                ),
            );
        }
    }

    fn node_metadata(&mut self, i: usize, entries: Metadata, range: TextRange) {
        let get = |key: &str| {
            entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let agentflow = self.chart.agentflow;
        let shape = get("shape").map(|name| {
            let s = if agentflow {
                agentflow::named(name)
            } else {
                Shape::from_name(name).map(agentflow::Named::Shape)
            };
            (name.to_owned(), s)
        });
        let markdown = !matches!(get("labelType"), Some("text" | "string"));
        let label = get("label")
            .filter(|l| !l.is_empty())
            .map(|l| text::label(l, markdown));
        let node = &mut self.chart.nodes[i];
        if let Some(label) = label {
            node.label = label;
        }
        let dropped = merge_metadata(&mut node.metadata, entries);
        match shape {
            Some((_, Some(agentflow::Named::Shape(s)))) => node.shape = s,
            Some((name, Some(agentflow::Named::Removed))) => {
                node.shape = Shape::Rounded;
                self.error(range, agentflow::removed(&name));
            }
            Some((name, Some(agentflow::Named::Unsupported))) => {
                node.shape = Shape::Rounded;
                self.warning(range, agentflow::unsupported(&name));
            }
            Some((name, None)) => self.error(range, format!("unknown shape `{name}`")),
            None => {}
        }
        if dropped {
            self.too_many_keys(range);
        }
    }

    fn edge_metadata(&mut self, e: usize, entries: Metadata, range: TextRange) {
        let edge = &mut self.chart.edges[e];
        for (k, v) in &entries {
            match k.as_str() {
                "animate" => edge.animate = v == "true",
                "animation" => edge.animation = Some(v.clone()),
                "curve" => edge.curve = Some(v.clone()),
                _ => {}
            }
        }
        if merge_metadata(&mut edge.metadata, entries) {
            self.too_many_keys(range);
        }
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
        // `a & b --> c & d` makes every pair, so a few long `&` lists could make billions.
        let pairs = from.len().saturating_mul(to.len());
        if self.chart.edges.len().saturating_add(pairs) > MAX_EDGES {
            if !self.too_many_edges {
                self.too_many_edges = true;
                self.diagnostics.push(Diagnostic::new(
                    syntax.text_range(),
                    format!("a flowchart has at most {MAX_EDGES} edges; leaving out the rest"),
                ));
            }
            return;
        }
        let mut label = label;
        if let Some(l) = &label {
            let bytes = pairs.saturating_mul(l.text.len());
            if self.label_bytes.saturating_add(bytes) > MAX_LABEL_BYTES {
                if !self.too_much_label {
                    self.too_much_label = true;
                    self.warning(
                        syntax.text_range(),
                        format!(
                            "edge labels total more than {} MiB; leaving out the rest",
                            MAX_LABEL_BYTES >> 20
                        ),
                    );
                }
                label = None;
            } else {
                self.label_bytes += bytes;
            }
        }
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
                    constraint: true,
                    ports: [None; 2],
                    rows: [None; 2],
                    label: label.clone(),
                    start_label: None,
                    end_label: None,
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
        let (explicit, mut title) = subgraph_header(&header);
        // MAGF-2: a `flow` is titled only by `flow id["Title"]`.
        if self.chart.agentflow && child(&header, SHAPE).is_none() {
            title = Label::default();
        }
        let (members, direction) = self.statements(syntax, true);
        let meta = child(&header, METADATA).map(|m| self.metadata(&m));
        let id = explicit.unwrap_or_else(|| format!("subGraph{}", self.subgraph_count));
        self.subgraph_count += 1;

        let mut kept = Vec::new();
        // Subgraph levels above `id`, and the most at and below it once `kept` is added.
        let above = self.levels_above(&id);
        let mut height = 1;
        for m in &members.list {
            if *m == id || self.globals.contains(m) {
                continue;
            }
            if self.nested_in(&id, |c| c == m) {
                self.warning(
                    header.text_range(),
                    format!("subgraph `{m}` cannot be nested in its own descendant `{id}`"),
                );
                continue;
            }
            match self.owners.get(m) {
                Some(owner) => {
                    let listed = |c: &str| members.contains(c) && !self.globals.contains(c);
                    if *owner != id && !self.nested_in(owner, listed) {
                        let owner = owner.clone();
                        self.warning(
                            header.text_range(),
                            format!("`{m}` already belongs to subgraph `{owner}`"),
                        );
                        self.lost.push(id.clone());
                    }
                }
                None => {
                    let below = self.heights.get(m).copied().unwrap_or(0);
                    if above + 1 + below > MAX_NESTING {
                        self.warning(
                            header.text_range(),
                            format!("`{m}` in `{id}` nests subgraphs more than {MAX_NESTING} deep"),
                        );
                        continue;
                    }
                    height = height.max(1 + below);
                    kept.push(m.clone());
                }
            }
        }
        for m in &kept {
            self.owners.insert(m.clone(), id.clone());
        }
        // Raise the heights of `id` and the subgraphs it sits in.
        let mut at = id.as_str();
        loop {
            let h = self.heights.entry(at.to_owned()).or_insert(0);
            if *h >= height {
                break;
            }
            *h = height;
            match self.owners.get(at) {
                Some(owner) => at = owner,
                None => break,
            }
            height += 1;
        }
        match self.subgraph_index.get(&id) {
            Some(&s) => {
                let existing = &mut self.subgraphs[s];
                // `kept` has no members owned before, so none of `existing.members`.
                existing.members.extend(kept);
                if let Some(meta) = meta
                    && merge_metadata(&mut existing.metadata, meta)
                {
                    self.too_many_keys(header.text_range());
                }
            }
            None => {
                self.subgraph_index.insert(id.clone(), self.subgraphs.len());
                self.subgraphs.push(Subgraph {
                    id: id.clone(),
                    title,
                    direction,
                    members: kept,
                    classes: Vec::new(),
                    styles: Vec::new(),
                    metadata: meta.unwrap_or_default(),
                    range: syntax.text_range(),
                    region: false,
                });
            }
        }
        id
    }

    /// How many subgraphs `id` sits in. Nesting stays within `MAX_NESTING`, so this is a short
    /// walk.
    fn levels_above(&self, id: &str) -> usize {
        let mut n = 0;
        let mut at = id;
        while let Some(owner) = self.owners.get(at)
            && n <= MAX_NESTING
        {
            n += 1;
            at = owner;
        }
        n
    }

    /// Whether subgraph `id`, or a resolved subgraph it sits in at any depth, is `listed`.
    fn nested_in(&self, id: &str, listed: impl Fn(&str) -> bool) -> bool {
        let mut seen = HashSet::new();
        let mut current = Some(id);
        while let Some(c) = current {
            if listed(c) {
                return true;
            }
            if !seen.insert(c) {
                break;
            }
            current = self.owners.get(c).map(String::as_str);
        }
        false
    }

    // ---- agentflow ----

    /// MAGF-4: the shape an agentflow node drawn with `delimited`'s brackets gets, reporting the
    /// shapes agentflow removed.
    fn agentflow_delimited(&mut self, delimited: Shape, range: TextRange) -> Shape {
        match delimited {
            Shape::Rect | Shape::Rounded => Shape::Rounded,
            Shape::Diamond | Shape::Hexagon | Shape::FramedRect | Shape::LeanRight => delimited,
            other => {
                let name = agentflow::delimited_name(other);
                self.error(range, agentflow::removed(name));
                Shape::Rounded
            }
        }
    }

    /// MAGF-2: a `connector id["Title"]` declaration. Returns the connector's id.
    fn connector(&mut self, stmt: &SyntaxNode) -> Option<String> {
        let syntax = child(stmt, NODE)?;
        let shape = child(&syntax, SHAPE);
        if let Some(open) = shape.as_ref().and_then(|s| token(s, SHAPE_OPEN))
            && open.text() != "["
        {
            self.error(open.text_range(), "a connector takes only a `[title]`");
        }
        let id = self.node(&syntax)?;
        let i = self.node_index[&id];
        let node = &mut self.chart.nodes[i];
        if node.label.text.trim().is_empty() {
            node.label = Label::plain(id.clone());
        }
        if !node.metadata.iter().any(|(k, _)| k == "shape") {
            node.shape = Shape::Rounded;
        }
        Some(id)
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

    /// Counts `bytes` of style or class text copied onto one more target. Returns false, warning
    /// once, past `MAX_STYLE_BYTES`.
    fn copy_styles(&mut self, bytes: usize, range: TextRange) -> bool {
        match self.style_bytes.checked_add(bytes) {
            Some(total) if total <= MAX_STYLE_BYTES => {
                self.style_bytes = total;
                true
            }
            _ => {
                if !self.too_much_style {
                    self.too_much_style = true;
                    self.warning(
                        range,
                        "styles and classes applied total more than 64 MiB; leaving out the rest",
                    );
                }
                false
            }
        }
    }

    /// FLOWMO-11.
    fn class_def_stmt(&mut self, stmt: &SyntaxNode) {
        let (Some(names), Some(style)) = (child(stmt, ID_LIST), token(stmt, STYLE_TEXT)) else {
            return;
        };
        let styles = self.styles(&style);
        let bytes = style_bytes(&styles);
        for name in tokens(&names).filter(|t| t.kind() == IDENT) {
            if !self.copy_styles(bytes, name.text_range()) {
                break;
            }
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

    /// FLOWMO-11.
    fn class_stmt(&mut self, stmt: &SyntaxNode) {
        let (Some(ids), Some(class)) = (child(stmt, ID_LIST), token(stmt, IDENT)) else {
            return;
        };
        let class = class.text().to_owned();
        for t in tokens(&ids).filter(|t| t.kind() == IDENT) {
            if !self.copy_styles(COPY_OVERHEAD + class.len(), t.text_range()) {
                break;
            }
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
            if let Some(&s) = self.subgraph_index.get(id) {
                self.subgraphs[s].classes.push(class.clone());
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
        } else if let Some(&s) = self.subgraph_index.get(name) {
            self.subgraphs[s].styles.extend(styles);
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
        let bytes = COPY_OVERHEAD
            + styles.as_deref().map_or(0, style_bytes)
            + curve.as_ref().map_or(0, String::len);
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
            if !self.copy_styles(bytes, t.text_range()) {
                break;
            }
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
