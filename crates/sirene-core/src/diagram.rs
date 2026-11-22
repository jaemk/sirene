//! The entry point for every Mermaid diagram type sirene draws (LANGS-2).

use std::borrow::Cow;

use crate::architecture::{self, Architecture};
use crate::ascii::{self, Options};
use crate::block::{self, Block};
use crate::c4::{self, C4};
use crate::cynefin::{self, Cynefin};
use crate::diagnostic::{Diagnostic, Severity};
use crate::eventmodel::{self, EventModel};
use crate::flowchart::{self, Flowchart};
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
use crate::timing::Timing;
use crate::treemap::{self, Treemap};
use crate::treeview::{self, TreeView};
use crate::venn::{self, Venn};
use crate::wardley::{self, Wardley};
use crate::xychart::{self, XyChart};
use crate::{class, dot, er, plantuml, requirement, state, usecase, zenuml};

/// A Mermaid diagram type sirene draws, picked by the header keyword (LANGS-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
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
    /// A PlantUML sequence diagram (PSEQ-1), detected by its `@start` line (LANGS-1).
    PlantUml,
}

/// A diagram language (LANGS-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Mermaid,
    /// Graphviz DOT (DOT-1).
    Dot,
    /// PlantUML (PSEQ-7).
    PlantUml,
}

impl Lang {
    /// The language a diagram file's extension names: `.mmd` or `.mermaid` for Mermaid, `.dot`
    /// or `.gv` for DOT, and `.puml`, `.plantuml`, `.pu`, or `.iuml` for PlantUML, in any case
    /// (DOT-5, PSEQ-7).
    pub fn of_path(path: &std::path::Path) -> Option<Lang> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "mmd" | "mermaid" => Some(Lang::Mermaid),
            "dot" | "gv" => Some(Lang::Dot),
            "puml" | "plantuml" | "pu" | "iuml" => Some(Lang::PlantUml),
            _ => None,
        }
    }

    /// The language a Markdown code block's info word names: `mermaid`, `dot` or `graphviz`,
    /// or `plantuml` or `puml` (MARKDO-1).
    pub fn of_info(word: &str) -> Option<Lang> {
        match word {
            "mermaid" => Some(Lang::Mermaid),
            "dot" | "graphviz" => Some(Lang::Dot),
            "plantuml" | "puml" => Some(Lang::PlantUml),
            _ => None,
        }
    }
}

impl Kind {
    /// Every type, in declaration order.
    pub const ALL: &[Kind] = &[
        Kind::Flowchart,
        Kind::Sequence,
        Kind::State,
        Kind::Class,
        Kind::Er,
        Kind::Git,
        Kind::Timeline,
        Kind::Pie,
        Kind::Gantt,
        Kind::Mindmap,
        Kind::Journey,
        Kind::Quadrant,
        Kind::XyChart,
        Kind::Requirement,
        Kind::Kanban,
        Kind::Packet,
        Kind::Radar,
        Kind::Sankey,
        Kind::Treemap,
        Kind::Block,
        Kind::Architecture,
        Kind::C4,
        Kind::ZenUml,
        Kind::Info,
        Kind::Ishikawa,
        Kind::Venn,
        Kind::Wardley,
        Kind::TreeView,
        Kind::UseCase,
        Kind::Railroad,
        Kind::Swimlane,
        Kind::Cynefin,
        Kind::EventModel,
        Kind::Agentflow,
        Kind::Dot,
        Kind::PlantUml,
    ];

    /// The language this type is written in.
    pub fn lang(self) -> Lang {
        match self {
            Kind::Dot => Lang::Dot,
            Kind::PlantUml => Lang::PlantUml,
            _ => Lang::Mermaid,
        }
    }

