//! PlantUML use case, component, and deployment diagrams (PDSC-*), lowered to the graph model:
//! each element is a node of its keyword's shape, each container a subgraph, and each link an
//! edge.

use std::collections::HashMap;

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line, NoteStmt, keyword, name, stereotype};
use super::link::{self, Link};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Edge, Flowchart, Label, Node, Note, Shape, Style, Subgraph};
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str = "expected a use case, component, or deployment statement \
     such as `actor A`, `(Use)`, `[Component]`, or `A --> B`";

/// Element keywords and their shapes (PDSC-2).
pub(crate) const ELEMENTS: &[(&str, Shape)] = &[
    ("actor", Shape::Actor),
    ("usecase", Shape::UseCase),
    ("component", Shape::Rect),
    ("interface", Shape::Circle),
    ("node", Shape::Rect),
    ("database", Shape::Cylinder),
    ("storage", Shape::Rounded),
    ("cloud", Shape::Cloud),
    ("folder", Shape::Folder),
    ("package", Shape::Folder),
    ("frame", Shape::FramedRect),
    ("file", Shape::Document),
    ("artifact", Shape::Document),
    ("card", Shape::Rect),
    ("agent", Shape::Rect),
    ("rectangle", Shape::Rect),
    ("process", Shape::Rect),
    ("action", Shape::Rect),
    ("label", Shape::Text),
    ("stack", Shape::StackedRect),
    ("collections", Shape::StackedRect),
    ("queue", Shape::HorizontalCylinder),
    ("hexagon", Shape::Hexagon),
    ("person", Shape::Person),
    ("boundary", Shape::Circle),
    ("control", Shape::Circle),
    ("entity", Shape::Circle),
    ("circle", Shape::Circle),
    ("port", Shape::Circle),
    ("portin", Shape::Circle),
    ("portout", Shape::Circle),
];

/// An element's name at the start of `s` (PDSC-3): `:x:` (an actor), `(x)` (a use case),
/// `[x]` (a component), `() x` (an interface), `"x"`, or `x`, each with an optional `as alias`.
/// Returns the shape its brackets give, the id, the label, and the rest of the line.
fn element(s: &str) -> Option<(Option<Shape>, String, String, &str)> {
    let s = s.trim_start();
    if let Some(r) = s.strip_prefix("()") {
        let (id, label, rest) = name(r)?;
        return Some((Some(Shape::Circle), id, label, rest));
    }
    let (shape, close) = match s.as_bytes().first()? {
        b':' => (Shape::Actor, ':'),
        b'(' => (Shape::UseCase, ')'),
        b'[' => (Shape::Rect, ']'),
        _ => {
            let (id, label, rest) = name(s)?;
            return Some((None, id, label, rest));
        }
    };
    let end = s[1..].find(close)? + 1;
    let inner = s[1..end].trim();
    if inner.is_empty() {
        return None;
    }
    // A `/` after the bracket is PlantUML's business variant, drawn alike.
    let rest = s[end + 1..].trim_start();
    let rest = rest.strip_prefix('/').unwrap_or(rest).trim_start();
    let label = lines::text(inner);
    if let Some(after) = keyword(rest, "as") {
        let (alias, rest) = alias(after)?;
        return Some((Some(shape), alias, label, rest));
    }
    Some((Some(shape), inner.to_owned(), label, rest))
}

/// The alias after `as`: a bracketed element's inner text, or a name. Does not recurse, so a
/// chain of `as` stays flat (the rest is left for the caller).
fn alias(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let close = match s.as_bytes().first()? {
        b':' => ':',
        b'(' if !s.starts_with("()") => ')',
        b'[' => ']',
        _ => {
            let r = s.strip_prefix("()").unwrap_or(s);
            let (id, _, rest) = name(r)?;
            return Some((id, rest));
        }
    };
    let end = s[1..].find(close)? + 1;
    let inner = s[1..end].trim();
    if inner.is_empty() {
        return None;
    }
    let rest = s[end + 1..].trim_start();
    let rest = rest.strip_prefix('/').unwrap_or(rest).trim_start();
    Some((inner.to_owned(), rest))
}

