//! PlantUML network diagrams (PNW-*): `nwdiag { ... }` networks, the hosts on them, groups, and
//! peer links, lowered to the graph model. A network is a box linked to its hosts, each link
//! labeled with the host's address there.

use rowan::TextRange;

use super::description::ELEMENTS;
use super::lines::{self, Common, Cursor, Line};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Edge, Flowchart, Label, Node, Shape, Style, Subgraph};
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str = "expected `network`, `group`, a host such as \
     `web01 [address = \"10.0.0.1\"]`, or a link such as `a -- b`";

fn style(property: &str, value: impl Into<String>) -> Style {
    Style {
        property: property.into(),
        value: value.into(),
    }
}

/// One statement: a piece of a line between `;`, `{`, and `}`.
struct Stmt<'a> {
    text: &'a str,
    range: TextRange,
}

/// Splits `line` at `;`, after `{`, and around `}`, outside quotes and `[...]`.
fn split(line: &Line) -> Vec<Stmt<'_>> {
    let t = line.text.as_str();
    let mut spans = Vec::new();
    let (mut quoted, mut bracket, mut start) = (false, 0usize, 0);
    for (i, c) in t.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '[' if !quoted => bracket += 1,
            ']' if !quoted => bracket = bracket.saturating_sub(1),
            ';' if !quoted && bracket == 0 => {
                spans.push(start..i);
                start = i + 1;
            }
            '{' if !quoted && bracket == 0 => {
                spans.push(start..i + 1);
                start = i + 1;
            }
            '}' if !quoted && bracket == 0 => {
                spans.push(start..i);
                spans.push(i..i + 1);
                start = i + 1;
            }
            _ => {}
        }
    }
    spans.push(start..t.len());
    spans
        .into_iter()
        .map(|r| t[r].trim())
        .filter(|s| !s.is_empty())
        .map(|text| Stmt {
            text,
            range: line.range_of(text),
        })
        .collect()
}

/// Splits `s` at `,` outside quotes and `[...]`.
fn commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut quoted, mut bracket, mut start) = (false, 0usize, 0);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '[' if !quoted => bracket += 1,
            ']' if !quoted => bracket = bracket.saturating_sub(1),
            ',' if !quoted && bracket == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out.retain(|p| !p.is_empty());
    out
}

