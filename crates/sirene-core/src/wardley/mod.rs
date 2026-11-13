//! Mermaid Wardley maps (MWARD-*): components placed by visibility and evolution, linked
//! along their value chain.

mod parser;

use rowan::{GreenNode, TextRange};

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// The smallest `size` side: smaller leaves no room inside the padding.
const MIN_SIZE: f64 = 100.0;

/// The largest `size` side.
const MAX_SIZE: f64 = 20000.0;

/// What a node is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A user or customer: a bold label with no circle.
    Anchor,
    Component,
    /// A component inside a `pipeline` block, at its parent's visibility.
    PipelineComponent,
}

/// How a component is sourced, from its `(build)`, `(buy)`, `(outsource)`, or `(market)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Build,
    Buy,
    Outsource,
    Market,
}

impl Strategy {
    pub fn name(self) -> &'static str {
        match self {
            Strategy::Build => "build",
            Strategy::Buy => "buy",
            Strategy::Outsource => "outsource",
            Strategy::Market => "market",
        }
    }
}

/// An anchor or component. Positions are percentages: `x` is evolution and `y` visibility,
/// both from 0 to 100.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The name links refer to; a pipeline component's is `parent_name`.
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub kind: NodeKind,
    /// The `label [x, y]` offset in pixels, if given.
    pub label_offset: Option<(f32, f32)>,
    pub inertia: bool,
    pub strategy: Option<Strategy>,
    /// Whether a `pipeline` block names it, so it is drawn as a square above the pipeline.
    pub pipeline_parent: bool,
}

/// Which way a link's flow arrows point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Forward,
    Backward,
    Both,
}

/// A dependency between two nodes, by index.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub source: usize,
    pub target: usize,
    pub dashed: bool,
    pub label: Option<String>,
    pub flow: Option<Flow>,
}

/// An `evolve` arrow from a node to evolution `x` (a percentage) at visibility `y`.
#[derive(Debug, Clone, PartialEq)]
pub struct Trend {
    pub node: usize,
    pub x: f64,
    pub y: f64,
}

/// A `pipeline` block: its parent node and its components, by index.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub parent: usize,
    pub components: Vec<usize>,
}

/// A numbered annotation at `x`, `y` (percentages).
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub number: u32,
    pub x: f64,
    pub y: f64,
    pub text: String,
}

/// Text at a position: a note, accelerator, or deaccelerator.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub text: String,
    pub x: f64,
    pub y: f64,
}

/// A Wardley map (MWARD-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Wardley {
    pub title: Option<String>,
    /// The `size [width, height]` in pixels, if given.
    pub size: Option<(f32, f32)>,
    /// The `evolution` stages, or none for the default four.
    pub stages: Vec<String>,
    /// The stage boundaries given with `@`, from 0 to 1.
    pub boundaries: Vec<f64>,
    /// Anchors, then components, then pipeline components, as Mermaid orders them.
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub trends: Vec<Trend>,
    pub pipelines: Vec<Pipeline>,
    pub notes: Vec<Marker>,
    pub annotations: Vec<Annotation>,
    /// Where the list of annotation texts goes, if anywhere.
    pub annotations_box: Option<(f64, f64)>,
    pub accelerators: Vec<Marker>,
    pub deaccelerators: Vec<Marker>,
}

/// Mermaid's evolution stages when there is no `evolution` statement.
pub const DEFAULT_STAGES: [&str; 4] = ["Genesis", "Custom Built", "Product", "Commodity"];

impl Wardley {
    /// The index of the node `name` refers to: the one with that id, else the first with that
    /// label.
    pub fn resolve(&self, name: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.id == name)
            .or_else(|| self.nodes.iter().position(|n| n.label == name))
    }

    /// The stages to draw: the given ones, or the default four.
    pub fn stage_names(&self) -> Vec<&str> {
        if self.stages.is_empty() {
            DEFAULT_STAGES.to_vec()
        } else {
            self.stages.iter().map(String::as_str).collect()
        }
    }

    /// Adds `node`, or updates the node with its id: its position, kind, and decorators, and
    /// its label offset when given.
    fn add(&mut self, node: Node) -> usize {
        match self.nodes.iter().position(|n| n.id == node.id) {
            Some(i) => {
                let old = &mut self.nodes[i];
                let offset = node.label_offset.or(old.label_offset);
                let parent = old.pipeline_parent;
                *old = Node {
                    label_offset: offset,
                    pipeline_parent: parent,
                    ..node
                };
                i
            }
            None => {
                self.nodes.push(node);
                self.nodes.len() - 1
            }
        }
    }
}

