//! Source edits for changes made on the drawing (WYSIWY-8).

use std::collections::HashSet;
use std::ops::Range;

use rowan::TextRange;

use super::model::{Arrow, Edge, Flowchart, Stroke};
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
/// `None` when there is no such edge or the label is unchanged.
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

/// Deletes the nodes `nodes`, the edges `edges` (indexes into `Flowchart::edges`), and every
/// edge touching a deleted node. `None` when nothing changes.
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

    /// The edit appending `line` as a statement at the end of the document.
    fn append(&self, line: &str) -> TextEdit {
        let src = self.src;
        let last = self
            .root
            .children()
            .filter(|n| !matches!(n.kind(), HEADER | ERROR))
            .last();
        let indent = last.map_or("    ", |n| indent(src, usize::from(n.text_range().start())));
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

    /// The first id of `A` to `Z`, then `n1`, `n2`, ..., not used by a node, subgraph, or edge,
    /// nor named in the source (as by a `class` statement for a node not written yet).
    fn fresh_id(&self) -> String {
        let chart = &self.chart;
        let named: HashSet<String> = self
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == IDENT)
            .map(|t| t.text().to_owned())
            .collect();
        let used = |id: &str| {
            named.contains(id)
                || chart.node(id).is_some()
                || chart.subgraph(id).is_some()
                || chart.edge(id).is_some()
        };
        ('A'..='Z')
            .map(String::from)
            .chain((1..).map(|n| format!("n{n}")))
            .find(|id| !used(id))
            .expect("ids are unbounded")
    }
}

/// The canonical stroke for an edge's kind, arrowheads, and length.
fn stroke_text(e: &Edge) -> String {
    let n = usize::from(e.length);
    let head = |a: Option<Arrow>, point: char| match a {
        Some(Arrow::Point) => point.to_string(),
        Some(Arrow::Cross) => "x".to_owned(),
        Some(Arrow::Circle) => "o".to_owned(),
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
