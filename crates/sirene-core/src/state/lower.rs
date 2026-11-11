//! Lowers a state diagram syntax tree to the graph model (MSTATE-2).

use std::collections::{HashMap, HashSet};

use rowan::TextRange;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::flowchart::{
    Arrow, ClassDef, Direction, Edge, Flowchart, Label, Node, Note, NoteSide, Shape, Stroke, Style,
    Subgraph,
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
    for c in root.descendants().filter(|n| n.kind() == COMPOSITE) {
        if let Some(id) = token(&c, IDENT) {
            l.composites.insert(id.text().to_owned());
        }
    }
    l.block(root, ROOT_SCOPE);
    l.finish()
}

/// The container id of the diagram itself.
const ROOT_SCOPE: &str = "";

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

/// A label from description text: `<br>` and `\n` sequences break lines.
fn label(raw: &str) -> Label {
    text::label(&raw.trim().replace("\\n", "\n"), false)
}

#[derive(Default)]
struct Lowerer {
    chart: Flowchart,
    diagnostics: Vec<Diagnostic>,
    node_index: HashMap<String, usize>,
    /// Ids declared with `state X {`.
    composites: HashSet<String>,
    /// Lowered composite states and regions, in the order they open.
    subgraphs: Vec<Subgraph>,
    /// Members of each container, by container id.
    members: HashMap<String, Vec<String>>,
    /// The container of each node and subgraph.
    parent: HashMap<String, String>,
    /// Container ids that are regions.
    regions: HashSet<String>,
    /// States given a description, whose later descriptions add lines.
    described: HashSet<String>,
    /// `class`, `style`, and `:::` statements, applied once every state exists.
    styling: Vec<(String, Styling, TextRange)>,
    /// The source range of each edge, for the region check.
    edge_ranges: Vec<TextRange>,
}

enum Styling {
    Class(String),
    Styles(Vec<Style>),
}

impl Lowerer {
    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn add_member(&mut self, container: &str, id: &str) {
        let list = self.members.entry(container.to_owned()).or_default();
        if !list.iter().any(|m| m == id) {
            list.push(id.to_owned());
        }
        self.parent
            .entry(id.to_owned())
            .or_insert_with(|| container.to_owned());
    }

    /// The node `id`, created in `container` with `shape` and `label` if new.
    fn node(&mut self, id: &str, container: &str, shape: Shape, label: Label) -> usize {
        if let Some(&i) = self.node_index.get(id) {
            return i;
        }
        let i = self.chart.nodes.len();
        self.chart.nodes.push(Node {
            id: id.to_owned(),
            label,
            shape,
            sections: Vec::new(),
            classes: Vec::new(),
            styles: Vec::new(),
            click: None,
            tooltip: None,
            metadata: Vec::new(),
            mentions: Vec::new(),
        });
        self.node_index.insert(id.to_owned(), i);
        self.add_member(container, id);
        i
    }

    /// The id a `STATE_REF` names, creating its state. `[*]` is the start state of `container`
    /// as a source and its end state as a target.
    fn reference(&mut self, r: &SyntaxNode, container: &str, target: bool) -> Option<String> {
        let id = if token(r, STAR).is_some() {
            let (suffix, shape) = if target {
                ("end", Shape::FramedCircle)
            } else {
                ("start", Shape::SmallCircle)
            };
            let id = format!("{container}:[*]{suffix}");
            self.node(&id, container, shape, Label::default());
            id
        } else {
            let name = token(r, IDENT)?.text().to_owned();
            if !self.composites.contains(&name) {
                let label = Label::plain(&name);
                self.node(&name, container, Shape::Rounded, label);
            }
            name
        };
        let class = tokens(r)
            .skip_while(|t| t.kind() != TRIPLE_COLON)
            .find(|t| t.kind() == IDENT);
        if let Some(class) = class {
            self.styling.push((
                id.clone(),
                Styling::Class(class.text().to_owned()),
                r.text_range(),
            ));
        }
        if let Some(i) = self.node_index.get(&id) {
            self.chart.nodes[*i].mentions.push(r.text_range());
        }
        Some(id)
    }

