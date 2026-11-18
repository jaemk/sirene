//! Drawing edits to DOT source (WYSIWY-29): adding, renaming, connecting, and deleting nodes and
//! edges. Nodes are named by their DOT id and edges by their index in the lowered graph, as
//! `Flowchart::edges` gives them. Each edit is checked by lowering the result.

use std::collections::HashSet;
use std::ops::Range;

use rowan::{TextRange, TextSize};

use super::lower::id_text;
use crate::edits::{combine, indent, line_start, newline, normalize, span, statement_span};
use crate::flowchart::Flowchart;
use crate::flowchart::edit::TextEdit;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// A parsed source and its lowered graph.
struct Doc {
    graph: Option<SyntaxNode>,
    chart: Flowchart,
}

impl Doc {
    fn new(src: &str) -> Doc {
        let parse = super::parse(src);
        let root = parse.syntax();
        Doc {
            graph: root.children().find(|n| n.kind() == DOT_GRAPH),
            chart: parse.lower().flowchart,
        }
    }

    fn directed(&self) -> bool {
        (self.graph.iter())
            .flat_map(|g| tokens(g).filter(|t| t.kind() == DOT_KW))
            .any(|t| t.text().eq_ignore_ascii_case("digraph"))
    }

    /// The statement of edge `i`.
    fn edge_stmt(&self, i: usize) -> Option<SyntaxNode> {
        let range = self.chart.edges.get(i)?.range;
        let g = self.graph.as_ref()?;
        (g.descendants())
            .filter(|n| n.kind() == DOT_EDGE_STMT)
            .find(|n| n.text_range().contains_range(range))
    }

    /// The node statements naming node `id`, in order.
    fn node_stmts(&self, id: &str) -> Vec<SyntaxNode> {
        let Some(g) = &self.graph else {
            return Vec::new();
        };
        (g.descendants())
            .filter(|n| n.kind() == DOT_NODE_STMT)
            .filter(|n| node_name(n).as_deref() == Some(id))
            .collect()
    }
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

/// The id a node statement or `NODE` names.
fn node_name(n: &SyntaxNode) -> Option<String> {
    let node = if n.kind() == NODE {
        n.clone()
    } else {
        n.children().find(|c| c.kind() == NODE)?
    };
    node.children()
        .find(|c| c.kind() == DOT_ID)
        .map(|id| id_text(&id))
}

/// `text` as a DOT quoted string, a line break as `\n`.
fn quote(text: &str) -> String {
    let s = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{s}\"")
}

/// Node `id` as written in DOT: bare when it is a name or numeral, else quoted.
fn id_source(id: &str) -> String {
    let name = id
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || !c.is_ascii())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || !c.is_ascii());
    let numeral = !id.is_empty() && id.chars().all(|c| c.is_ascii_digit());
    let keyword = ["node", "edge", "graph", "digraph", "subgraph", "strict"]
        .iter()
        .any(|k| id.eq_ignore_ascii_case(k));
    if (name || numeral) && !keyword {
        id.to_owned()
    } else {
        quote(id)
    }
}

/// The range of `stmt`, with a `;` right after it.
fn with_semi(src: &str, stmt: &SyntaxNode) -> TextRange {
    let r = stmt.text_range();
    let end = usize::from(r.end());
    let rest = &src[end..];
    let after = rest.trim_start_matches([' ', '\t']);
    match after.strip_prefix(';') {
        Some(_) => TextRange::new(
            r.start(),
            TextSize::from((end + rest.len() - after.len() + 1) as u32),
        ),
        None => r,
    }
}

/// The edit adding `line` as the last statement of the graph, indented as its first statement
/// is or by four spaces.
fn append_stmt(src: &str, doc: &Doc, line: &str) -> Option<TextEdit> {
    let g = doc.graph.as_ref()?;
    let close = tokens(g).filter(|t| t.kind() == R_BRACE).last()?;
    let close = usize::from(close.text_range().start());
    let first = g.children().find(|n| {
        matches!(
            n.kind(),
            DOT_NODE_STMT | DOT_EDGE_STMT | DOT_ATTR_STMT | DOT_ASSIGN | SUBGRAPH
        )
    });
    let pad = first
        .map(|n| indent(src, usize::from(n.text_range().start())).to_owned())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "    ".to_owned());
    let nl = newline(src);
    let at = line_start(src, close);
    Some(if src[at..close].trim().is_empty() {
        TextEdit {
            range: at..at,
            text: format!("{pad}{line}{nl}"),
        }
    } else {
        TextEdit {
            range: close..close,
            text: format!("{nl}{pad}{line}{nl}"),
        }
    })
}

