//! The entry point for every Mermaid diagram type sirene draws (LANGS-2).

use crate::architecture::{self, Architecture};
use crate::ascii::{self, Options};
use crate::block::{self, Block};
use crate::c4::{self, C4};
use crate::cynefin::{self, Cynefin};
use crate::diagnostic::Diagnostic;
use crate::eventmodel::{self, EventModel};
use crate::flowchart::{self, Flowchart, scan};
use crate::gantt::{self, Gantt};
use crate::git::{self, GitGraph};
use crate::info::{self, Info};
use crate::ishikawa::{self, Ishikawa};
use crate::journey::{self, Journey};
use crate::kanban::{self, Kanban};
use crate::mindmap::{self, Mindmap};
use crate::packet::{self, Packet};
use crate::pie::{self, Pie};
use crate::quadrant::{self, QuadrantChart};
use crate::radar::{self, Radar};
use crate::railroad::{self, Railroad};
use crate::sankey::{self, Sankey};
use crate::scene::{self, Scene, TextMetrics, Theme};
use crate::sequence::{self, Sequence};
use crate::timeline::{self, Timeline};
use crate::treemap::{self, Treemap};
use crate::treeview::{self, TreeView};
use crate::venn::{self, Venn};
use crate::wardley::{self, Wardley};
use crate::xychart::{self, XyChart};
use crate::{class, er, requirement, state, usecase, zenuml};

/// A Mermaid diagram type sirene draws, picked by the header keyword (LANGS-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Flowchart,
    Sequence,
    State,
    Class,
    Er,
    Git,
    Timeline,
    Pie,
    Gantt,
    Mindmap,
    Journey,
    Quadrant,
    XyChart,
    Requirement,
    Kanban,
    Packet,
    Radar,
    Sankey,
    Treemap,
    Block,
    Architecture,
    C4,
    ZenUml,
    Info,
    Ishikawa,
    Venn,
    Wardley,
    TreeView,
    UseCase,
    Railroad,
    /// A flowchart whose top-level subgraphs are lanes (`swimlane-beta`).
    Swimlane,
    Cynefin,
    EventModel,
}

impl Kind {
    /// The type of the diagram `src` starts with; a flowchart unless the header says otherwise.
    pub fn of(src: &str) -> Kind {
        let word = header(src);
        // `gitGraph:` and `gitGraph TB:` name the direction in the same word or the next.
        let word = word.strip_suffix(':').unwrap_or(word);
        if sequence::HEADERS.contains(&word) {
            Kind::Sequence
        } else if state::HEADERS.contains(&word) {
            Kind::State
        } else if class::HEADERS.contains(&word) {
            Kind::Class
        } else if er::HEADERS.contains(&word) {
            Kind::Er
        } else if git::HEADERS.contains(&word) {
            Kind::Git
        } else if timeline::HEADERS.contains(&word) {
            Kind::Timeline
        } else if pie::HEADERS.contains(&word) {
            Kind::Pie
        } else if gantt::HEADERS.contains(&word) {
            Kind::Gantt
        } else if mindmap::HEADERS.contains(&word) {
            Kind::Mindmap
        } else if journey::HEADERS.contains(&word) {
            Kind::Journey
        } else if quadrant::HEADERS.contains(&word) {
            Kind::Quadrant
        } else if xychart::HEADERS.contains(&word) {
            Kind::XyChart
        } else if requirement::HEADERS.contains(&word) {
            Kind::Requirement
        } else if kanban::HEADERS.contains(&word) {
            Kind::Kanban
        } else if packet::HEADERS.contains(&word) {
            Kind::Packet
        } else if radar::HEADERS.contains(&word) {
            Kind::Radar
        } else if sankey::HEADERS.contains(&word) {
            Kind::Sankey
        } else if treemap::HEADERS.contains(&word) {
            Kind::Treemap
        } else if block::HEADERS.contains(&word) {
            Kind::Block
        } else if architecture::HEADERS.contains(&word) {
            Kind::Architecture
        } else if c4::HEADERS.contains(&word) {
            Kind::C4
        } else if zenuml::HEADERS.contains(&word) {
            Kind::ZenUml
        } else if info::HEADERS.contains(&word) {
            Kind::Info
        } else if ishikawa::HEADERS.contains(&word) {
            Kind::Ishikawa
        } else if venn::HEADERS.contains(&word) {
            Kind::Venn
        } else if wardley::HEADERS.contains(&word) {
            Kind::Wardley
        } else if treeview::HEADERS.contains(&word) {
            Kind::TreeView
        } else if usecase::HEADERS.contains(&word) {
            Kind::UseCase
        } else if railroad::HEADERS.contains(&word) {
            Kind::Railroad
        } else if word == flowchart::SWIMLANE {
            Kind::Swimlane
        } else if cynefin::HEADERS.contains(&word) {
            Kind::Cynefin
        } else if eventmodel::HEADERS.contains(&word) {
            Kind::EventModel
        } else {
            Kind::Flowchart
        }
    }
}

