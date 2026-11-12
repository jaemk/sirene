//! Builds a C4 diagram's elements, boundaries, and relations (MC4-2).

use std::collections::HashMap;

use super::{Base, Body, Boundary, C4, Colors, Element, Rel};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Arrow, Edge, Label, Node, Shape, Stroke, Subgraph, text};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (C4, Vec<Diagnostic>) {
    let mut l = Lowerer {
        c4: C4 {
            shapes_in_row: 4,
            boundaries_in_row: 2,
            ..C4::default()
        },
        ..Lowerer::default()
    };
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            l.c4.chart.title = text::frontmatter_title(tok.text());
        }
    }
    l.statements(root, None);
    // Relations and styles may name elements declared after them.
    for call in std::mem::take(&mut l.later) {
        l.late(&call);
    }
    (l.c4, l.diagnostics)
}

#[derive(Default)]
struct Lowerer {
    c4: C4,
    diagnostics: Vec<Diagnostic>,
    /// Element aliases to node indexes, and boundary aliases to subgraph indexes.
    nodes: HashMap<String, usize>,
    boundaries: HashMap<String, usize>,
    /// Relations and style updates, read once every element is known.
    later: Vec<Call>,
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

/// A call's name and arguments.
struct Call {
    name: SyntaxToken,
    node: SyntaxNode,
    /// Positional arguments, in order.
    args: Vec<(String, SyntaxNode)>,
    /// `$key=value` arguments.
    named: Vec<(String, String, SyntaxNode)>,
}

fn call(node: &SyntaxNode) -> Option<Call> {
    let name = token(node, IDENT)?;
    let mut args = Vec::new();
    let mut named = Vec::new();
    for a in node.children().filter(|c| c.kind() == ATTR) {
        let value = tokens(&a)
            .find(|t| matches!(t.kind(), STRING | TEXT))
            .map(|t| match t.kind() {
                STRING => text::unquote(t.text()).to_owned(),
                _ => t.text().trim().to_owned(),
            })
            .unwrap_or_default();
        match token(&a, ATTR_KEY) {
            Some(k) => {
                let key = k.text().trim_start_matches('$');
                let key = key.trim_end_matches('=').trim().to_owned();
                named.push((key, value, a));
            }
            None => args.push((value, a)),
        }
    }
    Some(Call {
        name,
        node: node.clone(),
        args,
        named,
    })
}

impl Call {
    /// The argument named `key`, by position in `params` or as `$key`; `None` when empty.
    fn get(&self, params: &[&str], key: &str) -> Option<String> {
        let by_name = self
            .named
            .iter()
            .rev()
            .find(|(k, _, _)| k == key)
            .map(|(_, v, _)| v.clone());
        let by_position = params
            .iter()
            .position(|p| *p == key)
            .and_then(|i| self.args.get(i))
            .map(|(v, _)| v.clone());
        by_name.or(by_position).filter(|v| !v.trim().is_empty())
    }
}

const PERSON: &[&str] = &["alias", "label", "descr", "sprite", "tags", "link", "type"];
const CONTAINER: &[&str] = &["alias", "label", "techn", "descr", "sprite", "tags", "link"];
const BOUNDARY: &[&str] = &["alias", "label", "type", "tags", "link"];
const NAMED_BOUNDARY: &[&str] = &["alias", "label", "tags", "link"];
const DEPLOYMENT: &[&str] = &["alias", "label", "type", "descr", "sprite", "tags", "link"];
const REL: &[&str] = &[
    "from", "to", "label", "techn", "descr", "sprite", "tags", "link",
];
const REL_INDEX: &[&str] = &[
    "index", "from", "to", "label", "techn", "descr", "sprite", "tags", "link",
];
const ELEMENT_STYLE: &[&str] = &[
    "elementName",
    "bgColor",
    "fontColor",
    "borderColor",
    "shadowing",
    "shape",
    "sprite",
    "techn",
    "legendText",
    "legendSprite",
];
const REL_STYLE: &[&str] = &["from", "to", "textColor", "lineColor", "offsetX", "offsetY"];
const LAYOUT: &[&str] = &["c4ShapeInRow", "c4BoundaryInRow"];

/// An element macro's base, body, and whether it is external.
fn element_kind(name: &str) -> Option<(Base, Body, bool)> {
    let (name, external) = match name.strip_suffix("_Ext") {
        Some(n) => (n, true),
        None => (name, false),
    };
    let (base, rest) = [
        ("Person", Base::Person),
        ("System", Base::System),
        ("Container", Base::Container),
        ("Component", Base::Component),
    ]
    .into_iter()
    .find_map(|(p, b)| name.strip_prefix(p).map(|r| (b, r)))?;
    let body = match rest {
        "" => Body::Box,
        "Db" if base != Base::Person => Body::Db,
        "Queue" if base != Base::Person => Body::Queue,
        _ => return None,
    };
    Some((base, body, external))
}

/// A boundary macro's parameters, fixed type, and whether it is a deployment node.
fn boundary_kind(name: &str) -> Option<(&'static [&'static str], Option<&'static str>, bool)> {
    Some(match name {
        "Boundary" => (BOUNDARY, None, false),
        "Enterprise_Boundary" => (NAMED_BOUNDARY, Some("ENTERPRISE"), false),
        "System_Boundary" => (NAMED_BOUNDARY, Some("SYSTEM"), false),
        "Container_Boundary" => (NAMED_BOUNDARY, Some("CONTAINER"), false),
        "Deployment_Node" | "Node" | "Node_L" | "Node_R" => (DEPLOYMENT, None, true),
        _ => return None,
    })
}

