//! Mermaid git graphs (MGIT-*): branches as lanes of commits, with merges and cherry-picks.

mod lower;
mod parser;

use rowan::{GreenNode, TextRange};

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// Which way commits run along their lanes (MGIT-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    LeftRight,
    TopBottom,
    BottomTop,
}

/// How a commit is drawn (`type:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitType {
    #[default]
    Normal,
    Reverse,
    Highlight,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub id: String,
    pub kind: CommitType,
    /// The branch it is on, an index into `GitGraph::branches`.
    pub branch: usize,
    /// Its parents, indexes into `GitGraph::commits`: none for a root, two for a merge (the
    /// branch it was made on first).
    pub parents: Vec<usize>,
    pub tags: Vec<String>,
    /// `msg:`, kept but not drawn.
    pub message: Option<String>,
    /// For a cherry-pick, the id of the commit copied.
    pub cherry_pick: Option<String>,
    pub range: TextRange,
}

impl Commit {
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// A git graph (MGIT-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GitGraph {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub direction: Direction,
    /// The branches in drawing order, `main` first unless an `order` moves it.
    pub branches: Vec<Branch>,
    /// The commits in source order, which is their order along the lanes.
    pub commits: Vec<Commit>,
}

/// The result of parsing git graph source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`GitGraph`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (GitGraph, Vec<Diagnostic>) {
        let (graph, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a git graph: {} branches, {} commits; {} diagnostics",
            graph.branches.len(),
            graph.commits.len(),
            diagnostics.len()
        );
        (graph, diagnostics)
    }
}

/// Parses git graph source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