/// The Mermaid types sirene draws, for messages about the others (LANGS-4).
pub const DRAWN: &str = "flowchart, sequence, state, class, ER, git graph, timeline, pie, gantt, \
     mindmap, journey, quadrant, XY, requirement, kanban, packet, radar, sankey, treemap, block, architecture, C4, ZenUML, info, ishikawa, venn, Wardley, tree view, use case, railroad, swimlane, Cynefin, and event modeling diagrams";

/// A lowered diagram of any type (LANGS-2).
#[derive(Debug, Clone, PartialEq)]
pub enum Diagram {
    /// Flowcharts, and the state, class, and ER diagrams lowered to the graph model.
    Graph(Flowchart),
    Sequence(Sequence),
    Git(GitGraph),
    Timeline(Timeline),
    Pie(Pie),
    Gantt(Gantt),
    Mindmap(Mindmap),
    Journey(Journey),
    Quadrant(QuadrantChart),
    XyChart(XyChart),
    Kanban(Kanban),
    Packet(Packet),
    Radar(Radar),
    Sankey(Sankey),
    Treemap(Treemap),
    Block(Block),
    Architecture(Architecture),
    C4(C4),
    Info(Info),
    Ishikawa(Ishikawa),
    Venn(Venn),
    Wardley(Wardley),
    TreeView(TreeView),
    Railroad(Railroad),
    Cynefin(Cynefin),
    EventModel(EventModel),
}

impl Default for Diagram {
    fn default() -> Self {
        Diagram::Graph(Flowchart::default())
    }
}

impl Diagram {
    /// The graph model, for the types lowered to it.
    pub fn graph(&self) -> Option<&Flowchart> {
        match self {
            Diagram::Graph(f) => Some(f),
            _ => None,
        }
    }

    /// The graph model, or an empty one for the other types.
    pub fn into_graph(self) -> Flowchart {
        match self {
            Diagram::Graph(f) => f,
            _ => Flowchart::default(),
        }
    }

    /// The drawing, with text measured by `metrics` (DISPLA-1).
    pub fn scene(&self, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
        match self {
            Diagram::Graph(f) => scene::flowchart(f, metrics, theme),
            Diagram::Sequence(s) => scene::sequence(s, metrics, theme),
            Diagram::Git(g) => scene::git_graph(g, metrics, theme),
            Diagram::Timeline(t) => scene::timeline(t, metrics, theme),
            Diagram::Pie(p) => scene::pie(p, metrics, theme),
            Diagram::Gantt(g) => scene::gantt(g, metrics, theme),
            Diagram::Mindmap(m) => scene::mindmap(m, metrics, theme),
            Diagram::Journey(j) => scene::journey(j, metrics, theme),
            Diagram::Quadrant(q) => scene::quadrant(q, metrics, theme),
            Diagram::XyChart(c) => scene::xychart(c, metrics, theme),
            Diagram::Kanban(k) => scene::kanban(k, metrics, theme),
            Diagram::Packet(p) => scene::packet(p, metrics, theme),
            Diagram::Radar(r) => scene::radar(r, metrics, theme),
            Diagram::Sankey(s) => scene::sankey(s, metrics, theme),
            Diagram::Treemap(t) => scene::treemap(t, metrics, theme),
            Diagram::Block(b) => scene::block(b, metrics, theme),
            Diagram::Architecture(a) => scene::architecture(a, metrics, theme),
            Diagram::C4(c) => scene::c4(c, metrics, theme),
            Diagram::Info(i) => scene::info(i, metrics, theme),
            Diagram::Ishikawa(i) => scene::ishikawa(i, metrics, theme),
            Diagram::Venn(v) => scene::venn(v, metrics, theme),
            Diagram::Wardley(w) => scene::wardley(w, metrics, theme),
            Diagram::TreeView(t) => scene::treeview(t, metrics, theme),
            Diagram::Railroad(r) => scene::railroad(r, metrics, theme),
            Diagram::Cynefin(c) => scene::cynefin(c, metrics, theme),
            Diagram::EventModel(c) => scene::eventmodel(c, metrics, theme),
        }
    }