    /// Lowers the statements of `parent` into `container`, splitting them into concurrent
    /// regions at `--`. Returns the last `direction`.
    fn block(&mut self, parent: &SyntaxNode, container: &str) -> Option<Direction> {
        let stmts: Vec<SyntaxNode> = parent.children().collect();
        let splits = stmts.iter().filter(|s| s.kind() == REGION_STMT).count();
        if splits > 0 && container == ROOT_SCOPE {
            for s in stmts.iter().filter(|s| s.kind() == REGION_STMT) {
                self.warning(
                    s.text_range(),
                    "`--` separates regions only inside a composite state",
                );
            }
        }
        let regions = splits > 0 && container != ROOT_SCOPE;
        let mut region = 0;
        let mut scope = container.to_owned();
        if regions {
            scope = self.open_region(container, region, &stmts);
        }
        let mut direction = None;
        for stmt in &stmts {
            match stmt.kind() {
                REGION_STMT if regions => {
                    region += 1;
                    scope = self.open_region(container, region, &stmts);
                }
                DIRECTION_STMT => {
                    let dir = text_of(stmt, DIR).and_then(|d| Direction::from_text(&d));
                    if container == ROOT_SCOPE {
                        if let Some(d) = dir {
                            self.chart.direction = d;
                        }
                    } else {
                        direction = dir.or(direction);
                    }
                }
                TRANSITION_STMT => self.transition(stmt, &scope),
                STATE_STMT => self.state(stmt, &scope),
                DESCRIPTION_STMT => self.description(stmt, &scope),
                COMPOSITE => self.composite(stmt, &scope),
                NOTE_STMT => self.note(stmt, &scope),
                CLASSDEF_STMT => self.class_def(stmt),
                CLASS_STMT => self.class(stmt),
                STYLE_STMT => self.style(stmt),
                ACC_TITLE_STMT => self.chart.acc_title = text_of(stmt, ACC_TEXT),
                ACC_DESCR_STMT => self.chart.acc_descr = text_of(stmt, ACC_TEXT),
                CLICK_STMT => self.warning(
                    stmt.text_range(),
                    "`click` is not supported in state diagrams; ignoring it",
                ),
                _ => {}
            }
        }
        direction
    }

    /// Starts region `k` of `container` and returns its id.
    fn open_region(&mut self, container: &str, k: usize, stmts: &[SyntaxNode]) -> String {
        let id = format!("{container}:{k}");
        let range = stmts
            .iter()
            .filter(|s| s.kind() == REGION_STMT)
            .nth(k.saturating_sub(1))
            .map_or_else(
                || {
                    stmts
                        .first()
                        .map_or_else(TextRange::default, |s| s.text_range())
                },
                |s| s.text_range(),
            );
        self.subgraphs.push(Subgraph {
            id: id.clone(),
            title: Label::default(),
            direction: None,
            members: Vec::new(),
            classes: Vec::new(),
            styles: Vec::new(),
            metadata: Vec::new(),
            range,
            region: true,
        });
        self.regions.insert(id.clone());
        self.add_member(container, &id);
        id
    }

