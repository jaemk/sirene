//! Parser selectors shared by contract tests and the independent fuzz workspace.

use sirene_core::{Diagnostic, syntax::SyntaxNode};

pub struct Parser {
    pub name: &'static str,
    pub header: &'static str,
    pub normalize_cr: bool,
    pub parse: fn(&str) -> (SyntaxNode, Vec<Diagnostic>),
}

macro_rules! entry {
    ($module:ident, $header:literal, $normalize:literal) => {
        Parser {
            name: stringify!($module),
            header: $header,
            normalize_cr: $normalize,
            parse: |src| {
                let parsed = sirene_core::$module::parse(src);
                (parsed.syntax(), parsed.diagnostics().to_vec())
            },
        }
    };
}

// Order and headers are part of the fuzz corpus format: the first byte selects an entry.
pub static PARSERS: &[Parser] = &[
    entry!(flowchart, "flowchart TD", true),
    entry!(architecture, "architecture-beta", false),
    entry!(block, "block-beta", false),
    entry!(c4, "C4Context", false),
    entry!(class, "classDiagram", false),
    entry!(cynefin, "cynefin-beta", false),
    entry!(dot, "digraph {", false),
    entry!(er, "erDiagram", false),
    entry!(eventmodel, "eventmodeling", false),
    entry!(gantt, "gantt", false),
    entry!(git, "gitGraph", false),
    entry!(info, "info", false),
    entry!(ishikawa, "ishikawa-beta", false),
    entry!(journey, "journey", false),
    entry!(kanban, "kanban", false),
    entry!(mindmap, "mindmap", false),
    entry!(packet, "packet-beta", false),
    entry!(pie, "pie", false),
    entry!(plantuml, "@startuml", true),
    entry!(quadrant, "quadrantChart", false),
    entry!(radar, "radar-beta", false),
    entry!(railroad, "railroad-beta", false),
    entry!(requirement, "requirementDiagram", false),
    entry!(sankey, "sankey-beta", false),
    entry!(sequence, "sequenceDiagram", false),
    entry!(state, "stateDiagram-v2", false),
    entry!(timeline, "timeline", false),
    entry!(treemap, "treemap-beta", false),
    entry!(treeview, "treeView-beta", false),
    entry!(usecase, "usecase-beta", false),
    entry!(venn, "venn-beta", false),
    entry!(wardley, "wardley-beta", false),
    entry!(xychart, "xychart-beta", false),
    entry!(zenuml, "zenuml", false),
];