/// The edit setting the `label` of statement `stmt` to `value`: its last `label` attribute
/// replaced, else one added to its first attribute list, else a list added.
fn label_edit(src: &str, stmt: &SyntaxNode, value: &str) -> TextEdit {
    let lists: Vec<SyntaxNode> = (stmt.children())
        .filter(|n| n.kind() == DOT_ATTR_LIST)
        .collect();
    let label = (lists.iter())
        .flat_map(|l| l.children().filter(|a| a.kind() == ATTR))
        .filter(|a| {
            let key = a.children().find(|c| c.kind() == DOT_ID);
            key.is_some_and(|k| id_text(&k) == "label")
        })
        .last();
    if let Some(v) = label.and_then(|a| a.children().filter(|c| c.kind() == DOT_ID).nth(1)) {
        return TextEdit {
            range: span(v.text_range()),
            text: value.to_owned(),
        };
    }
    if let Some(list) = lists.first() {
        let open = usize::from(list.text_range().start()) + 1;
        let empty = !list.children().any(|a| a.kind() == ATTR);
        let sep = if empty { "" } else { ", " };
        return TextEdit {
            range: open..open,
            text: format!("label={value}{sep}"),
        };
    }
    let end = usize::from(stmt.text_range().end());
    let end = src[..end].trim_end().len();
    TextEdit {
        range: end..end,
        text: format!(" [label={value}]"),
    }
}

/// Each edge's ends and label.
fn edges(chart: &Flowchart) -> Vec<(String, String, String)> {
    (chart.edges.iter())
        .map(|e| {
            let label = e.label.as_ref().map(|l| l.text.clone()).unwrap_or_default();
            (e.from.clone(), e.to.clone(), label)
        })
        .collect()
}

/// Shows node `id` as `text`. `None` when there is no such node, the text is empty, nothing
/// changes, or the label would not read back.
pub fn set_node_label(src: &str, id: &str, text: &str) -> Option<TextEdit> {
    let want = normalize(text);
    let doc = Doc::new(src);
    let node = doc.chart.node(id)?;
    if want.is_empty() || node.label.text == want {
        return None;
    }
    let stmts = doc.node_stmts(id);
    let labeled = stmts.iter().rev().find(|s| {
        (s.children().filter(|l| l.kind() == DOT_ATTR_LIST))
            .flat_map(|l| l.children().filter(|a| a.kind() == ATTR))
            .any(|a| a.children().next().is_some_and(|k| id_text(&k) == "label"))
    });
    let edit = match labeled.or(stmts.last()) {
        Some(stmt) => label_edit(src, stmt, &quote(&want)),
        None => append_stmt(
            src,
            &doc,
            &format!("{} [label={}]", id_source(id), quote(&want)),
        )?,
    };
    let after = Doc::new(&edit.apply(src)).chart;
    let reads = after.node(id).is_some_and(|n| n.label.text == want)
        && after.nodes.len() == doc.chart.nodes.len()
        && edges(&after) == edges(&doc.chart);
    reads.then_some(edit)
}

/// Labels edge `i` `text`. `None` when there is no such edge, nothing changes, or the label
/// would not read back on that edge alone (as when its statement makes several edges).
pub fn set_edge_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let want = normalize(text);
    let doc = Doc::new(src);
    let mut expect = edges(&doc.chart);
    if expect.get(i)?.2 == want {
        return None;
    }
    expect[i].2 = want.clone();
    let stmt = doc.edge_stmt(i)?;
    let edit = label_edit(src, &stmt, &quote(&want));
    (edges(&Doc::new(&edit.apply(src)).chart) == expect).then_some(edit)
}

/// Adds a node labeled `label` with a fresh id (`n1`, `n2`, ...) as the graph's last statement.
/// Returns the id and the edit; `None` when there is no graph to add it to.
pub fn add_node(src: &str, label: &str) -> Option<(String, TextEdit)> {
    let doc = Doc::new(src);
    let taken: HashSet<&str> = (doc.chart.nodes.iter().map(|n| n.id.as_str()))
        .chain(doc.chart.subgraphs.iter().map(|s| s.id.as_str()))
        .collect();
    let id = (1..)
        .map(|k| format!("n{k}"))
        .find(|id| !taken.contains(id.as_str()))?;
    let line = format!("{id} [label={}]", quote(&normalize(label)));
    let edit = append_stmt(src, &doc, &line)?;
    Doc::new(&edit.apply(src)).chart.node(&id)?;
    Some((id, edit))
}

