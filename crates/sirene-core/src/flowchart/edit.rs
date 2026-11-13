//! Source edits for changes made on the drawing (WYSIWY-8).

use std::collections::HashSet;
use std::ops::Range;

use rowan::TextRange;

use super::model::{Arrow, Direction, Edge, Flowchart, Node, Shape, Stroke, Subgraph};
use super::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

/// A replacement of the bytes `range` of a source text with `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub text: String,
}

impl TextEdit {
    /// `src` with the edit applied.
    pub fn apply(&self, src: &str) -> String {
        let mut out = String::with_capacity(src.len() + self.text.len());
        out.push_str(&src[..self.range.start]);
        out.push_str(&self.text);
        out.push_str(&src[self.range.end..]);
        out
    }

    /// The smallest single edit that turns `old` into `new`, or `None` when they are equal.
    pub fn between(old: &str, new: &str) -> Option<TextEdit> {
        if old == new {
            return None;
        }
        let prefix = old
            .char_indices()
            .zip(new.chars())
            .find(|((_, a), b)| a != b)
            .map_or(old.len().min(new.len()), |((i, _), _)| i);
        let room = old.len().min(new.len()) - prefix;
        let mut suffix = 0;
        for (a, b) in old[prefix..].chars().rev().zip(new[prefix..].chars().rev()) {
            if a != b || suffix + a.len_utf8() > room {
                break;
            }
            suffix += a.len_utf8();
        }
        Some(TextEdit {
            range: prefix..old.len() - suffix,
            text: new[prefix..new.len() - suffix].to_owned(),
        })
    }
}

/// Source text for a label (WYSIWY-8): bare when that is safe, otherwise a quoted string.
pub fn label_source(text: &str, markdown: bool) -> String {
    let text = normalize(text);
    let lines: Vec<&str> = text.lines().collect();
    let bare = |c: char| c.is_alphanumeric() || " _.,!?'".contains(c);
    if !markdown && lines.len() == 1 && lines[0].chars().all(bare) {
        return lines[0].to_owned();
    }
    let mut body = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            body.push_str("<br>");
        }
        for c in line.chars() {
            match c {
                '`' if !markdown => body.push_str("#96;"),
                '"' => body.push_str("#quot;"),
                '#' => body.push_str("#35;"),
                '&' => body.push_str("#amp;"),
                '<' => body.push_str("#lt;"),
                _ => body.push(c),
            }
        }
    }
    if markdown {
        format!("\"`{body}`\"")
    } else {
        format!("\"{body}\"")
    }
}

/// The label text that parsing `label_source(text, _)` gives back: control characters as
/// spaces, and each line trimmed.
fn normalize(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Sets the label of node `id`. `None` when there is no such node, its label comes from
/// metadata, or the label is unchanged.
pub fn set_node_label(src: &str, id: &str, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let node = doc.chart.node(id)?;
    if node.metadata.iter().any(|(k, _)| k == "label") || node.label.text == normalize(label) {
        return None;
    }
    let mentions: Vec<SyntaxNode> = doc
        .root
        .descendants()
        .filter(|n| n.kind() == NODE && node.mentions.contains(&n.text_range()))
        .collect();
    let labelled = mentions
        .iter()
        .rev()
        .find_map(|m| child(m, SHAPE))
        .and_then(|s| child(&s, LABEL));
    let edit = match labelled {
        Some(l) => {
            let markdown = tokens(&l).any(|t| t.kind() == MD_STRING);
            (span(l.text_range()), label_source(label, markdown))
        }
        None => {
            let end = usize::from(token(mentions.first()?, IDENT)?.text_range().end());
            (end..end, format!("[{}]", label_source(label, false)))
        }
    };
    doc.finish(vec![edit])
}

/// Sets the label of edge `edge` (an index into `Flowchart::edges`); an empty label removes it.
/// Edges sharing a stroke (`A & B --> C`) share the label. `None` when there is no such edge or
/// the label is unchanged.
pub fn set_edge_label(src: &str, edge: usize, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let e = doc.chart.edges.get(edge)?;
    if e.label.as_ref().map_or("", |l| l.text.as_str()) == normalize(label) {
        return None;
    }
    let syntax = doc
        .root
        .descendants()
        .find(|n| n.kind() == EDGE && n.text_range() == e.range)?;
    let label = normalize(label);
    let piped = child(&syntax, EDGE_LABEL)
        .filter(|l| token(l, PIPE).is_some())
        .and_then(|l| child(&l, LABEL));
    let edit = match piped {
        Some(l) if !label.is_empty() => {
            let markdown = tokens(&l).any(|t| t.kind() == MD_STRING);
            (span(l.text_range()), label_source(&label, markdown))
        }
        _ => {
            let first = syntax
                .children_with_tokens()
                .find(|el| matches!(el.kind(), STROKE | STROKE_OPEN))?;
            let start = usize::from(first.text_range().start());
            let mut text = stroke_text(e);
            if !label.is_empty() {
                text = format!("{text}|{}|", label_source(&label, false));
            }
            (start..usize::from(syntax.text_range().end()), text)
        }
    };
    doc.finish(vec![edit])
}

/// Sets the shape of node `id`. A shape with classic delimiters replaces the delimiters of the
/// node's last mention that has them (or adds them, with the label, after its first mention) and
/// drops `shape` from its metadata; any other shape is written as `@{ shape: NAME }` on the last
/// mention with a shape or metadata (or the first). `None` when there is no such node or the
/// shape is unchanged.
pub fn set_node_shape(src: &str, id: &str, shape: Shape) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let node = doc.chart.node(id)?;
    if node.shape == shape {
        return None;
    }
    let mentions = doc.mentions(node);
    let mut edits = Vec::new();
    match shape.delimiters() {
        Some((open, close)) => {
            for meta in mentions.iter().filter_map(|m| child(m, METADATA)) {
                edits.extend(doc.remove_meta_key(&meta, "shape"));
            }
            match mentions.iter().rev().find_map(|m| child(m, SHAPE)) {
                Some(s) => {
                    let label = child(&s, LABEL).map_or(String::new(), |l| label_text(&l));
                    edits.push((span(s.text_range()), format!("{open}{label}{close}")));
                }
                None => {
                    let end = usize::from(token(mentions.first()?, IDENT)?.text_range().end());
                    let label = label_source(&node.label.text, false);
                    edits.push((end..end, format!("{open}{label}{close}")));
                }
            }
        }
        None => {
            let last = mentions
                .iter()
                .rev()
                .find(|m| child(m, SHAPE).is_some() || child(m, METADATA).is_some())
                .or(mentions.first())?;
            match child(last, METADATA) {
                Some(meta) => edits.push(doc.set_meta_key(&meta, "shape", shape.name())),
                None => {
                    let end = usize::from(last.text_range().end());
                    edits.push((end..end, format!("@{{ shape: {} }}", shape.name())));
                }
            }
        }
    }
    doc.finish(edits)
}

