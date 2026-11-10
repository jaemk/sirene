//! Collects a sankey diagram's nodes and links, leaving out any link that would close a cycle
//! (MSANKEY-2).

use super::{Align, Link, LinkColor, Sankey};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::scene::parse_color;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Sankey, Vec<Diagnostic>) {
    let mut s = Sankey::default();
    let mut diagnostics = Vec::new();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            s.title = text::frontmatter_title(tok.text());
            for (k, v) in text::frontmatter_config(tok.text(), "sankey") {
                if config(&mut s, &k, &v).is_none() {
                    diagnostics.push(Diagnostic::warning(
                        tok.text_range(),
                        format!("ignoring `{k}: {v}`: not a valid value"),
                    ));
                }
            }
        }
    }
    for row in root.children().filter(|r| r.kind() == FLOW) {
        let names: Vec<String> = tokens(&row)
            .filter(|t| matches!(t.kind(), TEXT | STRING))
            .map(|t| name(&t))
            .collect();
        let Some(value) = tokens(&row).find(|t| t.kind() == NUMBER) else {
            continue;
        };
        let [source, target] = &names[..] else {
            continue;
        };
        let v: f64 = value.text().parse().unwrap_or(0.0);
        if v < 0.0 {
            diagnostics.push(Diagnostic::new(
                value.text_range(),
                format!("`{source}` -> `{target}` has a negative value; ignoring it"),
            ));
            continue;
        }
        let a = node(&mut s, source);
        let b = node(&mut s, target);
        if let Some(path) = path(&s, b, a) {
            let names: Vec<String> = std::iter::once(&a)
                .chain(&path)
                .map(|&i| format!("`{}`", s.nodes[i]))
                .collect();
            let cycle = if a == b {
                format!("`{source}` flows into itself")
            } else {
                format!("it closes the cycle {}", names.join(" -> "))
            };
            diagnostics.push(Diagnostic::new(
                row.text_range(),
                format!("`{source}` -> `{target}`: {cycle}, which a sankey diagram cannot draw; ignoring it"),
            ));
            continue;
        }
        s.links.push(Link {
            source: a,
            target: b,
            value: v,
        });
    }
    // A node only named by links that were left out has nothing to draw.
    let used: Vec<bool> = (0..s.nodes.len())
        .map(|i| s.links.iter().any(|l| l.source == i || l.target == i))
        .collect();
    if used.iter().any(|u| !u) {
        let mut map = vec![0; s.nodes.len()];
        let mut kept = Vec::new();
        for (i, name) in std::mem::take(&mut s.nodes).into_iter().enumerate() {
            if used[i] {
                map[i] = kept.len();
                kept.push(name);
            }
        }
        s.nodes = kept;
        for l in &mut s.links {
            l.source = map[l.source];
            l.target = map[l.target];
        }
    }
    (s, diagnostics)
}

/// Applies one `config.sankey` key, or returns `None` for a bad value. Unknown keys are kept
/// quiet.
fn config(s: &mut Sankey, k: &str, v: &str) -> Option<()> {
    let n = v.parse::<f32>().ok().filter(|n| n.is_finite());
    match k {
        "width" => s.width = n.filter(|&n| n > 0.0)?,
        "height" => s.height = n.filter(|&n| n > 0.0)?,
        "nodeWidth" => s.node_width = n.filter(|&n| n > 0.0)?,
        "nodePadding" => s.node_padding = Some(n.filter(|&n| n >= 0.0)?),
        "showValues" => s.show_values = v.parse().ok()?,
        "prefix" => s.prefix = v.to_owned(),
        "suffix" => s.suffix = v.to_owned(),
        "linkColor" => {
            s.link_color = match v {
                "source" => LinkColor::Source,
                "target" => LinkColor::Target,
                "gradient" => LinkColor::Gradient,
                _ => {
                    parse_color(v)?;
                    LinkColor::Fixed(v.to_owned())
                }
            }
        }
        "nodeAlignment" => {
            s.align = match v {
                "justify" => Align::Justify,
                "left" => Align::Left,
                "right" => Align::Right,
                "center" => Align::Center,
                _ => return None,
            }
        }
        _ => {}
    }
    Some(())
}

/// A field's name: unquoted, with `""` read as `"`.
fn name(t: &SyntaxToken) -> String {
    match t.kind() {
        STRING => {
            let s = t.text();
            s[1..s.len() - 1].replace("\"\"", "\"")
        }
        _ => t.text().to_owned(),
    }
}

/// The index of node `name`, added when new.
fn node(s: &mut Sankey, name: &str) -> usize {
    match s.nodes.iter().position(|n| n == name) {
        Some(i) => i,
        None => {
            s.nodes.push(name.to_owned());
            s.nodes.len() - 1
        }
    }
}

/// The nodes on a path of links from `from` to `to`, both included, if there is one; a node
/// always reaches itself.
fn path(s: &Sankey, from: usize, to: usize) -> Option<Vec<usize>> {
    if from == to {
        return Some(vec![from]);
    }
    // Breadth first, remembering how each node was reached.
    let mut came = vec![None; s.nodes.len()];
    let mut queue = std::collections::VecDeque::from([from]);
    let mut seen = vec![false; s.nodes.len()];
    seen[from] = true;
    while let Some(i) = queue.pop_front() {
        for l in s.links.iter().filter(|l| l.source == i) {
            if seen[l.target] {
                continue;
            }
            seen[l.target] = true;
            came[l.target] = Some(i);
            if l.target == to {
                let mut out = Vec::new();
                let mut at = to;
                while let Some(prev) = came[at] {
                    out.push(prev);
                    at = prev;
                }
                out.reverse();
                out.push(to);
                return Some(out);
            }
            queue.push_back(l.target);
        }
    }
    None
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}
