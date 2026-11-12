//! PlantUML Chen entity-relationship diagrams (PCHEN-*): `@startchen` entities, relationships,
//! and their attributes, lowered to the graph model as boxes, diamonds, and ellipses.

use std::collections::HashSet;

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Arrow, Edge, Flowchart, Label, Node, Shape, Stroke, Style};
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str = "expected `entity`, `relationship`, or a link such as `A -N- B`";

fn style(property: &str, value: &str) -> Style {
    Style {
        property: property.into(),
        value: value.into(),
    }
}

/// The `<<...>>` stereotypes at the end of `s`, and the text before them.
fn stereotypes(s: &str) -> (&str, Vec<String>) {
    let mut rest = s.trim_end();
    let mut out = Vec::new();
    while let Some(body) = rest.strip_suffix(">>")
        && let Some(open) = body.rfind("<<")
    {
        out.push(body[open + 2..].trim().to_ascii_lowercase());
        rest = body[..open].trim_end();
    }
    out.reverse();
    (rest, out)
}

struct Lower {
    chart: Flowchart,
    d: Vec<Diagnostic>,
    /// Entity and relationship ids, for links.
    named: HashSet<String>,
    attributes: usize,
}

impl Lower {
    fn node(&mut self, id: String, label: Label, shape: Shape, range: TextRange) {
        let mut n = Node::new(&id, label, shape);
        n.mentions.push(range);
        self.chart.nodes.push(n);
    }

    fn line(&mut self, from: &str, to: &str, range: TextRange) -> &mut Edge {
        let mut e = Edge::new(from, to, range);
        e.end = None;
        self.chart.edges.push(e);
        self.chart.edges.last_mut().expect("just pushed")
    }

    /// `entity` or `relationship` and its optional `{` attribute block.
    fn declaration(&mut self, line: &Line, rest: &str, entity: bool, cursor: &mut Cursor) {
        let open = rest.trim_end().ends_with('{');
        let rest = rest.trim_end().trim_end_matches('{');
        let Some((id, label, after)) = lines::name(rest) else {
            return self.d.push(Diagnostic::new(line.range, "expected a name"));
        };
        let (after, kinds) = stereotypes(after);
        if !after.trim().is_empty() {
            self.d.push(Diagnostic::new(
                line.range_of(after.trim()),
                "expected `{` or the end of the line",
            ));
        }
        let strong = kinds.iter().any(|k| k == "weak" || k == "identifying");
        let shape = match (entity, strong) {
            (true, false) => Shape::Rect,
            (true, true) => Shape::FramedRect,
            (false, _) => Shape::Diamond,
        };
        if self.named.contains(&id) {
            self.d.push(Diagnostic::new(
                line.range,
                format!("`{id}` is already declared"),
            ));
        } else {
            self.node(id.clone(), Label::plain(label), shape, line.range);
            if strong && !entity {
                // A double diamond: drawn with a heavier outline.
                if let Some(n) = self.chart.nodes.last_mut() {
                    n.styles.push(style("stroke-width", "3px"));
                }
            }
            self.named.insert(id.clone());
        }
        for k in kinds
            .iter()
            .filter(|k| !matches!(k.as_str(), "weak" | "identifying"))
        {
            self.d.push(Diagnostic::warning(
                line.range,
                format!("the stereotype `<<{k}>>` is not drawn"),
            ));
        }
        if open {
            self.attributes(&id, cursor, line, 1);
        }
    }

    /// Attribute lines up to `}`, each linked to `owner`.
    fn attributes(&mut self, owner: &str, cursor: &mut Cursor, open: &Line, depth: usize) {
        loop {
            let Some(line) = cursor.next() else {
                return self
                    .d
                    .push(Diagnostic::new(open.range, "`{` has no closing `}`"));
            };
            let t = line.text.trim();
            if t == "}" {
                return;
            }
            let block = t.ends_with('{');
            let t = t.trim_end_matches('{').trim_end();
            let (t, kinds) = stereotypes(t);
            let (name, ty) = match t.split_once(':') {
                Some((n, ty)) => (n.trim(), Some(ty.trim())),
                None => (t.trim(), None),
            };
            if name.is_empty() {
                self.d
                    .push(Diagnostic::new(line.range, "expected an attribute name"));
                continue;
            }
            let name = lines::text(lines::unquote(name));
            let text = match ty {
                Some(ty) if !ty.is_empty() => format!("{name} : {ty}"),
                _ => name,
            };
            let key = kinds.iter().any(|k| k == "key");
            let label = if key {
                Label {
                    text: format!("**{text}**"),
                    markdown: true,
                }
            } else {
                Label::plain(text)
            };
            let id = format!("attr:{}", self.attributes);
            self.attributes += 1;
            self.node(id.clone(), label, Shape::Ellipse, line.range);
            let node = self.chart.nodes.last_mut().expect("just pushed");
            for k in &kinds {
                match k.as_str() {
                    "key" => {}
                    "derived" => node.styles.push(style("stroke-dasharray", "5 5")),
                    "multi" => node.styles.push(style("stroke-width", "3px")),
                    other => self.d.push(Diagnostic::warning(
                        line.range,
                        format!("the stereotype `<<{other}>>` is not drawn"),
                    )),
                }
            }
            self.line(owner, &id, line.range);
            if block {
                if depth >= MAX_DEPTH {
                    self.d.push(Diagnostic::new(
                        line.range,
                        format!("attributes nest more than {MAX_DEPTH} levels deep"),
                    ));
                    // Skip the rest of the document's lines.
                    while cursor.next().is_some() {}
                    return;
                }
                self.attributes(&id, cursor, &line, depth + 1);
            }
        }
    }

