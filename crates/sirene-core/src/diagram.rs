//! The entry point for every Mermaid diagram type sirene draws (LANGS-2).

use crate::flowchart::{self, Lowered, scan};
use crate::state;

/// A Mermaid diagram type sirene draws, picked by the header keyword (LANGS-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Flowchart,
    State,
}

impl Kind {
    /// The type of the diagram `src` starts with; a flowchart unless the header says otherwise.
    pub fn of(src: &str) -> Kind {
        let word = header(src);
        if state::HEADERS.contains(&word) {
            Kind::State
        } else {
            Kind::Flowchart
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
        Kind::Flowchart => flowchart::parse(src).lower(),
        Kind::State => state::parse(src).lower(),
    }
}

/// The keyword of a Mermaid diagram type sirene does not draw, e.g. `pie` (FLOWCH-16).
pub fn other_diagram(src: &str) -> Option<String> {
    match Kind::of(src) {
        Kind::Flowchart => flowchart::parse(src).other_diagram(),
        Kind::State => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(Kind::of("stateDiagram-v2\n  A --> B\n"), Kind::State);
        assert_eq!(Kind::of("stateDiagram\n"), Kind::State);
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
        assert_eq!(other_diagram("pie\n"), Some("pie".to_owned()));
        assert_eq!(other_diagram("stateDiagram-v2\n"), None);
        assert_eq!(other_diagram("flowchart\n"), None);
    }
}