/// Adds an edge from node `from` to node `to` as the graph's last statement. `None` when either
/// is not a node of the graph.
pub fn add_edge(src: &str, from: &str, to: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    doc.chart.node(from)?;
    doc.chart.node(to)?;
    let op = if doc.directed() { "->" } else { "--" };
    let line = format!("{} {op} {}", id_source(from), id_source(to));
    let edit = append_stmt(src, &doc, &line)?;
    let mut expect = edges(&doc.chart);
    expect.push((from.to_owned(), to.to_owned(), String::new()));
    let after = edges(&Doc::new(&edit.apply(src)).chart);
    let mut sorted = after.clone();
    sorted.sort();
    expect.sort();
    (sorted == expect).then_some(edit)
}

/// The text replacing edge statement `stmt` without the edges in `dead`: the runs of its edges
/// that are kept, each with the statement's attributes, and the nodes left on no edge alone.
/// `None` when an operand is a subgraph.
fn rebuild(
    src: &str,
    stmt: &SyntaxNode,
    chart: &Flowchart,
    dead: &HashSet<usize>,
    gone: &[&str],
    op: &str,
) -> Option<String> {
    let operands: Vec<SyntaxNode> = (stmt.children())
        .filter(|n| matches!(n.kind(), NODE | SUBGRAPH | ERROR))
        .collect();
    if operands.iter().any(|n| n.kind() != NODE) {
        return None;
    }
    let text = |n: &SyntaxNode| src[span(n.text_range())].to_owned();
    // Window `k` joins operands `k` and `k + 1`; it is cut when one of its edges is.
    let cut: Vec<bool> = (operands.windows(2))
        .map(|w| {
            let range = w[0].text_range().cover(w[1].text_range());
            (chart.edges.iter().enumerate()).any(|(i, e)| e.range == range && dead.contains(&i))
        })
        .collect();
    let attrs = (stmt.children())
        .find(|n| n.kind() == DOT_ATTR_LIST)
        .map(|l| {
            let start = usize::from(l.text_range().start());
            src[start..usize::from(stmt.text_range().end())]
                .trim_end()
                .to_owned()
        });
    let mut pieces = Vec::new();
    let mut on_edge = vec![false; operands.len()];
    let mut k = 0;
    while k < cut.len() {
        if cut[k] {
            k += 1;
            continue;
        }
        let first = k;
        while k < cut.len() && !cut[k] {
            k += 1;
        }
        let run: Vec<String> = operands[first..=k].iter().map(text).collect();
        on_edge[first..=k].iter_mut().for_each(|b| *b = true);
        let mut piece = run.join(&format!(" {op} "));
        if let Some(a) = &attrs {
            piece = format!("{piece} {a}");
        }
        pieces.push(piece);
    }
    for (n, kept) in operands.iter().zip(on_edge) {
        let name = node_name(n)?;
        // A node mentioned elsewhere stays without this mention.
        let elsewhere = (chart.node(&name).into_iter())
            .flat_map(|node| &node.mentions)
            .any(|m| !stmt.text_range().contains_range(*m));
        if !kept && !elsewhere && !gone.contains(&name.as_str()) {
            pieces.push(id_source(&name));
        }
    }
    let nl = newline(src);
    let pad = indent(src, usize::from(stmt.text_range().start()));
    Some(pieces.join(&format!("{nl}{pad}")))
}

