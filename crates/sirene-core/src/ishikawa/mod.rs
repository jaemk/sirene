//! Mermaid ishikawa (fishbone) diagrams (MISH-*): an effect and the causes that lead to it,
//! nested by indentation.

mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::lines::split_br;
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

/// A cause, or the effect at the root.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cause {
    /// The label's lines, split at `<br>`.
    pub lines: Vec<String>,
    pub children: Vec<Cause>,
}

impl Cause {
    /// The number of causes under this one, at any depth.
    pub fn descendants(&self) -> usize {
        self.children.iter().map(|c| 1 + c.descendants()).sum()
    }
}

/// An ishikawa diagram (MISH-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ishikawa {
    /// The frontmatter title.
    pub title: Option<String>,
    /// The first line: the effect, with the causes as its children. `None` when the diagram has
    /// no lines.
    pub effect: Option<Cause>,
}

/// The result of parsing ishikawa source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to an [`Ishikawa`], with the parser's diagnostics.
    pub fn lower(&self) -> (Ishikawa, Vec<Diagnostic>) {
        let root = self.syntax();
        let src = root.to_string();
        let mut chart = Ishikawa::default();
        // Each line's label and indentation, in order.
        let mut lines: Vec<(Vec<String>, usize)> = Vec::new();
        for el in root.children_with_tokens() {
            if el.kind() == FRONTMATTER
                && let Some(tok) = el.as_token()
            {
                chart.title = text::frontmatter_title(tok.text());
            }
        }
        for node in root.descendants().filter(|n| n.kind() == NODE) {
            let start = usize::from(node.text_range().start());
            let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
            let level = src[line_start..start].chars().count();
            let label = split_br(&text::decode_entities(node.text().to_string().trim()));
            lines.push((label, level));
        }
        chart.effect = tree(lines);
        log::debug!("lowered an ishikawa diagram");
        (chart, self.diagnostics.clone())
    }
}

/// The tree of `lines` as Mermaid builds it: the first line is the root; the first cause's
/// indentation is level 1, and each line is a child of the nearest line above it with a lower
/// level (MISH-2).
fn tree(lines: Vec<(Vec<String>, usize)>) -> Option<Cause> {
    let mut it = lines.into_iter();
    let (root, _) = it.next()?;
    // An arena: each node's label, children, and level.
    let mut nodes: Vec<(Vec<String>, Vec<usize>)> = vec![(root, Vec::new())];
    let mut stack: Vec<(usize, usize)> = vec![(0, 0)];
    let mut base = None;
    for (label, raw) in it {
        let base = *base.get_or_insert(raw);
        let level = (raw as isize - base as isize + 1).max(1) as usize;
        while stack.len() > 1 && stack.last().is_some_and(|&(l, _)| l >= level) {
            stack.pop();
        }
        let parent = stack.last().expect("the root stays on the stack").1;
        let i = nodes.len();
        nodes.push((label, Vec::new()));
        nodes[parent].1.push(i);
        stack.push((level, i));
    }
    fn build(nodes: &[(Vec<String>, Vec<usize>)], i: usize) -> Cause {
        Cause {
            lines: nodes[i].0.clone(),
            children: nodes[i].1.iter().map(|&c| build(nodes, c)).collect(),
        }
    }
    Some(build(&nodes, 0))
}

/// Parses ishikawa source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(s: &str, level: usize) -> (Vec<String>, usize) {
        (vec![s.to_owned()], level)
    }

    fn shape(c: &Cause) -> String {
        if c.children.is_empty() {
            c.lines.join(" ")
        } else {
            let kids: Vec<String> = c.children.iter().map(shape).collect();
            format!("{}({})", c.lines.join(" "), kids.join(","))
        }
    }

    #[test]
    fn trees() {
        assert_eq!(tree(Vec::new()), None);
        let t = tree(vec![
            line("E", 4),
            line("a", 2),
            line("b", 4),
            line("c", 6),
            line("d", 4),
            line("e", 2),
            line("f", 0),
        ])
        .unwrap();
        // The effect may be indented more than the causes; a line left of the first cause is a
        // cause too.
        assert_eq!(shape(&t), "E(a(b(c),d),e,f)");
        assert_eq!(t.descendants(), 6);
    }
}