/// What follows an element's name: stereotypes, a color, a `[description]`, and a `{`.
#[derive(Default)]
struct Decor {
    stereotypes: Vec<String>,
    fill: Option<String>,
    description: Option<String>,
    open: bool,
}

/// An open `{` block: a container, or `together` (no subgraph).
struct Open {
    subgraph: Option<usize>,
    range: TextRange,
}

struct Lower {
    chart: Flowchart,
    d: Vec<Diagnostic>,
    index: HashMap<String, usize>,
    /// Per node: whether only a link or note named it, so the diagram picks its shape.
    implicit: Vec<bool>,
    containers: HashMap<String, usize>,
    stack: Vec<Open>,
    /// The element or container declared last, which `note left : text` attaches to.
    last: Option<String>,
    /// The actor shape `skinparam actorStyle` picks.
    actor: Shape,
}

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let lines = lines::read(root);
    let use_cases = lines.iter().any(has_use_case);
    let mut l = Lower {
        chart: Flowchart::default(),
        d: Vec::new(),
        index: HashMap::new(),
        implicit: Vec::new(),
        containers: HashMap::new(),
        stack: Vec::new(),
        last: None,
        actor: Shape::Actor,
    };
    let mut cursor = Cursor::new(lines);
    while let Some(line) = cursor.next() {
        l.statement(&line, &mut cursor);
    }
    for open in std::mem::take(&mut l.stack) {
        l.error(open.range, "unclosed `{`: expected `}`");
    }
    // A name only links mention is an actor in a diagram with use cases, and an interface
    // otherwise, as PlantUML draws them (PDSC-5).
    let bare = if use_cases {
        Shape::Actor
    } else {
        Shape::Circle
    };
    // A name only links mention that a container later takes is the container, not a node.
    let implicit = std::mem::take(&mut l.implicit);
    let nodes = std::mem::take(&mut l.chart.nodes);
    for (mut node, implicit) in nodes.into_iter().zip(implicit) {
        if implicit && l.containers.contains_key(&node.id) {
            continue;
        }
        if implicit {
            node.shape = bare;
        }
        if node.shape == Shape::Actor {
            node.shape = l.actor;
        }
        l.chart.nodes.push(node);
    }
    (l.chart, l.d)
}

/// Whether a line declares or links a use case.
fn has_use_case(line: &Line) -> bool {
    let t = line.text.as_str();
    let use_case = |s: &str| s.starts_with('(') && !s.starts_with("()");
    line.word() == "usecase"
        || use_case(t)
        || link::link(t).is_some_and(|l| use_case(l.from) || use_case(l.to))
}