    /// The drawing as text (ASCIIR-1).
    pub fn ascii(&self, options: &Options) -> String {
        match self {
            Diagram::Graph(f) => ascii::render(f, options),
            Diagram::Sequence(s) => ascii::sequence(s, options),
            Diagram::Git(g) => ascii::git_graph(g, options),
            Diagram::Timeline(t) => ascii::timeline(t, options),
            Diagram::Pie(p) => ascii::pie(p, options),
            Diagram::Gantt(g) => ascii::gantt(g, options),
            Diagram::Mindmap(m) => ascii::mindmap(m, options),
            Diagram::Journey(j) => ascii::journey(j, options),
            Diagram::Quadrant(q) => ascii::quadrant(q, options),
            Diagram::XyChart(c) => ascii::xychart(c, options),
            Diagram::Kanban(k) => ascii::kanban(k, options),
            Diagram::Packet(p) => ascii::packet(p, options),
            Diagram::Radar(r) => ascii::radar(r, options),
            Diagram::Sankey(s) => ascii::sankey(s, options),
            Diagram::Treemap(t) => ascii::treemap(t, options),
            Diagram::Block(b) => ascii::block(b, options),
            Diagram::Architecture(a) => ascii::architecture(a, options),
            Diagram::C4(c) => ascii::c4(c, options),
            Diagram::Info(i) => ascii::info(i, options),
            Diagram::Ishikawa(i) => ascii::ishikawa(i, options),
            Diagram::Venn(v) => ascii::venn(v, options),
            Diagram::Wardley(w) => ascii::wardley(w, options),
            Diagram::TreeView(t) => ascii::treeview(t, options),
            Diagram::Railroad(r) => ascii::railroad(r, options),
            Diagram::Cynefin(c) => ascii::cynefin(c, options),
            Diagram::EventModel(c) => ascii::eventmodel(c, options),
        }
    }
}

/// A diagram and the problems found reading it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lowered {
    pub diagram: Diagram,
    pub diagnostics: Vec<Diagnostic>,
}

impl From<flowchart::Lowered> for Lowered {
    fn from(l: flowchart::Lowered) -> Self {
        Lowered {
            diagram: Diagram::Graph(l.flowchart),
            diagnostics: l.diagnostics,
        }
    }
}

/// The first word of `src` after frontmatter, comments, and directives.
pub(crate) fn header(src: &str) -> &str {
    let mut r = src;
    let first = scan::line_len(r);
    if r[..first].trim_end() == "---" {
        let mut at = first + scan::newline_len(&r[first..]);
        while at < r.len() {
            let len = scan::line_len(&r[at..]);
            let next = at + len + scan::newline_len(&r[at + len..]);
            if r[at..at + len].trim() == "---" {
                r = &r[next..];
                break;
            }
            at = next;
        }
    }
    loop {
        r = r.trim_start();
        if let Some(body) = r.strip_prefix("%%{") {
            r = body.find("}%%").map_or("", |i| &body[i + 3..]);
        } else if r.starts_with("%%") {
            r = &r[scan::line_len(r)..];
        } else {
            break;
        }
    }
    let end = r
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(r.len());
    &r[..end]
}

