//! Builds an architecture diagram's services, junctions, groups, and edges (MARCH-2).

use std::collections::HashMap;

use super::{Align, Architecture, Item, Port, Side};
use crate::diagnostic::Diagnostic;
use crate::flowchart::lower::label_of;
use crate::flowchart::{Arrow, Edge, Label, Node, Shape, Stroke, Subgraph, text};
use crate::lines::{MAX_DEPTH, too_deep};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Architecture, Vec<Diagnostic>) {
    let mut l = Lowerer::default();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            l.arch.chart.title = text::frontmatter_title(tok.text());
        }
    }
    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => {
                l.arch.chart.title = token(&stmt, TEXT)
                    .map(|t| t.text().trim().to_owned())
                    .filter(|t| !t.is_empty());
            }
            ACC_TITLE_STMT => {
                l.arch.chart.acc_title = token(&stmt, ACC_TEXT).map(|t| t.text().trim().to_owned());
            }
            ACC_DESCR_STMT => {
                l.arch.chart.acc_descr = token(&stmt, ACC_TEXT).map(|t| t.text().trim().to_owned());
            }
            ENTITY => l.declaration(&stmt),
            RELATION_STMT => l.edge(&stmt),
            SETTING_STMT => l.align(&stmt),
            _ => {}
        }
    }
    (l.arch, l.diagnostics)
}

#[derive(Default)]
struct Lowerer {
    arch: Architecture,
    diagnostics: Vec<Diagnostic>,
    /// Service and junction ids to node indexes.
    nodes: HashMap<String, usize>,
    /// Group ids to subgraph indexes.
    groups: HashMap<String, usize>,
    /// Whether the nesting limit has been reported (LANGS-8).
    too_deep: bool,
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|n| n.kind() == kind)
}

/// One end of an edge: its id, side, and whether it has `{group}`.
struct End {
    id: SyntaxToken,
    side: Side,
    group: bool,
}

fn end(node: &SyntaxNode) -> Option<End> {
    Some(End {
        id: token(node, IDENT)?,
        side: Side::from_char(token(node, SIDE)?.text().chars().next()?)?,
        group: child(node, METADATA).is_some(),
    })
}

impl Lowerer {
    fn declaration(&mut self, stmt: &SyntaxNode) {
        if child(stmt, ERROR).is_some() {
            return;
        }
        let Some(kw) = token(stmt, REQ_KW) else {
            return;
        };
        let mut idents = tokens(stmt).filter(|t| t.kind() == IDENT);
        let Some(id) = idents.next() else {
            return;
        };
        let parent_tok = idents.next();
        let name = id.text().to_owned();
        if self.nodes.contains_key(&name) || self.groups.contains_key(&name) {
            self.diagnostics.push(Diagnostic::new(
                id.text_range(),
                format!("`{name}` is already declared; ignoring this"),
            ));
            return;
        }
        let parent = parent_tok.and_then(|p| match self.groups.get(p.text()) {
            Some(&g) => Some(g),
            None => {
                self.diagnostics.push(Diagnostic::new(
                    p.text_range(),
                    format!(
                        "no group `{}` declared above; drawing `{name}` outside any group",
                        p.text()
                    ),
                ));
                None
            }
        });
        let icon = child(stmt, SHAPE)
            .and_then(|s| token(&s, TEXT))
            .map(|t| t.text().trim().to_owned())
            .filter(|t| !t.is_empty());
        let label = child(stmt, LABEL).map(|l| label_of(&l)).unwrap_or_default();
        if kw.text() == "group" {
            // LANGS-8: the groups around `parent`, itself included; never more than the limit.
            let around = |mut g: usize| {
                let mut n = 1;
                while let Some(p) = self.arch.group_parent[g] {
                    g = p;
                    n += 1;
                }
                n
            };
            let parent = parent.filter(|&g| {
                let fits = around(g) < MAX_DEPTH;
                if !fits && !std::mem::replace(&mut self.too_deep, true) {
                    self.diagnostics.push(Diagnostic::new(
                        id.text_range(),
                        format!(
                            "{}; drawing a deeper group outside any group",
                            too_deep("groups", MAX_DEPTH)
                        ),
                    ));
                }
                fits
            });
            let s = self.arch.chart.subgraphs.len();
            self.groups.insert(name.clone(), s);
            self.arch.chart.subgraphs.push(Subgraph {
                id: name.clone(),
                title: label,
                direction: None,
                members: Vec::new(),
                classes: Vec::new(),
                styles: Vec::new(),
                metadata: Vec::new(),
                range: stmt.text_range(),
                region: false,
            });
            self.arch.group_icons.push(icon);
            self.arch.group_parent.push(parent);
        } else {
            let i = self.arch.chart.nodes.len();
            self.nodes.insert(name.clone(), i);
            let junction = kw.text() == "junction";
            self.arch.chart.nodes.push(Node {
                id: name.clone(),
                label: if junction { Label::default() } else { label },
                shape: if junction { Shape::Text } else { Shape::Rect },
                sections: Vec::new(),
                classes: Vec::new(),
                styles: Vec::new(),
                click: None,
                tooltip: None,
                metadata: Vec::new(),
                mentions: vec![id.text_range()],
            });
            self.arch.items.push(if junction {
                Item::Junction
            } else {
                Item::Service { icon }
            });
            self.arch.node_parent.push(parent);
        }
        if let Some(g) = parent {
            self.arch.chart.subgraphs[g].members.push(name);
        }
    }