impl Lower {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::new(range, message));
    }

    fn warn(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::warning(range, message));
    }

    /// The innermost open container.
    fn container(&self) -> Option<usize> {
        self.stack.iter().rev().find_map(|o| o.subgraph)
    }

    /// The node `id`, created in the current container if new.
    fn node(&mut self, id: &str, label: String, shape: Shape, range: TextRange) -> usize {
        if let Some(&i) = self.index.get(id) {
            self.chart.nodes[i].mentions.push(range);
            return i;
        }
        let i = self.chart.nodes.len();
        let mut node = Node::new(id, Label::plain(label), shape);
        node.mentions.push(range);
        self.chart.nodes.push(node);
        self.implicit.push(false);
        self.index.insert(id.to_owned(), i);
        if let Some(c) = self.container() {
            self.chart.subgraphs[c].members.push(id.to_owned());
        }
        i
    }

    /// The node or container a link end or note target names, creating a node for a new name.
    fn end(&mut self, raw: &str, range: TextRange) -> Option<String> {
        let (shape, id, label, _) = element(raw)?;
        if !self.index.contains_key(&id) && self.containers.contains_key(&id) {
            return Some(id);
        }
        let new = !self.index.contains_key(&id);
        let i = self.node(&id, label, shape.unwrap_or(Shape::Rect), range);
        if new {
            self.implicit[i] = shape.is_none();
        }
        Some(id)
    }

    fn statement(&mut self, line: &Line, cursor: &mut Cursor) {
        let t = line.text.as_str();
        let w = line.word();
        if t == "}" {
            if self.stack.pop().is_none() {
                self.error(line.range, "`}` without a `{` to close");
            }
            return;
        }
        if w == "skinparam" && self.skinparam(line) {
            return;
        }
        match lines::common(line, cursor, &mut self.d) {
            Common::No => {}
            Common::Done => return,
            Common::Title(title) => {
                self.chart.title = Some(title);
                return;
            }
            Common::Direction(d) => {
                self.chart.direction = d;
                return;
            }
        }
        if w == "note" {
            return self.note(line, cursor);
        }
        if w == "together" && t.ends_with('{') {
            return self.open(None, line.range);
        }
        let link = link::link(t);
        // `Card --> X` links an element named like a keyword. The arrow (or a multiplicity)
        // must follow the word directly: in `node x.y` the `x.` is part of a name.
        let after = t[lines::word(t).len()..].trim_start();
        // `-up->` swaps the ends, so the word may be either.
        let named_like_keyword = link
            .as_ref()
            .is_some_and(|l| [l.from, l.to].contains(&lines::word(t)))
            && after.starts_with(['-', '.', '<', '*', '}', '|', '"', '+', '#']);
        if let Some(&(kw, shape)) = ELEMENTS.iter().find(|(k, _)| *k == w)
            && !named_like_keyword
        {
            let rest = line.after_word();
            let rest = rest.strip_prefix('/').unwrap_or(rest);
            return self.declare(line, rest, kw, Some(shape), cursor);
        }
        if let Some(link) = link {
            return self.link(line, &link);
        }
        if t.starts_with([':', '(', '[']) {
            return self.declare(line, t, "", None, cursor);
        }
        self.error(line.range, STATEMENT);
    }

    /// `skinparam actorStyle awesome` or `hollow`; `false` for the settings that are not drawn.
    fn skinparam(&mut self, line: &Line) -> bool {
        let s = lines::squeeze(line.after_word());
        self.actor = match s.as_str() {
            "actorstyle awesome" => Shape::AwesomeActor,
            "actorstyle hollow" => Shape::HollowActor,
            "actorstyle default" | "actorstyle stickman" => Shape::Actor,
            _ => return false,
        };
        true
    }

    fn open(&mut self, subgraph: Option<usize>, range: TextRange) {
        if self.stack.len() >= MAX_DEPTH {
            return self.error(range, crate::lines::too_deep("containers", MAX_DEPTH));
        }
        self.stack.push(Open { subgraph, range });
    }

    /// An element (PDSC-2, PDSC-3) or a container (PDSC-4): `kw` and its `shape`, or a short
    /// form with no keyword.
    fn declare(
        &mut self,
        line: &Line,
        rest: &str,
        kw: &str,
        shape: Option<Shape>,
        cursor: &mut Cursor,
    ) {
        let Some((short, id, label, rest)) = element(rest) else {
            let what = if kw.is_empty() {
                STATEMENT.to_owned()
            } else {
                format!("expected a name after `{kw}`")
            };
            return self.error(line.range, what);
        };
        let shape = shape.or(short).unwrap_or(Shape::Rect);
        let decor = self.decor(line, rest, cursor);
        let mut title = decor.description.unwrap_or(label);
        if !decor.stereotypes.is_empty() {
            let s: Vec<String> = decor.stereotypes.iter().map(|s| format!("«{s}»")).collect();
            title = format!("{}\n{title}", s.join(" "));
        }
        let fill = decor.fill.map(|value| Style {
            property: "fill".into(),
            value,
        });
        self.last = Some(id.clone());
        if decor.open {
            let i = match self.containers.get(&id) {
                Some(&i) => i,
                None => {
                    let i = self.chart.subgraphs.len();
                    // Drop the membership of a link-only node this container replaces.
                    if self.index.get(&id).is_some_and(|&n| self.implicit[n]) {
                        for s in &mut self.chart.subgraphs {
                            s.members.retain(|m| *m != id);
                        }
                    }
                    self.chart
                        .subgraphs
                        .push(Subgraph::new(&id, Label::plain(title), line.range));
                    if let Some(c) = self.container() {
                        self.chart.subgraphs[c].members.push(id.clone());
                    }
                    self.containers.insert(id, i);
                    i
                }
            };
            self.chart.subgraphs[i].styles.extend(fill);
            return self.open(Some(i), line.range);
        }
        let i = self.node(&id, title.clone(), shape, line.range);
        let node = &mut self.chart.nodes[i];
        node.label = Label::plain(title);
        node.shape = shape;
        node.styles.extend(fill);
        self.implicit[i] = false;
    }

    /// The stereotypes, color, description, and `{` after an element's name.
    fn decor(&mut self, line: &Line, mut rest: &str, cursor: &mut Cursor) -> Decor {
        let mut decor = Decor::default();
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix("<<") {
                let Some(end) = r.find(">>") else { break };
                let s = stereotype(&r[..end]);
                if !s.is_empty() {
                    decor.stereotypes.push(s);
                }
                rest = r[end + 2..].trim_start();
            } else if let Some(r) = rest.strip_prefix("[[") {
                rest = r.find("]]").map_or("", |e| r[e + 2..].trim_start());
            } else if let Some(r) = rest.strip_prefix('[') {
                // A description in place of the name: `[text]`, or lines up to `]`.
                match r.find(']') {
                    Some(end) => {
                        decor.description = Some(lines::text(r[..end].trim()));
                        rest = r[end + 1..].trim_start();
                    }
                    None => {
                        let (body, end) = cursor.until(&["]"]);
                        if end.is_none() {
                            self.error(line.range, "unclosed `[`: expected `]`");
                        }
                        let mut text: Vec<&str> = Vec::new();
                        if !r.trim().is_empty() {
                            text.push(r.trim());
                        }
                        text.extend(body.iter().map(|l| l.text.as_str()));
                        decor.description = Some(lines::text(&text.join("\n")));
                        rest = "";
                    }
                }
            } else if rest.starts_with('#') {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                let end = rest[..end].find('{').unwrap_or(end);
                let spec = &rest[..end];
                let fill = spec.split(';').next().unwrap_or(spec);
                if !fill.starts_with("##") {
                    match lines::color(fill) {
                        Some(c) => decor.fill = Some(c),
                        None => self.warn(
                            line.range_of(spec),
                            format!("unknown color `{fill}`; drawing the default"),
                        ),
                    }
                }
                rest = rest[end..].trim_start();
            } else if rest == "{" {
                decor.open = true;
                break;
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                self.warn(
                    line.range_of(&rest[..end]),
                    format!("`{}` is not drawn; ignoring it", &rest[..end]),
                );
                rest = rest[end..].trim_start();
            }
        }
        decor
    }

    /// A link between elements (PDSC-5).
    fn link(&mut self, line: &Line, link: &Link) {
        for item in &link.unknown {
            self.warn(
                line.range_of(link.arrow),
                format!("`{}`: {item} is not drawn; ignoring it", link.arrow),
            );
        }
        let ends = (
            self.end(link.from, line.range_of(link.from)),
            self.end(link.to, line.range_of(link.to)),
        );
        let (Some(from), Some(to)) = ends else {
            return self.error(line.range, STATEMENT);
        };
        let mut edge = Edge::new(from, to, line.range);
        edge.start = link.start;
        edge.end = link.end;
        edge.stroke = link.stroke;
        edge.length = link.length;
        edge.constraint = link.constraint;
        edge.styles = link.styles.clone();
        // `<<extend>>` is drawn `«extend»`.
        edge.label = link
            .label
            .as_deref()
            .map(|l| Label::plain(l.replace("<<", "«").replace(">>", "»")));
        edge.start_label = link.from_label.as_deref().map(Label::plain);
        edge.end_label = link.to_label.as_deref().map(Label::plain);
        self.chart.edges.push(edge);
    }

    /// Notes (PDSC-6): on an element or container, of their own, and on links.
    fn note(&mut self, line: &Line, cursor: &mut Cursor) {
        let Some(stmt) = lines::note(line, cursor, &mut self.d) else {
            return;
        };
        let (side, target, text, range) = match stmt {
            NoteStmt::Free { id, text, range } => {
                let id = id.unwrap_or_else(|| format!("note:{}", self.chart.nodes.len()));
                let i = self.node(&id, text.clone(), Shape::Note, range);
                self.chart.nodes[i].label = Label::plain(text);
                self.chart.nodes[i].shape = Shape::Note;
                self.implicit[i] = false;
                return;
            }
            NoteStmt::OnLink { text } => {
                if !lines::link_note(&mut self.chart, text) {
                    self.warn(line.range, lines::LINK_NOTE);
                }
                return;
            }
            NoteStmt::Attached {
                side,
                target,
                text,
                range,
            } => (side, target, text, range),
        };
        let target = match target {
            Some(t) => self.end(&t, line.range),
            None => self.last.clone(),
        };
        let Some(target) = target else {
            return self.warn(
                line.range,
                "a note needs `of` an element, or an element before it; ignoring it",
            );
        };
        self.chart.notes.push(Note {
            target,
            side,
            label: Label::plain(text),
            range,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowchart::{Arrow, Stroke};

    fn lower_src(body: &str) -> (Flowchart, Vec<String>) {
        let src = format!("@startuml\n{body}\n@enduml\n");
        let (diagram, d) = super::super::parse(&src).lower();
        (
            diagram.into_graph(),
            d.into_iter().map(|d| d.message).collect(),
        )
    }

    fn shape(g: &Flowchart, id: &str) -> Shape {
        g.node(id).unwrap_or_else(|| panic!("no node {id}")).shape
    }

    #[test]
    fn names() {
        let e = |s| element(s).map(|(sh, id, label, rest)| (sh, id, label, rest.to_owned()));
        assert_eq!(
            e(":Main Admin: as Admin"),
            Some((
                Some(Shape::Actor),
                "Admin".into(),
                "Main Admin".into(),
                "".into()
            ))
        );
        assert_eq!(
            e("(Use the\\napp)/ as (UC)"),
            Some((
                Some(Shape::UseCase),
                "UC".into(),
                "Use the\napp".into(),
                "".into()
            ))
        );
        assert_eq!(
            e("[Web] #pink"),
            Some((
                Some(Shape::Rect),
                "Web".into(),
                "Web".into(),
                "#pink".into()
            ))
        );
        assert_eq!(
            e("() \"Data\" as D"),
            Some((Some(Shape::Circle), "D".into(), "Data".into(), "".into()))
        );
        assert_eq!(
            e("Bob <<Human>>"),
            Some((None, "Bob".into(), "Bob".into(), "<<Human>>".into()))
        );
        assert_eq!(e("()"), None);
        assert_eq!(e("[]"), None);
    }

    #[test]
    fn container_replaces_link_only_node() {
        let (g, d) = lower_src("[Api] --> N\nnode N {\n  [Db]\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("N").is_none(), "loose node N");
        assert_eq!(g.subgraphs.len(), 1);
        assert_eq!(g.subgraphs[0].id, "N");
        assert_eq!(g.subgraphs[0].members, ["Db"]);
        assert_eq!(g.edges[0].to, "N");
        assert!(g.node("Api").is_some());
    }

    #[test]
    fn container_replaces_link_only_node_variants() {
        // Implicit member of an outer container that later becomes a nested container.
        let (g, d) = lower_src("node Outer {\n  [A] --> N\n  node N {\n    [Db]\n  }\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("N").is_none());
        let outer = g.subgraphs.iter().find(|s| s.id == "Outer").unwrap();
        assert_eq!(outer.members, ["A", "N"]);
        let n = g.subgraphs.iter().find(|s| s.id == "N").unwrap();
        assert_eq!(n.members, ["Db"]);
        // Implicit in one container, the container declared at top level later.
        let (g, _) = lower_src("node Outer {\n  [A] --> N\n}\nnode N {\n  [Db]\n}");
        assert!(g.node("N").is_none());
        let outer = g.subgraphs.iter().find(|s| s.id == "Outer").unwrap();
        assert_eq!(outer.members, ["A"]);
        // A link to the container after it is declared adds no node.
        let (g, d) = lower_src("node N {\n  [Db]\n}\n[Api] --> N\nN --> [Api]");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("N").is_none());
        assert_eq!(g.edges.len(), 2);
        assert!(g.edges.iter().all(|e| e.from == "N" || e.to == "N"));
        // Reopening a container reuses it.
        let (g, _) = lower_src("node N {\n  [A]\n}\nnode N {\n  [B]\n}");
        assert_eq!(g.subgraphs.len(), 1);
        assert_eq!(g.subgraphs[0].members, ["A", "B"]);
        // An explicit element is not dropped by a same-id container later? It stays a node.
        let (g, _) = lower_src("[N]\n[A] --> N\nnode N {\n  [Db]\n}");
        assert!(g.node("N").is_some());
    }

    #[test]
    fn alias_forms() {
        let e = |s| element(s).map(|(sh, id, label, rest)| (sh, id, label, rest.to_owned()));
        assert_eq!(
            e("(a) as (b)"),
            Some((Some(Shape::UseCase), "b".into(), "a".into(), "".into()))
        );
        assert_eq!(
            e("() a as b"),
            Some((Some(Shape::Circle), "b".into(), "a".into(), "".into()))
        );
        assert_eq!(
            e("[a] as () b"),
            Some((Some(Shape::Rect), "b".into(), "a".into(), "".into()))
        );
        assert_eq!(e("[a] as"), None);
        assert_eq!(e("[a] as []"), None);
        assert_eq!(e("[a] as [b"), None);
        // `as` with no space is not an alias.
        let (_, id, _, rest) = e("[a] ask").unwrap();
        assert_eq!((id.as_str(), rest.as_str()), ("a", "ask"));
        let (g, d) = lower_src("actor A as \"B\"\n(Use)");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.node("A").unwrap().label.text, "B");
        assert_eq!(shape(&g, "A"), Shape::Actor);
        let (g, d) = lower_src("(a) as (b)\n() c as d\n[x] as y\nb --> d");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.node("b").unwrap().label.text, "a");
        assert_eq!(shape(&g, "d"), Shape::Circle);
        assert_eq!(g.node("y").unwrap().label.text, "x");
        // A chained alias leaves the rest to the decor path as a warning.
        let (_, d) = lower_src("[a] as [b] as [c]");
        assert_eq!(d, ["cannot read `as [c]`"]);
        // `[a] as` alone is a statement error, not a panic.
        let (_, d) = lower_src("[a] as");
        assert_eq!(d, [STATEMENT]);
    }

    #[test]
    fn dotted_name_after_keyword_is_a_declaration() {
        // `x.` reads as an arrow head, but `component x.y.Z` declares an element.
        let (g, d) = lower_src("component x.y.Z\nnode x.n {\n  [Db]\n}\n[Api] --> x.y.Z");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("x.y.Z").is_some() && g.node("component").is_none());
        assert_eq!(g.edges.len(), 1);
    }

    #[test]
    fn alias_on_link_end() {
        let (g, d) = lower_src("component Z\n[a] as [b] --> [c] as [d]\nb --> d");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("b").is_some() && g.node("d").is_some());
        assert!(g.node("a").is_none() && g.node("c").is_none());
        assert_eq!(g.node("b").unwrap().label.text, "a");
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn actor_style_skinparam() {
        for (style, expect) in [
            ("awesome", Shape::AwesomeActor),
            ("hollow", Shape::HollowActor),
            ("default", Shape::Actor),
            ("stickman", Shape::Actor),
        ] {
            let (g, d) = lower_src(&format!("skinparam actorStyle {style}\nactor A\n:B:"));
            assert!(d.is_empty(), "{d:?}");
            assert_eq!(shape(&g, "A"), expect);
            assert_eq!(shape(&g, "B"), expect);
        }
    }

    #[test]
    fn alias_chain_does_not_recurse() {
        let e = |s| element(s).map(|(_, id, label, rest)| (id, label, rest.to_owned()));
        assert_eq!(e("[a] as [b]"), Some(("b".into(), "a".into(), "".into())));
        let chain = format!("[a]{}", " as [a]".repeat(10_000));
        let (id, _, rest) = e(&chain).unwrap();
        assert_eq!(id, "a");
        assert!(rest.starts_with("as"));
        // Through a link end and a declaration.
        let (_, _) = lower_src(&format!("{chain} --> X"));
        let (_, _) = lower_src(&chain);
    }

    #[test]
    fn use_cases() {
        let (g, d) = lower_src(
            "skinparam actorStyle awesome\nleft to right direction\nactor Guest as G\n\
             :Admin: <<Human>>\nusecase (Check out) as CO\n(Browse)\nG --> (Browse)\n\
             G -> CO : pays\nAdmin ..> (Browse) : <<extend>>\nUser --> (Browse)\n\
             rectangle Shop #lightblue {\n  (Pay)\n}\nCO .> (Pay)",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(shape(&g, "G"), Shape::AwesomeActor);
        assert_eq!(g.node("G").unwrap().label.text, "Guest");
        assert_eq!(g.node("Admin").unwrap().label.text, "«Human»\nAdmin");
        assert_eq!(shape(&g, "CO"), Shape::UseCase);
        assert_eq!(g.node("CO").unwrap().label.text, "Check out");
        // A bare name in a diagram with use cases is an actor.
        assert_eq!(shape(&g, "User"), Shape::AwesomeActor);
        assert_eq!(g.subgraphs[0].members, ["Pay"]);
        assert_eq!(g.subgraphs[0].styles[0].value, "#add8e6");
        let e = &g.edges[1];
        assert_eq!((e.from.as_str(), e.to.as_str(), e.length), ("G", "CO", 0));
        assert_eq!(e.label.as_ref().unwrap().text, "pays");
        assert_eq!(g.edges[2].stroke, Stroke::Dotted);
        assert_eq!(g.edges.len(), 5);
    }

    #[test]
    fn components_and_deployments() {
        let (g, d) = lower_src(
            "package \"Front end\" as FE {\n  [Web] as W\n  node Server {\n    [Api]\n  }\n}\n\
             database Db\ncloud Net\ninterface HTTP\nDataAccess - [Api]\nW --> HTTP\n\
             HTTP - [Api]\n[Api] ..> Db : reads\ncomponent C [\n  first\n  second\n]\n\
             artifact A <<jar>>\nqueue Q\nFE --> Net",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.subgraphs[0].title.text, "Front end");
        assert_eq!(g.subgraphs[0].members, ["W", "Server"]);
        assert_eq!(g.subgraphs[1].members, ["Api"]);
        assert_eq!(shape(&g, "Db"), Shape::Cylinder);
        assert_eq!(shape(&g, "Net"), Shape::Cloud);
        assert_eq!(shape(&g, "HTTP"), Shape::Circle);
        // A bare name in a diagram with no use cases is an interface.
        assert_eq!(shape(&g, "DataAccess"), Shape::Circle);
        assert_eq!(shape(&g, "Api"), Shape::Rect);
        assert_eq!(g.node("C").unwrap().label.text, "first\nsecond");
        assert_eq!(g.node("A").unwrap().label.text, "«jar»\nA");
        assert_eq!(shape(&g, "Q"), Shape::HorizontalCylinder);
        let last = g.edges.last().unwrap();
        assert_eq!((last.from.as_str(), last.to.as_str()), ("FE", "Net"));
        assert_eq!(last.end, Some(Arrow::Point));
    }

    #[test]
    fn notes_and_errors() {
        let (g, d) = lower_src(
            "actor A\nnote right of A : hi\n(Use)\nnote left of (Use)\n  two\n  lines\nend note\n\
             package P {\n}\nnote bottom of P : pkg\nnote \"free\" as N\nN .. A\nnote on link : x\n\
             A --> (Use)\nnote left : last",
        );
        assert!(d.is_empty(), "{d:?}");
        let notes: Vec<_> = g
            .notes
            .iter()
            .map(|n| (n.target.as_str(), n.label.text.as_str()))
            .collect();
        assert_eq!(
            notes,
            [
                ("A", "hi"),
                ("Use", "two\nlines"),
                ("P", "pkg"),
                ("P", "last")
            ]
        );
        assert_eq!(shape(&g, "N"), Shape::Note);
        assert_eq!(g.edges[0].label.as_ref().unwrap().text, "x");
        let (_, d) =
            lower_src("actor\n}\nwhat is this\nactor A ?\nA -[wavy]-> B\ncomponent C [\n  open\n");
        assert_eq!(
            d,
            [
                "expected a name after `actor`",
                "`}` without a `{` to close",
                STATEMENT,
                "`?` is not drawn; ignoring it",
                "`-[wavy]->`: style `wavy` is not drawn; ignoring it",
                "unclosed `[`: expected `]`",
            ]
        );
    }
}