/// Parses and lowers `src` with the parser its header picks (LANGS-2).
pub fn lower(src: &str) -> Lowered {
    match Kind::of(src) {
        Kind::Flowchart | Kind::Swimlane => flowchart::parse(src).lower().into(),
        Kind::Sequence => {
            let (seq, diagnostics) = sequence::parse(src).lower();
            Lowered {
                diagram: Diagram::Sequence(seq),
                diagnostics,
            }
        }
        Kind::State => state::parse(src).lower().into(),
        Kind::Class => class::parse(src).lower().into(),
        Kind::Er => er::parse(src).lower().into(),
        Kind::Requirement => requirement::parse(src).lower().into(),
        Kind::UseCase => usecase::parse(src).lower().into(),
        Kind::Git => {
            let (graph, diagnostics) = git::parse(src).lower();
            Lowered {
                diagram: Diagram::Git(graph),
                diagnostics,
            }
        }
        Kind::Timeline => {
            let (timeline, diagnostics) = timeline::parse(src).lower();
            Lowered {
                diagram: Diagram::Timeline(timeline),
                diagnostics,
            }
        }
        Kind::Pie => {
            let (pie, diagnostics) = pie::parse(src).lower();
            Lowered {
                diagram: Diagram::Pie(pie),
                diagnostics,
            }
        }
        Kind::Gantt => {
            let (gantt, diagnostics) = gantt::parse(src).lower();
            Lowered {
                diagram: Diagram::Gantt(gantt),
                diagnostics,
            }
        }
        Kind::Mindmap => {
            let (mindmap, diagnostics) = mindmap::parse(src).lower();
            Lowered {
                diagram: Diagram::Mindmap(mindmap),
                diagnostics,
            }
        }
        Kind::Journey => {
            let (journey, diagnostics) = journey::parse(src).lower();
            Lowered {
                diagram: Diagram::Journey(journey),
                diagnostics,
            }
        }
        Kind::Quadrant => {
            let (chart, diagnostics) = quadrant::parse(src).lower();
            Lowered {
                diagram: Diagram::Quadrant(chart),
                diagnostics,
            }
        }
        Kind::XyChart => {
            let (chart, diagnostics) = xychart::parse(src).lower();
            Lowered {
                diagram: Diagram::XyChart(chart),
                diagnostics,
            }
        }
        Kind::Kanban => {
            let (board, diagnostics) = kanban::parse(src).lower();
            Lowered {
                diagram: Diagram::Kanban(board),
                diagnostics,
            }
        }
        Kind::Packet => {
            let (packet, diagnostics) = packet::parse(src).lower();
            Lowered {
                diagram: Diagram::Packet(packet),
                diagnostics,
            }
        }
        Kind::Radar => {
            let (chart, diagnostics) = radar::parse(src).lower();
            Lowered {
                diagram: Diagram::Radar(chart),
                diagnostics,
            }
        }
        Kind::Sankey => {
            let (chart, diagnostics) = sankey::parse(src).lower();
            Lowered {
                diagram: Diagram::Sankey(chart),
                diagnostics,
            }
        }
        Kind::Treemap => {
            let (treemap, diagnostics) = treemap::parse(src).lower();
            Lowered {
                diagram: Diagram::Treemap(treemap),
                diagnostics,
            }
        }
        Kind::Block => {
            let (block, diagnostics) = block::parse(src).lower();
            Lowered {
                diagram: Diagram::Block(block),
                diagnostics,
            }
        }
        Kind::Architecture => {
            let (arch, diagnostics) = architecture::parse(src).lower();
            Lowered {
                diagram: Diagram::Architecture(arch),
                diagnostics,
            }
        }
        Kind::C4 => {
            let (c4, diagnostics) = c4::parse(src).lower();
            Lowered {
                diagram: Diagram::C4(c4),
                diagnostics,
            }
        }
        Kind::ZenUml => {
            let (seq, diagnostics) = zenuml::parse(src).lower();
            Lowered {
                diagram: Diagram::Sequence(seq),
                diagnostics,
            }
        }
        Kind::Info => {
            let (info, diagnostics) = info::parse(src).lower();
            Lowered {
                diagram: Diagram::Info(info),
                diagnostics,
            }
        }
        Kind::Ishikawa => {
            let (chart, diagnostics) = ishikawa::parse(src).lower();
            Lowered {
                diagram: Diagram::Ishikawa(chart),
                diagnostics,
            }
        }
        Kind::Venn => {
            let (chart, diagnostics) = venn::parse(src).lower();
            Lowered {
                diagram: Diagram::Venn(chart),
                diagnostics,
            }
        }
        Kind::Wardley => {
            let (map, diagnostics) = wardley::parse(src).lower();
            Lowered {
                diagram: Diagram::Wardley(map),
                diagnostics,
            }
        }
        Kind::TreeView => {
            let (view, diagnostics) = treeview::parse(src).lower();
            Lowered {
                diagram: Diagram::TreeView(view),
                diagnostics,
            }
        }
        Kind::Railroad => {
            let (chart, diagnostics) = railroad::parse(src).lower();
            Lowered {
                diagram: Diagram::Railroad(chart),
                diagnostics,
            }
        }
        Kind::Cynefin => {
            let (chart, diagnostics) = cynefin::parse(src).lower();
            Lowered {
                diagram: Diagram::Cynefin(chart),
                diagnostics,
            }
        }
        Kind::EventModel => {
            let (chart, diagnostics) = eventmodel::parse(src).lower();
            Lowered {
                diagram: Diagram::EventModel(chart),
                diagnostics,
            }
        }
    }
}