    fn transition(&mut self, stmt: &SyntaxNode, scope: &str) {
        let refs: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == STATE_REF).collect();
        let [from, to] = refs.as_slice() else {
            return;
        };
        let (Some(from), Some(to)) = (
            self.reference(from, scope, false),
            self.reference(to, scope, true),
        ) else {
            return;
        };
        let label = text_of(stmt, TEXT)
            .filter(|t| !t.is_empty())
            .map(|t| label(&t));
        self.edge_ranges.push(stmt.text_range());
        self.chart.edges.push(Edge {
            id: None,
            from,
            to,
            stroke: Stroke::Normal,
            start: None,
            end: Some(Arrow::Point),
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
        });
    }

    /// `state "Desc" as s`, `state s <<fork>>`, `state s : Desc`, or a bare state.
    fn state(&mut self, stmt: &SyntaxNode, scope: &str) {
        if let Some(r) = child(stmt, STATE_REF) {
            self.reference(&r, scope, false);
            return;
        }
        let Some(id) = token(stmt, IDENT).map(|t| t.text().to_owned()) else {
            return;
        };
        if self.composites.contains(&id) {
            return;
        }
        let shape = match text_of(stmt, STEREOTYPE).as_deref() {
            None => Shape::Rounded,
            Some("<<fork>>" | "<<join>>") => Shape::Fork,
            Some("<<choice>>") => Shape::Diamond,
            Some(other) => {
                self.warning(
                    stmt.text_range(),
                    format!("`{other}` states are not supported; drawing a plain state"),
                );
                Shape::Rounded
            }
        };
        let plain = shape == Shape::Rounded;
        let i = self.node(&id, scope, shape, Label::plain(&id));
        let node = &mut self.chart.nodes[i];
        node.mentions.push(stmt.text_range());
        if !plain {
            node.shape = shape;
            node.label = Label::default();
        }
        if let Some(s) = token(stmt, STRING) {
            self.chart.nodes[i].label = label(text::unquote(s.text()));
            self.described.insert(id.clone());
        }
        if let Some(t) = text_of(stmt, TEXT) {
            self.describe(&id, &t);
        }
    }

    fn description(&mut self, stmt: &SyntaxNode, scope: &str) {
        let Some(r) = child(stmt, STATE_REF) else {
            return;
        };
        let Some(id) = self.reference(&r, scope, false) else {
            return;
        };
        let t = text_of(stmt, TEXT).unwrap_or_default();
        self.describe(&id, &t);
    }

    /// Sets the description of `id`, or adds a line to it.
    fn describe(&mut self, id: &str, t: &str) {
        let new = label(t);
        let first = self.described.insert(id.to_owned());
        let target = match self.node_index.get(id) {
            Some(&i) => &mut self.chart.nodes[i].label,
            None => match self.subgraphs.iter_mut().find(|s| s.id == id) {
                Some(s) => &mut s.title,
                None => return,
            },
        };
        if first {
            *target = new;
        } else {
            target.text.push('\n');
            target.text.push_str(&new.text);
        }
    }

    fn composite(&mut self, stmt: &SyntaxNode, scope: &str) {
        let Some(id) = token(stmt, IDENT).map(|t| t.text().to_owned()) else {
            return;
        };
        if self.subgraphs.iter().any(|s| s.id == id) {
            self.warning(
                stmt.text_range(),
                format!("composite state `{id}` is already defined; adding to it"),
            );
        } else {
            let title = token(stmt, STRING)
                .map_or_else(|| Label::plain(&id), |s| label(text::unquote(s.text())));
            if token(stmt, STRING).is_some() {
                self.described.insert(id.clone());
            }
            self.subgraphs.push(Subgraph {
                id: id.clone(),
                title,
                direction: None,
                members: Vec::new(),
                classes: Vec::new(),
                styles: Vec::new(),
                metadata: Vec::new(),
                range: stmt.text_range(),
                region: false,
            });
            self.add_member(scope, &id);
        }
        let direction = self.block(stmt, &id);
        if let Some(d) = direction
            && let Some(s) = self.subgraphs.iter_mut().find(|s| s.id == id)
        {
            s.direction = Some(d);
        }
    }

    fn note(&mut self, stmt: &SyntaxNode, scope: &str) {
        // The parser has reported it.
        if child(stmt, ERROR).is_some() {
            return;
        }
        let Some(side) = text_of(stmt, SIDE) else {
            self.warning(
                stmt.text_range(),
                "floating notes are not supported; ignoring it",
            );
            return;
        };
        let Some(target) = text_of(stmt, IDENT) else {
            return;
        };
        // A note on a composite state sits beside its box.
        if !self.composites.contains(&target) {
            let label = Label::plain(target.clone());
            self.node(&target, scope, Shape::Rounded, label);
        }
        let body: Vec<String> = token(stmt, TEXT)
            .map(|t| {
                t.text()
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        self.chart.notes.push(Note {
            target,
            side: if side == "left" {
                NoteSide::Before
            } else {
                NoteSide::After
            },
            label: text::label(&body.join("\n"), false),
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

    /// The containers holding `id`, outermost first.
    fn chain(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut at = self.parent.get(id);
        while let Some(c) = at {
            if c == ROOT_SCOPE || out.contains(c) {
                break;
            }
            out.push(c.clone());
            at = self.parent.get(c);
        }
        out.reverse();
        out
    }

    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        for (id, styling, range) in std::mem::take(&mut self.styling) {
            let (classes, styles) = if let Some(&i) = self.node_index.get(&id) {
                let n = &mut self.chart.nodes[i];
                (&mut n.classes, &mut n.styles)
            } else if let Some(s) = self.subgraphs.iter_mut().find(|s| s.id == id) {
                (&mut s.classes, &mut s.styles)
            } else {
                self.warning(range, format!("unknown state `{id}`"));
                continue;
            };
            match styling {
                Styling::Class(c) => classes.push(c),
                Styling::Styles(s) => styles.extend(s),
            }
        }

        // MSTATE-4: a transition between two regions of one composite state.
        let mut keep = Vec::new();
        for (e, range) in self.chart.edges.iter().zip(&self.edge_ranges) {
            let (a, b) = (self.chain(&e.from), self.chain(&e.to));
            let split = a
                .iter()
                .zip(&b)
                .find(|(x, y)| x != y)
                .is_some_and(|(x, y)| self.regions.contains(x) && self.regions.contains(y));
            keep.push(!split);
            if split {
                self.diagnostics.push(Diagnostic::warning(
                    *range,
                    format!(
                        "transitions between concurrent regions are not supported; ignoring `{} --> {}`",
                        e.from, e.to
                    ),
                ));
            }
        }
        let mut k = keep.into_iter();
        self.chart.edges.retain(|_| k.next().unwrap_or(true));

        for s in &mut self.subgraphs {
            s.members = self.members.remove(&s.id).unwrap_or_default();
        }
        self.chart.subgraphs = self.subgraphs;
        (self.chart, self.diagnostics)
    }
}
