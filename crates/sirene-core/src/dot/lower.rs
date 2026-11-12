//! Lowers a DOT syntax tree to the graph model (DOT-2, DOT-3).

use std::collections::{HashMap, HashSet};

use rowan::TextRange;

use super::parser;
use super::x11::X11;
use crate::diagnostic::Diagnostic;
use crate::flowchart::text::decode_html_entities;
use crate::flowchart::{
    Arrow, Compass, Direction, Edge, EdgeOrder, Flowchart, Label, Node, RankEnd, Row, Shape,
    Stroke, Style, Subgraph,
};
use crate::scene::{Color, parse_color};
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// A lowered DOT graph: the graph model, the problems found, and the graph's `bgcolor`.
pub(crate) struct Output {
    pub flowchart: Flowchart,
    pub diagnostics: Vec<Diagnostic>,
    pub background: Option<Color>,
}

pub(crate) fn lower(root: &SyntaxNode) -> Output {
    let mut l = Lowerer::default();
    let mut graphs = root.children().filter(|n| n.kind() == DOT_GRAPH);
    if let Some(g) = graphs.next() {
        l.graph(&g);
    }
    for extra in graphs {
        l.warning(
            extra.text_range(),
            "only the first graph in a file is drawn",
        );
    }
    l.finish()
}

/// An attribute value: the text of a quoted string, name, or numeral, or of an HTML string
/// without its outer `<` and `>`.
#[derive(Debug, Clone)]
struct Value {
    text: String,
    html: bool,
}

#[derive(Debug, Clone)]
struct Attr {
    key: String,
    value: Value,
    range: TextRange,
}

/// The last value of `key` among `attrs`.
fn get<'a>(attrs: &'a [Attr], key: &str) -> Option<&'a Attr> {
    attrs.iter().rev().find(|a| a.key == key)
}

fn get_text<'a>(attrs: &'a [Attr], key: &str) -> Option<&'a str> {
    get(attrs, key).map(|a| a.value.text.as_str())
}

/// What a statement's attributes apply to, for knowing which are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Graph,
    Node,
    Edge,
}

const GRAPH_ATTRS: &[&str] = &[
    "rankdir",
    "label",
    "bgcolor",
    "color",
    "fillcolor",
    "pencolor",
    "fontcolor",
    "style",
    "compound",
    "ordering",
];
const NODE_ATTRS: &[&str] = &[
    "label",
    "shape",
    "style",
    "color",
    "fillcolor",
    "fontcolor",
    "penwidth",
    "ordering",
    "group",
];
const EDGE_ATTRS: &[&str] = &[
    "label",
    "headlabel",
    "taillabel",
    "style",
    "color",
    "fontcolor",
    "penwidth",
    "arrowhead",
    "arrowtail",
    "dir",
    "minlen",
    "constraint",
    "lhead",
    "ltail",
    "headport",
    "tailport",
];

/// Whether `x` is a compass point, `c` and `_` (the center) included.
fn is_compass(x: &str) -> bool {
    Compass::parse(x).is_some() || matches!(x, "c" | "C" | "_")
}

/// A Graphviz boolean: `true`, `yes`, `false`, `no` in any case, or an integer, nonzero for true.
fn boolean(text: &str) -> Option<bool> {
    let t = text.trim().to_ascii_lowercase();
    match t.as_str() {
        "true" | "yes" => Some(true),
        "false" | "no" => Some(false),
        _ => t.parse::<i64>().ok().map(|n| n != 0),
    }
}

/// The defaults in effect in a graph or subgraph (DOT-2).
#[derive(Debug, Clone, Default)]
struct Scope {
    node: Vec<Attr>,
    edge: Vec<Attr>,
    /// Graph attributes set here and in the enclosing scopes, which new clusters start with.
    graph: Vec<Attr>,
    /// The innermost cluster, whose members the nodes mentioned here become.
    cluster: Option<usize>,
    /// Whether this scope is the root graph or a cluster, whose graph attributes are drawn.
    drawn: bool,
    /// This subgraph's own `rank`, which applies to every node mentioned in it.
    rank: Option<Attr>,
}

struct Cluster {
    id: String,
    parent: Option<usize>,
    attrs: Vec<Attr>,
    range: TextRange,
}

/// A port written on an edge end: `a:x` or `a:port:x`, read once the node's record fields
/// are known.
#[derive(Clone)]
struct Port {
    parts: Vec<String>,
    range: TextRange,
}

struct PendingEdge {
    from: usize,
    to: usize,
    /// The ports written on the tail and head (`a:n -> b:p:s`).
    ports: [Option<Port>; 2],
    attrs: Vec<Attr>,
    range: TextRange,
}

#[derive(Default)]
struct Lowerer {
    diagnostics: Vec<Diagnostic>,
    directed: bool,
    strict: bool,
    name: String,
    root_attrs: Vec<Attr>,
    ids: Vec<String>,
    node_index: HashMap<String, usize>,
    node_attrs: Vec<Vec<Attr>>,
    node_cluster: Vec<Option<usize>>,
    mentions: Vec<Vec<TextRange>>,
    /// Each node's record port names and their lines (`Edge::rows`), once it is lowered.
    record_ports: Vec<HashMap<String, usize>>,
    edges: Vec<PendingEdge>,
    clusters: Vec<Cluster>,
    cluster_index: HashMap<String, usize>,
    /// Nodes of each subgraph with a `rank`, its end (none for `same`), and the range of its
    /// `rank`.
    rank_groups: Vec<(Option<RankEnd>, Vec<usize>, TextRange)>,
    /// The root graph's `compound`, which lets `lhead` and `ltail` end edges at clusters.
    compound: bool,
    /// Whether `rankdir` is LR or RL, where record ports are drawn.
    horizontal: bool,
    /// Names already warned about, so each is reported once per file.
    warned: HashSet<String>,
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn keyword(node: &SyntaxNode) -> Option<String> {
    tokens(node)
        .find(|t| t.kind() == DOT_KW)
        .map(|t| t.text().to_ascii_lowercase())
}

fn ids(node: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    node.children().filter(|n| n.kind() == DOT_ID)
}

/// The text of a quoted string: without its quotes, `\"` as `"`, and a backslash before a
/// line break joining the lines. Other escapes stay for the label (DOT-2).
fn unquote(s: &str) -> String {
    let inner = s.strip_prefix('"').unwrap_or(s);
    // An unclosed string runs to the end of the source, which may end in an escaped quote.
    let inner = match parser::string_len(s) {
        Some(n) if n == s.len() => inner.strip_suffix('"').unwrap_or(inner),
        _ => inner,
    };
    inner
        .replace("\\\r\n", "")
        .replace("\\\n", "")
        .replace("\\\"", "\"")
}

fn value(id: &SyntaxNode) -> Value {
    let mut text = String::new();
    let mut html = false;
    for t in tokens(id) {
        match t.kind() {
            STRING => text.push_str(&unquote(t.text())),
            HTML_STRING => {
                html = true;
                let s = t.text();
                let inner = s.strip_prefix('<').unwrap_or(s);
                // An unclosed HTML string keeps its last character.
                let inner = match parser::html_len(s) {
                    Some(n) if n == s.len() => inner.strip_suffix('>').unwrap_or(inner),
                    _ => inner,
                };
                text.push_str(inner);
            }
            IDENT | NUMBER => text.push_str(t.text()),
            _ => {}
        }
    }
    Value { text, html }
}

/// A label's escapes: `\n`, `\l`, and `\r` break lines, `\N` is the node's name, `\G` the
/// graph's, `\E` the edge's, `\T` and `\H` its tail and head, and `\x` is `x`. A name's own
/// escapes are read too, as Graphviz does: the node `"a\nb"` is labeled on two lines. HTML
/// entities are decoded after the names are put in and before the line breaks are read.
fn escape(text: &str, names: &Names) -> String {
    let mut named = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            named.push(c);
            continue;
        }
        match chars.next() {
            Some('N') => named.push_str(names.node),
            Some('G') => named.push_str(names.graph),
            Some('E') => named.push_str(&names.edge()),
            Some('T') => named.push_str(names.tail),
            Some('H') => named.push_str(names.head),
            Some(c) => {
                named.push('\\');
                named.push(c);
            }
            None => named.push('\\'),
        }
    }
    let named = decode_html_entities(&named);
    let mut out = String::new();
    let mut chars = named.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'l' | 'r') => out.push('\n'),
            Some(c) => out.push(c),
            None => out.push('\\'),
        }
    }
    // A final `\l` or `\n` ends the last line rather than starting another.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// The names a label's escapes refer to.
