//! PlantUML state diagrams (PSTA-*), lowered to the graph model: states are nodes, composite
//! states subgraphs, and transitions edges.

use std::collections::{HashMap, HashSet};

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line, NoteStmt, id_of, name};
use super::link::{self, Link};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Edge, Flowchart, Label, Node, Note, Row, Shape, Style, Subgraph};
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str =
    "expected a state diagram statement such as `[*] --> A`, `state A`, or `A : description`";

/// The container id of the diagram itself.
const ROOT: &str = "";

/// An open composite state, and the region its statements go in.
struct Scope {
    id: String,
    scope: String,
    range: TextRange,
    /// A composite opened past `MAX_DEPTH`: not drawn, its statements go in the parent.
    skipped: bool,
}

/// The members of a container, in order, with a set for O(1) dedupe.
#[derive(Default)]
struct Members {
    list: Vec<String>,
    set: HashSet<String>,
}

struct Lower {
    chart: Flowchart,
    d: Vec<Diagnostic>,
    index: HashMap<String, usize>,
    /// Ids declared with `state X {`, found before lowering.
    composites: HashSet<String>,
    subgraphs: Vec<Subgraph>,
    subgraph_index: HashMap<String, usize>,
    /// Members of each container, by container id.
    members: HashMap<String, Members>,
    /// The container of each node and subgraph.
    parent: HashMap<String, String>,
    regions: HashSet<String>,
    /// The number of regions of each composite state split by `--`.
    region_count: HashMap<String, usize>,
    /// Description lines of each state, drawn below its name.
    descriptions: HashMap<String, Vec<String>>,
    stack: Vec<Scope>,
    /// The last state declared, which `note left: text` attaches to.
    last: Option<String>,
}

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let lines = lines::read(root);
    let composites = lines
        .iter()
        .filter(|l| l.word() == "state" && l.text.ends_with('{'))
        .filter_map(|l| name(l.after_word()).map(|(id, _, _)| id))
        .collect();
    let mut l = Lower {
        chart: Flowchart::default(),
        d: Vec::new(),
        index: HashMap::new(),
        composites,
        subgraphs: Vec::new(),
        subgraph_index: HashMap::new(),
        members: HashMap::new(),
        parent: HashMap::new(),
        regions: HashSet::new(),
        region_count: HashMap::new(),
        descriptions: HashMap::new(),
        stack: Vec::new(),
        last: None,
    };
    let mut cursor = Cursor::new(lines);
    while let Some(line) = cursor.next() {
        l.statement(&line, &mut cursor);
    }
    for open in std::mem::take(&mut l.stack) {
        l.error(
            open.range,
            format!("unclosed state `{}`: expected `}}`", open.id),
        );
    }
    l.finish()
}

/// The shape and label a `<<stereotype>>` gives a state (PSTA-3), or `None` for one drawn as
/// a plain state.
fn stereotype_shape(s: &str, name: &str) -> Option<(Shape, Label)> {
    Some(match s.to_ascii_lowercase().as_str() {
        "choice" => (Shape::Diamond, Label::default()),
        "fork" | "join" => (Shape::Fork, Label::default()),
        "start" => (Shape::SmallCircle, Label::default()),
        "end" => (Shape::FramedCircle, Label::default()),
        "history" => (Shape::Circle, Label::plain("H")),
        "history*" => (Shape::Circle, Label::plain("H*")),
        "entrypoint" | "exitpoint" => (Shape::Circle, Label::plain(name)),
        _ => return None,
    })
}