/// The keyword of a Mermaid diagram type sirene does not draw, e.g. `pie` (FLOWCH-16).
pub fn other_diagram(src: &str) -> Option<String> {
    match Kind::of(src) {
        Kind::Flowchart => flowchart::parse(src).other_diagram(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(Kind::of("stateDiagram-v2\n  A --> B\n"), Kind::State);
        assert_eq!(Kind::of("stateDiagram\n"), Kind::State);
        assert_eq!(Kind::of("sequenceDiagram\n"), Kind::Sequence);
        assert_eq!(Kind::of("classDiagram\n"), Kind::Class);
        assert_eq!(Kind::of("classDiagram-v2\n"), Kind::Class);
        assert_eq!(Kind::of("erDiagram\n"), Kind::Er);
        assert_eq!(Kind::of("gitGraph\n"), Kind::Git);
        assert_eq!(Kind::of("gitGraph:\n"), Kind::Git);
        assert_eq!(Kind::of("gitGraph TB:\n"), Kind::Git);
        assert_eq!(Kind::of("timeline TD\n"), Kind::Timeline);
        assert_eq!(Kind::of("pie showData\n"), Kind::Pie);
        assert_eq!(Kind::of("gantt\n"), Kind::Gantt);
        assert_eq!(Kind::of("mindmap\n"), Kind::Mindmap);
        assert_eq!(Kind::of("journey\n"), Kind::Journey);
        assert_eq!(Kind::of("quadrantChart\n"), Kind::Quadrant);
        assert_eq!(Kind::of("xychart-beta\n"), Kind::XyChart);
        assert_eq!(Kind::of("xychart horizontal\n"), Kind::XyChart);
        assert_eq!(Kind::of("requirementDiagram\n"), Kind::Requirement);
        assert_eq!(Kind::of("kanban\n"), Kind::Kanban);
        assert_eq!(Kind::of("packet\n"), Kind::Packet);
        assert_eq!(Kind::of("packet-beta\n"), Kind::Packet);
        assert_eq!(Kind::of("radar-beta\n"), Kind::Radar);
        assert_eq!(Kind::of("sankey\n"), Kind::Sankey);
        assert_eq!(Kind::of("sankey-beta\n"), Kind::Sankey);
        assert_eq!(Kind::of("treemap-beta\n"), Kind::Treemap);
        assert_eq!(Kind::of("treemap\n"), Kind::Treemap);
        assert_eq!(Kind::of("block-beta\n"), Kind::Block);
        assert_eq!(Kind::of("block\n"), Kind::Block);
        assert_eq!(Kind::of("architecture-beta\n"), Kind::Architecture);
        assert_eq!(Kind::of("architecture\n"), Kind::Architecture);
        assert_eq!(Kind::of("C4Context\n"), Kind::C4);
        assert_eq!(Kind::of("C4Deployment\n"), Kind::C4);
        assert_eq!(Kind::of("zenuml\n"), Kind::ZenUml);
        assert_eq!(Kind::of("info showInfo\n"), Kind::Info);
        assert_eq!(Kind::of("ishikawa-beta\n"), Kind::Ishikawa);
        assert_eq!(Kind::of("ishikawa\n"), Kind::Ishikawa);
        assert_eq!(Kind::of("venn-beta\n"), Kind::Venn);
        assert_eq!(Kind::of("wardley-beta\n"), Kind::Wardley);
        assert_eq!(Kind::of("treeView-beta\n"), Kind::TreeView);
        assert_eq!(Kind::of("usecase-beta\n"), Kind::UseCase);
        assert_eq!(Kind::of("railroad-beta\n"), Kind::Railroad);
        assert_eq!(Kind::of("railroad-ebnf-beta\n"), Kind::Railroad);
        assert_eq!(Kind::of("railroad-abnf-beta\n"), Kind::Railroad);
        assert_eq!(Kind::of("railroad-peg-beta\n"), Kind::Railroad);
        assert_eq!(Kind::of("swimlane-beta LR\n"), Kind::Swimlane);
        assert_eq!(Kind::of("cynefin-beta\n"), Kind::Cynefin);
        assert_eq!(Kind::of("cynefin-beta:\n"), Kind::Cynefin);
        assert_eq!(Kind::of("eventmodeling\n"), Kind::EventModel);
        assert_eq!(
            Kind::of("---\ntitle: x\n---\n%% c\n%%{init: {}}%%\nstateDiagram-v2\n"),
            Kind::State
        );
        assert_eq!(Kind::of("flowchart LR\n"), Kind::Flowchart);
        assert_eq!(Kind::of("stateDiagramX\n"), Kind::Flowchart);
        assert_eq!(Kind::of(""), Kind::Flowchart);
        assert_eq!(Kind::of("---\nunclosed\n"), Kind::Flowchart);
    }

    #[test]
    fn other_diagrams() {
        assert_eq!(other_diagram("zenuml\n"), None);
        assert_eq!(other_diagram("info\n"), None);
        assert_eq!(other_diagram("venn-beta\n"), None);
        assert_eq!(other_diagram("wardley-beta\n"), None);
        assert_eq!(other_diagram("treeView-beta\n"), None);
        assert_eq!(other_diagram("usecase-beta\n"), None);
        assert_eq!(other_diagram("railroad-ebnf-beta\n"), None);
        assert_eq!(other_diagram("swimlane-beta\n"), None);
        assert_eq!(other_diagram("cynefin-beta\n"), None);
        assert_eq!(other_diagram("eventmodeling\n"), None);
        assert_eq!(
            other_diagram("agentflow-beta\n"),
            Some("agentflow-beta".to_owned())
        );
        assert_eq!(other_diagram("xychart-beta\n"), None);
        assert_eq!(other_diagram("requirementDiagram\n"), None);
        assert_eq!(other_diagram("kanban\n"), None);
        assert_eq!(other_diagram("packet-beta\n"), None);
        assert_eq!(other_diagram("radar-beta\n"), None);
        assert_eq!(other_diagram("sankey-beta\n"), None);
        assert_eq!(other_diagram("treemap-beta\n"), None);
        assert_eq!(other_diagram("block-beta\n"), None);
        assert_eq!(other_diagram("architecture-beta\n"), None);
        assert_eq!(other_diagram("C4Container\n"), None);
        assert_eq!(other_diagram("quadrantChart\n"), None);
        assert_eq!(other_diagram("journey\n"), None);
        assert_eq!(other_diagram("mindmap\n"), None);
        assert_eq!(other_diagram("gantt\n"), None);
        assert_eq!(other_diagram("pie\n"), None);
        assert_eq!(other_diagram("stateDiagram-v2\n"), None);
        assert_eq!(other_diagram("gitGraph\n"), None);
        assert_eq!(other_diagram("sequenceDiagram\n"), None);
        assert_eq!(other_diagram("flowchart\n"), None);
    }
}
