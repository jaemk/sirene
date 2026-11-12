//! Lowers a use case diagram syntax tree to the graph model (MUSE-2 to MUSE-7).

use std::collections::HashMap;

use rowan::TextRange;

use super::json;
use super::parser::{Op, op};
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
    l.chart.direction = Direction::LeftRight;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(t) = el.as_token()
        {
            l.chart.title = text::frontmatter_title(t.text());
        }
    }
    for stmt in root.children() {
        l.statement(&stmt, None);
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

/// The id Mermaid derives from a quoted name: each character outside `[A-Za-z0-9_]` becomes
/// `_` (two for a character outside the Basic Multilingual Plane, as JavaScript counts them).
pub(crate) fn generated_id(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_string()
            } else {
                "_".repeat(c.len_utf16())
            }
        })
        .collect()
}

/// A label token's text as a label: quotes removed, Markdown kept as such.
fn label_of(t: &SyntaxToken) -> Label {
    match t.kind() {
        MD_STRING => text::label(text::unquote(t.text()), true),
        STRING => {
            let s = t.text();
            text::label(&s[1..s.len().saturating_sub(1).max(1)], false)
        }
        _ => text::label(t.text(), false),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Actor,
    UseCase,
    Boundary,
    Json,
    Edge,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Actor => "actor",
            Kind::UseCase => "use case",
            Kind::Boundary => "system boundary",
            Kind::Json => "JSON table",
            Kind::Edge => "edge",
        }
    }
}

/// A name in the source: its id, its label, whether it was given in quotes, and where.
struct Name {
    id: String,
    label: Label,
    range: TextRange,
}

/// An actor or use case occurrence.
struct Entity {
    name: Name,
    /// `Some(true)` for `[label]`, `Some(false)` for `(label)`.
    rect: Option<bool>,
    metadata: Option<(String, TextRange)>,
    stereotype: Option<String>,
    classes: Vec<(String, TextRange)>,
}

impl Entity {
    fn explicit(&self) -> bool {
        self.rect.is_some() || self.metadata.is_some() || self.stereotype.is_some()
    }
}

#[derive(Default)]
struct Element {
    label: Label,
    rect: bool,
    stereotype: Option<String>,
    actor_type: Option<String>,
    icon: Option<String>,
    business: bool,
    parent: Option<String>,
}

#[derive(Default)]
struct Lowerer {
    chart: Flowchart,
    diagnostics: Vec<Diagnostic>,
    kinds: HashMap<String, Kind>,
    /// Actor and use case details by node index.
    elements: Vec<Element>,
    node_index: HashMap<String, usize>,
    subgraph_index: HashMap<String, usize>,
    /// `id@{ ... }` statements and metadata on boundaries, applied once everything exists.
    assignments: Vec<(String, String, TextRange)>,
    /// `:::`, `class`, and `style`, applied once everything exists.
    styling: Vec<(String, Styling, TextRange)>,
    /// Notes, checked once everything exists.
    notes: Vec<Note>,
    /// Relationship endpoints checked once everything exists: edge index, source and target.
    relations: Vec<(usize, Option<&'static str>, TextRange)>,
    /// Relationship endpoints, which become use cases unless declared.
    pending: Vec<(String, Label, TextRange)>,
}

enum Styling {
    Class(String),
    Styles(Vec<Style>),
}

impl Lowerer {
    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn statement(&mut self, stmt: &SyntaxNode, parent: Option<&str>) {
        match stmt.kind() {
            DIRECTION_STMT => {
                if let Some(d) = text_of(stmt, DIR).and_then(|d| Direction::from_text(&d)) {
                    self.chart.direction = d;
                }
            }
            VERTEX_STMT => self.vertex(stmt, parent),
            SUBGRAPH => self.boundary(stmt),
            NOTE_STMT => self.note(stmt),
            JSON_STMT => self.json(stmt),
            META_STMT => {
                let (Some(name), Some(meta)) = (
                    tokens(stmt).find(|t| matches!(t.kind(), IDENT | STRING | MD_STRING)),
                    child(stmt, METADATA),
                ) else {
                    return;
                };
                let id = self.name_of(&name).id;
                if let Some(body) = text_of(&meta, META_BODY) {
                    self.assignments.push((id, body, meta.text_range()));
                }
            }
            CLASSDEF_STMT => self.class_def(stmt),
            CLASS_STMT => self.class(stmt),
            STYLE_STMT => self.style(stmt),
            ACC_TITLE_STMT => self.chart.acc_title = text_of(stmt, ACC_TEXT),
            ACC_DESCR_STMT => self.chart.acc_descr = text_of(stmt, ACC_TEXT),
            _ => {}
        }
    }