    /// The node an edge end names, or an error.
    fn node(&mut self, id: &SyntaxToken) -> Option<usize> {
        if let Some(&i) = self.nodes.get(id.text()) {
            return Some(i);
        }
        let message = if self.groups.contains_key(id.text()) {
            format!(
                "`{}` is a group; an edge ends at a service or junction, with `{{group}}` after \
                 it to end at its group",
                id.text()
            )
        } else {
            format!("no service or junction `{}`; ignoring this edge", id.text())
        };
        self.diagnostics
            .push(Diagnostic::new(id.text_range(), message));
        None
    }

    fn edge(&mut self, stmt: &SyntaxNode) {
        if child(stmt, ERROR).is_some() {
            return;
        }
        let ends: Vec<SyntaxNode> = stmt.children().filter(|c| c.kind() == ENTITY_REF).collect();
        let (Some(a), Some(b), Some(link)) = (
            ends.first().and_then(end),
            ends.get(1).and_then(end),
            child(stmt, EDGE),
        ) else {
            return;
        };
        let (Some(from), Some(to)) = (self.node(&a.id), self.node(&b.id)) else {
            return;
        };
        let parents = (self.arch.node_parent[from], self.arch.node_parent[to]);
        let mut group_end = |e: &End, parent: Option<usize>| -> bool {
            if !e.group {
                return false;
            }
            let why = match parent {
                None => format!("`{}` is not in a group", e.id.text()),
                Some(_) if parents.0 == parents.1 => "both ends are in the same group".to_owned(),
                Some(_) => return true,
            };
            self.diagnostics.push(Diagnostic::warning(
                e.id.text_range(),
                format!("{why}; drawing the edge from `{}` itself", e.id.text()),
            ));
            false
        };
        let from_group = group_end(&a, parents.0);
        let to_group = group_end(&b, parents.1);
        let arrows: Vec<SyntaxToken> = tokens(&link).filter(|t| t.kind() == ARROW).collect();
        let start = arrows
            .iter()
            .any(|t| t.text() == "<")
            .then_some(Arrow::Point);
        let end = arrows
            .iter()
            .any(|t| t.text() == ">")
            .then_some(Arrow::Point);
        let label = child(&link, EDGE_LABEL)
            .map(|l| label_of(&l))
            .filter(|l| !l.text.trim().is_empty());
        let id_of = |n: usize, group: bool, parent: Option<usize>| match (group, parent) {
            (true, Some(g)) => self.arch.chart.subgraphs[g].id.clone(),
            _ => self.arch.chart.nodes[n].id.clone(),
        };
        let edge = Edge {
            id: None,
            from: id_of(from, from_group, parents.0),
            to: id_of(to, to_group, parents.1),
            stroke: Stroke::Normal,
            start,
            end,
            length: 1,
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
        };
        self.arch.chart.edges.push(edge);
        self.arch.ports.push(Port {
            from,
            to,
            from_side: a.side,
            to_side: b.side,
            from_group,
            to_group,
        });
    }

    fn align(&mut self, stmt: &SyntaxNode) {
        if child(stmt, ERROR).is_some() {
            return;
        }
        let row = token(stmt, IDENT).is_some_and(|t| t.text() == "row");
        let mut nodes = Vec::new();
        for r in stmt.children().filter(|c| c.kind() == ENTITY_REF) {
            let Some(id) = token(&r, IDENT) else { continue };
            match self.nodes.get(id.text()) {
                Some(&i) => nodes.push(i),
                None => self.diagnostics.push(Diagnostic::warning(
                    id.text_range(),
                    format!("no service or junction `{}`; ignoring it", id.text()),
                )),
            }
        }
        if nodes.len() < 2 {
            self.diagnostics.push(Diagnostic::warning(
                stmt.text_range(),
                "`align` needs at least two services or junctions; ignoring it",
            ));
            return;
        }
        self.arch.aligns.push(Align { row, nodes });
    }
}