fn is_rel(name: &str) -> bool {
    matches!(
        name,
        "Rel"
            | "BiRel"
            | "Rel_U"
            | "Rel_Up"
            | "Rel_D"
            | "Rel_Down"
            | "Rel_L"
            | "Rel_Left"
            | "Rel_R"
            | "Rel_Right"
            | "Rel_Back"
            | "RelIndex"
    )
}

fn label(s: &str) -> Label {
    text::label(s, false)
}

impl Lowerer {
    fn statements(&mut self, node: &SyntaxNode, parent: Option<usize>) {
        // A boundary's own call heads its block.
        let head = (node.kind() == COMPOSITE)
            .then(|| child(node, ENTITY))
            .flatten();
        for stmt in node.children() {
            if Some(&stmt) == head.as_ref() {
                continue;
            }
            match stmt.kind() {
                TITLE_STMT => {
                    self.c4.chart.title = token(&stmt, TEXT)
                        .map(|t| t.text().trim().to_owned())
                        .filter(|t| !t.is_empty());
                }
                ACC_TITLE_STMT => {
                    self.c4.chart.acc_title =
                        token(&stmt, ACC_TEXT).map(|t| t.text().trim().to_owned());
                }
                ACC_DESCR_STMT => {
                    self.c4.chart.acc_descr =
                        token(&stmt, ACC_TEXT).map(|t| t.text().trim().to_owned());
                }
                ENTITY => {
                    if let Some(c) = self.whole(&stmt) {
                        self.call(c, parent);
                    }
                }
                COMPOSITE => {
                    let Some(head) = child(&stmt, ENTITY) else {
                        continue;
                    };
                    let inner = match self.whole(&head) {
                        Some(c) if boundary_kind(c.name.text()).is_some() => {
                            self.boundary(&c, parent).or(parent)
                        }
                        Some(c) => {
                            self.diagnostics.push(Diagnostic::warning(
                                c.name.text_range(),
                                format!(
                                    "`{}` is not a boundary; drawing what is inside it outside",
                                    c.name.text()
                                ),
                            ));
                            self.call(c, parent);
                            parent
                        }
                        None => parent,
                    };
                    self.statements(&stmt, inner);
                }
                _ => {}
            }
        }
    }

    /// The call in `node` when it parsed without errors.
    fn whole(&self, node: &SyntaxNode) -> Option<Call> {
        if child(node, ERROR).is_some() {
            return None;
        }
        call(node)
    }