    fn name_of(&self, t: &SyntaxToken) -> Name {
        let label = label_of(t);
        let id = match t.kind() {
            IDENT => t.text().to_owned(),
            _ => generated_id(&label.text),
        };
        Name {
            id,
            label,
            range: t.text_range(),
        }
    }

    fn entity(&self, node: &SyntaxNode) -> Option<Entity> {
        let name = tokens(node).find(|t| matches!(t.kind(), IDENT | STRING | MD_STRING))?;
        let mut name = self.name_of(&name);
        let mut rect = None;
        if let Some(shape) = child(node, SHAPE) {
            rect = Some(token(&shape, SHAPE_OPEN).is_some_and(|t| t.text() == "["));
            if let Some(l) = tokens(&shape).find(|t| matches!(t.kind(), TEXT | STRING | MD_STRING))
            {
                name.label = label_of(&l);
            }
        }
        let metadata =
            child(node, METADATA).and_then(|m| text_of(&m, META_BODY).map(|b| (b, m.text_range())));
        let stereotype = token(node, STEREOTYPE).map(|t| {
            let s = t.text();
            s[2..s.len() - 2].trim().to_owned()
        });
        let classes = child(node, CLASS_SHORTHAND)
            .map(|s| {
                s.descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| t.kind() == IDENT)
                    .map(|t| (t.text().to_owned(), t.text_range()))
                    .collect()
            })
            .unwrap_or_default();
        Some(Entity {
            name,
            rect,
            metadata,
            stereotype,
            classes,
        })
    }

    /// Registers `id` as `kind`, warning when it is already something else.
    fn claim(&mut self, id: &str, kind: Kind, range: TextRange) -> bool {
        match self.kinds.get(id) {
            Some(&k) if k != kind => {
                self.warning(
                    range,
                    format!(
                        "`{id}` is already a {}; it cannot also be a {}",
                        k.name(),
                        kind.name()
                    ),
                );
                false
            }
            _ => {
                self.kinds.insert(id.to_owned(), kind);
                true
            }
        }
    }

    /// The node for an actor or use case, created if new.
    fn node(&mut self, id: &str, label: &Label, range: TextRange) -> usize {
        let i = match self.node_index.get(id) {
            Some(&i) => i,
            None => {
                let i = self.chart.nodes.len();
                self.chart.nodes.push(Node {
                    id: id.to_owned(),
                    label: label.clone(),
                    shape: Shape::UseCase,
                    sections: Vec::new(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                    click: None,
                    tooltip: None,
                    metadata: Vec::new(),
                    mentions: Vec::new(),
                });
                self.elements.push(Element {
                    label: label.clone(),
                    ..Element::default()
                });
                self.node_index.insert(id.to_owned(), i);
                i
            }
        };
        self.chart.nodes[i].mentions.push(range);
        i
    }

    /// MUSE-2: declares an actor or use case.
    fn declare(&mut self, e: &Entity, actor: bool, parent: Option<&str>) {
        let id = e.name.id.clone();
        let kind = if actor { Kind::Actor } else { Kind::UseCase };
        if !self.claim(&id, kind, e.name.range) {
            return;
        }
        let i = self.node(&id, &e.name.label, e.name.range);
        let el = &mut self.elements[i];
        el.label = e.name.label.clone();
        if let Some(rect) = e.rect {
            el.rect = rect;
        }
        if e.stereotype.is_some() {
            el.stereotype.clone_from(&e.stereotype);
        }
        if let Some(p) = parent {
            match &el.parent {
                Some(old) if old != p => {
                    let message = format!("`{id}` is already in system boundary `{old}`");
                    self.warning(e.name.range, message);
                }
                _ => {
                    el.parent = Some(p.to_owned());
                    if let Some(&s) = self.subgraph_index.get(p)
                        && !self.chart.subgraphs[s].members.contains(&id)
                    {
                        self.chart.subgraphs[s].members.push(id.clone());
                    }
                }
            }
        }
        if let Some((body, range)) = &e.metadata {
            self.element_metadata(i, body, *range);
        }
        for (c, range) in &e.classes {
            self.styling
                .push((id.clone(), Styling::Class(c.clone()), *range));
        }
    }

    /// MUSE-3: an actor's `type`, `icon`, and `business`, or a use case's `business`.
    fn element_metadata(&mut self, i: usize, body: &str, range: TextRange) {
        let (entries, invalid) = text::metadata(body);
        for bad in invalid {
            self.warning(
                range,
                format!("ignoring metadata entry `{bad}` without `:`"),
            );
        }
        let id = self.chart.nodes[i].id.clone();
        let kind = self.kinds[&id];
        for (key, value) in entries {
            let el = &mut self.elements[i];
            match (kind, key.as_str(), value.as_str()) {
                (Kind::Actor, "type", "normal" | "hollow" | "awesome") => {
                    el.actor_type = Some(value);
                }
                (Kind::Actor, "icon", _) => el.icon = Some(value),
                (_, "business", "true" | "false") => el.business = value == "true",
                _ => self.warning(
                    range,
                    format!(
                        "ignoring metadata `{key}: {value}`, which is not valid for {} `{id}`",
                        kind.name()
                    ),
                ),
            }
        }
    }

    /// An actor or entity statement, with an optional relationship.
    fn vertex(&mut self, stmt: &SyntaxNode, parent: Option<&str>) {
        let actor = token(stmt, USECASE_KW).is_some();
        let nodes: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == NODE).collect();
        let edge = child(stmt, EDGE);
        let Some(first) = nodes.first().and_then(|n| self.entity(n)) else {
            return;
        };
        let Some(edge) = edge else {
            // A declaration: an actor list, or a use case on its own.
            for n in &nodes {
                if let Some(e) = self.entity(n) {
                    self.declare(&e, actor, parent);
                }
            }
            return;
        };
        if actor || first.explicit() {
            self.declare(&first, actor, parent);
        }
        let target = nodes.get(1).and_then(|n| self.entity(n));
        if let Some(t) = &target
            && t.explicit()
        {
            self.declare(t, false, parent);
        }
        let Some(target) = target else { return };
        self.relation(&edge, &first, actor, &target);
    }