/// The result of parsing Wardley map source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

/// `text` trimmed, without surrounding double quotes.
fn unquote(text: &str) -> String {
    let t = text.trim();
    t.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(t)
        .to_owned()
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + use<> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
}

/// The first direct name (`IDENT` or `STRING`) of `node`, unquoted.
fn name(node: &SyntaxNode) -> Option<String> {
    tokens(node)
        .find(|t| matches!(t.kind(), IDENT | STRING))
        .map(|t| unquote(t.text()))
}

/// The numbers of `node`'s first child of `kind`.
fn numbers(node: &SyntaxNode, kind: SyntaxKind) -> Vec<f64> {
    node.children()
        .find(|n| n.kind() == kind)
        .into_iter()
        .flat_map(|c| tokens(&c).collect::<Vec<_>>())
        .filter(|t| t.kind() == NUMBER)
        .filter_map(|t| t.text().parse().ok())
        .collect()
}

/// The `label [x, y]` offset of `node`, if any.
fn offset(node: &SyntaxNode) -> Option<(f32, f32)> {
    let label = node.children().find(|n| n.kind() == LABEL)?;
    match numbers(&label, COORDS)[..] {
        [x, y] => Some((x as f32, y as f32)),
        _ => None,
    }
}

/// Lowers coordinates and reports values out of range.
struct Lowering {
    diagnostics: Vec<Diagnostic>,
}

impl Lowering {
    /// `value` as a percentage: values up to 1 are fractions, others percentages up to 100.
    fn percent(&mut self, value: f64, range: TextRange, what: &str) -> Option<f64> {
        let p = if value <= 1.0 { value * 100.0 } else { value };
        if (0.0..=100.0).contains(&p) {
            Some(p)
        } else {
            self.diagnostics.push(Diagnostic::new(
                range,
                format!("{what} must be from 0 to 1, or a percentage up to 100; got {value}"),
            ));
            None
        }
    }