struct Names<'a> {
    graph: &'a str,
    node: &'a str,
    tail: &'a str,
    head: &'a str,
    directed: bool,
}

impl Names<'_> {
    fn edge(&self) -> String {
        let op = if self.directed { "->" } else { "--" };
        format!("{}{op}{}", self.tail, self.head)
    }
}

/// An HTML label's text: tags dropped, a line per `<br/>` and per table row with a space
/// between its cells, and the common entities decoded. Line breaks in the source are not text,
/// as in Graphviz.
fn html_text(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.extend(rest[..i].chars().filter(|c| !c.is_control()));
        let (tag, after) = match rest[i..].find('>') {
            Some(j) => (&rest[i + 1..i + j], i + j + 1),
            None => (&rest[i + 1..], rest.len()),
        };
        let tag = tag.trim().to_ascii_lowercase();
        let name = tag.trim_end_matches('/').split_whitespace().next();
        match name {
            Some("br") => out.push('\n'),
            // Nested tables end several rows at once.
            Some("/tr") if !out.trim_end_matches(' ').ends_with('\n') => out.push('\n'),
            Some("/td") => out.push(' '),
            _ => {}
        }
        rest = &rest[after..];
    }
    out.extend(rest.chars().filter(|c| !c.is_control()));
    // Source indentation is not text, but `&nbsp;` padding is.
    let blank = |c: char| c.is_ascii_whitespace();
    let out = out
        .lines()
        .map(|l| l.trim_matches(blank))
        .collect::<Vec<_>>()
        .join("\n");
    decode_html_entities(out.trim_matches(blank))
}

/// A cell of an HTML table: its text and its `PORT`.
type Cell = (String, Option<String>);

/// The value of attribute `key` in a tag's text (`td port="p"`), in any case.
fn tag_attr(tag: &str, key: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(k) = lower[from..].find(key) {
        let at = from + k;
        from = at + key.len();
        let before = lower[..at].chars().next_back();
        if before.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }
        let rest = tag[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
            _ => rest.split(char::is_whitespace).next().unwrap_or(""),
        };
        return Some(value.trim_end_matches('/').to_owned());
    }
    None
}

/// The outermost table of an HTML label as sections of rows of cells: each `<TR>` a row,
/// each `<TD>` a cell whose content is drawn as text (a nested table too), and `<HR/>` between
/// rows a new section (DOT-3). `None` when the label is not one table.
fn html_table(s: &str) -> Option<Vec<Vec<Vec<Cell>>>> {
    let mut sections: Vec<Vec<Vec<Cell>>> = vec![Vec::new()];
    let mut outside = String::new();
    let mut depth = 0usize;
    let mut tables = 0;
    let mut cell: Option<(usize, Option<String>)> = None;
    let mut at = 0;
    while let Some(i) = s[at..].find('<') {
        let start = at + i;
        if depth == 0 {
            outside.push_str(&s[at..start]);
        }
        let j = s[start..].find('>')?;
        let (tag, end) = (&s[start + 1..start + j], start + j + 1);
        let name = tag
            .trim()
            .trim_end_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match name.as_str() {
            "table" => {
                depth += 1;
                if depth == 1 {
                    tables += 1;
                }
            }
            "/table" => depth = depth.checked_sub(1)?,
            "tr" if depth == 1 => sections.last_mut()?.push(Vec::new()),
            "hr" if depth == 1 => sections.push(Vec::new()),
            "td" if depth == 1 => cell = Some((end, tag_attr(tag, "port"))),
            "/td" if depth == 1 => {
                if let Some((from, port)) = cell.take() {
                    let text = html_text(&s[from..start]);
                    let row = sections.last_mut()?;
                    if row.is_empty() {
                        row.push(Vec::new());
                    }
                    row.last_mut()?.push((text, port.filter(|p| !p.is_empty())));
                }
            }
            _ => {}
        }
        at = end;
    }
    outside.push_str(&s[at..]);
    sections.retain_mut(|rows| {
        rows.retain(|r| !r.is_empty());
        !rows.is_empty()
    });
    let text = html_text(&outside);
    (tables == 1 && depth == 0 && text.is_empty() && !sections.is_empty()).then_some(sections)
}

/// A field of a record label: text and its `<port>` name, or a group of fields in braces
/// (DOT-3).
#[derive(Debug, Clone, PartialEq)]
enum Field {
    Text(String, Option<String>),
    Group(Vec<Field>),
}

/// How deep a record label's groups nest; a brace past that is text.
const MAX_RECORD_DEPTH: usize = 16;

/// The fields of a record label: `|` separates them, braces group them, and a `<port>` names
/// one. `\{`, `\}`, `\|`, `\<`, `\>`, and `\ ` are literal.
fn record_fields(s: &str) -> Vec<Field> {
    fn group(chars: &mut std::str::Chars, depth: usize) -> Vec<Field> {
        let mut fields = Vec::new();
        let mut text = String::new();
        let mut port: Option<String> = None;
        let mut nested: Option<Vec<Field>> = None;
        let push = |fields: &mut Vec<Field>,
                    text: &mut String,
                    port: &mut Option<String>,
                    nested: &mut Option<Vec<Field>>| {
            match nested.take() {
                Some(g) => fields.push(Field::Group(g)),
                None => fields.push(Field::Text(
                    std::mem::take(text).trim().to_owned(),
                    port.take(),
                )),
            }
            text.clear();
            port.take();
        };
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some(e @ ('{' | '}' | '|' | '<' | '>' | ' ')) => text.push(e),
                    Some(e) => {
                        text.push('\\');
                        text.push(e);
                    }
                    None => {}
                },
                '{' if depth < MAX_RECORD_DEPTH => nested = Some(group(chars, depth + 1)),
                '}' if depth > 0 => break,
                '|' => push(&mut fields, &mut text, &mut port, &mut nested),
                '<' => {
                    let mut name = String::new();
                    for p in chars.by_ref() {
                        if p == '>' {
                            break;
                        }
                        name.push(p);
                    }
                    let name = name.trim();
                    if !name.is_empty() {
                        port = Some(name.to_owned());
                    }
                }
                _ => text.push(c),
            }
        }
        push(&mut fields, &mut text, &mut port, &mut nested);
        fields
    }
    let mut fields = group(&mut s.chars(), 0);
    // `{a|b|c}` stacks its fields, as compartments do.
    if let [Field::Group(g)] = fields.as_slice() {
        fields = g.clone();
    }
    fields
}

/// The texts in a field and their port names, in order.
fn leaves(field: &Field, out: &mut Vec<(String, Option<String>)>) {
    match field {
        Field::Text(t, port) => out.push((t.clone(), port.clone())),
        Field::Group(g) => g.iter().for_each(|f| leaves(f, out)),
    }
}