    /// MUSE-4: a relationship between two endpoints.
    fn relation(&mut self, edge: &SyntaxNode, source: &Entity, actor: bool, target: &Entity) {
        for (e, declared) in [
            (source, actor || source.explicit()),
            (target, target.explicit()),
        ] {
            if !declared && !e.classes.is_empty() {
                self.warning(
                    e.classes[0].1,
                    format!(
                        "ignoring `:::` on `{}`: declare it (with a shape, metadata, or a \
                         stereotype) to give it classes",
                        e.name.id
                    ),
                );
            }
        }
        let strokes: Vec<SyntaxToken> = tokens(edge).filter(|t| t.kind() == STROKE).collect();
        let Some(first) = strokes.first() else { return };
        let Some((op1, _)) = op(first.text()) else {
            return;
        };
        let closer = strokes
            .get(1)
            .and_then(|t| op(t.text()).map(|o| (o.0, t.text())));
        let dashes = |s: &str| s.bytes().filter(|b| *b == b'-').count();
        let minlen = |s: &str| u8::try_from(dashes(s).saturating_sub(1).clamp(1, 10)).unwrap_or(1);
        let label = child(edge, EDGE_LABEL)
            .and_then(|l| tokens(&l).find(|t| matches!(t.kind(), TEXT | STRING | MD_STRING)))
            .map(|t| label_of(&t));
        let (mut start, mut end, mut stroke, mut length) = (None, None, Stroke::Normal, 1);
        let mut kind = "association";
        let mut label = label;
        match (op1, closer) {
            (Op::Dependency, _) => {
                let rel = text_of(edge, RELATION)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                stroke = Stroke::Dotted;
                end = Some(Arrow::Point);
                label = Some(Label::plain(format!("«{rel}»")));
                kind = if rel == "include" {
                    "include"
                } else {
                    "extend"
                };
            }
            (Op::Generalization, _) => {
                end = Some(Arrow::Triangle);
                kind = "generalization";
            }
            (Op::Line, Some((close, text))) => {
                match close {
                    Op::Forward => end = Some(Arrow::Point),
                    Op::ForwardCircle => end = Some(Arrow::Circle),
                    Op::ForwardCross => end = Some(Arrow::Cross),
                    _ => {}
                }
                if matches!(close, Op::Forward | Op::Line) {
                    length = minlen(text);
                }
            }
            (Op::Backward, Some((_, text))) => {
                start = Some(Arrow::Point);
                length = minlen(text);
            }
            (Op::Forward, _) => {
                end = Some(Arrow::Point);
                length = minlen(first.text());
            }
            (Op::Backward, None) => {
                start = Some(Arrow::Point);
                length = minlen(first.text());
            }
            (Op::Line, None) => length = minlen(first.text()),
            (Op::ForwardCircle, _) => end = Some(Arrow::Circle),
            (Op::BackwardCircle, _) => start = Some(Arrow::Circle),
            (Op::ForwardCross, _) => end = Some(Arrow::Cross),
            (Op::BackwardCross, _) => start = Some(Arrow::Cross),
        }
        let markers_ok = matches!(
            (start, end),
            (None, None) | (Some(Arrow::Point), None) | (None, Some(Arrow::Point))
        ) && kind == "association";
        let id = child(edge, EDGE_ID).and_then(|e| text_of(&e, IDENT));
        if let Some(id) = &id {
            self.claim(id, Kind::Edge, edge.text_range());
        }
        let index = self.chart.edges.len();
        self.chart.edges.push(Edge {
            id,
            from: source.name.id.clone(),
            to: target.name.id.clone(),
            stroke,
            start,
            end,
            length,
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
            range: edge.text_range(),
        });
        // Endpoints not declared anywhere become use cases in `finish`, in source order.
        for e in [source, target] {
            self.pending
                .push((e.name.id.clone(), e.name.label.clone(), e.name.range));
        }
        let rule = match kind {
            "include" | "extend" => Some(if kind == "include" {
                "include"
            } else {
                "extend"
            }),
            "generalization" => Some("generalization"),
            _ if !markers_ok => Some("marked"),
            _ => None,
        };
        self.relations.push((index, rule, edge.text_range()));
    }

