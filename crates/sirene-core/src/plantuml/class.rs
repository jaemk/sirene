//! PlantUML class and object diagrams (PCLS-*), lowered to the graph model: each class is a
//! node with its members in compartments, and each link an edge.

use std::collections::HashMap;

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line, NoteStmt, id_of, keyword, name, stereotype};
use super::link::{self, Link};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{
    Arrow, Edge, Flowchart, Label, Node, Note, Row, Shape, Stroke, Style, Subgraph,
};
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str =
    "expected a class diagram statement such as `class A`, `A <|-- B`, or `A : member`";

/// Declaration keywords and the stereotype each shows above the name (PCLS-2).
const KINDS: &[(&str, Option<&str>)] = &[
    ("class", None),
    ("interface", Some("interface")),
    ("enum", Some("enumeration")),
    ("annotation", Some("annotation")),
    ("entity", Some("entity")),
    ("exception", Some("exception")),
    ("metaclass", Some("metaclass")),
    ("protocol", Some("protocol")),
    ("struct", Some("struct")),
    ("stereotype", Some("stereotype")),
    ("dataclass", Some("dataclass")),
    ("record", Some("record")),
    ("object", None),
    ("map", None),
    ("json", None),
    ("circle", None),
    ("diamond", None),
];

/// Grouping keywords that take a `{` block of classes (PCLS-6).
const PACKAGES: &[&str] = &[
    "package",
    "namespace",
    "rectangle",
    "folder",
    "frame",
    "cloud",
    "node",
    "database",
    "card",
];

#[derive(Default)]
struct Class {
    stereotypes: Vec<String>,
    fields: Vec<Row>,
    methods: Vec<Row>,
    /// The compartments as written, when the body has separators (`--`, `..`, `==`, `__`).
    sections: Option<Vec<Vec<Row>>>,
    /// A circle, diamond, or note: drawn without compartments.
    plain: bool,
    /// An object or map: one compartment of fields.
    fields_only: bool,
}

/// What `hide` turned off (PCLS-7).
#[derive(Default, Clone, Copy)]
struct Hide {
    empty_fields: bool,
    empty_methods: bool,
    fields: bool,
    methods: bool,
    stereotypes: bool,
}

/// An open `{` block: a package, or `together` (no subgraph).
struct Open {
    subgraph: Option<usize>,
    range: TextRange,
}

struct Lower {
    chart: Flowchart,
    d: Vec<Diagnostic>,
    index: HashMap<String, usize>,
    classes: Vec<Class>,
    packages: HashMap<String, usize>,
    stack: Vec<Open>,
    /// The last class declared, which `note left: text` attaches to.
    last: Option<String>,
    hide: Hide,
    /// Nodes created only by a link end, so a later package of that name can replace them.
    implicit: std::collections::HashSet<String>,
}

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let mut l = Lower {
        chart: Flowchart::default(),
        d: Vec::new(),
        index: HashMap::new(),
        classes: Vec::new(),
        packages: HashMap::new(),
        stack: Vec::new(),
        last: None,
        hide: Hide::default(),
        implicit: Default::default(),
    };
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        l.statement(&line, &mut cursor);
    }
    for open in std::mem::take(&mut l.stack) {
        l.error(open.range, "unclosed `{`: expected `}`");
    }
    l.finish()
}

/// A member line: whether it is a method, and its row (PCLS-3).
fn member(raw: &str) -> (bool, Row) {
    let mut t = raw.trim().to_owned();
    let (mut underline, mut italic, mut kind) = (false, false, None);
    for (m, f) in [
        ("{static}", 0),
        ("{classifier}", 0),
        ("{abstract}", 1),
        ("{field}", 2),
        ("{method}", 3),
    ] {
        let lower = t.to_ascii_lowercase();
        if let Some(at) = lower.find(m) {
            t.replace_range(at..at + m.len(), "");
            match f {
                0 => underline = true,
                1 => italic = true,
                2 => kind = Some(false),
                _ => kind = Some(true),
            }
        }
    }
    let mut t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    // `+ make()` after a removed modifier is `+make()`.
    if t.starts_with(['+', '-', '#', '~']) && t[1..].starts_with(' ') {
        t.remove(1);
    }
    let method = kind.unwrap_or_else(|| t.contains('('));
    (
        method,
        Row {
            cells: vec![t],
            italic,
            underline,
        },
    )
}