/// Removes nodes `nodes` with their edges, and edges `edge_ids`. An edge statement that makes
/// several edges keeps the rest. `None` when nothing is removed or the result would not read
/// back, as when an edge to be removed runs to or from a subgraph.
pub fn delete(src: &str, nodes: &[&str], edge_ids: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let g = doc.graph.as_ref()?;
    let chart = &doc.chart;
    let dead: HashSet<usize> = (chart.edges.iter().enumerate())
        .filter(|(i, e)| {
            edge_ids.contains(i)
                || nodes.contains(&e.from.as_str())
                || nodes.contains(&e.to.as_str())
        })
        .map(|(i, _)| i)
        .collect();
    let op = if doc.directed() { "->" } else { "--" };
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for stmt in g.descendants() {
        match stmt.kind() {
            DOT_EDGE_STMT => {
                let r = stmt.text_range();
                let hit = (chart.edges.iter().enumerate())
                    .any(|(i, e)| dead.contains(&i) && r.contains_range(e.range));
                if !hit {
                    continue;
                }
                let text = rebuild(src, &stmt, chart, &dead, nodes, op)?;
                if text.is_empty() {
                    edits.push((statement_span(src, with_semi(src, &stmt)), String::new()));
                } else {
                    edits.push((span(r), text));
                }
            }
            DOT_NODE_STMT if node_name(&stmt).is_some_and(|n| nodes.contains(&n.as_str())) => {
                edits.push((statement_span(src, with_semi(src, &stmt)), String::new()));
            }
            _ => {}
        }
    }
    let edit = combine(src, edits)?;
    let after = Doc::new(&edit.apply(src)).chart;
    let want_nodes: HashSet<&str> = (chart.nodes.iter().map(|n| n.id.as_str()))
        .filter(|id| !nodes.contains(id))
        .collect();
    let got_nodes: HashSet<&str> = after.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut want_edges: Vec<_> = (edges(chart).into_iter().enumerate())
        .filter(|(i, _)| !dead.contains(i))
        .map(|(_, e)| e)
        .collect();
    let mut got_edges = edges(&after);
    want_edges.sort();
    got_edges.sort();
    (want_nodes == got_nodes && want_edges == got_edges).then_some(edit)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str =
        "digraph G {\n    a [label=\"Start\"];\n    a -> b -> c [color=red];\n    b -> d\n}\n";

    fn chart(src: &str) -> Flowchart {
        super::super::parse(src).lower().flowchart
    }

    #[test]
    fn renames_nodes() {
        let out = set_node_label(SRC, "a", "Begin").unwrap().apply(SRC);
        assert!(out.contains("    a [label=\"Begin\"];\n"), "{out}");
        let out = set_node_label(SRC, "d", "Done \"x\"").unwrap().apply(SRC);
        assert!(
            out.ends_with("    b -> d\n    d [label=\"Done \\\"x\\\"\"]\n}\n"),
            "{out}"
        );
        assert_eq!(chart(&out).node("d").unwrap().label.text, "Done \"x\"");
        let src = "graph { x [shape=box] }";
        let out = set_node_label(src, "x", "X").unwrap().apply(src);
        assert_eq!(out, "graph { x [label=\"X\", shape=box] }");
        assert!(set_node_label(SRC, "a", "Start").is_none());
        assert!(set_node_label(SRC, "zz", "x").is_none());
    }

    #[test]
    fn labels_edges() {
        let c = chart(SRC);
        let bd = c
            .edges
            .iter()
            .position(|e| e.from == "b" && e.to == "d")
            .unwrap();
        let out = set_edge_label(SRC, bd, "go").unwrap().apply(SRC);
        assert!(out.contains("    b -> d [label=\"go\"]\n"), "{out}");
        // `a -> b -> c` makes two edges; one can't be labeled alone.
        let ab = c.edges.iter().position(|e| e.from == "a").unwrap();
        assert!(set_edge_label(SRC, ab, "x").is_none());
    }

    #[test]
    fn adds_nodes_and_edges() {
        let (id, e) = add_node(SRC, "New").unwrap();
        assert_eq!(id, "n1");
        let out = e.apply(SRC);
        assert!(
            out.ends_with("    b -> d\n    n1 [label=\"New\"]\n}\n"),
            "{out}"
        );
        let out = add_edge(&out, "n1", "a").unwrap().apply(&out);
        assert!(out.ends_with("    n1 -> a\n}\n"), "{out}");
        let src = "graph { \"x y\" }";
        let out = add_edge(src, "x y", "x y").unwrap().apply(src);
        assert_eq!(out, "graph { \"x y\" \n    \"x y\" -- \"x y\"\n}");
        assert!(add_edge(SRC, "a", "zz").is_none());
    }

    #[test]
    fn deletes() {
        let c = chart(SRC);
        let bc = c
            .edges
            .iter()
            .position(|e| e.from == "b" && e.to == "c")
            .unwrap();
        let out = delete(SRC, &[], &[bc]).unwrap().apply(SRC);
        assert_eq!(
            out,
            "digraph G {\n    a [label=\"Start\"];\n    a -> b [color=red]\n    c;\n    b -> d\n}\n"
        );
        let out = delete(SRC, &["b"], &[]).unwrap().apply(SRC);
        assert_eq!(
            out,
            "digraph G {\n    a [label=\"Start\"];\n    c;\n    d\n}\n"
        );
        let out = delete(SRC, &["a"], &[]).unwrap().apply(SRC);
        assert_eq!(out, "digraph G {\n    b -> c [color=red];\n    b -> d\n}\n");
        assert!(delete("digraph { a -> {b c} }", &["b"], &[]).is_none());
        assert!(delete(SRC, &[], &[]).is_none());
    }
}