/// A Graphviz color: an X11 name, `#rrggbb`, `#rrggbbaa`, an `H,S,V` triple of fractions, or
/// the first color of a list such as `red:blue` (DOT-3).
fn color(value: &str) -> Option<Color> {
    let first = value.split(':').next().unwrap_or("");
    let first = first.split(';').next().unwrap_or("").trim();
    let v = first.to_ascii_lowercase();
    let v = match v.strip_prefix('/') {
        Some(scheme) => {
            let (scheme, name) = scheme.split_once('/')?;
            if !matches!(scheme, "" | "x11" | "svg") {
                return None;
            }
            name.to_owned()
        }
        None => v,
    };
    if v.starts_with('#') {
        return parse_color(&v);
    }
    let hsv: Vec<f32> = v
        .split([',', ' '])
        .filter(|s| !s.is_empty())
        .map_while(|s| s.parse().ok())
        .collect();
    if let [h, s, b] = hsv[..] {
        return Some(hsv_color(h, s, b));
    }
    if let Ok(i) = X11.binary_search_by(|(n, _)| (*n).cmp(v.as_str())) {
        return Some(Color::rgb(X11[i].1));
    }
    parse_color(&v).filter(|_| !v.starts_with("rgb"))
}

fn hsv_color(h: f32, s: f32, v: f32) -> Color {
    let (h, s, v) = (
        h.clamp(0.0, 1.0) * 6.0,
        s.clamp(0.0, 1.0),
        v.clamp(0.0, 1.0),
    );
    let i = h.floor().min(5.0);
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as u8 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let c = |x: f32| (x * 255.0).round() as u8;
    Color {
        r: c(r),
        g: c(g),
        b: c(b),
        a: 255,
    }
}

