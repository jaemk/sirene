//! Mermaid sankey diagrams (MSANKEY-*): flows between nodes, as bands as wide as their values.

mod layout;
mod lower;
mod parser;

use rowan::GreenNode;

pub use layout::{Layout, LinkBand, NodeBox};
pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// A flow of `value` from node `source` to node `target`, by index into the nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub source: usize,
    pub target: usize,
    pub value: f64,
}

/// How links are colored (`linkColor`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LinkColor {
    Source,
    Target,
    /// From the source's color to the target's.
    #[default]
    Gradient,
    /// One color for every link, such as `#a1a1a1`.
    Fixed(String),
}

/// Which column a node goes in (`nodeAlignment`), as in d3-sankey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// By depth, with the nodes nothing leaves in the last column.
    #[default]
    Justify,
    /// By depth.
    Left,
    /// By the longest path to a node nothing leaves, counted from the last column.
    Right,
    /// By depth, with a node nothing enters just before its nearest target.
    Center,
}

/// A sankey diagram (MSANKEY-2).
#[derive(Debug, Clone, PartialEq)]
pub struct Sankey {
    /// Frontmatter `title`.
    pub title: Option<String>,
    /// Node names, in the order they first appear.
    pub nodes: Vec<String>,
    pub links: Vec<Link>,
    /// Frontmatter `config.sankey`: the flow area's `width` (600) and `height` (400).
    pub width: f32,
    pub height: f32,
    pub link_color: LinkColor,
    pub align: Align,
    /// `showValues`: each node's value under its name, on unless `false`.
    pub show_values: bool,
    /// `prefix` and `suffix` around each shown value, such as `$` or ` TWh`.
    pub prefix: String,
    pub suffix: String,
    /// `nodeWidth`, 10 unless set.
    pub node_width: f32,
    /// `nodePadding`: the least space between nodes in a column; unset, 10, plus 15 when values
    /// are shown, as in Mermaid.
    pub node_padding: Option<f32>,
}

impl Default for Sankey {
    fn default() -> Self {
        Sankey {
            title: None,
            nodes: Vec::new(),
            links: Vec::new(),
            width: 600.0,
            height: 400.0,
            link_color: LinkColor::Gradient,
            align: Align::Justify,
            show_values: true,
            prefix: String::new(),
            suffix: String::new(),
            node_width: 10.0,
            node_padding: None,
        }
    }
}

impl Sankey {
    /// The least space between nodes in a column.
    pub fn padding(&self) -> f32 {
        self.node_padding
            .unwrap_or(if self.show_values { 25.0 } else { 10.0 })
    }

    /// A value as shown: rounded to two decimals, with the prefix and suffix.
    pub fn format_value(&self, v: f64) -> String {
        let rounded = (v * 100.0).round() / 100.0;
        format!(
            "{}{}{}",
            self.prefix,
            crate::xychart::format_number(rounded),
            self.suffix
        )
    }
}

/// The result of parsing sankey source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Sankey`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Sankey, Vec<Diagnostic>) {
        let (sankey, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a sankey diagram: {} nodes, {} links; {} diagnostics",
            sankey.nodes.len(),
            sankey.links.len(),
            diagnostics.len()
        );
        (sankey, diagnostics)
    }
}

/// Parses sankey source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        let mut s = Sankey::default();
        assert_eq!(s.format_value(1.005), "1");
        assert_eq!(s.format_value(2.456), "2.46");
        s.prefix = "$".into();
        s.suffix = " TWh".into();
        assert_eq!(s.format_value(10.0), "$10 TWh");
        assert_eq!(s.padding(), 25.0);
        s.show_values = false;
        assert_eq!(s.padding(), 10.0);
        s.node_padding = Some(4.0);
        assert_eq!(s.padding(), 4.0);
    }
}