/// An edge's line: its stroke, arrowheads, and length (FLOWMO-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub stroke: Stroke,
    pub start: Option<Arrow>,
    pub end: Option<Arrow>,
    /// Ranks spanned, 1 to 10.
    pub length: u8,
}

impl Line {
    pub fn of(e: &Edge) -> Line {
        Line {
            stroke: e.stroke,
            start: e.start,
            end: e.end,
            length: e.length,
        }
    }
}

/// The longest edge length mermaid reads.
pub const MAX_LENGTH: u8 = 10;

/// Sets the line of edge `edge` (an index into `Flowchart::edges`), rewriting its stroke in
/// canonical form and keeping its label. The length is clamped to 1..=`MAX_LENGTH`. `None` when
/// there is no such edge or the line is unchanged. Edges sharing a stroke share the line.
pub fn set_edge_line(src: &str, edge: usize, line: Line) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let e = doc.chart.edges.get(edge)?;
    let line = Line {
        length: line.length.clamp(1, MAX_LENGTH),
        ..line
    };
    if Line::of(e) == line {
        return None;
    }
    let syntax = doc.edge_syntax(e)?;
    let first = syntax
        .children_with_tokens()
        .find(|el| matches!(el.kind(), STROKE | STROKE_OPEN))?;
    let start = usize::from(first.text_range().start());
    let mut text = stroke_text(&Edge {
        stroke: line.stroke,
        start: line.start,
        end: line.end,
        length: line.length,
        ..e.clone()
    });
    if let Some(label) = edge_label_text(&syntax) {
        text = format!("{text}|{label}|");
    }
    doc.finish(vec![(start..usize::from(syntax.text_range().end()), text)])
}

/// Reverses edge `edge`: `A --> B` becomes `B --> A`. An edge alone in its statement swaps its
/// ends in place; one in a chain is taken out (as `delete` does) and written reversed as its own
/// statement at the end. `None` when there is no such edge, it shares its stroke with others
/// (`A & B --> C`), or it has an id and is in a chain.
pub fn reverse_edge(src: &str, edge: usize) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let chart = &doc.chart;
    let e = chart.edges.get(edge)?;
    if doc.shares_stroke(e) {
        return None;
    }
    let syntax = doc.edge_syntax(e)?;
    let stmt = syntax.parent()?;
    let parts: Vec<SyntaxNode> = stmt.children().collect();
    if let [a, mid, b] = parts.as_slice()
        && a.kind() == NODE_GROUP
        && *mid == syntax
        && b.kind() == NODE_GROUP
    {
        let between = &src[usize::from(a.text_range().end())..usize::from(b.text_range().start())];
        let range = usize::from(a.text_range().start())..usize::from(b.text_range().end());
        return doc.finish(vec![(range, format!("{}{between}{}", b.text(), a.text()))]);
    }
    if e.id.is_some() {
        return None;
    }
    let mut stroke = stroke_text(e);
    if let Some(label) = edge_label_text(&syntax) {
        stroke = format!("{stroke}|{label}|");
    }
    let line = format!("{} {stroke} {}", e.to, e.from);
    let without = delete(src, &[], &[edge])?.apply(src);
    let out = Doc::new(&without).append(&line).apply(&without);
    TextEdit::between(src, &out)
}

/// Source text for a label node: kept as written when quoted, otherwise rewritten by
/// `label_source`, which quotes it when its text could end the delimiters around it.
fn label_text(label: &SyntaxNode) -> String {
    if tokens(label).any(|t| matches!(t.kind(), STRING | MD_STRING)) {
        return label.text().to_string().trim().to_owned();
    }
    label_source(
        &text::plain_label(&label.text().to_string(), false).text,
        false,
    )
}

/// An edge's label as source text for `|...|`, when it has one.
fn edge_label_text(edge: &SyntaxNode) -> Option<String> {
    let label = child(edge, EDGE_LABEL).and_then(|l| child(&l, LABEL))?;
    let text = label_text(&label);
    (!text.is_empty()).then_some(text)
}

/// Adds a node labelled `label` (just its id when `label` is empty). Returns its id and the
/// edit.
pub fn add_node(src: &str, label: &str) -> (String, TextEdit) {
    let doc = Doc::new(src);
    let id = doc.fresh_id();
    let line = if label.trim().is_empty() {
        id.clone()
    } else {
        format!("{id}[{}]", label_source(label, false))
    };
    (id, doc.append(&line))
}

/// Adds an edge from `from` to `to`.
pub fn add_edge(src: &str, from: &str, to: &str) -> TextEdit {
    Doc::new(src).append(&format!("{from} --> {to}"))
}

/// Sets `property` of class `name` to `value`, or removes it when `value` is `None`, in the
/// class's last `classDef` naming only it, or in a new `classDef` at the end. A `classDef` left
/// with no declarations is removed. `None` when nothing changes.
pub fn set_class_style(
    src: &str,
    name: &str,
    property: &str,
    value: Option<&str>,
) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let def = doc
        .root
        .descendants()
        .filter(|n| n.kind() == CLASSDEF_STMT)
        .filter(|n| {
            child(n, ID_LIST).is_some_and(|l| {
                let ids: Vec<SyntaxToken> = tokens(&l).filter(|t| t.kind() == IDENT).collect();
                matches!(ids.as_slice(), [t] if t.text() == name)
            })
        })
        .last();
    let Some(def) = def else {
        let value = value?;
        return Some(doc.append(&format!("classDef {name} {property}:{value}")));
    };
    let text = token(&def, STYLE_TEXT)?;
    let (decls, invalid) = text::styles(text.text());
    let mut out: Vec<String> = Vec::new();
    let mut set = false;
    for d in &decls {
        if d.property != property {
            out.push(format!("{}:{}", d.property, d.value));
        } else if let (Some(v), false) = (value, set) {
            out.push(format!("{property}:{v}"));
            set = true;
        }
    }
    if let (Some(v), false) = (value, set) {
        out.push(format!("{property}:{v}"));
    }
    out.extend(invalid);
    let new = out.join(",");
    if new == text.text().trim() {
        return None;
    }
    let edit = if out.is_empty() {
        doc.remove(&def)
    } else {
        (span(text.text_range()), new)
    };
    doc.finish(vec![edit])
}

