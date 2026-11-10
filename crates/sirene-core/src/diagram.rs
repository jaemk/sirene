//! The entry point for every Mermaid diagram type sirene draws (LANGS-2).

use crate::ascii::{self, Options};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{self, Flowchart, scan};
use crate::gantt::{self, Gantt};
use crate::git::{self, GitGraph};
use crate::mindmap::{self, Mindmap};
use crate::pie::{self, Pie};
use crate::scene::{self, Scene, TextMetrics, Theme};
use crate::sequence::{self, Sequence};
use crate::timeline::{self, Timeline};
use crate::{class, er, state};

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
        } else {
            Kind::Flowchart
        }
    }
}

/// The Mermaid types sirene draws, for messages about the others (LANGS-4).
pub const DRAWN: &str = "flowchart, sequence, state, class, ER, git graph, timeline, pie, gantt, \
     and mindmap diagrams";

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
fn header(src: &str) -> &str {
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
        Kind::Flowchart => flowchart::parse(src).lower().into(),
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
        assert_eq!(other_diagram("journey\n"), Some("journey".to_owned()));
        assert_eq!(other_diagram("mindmap\n"), None);
        assert_eq!(other_diagram("gantt\n"), None);
        assert_eq!(other_diagram("pie\n"), None);
        assert_eq!(other_diagram("stateDiagram-v2\n"), None);
        assert_eq!(other_diagram("gitGraph\n"), None);
        assert_eq!(other_diagram("sequenceDiagram\n"), None);
        assert_eq!(other_diagram("flowchart\n"), None);
    }
}
