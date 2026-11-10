//! Mindmaps as text (MMIND-4): an outline of the tree, each node in its shape's delimiters.

use super::{Charset, Options, width};
use crate::mindmap::{Mindmap, Node};

/// The branch, last branch, continuing, and blank prefixes.
fn glyphs(charset: Charset) -> [&'static str; 4] {
    match charset {
        Charset::Unicode => ["├── ", "└── ", "│   ", "    "],
        Charset::Ascii => ["+-- ", "`-- ", "|   ", "    "],
    }
}

/// A node's display lines, the first opened and the last closed by its shape's delimiters.
fn label(node: &Node) -> Vec<String> {
    let (open, close) = node.shape.delimiters();
    let mut lines = node.label.lines();
    if lines.is_empty() {
        lines.push(String::new());
    }
    let pad = " ".repeat(open.len());
    let last = lines.len() - 1;
    lines
        .into_iter()
        .enumerate()
        .map(|(k, l)| {
            let mut s = if k == 0 {
                format!("{open}{l}")
            } else {
                format!("{pad}{l}")
            };
            if k == last {
                s.push_str(close);
            }
            s
        })
        .collect()
}

/// Renders `mindmap` as text (MMIND-4).
pub fn render(mindmap: &Mindmap, options: &Options) -> String {
    let [branch, last, through, blank] = glyphs(options.charset);
    let mut rows: Vec<String> = Vec::new();
    if let Some(root) = mindmap.nodes.first() {
        rows.extend(label(root));
        // Depth-first: each entry is a node and the prefix of its line.
        let mut stack: Vec<(usize, String)> = Vec::new();
        let push_kids = |stack: &mut Vec<(usize, String)>, i: usize, prefix: &str| {
            for &c in mindmap.nodes[i].children.iter().rev() {
                stack.push((c, prefix.to_owned()));
            }
        };
        push_kids(&mut stack, 0, "");
        while let Some((i, prefix)) = stack.pop() {
            let node = &mindmap.nodes[i];
            let parent = node.parent.expect("a non-root node has a parent");
            let is_last = mindmap.nodes[parent].children.last() == Some(&i);
            let (head, rest) = if is_last {
                (last, blank)
            } else {
                (branch, through)
            };
            // Continuation lines line up with the first, under the node's own prefix.
            let child_prefix = format!("{prefix}{rest}");
            for (k, l) in label(node).into_iter().enumerate() {
                if k == 0 {
                    rows.push(format!("{prefix}{head}{l}"));
                } else {
                    rows.push(format!("{child_prefix}{l}"));
                }
            }
            push_kids(&mut stack, i, &child_prefix);
        }
    }
    let mut out = String::new();
    if let Some(t) = mindmap
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
        let pad = (w - width(t)).max(0) / 2;
        out.push_str(&" ".repeat(pad as usize));
        out.push_str(t);
        out.push('\n');
        if !rows.is_empty() {
            out.push('\n');
        }
    }
    for r in rows {
        out.push_str(r.trim_end());
        out.push('\n');
    }
    out
}
