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
use crate::theme::ThemeConfig;
use crate::timeline::{self, Timeline};
use crate::treemap::{self, Treemap};
use crate::treeview::{self, TreeView};
use crate::venn::{self, Venn};
use crate::wardley::{self, Wardley};
use crate::xychart::{self, XyChart};
use crate::{class, dot, er, requirement, state, usecase, zenuml};

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
    /// A flowchart of `flow` containers with agentflow's shapes and edges (`agentflow-beta`).
    Agentflow,
    /// A Graphviz DOT graph (DOT-1), detected by its `graph` or `digraph` header (LANGS-1).
    Dot,
}

/// A diagram language (LANGS-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Mermaid,
    /// Graphviz DOT (DOT-1).
    Dot,
}

impl Lang {
    /// The language a diagram file's extension names: `.mmd` or `.mermaid` for Mermaid, `.dot`
    /// or `.gv` for DOT, in any case (DOT-5).
    pub fn of_path(path: &std::path::Path) -> Option<Lang> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "mmd" | "mermaid" => Some(Lang::Mermaid),
            "dot" | "gv" => Some(Lang::Dot),
            _ => None,
        }
    }

    /// The language a Markdown code block's info word names: `mermaid`, or `dot` or `graphviz`
    /// (MARKDO-1).
    pub fn of_info(word: &str) -> Option<Lang> {
        match word {
            "mermaid" => Some(Lang::Mermaid),
            "dot" | "graphviz" => Some(Lang::Dot),
            _ => None,
        }
    }
}

impl Kind {
    /// The type of the diagram `src` starts with: DOT when it starts with a DOT graph header,
    /// otherwise the Mermaid type its header names, and a flowchart when it names none.
    pub fn of(src: &str) -> Kind {
        Kind::of_lang(src, None)
    }

    /// The type of the diagram `src` starts with, read as `lang` when given (DOT-5): DOT
    /// whatever the source looks like, or a Mermaid type even when it starts like a DOT graph.
    pub fn of_lang(src: &str, lang: Option<Lang>) -> Kind {
        match lang {
            Some(Lang::Dot) => Kind::Dot,
            Some(Lang::Mermaid) => Kind::mermaid(src),
            None if dot::is_dot(src) => Kind::Dot,
            None => Kind::mermaid(src),
        }
    }