/// `key = value`, the value unquoted.
fn attribute(s: &str) -> Option<(String, &str)> {
    let (k, v) = s.split_once('=')?;
    let k = k.trim();
    (!k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then(|| (k.to_ascii_lowercase(), lines::unquote(v.trim())))
}

enum Block {
    Root,
    Network(String),
    Group(usize),
}

#[derive(Default)]
struct Host {
    address: Option<String>,
    shape: Option<Shape>,
    description: Option<String>,
    color: Option<String>,
}

struct Lower {
    chart: Flowchart,
    d: Vec<Diagnostic>,
    blocks: Vec<(Block, TextRange)>,
    /// Hosts already on a network, whose later networks hang below them.
    attached: Vec<String>,
    networks: usize,
}

impl Lower {
    fn host(&mut self, id: &str, range: TextRange) -> &mut Node {
        let at = match self.chart.nodes.iter().position(|n| n.id == id) {
            Some(at) => at,
            None => {
                let mut n = Node::new(id, Label::plain(id), Shape::Rect);
                n.mentions.push(range);
                self.chart.nodes.push(n);
                self.chart.nodes.len() - 1
            }
        };
        &mut self.chart.nodes[at]
    }

    /// `[key = value, ...]` after a host's name.
    fn host_attributes(&mut self, s: &str, range: TextRange) -> Host {
        let mut h = Host::default();
        for part in commas(s) {
            let Some((k, v)) = attribute(part) else {
                self.d.push(Diagnostic::new(
                    range,
                    format!("expected `key = value`, not `{part}`"),
                ));
                continue;
            };
            match k.as_str() {
                "address" => h.address = Some(v.to_owned()),
                "description" => h.description = Some(lines::text(v)),
                "color" => h.color = lines::color(v),
                "shape" => match ELEMENTS.iter().find(|(n, _)| n.eq_ignore_ascii_case(v)) {
                    Some(&(_, s)) => h.shape = Some(s),
                    None => self.d.push(Diagnostic::warning(
                        range,
                        format!("the shape `{v}` is not drawn; drawing a box"),
                    )),
                },
                _ => self.d.push(Diagnostic::warning(
                    range,
                    format!("the attribute `{k}` is not drawn"),
                )),
            }
        }
        h
    }

    /// A host statement: `name`, `name [attributes]`, or in a group `a, b`.
    fn host_stmt(&mut self, s: &Stmt) {
        let block = self.blocks.last().map(|(b, _)| b);
        let names = match block {
            Some(Block::Group(_)) => commas(s.text),
            _ => vec![s.text],
        };
        for part in names {
            let (name, attrs) = match part.find('[') {
                Some(at) => {
                    let Some(body) = part[at + 1..].trim_end().strip_suffix(']') else {
                        self.d
                            .push(Diagnostic::new(s.range, "`[` has no closing `]`"));
                        continue;
                    };
                    (part[..at].trim(), Some(body))
                }
                None => (part.trim(), None),
            };
            let id = lines::unquote(name).to_owned();
            if id.is_empty() || id.contains(char::is_whitespace) {
                self.d.push(Diagnostic::new(s.range, STATEMENT));
                continue;
            }
            let h = attrs.map_or_else(Host::default, |a| self.host_attributes(a, s.range));
            let node = self.host(&id, s.range);
            if let Some(shape) = h.shape {
                node.shape = shape;
            }
            if let Some(text) = h.description {
                node.label = Label::plain(text);
            }
            if let Some(c) = h.color {
                node.styles.push(style("fill", c));
            }
            match self.blocks.last().map(|(b, _)| b) {
                Some(Block::Network(net)) => {
                    let net = net.clone();
                    // The host's first network sits above it; later ones hang below it.
                    let mut e = if self.attached.contains(&id) {
                        Edge::new(&id, &net, s.range)
                    } else {
                        self.attached.push(id.clone());
                        Edge::new(&net, &id, s.range)
                    };
                    e.end = None;
                    e.label = h.address.map(Label::plain);
                    self.chart.edges.push(e);
                }
                Some(&Block::Group(g)) => {
                    let members = &mut self.chart.subgraphs[g].members;
                    if !members.contains(&id) {
                        members.push(id);
                    }
                    if h.address.is_some() {
                        self.d.push(Diagnostic::warning(
                            s.range,
                            "an address outside a network is not drawn",
                        ));
                    }
                }
                _ => {
                    if h.address.is_some() {
                        self.d.push(Diagnostic::warning(
                            s.range,
                            "an address outside a network is not drawn",
                        ));
                    }
                }
            }
        }
    }

    /// `key = value` inside a network or group.
    fn block_attribute(&mut self, k: &str, v: &str, range: TextRange) {
        match self.blocks.last().map(|(b, _)| b) {
            Some(Block::Network(net)) => {
                let net = net.clone();
                let node = self.host(&net, range);
                match k {
                    "address" if node.label.text.is_empty() => node.label.text = v.to_owned(),
                    "address" => node.label.text = format!("{}\n{v}", node.label.text),
                    "description" => {
                        let address = node.label.text.split_once('\n').map(|(_, a)| a.to_owned());
                        node.label.text = match address {
                            Some(a) => format!("{}\n{a}", lines::text(v)),
                            None => lines::text(v),
                        };
                    }
                    "color" => {
                        if let Some(c) = lines::color(v) {
                            node.styles.push(style("fill", c));
                        }
                    }
                    "width" => {}
                    _ => self.d.push(Diagnostic::warning(
                        range,
                        format!("the network attribute `{k}` is not drawn"),
                    )),
                }
            }
            Some(&Block::Group(g)) => match k {
                "description" => self.chart.subgraphs[g].title = Label::plain(lines::text(v)),
                "color" => {
                    if let Some(c) = lines::color(v) {
                        self.chart.subgraphs[g].styles.push(style("fill", c));
                    }
                }
                _ => self.d.push(Diagnostic::warning(
                    range,
                    format!("the group attribute `{k}` is not drawn"),
                )),
            },
            _ => self.d.push(Diagnostic::warning(
                range,
                format!("the diagram attribute `{k}` is not drawn"),
            )),
        }
    }

    /// `a -- b -- c`.
    fn link(&mut self, s: &Stmt) -> bool {
        if !s.text.contains("--") {
            return false;
        }
        let ends: Vec<&str> = s.text.split("--").map(str::trim).collect();
        if ends
            .iter()
            .any(|e| e.is_empty() || e.contains(char::is_whitespace))
        {
            return false;
        }
        for pair in ends.windows(2) {
            let (a, b) = (lines::unquote(pair[0]), lines::unquote(pair[1]));
            self.host(a, s.range);
            self.host(b, s.range);
            let mut e = Edge::new(a, b, s.range);
            e.end = None;
            self.chart.edges.push(e);
        }
        true
    }

    fn open(&mut self, s: &Stmt, head: &str) {
        let word = lines::word(head).to_ascii_lowercase();
        let name = lines::unquote(head[word.len()..].trim());
        let block = match word.as_str() {
            "nwdiag" if name.is_empty() => Block::Root,
            "network" => {
                self.networks += 1;
                let id = match name {
                    "" => format!("network:{}", self.networks),
                    _ => format!("network:{name}"),
                };
                let node = self.host(&id, s.range);
                node.label = Label::plain(name);
                node.shape = Shape::Rect;
                node.styles.push(style("stroke-width", "3px"));
                Block::Network(id)
            }
            "group" => {
                let id = format!("group:{}", self.chart.subgraphs.len());
                self.chart
                    .subgraphs
                    .push(Subgraph::new(&id, Label::plain(name), s.range));
                Block::Group(self.chart.subgraphs.len() - 1)
            }
            _ => {
                self.d.push(Diagnostic::new(
                    s.range,
                    "expected `nwdiag {`, `network NAME {`, or `group NAME {`",
                ));
                Block::Root
            }
        };
        self.blocks.push((block, s.range));
    }

    fn stmt(&mut self, s: &Stmt) {
        if s.text == "}" {
            if self.blocks.pop().is_none() {
                self.d.push(Diagnostic::new(s.range, "`}` closes no block"));
            }
            return;
        }
        if let Some(head) = s.text.strip_suffix('{') {
            return self.open(s, head.trim());
        }
        if let Some((k, v)) = attribute(s.text)
            && !s.text.contains('[')
        {
            return self.block_attribute(&k, v, s.range);
        }
        if self.link(s) {
            return;
        }
        self.host_stmt(s);
    }
}

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let mut l = Lower {
        chart: Flowchart::default(),
        d: Vec::new(),
        blocks: Vec::new(),
        attached: Vec::new(),
        networks: 0,
    };
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        match lines::common(&line, &mut cursor, &mut l.d) {
            Common::No => {}
            Common::Done | Common::Direction(_) => continue,
            Common::Title(t) => {
                l.chart.title = Some(t);
                continue;
            }
        }
        for s in split(&line) {
            l.stmt(&s);
        }
    }
    for (_, range) in l.blocks.drain(..).rev() {
        l.d.push(Diagnostic::new(range, "`{` has no closing `}`"));
    }
    (l.chart, l.d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements() {
        let line = Line {
            text: "network a { web [address = \"1, 2\"]; db }".into(),
            range: TextRange::default(),
        };
        let parts: Vec<&str> = split(&line).iter().map(|s| s.text).collect();
        assert_eq!(
            parts,
            ["network a {", "web [address = \"1, 2\"]", "db", "}"]
        );
        assert_eq!(
            commas("a, b [x = \"1,2\", y = 3], c"),
            ["a", "b [x = \"1,2\", y = 3]", "c"]
        );
        assert_eq!(
            attribute("Color = \"#red\""),
            Some(("color".into(), "#red"))
        );
        assert_eq!(attribute("a -- b"), None);
    }
}
