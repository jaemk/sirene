//! Builds a block diagram's grid, blocks, groups, and edges (MBLOCK-2).

use std::collections::HashMap;

use rowan::TextRange;

use super::{Block, Cell, CellKind, Group, Heading};
use crate::diagnostic::Diagnostic;
use crate::flowchart::lower::{edge_style, label_of};
use crate::flowchart::{ClassDef, Edge, Label, Node, Shape, Style, Subgraph, text};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Block, Vec<Diagnostic>) {
    let mut l = Lowerer {
        block: Block {
            groups: vec![Group::default()],
            padding: 8.0,
            ..Block::default()
        },
        ..Lowerer::default()
    };
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            l.block.chart.title = text::frontmatter_title(tok.text());
            for (k, v) in text::frontmatter_config(tok.text(), "block") {
                let n = v.parse::<f32>().ok().filter(|n| n.is_finite() && *n >= 0.0);
                if k == "padding" {
                    match n {
                        Some(n) => l.block.padding = n,
                        None => l.diagnostics.push(Diagnostic::warning(
                            tok.text_range(),
                            format!("ignoring `{k}: {v}`: not a valid value"),
                        )),
                    }
                }
            }
        }
    }
    l.statements(root, 0);
    // Styles last, so they may name a block defined below them.
    for stmt in root.descendants() {
        match stmt.kind() {
            CLASSDEF_STMT => l.class_def(&stmt),
            CLASS_STMT => l.class_stmt(&stmt),
            STYLE_STMT => l.style_stmt(&stmt),
            _ => {}
        }
    }
    l.check_widths();
    (l.block, l.diagnostics)
}

#[derive(Default)]
struct Lowerer {
    block: Block,
    diagnostics: Vec<Diagnostic>,
    /// Node ids to indexes.
    nodes: HashMap<String, usize>,
    /// Group ids to subgraph indexes.
    subgraphs: HashMap<String, usize>,
    /// Each cell given a width: its group, its index there, the width, and where it was given.
    widths: Vec<(usize, usize, usize, TextRange)>,
    anonymous: usize,
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

/// The ids in a statement's `ID_LIST`.
fn ids(stmt: &SyntaxNode) -> Vec<SyntaxToken> {
    child(stmt, ID_LIST)
        .map(|l| tokens(&l).filter(|t| t.kind() == IDENT).collect())
        .unwrap_or_default()
}

/// What a `NODE` says: its id, its shape and label if it has them, its width, and a block
/// arrow's directions.
struct Mention {
    id: String,
    shape: Option<Shape>,
    label: Option<Label>,
    width: Option<(usize, TextRange)>,
    arrow: Option<Vec<Heading>>,
}

fn mention(node: &SyntaxNode) -> Option<Mention> {
    let id = token(node, IDENT)?.text().to_owned();
    let mut m = Mention {
        id,
        shape: None,
        label: None,
        width: None,
        arrow: None,
    };
    if let Some(shape) = child(node, SHAPE) {
        if shape.children().any(|c| c.kind() == ERROR) {
            return None;
        }
        let open = token(&shape, SHAPE_OPEN)?;
        let close = token(&shape, SHAPE_CLOSE)?;
        m.label = child(&shape, LABEL).map(|l| label_of(&l));
        if open.text() == "<[" {
            m.shape = Some(Shape::Text);
            m.arrow = Some(
                tokens(&shape)
                    .filter(|t| t.kind() == DIR)
                    .filter_map(|t| Heading::from_text(t.text()))
                    .collect(),
            );
        } else {
            m.shape = Some(Shape::from_delimiters(open.text(), close.text()));
        }
    }
    if let Some(n) = token(node, NUMBER) {
        let w = n.text().parse::<usize>().unwrap_or(usize::MAX);
        m.width = Some((w, n.text_range()));
    }
    Some(m)
}

impl Lowerer {
    fn statements(&mut self, parent: &SyntaxNode, group: usize) {
        for stmt in parent.children() {
            match stmt.kind() {
                SETTING_STMT => {
                    if let Some(n) = token(&stmt, NUMBER) {
                        match n.text().parse::<usize>() {
                            Ok(c) if c > 0 => self.block.groups[group].columns = Some(c),
                            _ => self.diagnostics.push(Diagnostic::new(
                                n.text_range(),
                                "a group has at least 1 column; ignoring this",
                            )),
                        }
                    } else if token(&stmt, IDENT).is_some() {
                        self.block.groups[group].columns = None;
                    }
                }
                ACC_TITLE_STMT => {
                    self.block.chart.acc_title =
                        token(&stmt, ACC_TEXT).map(|t| t.text().trim().to_owned());
                }
                ACC_DESCR_STMT => {
                    self.block.chart.acc_descr =
                        token(&stmt, ACC_TEXT).map(|t| t.text().trim().to_owned());
                }
                COMPOSITE => self.composite(&stmt, group),
                VERTEX_STMT => self.vertex(&stmt, group),
                _ => {}
            }
        }
    }