    fn call(&mut self, c: Call, parent: Option<usize>) {
        let name = c.name.text();
        if let Some(kind) = element_kind(name) {
            self.element(&c, kind, parent);
        } else if boundary_kind(name).is_some() {
            // A boundary without braces holds nothing.
            self.boundary(&c, parent);
        } else if is_rel(name) || matches!(name, "UpdateElementStyle" | "UpdateRelStyle") {
            self.later.push(c);
        } else if name == "UpdateLayoutConfig" {
            self.layout_config(&c);
        } else {
            self.diagnostics.push(Diagnostic::warning(
                c.name.text_range(),
                format!("`{name}` is not supported; ignoring it"),
            ));
        }
    }

    /// The alias, or an error.
    fn alias(&mut self, c: &Call, params: &[&str]) -> Option<String> {
        let alias = c.get(params, "alias");
        let Some(alias) = alias else {
            self.diagnostics.push(Diagnostic::new(
                c.node.text_range(),
                format!("`{}` needs an alias first", c.name.text()),
            ));
            return None;
        };
        if self.nodes.contains_key(&alias) || self.boundaries.contains_key(&alias) {
            self.diagnostics.push(Diagnostic::new(
                c.node.text_range(),
                format!("`{alias}` is already declared; ignoring this"),
            ));
            return None;
        }
        Some(alias)
    }

    fn element(
        &mut self,
        c: &Call,
        (base, body, external): (Base, Body, bool),
        parent: Option<usize>,
    ) {
        let params = match base {
            Base::Person | Base::System => PERSON,
            Base::Container | Base::Component => CONTAINER,
        };
        let Some(alias) = self.alias(c, params) else {
            return;
        };
        let i = self.c4.chart.nodes.len();
        self.nodes.insert(alias.clone(), i);
        let shape = match body {
            Body::Box => Shape::Rounded,
            Body::Db => Shape::Cylinder,
            Body::Queue => Shape::HorizontalCylinder,
        };
        self.c4.chart.nodes.push(Node {
            id: alias.clone(),
            label: label(&c.get(params, "label").unwrap_or_else(|| alias.clone())),
            shape,
            sections: Vec::new(),
            classes: Vec::new(),
            styles: Vec::new(),
            click: None,
            tooltip: None,
            metadata: Vec::new(),
            mentions: vec![c.node.text_range()],
        });
        self.c4.elements.push(Element {
            base,
            body,
            external,
            techn: c.get(params, "techn").map(|t| label(&t).text),
            descr: c.get(params, "descr").map(|t| label(&t).text),
            colors: Colors::default(),
        });
        self.c4.node_parent.push(parent);
        if let Some(g) = parent {
            self.c4.chart.subgraphs[g].members.push(alias);
        }
    }

    fn boundary(&mut self, c: &Call, parent: Option<usize>) -> Option<usize> {
        let (params, kind, deployment) = boundary_kind(c.name.text())?;
        let alias = self.alias(c, params)?;
        let g = self.c4.chart.subgraphs.len();
        self.boundaries.insert(alias.clone(), g);
        self.c4.chart.subgraphs.push(Subgraph {
            id: alias.clone(),
            title: label(&c.get(params, "label").unwrap_or_else(|| alias.clone())),
            direction: None,
            members: Vec::new(),
            classes: Vec::new(),
            styles: Vec::new(),
            metadata: Vec::new(),
            range: c
                .node
                .parent()
                .unwrap_or_else(|| c.node.clone())
                .text_range(),
            region: false,
        });
        self.c4.boundaries.push(Boundary {
            kind: kind.map(str::to_owned).or_else(|| c.get(params, "type")),
            descr: c.get(params, "descr").map(|t| label(&t).text),
            deployment,
        });
        self.c4.boundary_parent.push(parent);
        if let Some(p) = parent {
            self.c4.chart.subgraphs[p].members.push(alias);
        }
        Some(g)
    }

    fn layout_config(&mut self, c: &Call) {
        for (key, slot) in [
            ("c4ShapeInRow", &mut self.c4.shapes_in_row),
            ("c4BoundaryInRow", &mut self.c4.boundaries_in_row),
        ] {
            let Some(v) = c.get(LAYOUT, key) else {
                continue;
            };
            match v.trim().parse::<usize>() {
                Ok(n) if n > 0 => *slot = n,
                _ => self.diagnostics.push(Diagnostic::warning(
                    c.node.text_range(),
                    format!("`{key}` must be a whole number of at least 1; ignoring `{v}`"),
                )),
            }
        }
    }