    /// The type's name in messages, e.g. `sequence diagram` or `DOT graph`.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Flowchart => "flowchart",
            Kind::Sequence => "sequence diagram",
            Kind::State => "state diagram",
            Kind::Class => "class diagram",
            Kind::Er => "ER diagram",
            Kind::Git => "git graph",
            Kind::Timeline => "timeline",
            Kind::Pie => "pie chart",
            Kind::Gantt => "gantt chart",
            Kind::Mindmap => "mindmap",
            Kind::Journey => "user journey",
            Kind::Quadrant => "quadrant chart",
            Kind::XyChart => "XY chart",
            Kind::Requirement => "requirement diagram",
            Kind::Kanban => "kanban board",
            Kind::Packet => "packet diagram",
            Kind::Radar => "radar chart",
            Kind::Sankey => "sankey diagram",
            Kind::Treemap => "treemap",
            Kind::Block => "block diagram",
            Kind::Architecture => "architecture diagram",
            Kind::C4 => "C4 diagram",
            Kind::ZenUml => "ZenUML diagram",
            Kind::Info => "info diagram",
            Kind::Ishikawa => "ishikawa diagram",
            Kind::Venn => "venn diagram",
            Kind::Wardley => "Wardley map",
            Kind::TreeView => "tree view",
            Kind::UseCase => "use case diagram",
            Kind::Railroad => "railroad diagram",
            Kind::Swimlane => "swimlane diagram",
            Kind::Cynefin => "Cynefin diagram",
            Kind::EventModel => "event modeling diagram",
            Kind::Agentflow => "agentflow diagram",
            Kind::Dot => "DOT graph",
            Kind::PlantUml => "PlantUML diagram",
        }
    }

    /// The type of the diagram `src` starts with: DOT when it starts with a DOT graph header,
    /// otherwise the Mermaid type its header names, and a flowchart when it names none.
    pub fn of(src: &str) -> Kind {
        Kind::of_lang(src, None)
    }

    /// The type of the diagram `src` starts with, read as `lang` when given (DOT-5): DOT
    /// whatever the source looks like, or a Mermaid type even when it starts like a DOT graph.
    pub fn of_lang(src: &str, lang: Option<Lang>) -> Kind {
        prepare(src, lang).0
    }

    fn mermaid_word(word: &str) -> Kind {
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
    Timing(Timing),
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
    pub fn scene(&self, metrics: &impl TextMetrics, theme: &Theme) -> crate::Result<Scene> {
        let _phase = crate::profile::Phase::start("scene");
        Ok(match self {
            Diagram::Graph(f) => return scene::flowchart(f, metrics, theme),
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
            Diagram::Block(b) => return scene::block(b, metrics, theme),
            Diagram::Architecture(a) => return scene::architecture(a, metrics, theme),
            Diagram::C4(c) => return scene::c4(c, metrics, theme),
            Diagram::Info(i) => scene::info(i, metrics, theme),
            Diagram::Ishikawa(i) => scene::ishikawa(i, metrics, theme),
            Diagram::Venn(v) => scene::venn(v, metrics, theme),
            Diagram::Wardley(w) => scene::wardley(w, metrics, theme),
            Diagram::TreeView(t) => scene::treeview(t, metrics, theme),
            Diagram::Railroad(r) => scene::railroad(r, metrics, theme),
            Diagram::Cynefin(c) => scene::cynefin(c, metrics, theme),
            Diagram::EventModel(c) => scene::eventmodel(c, metrics, theme),
            Diagram::Timing(t) => scene::timing(t, metrics, theme),
        })
    }

    /// The drawing as text (ASCIIR-1).
    pub fn ascii(&self, options: &Options) -> crate::Result<String> {
        let _phase = crate::profile::Phase::start("ascii");
        Ok(match self {
            Diagram::Graph(f) => return ascii::render(f, options),
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
            Diagram::Timing(t) => ascii::timing(t, options),
        })
    }
}