    fn composite(&mut self, stmt: &SyntaxNode, group: usize) {
        let header = child(stmt, SUBGRAPH_HEADER);
        let m = header
            .as_ref()
            .and_then(|h| child(h, NODE))
            .and_then(|n| mention(&n));
        let id = match &m {
            Some(m) => m.id.clone(),
            None => {
                self.anonymous += 1;
                format!("block-{}", self.anonymous)
            }
        };
        let s = self.block.chart.subgraphs.len();
        self.subgraphs.insert(id.clone(), s);
        self.block.chart.subgraphs.push(Subgraph {
            id: id.clone(),
            title: m.as_ref().and_then(|m| m.label.clone()).unwrap_or_default(),
            direction: None,
            members: Vec::new(),
            classes: Vec::new(),
            styles: Vec::new(),
            metadata: Vec::new(),
            range: stmt.text_range(),
            region: false,
        });
        let g = self.block.groups.len();
        self.block.groups.push(Group {
            subgraph: Some(s),
            columns: None,
            cells: Vec::new(),
        });
        let width = m.as_ref().and_then(|m| m.width);
        self.place(group, CellKind::Group(g), width, &id);
        self.statements(stmt, g);
    }

    fn vertex(&mut self, stmt: &SyntaxNode, group: usize) {
        let mut prev: Option<String> = None;
        let mut edge: Option<SyntaxNode> = None;
        for el in stmt.children() {
            match el.kind() {
                NODE => {
                    let Some(m) = mention(&el) else {
                        prev = None;
                        continue;
                    };
                    if m.id == "space" && m.shape.is_none() {
                        let n = m.width.map_or(1, |(w, _)| w.min(1000));
                        for _ in 0..n {
                            self.block.groups[group].cells.push(Cell {
                                kind: CellKind::Space,
                                width: 1,
                            });
                        }
                        prev = None;
                        continue;
                    }
                    let id = m.id.clone();
                    self.node(m, group, el.text_range());
                    if let (Some(from), Some(e)) = (prev.take(), edge.take()) {
                        self.edge(&e, from, id.clone());
                    }
                    prev = Some(id);
                }
                EDGE => edge = Some(el),
                _ => {}
            }
        }
    }