/// A compartment separator (`--`, `..`, `==`, `__`), with its title if it has one.
fn separator(t: &str) -> Option<Option<String>> {
    let c = t.chars().next()?;
    if !matches!(c, '-' | '.' | '=' | '_')
        || !t.starts_with([c, c].iter().collect::<String>().as_str())
    {
        return None;
    }
    let inner = t.trim_matches(c).trim();
    if inner.is_empty() {
        return Some(None);
    }
    t.ends_with([c, c].iter().collect::<String>().as_str())
        .then(|| Some(inner.to_owned()))
}

impl Lower {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::new(range, message));
    }

    fn warn(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::warning(range, message));
    }

    /// The innermost open package.
    fn package(&self) -> Option<usize> {
        self.stack.iter().rev().find_map(|o| o.subgraph)
    }

    /// The node for class `id`, created in the current package if new.
    fn class(&mut self, id: &str, range: TextRange) -> usize {
        if let Some(&i) = self.index.get(id) {
            self.chart.nodes[i].mentions.push(range);
            return i;
        }
        let i = self.chart.nodes.len();
        let mut node = Node::new(id, Label::plain(id), Shape::Rect);
        node.mentions.push(range);
        self.chart.nodes.push(node);
        self.classes.push(Class::default());
        self.index.insert(id.to_owned(), i);
        if let Some(p) = self.package() {
            self.chart.subgraphs[p].members.push(id.to_owned());
        }
        i
    }

    /// The node or package a link end names, creating a class for a new name.
    fn end(&mut self, raw: &str, range: TextRange) -> String {
        let id = id_of(raw);
        if !self.index.contains_key(&id) && self.packages.contains_key(&id) {
            return id;
        }
        if !self.index.contains_key(&id) {
            self.implicit.insert(id.clone());
        }
        self.class(&id, range);
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
        if matches!(w.as_str(), "hide" | "show") && self.hide(line) {
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
        let link = link::link(t);
        // `Entity --> X` links a class named like a keyword. The arrow (or a multiplicity)
        // must follow the word directly: in `class x.y.Z` the `x.` is part of a name.
        let after = t[lines::word(t).len()..].trim_start();
        // `-up->` swaps the ends, so the word may be either.
        let named_like_keyword = link
            .as_ref()
            .is_some_and(|l| [l.from, l.to].contains(&lines::word(t)))
            && after.starts_with(['-', '.', '<', '*', '}', '|', '"', '+', '#']);
        if w == "note" && !named_like_keyword {
            return self.note(line, cursor);
        }
        if w == "together" && t.ends_with('{') {
            return self.open(None, line.range);
        }
        if !named_like_keyword
            && PACKAGES.contains(&w.as_str())
            && (t.ends_with('{') || w == "package" || w == "namespace")
        {
            return self.package_decl(line);
        }
        if !named_like_keyword && let Some(rest) = keyword(t, "abstract") {
            let rest = keyword(rest, "class").unwrap_or(rest);
            return self.declare(line, rest, "class", Some("abstract"), cursor);
        }
        if !named_like_keyword && let Some(&(kw, stereo)) = KINDS.iter().find(|(k, _)| *k == w) {
            return self.declare(line, line.after_word(), kw, stereo, cursor);
        }
        if let Some(link) = link {
            return self.link(line, &link);
        }
        // The member colon is the first `:` that is not half of a `::` pair.
        let colon = {
            let b = t.as_bytes();
            let mut i = 0;
            let mut found = None;
            while i < b.len() {
                if b[i] == b':' {
                    if b.get(i + 1) == Some(&b':') {
                        i += 2;
                        continue;
                    }
                    found = Some(i);
                    break;
                }
                i += 1;
            }
            found
        };
        if let Some(c) = colon
            && let (target, member) = (&t[..c], &t[c + 1..])
            && !target.trim().is_empty()
        {
            let i = self.class(&id_of(target), line.range_of(target.trim()));
            return self.add_member(i, member);
        }
        self.error(line.range, STATEMENT);
    }

    /// `hide`/`show` of members, empty members, circles, and stereotypes; `false` for the
    /// forms the shared settings warn about.
    fn hide(&mut self, line: &Line) -> bool {
        let on = line.word() == "hide";
        let what = lines::squeeze(line.after_word());
        let h = &mut self.hide;
        match what.as_str() {
            "empty members" => (h.empty_fields, h.empty_methods) = (on, on),
            "empty fields" | "empty attributes" => h.empty_fields = on,
            "empty methods" => h.empty_methods = on,
            "members" => (h.fields, h.methods) = (on, on),
            "fields" | "attributes" => h.fields = on,
            "methods" => h.methods = on,
            "stereotype" | "stereotypes" => h.stereotypes = on,
            // Spots are not drawn.
            "circle" | "spot" | "class circle" | "class spot" => {}
            _ => return false,
        }
        true
    }

    fn open(&mut self, subgraph: Option<usize>, range: TextRange) {
        if self.stack.len() >= MAX_DEPTH {
            return self.error(range, crate::lines::too_deep("packages", MAX_DEPTH));
        }
        self.stack.push(Open { subgraph, range });
    }

    /// `package name [<<style>>] [#color] {` (PCLS-6).
    fn package_decl(&mut self, line: &Line) {
        let body = line.after_word().trim_end_matches('{').trim_end();
        let Some((id, label, rest)) = name(body) else {
            return self.error(line.range, "expected a package name");
        };
        let i = match self.packages.get(&id) {
            Some(&i) => i,
            None => {
                if self.implicit.contains(&id) {
                    // The stray node goes in `finish`; drop its membership now, before the
                    // package adds an entry under the same id.
                    for sg in &mut self.chart.subgraphs {
                        sg.members.retain(|m| *m != id);
                    }
                }
                let i = self.chart.subgraphs.len();
                self.chart
                    .subgraphs
                    .push(Subgraph::new(&id, Label::plain(label), line.range));
                if let Some(p) = self.package() {
                    self.chart.subgraphs[p].members.push(id.clone());
                }
                self.packages.insert(id, i);
                i
            }
        };
        for part in rest.split_whitespace() {
            if part.starts_with('#') {
                match lines::color(part) {
                    Some(c) => self.chart.subgraphs[i].styles.push(Style {
                        property: "fill".into(),
                        value: c,
                    }),
                    None => self.warn(
                        line.range_of(part),
                        format!("unknown color `{part}`; drawing the default"),
                    ),
                }
            }
        }
        if line.text.ends_with('{') {
            self.open(Some(i), line.range);
        }
    }

    /// A class declaration (PCLS-2): its name, stereotypes, color, `extends` and `implements`
    /// links, and a `{` body of members.
    fn declare(
        &mut self,
        line: &Line,
        rest: &str,
        kw: &str,
        stereo: Option<&str>,
        cursor: &mut Cursor,
    ) {
        let Some((id, label, mut rest)) = name(rest) else {
            return self.error(line.range, format!("expected a name after `{kw}`"));
        };
        let i = self.class(&id, line.range);
        self.implicit.remove(&id);
        self.chart.nodes[i].label = Label::plain(label);
        self.last = Some(id.clone());
        if let Some(s) = stereo
            && !self.classes[i].stereotypes.iter().any(|x| x == s)
        {
            self.classes[i].stereotypes.insert(0, s.to_owned());
        }
        match kw {
            "object" | "map" => self.classes[i].fields_only = true,
            "circle" => {
                self.chart.nodes[i].shape = Shape::Circle;
                self.classes[i].plain = true;
            }
            "diamond" => {
                self.chart.nodes[i].shape = Shape::Diamond;
                self.chart.nodes[i].label = Label::default();
                self.classes[i].plain = true;
            }
            _ => {}
        }
        let mut body = false;
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix("<<") {
                let Some(end) = r.find(">>") else { break };
                let s = stereotype(&r[..end]);
                if !s.is_empty() {
                    self.classes[i].stereotypes.push(s);
                }
                rest = r[end + 2..].trim_start();
            } else if let Some(r) = rest.strip_prefix("[[") {
                rest = r.find("]]").map_or("", |e| r[e + 2..].trim_start());
            } else if rest.starts_with('#') {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                let end = rest[..end].find('{').unwrap_or(end);
                let spec = &rest[..end];
                let fill = spec.split(';').next().unwrap_or(spec);
                if !fill.starts_with("##") {
                    match lines::color(fill) {
                        Some(c) => self.chart.nodes[i].styles.push(Style {
                            property: "fill".into(),
                            value: c,
                        }),
                        None => self.warn(
                            line.range_of(spec),
                            format!("unknown color `{fill}`; drawing the default"),
                        ),
                    }
                }
                rest = rest[end..].trim_start();
            } else if let Some(r) = keyword(rest, "extends").or_else(|| keyword(rest, "implements"))
            {
                let dotted = keyword(rest, "implements").is_some();
                let end = r.find('{').unwrap_or(r.len());
                let end = r[..end].find(" implements").unwrap_or(end);
                for parent in r[..end].split(',') {
                    let parent = parent.trim();
                    if parent.is_empty() {
                        continue;
                    }
                    let range = line.range_of(parent);
                    let to = self.end(parent, range);
                    let mut edge = Edge::new(to, &id, line.range);
                    edge.start = Some(Arrow::Triangle);
                    edge.end = None;
                    if dotted {
                        edge.stroke = Stroke::Dotted;
                    }
                    self.chart.edges.push(edge);
                }
                rest = r[end..].trim_start();
            } else if let Some(r) = rest.strip_prefix('{') {
                match r.trim_end().strip_suffix('}') {
                    // `class A { }` or `class A { x }` on one line.
                    Some(inner) => {
                        for m in inner.split(';') {
                            self.add_member(i, m);
                        }
                    }
                    None => body = true,
                }
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
        if body {
            self.body(i, kw, line, cursor);
        }
    }

    /// The members of a `{` ... `}` body (PCLS-3), or a map's `key => value` rows (PCLS-5).
    fn body(&mut self, i: usize, kw: &str, line: &Line, cursor: &mut Cursor) {
        let (rows, end) = cursor.until(&["}"]);
        if end.is_none() {
            self.error(line.range, format!("unclosed `{kw}` body: expected `}}`"));
        }
        if kw == "json" {
            return self.warn(line.range, "`json` objects are not drawn yet; ignoring it");
        }
        for row in rows {
            let t = row.text.as_str();
            if kw == "map" {
                if let Some((key, target)) = t.split_once("*->") {
                    let key = key.trim().to_owned();
                    let target = self.end(target.trim(), row.range);
                    let from = self.chart.nodes[i].id.clone();
                    let mut edge = Edge::new(from, target, row.range);
                    edge.start = Some(Arrow::Circle);
                    self.chart.edges.push(edge);
                    self.classes[i].fields.push(Row {
                        cells: vec![key],
                        ..Row::default()
                    });
                } else {
                    let (k, v) = t.split_once("=>").unwrap_or((t, ""));
                    self.classes[i].fields.push(Row {
                        cells: vec![k.trim().to_owned(), v.trim().to_owned()],
                        ..Row::default()
                    });
                }
                continue;
            }
            if let Some(title) = separator(t) {
                let class = &mut self.classes[i];
                let sections = class.sections.get_or_insert_with(|| {
                    let mut first = std::mem::take(&mut class.fields);
                    first.append(&mut class.methods);
                    vec![first]
                });
                let mut next = Vec::new();
                if let Some(title) = title {
                    next.push(Row {
                        cells: vec![title],
                        italic: true,
                        underline: false,
                    });
                }
                sections.push(next);
                continue;
            }
            self.add_member(i, t);
        }
    }

    fn add_member(&mut self, i: usize, raw: &str) {
        let raw = raw.trim();
        if raw.is_empty() {
            return;
        }
        let (method, row) = member(raw);
        let class = &mut self.classes[i];
        if let Some(sections) = &mut class.sections {
            if let Some(last) = sections.last_mut() {
                last.push(row);
            }
        } else if method {
            class.methods.push(row);
        } else {
            class.fields.push(row);
        }
    }

    /// A link between classes (PCLS-4).
    fn link(&mut self, line: &Line, link: &Link) {
        for item in &link.unknown {
            self.warn(
                line.range_of(link.arrow),
                format!("`{}`: {item} is not drawn; ignoring it", link.arrow),
            );
        }
        if link.from.starts_with('(') && link.from.contains(',') {
            return self.warn(
                line.range,
                "association classes (`(A, B) .. C`) are not drawn yet; ignoring it",
            );
        }
        let from = self.end(link.from, line.range_of(link.from));
        let to = self.end(link.to, line.range_of(link.to));
        let mut edge = Edge::new(from, to, line.range);
        edge.start = link.start;
        edge.end = link.end;
        edge.stroke = link.stroke;
        edge.length = link.length;
        edge.constraint = link.constraint;
        edge.styles = link.styles.clone();
        edge.label = link.label.as_deref().map(Label::plain);
        edge.start_label = link.from_label.as_deref().map(Label::plain);
        edge.end_label = link.to_label.as_deref().map(Label::plain);
        self.chart.edges.push(edge);
    }

    /// Notes (PCLS-8): `note left of A : text`, a block to `end note`, `note "text" as N`,
    /// `note left : text` after a declaration, and `note on link`.
    fn note(&mut self, line: &Line, cursor: &mut Cursor) {
        let Some(stmt) = lines::note(line, cursor, &mut self.d) else {
            return;
        };
        let (side, target, text, range) = match stmt {
            NoteStmt::Free { id, text, range } => {
                let id = id.unwrap_or_else(|| format!("note:{}", self.chart.nodes.len()));
                let i = self.class(&id, range);
                self.chart.nodes[i].label = Label::plain(text);
                self.chart.nodes[i].shape = Shape::Note;
                self.classes[i].plain = true;
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
        let target = match target.and_then(|t| name(&t).map(|n| n.0)) {
            Some(t) => t,
            None => match self.last.clone() {
                Some(t) => t,
                None => {
                    return self.warn(
                        line.range,
                        "a note needs `of` a class, or a class before it; ignoring it",
                    );
                }
            },
        };
        if !self.index.contains_key(&target) && self.packages.contains_key(&target) {
            return self.warn(
                line.range,
                format!("a note on package `{target}` is not drawn yet; ignoring it"),
            );
        }
        self.class(&target, line.range);
        self.chart.notes.push(Note {
            target,
            side,
            label: Label::plain(text),
            range,
        });
    }

    /// Stereotypes above the name, then the compartments (PCLS-2, PCLS-3, PCLS-7).
    fn finish(mut self) -> (Flowchart, Vec<Diagnostic>) {
        let hide = self.hide;
        // A package of the same name replaces a node made only by a link end.
        let stray: Vec<usize> = self
            .chart
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| self.implicit.contains(&n.id) && self.packages.contains_key(&n.id))
            .map(|(i, _)| i)
            .collect();
        for &i in stray.iter().rev() {
            self.chart.nodes.remove(i);
            self.classes.remove(i);
        }
        for (node, class) in self.chart.nodes.iter_mut().zip(self.classes) {
            if !class.stereotypes.is_empty() && !hide.stereotypes {
                let s: Vec<String> = class.stereotypes.iter().map(|s| format!("«{s}»")).collect();
                node.label.text = format!("{}\n{}", s.join(" "), node.label.text);
            }
            if class.plain {
                continue;
            }
            if let Some(sections) = class.sections {
                node.sections = sections
                    .into_iter()
                    .filter(|s| !(s.is_empty() && hide.empty_fields && hide.empty_methods))
                    .collect();
                continue;
            }
            let fields = (!hide.fields).then_some(class.fields);
            let methods = (!hide.methods && !class.fields_only).then_some(class.methods);
            let any = fields.iter().chain(&methods).any(|s| !s.is_empty());
            if !any {
                continue;
            }
            for (section, hide_empty) in
                [(fields, hide.empty_fields), (methods, hide.empty_methods)]
            {
                if let Some(s) = section
                    && !(s.is_empty() && hide_empty)
                {
                    node.sections.push(s);
                }
            }
        }
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

    fn cells(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|r| r.cells[0].as_str()).collect()
    }

    #[test]
    fn keyword_named_classes_link() {
        for name in [
            "Exception",
            "Entity",
            "Map",
            "Object",
            "Annotation",
            "Circle",
            "Record",
            "Package",
            "abstract",
        ] {
            let (g, d) = lower_src(&format!("class Z\n{name} --> X"));
            assert!(d.is_empty(), "{name}: {d:?}");
            assert_eq!(g.edges.len(), 1, "{name}");
            assert_eq!(g.edges[0].to, "X");
        }
        let (g, d) = lower_src("class Z\nException <|-- IOException");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.edges.len(), 1);
        let (g, d) = lower_src("class A\nentity E\npackage P {\nclass B\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("A").is_some() && g.node("E").is_some() && g.node("B").is_some());
    }

    #[test]
    fn namespaced_member() {
        let (g, d) = lower_src("class ns::Foo\nns::Foo : +bar()");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("ns").is_none());
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.nodes[0].sections.len(), 2);
    }

    #[test]
    fn dotted_name_after_keyword_is_a_declaration() {
        // `x.` reads as an arrow head, but `class x.y.Z {` declares a class.
        let (g, d) = lower_src("class x.y.Z {\n  -id : int\n  +run()\n}\nclass x.Y");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.edges.is_empty());
        assert!(g.node("x.y.Z").is_some() && g.node("x.Y").is_some());
    }

    #[test]
    fn link_end_package_replaces_stray_node() {
        let (g, d) = lower_src("A --> P\npackage P {\nclass B\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("P").is_none());
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].to, "P");
        assert!(
            g.subgraphs
                .iter()
                .all(|s| !s.members.iter().any(|m| m == "P"))
        );
    }

    #[test]
    fn note_named_class_links() {
        for name in ["note", "Note", "NOTE"] {
            let (g, d) = lower_src(&format!("class Z\n{name} --> X"));
            assert!(d.is_empty(), "{name}: {d:?}");
            assert_eq!(g.edges.len(), 1, "{name}");
            assert_eq!(g.edges[0].from, name);
            assert!(g.notes.is_empty(), "{name}");
        }
        // Real notes, even with arrow-like text, stay notes.
        let (g, d) = lower_src("class A\nnote \"x -> y\" as N\nnote right of A : a -> b");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.edges.is_empty(), "{:?}", g.edges.len());
        assert_eq!(g.node("N").unwrap().shape, Shape::Note);
        assert_eq!(g.notes.len(), 1);
    }

    #[test]
    fn keyword_named_class_with_multiplicities() {
        let (g, d) = lower_src("class Z\nEntity \"1\" --> \"n\" Map : uses");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.edges.len(), 1);
        let e = &g.edges[0];
        assert_eq!((e.from.as_str(), e.to.as_str()), ("Entity", "Map"));
        assert_eq!(e.start_label.as_ref().unwrap().text, "1");
        assert_eq!(e.end_label.as_ref().unwrap().text, "n");
        // A declaration that merely follows a keyword is still a declaration.
        let (g, d) = lower_src("class Entity\nentity E2 <<x>>");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("E2").is_some());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn member_colon_skips_double_colons() {
        let (g, d) = lower_src("class Z\nns::Foo::Bar : +x : int");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("ns").is_none() && g.node("ns::Foo").is_none());
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(cells(&g.nodes[1].sections[0]), ["+x : int"]);
        // A member holding `::` itself.
        let (g, d) = lower_src("class Z\nA : +std::vector<int> v");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            cells(&g.node("A").unwrap().sections[0]),
            ["+std::vector<int> v"]
        );
        // Only `::` pairs: no member colon is an error, and a leading colon has no target.
        let (_, d) = lower_src("class Z\nns::Foo");
        assert_eq!(d, [STATEMENT]);
        let (_, d) = lower_src("class Z\n: x");
        assert_eq!(d, [STATEMENT]);
    }

    #[test]
    fn implicit_node_declared_then_packaged() {
        // Declared as a class, so the later package does not replace it.
        let (g, d) = lower_src("A --> P\nclass P\npackage P {\nclass B\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("P").is_some());
        assert_eq!(g.edges[0].to, "P");
        // A link end that names an existing package never makes a node.
        let (g, d) = lower_src("package P {\nclass B\n}\nA --> P\nP --> C");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("P").is_none());
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn stray_node_removal_keeps_classes_aligned() {
        let (g, d) = lower_src(
            "X --> P1\nclass K {\n  km()\n}\nY --> P2\nclass L {\n  lf\n}\n\
             package P1 {\n}\npackage P2 {\n  class In\n}\nclass M {\n  mm()\n}\n\
             note left of M : n",
        );
        assert!(d.is_empty(), "{d:?}");
        let ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["X", "K", "Y", "L", "In", "M"]);
        assert_eq!(cells(&g.node("K").unwrap().sections[1]), ["km()"]);
        assert_eq!(cells(&g.node("L").unwrap().sections[0]), ["lf"]);
        assert_eq!(cells(&g.node("M").unwrap().sections[1]), ["mm()"]);
        assert!(g.node("X").unwrap().sections.is_empty());
        assert!(
            g.subgraphs
                .iter()
                .all(|s| s.members.iter().all(|m| m != "P1" && m != "P2"))
        );
        assert_eq!(g.subgraphs[1].members, ["In"]);
        assert_eq!(g.notes[0].target, "M");
    }

    #[test]
    fn stray_package_removed_from_enclosing_subgraph() {
        // The link end `Q` is created inside `outer`, then a package `Q` replaces it.
        let (g, d) = lower_src("package outer {\nA --> Q\n}\npackage Q {\nclass B\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("Q").is_none());
        assert_eq!(g.subgraphs[0].members, ["A"]);
    }

    #[test]
    fn stray_removal_keeps_nested_package_membership() {
        // The package `Q` itself stays a member of `outer`; only the stray node goes.
        let (g, d) = lower_src("A --> Q\npackage outer {\npackage Q {\nclass B\n}\n}");
        assert!(d.is_empty(), "{d:?}");
        assert!(g.node("Q").is_none());
        let outer = g.subgraphs.iter().find(|s| s.id == "outer").unwrap();
        assert_eq!(outer.members, ["Q"]);
    }

    #[test]
    fn declarations() {
        let (g, d) = lower_src(
            "class \"Long name\" as L <<Entity>> #pink\nabstract class Shape\ninterface I<T>\n\
             enum E {\n  RED\n  GREEN\n}\nclass A extends Shape implements I",
        );
        assert!(d.is_empty(), "{d:?}");
        let label = |id: &str| g.node(id).unwrap().label.text.clone();
        assert_eq!(label("L"), "«Entity»\nLong name");
        assert_eq!(g.node("L").unwrap().styles[0].value, "#ffc0cb");
        assert_eq!(label("Shape"), "«abstract»\nShape");
        assert_eq!(label("I"), "«interface»\nI<T>");
        assert_eq!(cells(&g.node("E").unwrap().sections[0]), ["RED", "GREEN"]);
        let edges: Vec<_> = g
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.stroke))
            .collect();
        assert_eq!(
            edges,
            [("Shape", "A", Stroke::Normal), ("I", "A", Stroke::Dotted)]
        );
    }

    #[test]
    fn members() {
        let (g, d) = lower_src(
            "class A {\n  -id : int\n  +{static} make() : A\n  {abstract} #draw()\n  {method} run\n}\n\
             A : +name : String\nclass B {\n  a\n  --\n  b()\n  .. Getters ..\n  c()\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        let a = g.node("A").unwrap();
        assert_eq!(cells(&a.sections[0]), ["-id : int", "+name : String"]);
        assert_eq!(cells(&a.sections[1]), ["+make() : A", "#draw()", "run"]);
        assert!(a.sections[1][0].underline && a.sections[1][1].italic);
        let b = g.node("B").unwrap();
        let sections: Vec<_> = b.sections.iter().map(|s| cells(s)).collect();
        assert_eq!(sections, [vec!["a"], vec!["b()"], vec!["Getters", "c()"]]);
        let (g, _) = lower_src("hide empty members\nclass A {\n  x\n}\nclass B");
        assert_eq!(g.node("A").unwrap().sections.len(), 1);
        assert!(g.node("B").unwrap().sections.is_empty());
    }

    #[test]
    fn links_packages_and_notes() {
        let (g, d) = lower_src(
            "package shop {\n  class Order\n  package items {\n    class Item\n  }\n}\n\
             Order \"1\" *-- \"many\" Item : contains\nOrder ..> Customer\nnote left of Order : n\n\
             note \"free\" as N\nN .. Item\nnote on link : about\nmap M {\n  k => v\n}",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.subgraphs[0].members, ["Order", "items"]);
        assert_eq!(g.subgraphs[1].members, ["Item"]);
        let e = &g.edges[0];
        assert_eq!((e.start, e.end), (Some(Arrow::Diamond), None));
        assert_eq!(e.start_label.as_ref().unwrap().text, "1");
        assert_eq!(e.label.as_ref().unwrap().text, "contains");
        assert!(g.node("Customer").is_some());
        assert_eq!(g.notes[0].target, "Order");
        assert_eq!(g.node("N").unwrap().shape, Shape::Note);
        assert_eq!(g.edges[2].label.as_ref().unwrap().text, "about");
        assert_eq!(g.node("M").unwrap().sections[0][0].cells, ["k", "v"]);
    }

    #[test]
    fn errors() {
        let (_, d) = lower_src(
            "class A {\n  x\nwhat is this\n}\n}\nclass B <<x>> ?\nA -[wavy]-> B\nclass C {",
        );
        assert_eq!(
            d,
            [
                "`}` without a `{` to close",
                "`?` is not drawn; ignoring it",
                "`-[wavy]->`: style `wavy` is not drawn; ignoring it",
                "unclosed `class` body: expected `}`",
            ],
        );
    }
}
