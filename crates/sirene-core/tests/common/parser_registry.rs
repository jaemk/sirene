//! Parser selectors shared by contract tests and the independent fuzz workspace.

use std::borrow::Cow;

use sirene_core::{Diagnostic, diagram::Kind, syntax::SyntaxNode};

pub struct Parser {
    pub name: &'static str,
    pub kind: Kind,
    pub header: &'static str,
    pub normalize_cr: bool,
    pub parse: fn(&str) -> (SyntaxNode, Vec<Diagnostic>),
}

macro_rules! entry {
    ($module:ident, $kind:ident, $header:literal, $normalize:literal) => {
        Parser {
            name: stringify!($module),
            kind: Kind::$kind,
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
    entry!(flowchart, Flowchart, "flowchart TD", true),
    entry!(architecture, Architecture, "architecture-beta", false),
    entry!(block, Block, "block-beta", false),
    entry!(c4, C4, "C4Context", false),
    entry!(class, Class, "classDiagram", false),
    entry!(cynefin, Cynefin, "cynefin-beta", false),
    entry!(dot, Dot, "digraph {", false),
    entry!(er, Er, "erDiagram", false),
    entry!(eventmodel, EventModel, "eventmodeling", false),
    entry!(gantt, Gantt, "gantt", false),
    entry!(git, Git, "gitGraph", false),
    entry!(info, Info, "info", false),
    entry!(ishikawa, Ishikawa, "ishikawa-beta", false),
    entry!(journey, Journey, "journey", false),
    entry!(kanban, Kanban, "kanban", false),
    entry!(mindmap, Mindmap, "mindmap", false),
    entry!(packet, Packet, "packet-beta", false),
    entry!(pie, Pie, "pie", false),
    entry!(plantuml, PlantUml, "@startuml", true),
    entry!(quadrant, Quadrant, "quadrantChart", false),
    entry!(radar, Radar, "radar-beta", false),
    entry!(railroad, Railroad, "railroad-beta", false),
    entry!(requirement, Requirement, "requirementDiagram", false),
    entry!(sankey, Sankey, "sankey-beta", false),
    entry!(sequence, Sequence, "sequenceDiagram", false),
    entry!(state, State, "stateDiagram-v2", false),
    entry!(timeline, Timeline, "timeline", false),
    entry!(treemap, Treemap, "treemap-beta", false),
    entry!(treeview, TreeView, "treeView-beta", false),
    entry!(usecase, UseCase, "usecase-beta", false),
    entry!(venn, Venn, "venn-beta", false),
    entry!(wardley, Wardley, "wardley-beta", false),
    entry!(xychart, XyChart, "xychart-beta", false),
    entry!(zenuml, ZenUml, "zenuml", false),
];

impl Parser {
    pub fn syntax_source<'a>(&self, src: &'a str) -> Cow<'a, str> {
        if !self.normalize_cr {
            return Cow::Borrowed(src);
        }
        // Direct flowchart and PlantUML entry points replace a CR outside CRLF.
        let mut chars = src.chars().peekable();
        let mut out = String::with_capacity(src.len());
        while let Some(c) = chars.next() {
            out.push(if c == '\r' && chars.peek() != Some(&'\n') {
                '\n'
            } else {
                c
            });
        }
        Cow::Owned(out)
    }
}

pub fn check_ranges(src: &str, diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        let start = usize::from(d.range.start());
        let end = usize::from(d.range.end());
        assert!(
            start <= end && src.get(start..end).is_some(),
            "{d:?} in {src:?}"
        );
    }
}