/// A diagram and the problems found reading it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lowered {
    pub diagram: Diagram,
    pub diagnostics: Vec<Diagnostic>,
    /// The diagram's own `theme` and `themeVariables` (MTHEME-1).
    pub theme: ThemeConfig,
    /// The type the source was read as (LANGS-1); [`Kind::lang`] is its language.
    pub kind: Kind,
}

impl Lowered {
    /// The drawing in `theme` with the diagram's own theme settings applied over it, as
    /// `sirene svg` and `sirene png` draw it.
    pub fn scene(&self, metrics: &impl TextMetrics, theme: &Theme) -> crate::Result<Scene> {
        self.diagram.scene(metrics, &self.theme.apply(theme))
    }

    /// The drawing as text, as `sirene ascii` prints it.
    pub fn ascii(&self, options: &Options) -> crate::Result<String> {
        self.diagram.ascii(options)
    }

    /// A standalone SVG document, with text measured by [`scene::Approx`], as `sirene svg`
    /// writes it.
    pub fn svg(&self, theme: &Theme) -> crate::Result<String> {
        Ok(scene::svg::write(&self.scene(&scene::Approx, theme)?))
    }

    /// A PNG drawn with the embedded font, as `sirene png` writes it. With no `scale`, the
    /// quality's own scale is used, shrunk so the image fits in the largest side
    /// [`scene::png::write`] draws; a given `scale` is kept.
    #[cfg(feature = "png")]
    pub fn png(
        &self,
        theme: &Theme,
        quality: scene::png::Quality,
        scale: Option<f32>,
    ) -> crate::Result<Vec<u8>> {
        let scene = self.scene(&scene::png::FontMetrics, theme)?;
        let scale = scale.unwrap_or_else(|| scene::png::fit(&scene, quality.scale()));
        scene::png::write(&scene, scene::png::Options { scale, quality })
    }
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
    let r = &src[crate::lines::preamble_end(src)..];
    let end = r
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(r.len());
    &r[..end]
}

/// Pick the language and prepare its source without changing byte offsets. DOT treats
/// lone CR as whitespace. The PlantUML parser handles its own line endings.
fn prepare(src: &str, lang: Option<Lang>) -> (Kind, Cow<'_, str>) {
    match lang {
        Some(Lang::Dot) => return (Kind::Dot, Cow::Borrowed(src)),
        Some(Lang::PlantUml) => return (Kind::PlantUml, Cow::Borrowed(src)),
        None if dot::is_dot(src) => return (Kind::Dot, Cow::Borrowed(src)),
        _ => {}
    }
    let prepared = crate::lines::lone_cr_as_lf(src);
    let word = header(&prepared);
    let mermaid = Kind::mermaid_word(word);
    let kind = if lang.is_none()
        && mermaid == Kind::Flowchart
        && !matches!(word, "flowchart" | "flowchart-elk" | "graph")
        && plantuml::is_plantuml(&prepared)
    {
        Kind::PlantUml
    } else {
        mermaid
    };
    (kind, prepared)
}