impl Lower {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::new(range, message));
    }

    fn warn(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::warning(range, message));
    }

    /// The container new states go in.
    fn scope(&self) -> String {
        self.stack
            .last()
            .map_or_else(|| ROOT.to_owned(), |s| s.scope.clone())
    }

    fn add_member(&mut self, container: &str, id: &str) {
        let m = self.members.entry(container.to_owned()).or_default();
        if m.set.insert(id.to_owned()) {
            m.list.push(id.to_owned());
        }
        self.parent
            .entry(id.to_owned())
            .or_insert_with(|| container.to_owned());
    }

    /// The node `id`, created in `container` if new.
    fn node(&mut self, id: &str, container: &str, shape: Shape, label: Label) -> usize {
        if let Some(&i) = self.index.get(id) {
            return i;
        }
        let i = self.chart.nodes.len();
        self.chart.nodes.push(Node::new(id, label, shape));
        self.index.insert(id.to_owned(), i);
        self.add_member(container, id);
        i
    }

    fn subgraph(&mut self, s: Subgraph, container: &str) {
        let id = s.id.clone();
        self.subgraph_index.insert(id.clone(), self.subgraphs.len());
        self.subgraphs.push(s);
        self.add_member(container, &id);
    }

    /// The id a transition end names, creating its state. `[*]` is the scope's start state as
    /// a source and its end state as a target; `[H]` and `[H*]` its history (PSTA-2).
    fn end(&mut self, raw: &str, target: bool, range: TextRange) -> String {
        let scope = self.scope();
        let raw = raw.trim();
        let (owner, pseudo) = match raw.find('[') {
            Some(at) if raw.ends_with(']') => (&raw[..at], &raw[at..]),
            _ => ("", ""),
        };
        let history = match pseudo.to_ascii_uppercase().as_str() {
            "[H]" => Some("H"),
            "[H*]" => Some("H*"),
            _ => None,
        };
        let id = if raw == "[*]" {
            let (suffix, shape) = if target {
                ("end", Shape::FramedCircle)
            } else {
                ("start", Shape::SmallCircle)
            };
            let id = format!("{scope}:[*]{suffix}");
            self.node(&id, &scope, shape, Label::default());
            id
        } else if let Some(h) = history {
            let container = if owner.is_empty() {
                scope
            } else {
                id_of(owner)
            };
            let id = format!("{container}:[{h}]");
            self.node(&id, &container, Shape::Circle, Label::plain(h));
            id
        } else {
            let id = id_of(raw);
            if !self.composites.contains(&id) {
                self.node(&id, &scope, Shape::Rounded, Label::plain(&id));
            }
            id
        };
        if let Some(&i) = self.index.get(&id) {
            self.chart.nodes[i].mentions.push(range);
        }
        id
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
        if matches!(t, "--" | "||") {
            return self.region(line);
        }
        if w == "hide" && lines::squeeze(line.after_word()) == "empty description" {
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
        if w == "state" {
            return self.declare(line);
        }
        if let Some(link) = link::link(t) {
            return self.transition(line, &link);
        }
        if let Some((target, text)) = t.split_once(':')
            && !target.trim().is_empty()
        {
            let id = self.end(target, false, line.range_of(target.trim()));
            self.describe(&id, text);
            return;
        }
        self.error(line.range, STATEMENT);
    }

    /// `--` or `||` in a composite state starts a concurrent region (PSTA-5).
    fn region(&mut self, line: &Line) {
        let Some(open) = self.stack.last() else {
            return self.warn(
                line.range,
                format!(
                    "`{}` separates regions only inside a composite state",
                    line.text
                ),
            );
        };
        if open.skipped {
            // Already reported as too deep.
            return;
        }
        let (composite, range) = (open.id.clone(), open.range);
        let regions = self.region_count.get(&composite).copied().unwrap_or(0);
        if regions == 0 {
            // The statements so far are the first region.
            let first = format!("{composite}:0");
            let mut region = Subgraph::new(&first, Label::default(), range);
            region.region = true;
            let moved = self.members.remove(&composite).unwrap_or_default();
            for m in &moved.list {
                self.parent.insert(m.clone(), first.clone());
            }
            self.members.insert(first.clone(), moved);
            self.subgraph(region, &composite);
            self.regions.insert(first);
        }
        let n = regions.max(1);
        self.region_count.insert(composite.clone(), n + 1);
        let id = format!("{composite}:{n}");
        self.stack.last_mut().expect("checked above").scope = id.clone();
        let mut region = Subgraph::new(&id, Label::default(), line.range);
        region.region = true;
        self.subgraph(region, &composite);
        self.regions.insert(id);
    }

    /// `state Name [as ...] [<<stereotype>>] [#color] [: description] [{]` (PSTA-1, PSTA-3,
    /// PSTA-4).
    fn declare(&mut self, line: &Line) {
        let Some((id, label, mut rest)) = name(line.after_word()) else {
            return self.error(line.range, "expected a state name after `state`");
        };
        let scope = self.scope();
        self.last = Some(id.clone());
        let composite = line.text.ends_with('{');
        if composite {
            // A composite state opened again adds to it, keeping its place; a new one nests
            // in the current scope, whose real depth counts re-opened ancestors too.
            let new = !self.subgraph_index.contains_key(&id);
            if self.stack.len() >= MAX_DEPTH || (new && self.depth(&scope) >= MAX_DEPTH) {
                // One error per too-deep run, not one per level inside it.
                if !self.stack.last().is_some_and(|s| s.skipped) {
                    self.error(line.range, crate::lines::too_deep("states", MAX_DEPTH));
                }
                // Pushed so its `}` closes it and not the parent.
                self.stack.push(Scope {
                    id,
                    scope,
                    range: line.range,
                    skipped: true,
                });
                return;
            }
            if new {
                self.subgraph(Subgraph::new(&id, Label::plain(&label), line.range), &scope);
            }
            // Statements resume in the last region of a composite already split.
            let inner = match self.region_count.get(&id) {
                Some(&n) if n > 0 => format!("{id}:{}", n - 1),
                _ => id.clone(),
            };
            self.stack.push(Scope {
                id: id.clone(),
                scope: inner,
                range: line.range,
                skipped: false,
            });
        } else if !self.composites.contains(&id) {
            let i = self.node(&id, &scope, Shape::Rounded, Label::plain(&label));
            self.chart.nodes[i].label = Label::plain(&label);
            self.chart.nodes[i].mentions.push(line.range);
        }
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix("<<") {
                let Some(end) = r.find(">>") else { break };
                let s = lines::stereotype(&r[..end]);
                match stereotype_shape(&s, &label) {
                    Some((shape, l)) if !composite => {
                        if let Some(&i) = self.index.get(&id) {
                            self.chart.nodes[i].shape = shape;
                            self.chart.nodes[i].label = l;
                        }
                    }
                    _ => self.warn(
                        line.range_of(&rest[..end + 4]),
                        format!("`<<{s}>>` states are not supported; drawing a plain state"),
                    ),
                }
                rest = r[end + 2..].trim_start();
            } else if let Some(r) = rest.strip_prefix("[[") {
                rest = r.find("]]").map_or("", |e| r[e + 2..].trim_start());
            } else if rest.starts_with('#') {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                let end = rest[..end].find(['{', ':']).unwrap_or(end);
                let spec = &rest[..end];
                let fill = spec.split(';').next().unwrap_or(spec);
                if fill.starts_with("##") {
                    self.warn(
                        line.range_of(spec),
                        format!("`{spec}` borders are not drawn; ignoring it"),
                    );
                } else {
                    match lines::color(fill) {
                        Some(c) => self.fill(&id, c),
                        None => self.warn(
                            line.range_of(spec),
                            format!("unknown color `{fill}`; drawing the default"),
                        ),
                    }
                }
                rest = rest[end..].trim_start();
            } else if let Some(text) = rest.strip_prefix(':') {
                self.describe(&id, text);
                break;
            } else if rest == "{" {
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
    }

    fn fill(&mut self, id: &str, color: String) {
        let style = Style {
            property: "fill".into(),
            value: color,
        };
        if let Some(&i) = self.index.get(id) {
            self.chart.nodes[i].styles.push(style);
        } else if let Some(&i) = self.subgraph_index.get(id) {
            self.subgraphs[i].styles.push(style);
        }
    }

    /// Adds a description line below a state's name, or to a composite state's title
    /// (PSTA-4).
    fn describe(&mut self, id: &str, text: &str) {
        let text = lines::text(text.trim());
        if text.is_empty() {
            return;
        }
        // Applied in `finish`, as a composite state may be opened after its description.
        self.descriptions
            .entry(id.to_owned())
            .or_default()
            .extend(text.lines().map(str::to_owned));
    }

    /// A transition (PSTA-2).
    fn transition(&mut self, line: &Line, link: &Link) {
        for item in &link.unknown {
            self.warn(
                line.range_of(link.arrow),
                format!("`{}`: {item} is not drawn; ignoring it", link.arrow),
            );
        }
        let from = self.end(link.from, false, line.range_of(link.from));
        let to = self.end(link.to, true, line.range_of(link.to));
        let mut edge = Edge::new(from, to, line.range);
        edge.start = link.start;
        edge.end = link.end;
        edge.stroke = link.stroke;
        edge.length = link.length;
        edge.constraint = link.constraint;
        edge.styles = link.styles.clone();
        edge.label = link.label.as_deref().map(Label::plain);
        self.chart.edges.push(edge);
    }

    /// Notes (PSTA-6): `note left of S : text` or a block to `end note`, `note left : text`
    /// after a declaration, `note "text" as N`, and `note on link`.
    fn note(&mut self, line: &Line, cursor: &mut Cursor) {
        let Some(stmt) = lines::note(line, cursor, &mut self.d) else {
            return;
        };
        let (side, target, text, range) = match stmt {
            NoteStmt::Free { id, text, range } => {
                let id = id.unwrap_or_else(|| format!("note:{}", self.chart.nodes.len()));
                let scope = self.scope();
                let i = self.node(&id, &scope, Shape::Note, Label::default());
                self.chart.nodes[i].label = Label::plain(text);
                self.chart.nodes[i].shape = Shape::Note;
                self.chart.nodes[i].mentions.push(range);
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
        let target = target.and_then(|t| name(&t).map(|n| n.0));
        let target = match target.or_else(|| self.last.clone()) {
            Some(t) => t,
            None => {
                return self.warn(
                    line.range,
                    "a note needs `of` a state, or a state before it; ignoring it",
                );
            }
        };
        if !self.composites.contains(&target) {
            let scope = self.scope();
            self.node(&target, &scope, Shape::Rounded, Label::plain(&target));
        }
        self.chart.notes.push(Note {
            target,
            side,
            label: Label::plain(text),
            range,
        });
    }

    /// The number of composite states (not regions) from `container` out to the diagram.
    fn depth(&self, container: &str) -> usize {
        let mut depth = 0;
        let mut seen = HashSet::new();
        let mut at = Some(container);
        while let Some(c) = at {
            if c == ROOT || !seen.insert(c) {
                break;
            }
            if !self.regions.contains(c) {
                depth += 1;
            }
            at = self.parent.get(c).map(String::as_str);
        }
        depth
    }

    /// The containers holding `id`, outermost first.
    fn chain(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut at = self.parent.get(id);
        while let Some(c) = at {
            if c == ROOT || !seen.insert(c.as_str()) {
                break;
            }
            out.push(c.clone());
            at = self.parent.get(c);
        }
        out.reverse();
        out
    }

    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        for (id, rows) in std::mem::take(&mut self.descriptions) {
            if let Some(&i) = self.index.get(&id) {
                self.chart.nodes[i].sections = vec![
                    rows.into_iter()
                        .map(|r| Row {
                            cells: vec![r],
                            ..Row::default()
                        })
                        .collect(),
                ];
            } else if let Some(&i) = self.subgraph_index.get(&id) {
                let title = &mut self.subgraphs[i].title.text;
                for r in rows {
                    title.push('\n');
                    title.push_str(&r);
                }
            }
        }
        // PSTA-5: a transition between two regions of one composite state is not drawn.
        let edges = std::mem::take(&mut self.chart.edges);
        for e in edges {
            let (a, b) = (self.chain(&e.from), self.chain(&e.to));
            let split = a
                .iter()
                .zip(&b)
                .find(|(x, y)| x != y)
                .is_some_and(|(x, y)| self.regions.contains(x) && self.regions.contains(y));
            if split {
                self.d.push(Diagnostic::warning(
                    e.range,
                    format!(
                        "transitions between concurrent regions are not supported; ignoring `{} --> {}`",
                        e.from, e.to
                    ),
                ));
            } else {
                self.chart.edges.push(e);
            }
        }
        // A composite state named only in transitions, never opened, is an empty box.
        for s in &mut self.subgraphs {
            s.members = self.members.remove(&s.id).unwrap_or_default().list;
        }
        self.chart.subgraphs = self.subgraphs;
        (self.chart, self.d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_src(body: &str) -> (Flowchart, Vec<String>) {
        let src = format!("@startuml\n{body}\n@enduml\n");
        let (diagram, d) = super::super::parse(&src).lower();
        (
            diagram.into_graph(),
            d.into_iter().map(|d| d.message).collect(),
        )
    }

    fn edges(g: &Flowchart) -> Vec<String> {
        g.edges
            .iter()
            .map(|e| match &e.label {
                Some(l) => format!("{}>{}:{}", e.from, e.to, l.text),
                None => format!("{}>{}", e.from, e.to),
            })
            .collect()
    }

    #[test]
    fn states_and_transitions() {
        let (g, d) = lower_src(
            "[*] --> Idle\nstate \"Waiting for input\" as Idle #pink\nIdle : entry / reset\n\
             Idle -> Busy : start\nstate Busy : working\nBusy --> [*]\nstate c <<choice>>\n\
             Busy --> c\nc -up-> Idle",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            edges(&g),
            [
                ":[*]start>Idle",
                "Idle>Busy:start",
                "Busy>:[*]end",
                "Busy>c",
                "Idle>c"
            ]
        );
        let idle = g.node("Idle").unwrap();
        assert_eq!(idle.label.text, "Waiting for input");
        assert_eq!(idle.sections[0][0].cells, ["entry / reset"]);
        assert_eq!(idle.styles[0].value, "#ffc0cb");
        assert_eq!(g.edges[1].length, 0);
        assert_eq!(g.node("c").unwrap().shape, Shape::Diamond);
        assert_eq!(g.node(":[*]start").unwrap().shape, Shape::SmallCircle);
    }

    #[test]
    fn composites_and_regions() {
        let (g, d) = lower_src(
            "[*] --> Active\nstate Active {\n  [*] --> On\n  On --> Off\n  --\n  [*] --> Up\n}\n\
             Active --> Done\nstate Done {\n  state Inner {\n    X --> [H]\n  }\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        let s = |id: &str| g.subgraphs.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s("Active").members, ["Active:0", "Active:1"]);
        assert!(s("Active:0").region);
        assert_eq!(s("Active:0").members, ["Active:[*]start", "On", "Off"]);
        assert_eq!(s("Active:1").members, ["Active:1:[*]start", "Up"]);
        assert_eq!(s("Done").members, ["Inner"]);
        assert_eq!(s("Inner").members, ["X", "Inner:[H]"]);
        assert!(g.edges.iter().any(|e| e.from == "Active" && e.to == "Done"));
        let (_, d) = lower_src("state A {\n  X --> Y\n  --\n  state Z\n  Z --> X\n}\n--");
        assert_eq!(
            d,
            [
                "transitions between concurrent regions are not supported; ignoring `Z --> X`",
                "`--` separates regions only inside a composite state",
            ]
        );
    }

    #[test]
    fn notes_and_errors() {
        let (g, d) = lower_src(
            "state A\nnote left of A : n\nnote right : after\nA --> B\nnote on link : go\n\
             note \"free\" as N\nN .. B",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.notes.len(), 2);
        assert_eq!(g.edges[0].label.as_ref().unwrap().text, "go");
        assert_eq!(g.node("N").unwrap().shape, Shape::Note);
        let (_, d) = lower_src("state A {\n}\n}\nwhat\nstate B <<sdlreceive>>\nstate C {");
        assert_eq!(
            d,
            [
                "`}` without a `{` to close",
                STATEMENT,
                "`<<sdlreceive>>` states are not supported; drawing a plain state",
                "unclosed state `C`: expected `}`",
            ]
        );
    }

    #[test]
    fn reopened_composite_resumes_regions() {
        let (g, d) = lower_src(
            "state X {\n  A --> B\n  --\n  C --> D\n}\nstate X {\n  E --> F\n  --\n  G --> H\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        let ids: Vec<_> = g.subgraphs.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["X", "X:0", "X:1", "X:2"]);
        let s = |id: &str| g.subgraphs.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s("X").members, ["X:0", "X:1", "X:2"]);
        assert_eq!(s("X:0").members, ["A", "B"]);
        assert_eq!(s("X:1").members, ["C", "D", "E", "F"]);
        assert_eq!(s("X:2").members, ["G", "H"]);
    }

    #[test]
    fn description_before_composite_is_its_title() {
        let (g, d) = lower_src(
            "X : described before\nstate Y : also before\n[*] --> X\nstate X {\n  A --> B\n}\n\
             X : after\nstate Y {\n  C --> D\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        let s = |id: &str| g.subgraphs.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s("X").title.text, "X\ndescribed before\nafter");
        assert_eq!(s("Y").title.text, "Y\nalso before");
        assert!(g.node("X").is_none() && g.node("Y").is_none());
    }

    #[test]
    fn too_deep_composite_closes_itself() {
        let n = MAX_DEPTH + 2;
        let mut body = String::new();
        for i in 0..n {
            body.push_str(&format!("state s{i} {{\n"));
        }
        body.push_str("--\n");
        for _ in 0..n {
            body.push_str("}\n");
        }
        body.push_str("A --> B");
        let (g, d) = lower_src(&body);
        let deep = crate::lines::too_deep("states", MAX_DEPTH);
        assert_eq!(d, [deep]);
        assert_eq!(g.subgraphs.len(), MAX_DEPTH);
        assert!(g.subgraphs.iter().all(|s| !s.region));
        assert!(
            g.subgraphs
                .iter()
                .all(|s| !s.members.iter().any(|m| m == "A"))
        );
    }

    #[test]
    fn reopened_chain_counts_real_depth() {
        // Each `state sK {` is re-opened at the top level and nests the next one: the stack
        // stays at two, but the composites chain without bound.
        let mut body = String::new();
        for i in 0..MAX_DEPTH + 5 {
            body.push_str(&format!("state s{i} {{\nstate s{} {{\n}}\n}}\n", i + 1));
        }
        let (g, d) = lower_src(&body);
        assert_eq!(d, [crate::lines::too_deep("states", MAX_DEPTH)]);
        let parent: HashMap<&str, &str> = g
            .subgraphs
            .iter()
            .flat_map(|s| s.members.iter().map(|m| (m.as_str(), s.id.as_str())))
            .collect();
        let depth = |id: &str| {
            let (mut n, mut at) = (1, id);
            while let Some(p) = parent.get(at) {
                n += 1;
                at = p;
            }
            n
        };
        let deepest = g.subgraphs.iter().map(|s| depth(&s.id)).max();
        assert_eq!(deepest, Some(MAX_DEPTH));
    }

    fn sub<'a>(g: &'a Flowchart, id: &str) -> &'a Subgraph {
        g.subgraphs.iter().find(|s| s.id == id).unwrap()
    }

    #[test]
    fn reopened_unsplit_composite_splits_with_earlier_members() {
        let (g, d) = lower_src("state X {\n  A --> B\n}\nstate X {\n  C --> D\n  --\n  E --> F\n}");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(sub(&g, "X").members, ["X:0", "X:1"]);
        assert_eq!(sub(&g, "X:0").members, ["A", "B", "C", "D"]);
        assert_eq!(sub(&g, "X:1").members, ["E", "F"]);
    }

    #[test]
    fn reopened_composite_in_deeper_scope_keeps_place() {
        let (g, d) = lower_src(
            "state P {\n  state X {\n    A --> B\n    --\n    C --> D\n  }\n}\n\
             state P {\n  state X {\n    E --> F\n  }\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(sub(&g, "P").members, ["X"]);
        assert_eq!(sub(&g, "X").members, ["X:0", "X:1"]);
        assert_eq!(sub(&g, "X:1").members, ["C", "D", "E", "F"]);
        assert_eq!(g.subgraphs.len(), 4);
    }

    #[test]
    fn start_in_resumed_region_is_shared() {
        let (g, d) = lower_src(
            "state X {\n  A --> B\n  --\n  [*] --> C\n}\nstate X {\n  [*] --> D\n  D --> [*]\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            sub(&g, "X:1").members,
            ["X:1:[*]start", "C", "D", "X:1:[*]end"]
        );
        assert!(g.node("X:[*]start").is_none());
        let from_start = edges(&g)
            .iter()
            .filter(|e| e.starts_with("X:1:[*]start>"))
            .count();
        assert_eq!(from_start, 2);
    }

    fn too_deep_body(extra: &str) -> String {
        let mut body = String::new();
        for i in 0..=MAX_DEPTH {
            body.push_str(&format!("state s{i} {{\n"));
        }
        body.push_str(extra);
        body
    }

    #[test]
    fn description_of_skipped_composite_is_dropped() {
        let last = format!("s{MAX_DEPTH}");
        let mut body = too_deep_body(&format!("{last} : text\nA --> B\n"));
        for _ in 0..=MAX_DEPTH {
            body.push_str("}\n");
        }
        let (g, d) = lower_src(&body);
        assert_eq!(d, [crate::lines::too_deep("states", MAX_DEPTH)]);
        assert!(g.node(&last).is_none());
        assert!(g.subgraphs.iter().all(|s| s.id != last));
        // The placeholder's statements land in its parent.
        let parent = format!("s{}", MAX_DEPTH - 1);
        assert_eq!(sub(&g, &parent).members, ["A", "B"]);
    }

    #[test]
    fn unclosed_placeholder_reports_both() {
        let (g, d) = lower_src(&too_deep_body("A --> B\n"));
        let unclosed: Vec<_> = d
            .iter()
            .filter(|m| m.starts_with("unclosed state"))
            .collect();
        assert_eq!(unclosed.len(), MAX_DEPTH + 1);
        assert_eq!(
            unclosed[MAX_DEPTH],
            &format!("unclosed state `s{MAX_DEPTH}`: expected `}}`")
        );
        assert_eq!(d.len(), MAX_DEPTH + 2);
        assert!(d.contains(&crate::lines::too_deep("states", MAX_DEPTH)));
        assert_eq!(g.subgraphs.len(), MAX_DEPTH);
    }

    #[test]
    fn placeholder_on_stack_closes_in_order() {
        // After the placeholder's `}`, statements return to the real parent.
        let mut body = too_deep_body("}\nA --> B\n");
        for _ in 0..MAX_DEPTH {
            body.push_str("}\n");
        }
        let (g, d) = lower_src(&body);
        assert_eq!(d, [crate::lines::too_deep("states", MAX_DEPTH)]);
        let parent = format!("s{}", MAX_DEPTH - 1);
        assert_eq!(sub(&g, &parent).members, ["A", "B"]);
    }

    #[test]
    fn long_chain_reports_too_deep_once() {
        let mut body = String::new();
        for i in 0..10_000 {
            body.push_str(&format!("state s{i} {{\n"));
        }
        let (g, d) = lower_src(&body);
        let deep = crate::lines::too_deep("states", MAX_DEPTH);
        assert_eq!(d.iter().filter(|m| **m == deep).count(), 1);
        assert_eq!(g.subgraphs.len(), MAX_DEPTH);
    }

    #[test]
    fn members_are_not_repeated() {
        let (g, _) = lower_src("state P {\n  state X {\n  }\n  note \"n\" as X\n  X --> X\n}");
        let p = g.subgraphs.iter().find(|s| s.id == "P").unwrap();
        assert_eq!(p.members, ["X"]);
    }
}