    /// MUSE-6: `systemBoundary id ... end`.
    fn boundary(&mut self, stmt: &SyntaxNode) {
        let Some(header) = child(stmt, SUBGRAPH_HEADER) else {
            return;
        };
        let Some(e) = child(&header, NODE).and_then(|n| self.entity(&n)) else {
            return;
        };
        let id = e.name.id.clone();
        if self.claim(&id, Kind::Boundary, e.name.range) && !self.subgraph_index.contains_key(&id) {
            self.subgraph_index
                .insert(id.clone(), self.chart.subgraphs.len());
            self.chart.subgraphs.push(Subgraph {
                id: id.clone(),
                title: e.name.label.clone(),
                direction: None,
                members: Vec::new(),
                classes: Vec::new(),
                styles: Vec::new(),
                metadata: Vec::new(),
                range: stmt.text_range(),
                region: false,
            });
        }
        if let Some((body, range)) = e.metadata {
            self.assignments.push((id.clone(), body, range));
        }
        for (c, range) in e.classes {
            self.styling.push((id.clone(), Styling::Class(c), range));
        }
        for s in stmt.children().filter(|n| n.kind() == VERTEX_STMT) {
            self.statement(&s, Some(&id));
        }
    }

    /// MUSE-5: `note for id text`.
    fn note(&mut self, stmt: &SyntaxNode) {
        let (Some(target), Some(label)) = (
            token(stmt, IDENT),
            tokens(stmt)
                .filter(|t| matches!(t.kind(), TEXT | STRING | MD_STRING))
                .last(),
        ) else {
            return;
        };
        self.notes.push(Note {
            target: target.text().to_owned(),
            side: NoteSide::After,
            label: label_of(&label),
            range: target.text_range(),
        });
    }