    /// The Mermaid type `src`'s header names, and a flowchart when it names none.
    fn mermaid(src: &str) -> Kind {
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
        } else if word == flowchart::AGENTFLOW {
            Kind::Agentflow
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
     mindmap, journey, quadrant, XY, requirement, kanban, packet, radar, sankey, treemap, block, architecture, C4, ZenUML, info, ishikawa, venn, Wardley, tree view, use case, railroad, swimlane, Cynefin, event modeling, and agentflow diagrams";

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
    /// The diagram's own `theme` and `themeVariables` (MTHEME-1).
    pub theme: ThemeConfig,
}

impl From<flowchart::Lowered> for Lowered {
    fn from(l: flowchart::Lowered) -> Self {
        Lowered {
            diagram: Diagram::Graph(l.flowchart),
            diagnostics: l.diagnostics,
            ..Default::default()
        }
    }
}

/// The first word of `src` after frontmatter, comments, and directives.
pub(crate) fn header(src: &str) -> &str {
    let mut r = &src[scan::bom_len(src)..];
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

/// Parses and lowers `src` with the parser its header picks (LANGS-2), and reads its theme
/// settings (MTHEME-1).
pub fn lower(src: &str) -> Lowered {
    lower_as(src, None)
}

/// [`lower`], reading `src` as `lang` when given instead of detecting it (DOT-5).
pub fn lower_as(src: &str, lang: Option<Lang>) -> Lowered {
    let kind = Kind::of_lang(src, lang);
    if kind == Kind::Dot {
        return lower_dot(src);
    }
    let mut lowered = lower_diagram(src, kind);
    lowered.theme = ThemeConfig::read(src);
    lowered.diagnostics.extend(lowered.theme.diagnostics());
    lowered
}

/// A DOT graph (DOT-2), its `bgcolor` as the theme's background.
fn lower_dot(src: &str) -> Lowered {
    let l = dot::parse(src).lower();
    let mut theme = ThemeConfig::default();
    if let Some(c) = l.background {
        let hex = format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a);
        theme.variables.push(("background".into(), hex));
    }
    Lowered {
        diagram: Diagram::Graph(l.flowchart),
        diagnostics: l.diagnostics,
        theme,
    }
}

fn lower_diagram(src: &str, kind: Kind) -> Lowered {
    match kind {
        Kind::Dot => lower_dot(src),
        Kind::Flowchart | Kind::Swimlane | Kind::Agentflow => flowchart::parse(src).lower().into(),
        Kind::Sequence => {
            let (seq, diagnostics) = sequence::parse(src).lower();
            Lowered {
                diagram: Diagram::Sequence(seq),
                diagnostics,
                ..Default::default()
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
                ..Default::default()
            }
        }
        Kind::Timeline => {
            let (timeline, diagnostics) = timeline::parse(src).lower();
            Lowered {
                diagram: Diagram::Timeline(timeline),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Pie => {
            let (pie, diagnostics) = pie::parse(src).lower();
            Lowered {
                diagram: Diagram::Pie(pie),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Gantt => {
            let (gantt, diagnostics) = gantt::parse(src).lower();
            Lowered {
                diagram: Diagram::Gantt(gantt),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Mindmap => {
            let (mindmap, diagnostics) = mindmap::parse(src).lower();
            Lowered {
                diagram: Diagram::Mindmap(mindmap),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Journey => {
            let (journey, diagnostics) = journey::parse(src).lower();
            Lowered {
                diagram: Diagram::Journey(journey),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Quadrant => {
            let (chart, diagnostics) = quadrant::parse(src).lower();
            Lowered {
                diagram: Diagram::Quadrant(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::XyChart => {
            let (chart, diagnostics) = xychart::parse(src).lower();
            Lowered {
                diagram: Diagram::XyChart(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Kanban => {
            let (board, diagnostics) = kanban::parse(src).lower();
            Lowered {
                diagram: Diagram::Kanban(board),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Packet => {
            let (packet, diagnostics) = packet::parse(src).lower();
            Lowered {
                diagram: Diagram::Packet(packet),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Radar => {
            let (chart, diagnostics) = radar::parse(src).lower();
            Lowered {
                diagram: Diagram::Radar(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Sankey => {
            let (chart, diagnostics) = sankey::parse(src).lower();
            Lowered {
                diagram: Diagram::Sankey(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Treemap => {
            let (treemap, diagnostics) = treemap::parse(src).lower();
            Lowered {
                diagram: Diagram::Treemap(treemap),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Block => {
            let (block, diagnostics) = block::parse(src).lower();
            Lowered {
                diagram: Diagram::Block(block),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Architecture => {
            let (arch, diagnostics) = architecture::parse(src).lower();
            Lowered {
                diagram: Diagram::Architecture(arch),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::C4 => {
            let (c4, diagnostics) = c4::parse(src).lower();
            Lowered {
                diagram: Diagram::C4(c4),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::ZenUml => {
            let (seq, diagnostics) = zenuml::parse(src).lower();
            Lowered {
                diagram: Diagram::Sequence(seq),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Info => {
            let (info, diagnostics) = info::parse(src).lower();
            Lowered {
                diagram: Diagram::Info(info),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Ishikawa => {
            let (chart, diagnostics) = ishikawa::parse(src).lower();
            Lowered {
                diagram: Diagram::Ishikawa(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Venn => {
            let (chart, diagnostics) = venn::parse(src).lower();
            Lowered {
                diagram: Diagram::Venn(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Wardley => {
            let (map, diagnostics) = wardley::parse(src).lower();
            Lowered {
                diagram: Diagram::Wardley(map),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::TreeView => {
            let (view, diagnostics) = treeview::parse(src).lower();
            Lowered {
                diagram: Diagram::TreeView(view),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Railroad => {
            let (chart, diagnostics) = railroad::parse(src).lower();
            Lowered {
                diagram: Diagram::Railroad(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::Cynefin => {
            let (chart, diagnostics) = cynefin::parse(src).lower();
            Lowered {
                diagram: Diagram::Cynefin(chart),
                diagnostics,
                ..Default::default()
            }
        }
        Kind::EventModel => {
            let (chart, diagnostics) = eventmodel::parse(src).lower();
            Lowered {
                diagram: Diagram::EventModel(chart),
                diagnostics,
                ..Default::default()
            }
        }
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
        assert_eq!(Kind::of("agentflow-beta LR\n"), Kind::Agentflow);
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
    fn forced_languages() {
        use std::path::Path;
        assert_eq!(Lang::of_path(Path::new("a/b.gv")), Some(Lang::Dot));
        assert_eq!(Lang::of_path(Path::new("b.DOT")), Some(Lang::Dot));
        assert_eq!(Lang::of_path(Path::new("b.Mermaid")), Some(Lang::Mermaid));
        assert_eq!(Lang::of_path(Path::new("b.mmd")), Some(Lang::Mermaid));
        assert_eq!(Lang::of_path(Path::new("b.txt")), None);
        assert_eq!(Lang::of_path(Path::new("-")), None);
        assert_eq!(Lang::of_info("graphviz"), Some(Lang::Dot));
        assert_eq!(Lang::of_info("mermaid"), Some(Lang::Mermaid));
        assert_eq!(Lang::of_info("dotnet"), None);

        let dot = "digraph { a -> b }\n";
        assert_eq!(Kind::of_lang(dot, None), Kind::Dot);
        assert_eq!(Kind::of_lang(dot, Some(Lang::Mermaid)), Kind::Flowchart);
        assert_eq!(Kind::of_lang("pie\n", Some(Lang::Dot)), Kind::Dot);
        assert_eq!(Kind::of_lang("pie\n", Some(Lang::Mermaid)), Kind::Pie);

        // A forced DOT source without a graph header is an error, not a Mermaid flowchart.
        let l = lower_as("a -> b\n", Some(Lang::Dot));
        assert!(
            l.diagnostics
                .iter()
                .any(|d| d.message.contains("`digraph`")),
            "{:?}",
            l.diagnostics
        );
        assert!(
            lower_as("flowchart LR\n  a --> b\n", Some(Lang::Mermaid))
                .diagnostics
                .is_empty()
        );
    }

    /// Every Mermaid 12.3 type has a parser: none is refused by its header.
    #[test]
    fn every_type_is_drawn() {
        for header in [
            "zenuml",
            "info",
            "venn-beta",
            "wardley-beta",
            "treeView-beta",
            "usecase-beta",
            "railroad-ebnf-beta",
            "swimlane-beta",
            "cynefin-beta",
            "eventmodeling",
            "agentflow-beta",
            "xychart-beta",
            "requirementDiagram",
            "kanban",
            "packet-beta",
            "radar-beta",
            "sankey-beta",
            "treemap-beta",
            "block-beta",
            "architecture-beta",
            "C4Container",
            "quadrantChart",
            "journey",
            "mindmap",
            "gantt",
            "pie",
            "stateDiagram-v2",
            "gitGraph",
            "sequenceDiagram",
            "flowchart",
        ] {
            let lowered = lower(&format!("{header}\n"));
            assert!(
                lowered
                    .diagnostics
                    .iter()
                    .all(|d| !d.message.contains("not supported")),
                "{header}"
            );
        }
    }
}