// The same kind-to-parser mapping drives model lowering and syntax-only theme extraction.
// Graph parsers use their existing Lowered conversion. Others name their Diagram variant.
macro_rules! dispatch {
    ($kind:expr, $mermaid:ident, $dot:expr, $plantuml:expr) => {
        match $kind {
            Kind::Dot => $dot,
            Kind::PlantUml => $plantuml,
            Kind::Flowchart | Kind::Swimlane | Kind::Agentflow => $mermaid!(flowchart),
            Kind::State => $mermaid!(state),
            Kind::Class => $mermaid!(class),
            Kind::Er => $mermaid!(er),
            Kind::Requirement => $mermaid!(requirement),
            Kind::UseCase => $mermaid!(usecase),
            Kind::Sequence => $mermaid!(sequence, Sequence),
            Kind::Git => $mermaid!(git, Git),
            Kind::Timeline => $mermaid!(timeline, Timeline),
            Kind::Pie => $mermaid!(pie, Pie),
            Kind::Gantt => $mermaid!(gantt, Gantt),
            Kind::Mindmap => $mermaid!(mindmap, Mindmap),
            Kind::Journey => $mermaid!(journey, Journey),
            Kind::Quadrant => $mermaid!(quadrant, Quadrant),
            Kind::XyChart => $mermaid!(xychart, XyChart),
            Kind::Kanban => $mermaid!(kanban, Kanban),
            Kind::Packet => $mermaid!(packet, Packet),
            Kind::Radar => $mermaid!(radar, Radar),
            Kind::Sankey => $mermaid!(sankey, Sankey),
            Kind::Treemap => $mermaid!(treemap, Treemap),
            Kind::Block => $mermaid!(block, Block),
            Kind::Architecture => $mermaid!(architecture, Architecture),
            Kind::C4 => $mermaid!(c4, C4),
            Kind::ZenUml => $mermaid!(zenuml, Sequence),
            Kind::Info => $mermaid!(info, Info),
            Kind::Ishikawa => $mermaid!(ishikawa, Ishikawa),
            Kind::Venn => $mermaid!(venn, Venn),
            Kind::Wardley => $mermaid!(wardley, Wardley),
            Kind::TreeView => $mermaid!(treeview, TreeView),
            Kind::Railroad => $mermaid!(railroad, Railroad),
            Kind::Cynefin => $mermaid!(cynefin, Cynefin),
            Kind::EventModel => $mermaid!(eventmodel, EventModel),
        }
    };
}

/// Parses and lowers `src` with the parser its header picks (LANGS-2), and reads its theme
/// settings (MTHEME-1).
pub fn lower(src: &str) -> Lowered {
    lower_as(src, None)
}

/// [`lower`], reading `src` as `lang` when given instead of detecting it (DOT-5).
pub fn lower_as(src: &str, lang: Option<Lang>) -> Lowered {
    lower_as_with_limit(src, lang, u32::MAX as usize)
}

fn lower_as_with_limit(src: &str, lang: Option<Lang>, limit: usize) -> Lowered {
    let _phase = crate::profile::Phase::start("lower");
    if let Some(diagnostic) = crate::lines::source_limit(src, limit) {
        return Lowered {
            diagnostics: vec![diagnostic],
            ..Default::default()
        };
    }
    let (kind, src) = prepare(src, lang);
    let mut lowered = lower_prepared(&src, kind);
    lowered.kind = kind;
    lowered.diagnostics.sort_by_key(|d| d.range.start());
    cap(&mut lowered.diagnostics);
    lowered
}

/// The most diagnostics one diagram reports (LANGS-8).
pub const MAX_DIAGNOSTICS: usize = 500;

/// Keeps the first [`MAX_DIAGNOSTICS`] and replaces the rest with one that counts them, an error
/// when any of them is.
fn cap(diagnostics: &mut Vec<Diagnostic>) {
    if diagnostics.len() <= MAX_DIAGNOSTICS {
        return;
    }
    let rest = diagnostics.split_off(MAX_DIAGNOSTICS);
    let message = format!("{} more problems not shown", rest.len());
    let error = rest.iter().any(|d| d.severity == Severity::Error);
    diagnostics.push(if error {
        Diagnostic::new(rest[0].range, message)
    } else {
        Diagnostic::warning(rest[0].range, message)
    });
}

/// Extract theme settings from the same directive tokens as the selected Mermaid parser.
pub(crate) fn read_theme(src: &str) -> ThemeConfig {
    if src.len() > u32::MAX as usize {
        return ThemeConfig::default();
    }
    let (kind, src) = prepare(src, Some(Lang::Mermaid));
    macro_rules! read {
        ($module:ident $(, $variant:ident)?) => {
            ThemeConfig::from_syntax(&$module::parse(&src).syntax())
        };
    }
    dispatch!(kind, read, ThemeConfig::default(), ThemeConfig::default())
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
        kind: Kind::Dot,
    }
}