    fn node(&mut self, m: Mention, group: usize, range: TextRange) {
        if self.subgraphs.contains_key(&m.id) {
            return;
        }
        match self.nodes.get(&m.id) {
            Some(&i) => {
                let node = &mut self.block.chart.nodes[i];
                if let Some(shape) = m.shape {
                    node.shape = shape;
                }
                if let Some(label) = m.label {
                    node.label = label;
                }
                if m.arrow.is_some() {
                    self.block.arrows[i] = m.arrow;
                }
                node.mentions.push(range);
            }
            None => {
                let i = self.block.chart.nodes.len();
                self.nodes.insert(m.id.clone(), i);
                self.block.chart.nodes.push(Node {
                    id: m.id.clone(),
                    label: m.label.unwrap_or_else(|| Label::plain(m.id.clone())),
                    shape: m.shape.unwrap_or(Shape::Rect),
                    sections: Vec::new(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                    click: None,
                    tooltip: None,
                    metadata: Vec::new(),
                    mentions: vec![range],
                });
                self.block.arrows.push(m.arrow);
                self.place(group, CellKind::Node(i), m.width, &m.id);
            }
        }
    }

    /// Adds a cell to `group`, and to its subgraph's members.
    fn place(&mut self, group: usize, kind: CellKind, width: Option<(usize, TextRange)>, id: &str) {
        let w = match width {
            Some((0, range)) => {
                self.diagnostics.push(Diagnostic::new(
                    range,
                    format!("`{id}` must be at least 1 column wide; drawing it 1 wide"),
                ));
                1
            }
            Some((w, range)) => {
                let k = self.block.groups[group].cells.len();
                self.widths.push((group, k, w, range));
                w.min(1000)
            }
            None => 1,
        };
        self.block.groups[group].cells.push(Cell { kind, width: w });
        if let Some(s) = self.block.groups[group].subgraph {
            self.block.chart.subgraphs[s].members.push(id.to_owned());
        }
    }

    fn edge(&mut self, e: &SyntaxNode, from: String, to: String) {
        let Some((stroke, start, end, _)) = edge_style(e) else {
            return;
        };
        let label = child(e, EDGE_LABEL)
            .map(|l| label_of(&l))
            .filter(|l| !l.text.trim().is_empty());
        self.block.chart.edges.push(Edge {
            id: None,
            from,
            to,
            stroke,
            start,
            end,
            length: 1,
            constraint: true,
            ports: [None; 2],
            label,
            start_label: None,
            end_label: None,
            classes: Vec::new(),
            styles: Vec::new(),
            curve: None,
            animate: false,
            animation: None,
            metadata: Vec::new(),
            range: e.text_range(),
        });
    }

    fn class_def(&mut self, stmt: &SyntaxNode) {
        let (styles, _) = text::styles(
            &token(stmt, STYLE_TEXT)
                .map(|t| t.text().to_owned())
                .unwrap_or_default(),
        );
        for name in ids(stmt) {
            let classes = &mut self.block.chart.classes;
            match classes.iter_mut().find(|c| c.name == name.text()) {
                Some(c) => c.styles.extend(styles.iter().cloned()),
                None => classes.push(ClassDef {
                    name: name.text().to_owned(),
                    styles: styles.clone(),
                }),
            }
        }
    }

    /// The classes or styles of the block or group `id`, or a warning when there is none.
    fn target(&mut self, id: &SyntaxToken) -> Option<(&mut Vec<String>, &mut Vec<Style>)> {
        let node = self.nodes.get(id.text()).copied();
        let group = self.subgraphs.get(id.text()).copied();
        let chart = &mut self.block.chart;
        match (node, group) {
            (Some(i), _) => {
                let n = &mut chart.nodes[i];
                Some((&mut n.classes, &mut n.styles))
            }
            (None, Some(s)) => {
                let g = &mut chart.subgraphs[s];
                Some((&mut g.classes, &mut g.styles))
            }
            (None, None) => {
                self.diagnostics.push(Diagnostic::warning(
                    id.text_range(),
                    format!("no block `{}`; ignoring it", id.text()),
                ));
                None
            }
        }
    }

    fn class_stmt(&mut self, stmt: &SyntaxNode) {
        let Some(class) = tokens(stmt).filter(|t| t.kind() == IDENT).last() else {
            return;
        };
        for id in ids(stmt) {
            if let Some((classes, _)) = self.target(&id) {
                classes.push(class.text().to_owned());
            }
        }
    }

    fn style_stmt(&mut self, stmt: &SyntaxNode) {
        let (styles, _) = text::styles(
            &token(stmt, STYLE_TEXT)
                .map(|t| t.text().to_owned())
                .unwrap_or_default(),
        );
        for id in ids(stmt) {
            if let Some((_, s)) = self.target(&id) {
                s.extend(styles.iter().cloned());
            }
        }
    }

    /// Narrows a block wider than its group's columns to fit, with a warning.
    fn check_widths(&mut self) {
        for &(g, k, w, range) in &self.widths {
            let group = &mut self.block.groups[g];
            if let Some(c) = group.columns
                && w > c
            {
                group.cells[k].width = c;
                self.diagnostics.push(Diagnostic::warning(
                    range,
                    format!(
                        "{w} columns is wider than the {c} columns of its group; drawing it {c} wide"
                    ),
                ));
            }
        }
    }
}