    /// `A -N- B`, `A =N= B`, `A ->- B`, or `A ->- d { B, C }`.
    fn link(&mut self, line: &Line) -> bool {
        let Some((from, _, rest)) = lines::name(&line.text) else {
            return false;
        };
        let op_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let op = &rest[..op_end];
        let c = match op.chars().next() {
            Some(c @ ('-' | '=')) if op.len() >= 2 && op.ends_with(c) => c,
            _ => return false,
        };
        let inner = &op[1..op.len() - 1];
        let rest = rest[op_end..].trim();
        let stroke = if c == '=' {
            Stroke::Thick
        } else {
            Stroke::Normal
        };
        let subclass = matches!(inner, ">" | "<");
        if subclass && let Some((kind, list)) = rest.split_once('{') {
            // A disjoint (`d`) or overlapping (`U`) specialization of `from` into the list.
            let Some(list) = list.trim_end().strip_suffix('}') else {
                self.d
                    .push(Diagnostic::new(line.range, "`{` has no closing `}`"));
                return true;
            };
            if !self.known(&from, line) {
                return true;
            }
            let id = format!("sub:{}", self.chart.nodes.len());
            self.node(
                id.clone(),
                Label::plain(kind.trim()),
                Shape::Circle,
                line.range,
            );
            self.line(&from, &id, line.range).stroke = stroke;
            for child in list.split(',').map(|c| lines::id_of(c.trim())) {
                if child.is_empty() || !self.known(&child, line) {
                    continue;
                }
                let e = self.line(&id, &child, line.range);
                e.stroke = stroke;
                e.start = Some(Arrow::Point);
            }
            return true;
        }
        let Some((to, _, after)) = lines::name(rest) else {
            self.d.push(Diagnostic::new(
                line.range,
                "expected a name after the link",
            ));
            return true;
        };
        if !after.trim().is_empty() {
            self.d.push(Diagnostic::new(
                line.range_of(after.trim()),
                "expected the end of the line",
            ));
        }
        if !(self.known(&from, line) & self.known(&to, line)) {
            return true;
        }
        let e = self.line(&from, &to, line.range);
        e.stroke = stroke;
        match inner {
            ">" => e.end = Some(Arrow::Point),
            "<" => e.start = Some(Arrow::Point),
            "" => {}
            card => {
                let card = card.trim_start_matches('(').trim_end_matches(')');
                e.label = Some(Label::plain(card));
            }
        }
        true
    }

    fn known(&mut self, id: &str, line: &Line) -> bool {
        let ok = self.named.contains(id);
        if !ok {
            self.d.push(Diagnostic::new(
                line.range,
                format!("no entity or relationship is named `{id}`"),
            ));
        }
        ok
    }
}

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let mut l = Lower {
        chart: Flowchart::default(),
        d: Vec::new(),
        named: HashSet::new(),
        attributes: 0,
    };
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        match lines::common(&line, &mut cursor, &mut l.d) {
            Common::No => {}
            Common::Done => continue,
            Common::Title(t) => {
                l.chart.title = Some(t);
                continue;
            }
            Common::Direction(d) => {
                l.chart.direction = d;
                continue;
            }
        }
        match line.word().as_str() {
            "entity" => l.declaration(&line, line.after_word(), true, &mut cursor),
            "relationship" => l.declaration(&line, line.after_word(), false, &mut cursor),
            _ => {
                if !l.link(&line) {
                    l.d.push(Diagnostic::new(line.range, STATEMENT));
                }
            }
        }
    }
    (l.chart, l.d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereotype_lists() {
        assert_eq!(
            stereotypes("Age : INT <<derived>> <<Key>>"),
            ("Age : INT", vec!["derived".to_owned(), "key".to_owned()])
        );
        assert_eq!(stereotypes("plain"), ("plain", vec![]));
    }
}