fn lower_prepared(src: &str, kind: Kind) -> Lowered {
    macro_rules! into_lowered {
        ($value:expr) => {
            Lowered::from($value)
        };
        ($value:expr, $variant:ident) => {{
            let (model, diagnostics) = $value;
            Lowered {
                diagram: Diagram::$variant(model),
                diagnostics,
                ..Default::default()
            }
        }};
    }

    macro_rules! lower {
        ($module:ident $(, $variant:ident)?) => {{
            let parsed = $module::parse(src);
            let mut lowered = into_lowered!(parsed.lower() $(, $variant)?);
            lowered.theme = ThemeConfig::from_syntax(&parsed.syntax());
            lowered.diagnostics.extend(lowered.theme.diagnostics());
            lowered
        }};
    }

    dispatch!(kind, lower, lower_dot(src), {
        let (diagram, diagnostics) = plantuml::parse(src).lower();
        Lowered {
            diagram,
            diagnostics,
            ..Default::default()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // LANGS-13, MTHEME-6: all public entry points agree on preparation and recovery.
    #[test]
    fn recovered_directives_and_theme_across_line_endings() {
        for nl in ["\n", "\r\n", "\r"] {
            let src = [
                "flowchart TD",
                "%%{broken",
                "A[\"literal %%{ marker\"]",
                "%%{init: {'theme':'dark'}}%%",
                "B",
                "",
            ]
            .join(nl);
            let result = lower(&src);
            assert_eq!(Kind::of(&src), Kind::Flowchart);
            assert_eq!(ThemeConfig::read(&src), result.theme);
            assert_eq!(
                result.theme.name.as_ref().map(|n| n.0.as_str()),
                Some("dark")
            );
            let start = src.find("%%{broken").unwrap();
            assert_eq!(
                result.diagnostics.len(),
                1,
                "{nl:?}: {:?}",
                result.diagnostics
            );
            assert_eq!(
                result.diagnostics[0].message,
                "unclosed directive: expected `}%%`"
            );
            assert_eq!(
                result.diagnostics[0].range,
                rowan::TextRange::new((start as u32).into(), ((start + 3) as u32).into())
            );
            let Diagram::Graph(graph) = result.diagram else {
                panic!("expected graph")
            };
            assert_eq!(
                graph
                    .nodes
                    .iter()
                    .map(|n| n.id.as_str())
                    .collect::<Vec<_>>(),
                ["A", "B"]
            );

            let pie = [
                "%% comment",
                "%%{init: {'theme':'dark'}}%%",
                "pie",
                "\"A\": 1",
                "",
            ]
            .join(nl);
            assert_eq!(Kind::of(&pie), Kind::Pie);
            let lowered = lower(&pie);
            assert_eq!(lowered.kind, Kind::Pie);
            assert_eq!(ThemeConfig::read(&pie), lowered.theme);
            assert_eq!(lowered.theme.name.unwrap().0, "dark");
            assert_eq!(lowered.diagnostics, []);

            let invalid = ["---", "config:", "  theme: dark", "pie", "\"A\": 1", ""].join(nl);
            let lowered = lower(&invalid);
            assert_eq!(lowered.theme, ThemeConfig::default());
            assert_eq!(ThemeConfig::read(&invalid), lowered.theme);
            assert_eq!(
                lowered.diagnostics[0].range,
                rowan::TextRange::new(0.into(), 3.into())
            );
        }
    }

    // LANGS-13: quotes protect literal markers and a malformed quote cannot conceal the next
    // directive, even across many successive unclosed openers.
    #[test]
    fn directive_recovery_and_quoted_markers() {
        let valid =
            "%%{init: {\n 'theme': 'dark',\n 'unused': '%%{ literal'\n}}%%\npie\n\"A\": 1\n";
        let result = lower(valid);
        assert_eq!(result.kind, Kind::Pie);
        assert_eq!(result.theme.name.unwrap().0, "dark");
        assert_eq!(result.diagnostics, []);
        let mut src = "flowchart TD\n".to_owned();
        for _ in 0..10_000 {
            src.push_str("%%{'unclosed\n");
        }
        src.push_str("A\n%%{init: {'theme':'dark'}}%%\n");
        let result = lower(&src);
        assert_eq!(result.diagnostics.len(), MAX_DIAGNOSTICS + 1);
        assert_eq!(result.theme.name.unwrap().0, "dark");
        let Diagram::Graph(graph) = result.diagram else {
            panic!("expected graph")
        };
        assert_eq!(graph.nodes[0].id, "A");
    }

    #[test]
    fn directive_recovery_in_custom_trivia_parsers() {
        let theme = "%%{init: {unused: '}%%', theme: dark}}%%";
        for (header, body, expected) in [
            ("usecase-beta", "actor A", Kind::UseCase),
            ("railroad-ebnf-beta", "r = 'token';", Kind::Railroad),
        ] {
            for nl in ["\n", "\r\n", "\r"] {
                let src = ["%%{broken", header, "%%{unfinished", body, theme, ""].join(nl);
                assert_eq!(Kind::of(&src), expected);
                let result = lower(&src);
                assert_eq!(ThemeConfig::read(&src), result.theme);
                assert_eq!(result.theme.name.unwrap().0, "dark");
                assert_eq!(
                    result.diagnostics.len(),
                    2,
                    "{src:?}: {:?}",
                    result.diagnostics
                );
                assert!(
                    result
                        .diagnostics
                        .iter()
                        .all(|d| d.message.starts_with("unclosed directive"))
                );
                match result.diagram {
                    Diagram::Graph(graph) => assert_eq!(graph.nodes[0].id, "A"),
                    Diagram::Railroad(chart) => assert_eq!(chart.rules[0].name, "r"),
                    _ => panic!("expected a use case or railroad diagram"),
                }
            }
        }
    }

    #[test]
    fn unclosed_directive_quote_does_not_consume_later_lines() {
        for newline in ["\n", "\r\n", "\r"] {
            let src = [
                "%%{init: {theme: 'broken",
                "pie",
                "\"Kept\": 1",
                "%% Unrelated marker }%%",
            ]
            .join(newline);
            assert_eq!(Kind::of(&src), Kind::Pie);
            let lowered = lower(&src);
            assert_eq!(lowered.diagnostics.len(), 1);
            assert!(
                lowered.diagnostics[0]
                    .message
                    .starts_with("unclosed directive")
            );
            let Diagram::Pie(pie) = lowered.diagram else {
                panic!("expected pie diagram")
            };
            assert_eq!(pie.slices[0].label, "Kept");
        }
    }

    #[test]
    fn directive_recovery_at_statement_separators() {
        for sep in ["\n;", " ; "] {
            let src =
                format!("flowchart TD\n%%{{unfinished{sep}%%{{init: {{theme: dark}}}}%%\nA\n");
            let result = lower(&src);
            assert_eq!(result.theme.name.unwrap().0, "dark");
            assert_eq!(
                result.diagnostics.len(),
                1,
                "{src:?}: {:?}",
                result.diagnostics
            );
            assert!(
                result.diagnostics[0]
                    .message
                    .starts_with("unclosed directive")
            );
            assert_eq!(flowchart::parse(&src).syntax().to_string(), src);
        }
        // An apostrophe in a node ID is not a directive quote. Even if later directives
        // appear quoted to the lookahead scanner, its search for the next marker is shared.
        let src = format!(
            "flowchart TD\n{}%%{{init: {{theme: dark}}}}%%\nB\n",
            "A';%%{unfinished\n".repeat(10_000)
        );
        let result = lower(&src);
        assert_eq!(result.diagnostics.len(), MAX_DIAGNOSTICS + 1);
        let Diagram::Graph(graph) = result.diagram else {
            panic!("expected graph")
        };
        assert!(graph.nodes.iter().any(|n| n.id == "B"));
        assert_eq!(flowchart::parse(&src).syntax().to_string(), src);
    }

    // LANGS-12, MTHEME-5: dispatch and theme extraction share parser boundaries.
    #[test]
    fn preamble_dispatch_and_theme_boundaries() {
        let src = "---\ntitle: |\n  @startuml\n---\npie\n  \"A\": 1\n";
        let result = lower(src);
        assert_eq!(result.kind, Kind::Pie);
        assert_eq!(
            Kind::of(&src.replace("pie", "flowchart-elk")),
            Kind::Flowchart
        );
        assert_eq!(result.diagnostics, []);
        let Diagram::Pie(pie) = result.diagram else {
            panic!("expected pie")
        };
        assert_eq!(
            pie.slices,
            [crate::pie::Slice {
                label: "A".into(),
                value: 1.0
            }]
        );

        let src = "%%{unclosed\npie\n  \"A\": 1\n";
        let result = lower(src);
        assert_eq!(result.kind, Kind::Pie);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].message,
            "unclosed directive: expected `}%%`"
        );
        assert_eq!(result.diagnostics[0].line_col(src), (1, 1));
        let Diagram::Pie(pie) = result.diagram else {
            panic!("expected pie")
        };
        assert_eq!(pie.slices[0].label, "A");

        let src = "---\npie\n  \"A\": 1\n";
        let result = lower(src);
        assert_eq!(result.kind, Kind::Pie);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].message,
            "unclosed frontmatter: expected a closing `---` line"
        );
        let Diagram::Pie(pie) = result.diagram else {
            panic!("expected pie")
        };
        assert_eq!(pie.slices[0].label, "A");

        let src = "flowchart TD\nA[\"%%{init: {'theme':'bogus'}}%%\"]\n%% %%{init: {'theme':'bogus'}}%%\n";
        let result = lower(src);
        assert_eq!(result.kind, Kind::Flowchart);
        assert_eq!(result.diagnostics, []);
        assert_eq!(result.theme, ThemeConfig::default());
        let Diagram::Graph(graph) = result.diagram else {
            panic!("expected graph")
        };
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].id, "A");

        let multiline = "%%{init: {\n 'theme': 'dark'\n}}%%\npie\n\"A\": 1\n";
        let result = lower(multiline);
        assert_eq!(result.kind, Kind::Pie);
        assert_eq!(result.theme.name.unwrap().0, "dark");
        assert_eq!(result.diagnostics, []);
        // MTHEME-5: C4 directives are collected from its parser too.
        let c4 = lower("%%{init: {'theme':'dark'}}%%\nC4Context\nPerson(user, \"User\")\n");
        assert_eq!(c4.kind, Kind::C4);
        assert_eq!(c4.theme.name.unwrap().0, "dark");
        assert_eq!(c4.diagnostics, []);
        assert_eq!(
            Kind::of("leading text\n@startuml\nAlice -> Bob: hi\n@enduml\n"),
            Kind::PlantUml
        );
        assert_eq!(Kind::of_lang(src, Some(Lang::PlantUml)), Kind::PlantUml);
        assert_eq!(
            Kind::of_lang("@startuml\n", Some(Lang::Mermaid)),
            Kind::Flowchart
        );
    }

    #[test]
    fn diagnostics_are_ordered_before_cap() {
        let mut src = "%%{init: {'theme':'bogus'}}%%\npie\n".to_owned();
        for _ in 0..510 {
            src.push_str("?\n");
        }
        let result = lower(&src);
        assert_eq!(result.diagnostics.len(), MAX_DIAGNOSTICS + 1);
        assert_eq!(result.diagnostics[0].line_col(&src), (1, 20));
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert!(
            result.diagnostics[0]
                .message
                .starts_with("`bogus` is not a Mermaid theme")
        );
        assert!(
            result
                .diagnostics
                .windows(2)
                .all(|w| w[0].range.start() <= w[1].range.start())
        );
        assert_eq!(
            result.diagnostics[MAX_DIAGNOSTICS].message,
            "11 more problems not shown"
        );
        assert_eq!(
            result.diagnostics[MAX_DIAGNOSTICS].severity,
            Severity::Error
        );
    }

    #[test]
    fn oversized_source_skips_normalization_and_theme_processing() {
        let src = "flowchart\r%%{init: broken}%%\nA";
        for lang in [
            None,
            Some(Lang::Mermaid),
            Some(Lang::Dot),
            Some(Lang::PlantUml),
        ] {
            let lowered = lower_as_with_limit(src, lang, 5);
            assert_eq!(lowered.diagnostics.len(), 1);
            assert!(lowered.diagnostics[0].message.contains("limit of 5 bytes"));
            assert_eq!(lowered.theme, ThemeConfig::default());
        }
    }

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

    /// `Lowered::kind` is the type the source was read as, with its language and name.
    #[test]
    fn lowered_kinds() {
        for (src, lang, kind, name) in [
            ("flowchart\nA\n", None, Kind::Flowchart, "flowchart"),
            (
                "sequenceDiagram\nA->>B: x\n",
                None,
                Kind::Sequence,
                "sequence diagram",
            ),
            (
                "swimlane-beta LR\n",
                None,
                Kind::Swimlane,
                "swimlane diagram",
            ),
            ("digraph { a -> b }\n", None, Kind::Dot, "DOT graph"),
            (
                "@startuml\nA -> B\n@enduml\n",
                None,
                Kind::PlantUml,
                "PlantUML diagram",
            ),
            ("pie\n\"a\" : 1\n", Some(Lang::Dot), Kind::Dot, "DOT graph"),
            (
                "graph { a }\n",
                Some(Lang::Mermaid),
                Kind::Flowchart,
                "flowchart",
            ),
        ] {
            let lowered = lower_as(src, lang);
            assert_eq!(lowered.kind, kind, "{src}");
            assert_eq!(kind.name(), name);
            assert_eq!(kind.lang(), lang.unwrap_or(kind.lang()));
        }
        assert_eq!(Kind::Dot.lang(), Lang::Dot);
        assert_eq!(Kind::PlantUml.lang(), Lang::PlantUml);
        assert_eq!(Kind::Gantt.lang(), Lang::Mermaid);
        // A source over the limit is not read as anything but the default.
        assert_eq!(lower_as_with_limit("pie\n", None, 1).kind, Kind::Flowchart);
        // Every name is distinct.
        let names: std::collections::HashSet<_> = Kind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(names.len(), Kind::ALL.len());
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
        assert_eq!(Lang::of_path(Path::new("a.PUML")), Some(Lang::PlantUml));
        assert_eq!(Kind::of_lang("pie\n", Some(Lang::PlantUml)), Kind::PlantUml);
        assert_eq!(Kind::of_lang("@startuml\n", None), Kind::PlantUml);
        assert_ne!(
            Kind::of_lang("@startuml\n", Some(Lang::Mermaid)),
            Kind::PlantUml
        );

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

    #[test]
    fn caps_diagnostics() {
        let mut src = String::from("pie\n");
        for _ in 0..MAX_DIAGNOSTICS + 9 {
            src.push_str("?\n");
        }
        let l = lower(&src);
        assert_eq!(l.diagnostics.len(), MAX_DIAGNOSTICS + 1);
        let last = l.diagnostics.last().unwrap();
        assert_eq!(last.message, "9 more problems not shown");
        assert_eq!(last.severity, Severity::Error);
        // At the first one left out: the header is line 1, so problem 501 is on line 502.
        assert_eq!(last.line_col(&src).0, MAX_DIAGNOSTICS + 2);
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