    /// A statement's `[visibility, evolution]` as percentages `(x, y)`.
    fn coords(&mut self, node: &SyntaxNode, what: &str) -> Option<(f64, f64)> {
        let [v, e] = numbers(node, COORDS)[..] else {
            return None;
        };
        let range = node.text_range();
        let x = self.percent(e, range, &format!("{what}'s evolution"))?;
        let y = self.percent(v, range, &format!("{what}'s visibility"))?;
        Some((x, y))
    }
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Wardley`] map, with the parser's diagnostics and any problems
    /// with names and coordinates. Statements are applied in Mermaid's order: anchors,
    /// components, notes, pipelines, links, evolves, annotations, and accelerators.
    pub fn lower(&self) -> (Wardley, Vec<Diagnostic>) {
        let root = self.syntax();
        let mut map = Wardley::default();
        let mut lw = Lowering {
            diagnostics: self.diagnostics.clone(),
        };
        for el in root.children_with_tokens() {
            if el.kind() == FRONTMATTER
                && let Some(tok) = el.as_token()
            {
                map.title = text::frontmatter_title(tok.text());
            }
        }
        // Statements by keyword, in source order.
        let mut by_kw: Vec<(String, SyntaxNode)> = Vec::new();
        for node in root.children() {
            match node.kind() {
                TITLE_STMT => {
                    if let Some(t) = tokens(&node).find(|t| t.kind() == TEXT) {
                        let t = unquote(t.text());
                        map.title = (!t.is_empty()).then_some(t);
                    }
                }
                WARDLEY_STMT | PIPELINE => {
                    if let Some(kw) = tokens(&node).find(|t| t.kind() == WARDLEY_KW) {
                        by_kw.push((kw.text().to_owned(), node));
                    }
                }
                LINK_STMT => by_kw.push(("->".to_owned(), node)),
                _ => {}
            }
        }
        let of = |kw: &str| -> Vec<SyntaxNode> {
            by_kw
                .iter()
                .filter(|(k, _)| k == kw)
                .map(|(_, n)| n.clone())
                .collect()
        };
        for n in &of("size") {
            if let [w, h] = numbers(n, COORDS)[..] {
                if [w, h].iter().all(|v| (MIN_SIZE..=MAX_SIZE).contains(v)) {
                    map.size = Some((w as f32, h as f32));
                } else {
                    lw.diagnostics.push(Diagnostic::warning(
                        n.text_range(),
                        format!(
                            "a map size must be from {MIN_SIZE} to {MAX_SIZE}; using the default"
                        ),
                    ));
                }
            }
        }
        for n in &of("evolution") {
            map.stages.clear();
            map.boundaries.clear();
            for stage in n.children().filter(|c| c.kind() == STAGE) {
                let names: Vec<String> = tokens(&stage)
                    .filter(|t| matches!(t.kind(), IDENT | STRING))
                    .map(|t| unquote(t.text()))
                    .collect();
                map.stages.push(names.join(" / "));
                if let Some(t) = tokens(&stage).find(|t| t.kind() == NUMBER)
                    && let Ok(b) = t.text().parse::<f64>()
                {
                    // A fraction of the axis, after the one before it.
                    let low = map.boundaries.last().copied().unwrap_or(0.0);
                    if (low..=1.0).contains(&b) {
                        map.boundaries.push(b);
                    } else {
                        lw.diagnostics.push(Diagnostic::new(
                            t.text_range(),
                            format!("a stage boundary must be from {low} to 1; got {b}"),
                        ));
                    }
                }
            }
        }
        for kw in ["anchor", "component"] {
            for n in &of(kw) {
                let (Some(id), Some((x, y))) = (name(n), lw.coords(n, kw)) else {
                    continue;
                };
                let decorators: Vec<String> = n
                    .children()
                    .filter(|c| c.kind() == DECORATOR)
                    .filter_map(|c| name(&c))
                    .collect();
                let strategy = decorators.iter().find_map(|d| match d.as_str() {
                    "build" => Some(Strategy::Build),
                    "buy" => Some(Strategy::Buy),
                    "outsource" => Some(Strategy::Outsource),
                    "market" => Some(Strategy::Market),
                    _ => None,
                });
                map.add(Node {
                    label: id.clone(),
                    id,
                    x,
                    y,
                    kind: if kw == "anchor" {
                        NodeKind::Anchor
                    } else {
                        NodeKind::Component
                    },
                    label_offset: offset(n),
                    inertia: decorators.iter().any(|d| d == "inertia"),
                    strategy,
                    pipeline_parent: false,
                });
            }
        }
        for n in &of("note") {
            let Some(text) = tokens(n).find(|t| t.kind() == STRING) else {
                continue;
            };
            if let Some((x, y)) = lw.coords(n, "a note") {
                map.notes.push(Marker {
                    text: unquote(text.text()),
                    x,
                    y,
                });
            }
        }
        for n in &of("pipeline") {
            let Some(parent_name) = name(n) else { continue };
            let Some(parent) = map.nodes.iter().position(|p| p.id == parent_name) else {
                lw.diagnostics.push(Diagnostic::new(
                    n.text_range(),
                    format!("unknown component `{parent_name}`: declare it before its pipeline"),
                ));
                continue;
            };
            map.nodes[parent].pipeline_parent = true;
            let y = map.nodes[parent].y;
            let mut components = Vec::new();
            for c in n.children().filter(|c| c.kind() == WARDLEY_STMT) {
                let Some(label) = name(&c) else { continue };
                let [e] = numbers(&c, COORDS)[..] else {
                    continue;
                };
                let Some(x) = lw.percent(e, c.text_range(), "a pipeline component's evolution")
                else {
                    continue;
                };
                let i = map.add(Node {
                    id: format!("{parent_name}_{label}"),
                    label,
                    x,
                    y,
                    kind: NodeKind::PipelineComponent,
                    label_offset: offset(&c),
                    inertia: false,
                    strategy: None,
                    pipeline_parent: false,
                });
                components.push(i);
            }
            // A pipeline declared again keeps its place, as Mermaid's `Map.set` does.
            let pipeline = Pipeline { parent, components };
            match map
                .pipelines
                .iter_mut()
                .find(|p| p.parent == pipeline.parent)
            {
                Some(old) => *old = pipeline,
                None => map.pipelines.push(pipeline),
            }
        }
        for n in &of("->") {
            self.link(&mut map, &mut lw, n);
        }
        for n in &of("evolve") {
            let Some(id) = name(n) else { continue };
            let Some(target) = tokens(n)
                .find(|t| t.kind() == NUMBER)
                .and_then(|t| t.text().parse::<f64>().ok())
            else {
                continue;
            };
            let Some(node) = map.nodes.iter().position(|p| p.id == id) else {
                lw.diagnostics.push(Diagnostic::warning(
                    n.text_range(),
                    format!("unknown component `{id}`; ignoring the `evolve`"),
                ));
                continue;
            };
            let Some(x) = lw.percent(target, n.text_range(), "an evolve target") else {
                continue;
            };
            let y = map.nodes[node].y;
            match map.trends.iter_mut().find(|t| t.node == node) {
                Some(old) => *old = Trend { node, x, y },
                None => map.trends.push(Trend { node, x, y }),
            }
        }
        if let Some(n) = of("annotations").first() {
            map.annotations_box = lw.coords(n, "the annotations box");
        }
        for n in &of("annotation") {
            let number = tokens(n)
                .find(|t| t.kind() == NUMBER)
                .and_then(|t| t.text().parse().ok());
            let text = tokens(n).find(|t| t.kind() == STRING);
            let (Some(number), Some(text)) = (number, text) else {
                continue;
            };
            if let Some((x, y)) = lw.coords(n, "an annotation") {
                map.annotations.push(Annotation {
                    number,
                    x,
                    y,
                    text: unquote(text.text()),
                });
            }
        }
        for (kw, what) in [
            ("accelerator", "an accelerator"),
            ("deaccelerator", "a deaccelerator"),
        ] {
            for n in &of(kw) {
                let (Some(text), Some((x, y))) = (name(n), lw.coords(n, what)) else {
                    continue;
                };
                let m = Marker { text, x, y };
                if kw == "accelerator" {
                    map.accelerators.push(m);
                } else {
                    map.deaccelerators.push(m);
                }
            }
        }
        log::debug!("lowered a Wardley map");
        (map, lw.diagnostics)
    }

    /// Adds a link statement's link, or warns about an unknown end.
    fn link(&self, map: &mut Wardley, lw: &mut Lowering, n: &SyntaxNode) {
        let names: Vec<String> = tokens(n)
            .filter(|t| matches!(t.kind(), IDENT | STRING))
            .map(|t| unquote(t.text()))
            .collect();
        let [from, to] = &names[..] else { return };
        let ends: Vec<Option<usize>> = [from, to].iter().map(|s| map.resolve(s)).collect();
        let (Some(source), Some(target)) = (ends[0], ends[1]) else {
            let unknown = if ends[0].is_none() { from } else { to };
            lw.diagnostics.push(Diagnostic::warning(
                n.text_range(),
                format!("unknown component `{unknown}`; the link is not drawn"),
            ));
            return;
        };
        let ports: Vec<String> = tokens(n)
            .filter(|t| t.kind() == PORT)
            .map(|t| t.text().to_owned())
            .collect();
        let arrow = tokens(n).find(|t| t.kind() == ARROW);
        // A port (`+>`, `+<`, `+<>`) gives the flow, else a labeled flow arrow.
        let flow_of = |p: &str| {
            if p.ends_with("<>") {
                Some(Flow::Both)
            } else if p.ends_with('<') {
                Some(Flow::Backward)
            } else if p.ends_with('>') {
                Some(Flow::Forward)
            } else {
                None
            }
        };
        let plain: Vec<&String> = ports.iter().filter(|p| !p.starts_with("+'")).collect();
        let labeled = ports.iter().find(|p| p.starts_with("+'"));
        let flow = plain
            .first()
            .and_then(|p| flow_of(p))
            .or_else(|| labeled.and_then(|p| flow_of(p)));
        let flow_label = labeled.map(|p| {
            let body = &p[2..];
            body[..body.find('\'').unwrap_or(body.len())].to_owned()
        });
        let annotation = tokens(n)
            .find(|t| t.kind() == TEXT)
            .map(|t| t.text().trim().to_owned());
        map.links.push(Link {
            source,
            target,
            dashed: arrow.is_some_and(|a| a.text() == "-.->"),
            label: flow_label.or(annotation),
            flow,
        });
    }
}

/// Parses Wardley map source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