/// Sets `property` of edge `edge` (its source-order index) to `value`, or removes it when `value`
/// is `None`, keeping its other `linkStyle` declarations. The edge's last `linkStyle` naming only
/// it is rewritten (removed when left empty), or a new one is added at the end; other
/// `linkStyle` statements that style it drop its index, so the result is its only style.
/// `linkStyle default` is left alone. `None` when nothing changes.
pub fn set_link_style(
    src: &str,
    edge: usize,
    property: &str,
    value: Option<&str>,
) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let e = doc.chart.edges.get(edge)?;
    let mut out: Vec<String> = Vec::new();
    let mut set = false;
    for s in &e.styles {
        if s.property != property {
            out.push(format!("{}:{}", s.property, s.value));
        } else if let (Some(v), false) = (value, set) {
            out.push(format!("{property}:{v}"));
            set = true;
        }
    }
    if let (Some(v), false) = (value, set) {
        out.push(format!("{property}:{v}"));
    }
    let old: Vec<String> = e
        .styles
        .iter()
        .map(|s| format!("{}:{}", s.property, s.value))
        .collect();
    if out == old {
        return None;
    }
    let new = out.join(",");
    let index = |stmt: &SyntaxNode| -> Vec<usize> {
        child(stmt, ID_LIST).map_or_else(Vec::new, |l| {
            tokens(&l)
                .filter(|t| t.kind() == NUMBER)
                .filter_map(|t| t.text().parse().ok())
                .collect()
        })
    };
    let stmts: Vec<(SyntaxNode, Vec<usize>)> = doc
        .root
        .descendants()
        .filter(|n| n.kind() == LINKSTYLE_STMT && token(n, DEFAULT_KW).is_none())
        .map(|n| {
            let nums = index(&n);
            (n, nums)
        })
        .filter(|(_, nums)| nums.contains(&edge))
        .collect();
    let own = stmts
        .iter()
        .rposition(|(_, nums)| nums.as_slice() == [edge]);
    // The style text of `stmt` with the blanks before it, to remove it alone.
    let style_span = |stmt: &SyntaxNode| {
        token(stmt, STYLE_TEXT).map(|t| {
            let r = span(t.text_range());
            let start = src[..r.start].trim_end_matches([' ', '\t']).len();
            start..r.end
        })
    };
    let mut edits = Vec::new();
    let mut curve = None;
    for (i, (stmt, nums)) in stmts.iter().enumerate() {
        if Some(i) == own || token(stmt, STYLE_TEXT).is_none() {
            continue;
        }
        let interpolate = token(stmt, INTERPOLATE_KW).is_some();
        if nums.iter().all(|&n| n == edge) {
            edits.push(match style_span(stmt) {
                Some(r) if interpolate => (r, String::new()),
                _ => doc.remove(stmt),
            });
        } else {
            if interpolate {
                curve = token(stmt, IDENT).map(|t| t.text().to_owned());
            }
            let kept: Vec<String> = nums
                .iter()
                .filter(|&&n| n != edge)
                .map(usize::to_string)
                .collect();
            let list = child(stmt, ID_LIST)?;
            edits.push((span(list.text_range()), kept.join(",")));
        }
    }
    match own.map(|i| &stmts[i].0) {
        Some(stmt) => match (token(stmt, STYLE_TEXT), out.is_empty()) {
            (Some(_), true) if token(stmt, INTERPOLATE_KW).is_some() => {
                edits.push((style_span(stmt)?, String::new()));
            }
            (_, true) => edits.push(doc.remove(stmt)),
            (Some(t), false) => edits.push((span(t.text_range()), new)),
            (None, false) => {
                let end = usize::from(stmt.text_range().end());
                edits.push((end..end, format!(" {new}")));
            }
        },
        None if !out.is_empty() => {
            let curve = curve
                .map(|c| format!(" interpolate {c}"))
                .unwrap_or_default();
            let line = doc.append(&format!("linkStyle {edge}{curve} {new}"));
            edits.push((line.range, line.text));
        }
        None => {}
    }
    doc.finish(edits)
}

/// Gives each node of `ids` class `name` with a `class` statement at the end, skipping nodes
/// that have it. `None` when every node has it or none exists.
pub fn apply_class(src: &str, ids: &[&str], name: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let missing: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| {
            doc.chart
                .node(id)
                .is_some_and(|n| !n.classes.iter().any(|c| c == name))
        })
        .collect();
    if missing.is_empty() {
        return None;
    }
    Some(doc.append(&format!("class {} {name}", missing.join(","))))
}

