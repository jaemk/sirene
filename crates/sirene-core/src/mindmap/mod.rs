//! Mermaid mindmaps (MMIND-*): a tree of ideas drawn out from a root.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Label;
use crate::syntax::SyntaxNode;

/// A node's shape, named by its delimiters (MMIND-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// No delimiters: a box with a line under it.
    #[default]
    Default,
    /// `[text]`.
    Square,
    /// `(text)`.
    Rounded,
    /// `((text))`.
    Circle,
    /// `)text(`.
    Cloud,
    /// `))text((`.
    Bang,
    /// `{{text}}`.
    Hexagon,
}

impl Shape {
    /// The shape `open` starts.
    pub(crate) fn of(open: &str) -> Shape {
        match open {
            "[" => Shape::Square,
            "(" => Shape::Rounded,
            "((" => Shape::Circle,
            ")" => Shape::Cloud,
            "))" => Shape::Bang,
            "{{" => Shape::Hexagon,
            _ => Shape::Default,
        }
    }

    /// The delimiters around a node's text, empty for the default shape.
    pub fn delimiters(self) -> (&'static str, &'static str) {
        match self {
            Shape::Default => ("", ""),
            Shape::Square => ("[", "]"),
            Shape::Rounded => ("(", ")"),
            Shape::Circle => ("((", "))"),
            Shape::Cloud => (")", "("),
            Shape::Bang => ("))", "(("),
            Shape::Hexagon => ("{{", "}}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The text before the shape; `None` for a shape alone.
    pub id: Option<String>,
    pub label: Label,
    pub shape: Shape,
    /// `::icon(name)`, kept but not drawn.
    pub icon: Option<String>,
    /// `:::a b` class names.
    pub classes: Vec<String>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// 0 for the root.
    pub depth: usize,
    /// The index of the root's child this node descends from, which picks its color; `None`
    /// for the root.
    pub section: Option<usize>,
}

/// A mindmap (MMIND-2). The first node is the root; each node comes after its parent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mindmap {
    pub title: Option<String>,
    pub nodes: Vec<Node>,
}

impl Mindmap {
    /// The nodes under `i` in order, each with its depth below `i`, depth first.
    pub fn descendants(&self, i: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(usize, usize)> = self.nodes[i]
            .children
            .iter()
            .rev()
            .map(|&c| (c, 1))
            .collect();
        while let Some((n, d)) = stack.pop() {
            out.push((n, d));
            stack.extend(self.nodes[n].children.iter().rev().map(|&c| (c, d + 1)));
        }
        out
    }
}

/// The result of parsing mindmap source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Mindmap`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Mindmap, Vec<Diagnostic>) {
        let (mindmap, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a mindmap: {} nodes; {} diagnostics",
            mindmap.nodes.len(),
            diagnostics.len()
        );
        (mindmap, diagnostics)
    }
}

/// Parses mindmap source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