fn hex(c: Color) -> String {
    if c.a == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

fn style(property: &str, value: impl Into<String>) -> Style {
    Style {
        property: property.into(),
        value: value.into(),
    }
}

/// A node shape, or `None` for one sirene does not draw (DOT-3).
fn shape(name: &str) -> Option<Shape> {
    Some(match name {
        "box" | "rect" | "rectangle" | "square" | "msquare" | "record" => Shape::Rect,
        "mrecord" => Shape::Rounded,
        "ellipse" | "oval" | "egg" => Shape::Ellipse,
        "circle" | "mcircle" => Shape::Circle,
        "doublecircle" => Shape::DoubleCircle,
        "point" => Shape::FilledCircle,
        "diamond" | "mdiamond" => Shape::Diamond,
        "plaintext" | "plain" | "none" | "underline" => Shape::Text,
        "cylinder" => Shape::Cylinder,
        "hexagon" | "octagon" | "doubleoctagon" | "tripleoctagon" | "septagon" | "pentagon" => {
            Shape::Hexagon
        }
        "parallelogram" => Shape::LeanRight,
        "trapezium" | "trapezoid" => Shape::TrapezoidBottom,
        "invtrapezium" | "invtrapezoid" => Shape::TrapezoidTop,
        "triangle" => Shape::Triangle,
        "invtriangle" => Shape::FlippedTriangle,
        "note" => Shape::Note,
        "tab" | "folder" => Shape::Folder,
        "box3d" => Shape::StackedRect,
        "component" => Shape::LinedRect,
        "cds" => Shape::Flag,
        "star" => Shape::Bang,
        _ => return None,
    })
}

/// The marker an arrow name draws, `Some(None)` for `none`, or `None` for an unknown name.
/// A name may stack several shapes (`invdot`); the first is drawn.
fn arrow(name: &str) -> Option<Option<Arrow>> {
    let name = match name {
        "empty" => "onormal",
        "invempty" => "oinv",
        "ediamond" => "odiamond",
        "open" => "vee",
        "halfopen" => "lvee",
        n => n,
    };
    // Modifiers: `o` for an open shape, then `l` or `r` for half of one.
    let mut s = name;
    let open = s.starts_with('o');
    if open {
        s = &s[1..];
    }
    if s.starts_with(['l', 'r']) {
        s = &s[1..];
    }
    const SHAPES: &[&str] = &[
        "normal", "inv", "vee", "curve", "icurve", "diamond", "dot", "box", "tee", "crow", "none",
    ];
    let first = SHAPES.iter().find(|sh| s.starts_with(**sh))?;
    Some(match *first {
        "none" => None,
        "normal" | "inv" if open => Some(Arrow::Triangle),
        "normal" | "inv" | "vee" | "curve" | "icurve" => Some(Arrow::Point),
        "diamond" if open => Some(Arrow::HollowDiamond),
        "diamond" => Some(Arrow::Diamond),
        "dot" => Some(Arrow::Circle),
        "box" | "tee" => Some(Arrow::One),
        "crow" if open => Some(Arrow::ZeroMany),
        "crow" => Some(Arrow::OneMany),
        _ => unreachable!("listed above"),
    })
}

impl Lowerer {
    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    /// Warns once per file under `key`.
    fn warn_once(&mut self, key: &str, range: TextRange, message: impl Into<String>) {
        if self.warned.insert(key.to_owned()) {
            self.warning(range, message);
        }
    }

    fn graph(&mut self, g: &SyntaxNode) {
        for t in tokens(g).filter(|t| t.kind() == DOT_KW) {
            match t.text().to_ascii_lowercase().as_str() {
                "strict" => self.strict = true,
                "digraph" => self.directed = true,
                _ => {}
            }
        }
        if let Some(id) = ids(g).next() {
            self.name = value(&id).text;
        }
        let mut scope = Scope {
            drawn: true,
            ..Scope::default()
        };
        self.stmts(g, &mut scope);
    }

    /// The attributes in a statement's `[...]` lists, each checked against what `target`
    /// draws.
    fn attr_lists(&mut self, stmt: &SyntaxNode, target: Target) -> Vec<Attr> {
        let mut out = Vec::new();
        for list in stmt.children().filter(|n| n.kind() == DOT_ATTR_LIST) {
            for a in list.children().filter(|n| n.kind() == ATTR) {
                let mut it = ids(&a);
                let (Some(k), Some(v)) = (it.next(), it.next()) else {
                    continue;
                };
                let attr = Attr {
                    key: value(&k).text,
                    value: value(&v),
                    range: a.text_range(),
                };
                self.check(&attr, target);
                out.push(attr);
            }
        }
        out
    }

    /// Warns, once per name, about an attribute `target` does not draw (DOT-3, DOT-4).
    fn check(&mut self, attr: &Attr, target: Target) {
        let known = match target {
            Target::Graph => GRAPH_ATTRS,
            Target::Node => NODE_ATTRS,
            Target::Edge => EDGE_ATTRS,
        };
        if known.contains(&attr.key.as_str()) {
            // A node's table label is drawn as compartments.
            let table = target == Target::Node
                && attr.key == "label"
                && html_table(&attr.value.text).is_some();
            if attr.value.html && attr.key.ends_with("label") && !table {
                self.warn_once("<html>", attr.range, "HTML labels are drawn as their text");
            }
            return;
        }
        let what = match target {
            Target::Graph => "graph",
            Target::Node => "node",
            Target::Edge => "edge",
        };
        let key = attr.key.clone();
        self.warn_once(
            &format!("{what}:{key}"),
            attr.range,
            format!("the {what} attribute `{key}` is not supported; ignored"),
        );
    }

    /// The statements in a graph or subgraph body. Returns the nodes they mention.
    fn stmts(&mut self, body: &SyntaxNode, scope: &mut Scope) -> Vec<usize> {
        let mut mentioned = Vec::new();
        for stmt in body.children() {
            match stmt.kind() {
                DOT_ASSIGN => {
                    let mut it = ids(&stmt);
                    if let (Some(k), Some(v)) = (it.next(), it.next()) {
                        let attr = Attr {
                            key: value(&k).text,
                            value: value(&v),
                            range: stmt.text_range(),
                        };
                        self.graph_attr(attr, scope);
                    }
                }
                DOT_ATTR_STMT => match keyword(&stmt).as_deref() {
                    Some("node") => {
                        let attrs = self.attr_lists(&stmt, Target::Node);
                        scope.node.extend(attrs);
                    }
                    Some("edge") => {
                        let attrs = self.attr_lists(&stmt, Target::Edge);
                        scope.edge.extend(attrs);
                    }
                    _ => {
                        for attr in self.attr_lists_unchecked(&stmt) {
                            self.graph_attr(attr, scope);
                        }
                    }
                },
                DOT_NODE_STMT => {
                    let Some(n) = stmt.children().find(|n| n.kind() == NODE) else {
                        continue;
                    };
                    // A port on a node statement means nothing; Graphviz ignores it too.
                    let (i, _) = self.node_ref(&n, scope);
                    let attrs = self.attr_lists(&stmt, Target::Node);
                    self.node_attrs[i].extend(attrs);
                    mentioned.push(i);
                }
                DOT_EDGE_STMT => mentioned.extend(self.edge_stmt(&stmt, scope)),
                SUBGRAPH => mentioned.extend(self.subgraph(&stmt, scope)),
                _ => {}
            }
        }
        mentioned
    }

    fn attr_lists_unchecked(&mut self, stmt: &SyntaxNode) -> Vec<Attr> {
        stmt.children()
            .filter(|n| n.kind() == DOT_ATTR_LIST)
            .flat_map(|list| list.children().filter(|n| n.kind() == ATTR))
            .filter_map(|a| {
                let mut it = ids(&a);
                let (k, v) = (it.next()?, it.next()?);
                Some(Attr {
                    key: value(&k).text,
                    value: value(&v),
                    range: a.text_range(),
                })
            })
            .collect()
    }

    /// A graph attribute: the root graph's or a cluster's; a plain subgraph's draw nothing.
    /// Either way it is a default for the clusters created later in the scope, as in Graphviz.
    fn graph_attr(&mut self, attr: Attr, scope: &mut Scope) {
        if attr.key == "rank" {
            if scope.drawn && scope.cluster.is_none() {
                self.warn_once(
                    "graph:rank",
                    attr.range,
                    "`rank` applies to subgraphs; ignored on the graph",
                );
            } else {
                scope.rank = Some(attr);
            }
            return;
        }
        self.check(&attr, Target::Graph);
        scope.graph.push(attr.clone());
        if !scope.drawn {
            return;
        }
        match scope.cluster {
            Some(c) => self.clusters[c].attrs.push(attr),
            None => self.root_attrs.push(attr),
        }
    }

    /// The node a `NODE` names, created with the scope's defaults if new, and the port
    /// written after it (`a:n`, `a:port:n`).
    fn node_ref(&mut self, n: &SyntaxNode, scope: &Scope) -> (usize, Option<Port>) {
        let mut it = ids(n);
        let Some(id) = it.next() else {
            return (self.node("", n.text_range(), scope), None);
        };
        let parts: Vec<String> = it.map(|p| value(&p).text).collect();
        let port = (!parts.is_empty()).then(|| Port {
            parts,
            range: n.text_range(),
        });
        let name = value(&id).text;
        (self.node(&name, id.text_range(), scope), port)
    }

    /// The record line and compass point of `port` on `node`: `x` is a record port of the
    /// node, else a compass point; `p:x` a record port and a compass point. An unknown
    /// record port is a warning and joins the node.
    fn port(&mut self, node: usize, port: &Port) -> (Option<usize>, Option<Compass>) {
        let range = port.range;
        let (name, compass) = match port.parts.as_slice() {
            [] => (None, None),
            [x] if !self.record_ports[node].contains_key(x) && is_compass(x) => (None, Some(x)),
            [x] => (Some(x), None),
            [p, x, ..] => (Some(p), Some(x)),
        };
        let row = name.and_then(|p| {
            let row = self.record_ports[node].get(p).copied();
            if row.is_none() {
                let id = self.ids[node].clone();
                self.warn_once(
                    &format!("port@{range:?}"),
                    range,
                    format!("`{id}` has no record port `{p}`; the edge joins the node"),
                );
            }
            row
        });
        let compass = compass.cloned().and_then(|x| self.compass(&x, range));
        (row, compass)
    }

    /// A compass point; an unknown one is a warning.
    fn compass(&mut self, x: &str, range: TextRange) -> Option<Compass> {
        if !is_compass(x) {
            self.warn_once(
                &format!("compass@{range:?}"),
                range,
                format!(
                    "unknown compass point `{x}`: expected n, ne, e, se, s, sw, w, nw, c, or _"
                ),
            );
        }
        Compass::parse(x)
    }

    fn node(&mut self, id: &str, range: TextRange, scope: &Scope) -> usize {
        let i = match self.node_index.get(id) {
            Some(&i) => {
                // The innermost cluster of those a node is mentioned in holds it.
                let inner = match self.node_cluster[i] {
                    None => true,
                    Some(held) => self.encloses(held, scope.cluster),
                };
                if inner {
                    self.node_cluster[i] = scope.cluster;
                }
                i
            }
            None => {
                let i = self.ids.len();
                self.ids.push(id.to_owned());
                self.node_index.insert(id.to_owned(), i);
                self.node_attrs.push(scope.node.clone());
                self.node_cluster.push(scope.cluster);
                self.mentions.push(Vec::new());
                self.record_ports.push(HashMap::new());
                i
            }
        };
        self.mentions[i].push(range);
        i
    }

    /// Whether the cluster `outer` holds the cluster `inner`, at any depth.
    fn encloses(&self, outer: usize, inner: Option<usize>) -> bool {
        let mut at = inner.and_then(|c| self.clusters[c].parent);
        while let Some(c) = at {
            if c == outer {
                return true;
            }
            at = self.clusters[c].parent;
        }
        false
    }

    /// `a -> b -> {c d} [attrs]`: an edge from every node of each operand to every node of
    /// the next. Returns the nodes mentioned.
    fn edge_stmt(&mut self, stmt: &SyntaxNode, scope: &mut Scope) -> Vec<usize> {
        let mut operands: Vec<(Vec<usize>, Option<Port>, TextRange)> = Vec::new();
        for n in stmt.children() {
            match n.kind() {
                NODE => {
                    let (i, port) = self.node_ref(&n, scope);
                    operands.push((vec![i], port, n.text_range()));
                }
                SUBGRAPH => {
                    let nodes = self.subgraph(&n, scope);
                    operands.push((nodes, None, n.text_range()));
                }
                // A subgraph nested too deep to read: no nodes, and no edge across it.
                ERROR => operands.push((Vec::new(), None, n.text_range())),
                _ => {}
            }
        }
        let mut attrs = scope.edge.clone();
        attrs.extend(self.attr_lists(stmt, Target::Edge));
        for pair in operands.windows(2) {
            let ((from, tail, a), (to, head, b)) = (&pair[0], &pair[1]);
            let range = a.cover(*b);
            for &f in from {
                for &t in to {
                    self.edges.push(PendingEdge {
                        from: f,
                        to: t,
                        ports: [tail.clone(), head.clone()],
                        attrs: attrs.clone(),
                        range,
                    });
                }
            }
        }
        operands.into_iter().flat_map(|(n, _, _)| n).collect()
    }

    /// A subgraph: a cluster when its name starts with `cluster`; its defaults are scoped to
    /// it. Returns the nodes mentioned in it.
    fn subgraph(&mut self, sub: &SyntaxNode, parent: &Scope) -> Vec<usize> {
        let name = ids(sub).next().map(|id| value(&id).text);
        let cluster = name.as_ref().is_some_and(|n| {
            n.get(..7)
                .is_some_and(|p| p.eq_ignore_ascii_case("cluster"))
        });
        let mut scope = Scope {
            node: parent.node.clone(),
            edge: parent.edge.clone(),
            graph: parent.graph.clone(),
            cluster: parent.cluster,
            drawn: cluster,
            rank: None,
        };
        if cluster {
            let id = name.unwrap_or_default();
            let c = match self.cluster_index.get(&id) {
                Some(&c) => c,
                None => {
                    let c = self.clusters.len();
                    self.cluster_index.insert(id.clone(), c);
                    // A new cluster starts with the graph attributes set around it so far.
                    self.clusters.push(Cluster {
                        id,
                        parent: parent.cluster,
                        attrs: parent.graph.clone(),
                        range: sub.text_range(),
                    });
                    c
                }
            };
            scope.cluster = Some(c);
        }
        let nodes = self.stmts(sub, &mut scope);
        if let Some(rank) = scope.rank.take() {
            self.rank(&rank, &nodes);
        }
        nodes
    }

    /// A subgraph's `rank`: `same` puts its nodes in one rank, and `min`, `max`, `source`, and
    /// `sink` in the first or last (DOT-2).
    fn rank(&mut self, attr: &Attr, nodes: &[usize]) {
        let value = attr.value.text.trim().to_ascii_lowercase();
        let end = match value.as_str() {
            "same" => None,
            "min" => Some(RankEnd::Min),
            "max" => Some(RankEnd::Max),
            "source" => Some(RankEnd::Source),
            "sink" => Some(RankEnd::Sink),
            _ => {
                self.warn_once(
                    &format!("rank@{:?}", attr.range),
                    attr.range,
                    format!("unknown `rank` `{value}`: expected same, min, max, source, or sink"),
                );
                return;
            }
        };
        let mut group = nodes.to_vec();
        group.sort_unstable();
        group.dedup();
        if group.len() > 1 || end.is_some() && !group.is_empty() {
            self.rank_groups.push((end, group, attr.range));
        }
    }

    fn finish(mut self) -> Output {
        let mut chart = Flowchart::default();
        let graph = self.name.clone();
        let root = std::mem::take(&mut self.root_attrs);
        if let Some(a) = get(&root, "rankdir") {
            match a.value.text.to_ascii_uppercase().as_str() {
                "TB" => chart.direction = Direction::TopBottom,
                "LR" => chart.direction = Direction::LeftRight,
                "BT" => chart.direction = Direction::BottomTop,
                "RL" => chart.direction = Direction::RightLeft,
                v => {
                    let range = a.range;
                    self.warning(
                        range,
                        format!("unknown `rankdir` `{v}`: expected TB, LR, BT, or RL"),
                    );
                }
            }
        }
        self.horizontal = matches!(chart.direction, Direction::LeftRight | Direction::RightLeft);
        if let Some(a) = get(&root, "label") {
            let title = self.label_text(&a.value.clone(), &graph, "");
            chart.title = Some(title).filter(|t| !t.trim().is_empty());
        }
        let background = get(&root, "bgcolor")
            .cloned()
            .and_then(|a| self.color_of(&a));

        self.compound =
            get(&root, "compound").is_some_and(|a| boolean(&a.value.text) == Some(true));
        self.name_clusters();
        let graph_order = get(&root, "ordering").cloned();
        let mut groups: HashMap<String, usize> = HashMap::new();
        for i in 0..self.ids.len() {
            if let Some(name) = get_text(&self.node_attrs[i], "group").filter(|g| !g.is_empty()) {
                let g = *groups.entry(name.to_owned()).or_insert_with(|| {
                    chart.groups.push(Vec::new());
                    chart.groups.len() - 1
                });
                chart.groups[g].push(self.ids[i].clone());
            }
            // A node's own `ordering`, else the graph's.
            let own = get(&self.node_attrs[i], "ordering").cloned();
            if let Some(order) = own
                .or_else(|| graph_order.clone())
                .and_then(|a| self.ordering(&a))
            {
                chart.ordering.push((self.ids[i].clone(), order));
            }
            let node = self.lower_node(i, &graph);
            chart.nodes.push(node);
        }
        let edges = self.merge_edges();
        for e in edges {
            let edge = self.lower_edge(&e, &graph);
            chart.edges.push(edge);
        }
        // Each cluster's members: its nodes in order, then the clusters directly inside it.
        let mut members = vec![Vec::new(); self.clusters.len()];
        for (i, c) in self.node_cluster.iter().enumerate() {
            if let Some(c) = *c {
                members[c].push(self.ids[i].clone());
            }
        }
        for k in &self.clusters {
            if let Some(p) = k.parent {
                members[p].push(k.id.clone());
            }
        }
        for (c, members) in members.into_iter().enumerate() {
            let sub = self.lower_cluster(c, &graph, members);
            chart.subgraphs.push(sub);
        }
        for (end, group, range) in std::mem::take(&mut self.rank_groups) {
            // Only the nodes in one cluster share a rank, as in Graphviz.
            let cluster = self.node_cluster[group[0]];
            if group.iter().any(|&i| self.node_cluster[i] != cluster) {
                let value = match end {
                    None => "same",
                    Some(RankEnd::Min) => "min",
                    Some(RankEnd::Max) => "max",
                    Some(RankEnd::Source) => "source",
                    Some(RankEnd::Sink) => "sink",
                };
                self.warn_once(
                    &format!("rank-clusters@{range:?}"),
                    range,
                    format!("`rank={value}` nodes in different clusters do not share a rank"),
                );
            }
            let ids = group.into_iter().map(|i| self.ids[i].clone()).collect();
            match end {
                None => chart.same_rank.push(ids),
                Some(end) => chart.end_ranks.push((end, ids)),
            }
        }
        Output {
            flowchart: chart,
            diagnostics: self.diagnostics,
            background,
        }
    }

    /// Renames each cluster that shares a node's name: Graphviz names nodes and subgraphs
    /// apart, and the graph model does not.
    fn name_clusters(&mut self) {
        for c in 0..self.clusters.len() {
            if !self.node_index.contains_key(&self.clusters[c].id) {
                continue;
            }
            let mut id = self.clusters[c].id.clone();
            while self.node_index.contains_key(&id) || self.cluster_index.contains_key(&id) {
                id.push('_');
            }
            self.cluster_index.insert(id.clone(), c);
            self.clusters[c].id = id;
        }
    }

    /// A strict graph's repeated edges as one, with their attributes combined (DOT-2).
    fn merge_edges(&mut self) -> Vec<PendingEdge> {
        let edges = std::mem::take(&mut self.edges);
        if !self.strict {
            return edges;
        }
        let mut out: Vec<PendingEdge> = Vec::new();
        let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
        for e in edges {
            let key = if self.directed || e.from <= e.to {
                (e.from, e.to)
            } else {
                (e.to, e.from)
            };
            match seen.get(&key) {
                Some(&k) => out[k].attrs.extend(e.attrs),
                None => {
                    seen.insert(key, out.len());
                    out.push(e);
                }
            }
        }
        out
    }

    fn label_text(&mut self, v: &Value, graph: &str, node: &str) -> String {
        if v.html {
            return html_text(&v.text);
        }
        let names = Names {
            graph,
            node,
            tail: "",
            head: "",
            directed: self.directed,
        };
        escape(&v.text, &names)
    }

    /// The color `a` names. An unknown one is a warning, once however many nodes or edges
    /// the attribute applies to.
    fn color_of(&mut self, a: &Attr) -> Option<Color> {
        let c = color(&a.value.text);
        if c.is_none() {
            let v = a.value.text.clone();
            self.warn_once(
                &format!("color@{:?}", a.range),
                a.range,
                format!("unknown color `{v}`; ignored"),
            );
        }
        c
    }

    /// The `style` attribute's words, lowercase.
    fn styles_of(attrs: &[Attr]) -> Vec<String> {
        get_text(attrs, "style")
            .map(|s| {
                s.split(',')
                    .map(|w| {
                        // `setlinewidth(2)` and the like take an argument.
                        let w = w.split('(').next().unwrap_or("");
                        w.trim().to_ascii_lowercase()
                    })
                    .filter(|w| !w.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Warns about each style word not in `known`.
    fn check_styles(&mut self, attrs: &[Attr], words: &[String], known: &[&str]) {
        let Some(a) = get(attrs, "style") else {
            return;
        };
        let range = a.range;
        for w in words {
            if !known.contains(&w.as_str()) {
                self.warn_once(
                    &format!("style:{w}"),
                    range,
                    format!("the style `{w}` is not supported; ignored"),
                );
            }
        }
    }

    fn lower_node(&mut self, i: usize, graph: &str) -> Node {
        let attrs = std::mem::take(&mut self.node_attrs[i]);
        let id = self.ids[i].clone();
        let shape_name = get_text(&attrs, "shape").map(str::to_ascii_lowercase);
        let mut shape = match shape_name.as_deref() {
            None => Shape::Ellipse,
            Some(name) => match shape(name) {
                Some(s) => s,
                None => {
                    let range = get(&attrs, "shape").map_or_else(TextRange::default, |a| a.range);
                    self.warn_once(
                        &format!("shape:{name}"),
                        range,
                        format!("the shape `{name}` is not supported; drawn as an ellipse"),
                    );
                    Shape::Ellipse
                }
            },
        };
        let record = matches!(shape_name.as_deref(), Some("record" | "mrecord"));
        let raw = get(&attrs, "label")
            .map(|a| a.value.clone())
            .unwrap_or(Value {
                text: "\\N".into(),
                html: false,
            });
        let mut sections = Vec::new();
        let label = if shape == Shape::FilledCircle {
            String::new()
        } else if record && !raw.html {
            let fields = record_fields(&raw.text);
            let mut texts = |f: &Field| {
                let mut out = Vec::new();
                leaves(f, &mut out);
                out.into_iter()
                    .map(|(t, port)| {
                        let text = self.label_text(
                            &Value {
                                text: t,
                                html: false,
                            },
                            graph,
                            &id,
                        );
                        (text, port)
                    })
                    .collect::<Vec<_>>()
            };
            // Each port's line: the title's lines, then the sections' rows (Edge::rows).
            let mut ports = HashMap::new();
            let mut row = 0;
            let mut name = |port: Option<String>, row: usize| {
                if let Some(p) = port {
                    ports.entry(p).or_insert(row);
                }
            };
            let title = fields.first().map(&mut texts).unwrap_or_default();
            for (t, port) in &title {
                name(port.clone(), row);
                row += t.split('\n').count();
            }
            for f in fields.iter().skip(1) {
                let mut rows = Vec::new();
                for (t, port) in texts(f) {
                    name(port, row);
                    let lines: Vec<Row> = t.split('\n').map(Row::plain).collect();
                    row += lines.len();
                    rows.extend(lines);
                }
                sections.push(rows);
            }
            self.record_ports[i] = ports;
            title
                .into_iter()
                .map(|(t, _)| t)
                .collect::<Vec<_>>()
                .join("\n")
        } else if raw.html
            && let Some(mut table) = html_table(&raw.text)
        {
            // The first row is the title, its cells a space apart; the other rows are
            // compartment rows, one cell per column (LANGS-3).
            if shape == Shape::Text {
                shape = Shape::Rect;
            }
            let first = table[0].remove(0);
            let mut ports = HashMap::new();
            let title = first
                .iter()
                .map(|(t, _)| t.as_str())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            for (_, port) in first {
                if let Some(p) = port {
                    ports.entry(p).or_insert(0);
                }
            }
            let mut row = title.split('\n').count();
            for rows in table.into_iter().filter(|rows| !rows.is_empty()) {
                let mut lines = Vec::new();
                for cells in rows {
                    for (_, port) in &cells {
                        if let Some(p) = port {
                            ports.entry(p.clone()).or_insert(row);
                        }
                    }
                    lines.push(Row {
                        cells: cells
                            .into_iter()
                            .map(|(t, _)| t.replace('\n', " "))
                            .collect(),
                        ..Row::default()
                    });
                    row += 1;
                }
                sections.push(lines);
            }
            self.record_ports[i] = ports;
            title
        } else {
            self.label_text(&raw, graph, &id)
        };

        let words = Self::styles_of(&attrs);
        self.check_styles(
            &attrs,
            &words,
            &[
                "filled", "rounded", "dashed", "dotted", "bold", "solid", "invis",
            ],
        );
        if words.iter().any(|w| w == "rounded") && shape == Shape::Rect {
            shape = Shape::Rounded;
        }
        let styles = self.shape_styles(&attrs, &words, false);
        Node {
            id,
            label: Label::plain(label),
            shape,
            sections,
            classes: Vec::new(),
            styles,
            click: None,
            tooltip: None,
            metadata: Vec::new(),
            mentions: std::mem::take(&mut self.mentions[i]),
        }
    }

    /// The fill, stroke, text color, width, and dashes of a node or cluster. A cluster also
    /// has `pencolor` for its border and `bgcolor` for its fill.
    fn shape_styles(&mut self, attrs: &[Attr], words: &[String], cluster: bool) -> Vec<Style> {
        let has = |w: &str| words.iter().any(|x| x == w);
        let mut out = Vec::new();
        let mut color_at = |key: &str| get(attrs, key).cloned().and_then(|a| self.color_of(&a));
        let line = color_at("color");
        let stroke = cluster.then(|| color_at("pencolor")).flatten().or(line);
        let fill = color_at("fillcolor");
        let bg = cluster.then(|| color_at("bgcolor")).flatten();
        if has("filled") {
            let c = fill.or(line).or(bg).unwrap_or(Color::rgb(0xd3d3d3));
            out.push(style("fill", hex(c)));
        } else if let Some(c) = bg {
            out.push(style("fill", hex(c)));
        }
        if let Some(c) = stroke {
            out.push(style("stroke", hex(c)));
        }
        if let Some(c) = color_at("fontcolor") {
            out.push(style("color", hex(c)));
        }
        let width = get_text(attrs, "penwidth").and_then(|w| w.trim().parse::<f32>().ok());
        if let Some(w) = width.filter(|w| w.is_finite() && *w >= 0.0) {
            out.push(style("stroke-width", format!("{w}px")));
        } else if has("bold") {
            out.push(style("stroke-width", "2px"));
        }
        if has("dashed") {
            out.push(style("stroke-dasharray", "5 5"));
        } else if has("dotted") {
            out.push(style("stroke-dasharray", "2 3"));
        }
        if has("invis") {
            out.push(style("fill", "none"));
            out.push(style("stroke", "none"));
            out.push(style("color", "none"));
        }
        out
    }

    fn arrow_attr(&mut self, attrs: &[Attr], key: &str) -> Option<Arrow> {
        let Some(a) = get(attrs, key) else {
            return Some(Arrow::Point);
        };
        let name = a.value.text.trim().to_ascii_lowercase();
        match arrow(&name) {
            Some(m) => m,
            None => {
                let range = a.range;
                self.warn_once(
                    &format!("arrow:{name}"),
                    range,
                    format!("unknown arrow `{name}`; drawn as `normal`"),
                );
                Some(Arrow::Point)
            }
        }
    }

    /// An `ordering` value: `out`, `in`, or empty for none (DOT-3).
    fn ordering(&mut self, a: &Attr) -> Option<EdgeOrder> {
        let v = a.value.text.trim().to_ascii_lowercase();
        match v.as_str() {
            "out" => Some(EdgeOrder::Out),
            "in" => Some(EdgeOrder::In),
            "" => None,
            _ => {
                self.warn_once(
                    &format!("ordering@{:?}", a.range),
                    a.range,
                    format!("unknown `ordering` `{v}`: expected out or in"),
                );
                None
            }
        }
    }

    /// The cluster that `key` (`lhead` or `ltail`) names, when `compound` is set and the cluster
    /// holds the edge's `end` node and not its `other` (DOT-3).
    fn clip(&mut self, attrs: &[Attr], key: &str, end: usize, other: usize) -> Option<String> {
        let a = get(attrs, key)?;
        let (range, name) = (a.range, a.value.text.clone());
        if !self.compound {
            self.warn_once(
                &format!("{key}:compound"),
                range,
                format!("`{key}` needs `compound=true` on the graph; ignored"),
            );
            return None;
        }
        let Some(&c) = self.cluster_index.get(&name) else {
            self.warn_once(
                &format!("{key}@{range:?}"),
                range,
                format!("`{key}` `{name}` is not a cluster; ignored"),
            );
            return None;
        };
        let inside = |n: usize| {
            let mut k = self.node_cluster[n];
            while let Some(x) = k {
                if x == c {
                    return true;
                }
                k = self.clusters[x].parent;
            }
            false
        };
        if !inside(end) || inside(other) {
            let side = if key == "lhead" { "head" } else { "tail" };
            self.warn_once(
                &format!("{key}@{range:?}"),
                range,
                format!("`{key}` `{name}` does not hold the edge's {side} alone; ignored"),
            );
            return None;
        }
        Some(self.clusters[c].id.clone())
    }

    fn lower_edge(&mut self, e: &PendingEdge, graph: &str) -> Edge {
        let attrs = &e.attrs;
        let (tail, head) = (self.ids[e.from].clone(), self.ids[e.to].clone());
        let from = self.clip(attrs, "ltail", e.from, e.to);
        let to = self.clip(attrs, "lhead", e.to, e.from);
        // A port written on the node, else `tailport` or `headport` (`port:compass`, `port`,
        // or `compass`).
        let mut ports = [None; 2];
        let mut rows = [None; 2];
        for (k, key) in ["tailport", "headport"].into_iter().enumerate() {
            let port = e.ports[k].clone().or_else(|| {
                get(attrs, key).map(|a| Port {
                    parts: match a.value.text.rsplit_once(':') {
                        Some((p, x)) => vec![p.to_owned(), x.to_owned()],
                        None => vec![a.value.text.clone()],
                    },
                    range: a.range,
                })
            });
            if let Some(port) = port {
                let node = if k == 0 { e.from } else { e.to };
                (rows[k], ports[k]) = self.port(node, &port);
            }
        }
        // An end clipped to a cluster joins its border, not a record line.
        if from.is_some() {
            rows[0] = None;
        }
        if to.is_some() {
            rows[1] = None;
        }
        if rows.iter().any(Option::is_some) && !self.horizontal {
            self.warn_once(
                "<rows>",
                e.range,
                "record ports are drawn only when `rankdir` is LR or RL; the edge joins the node",
            );
        }
        let dir = match get(attrs, "dir") {
            None => (if self.directed { "forward" } else { "none" }).to_owned(),
            Some(a) => {
                let d = a.value.text.trim().to_ascii_lowercase();
                if matches!(d.as_str(), "forward" | "back" | "both" | "none") {
                    d
                } else {
                    let range = a.range;
                    self.warn_once(
                        &format!("dir@{range:?}"),
                        range,
                        format!("unknown `dir` `{d}`: expected forward, back, both, or none"),
                    );
                    (if self.directed { "forward" } else { "none" }).to_owned()
                }
            }
        };
        let end = matches!(dir.as_str(), "forward" | "both")
            .then(|| self.arrow_attr(attrs, "arrowhead"))
            .flatten();
        let start = matches!(dir.as_str(), "back" | "both")
            .then(|| self.arrow_attr(attrs, "arrowtail"))
            .flatten();

        let names = Names {
            graph,
            node: "",
            tail: &tail,
            head: &head,
            directed: self.directed,
        };
        let label = |key: &str| {
            get(attrs, key).map(|a| {
                Label::plain(if a.value.html {
                    html_text(&a.value.text)
                } else {
                    escape(&a.value.text, &names)
                })
            })
        };
        let (label, start_label, end_label) =
            (label("label"), label("taillabel"), label("headlabel"));

        let words = Self::styles_of(attrs);
        self.check_styles(
            attrs,
            &words,
            &["dashed", "dotted", "bold", "solid", "invis"],
        );
        let has = |w: &str| words.iter().any(|x| x == w);
        let stroke = if has("invis") {
            Stroke::Invisible
        } else if has("dashed") || has("dotted") {
            Stroke::Dotted
        } else if has("bold") {
            Stroke::Thick
        } else {
            Stroke::Normal
        };
        let mut styles = Vec::new();
        if let Some(c) = get(attrs, "color").cloned().and_then(|a| self.color_of(&a)) {
            styles.push(style("stroke", hex(c)));
        }
        if let Some(c) = get(attrs, "fontcolor")
            .cloned()
            .and_then(|a| self.color_of(&a))
        {
            styles.push(style("color", hex(c)));
        }
        let width = get_text(attrs, "penwidth").and_then(|w| w.trim().parse::<f32>().ok());
        if let Some(w) = width.filter(|w| w.is_finite() && *w >= 0.0) {
            styles.push(style("stroke-width", format!("{w}px")));
        }
        let length = get_text(attrs, "minlen")
            .and_then(|m| m.trim().parse::<f32>().ok())
            .filter(|m| !m.is_nan())
            .map_or(1, |m| m.round().clamp(0.0, 10.0) as u8);
        let constraint = match get(attrs, "constraint") {
            None => true,
            Some(a) => boolean(&a.value.text).unwrap_or_else(|| {
                let range = a.range;
                self.warn_once(
                    &format!("constraint@{range:?}"),
                    range,
                    format!(
                        "`constraint` `{}` is not a boolean; read as true",
                        a.value.text
                    ),
                );
                true
            }),
        };
        Edge {
            id: None,
            from: from.unwrap_or(tail),
            to: to.unwrap_or(head),
            stroke,
            start,
            end,
            length,
            constraint,
            ports,
            rows,
            label,
            start_label,
            end_label,
            classes: Vec::new(),
            styles,
            curve: None,
            animate: false,
            animation: None,
            metadata: Vec::new(),
            range: e.range,
        }
    }

    /// Cluster `c` with its member nodes and clusters.
    fn lower_cluster(&mut self, c: usize, graph: &str, members: Vec<String>) -> Subgraph {
        let attrs = std::mem::take(&mut self.clusters[c].attrs);
        let title = get(&attrs, "label")
            .map(|a| a.value.clone())
            .map(|v| self.label_text(&v, graph, ""))
            .unwrap_or_default();
        let words = Self::styles_of(&attrs);
        self.check_styles(
            &attrs,
            &words,
            &[
                "filled", "rounded", "dashed", "dotted", "bold", "solid", "invis",
            ],
        );
        let styles = self.shape_styles(&attrs, &words, true);
        Subgraph {
            id: self.clusters[c].id.clone(),
            title: Label::plain(title),
            direction: None,
            members,
            classes: Vec::new(),
            styles,
            metadata: Vec::new(),
            range: self.clusters[c].range,
            region: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Names<'static> {
        Names {
            graph: "G",
            node: "n",
            tail: "a",
            head: "b",
            directed: true,
        }
    }

    #[test]
    fn escapes() {
        let e = |s: &str| escape(s, &names());
        assert_eq!(e("\\N in \\G"), "n in G");
        assert_eq!(e("one\\ltwo\\l"), "one\ntwo");
        assert_eq!(e("\\E: \\T to \\H"), "a->b: a to b");
        assert_eq!(e("a\\nb\\rc"), "a\nb\nc");
        assert_eq!(e("x\\\\y\\q"), "x\\yq");
        assert_eq!(unquote("\"say \\\"hi\\\"\""), "say \"hi\"");
        assert_eq!(unquote("\"a\\\nb\""), "ab");
        // An escaped backslash is not the start of a name escape.
        assert_eq!(e("\\\\N and \\\\n"), "\\N and \\n");
        assert_eq!(e("end\\"), "end\\");
        // A name's own escapes are read where it is substituted.
        let named = Names {
            node: "a\\nb",
            graph: "g\\\\N",
            ..names()
        };
        assert_eq!(escape("\\N", &named), "a\nb");
        assert_eq!(escape("\\G", &named), "g\\N");
        // Entities: named, decimal, and hex, decoded once; `#` codes are text.
        assert_eq!(e("a &amp; b &lt;&#169;&#xA9;"), "a & b <\u{a9}\u{a9}");
        assert_eq!(e("&amp;lt; #amp; & x;"), "&lt; #amp; & x;");
        // Decoded after names are put in, before line breaks are read.
        let amp = Names {
            node: "x&amp;y",
            ..names()
        };
        assert_eq!(escape("\\N", &amp), "x&y");
        assert_eq!(e("a&#92;nb"), "a\nb");
        // An unclosed string keeps an escaped quote at its end.
        assert_eq!(unquote("\"a\\\""), "a\"");
        assert_eq!(unquote("\"a"), "a");
    }

    #[test]
    fn html() {
        assert_eq!(html_text("<b>bold</b> &amp; plain"), "bold & plain");
        assert_eq!(html_text("one<BR/>two<br align=\"left\"/>"), "one\ntwo");
        assert_eq!(html_text("<table><tr><td>a</td></tr></table>"), "a");
        assert_eq!(
            html_text("<TABLE>\n  <TR><TD>a</TD><TD>b</TD></TR>\n  <TR><TD>c</TD></TR>\n</TABLE>"),
            "a b\nc"
        );
        assert_eq!(
            html_text(
                "<table><tr><td><table><tr><td>in</td></tr></table></td></tr><tr><td>out</td></tr></table>"
            ),
            "in\nout"
        );
        // An entity is decoded once, and is not a tag.
        assert_eq!(html_text("&lt;br/&gt; &amp;lt;"), "<br/> &lt;");
        // Unclosed and empty tags, with multibyte text at the cut.
        assert_eq!(html_text("a<é"), "a");
        assert_eq!(html_text("é<"), "é");
        assert_eq!(html_text("<>é</>"), "é");
        assert_eq!(html_text(""), "");
        // Numeric entities, and `&nbsp;` padding kept while source indentation is not.
        assert_eq!(html_text("&#9829; &#x2665;"), "\u{2665} \u{2665}");
        assert_eq!(html_text("\n  &nbsp;a&nbsp;\n"), "\u{a0}a\u{a0}");
    }

    #[test]
    fn html_tables() {
        assert_eq!(tag_attr("TD PORT=\"a b\"", "port").as_deref(), Some("a b"));
        assert_eq!(
            tag_attr("td port='x' bgcolor=red", "port").as_deref(),
            Some("x")
        );
        assert_eq!(tag_attr("td port=x/", "port").as_deref(), Some("x"));
        assert_eq!(tag_attr("td import=x", "port"), None);
        assert_eq!(tag_attr("td port", "port"), None);
        let cell = |t: &str| (t.to_owned(), None);
        assert_eq!(
            html_table("<table><tr><td>a</td><td>b</td></tr><hr/><tr><td>c</td></tr></table>"),
            Some(vec![
                vec![vec![cell("a"), cell("b")]],
                vec![vec![cell("c")]]
            ])
        );
        // Wrapping tags are fine; text beside the table, two tables, or an unclosed one are
        // not a table.
        assert!(html_table("<b><table><tr><td>a</td></tr></table></b>").is_some());
        assert_eq!(html_table("x<table><tr><td>a</td></tr></table>"), None);
        assert_eq!(html_table("<table></table><table></table>"), None);
        assert_eq!(html_table("<table><tr><td>a</td></tr>"), None);
        assert_eq!(html_table("<table><tr><td>a"), None);
        assert_eq!(html_table("<table></table>"), None);
        assert_eq!(html_table("</table><table>"), None);
    }

    #[test]
    fn record_nesting_is_bounded() {
        let deep = "{".repeat(100_000);
        let fields = record_fields(&deep);
        let mut out = Vec::new();
        fields.iter().for_each(|f| leaves(f, &mut out));
        // The braces past the limit are text.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0.len(), 100_000 - MAX_RECORD_DEPTH);
    }

    #[test]
    fn records() {
        let t = |s: &str| Field::Text(s.into(), None);
        let p = |s: &str, port: &str| Field::Text(s.into(), Some(port.into()));
        assert_eq!(record_fields("a|b|c"), [t("a"), t("b"), t("c")]);
        assert_eq!(record_fields("{a|b}"), [t("a"), t("b")]);
        assert_eq!(
            record_fields("<f0> left|{<f1> mid\\|dle|x}|right"),
            [
                p("left", "f0"),
                Field::Group(vec![p("mid|dle", "f1"), t("x")]),
                t("right")
            ]
        );
        // A blank port name is none; the last of two names wins.
        assert_eq!(record_fields("< > a|<x><y> b"), [t("a"), p("b", "y")]);
    }

    #[test]
    fn colors() {
        assert_eq!(color("red"), Some(Color::rgb(0xff0000)));
        assert_eq!(color("LightGrey"), Some(Color::rgb(0xd3d3d3)));
        assert_eq!(color("gray50"), Some(Color::rgb(0x7f7f7f)));
        assert_eq!(color("/x11/navy"), Some(Color::rgb(0x000080)));
        assert_eq!(
            color("#ff000080"),
            Some(Color::rgb(0xff0000).with_alpha(0x80))
        );
        assert_eq!(color("red:blue"), Some(Color::rgb(0xff0000)));
        assert_eq!(color("0.0 1.0 1.0"), Some(Color::rgb(0xff0000)));
        assert_eq!(color("0.6667,1,1"), Some(Color::rgb(0x0000ff)));
        assert_eq!(color("teal"), Some(Color::rgb(0x008080)));
        assert_eq!(color("/blues9/3"), None);
        assert_eq!(color("nope"), None);
        // The X11 table is sorted for binary search.
        assert!(X11.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn arrows() {
        assert_eq!(arrow("normal"), Some(Some(Arrow::Point)));
        assert_eq!(arrow("empty"), Some(Some(Arrow::Triangle)));
        assert_eq!(arrow("odiamond"), Some(Some(Arrow::HollowDiamond)));
        assert_eq!(arrow("invodot"), Some(Some(Arrow::Point)));
        assert_eq!(arrow("odot"), Some(Some(Arrow::Circle)));
        assert_eq!(arrow("crow"), Some(Some(Arrow::OneMany)));
        assert_eq!(arrow("tee"), Some(Some(Arrow::One)));
        assert_eq!(arrow("lvee"), Some(Some(Arrow::Point)));
        assert_eq!(arrow("none"), Some(None));
        assert_eq!(arrow("open"), Some(Some(Arrow::Point)));
        assert_eq!(arrow("bogus"), None);
    }
}