/// Takes class `name` off each node of `ids`: drops them from `class` statements naming it
/// (removing a statement left empty) and removes their `:::name` shorthands. `None` when
/// nothing changes.
pub fn remove_class(src: &str, ids: &[&str], name: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut edits = Vec::new();
    for n in doc.root.descendants() {
        match n.kind() {
            CLASS_STMT => {
                let named = tokens(&n)
                    .filter(|t| t.kind() == IDENT)
                    .last()
                    .is_some_and(|t| t.text() == name);
                let Some(list) = child(&n, ID_LIST).filter(|_| named) else {
                    continue;
                };
                let all: Vec<String> = tokens(&list)
                    .filter(|t| t.kind() == IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                let kept: Vec<&str> = all
                    .iter()
                    .map(String::as_str)
                    .filter(|i| !ids.contains(i))
                    .collect();
                if kept.is_empty() {
                    edits.push(doc.remove(&n));
                } else if kept.len() < all.len() {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            NODE if token(&n, IDENT).is_some_and(|t| ids.contains(&t.text())) => {
                if let Some(short) = child(&n, CLASS_SHORTHAND)
                    && token(&short, IDENT).is_some_and(|t| t.text() == name)
                {
                    edits.push((span(short.text_range()), String::new()));
                }
            }
            _ => {}
        }
    }
    doc.finish(edits)
}

/// The first `styleN` not used as a class name nor named in the source.
pub fn fresh_class_name(src: &str) -> String {
    let doc = Doc::new(src);
    let named = doc.named();
    (1..)
        .map(|n| format!("style{n}"))
        .find(|c| !named.contains(c) && !doc.chart.classes.iter().any(|d| d.name == *c))
        .expect("names are unbounded")
}

/// Adds an invisible edge (`~~~`) from `a` to `b`, which lays them out like linked nodes without
/// drawing a line.
pub fn keep_together(src: &str, a: &str, b: &str) -> TextEdit {
    Doc::new(src).append(&format!("{a} ~~~ {b}"))
}

/// The keyword sirene writes for `d`.
pub fn direction_text(d: Direction) -> &'static str {
    match d {
        Direction::TopBottom => "TD",
        Direction::BottomTop => "BT",
        Direction::LeftRight => "LR",
        Direction::RightLeft => "RL",
    }
}

/// Sets the direction of the diagram (`subgraph` `None`: the header's) or of a subgraph (its
/// last `direction` statement, or a new one after its header). `None` when it already reads
/// `dir` or there is no such subgraph.
pub fn set_direction(src: &str, subgraph: Option<&str>, dir: Direction) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let text = direction_text(dir);
    let same = |t: &SyntaxToken| Direction::from_text(t.text()) == Some(dir);
    let edit = match subgraph {
        None => {
            let header = child(&doc.root, HEADER)?;
            match token(&header, DIR) {
                Some(t) if same(&t) => return None,
                Some(t) => (span(t.text_range()), text.to_owned()),
                None => {
                    let at = usize::from(token(&header, GRAPH_KW)?.text_range().end());
                    (at..at, format!(" {text}"))
                }
            }
        }
        Some(id) => {
            let block = doc.subgraph_syntax(doc.chart.subgraph(id)?)?;
            let last = block
                .children()
                .filter(|n| n.kind() == DIRECTION_STMT)
                .last();
            match last.as_ref().map(|s| (s, token(s, DIR))) {
                Some((_, Some(t))) if same(&t) => return None,
                Some((_, Some(t))) => (span(t.text_range()), text.to_owned()),
                Some((s, None)) => (span(s.text_range()), format!("direction {text}")),
                None => {
                    let header = child(&block, SUBGRAPH_HEADER)?;
                    let end = usize::from(header.text_range().end());
                    // Before the header line's ending, `\r\n` or `\n`.
                    let at = src[end..].find('\n').map_or(src.len(), |i| end + i);
                    let at = if src[..at].ends_with('\r') {
                        at - 1
                    } else {
                        at
                    };
                    let indent = doc.body_indent(&block);
                    (at..at, format!("{}{indent}direction {text}", doc.newline))
                }
            }
        }
    };
    doc.finish(vec![edit])
}

/// Puts `nodes` in a new subgraph titled `title`, written at the end of the subgraph they share
/// (or of the document) with a bare mention of each. Returns the subgraph's id and the edit;
/// `None` when `nodes` is empty, names a missing node, or spans more than one subgraph.
pub fn group(src: &str, nodes: &[&str], title: &str) -> Option<(String, TextEdit)> {
    let doc = Doc::new(src);
    let (first, rest) = nodes.split_first()?;
    let parent = doc.parent(first);
    if nodes.iter().any(|id| doc.chart.node(id).is_none())
        || rest.iter().any(|id| doc.parent(id) != parent)
    {
        return None;
    }
    let id = doc.fresh_group_id();
    let scope = match parent {
        Some(p) => Some(doc.subgraph_syntax(doc.chart.subgraph(p)?)?),
        None => None,
    };
    let mut lines = vec![format!("subgraph {id} [{}]", label_source(title, false))];
    lines.extend(nodes.iter().map(|n| format!("    {n}")));
    lines.push("end".to_owned());
    let edit = doc.insert_lines(scope.as_ref(), &lines);
    Some((id, doc.finish(vec![edit])?))
}

/// Removes subgraph `id`, keeping its statements one level less indented, and its `direction`,
/// `style`, and `class` statements. Mentions of the subgraph in other statements, such as an
/// edge to it, name its members instead (`A --> B & C`), and `linkStyle` indexes follow. `None`
/// when there is no such subgraph, an edge to it has an id or it has no members, or its
/// `subgraph` line or `end` shares a line with another statement.
pub fn ungroup(src: &str, id: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let sg = doc.chart.subgraph(id)?;
    let linked = doc.chart.edges.iter().any(|e| e.from == id || e.to == id);
    if linked
        && (sg.members.is_empty()
            || doc
                .chart
                .edges
                .iter()
                .any(|e| (e.from == id || e.to == id) && e.id.is_some()))
    {
        return None;
    }
    let block = doc.subgraph_syntax(sg)?;
    let start = usize::from(block.text_range().start());
    let first_line = line_start(src, start);
    if !src[first_line..start].trim().is_empty() {
        return None;
    }
    let header_end = usize::from(child(&block, SUBGRAPH_HEADER)?.text_range().end());
    let body_start = header_end + src[header_end..].find('\n')? + 1;
    if !src[header_end..body_start].trim().is_empty() {
        return None;
    }
    let end_kw = token(&block, END_KW)?;
    let end_start = usize::from(end_kw.text_range().start());
    let end_line = line_start(src, end_start);
    let end_after = usize::from(end_kw.text_range().end());
    let end_line_end = src[end_after..]
        .find('\n')
        .map_or(src.len(), |i| end_after + i + 1);
    if !src[end_line..end_start].trim().is_empty()
        || !src[end_after..end_line_end].trim().is_empty()
    {
        return None;
    }

    let mut edits = vec![
        (first_line..body_start, String::new()),
        (end_line..end_line_end, String::new()),
    ];
    // One level of indent comes off each body line.
    let outer = indent(src, start);
    let inner = doc.body_indent(&block);
    if let Some(unit) = inner.strip_prefix(outer).filter(|u| !u.is_empty()) {
        let prefix = format!("{outer}{unit}");
        let mut at = body_start;
        while at < end_line {
            if src[at..].starts_with(&prefix) {
                let from = at + outer.len();
                edits.push((from..from + unit.len(), String::new()));
            }
            at += src[at..].find('\n').map_or(src.len() - at, |i| i + 1);
        }
    }
    for stmt in block.children().filter(|n| n.kind() == DIRECTION_STMT) {
        edits.push(doc.remove(&stmt));
    }
    for stmt in doc.root.descendants() {
        match stmt.kind() {
            STYLE_STMT if token(&stmt, IDENT).is_some_and(|t| t.text() == id) => {
                edits.push(doc.remove(&stmt));
            }
            CLASS_STMT => {
                let Some(list) = child(&stmt, ID_LIST) else {
                    continue;
                };
                let ids: Vec<String> = tokens(&list)
                    .filter(|t| t.kind() == IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                let kept: Vec<&str> = ids
                    .iter()
                    .map(String::as_str)
                    .filter(|i| *i != id)
                    .collect();
                if kept.is_empty() {
                    edits.push(doc.remove(&stmt));
                } else if kept.len() < ids.len() {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            NODE if token(&stmt, IDENT).is_some_and(|t| t.text() == id)
                && stmt.ancestors().any(|a| a.kind() == VERTEX_STMT) =>
            {
                edits.push((span(stmt.text_range()), sg.members.join(" & ")));
            }
            _ => {}
        }
    }
    let new = doc.finish(edits)?.apply(src);
    if !linked {
        return TextEdit::between(src, &new);
    }
    let members = sg.members.clone();
    let after = super::parse(&new).lower().flowchart;
    let map = match_edges(&doc.chart.edges, &after.edges, |end| {
        if end == id {
            members.clone()
        } else {
            vec![end.to_owned()]
        }
    });
    TextEdit::between(src, &relink(&new, &map))
}

/// Moves node `id` into subgraph `target`, or out to the top level when `target` is `None`.
/// From the top level, a bare mention is added to the target; from a subgraph, the statements
/// in it that mention the node move. Other nodes of a moved statement (`A --> B`) that would
/// leave the subgraph with it get a bare mention there, and `linkStyle` indexes follow the
/// moved edges. `None` when nothing would change or the result would move other nodes.
pub fn move_to_subgraph(src: &str, id: &str, target: Option<&str>) -> Option<TextEdit> {
    let doc = Doc::new(src);
    doc.chart.node(id)?;
    let current = doc.parent(id);
    if current == target {
        return None;
    }
    let scope = match target {
        Some(t) => Some(doc.subgraph_syntax(doc.chart.subgraph(t)?)?),
        None => None,
    };
    let mut edits = Vec::new();
    let mut lines = Vec::new();
    // Other nodes of the moved statements, and the subgraph they are in.
    let mut others: Vec<String> = Vec::new();
    let mut from = None;
    match current {
        None => lines.push(id.to_owned()),
        Some(c) => {
            let block = doc.subgraph_syntax(doc.chart.subgraph(c)?)?;
            for stmt in block.children().filter(|n| n.kind() == VERTEX_STMT) {
                let ids: Vec<String> = stmt
                    .descendants()
                    .filter(|n| n.kind() == NODE)
                    .filter_map(|n| token(&n, IDENT).map(|t| t.text().to_owned()))
                    .collect();
                if !ids.iter().any(|i| i == id) {
                    continue;
                }
                for i in ids {
                    if i != id && !others.contains(&i) {
                        others.push(i);
                    }
                }
                lines.push(stmt.text().to_string());
                edits.push(doc.remove(&stmt));
            }
            if lines.is_empty() {
                return None;
            }
            from = Some(block);
        }
    }
    edits.push(doc.insert_lines(scope.as_ref(), &lines));
    let moved = doc.finish(edits.clone())?.apply(src);
    let after = super::parse(&moved).lower().flowchart;
    let parent = |chart: &Flowchart, n: &str| {
        chart
            .subgraphs
            .iter()
            .find(|s| s.members.iter().any(|m| m == n))
            .map(|s| s.id.clone())
    };
    // Those that left keep a mention where they were.
    let stay: Vec<String> = others
        .iter()
        .filter(|n| doc.parent(n) == current && parent(&after, n).as_deref() != current)
        .cloned()
        .collect();
    let new = if stay.is_empty() {
        moved
    } else {
        edits.push(doc.insert_lines(from.as_ref(), &stay));
        doc.finish(edits)?.apply(src)
    };
    let after = super::parse(&new).lower().flowchart;
    let kept = others
        .iter()
        .all(|n| parent(&after, n) == parent(&doc.chart, n));
    if parent(&after, id).as_deref() != target || !kept {
        return None;
    }
    let map = match_edges(&doc.chart.edges, &after.edges, |end| vec![end.to_owned()]);
    TextEdit::between(src, &relink(&new, &map))
}

/// Swaps the statement with node `id`'s first mention and the statement before it (or, with
/// `later`, after it). See [`move_edge`].
pub fn move_node(src: &str, id: &str, later: bool) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let first = *doc.chart.node(id)?.mentions.first()?;
    let stmt = doc
        .root
        .descendants()
        .find(|n| n.kind() == NODE && n.text_range() == first)?
        .ancestors()
        .find(|n| n.kind() == VERTEX_STMT)?;
    doc.swap(&stmt, later)
}

/// Swaps the statement with edge `edge` and the statement before it (or, with `later`, after
/// it). Only a neighboring node or edge statement or subgraph swaps; `linkStyle` indexes follow
/// their edges. `None` at either end of its subgraph or document, or next to another kind of
/// statement.
pub fn move_edge(src: &str, edge: usize, later: bool) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let stmt = doc
        .edge_syntax(doc.chart.edges.get(edge)?)?
        .ancestors()
        .find(|n| n.kind() == VERTEX_STMT)?;
    doc.swap(&stmt, later)
}

/// Deletes the nodes `nodes`, the edges `edges` (indexes into `Flowchart::edges`), and every
/// edge touching a deleted node; edges sharing a stroke (`A & B --> C`) go together. `None`
/// when nothing changes.
pub fn delete(src: &str, nodes: &[&str], edges: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let chart = &doc.chart;
    let mut dead: HashSet<&str> = nodes.iter().copied().collect();
    let dead_edges: HashSet<TextRange> = edges
        .iter()
        .filter_map(|&i| chart.edges.get(i))
        .map(|e| e.range)
        .collect();
    // Model edges that go, for renumbering `linkStyle`.
    let gone: Vec<bool> = chart
        .edges
        .iter()
        .map(|e| {
            dead_edges.contains(&e.range)
                || dead.contains(e.from.as_str())
                || dead.contains(e.to.as_str())
        })
        .collect();
    // The ids of edges that go, so their `id@{ ... }`, `class`, and `style` statements go too.
    for (e, _) in chart.edges.iter().zip(&gone).filter(|(_, g)| **g) {
        if let Some(id) = &e.id {
            dead.insert(id);
        }
    }

    let mut edits = Vec::new();
    let mut chains = Vec::new();
    // Ids mentioned so far, per enclosing subgraph (or root). A piece left as one bare node goes
    // when the node was already mentioned in the same scope, which keeps first mentions (node
    // order, and what later `class` and `style` statements apply to) and subgraph membership.
    let mut seen: HashSet<(String, TextRange)> = HashSet::new();
    for stmt in doc.root.descendants() {
        match stmt.kind() {
            VERTEX_STMT => {
                let mut chain = Chain::new(src, &stmt, &dead, &dead_edges);
                let scope = chain.scope();
                if chain.changed {
                    chain.pieces.retain(|p| {
                        p.bare
                            .as_ref()
                            .is_none_or(|id| !seen.contains(&(id.clone(), scope)))
                    });
                }
                for id in chain.ids() {
                    seen.insert((id, scope));
                }
                if chain.changed {
                    chains.push(chain);
                }
            }
            CLASS_STMT => {
                let Some(list) = child(&stmt, ID_LIST) else {
                    continue;
                };
                let ids: Vec<String> = tokens(&list)
                    .filter(|t| t.kind() == IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                let kept: Vec<&str> = ids
                    .iter()
                    .map(String::as_str)
                    .filter(|id| !dead.contains(id))
                    .collect();
                if kept.is_empty() {
                    edits.push(doc.remove(&stmt));
                } else if kept.len() < ids.len() {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            STYLE_STMT | CLICK_STMT => {
                if token(&stmt, IDENT).is_some_and(|t| dead.contains(t.text())) {
                    edits.push(doc.remove(&stmt));
                }
            }
            LINKSTYLE_STMT => {
                let Some(list) = child(&stmt, ID_LIST) else {
                    continue;
                };
                let old: Vec<String> = tokens(&list)
                    .filter(|t| t.kind() == NUMBER)
                    .map(|t| t.text().to_owned())
                    .collect();
                let new: Vec<String> = old
                    .iter()
                    .filter_map(|n| {
                        let Ok(i) = n.parse::<usize>() else {
                            return Some(n.clone());
                        };
                        match gone.get(i) {
                            Some(true) => return None,
                            Some(false) => {}
                            None => return Some(n.clone()),
                        }
                        let before = gone.iter().take(i).filter(|&&g| g).count();
                        Some((i - before).to_string())
                    })
                    .collect();
                if new.is_empty() {
                    edits.push(doc.remove(&stmt));
                } else if new != old {
                    edits.push((span(list.text_range()), new.join(",")));
                }
            }
            _ => {}
        }
    }

    for chain in chains {
        if chain.pieces.is_empty() {
            edits.push(doc.remove(&chain.syntax));
        } else {
            let indent = indent(src, usize::from(chain.syntax.text_range().start()));
            let sep = format!("{}{indent}", doc.newline);
            let text: Vec<&str> = chain.pieces.iter().map(|p| p.text.as_str()).collect();
            edits.push((span(chain.syntax.text_range()), text.join(&sep)));
        }
    }
    doc.finish(edits)
}

/// One `VERTEX_STMT` after deletions: the pieces it splits into.
struct Chain {
    syntax: SyntaxNode,
    changed: bool,
    pieces: Vec<Piece>,
}

struct Piece {
    text: String,
    ids: Vec<String>,
    /// The id, when the piece is a single node mention without shape, class, or metadata.
    bare: Option<String>,
}

impl Chain {
    fn new(
        src: &str,
        stmt: &SyntaxNode,
        dead: &HashSet<&str>,
        dead_edges: &HashSet<TextRange>,
    ) -> Chain {
        let mut changed = false;
        let mut pieces: Vec<Piece> = Vec::new();
        let mut open = false;
        let mut edge: Option<SyntaxNode> = None;
        let mut prev_end = None;
        for part in stmt.children() {
            match part.kind() {
                EDGE => edge = Some(part),
                NODE_GROUP => {
                    let nodes: Vec<SyntaxNode> =
                        part.children().filter(|n| n.kind() == NODE).collect();
                    let kept: Vec<&SyntaxNode> = nodes
                        .iter()
                        .filter(|n| token(n, IDENT).is_none_or(|t| !dead.contains(t.text())))
                        .collect();
                    let edge = edge.take();
                    let edge_dead = edge
                        .as_ref()
                        .is_some_and(|e| dead_edges.contains(&e.text_range()));
                    if kept.len() < nodes.len() || edge_dead {
                        changed = true;
                    }
                    if kept.is_empty() {
                        open = false;
                        prev_end = None;
                        continue;
                    }
                    let text = if kept.len() == nodes.len() {
                        part.text().to_string()
                    } else {
                        let texts: Vec<String> =
                            kept.iter().map(|n| n.text().to_string()).collect();
                        texts.join(" & ")
                    };
                    let ids: Vec<String> = kept
                        .iter()
                        .filter_map(|n| token(n, IDENT))
                        .map(|t| t.text().to_owned())
                        .collect();
                    let start = usize::from(part.text_range().start());
                    match (
                        open && edge.is_some() && !edge_dead,
                        prev_end,
                        pieces.last_mut(),
                    ) {
                        (true, Some(end), Some(piece)) => {
                            piece.text.push_str(&src[end..start]);
                            piece.text.push_str(&text);
                            piece.ids.extend(ids);
                            piece.bare = None;
                        }
                        _ => {
                            let bare = match kept.as_slice() {
                                [n] if n.children().next().is_none() => ids.first().cloned(),
                                _ => None,
                            };
                            pieces.push(Piece { text, ids, bare });
                        }
                    }
                    open = true;
                    prev_end = Some(usize::from(part.text_range().end()));
                }
                _ => {}
            }
        }
        // Text after the last node (an incomplete edge or an error) stays with its piece.
        if let (true, Some(end), Some(piece)) = (open, prev_end, pieces.last_mut()) {
            let tail = &src[end..usize::from(stmt.text_range().end())];
            if !tail.is_empty() {
                piece.text.push_str(tail);
                piece.bare = None;
            }
        }
        Chain {
            syntax: stmt.clone(),
            changed,
            pieces,
        }
    }

    fn ids(&self) -> Vec<String> {
        self.pieces.iter().flat_map(|p| p.ids.clone()).collect()
    }

    /// The range of the subgraph or root the statement is in.
    fn scope(&self) -> TextRange {
        self.syntax
            .parent()
            .expect("statements have a parent")
            .text_range()
    }
}

/// A parsed source with what the edits need.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    chart: Flowchart,
    newline: &'static str,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Self {
        let parse = super::parse(src);
        Doc {
            src,
            root: parse.syntax(),
            chart: parse.lower().flowchart,
            newline: if src.contains("\r\n") { "\r\n" } else { "\n" },
        }
    }

    /// Applies `edits` and reduces them to one. Only removals overlap (two statements removed
    /// from one line both take the `;` between them), and they merge.
    fn finish(&self, mut edits: Vec<(Range<usize>, String)>) -> Option<TextEdit> {
        edits.sort_by_key(|(r, _)| (r.start, r.end));
        let mut out = String::with_capacity(self.src.len());
        let mut at = 0;
        for (range, text) in edits {
            debug_assert!(range.start >= at || text.is_empty(), "overlapping edits");
            out.push_str(&self.src[at.min(range.start)..range.start]);
            out.push_str(&text);
            at = at.max(range.end);
        }
        out.push_str(&self.src[at..]);
        TextEdit::between(self.src, &out)
    }

    /// The edit removing statement `stmt`: its whole line when it is alone on it, otherwise
    /// the statement with the whitespace and `;` after it.
    fn remove(&self, stmt: &SyntaxNode) -> (Range<usize>, String) {
        let src = self.src;
        let range = span(stmt.text_range());
        let line_start = src[..range.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[range.end..]
            .find('\n')
            .map_or(src.len(), |i| range.end + i);
        let before = &src[line_start..range.start];
        let after = src[range.end..line_end].trim();
        if before.trim().is_empty() && after.trim_start_matches(';').trim().is_empty() {
            let end = (line_end + 1).min(src.len());
            return (line_start..end, String::new());
        }
        if after.trim_start_matches(';').trim().is_empty() {
            // Last on its line: take the `;` and whitespace before it instead.
            let before = before.trim_end_matches([' ', '\t']);
            let before = before.strip_suffix(';').unwrap_or(before);
            let start = line_start + before.trim_end_matches([' ', '\t']).len();
            return (start..range.end, String::new());
        }
        let rest = &src[range.end..];
        let mut end = range.end + (rest.len() - rest.trim_start_matches([' ', '\t']).len());
        if src[end..].starts_with(';') {
            end += 1;
            let rest = &src[end..];
            end += rest.len() - rest.trim_start_matches([' ', '\t']).len();
        }
        (range.start..end, String::new())
    }

    /// The `NODE` syntax of each mention of `node`, in source order.
    fn mentions(&self, node: &Node) -> Vec<SyntaxNode> {
        self.root
            .descendants()
            .filter(|n| n.kind() == NODE && node.mentions.contains(&n.text_range()))
            .collect()
    }

    /// Whether `e` shares its stroke with other edges (`A & B --> C`), so its text cannot be
    /// changed alone.
    fn shares_stroke(&self, e: &Edge) -> bool {
        self.chart
            .edges
            .iter()
            .filter(|x| x.range == e.range)
            .count()
            > 1
    }

    /// The `EDGE` syntax of `e`.
    fn edge_syntax(&self, e: &Edge) -> Option<SyntaxNode> {
        self.root
            .descendants()
            .find(|n| n.kind() == EDGE && n.text_range() == e.range)
    }

    /// The edit setting `key` to `value` in a `METADATA` node: the existing entry's value, or a
    /// new entry first.
    fn set_meta_key(&self, meta: &SyntaxNode, key: &str, value: &str) -> (Range<usize>, String) {
        let Some(body) = token(meta, META_BODY) else {
            // `@{}`: the body goes between the braces.
            let at = usize::from(
                token(meta, META_OPEN)
                    .map_or(meta.text_range(), |t| t.text_range())
                    .end(),
            );
            return (at..at, format!(" {key}: {value} "));
        };
        let base = usize::from(body.text_range().start());
        let entries = text::metadata_entries(body.text());
        match entries.iter().rev().find(|e| e.key == key) {
            Some(e) => (base + e.value.start..base + e.value.end, value.to_owned()),
            None => {
                let lead = body.text().len() - body.text().trim_start().len();
                let at = base + lead;
                if body.text().trim().is_empty() {
                    (base..base + body.text().len(), format!(" {key}: {value} "))
                } else {
                    (at..at, format!("{key}: {value}, "))
                }
            }
        }
    }

    /// The edit removing every `key` entry from a `METADATA` node, or the whole `@{ ... }` when
    /// nothing else is left in it. `None` when it has no such entry.
    fn remove_meta_key(&self, meta: &SyntaxNode, key: &str) -> Option<(Range<usize>, String)> {
        let body = token(meta, META_BODY)?;
        let text = body.text();
        let entries = text::metadata_entries(text);
        if !entries.iter().any(|e| e.key == key) {
            return None;
        }
        let kept: Vec<&str> = entries
            .iter()
            .filter(|e| e.key != key)
            .map(|e| &text[e.entry.clone()])
            .collect();
        if kept.is_empty() {
            return Some((span(meta.text_range()), String::new()));
        }
        let lead = &text[..text.len() - text.trim_start().len()];
        let trail = &text[text.trim_end().len()..];
        Some((
            span(body.text_range()),
            format!("{lead}{}{trail}", kept.join(", ")),
        ))
    }

    /// The edit appending `line` as a statement at the end of the document.
    fn append(&self, line: &str) -> TextEdit {
        let src = self.src;
        let indent = self.top_indent();
        let sep = if src.is_empty() || src.ends_with('\n') {
            ""
        } else {
            self.newline
        };
        TextEdit {
            range: src.len()..src.len(),
            text: format!("{sep}{indent}{line}{}", self.newline),
        }
    }

    /// The `SUBGRAPH` block of `sg`.
    fn subgraph_syntax(&self, sg: &Subgraph) -> Option<SyntaxNode> {
        self.root
            .descendants()
            .find(|n| n.kind() == SUBGRAPH && n.text_range() == sg.range)
    }

    /// The id of the subgraph node `id` is a member of.
    fn parent(&self, id: &str) -> Option<&str> {
        self.chart
            .subgraphs
            .iter()
            .find(|s| s.members.iter().any(|m| m == id))
            .map(|s| s.id.as_str())
    }

    /// The indent of the statements in `block`: its first statement's, else its own plus four
    /// spaces.
    fn body_indent(&self, block: &SyntaxNode) -> String {
        match block.children().find(|n| n.kind() != SUBGRAPH_HEADER) {
            Some(first) => indent(self.src, usize::from(first.text_range().start())).to_owned(),
            None => format!(
                "{}    ",
                indent(self.src, usize::from(block.text_range().start()))
            ),
        }
    }

    /// The indent of top-level statements, as [`Doc::append`] uses.
    fn top_indent(&self) -> &str {
        self.root
            .children()
            .filter(|n| !matches!(n.kind(), HEADER | ERROR))
            .last()
            .map_or("    ", |n| {
                indent(self.src, usize::from(n.text_range().start()))
            })
    }

    /// The edit adding `lines` as statements at the end of `scope` (a `SUBGRAPH`, before its
    /// `end`) or of the document, each indented like the statements there.
    fn insert_lines(&self, scope: Option<&SyntaxNode>, lines: &[String]) -> (Range<usize>, String) {
        let src = self.src;
        let nl = self.newline;
        let Some(block) = scope else {
            let indent = self.top_indent();
            let sep = if src.is_empty() || src.ends_with('\n') {
                ""
            } else {
                nl
            };
            let body: String = lines.iter().map(|l| format!("{indent}{l}{nl}")).collect();
            return (src.len()..src.len(), format!("{sep}{body}"));
        };
        let inner = self.body_indent(block);
        let body: String = lines.iter().map(|l| format!("{inner}{l}{nl}")).collect();
        let Some(end) = token(block, END_KW) else {
            // An unclosed subgraph runs to the end of the document.
            let at = usize::from(block.text_range().end());
            let sep = if src[..at].ends_with('\n') { "" } else { nl };
            return (at..at, format!("{sep}{body}"));
        };
        let at = usize::from(end.text_range().start());
        let start = line_start(src, at);
        if src[start..at].trim().is_empty() {
            (start..start, body)
        } else {
            let outer = indent(src, usize::from(block.text_range().start()));
            (at..at, format!("{nl}{body}{outer}"))
        }
    }

    /// Swaps vertex statement `stmt` with the statement before it, or after it with `later`,
    /// when that is a vertex statement or subgraph; `linkStyle` indexes follow their edges.
    fn swap(&self, stmt: &SyntaxNode, later: bool) -> Option<TextEdit> {
        let siblings: Vec<SyntaxNode> = stmt
            .parent()?
            .children()
            .filter(|n| !matches!(n.kind(), HEADER | SUBGRAPH_HEADER | ERROR))
            .collect();
        let at = siblings.iter().position(|n| n == stmt)?;
        let other = if later {
            siblings.get(at + 1)?
        } else {
            siblings.get(at.checked_sub(1)?)?
        };
        if !matches!(other.kind(), VERTEX_STMT | SUBGRAPH) {
            return None;
        }
        let (first, second) = if later { (stmt, other) } else { (other, stmt) };
        let mut edits = vec![
            (span(first.text_range()), second.text().to_string()),
            (span(second.text_range()), first.text().to_string()),
        ];
        // Edges keep source order: the second statement's edges now come first.
        let inside = |r: TextRange, e: &Edge| r.contains_range(e.range);
        let edges = &self.chart.edges;
        let old_first: Vec<usize> = (0..edges.len())
            .filter(|&i| inside(first.text_range(), &edges[i]))
            .collect();
        let old_second: Vec<usize> = (0..edges.len())
            .filter(|&i| inside(second.text_range(), &edges[i]))
            .collect();
        let mut order: Vec<usize> = (0..edges.len())
            .filter(|i| !old_first.contains(i) && !old_second.contains(i))
            .collect();
        if let Some(&base) = old_first.first().or(old_second.first()) {
            let at = order.iter().position(|&i| i > base).unwrap_or(order.len());
            order.splice(
                at..at,
                old_second
                    .iter()
                    .chain(&old_first)
                    .copied()
                    .collect::<Vec<_>>(),
            );
        }
        let new_index = |old: usize| order.iter().position(|&i| i == old);
        for stmt in self
            .root
            .descendants()
            .filter(|n| n.kind() == LINKSTYLE_STMT)
        {
            let Some(list) = child(&stmt, ID_LIST) else {
                continue;
            };
            let old: Vec<String> = tokens(&list)
                .filter(|t| t.kind() == NUMBER)
                .map(|t| t.text().to_owned())
                .collect();
            let new: Vec<String> = old
                .iter()
                .map(|n| match n.parse::<usize>().ok().and_then(new_index) {
                    Some(i) => i.to_string(),
                    None => n.clone(),
                })
                .collect();
            if new != old {
                edits.push((span(list.text_range()), new.join(",")));
            }
        }
        self.finish(edits)
    }

    /// The first `groupN` id not used by a node, subgraph, or edge, nor named in the source.
    fn fresh_group_id(&self) -> String {
        let named = self.named();
        (1..)
            .map(|n| format!("group{n}"))
            .find(|id| !named.contains(id) && !self.used(id))
            .expect("ids are unbounded")
    }

    /// Every identifier in the source.
    fn named(&self) -> HashSet<String> {
        self.root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == IDENT)
            .map(|t| t.text().to_owned())
            .collect()
    }

    /// Whether `id` is a node, subgraph, or edge.
    fn used(&self, id: &str) -> bool {
        self.chart.node(id).is_some()
            || self.chart.subgraph(id).is_some()
            || self.chart.edge(id).is_some()
    }

    /// The first id of `A` to `Z`, then `n1`, `n2`, ..., not used by a node, subgraph, or edge,
    /// nor named in the source (as by a `class` statement for a node not written yet).
    fn fresh_id(&self) -> String {
        let named = self.named();
        ('A'..='Z')
            .map(String::from)
            .chain((1..).map(|n| format!("n{n}")))
            .find(|id| !named.contains(id) && !self.used(id))
            .expect("ids are unbounded")
    }
}

/// The canonical stroke for an edge's kind, arrowheads, and length.
/// For each edge of `old`, the indexes in `new` of the edges it became: the first unused edges
/// alike in all but their ends, one per pair of ends from `ends` (the ends an end became). An
/// edge with no match keeps its index.
fn match_edges(old: &[Edge], new: &[Edge], ends: impl Fn(&str) -> Vec<String>) -> Vec<Vec<usize>> {
    let alike = |a: &Edge, b: &Edge| {
        (a.stroke, a.start, a.end, a.length, &a.label, &a.id)
            == (b.stroke, b.start, b.end, b.length, &b.label, &b.id)
    };
    let mut used = vec![false; new.len()];
    old.iter()
        .enumerate()
        .map(|(i, e)| {
            let mut found = Vec::new();
            for from in ends(&e.from) {
                for to in ends(&e.to) {
                    let hit = (0..new.len()).find(|&j| {
                        !used[j] && new[j].from == from && new[j].to == to && alike(e, &new[j])
                    });
                    if let Some(j) = hit {
                        used[j] = true;
                        found.push(j);
                    }
                }
            }
            if found.is_empty() {
                found.push(i);
            }
            found.sort_unstable();
            found
        })
        .collect()
}

/// `src` (an edited document whose `linkStyle` statements still use the old edge indexes) with
/// each index replaced by the new indexes `map` gives it.
fn relink(src: &str, map: &[Vec<usize>]) -> String {
    let root = super::parse(src).syntax();
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for stmt in root.descendants().filter(|n| n.kind() == LINKSTYLE_STMT) {
        let Some(list) = child(&stmt, ID_LIST) else {
            continue;
        };
        let old: Vec<String> = tokens(&list)
            .filter(|t| t.kind() == NUMBER)
            .map(|t| t.text().to_owned())
            .collect();
        let mut new: Vec<String> = Vec::new();
        for n in &old {
            match n.parse::<usize>().ok().and_then(|i| map.get(i)) {
                Some(to) => new.extend(to.iter().map(usize::to_string)),
                None => new.push(n.clone()),
            }
        }
        if new != old {
            edits.push((span(list.text_range()), new.join(",")));
        }
    }
    let mut out = src.to_owned();
    for (range, text) in edits.into_iter().rev() {
        out.replace_range(range, &text);
    }
    out
}

fn stroke_text(e: &Edge) -> String {
    let n = usize::from(e.length);
    // Flowcharts have only the first three; the rest come from other diagram types.
    let head = |a: Option<Arrow>, point: char| match a {
        Some(Arrow::Cross) => "x".to_owned(),
        Some(Arrow::Circle) => "o".to_owned(),
        Some(_) => point.to_string(),
        None => String::new(),
    };
    let run = if e.end.is_some() { n + 1 } else { n + 2 };
    let line = match e.stroke {
        Stroke::Invisible => return "~".repeat(n + 2),
        Stroke::Normal => "-".repeat(run),
        Stroke::Thick => "=".repeat(run),
        Stroke::Dotted => format!("-{}-", ".".repeat(n)),
    };
    format!("{}{line}{}", head(e.start, '<'), head(e.end, '>'))
}

/// Leading whitespace of the line containing byte `at`.
fn indent(src: &str, at: usize) -> &str {
    let start = src[..at].rfind('\n').map_or(0, |i| i + 1);
    let line = &src[start..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// The start of the line containing byte `at`.
fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn span(r: TextRange) -> Range<usize> {
    usize::from(r.start())..usize::from(r.end())
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|n| n.kind() == kind)
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}