    /// MUSE-5: `json id@{ ... }`, a table of the object's flattened keys and values.
    fn json(&mut self, stmt: &SyntaxNode) {
        let (Some(name), Some(body)) = (token(stmt, IDENT), token(stmt, JSON_BODY)) else {
            return;
        };
        let id = name.text().to_owned();
        let rows = match json::rows(body.text()) {
            Ok(rows) => rows,
            Err(e) => {
                self.warning(body.text_range(), e);
                Vec::new()
            }
        };
        if !self.claim(&id, Kind::Json, name.text_range()) {
            return;
        }
        if self.node_index.contains_key(&id) {
            self.warning(
                name.text_range(),
                format!("JSON table `{id}` is declared twice"),
            );
            return;
        }
        let i = self.node(&id, &Label::plain(&id), name.text_range());
        let n = &mut self.chart.nodes[i];
        n.shape = Shape::Rect;
        let rows: Vec<Row> = rows
            .into_iter()
            .map(|(k, v)| Row {
                cells: vec![k, v],
                ..Row::default()
            })
            .collect();
        if !rows.is_empty() {
            n.sections = vec![rows];
        }
        if let Some(s) = child(stmt, CLASS_SHORTHAND) {
            for t in s
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| t.kind() == IDENT)
            {
                self.styling.push((
                    id.clone(),
                    Styling::Class(t.text().to_owned()),
                    t.text_range(),
                ));
            }
        }
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
                Some(class) => class.styles = styles.clone(),
                None => self.chart.classes.push(ClassDef {
                    name: n.to_owned(),
                    styles: styles.clone(),
                }),
            }
        }
    }

    /// `class a,b c,d`.
    fn class(&mut self, stmt: &SyntaxNode) {
        let lists: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == ID_LIST).collect();
        let [ids, classes] = lists.as_slice() else {
            return;
        };
        let classes: Vec<String> = tokens(classes)
            .filter(|t| t.kind() == IDENT)
            .map(|t| t.text().to_owned())
            .collect();
        for t in tokens(ids).filter(|t| t.kind() == IDENT) {
            for c in &classes {
                self.styling.push((
                    t.text().to_owned(),
                    Styling::Class(c.clone()),
                    t.text_range(),
                ));
            }
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

    /// MUSE-3: `id@{ ... }` for an actor, use case, system boundary, or edge.
    fn assign(&mut self, id: &str, body: &str, range: TextRange) {
        match self.kinds.get(id).copied() {
            None => self.warning(range, format!("ignoring metadata for unknown `{id}`")),
            Some(Kind::Actor | Kind::UseCase) => {
                let i = self.node_index[id];
                self.element_metadata(i, body, range);
            }
            Some(Kind::Boundary) => {
                let (entries, _) = text::metadata(body);
                let s = self.subgraph_index[id];
                for (k, v) in entries {
                    if k == "type" && (v == "rect" || v == "package") {
                        self.chart.subgraphs[s].metadata = vec![(k, v)];
                    } else {
                        self.warning(
                            range,
                            format!(
                                "ignoring metadata `{k}: {v}`: a system boundary takes `type: \
                                 rect` or `type: package`"
                            ),
                        );
                    }
                }
            }
            Some(Kind::Edge) => {
                let (entries, _) = text::metadata(body);
                let Some(e) = self
                    .chart
                    .edges
                    .iter()
                    .position(|e| e.id.as_deref() == Some(id))
                else {
                    return;
                };
                for (k, v) in entries {
                    let edge = &mut self.chart.edges[e];
                    match (k.as_str(), v.as_str()) {
                        ("animate", "true" | "false") => edge.animate = v == "true",
                        ("animation", "fast" | "slow") => {
                            edge.animation = Some(v);
                            edge.animate = true;
                        }
                        _ => self.warning(
                            range,
                            format!(
                                "ignoring metadata `{k}: {v}`: an edge takes `animate` and \
                                 `animation: fast|slow`"
                            ),
                        ),
                    }
                }
            }
            Some(Kind::Json) => self.warning(
                range,
                format!("ignoring metadata for JSON table `{id}`, which takes none"),
            ),
        }
    }

    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        // Endpoints that were never declared are use cases, unless declared later.
        for (id, label, range) in std::mem::take(&mut self.pending) {
            if !self.kinds.contains_key(&id) {
                self.kinds.insert(id.clone(), Kind::UseCase);
                self.node(&id, &label, range);
            } else if let Some(&i) = self.node_index.get(&id) {
                self.chart.nodes[i].mentions.push(range);
            }
        }
        for (id, body, range) in std::mem::take(&mut self.assignments) {
            self.assign(&id, &body, range);
        }
        for (index, rule, range) in std::mem::take(&mut self.relations) {
            let e = &self.chart.edges[index];
            let (from, to) = (
                self.kinds.get(&e.from).copied(),
                self.kinds.get(&e.to).copied(),
            );
            let message = match rule {
                _ if from == Some(Kind::Boundary) || to == Some(Kind::Boundary) => {
                    Some("a relationship cannot join a system boundary".to_owned())
                }
                Some(r @ ("include" | "extend"))
                    if from != Some(Kind::UseCase) || to != Some(Kind::UseCase) =>
                {
                    Some(format!("an {r} relationship joins two use cases"))
                }
                Some("generalization")
                    if from != to || !matches!(from, Some(Kind::Actor | Kind::UseCase)) =>
                {
                    Some("a generalization joins two actors or two use cases".to_owned())
                }
                Some("marked") if from == Some(Kind::Json) || to == Some(Kind::Json) => Some(
                    "a relationship with a JSON table takes only `-->`, `<--`, or `--`".to_owned(),
                ),
                _ => None,
            };
            if let Some(m) = message {
                self.warning(range, m);
            }
        }
        // Element shapes, stereotypes, and flags.
        for (n, el) in self.chart.nodes.iter_mut().zip(&self.elements) {
            let kind = self.kinds.get(&n.id).copied();
            if kind == Some(Kind::Json) {
                continue;
            }
            let actor = kind == Some(Kind::Actor);
            n.label = match &el.stereotype {
                Some(s) => Label {
                    text: format!("«{s}»\n{}", el.label.text),
                    markdown: el.label.markdown,
                },
                None => el.label.clone(),
            };
            if actor {
                n.shape = match el.actor_type.as_deref() {
                    Some("hollow") if el.icon.is_none() => Shape::HollowActor,
                    Some("awesome") if el.icon.is_none() => Shape::AwesomeActor,
                    _ => Shape::Actor,
                };
                if let Some(icon) = &el.icon {
                    n.metadata.push(("icon".into(), icon.clone()));
                }
            } else {
                n.shape = if el.rect { Shape::Rect } else { Shape::UseCase };
            }
            if el.business {
                n.metadata.push(("business".into(), "true".into()));
            }
            let range = n.mentions.first().copied().unwrap_or_default();
            let id = n.id.clone();
            let problem = if actor {
                let t = el.actor_type.as_deref().unwrap_or("normal");
                if el.icon.is_some() && t != "normal" {
                    Some(format!(
                        "actor `{id}` cannot combine `icon` with `type: {t}`"
                    ))
                } else if el.business && (el.icon.is_some() || t == "awesome") {
                    Some(format!(
                        "business actor `{id}` must have `type: normal` or `type: hollow`"
                    ))
                } else {
                    None
                }
            } else if el.rect && el.business {
                Some(format!(
                    "rectangular use case `{id}` cannot be a business use case"
                ))
            } else {
                None
            };
            if let Some(p) = problem {
                self.diagnostics.push(Diagnostic::warning(range, p));
            }
        }
        // Actors first, then use cases, then JSON tables, each in first-mention order.
        let order = |n: &Node| {
            let kind = match self.kinds.get(&n.id) {
                Some(Kind::Actor) => 0,
                Some(Kind::UseCase) => 1,
                _ => 2,
            };
            (kind, n.mentions.iter().map(|r| r.start()).min())
        };
        self.chart.nodes.sort_by_key(order);
        for note in std::mem::take(&mut self.notes) {
            match self.kinds.get(&note.target) {
                Some(Kind::Actor | Kind::UseCase) => self.chart.notes.push(note),
                Some(k) => {
                    let m = format!(
                        "ignoring note for `{}`: a note is for an actor or use case, not a {}",
                        note.target,
                        k.name()
                    );
                    self.warning(note.range, m);
                }
                None => {
                    let m = format!("ignoring note for unknown `{}`", note.target);
                    self.warning(note.range, m);
                }
            }
        }
        for (id, styling, range) in std::mem::take(&mut self.styling) {
            let (classes, styles) =
                if let Some(n) = self.chart.nodes.iter_mut().find(|n| n.id == id) {
                    (&mut n.classes, &mut n.styles)
                } else if let Some(s) = self.chart.subgraphs.iter_mut().find(|s| s.id == id) {
                    (&mut s.classes, &mut s.styles)
                } else if let Some(e) = self
                    .chart
                    .edges
                    .iter_mut()
                    .find(|e| e.id.as_deref() == Some(id.as_str()))
                {
                    (&mut e.classes, &mut e.styles)
                } else {
                    let m = format!("unknown actor, use case, system boundary, or edge `{id}`");
                    self.diagnostics.push(Diagnostic::warning(range, m));
                    continue;
                };
            match styling {
                Styling::Class(c) => {
                    if !classes.contains(&c) {
                        classes.push(c);
                    }
                }
                Styling::Styles(s) => styles.extend(s),
            }
        }
        (self.chart, self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids() {
        assert_eq!(generated_id("Place order"), "Place_order");
        assert_eq!(generated_id("a-b.c"), "a_b_c");
        assert_eq!(generated_id("é"), "_");
        assert_eq!(generated_id("😀x"), "__x");
    }
}