    /// An element or boundary id, or an error.
    fn target(&mut self, c: &Call, id: Option<String>, what: &str) -> Option<String> {
        let Some(id) = id else {
            self.diagnostics.push(Diagnostic::new(
                c.node.text_range(),
                format!("`{}` needs {what}", c.name.text()),
            ));
            return None;
        };
        if self.nodes.contains_key(&id) || self.boundaries.contains_key(&id) {
            return Some(id);
        }
        self.diagnostics.push(Diagnostic::new(
            c.node.text_range(),
            format!("no element or boundary `{id}`; ignoring this"),
        ));
        None
    }

    fn late(&mut self, c: &Call) {
        match c.name.text() {
            "UpdateElementStyle" => self.element_style(c),
            "UpdateRelStyle" => self.rel_style(c),
            _ => self.rel(c),
        }
    }

    fn rel(&mut self, c: &Call) {
        let name = c.name.text();
        let params = if name == "RelIndex" { REL_INDEX } else { REL };
        let from = self.target(c, c.get(params, "from"), "a `from` and a `to`");
        let to = self.target(c, c.get(params, "to"), "a `from` and a `to`");
        let (Some(mut from), Some(mut to)) = (from, to) else {
            return;
        };
        if name == "Rel_Back" {
            std::mem::swap(&mut from, &mut to);
        }
        let start = (name == "BiRel").then_some(Arrow::Point);
        self.c4.chart.edges.push(Edge {
            id: None,
            from,
            to,
            stroke: Stroke::Normal,
            start,
            end: Some(Arrow::Point),
            length: 1,
            constraint: true,
            ports: [None; 2],
            rows: [None; 2],
            label: c.get(params, "label").map(|t| label(&t)),
            start_label: None,
            end_label: None,
            classes: Vec::new(),
            styles: Vec::new(),
            curve: None,
            animate: false,
            animation: None,
            metadata: Vec::new(),
            range: c.node.text_range(),
        });
        self.c4.rels.push(Rel {
            techn: c.get(params, "techn").map(|t| label(&t).text),
            descr: c.get(params, "descr").map(|t| label(&t).text),
            colors: Colors::default(),
        });
    }

    fn element_style(&mut self, c: &Call) {
        let Some(id) = self.target(c, c.get(ELEMENT_STYLE, "elementName"), "an element") else {
            return;
        };
        let Some(&i) = self.nodes.get(&id) else {
            self.diagnostics.push(Diagnostic::warning(
                c.node.text_range(),
                format!("`{id}` is a boundary; boundary styles are not drawn"),
            ));
            return;
        };
        let colors = &mut self.c4.elements[i].colors;
        if let Some(v) = c.get(ELEMENT_STYLE, "bgColor") {
            colors.background = Some(v);
        }
        if let Some(v) = c.get(ELEMENT_STYLE, "fontColor") {
            colors.font = Some(v);
        }
        if let Some(v) = c.get(ELEMENT_STYLE, "borderColor") {
            colors.line = Some(v);
        }
    }

    fn rel_style(&mut self, c: &Call) {
        let from = self.target(c, c.get(REL_STYLE, "from"), "a `from` and a `to`");
        let to = self.target(c, c.get(REL_STYLE, "to"), "a `from` and a `to`");
        let (Some(from), Some(to)) = (from, to) else {
            return;
        };
        let text = c.get(REL_STYLE, "textColor");
        let line = c.get(REL_STYLE, "lineColor");
        let mut any = false;
        for (e, r) in self.c4.chart.edges.iter().zip(&mut self.c4.rels) {
            if e.from == from && e.to == to {
                any = true;
                if text.is_some() {
                    r.colors.font.clone_from(&text);
                }
                if line.is_some() {
                    r.colors.line.clone_from(&line);
                }
            }
        }
        if !any {
            self.diagnostics.push(Diagnostic::warning(
                c.node.text_range(),
                format!("no relation from `{from}` to `{to}`; ignoring this"),
            ));
        }
    }
}
