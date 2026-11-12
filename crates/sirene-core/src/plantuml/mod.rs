//! PlantUML diagrams (PUML-*): sequence diagrams lowered to the sequence model (PSEQ-*), and
//! the other kinds lowered to the graph model, one module each.

mod activity;
mod chen;
mod chronology;
mod class;
mod data;
mod description;
mod detect;
mod embedded;
mod files;
mod gantt;
mod lines;
mod link;
mod lower;
mod mindmap;
mod nwdiag;
mod parser;
mod regex;
mod state;
mod timing;

use rowan::GreenNode;

pub use detect::{Kind, kind};

use crate::diagnostic::Diagnostic;
use crate::diagram::Diagram;
use crate::flowchart::scan;
use crate::syntax::SyntaxNode;

/// Whether sirene draws diagrams of `kind` yet (PUML-2).
pub(crate) fn supported(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Sequence
            | Kind::Activity
            | Kind::Class
            | Kind::State
            | Kind::Description
            | Kind::Mindmap
            | Kind::Wbs
            | Kind::Gantt
            | Kind::Timing
            | Kind::Json
            | Kind::Yaml
            | Kind::Other("dot" | "ebnf" | "files" | "regex" | "chen" | "nwdiag" | "chronology")
    )
}

/// Whether `kind`'s documents are written in another language, whose lines PlantUML's `'`
/// comments do not apply to.
pub(crate) fn raw(kind: Kind) -> bool {
    matches!(kind, Kind::Other("dot" | "ebnf" | "regex"))
}

/// The result of parsing PlantUML source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
    kind: Kind,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The document's kind (PUML-1).
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Lowers the tree to its model: a sequence diagram, or a graph for the other kinds. The
    /// returned diagnostics include the parser's, ordered by position.
    pub fn lower(&self) -> (Diagram, Vec<Diagnostic>) {
        let root = self.syntax();
        let (diagram, lowering) = match self.kind {
            Kind::Activity => {
                let (chart, d) = activity::lower(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Class => {
                let (chart, d) = class::lower(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::State => {
                let (chart, d) = state::lower(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Description => {
                let (chart, d) = description::lower(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Mindmap => {
                let (map, d) = mindmap::lower_mindmap(&root);
                (Diagram::Mindmap(map), d)
            }
            Kind::Gantt => {
                let (g, d) = gantt::lower(&root);
                (Diagram::Gantt(g), d)
            }
            Kind::Timing => {
                let (t, d) = timing::lower(&root);
                (Diagram::Timing(t), d)
            }
            Kind::Json => {
                let (chart, d) = data::lower_json(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Yaml => {
                let (chart, d) = data::lower_yaml(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Other("dot") => {
                let (chart, d) = embedded::lower_dot(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Other("ebnf") => {
                let (chart, d) = embedded::lower_ebnf(&root);
                (Diagram::Railroad(chart), d)
            }
            Kind::Other("chen") => {
                let (chart, d) = chen::lower(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Other("chronology") => {
                let (t, d) = chronology::lower(&root);
                (Diagram::Timeline(t), d)
            }
            Kind::Other("nwdiag") => {
                let (chart, d) = nwdiag::lower(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Other("regex") => {
                let (chart, d) = regex::lower(&root);
                (Diagram::Railroad(chart), d)
            }
            Kind::Other("files") => {
                let (view, d) = files::lower(&root);
                (Diagram::TreeView(view), d)
            }
            Kind::Wbs => {
                let (chart, d) = mindmap::lower_wbs(&root);
                (Diagram::Graph(chart), d)
            }
            Kind::Sequence => {
                let (seq, d) = lower::lower(&root);
                (Diagram::Sequence(seq), d)
            }
            // The parser reports the kinds not drawn yet; their drawing is empty.
            _ => (Diagram::default(), Vec::new()),
        };
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a PlantUML {} diagram; {} diagnostics",
            self.kind.name(),
            diagnostics.len()
        );
        (diagram, diagnostics)
    }
}

/// Parses PlantUML source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let kind = kind(src);
    let (green, diagnostics) = parser::parse(src, kind);
    Parse {
        green,
        diagnostics,
        kind,
    }
}

/// Whether `src` has a PlantUML `@start...` line, which may follow other text as PlantUML
/// allows (PSEQ-1, LANGS-1).
pub fn is_plantuml(src: &str) -> bool {
    src[scan::bom_len(src)..]
        .lines()
        .any(|l| l.trim_start().starts_with("@start"))
}
