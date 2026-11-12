//! Which PlantUML diagram kind a document is (PUML-1): the `@start...` word, then the first
//! statement only one kind accepts.

use super::link::Link;
use crate::flowchart::{Arrow, Stroke, scan};

/// A PlantUML diagram kind (PUML-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Sequence,
    Activity,
    /// Class and object diagrams.
    Class,
    State,
    /// Use case, component, and deployment diagrams, which PlantUML parses alike.
    Description,
    Timing,
    Mindmap,
    Wbs,
    Gantt,
    Json,
    Yaml,
    /// Another `@start...` kind, by its word (`salt`, `ditaa`, ...).
    Other(&'static str),
}

impl Kind {
    /// The kind's name in messages, e.g. "activity".
    pub fn name(self) -> &'static str {
        match self {
            Kind::Sequence => "sequence",
            Kind::Activity => "activity",
            Kind::Class => "class",
            Kind::State => "state",
            Kind::Description => "use case, component, and deployment",
            Kind::Timing => "timing",
            Kind::Mindmap => "mindmap",
            Kind::Wbs => "WBS",
            Kind::Gantt => "Gantt",
            Kind::Json => "JSON",
            Kind::Yaml => "YAML",
            Kind::Other(word) => word,
        }
    }
}

/// The `@start...` kinds other than `@startuml`, by the word after `@start`.
const START_KINDS: &[(&str, Kind)] = &[
    ("mindmap", Kind::Mindmap),
    ("wbs", Kind::Wbs),
    ("gantt", Kind::Gantt),
    ("json", Kind::Json),
    ("yaml", Kind::Yaml),
    ("salt", Kind::Other("salt")),
    ("ditaa", Kind::Other("ditaa")),
    ("dot", Kind::Other("dot")),
    ("math", Kind::Other("math")),
    ("latex", Kind::Other("latex")),
    ("chen", Kind::Other("chen")),
    ("ebnf", Kind::Other("ebnf")),
    ("regex", Kind::Other("regex")),
    ("files", Kind::Other("files")),
    ("chronology", Kind::Other("chronology")),
    ("wire", Kind::Other("wire")),
    ("nwdiag", Kind::Other("nwdiag")),
    ("board", Kind::Other("board")),
    ("creole", Kind::Other("creole")),
    ("def", Kind::Other("def")),
];

/// The kind an `@start...` word names; `None` for `uml`, whose kind its statements decide.
pub(crate) fn start_kind(word: &str) -> Option<Kind> {
    if word.eq_ignore_ascii_case("uml") {
        return None;
    }
    Some(
        START_KINDS
            .iter()
            .find(|(w, _)| w.eq_ignore_ascii_case(word))
            .map_or(Kind::Other("unknown"), |&(_, k)| k),
    )
}

/// Keywords only one kind starts a statement with, matched without regard to case.
const STRONG: &[(&str, Kind)] = &[
    ("participant", Kind::Sequence),
    ("boundary", Kind::Sequence),
    ("control", Kind::Sequence),
    ("collections", Kind::Sequence),
    ("activate", Kind::Sequence),
    ("deactivate", Kind::Sequence),
    ("autonumber", Kind::Sequence),
    ("autoactivate", Kind::Sequence),
    ("return", Kind::Sequence),
    ("alt", Kind::Sequence),
    ("opt", Kind::Sequence),
    ("loop", Kind::Sequence),
    ("par", Kind::Sequence),
    ("critical", Kind::Sequence),
    ("ref", Kind::Sequence),
    ("create", Kind::Sequence),
    ("destroy", Kind::Sequence),
    ("box", Kind::Sequence),
    ("newpage", Kind::Sequence),
    ("start", Kind::Activity),
    ("stop", Kind::Activity),
    ("kill", Kind::Activity),
    ("detach", Kind::Activity),
    ("elseif", Kind::Activity),
    ("endif", Kind::Activity),
    ("while", Kind::Activity),
    ("endwhile", Kind::Activity),
    ("repeat", Kind::Activity),
    ("backward", Kind::Activity),
    ("fork", Kind::Activity),
    ("split", Kind::Activity),
    ("switch", Kind::Activity),
    ("partition", Kind::Activity),
    ("state", Kind::State),
    ("class", Kind::Class),
    ("enum", Kind::Class),
    ("abstract", Kind::Class),
    ("annotation", Kind::Class),
    ("object", Kind::Class),
    ("map", Kind::Class),
    ("struct", Kind::Class),
    ("exception", Kind::Class),
    ("metaclass", Kind::Class),
    ("protocol", Kind::Class),
    ("usecase", Kind::Description),
    ("component", Kind::Description),
    ("node", Kind::Description),
    ("artifact", Kind::Description),
    ("cloud", Kind::Description),
    ("folder", Kind::Description),
    ("frame", Kind::Description),
    ("storage", Kind::Description),
    ("card", Kind::Description),
    ("file", Kind::Description),
    ("agent", Kind::Description),
    ("stack", Kind::Description),
    ("hexagon", Kind::Description),
    ("person", Kind::Description),
    ("port", Kind::Description),
    ("portin", Kind::Description),
    ("portout", Kind::Description),
    ("robust", Kind::Timing),
    ("concise", Kind::Timing),
    ("clock", Kind::Timing),
    ("binary", Kind::Timing),
    ("analog", Kind::Timing),
    ("nwdiag", Kind::Other("nwdiag")),
];

/// Keywords several kinds accept, which pick a kind only when no statement names one.
/// Sequence diagrams accept none of them, so they also outweigh the sequence default.
const WEAK: &[(&str, Kind)] = &[
    ("interface", Kind::Class),
    ("package", Kind::Class),
    ("namespace", Kind::Class),
    ("rectangle", Kind::Description),
];

/// Whether a link line has a use case `(name)` or a component `[name]` end before its text.
fn description_link(line: &str) -> bool {
    let head = line.split(':').next().unwrap_or(line);
    if !["--", "->", "<-", ".."].iter().any(|a| head.contains(a)) {
        return false;
    }
    let b = head.as_bytes();
    // Scan backwards, keeping the next `)` and `]` after each byte, so the scan is linear.
    let (mut paren, mut bracket) = (None, None);
    for i in (0..b.len()).rev() {
        let after_gap = i == 0 || b[i - 1] == b' ' || b[i - 1] == b'>' || b[i - 1] == b'-';
        let close = match b[i] {
            b'(' => paren,
            b'[' if i == 0 || b[i - 1] == b' ' => bracket,
            b')' => {
                paren = Some(i);
                None
            }
            b']' => {
                bracket = Some(i);
                None
            }
            _ => None,
        };
        if after_gap && close.is_some_and(|j| j > i + 1 && !matches!(b[i + 1], b'-' | b'#')) {
            return true;
        }
    }
    false
}

/// Whether a link is one no sequence arrow looks like: dotted, headless, or with a class
/// diagram head (`<|--`, `*--`, `o--`, `}o--`).
fn class_link(l: &Link) -> bool {
    let sequence_head = |a: Option<Arrow>| matches!(a, Some(Arrow::Point | Arrow::Cross));
    l.stroke == Stroke::Dotted
        || !(sequence_head(l.start) || sequence_head(l.end)) && l.unknown.is_empty()
}

/// The length of a word at the start of `s`.
fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

/// What one statement line says about the kind: a kind it alone belongs to, or one it hints at.
fn classify(line: &str) -> (Option<Kind>, Option<Kind>) {
    let word = &line[..word_len(line)];
    let rest = line[word.len()..].trim_start();
    let strong = |k| (Some(k), None);
    if line.starts_with("[*]") {
        return strong(Kind::State);
    }
    if line.starts_with('#') && line.contains(':') && !line.starts_with("#:") {
        // `#color:action;` is a colored activity action.
        return strong(Kind::Activity);
    }
    if line.starts_with(':') && line.len() > 1 {
        // `:Actor:` is a use case actor when nothing, an arrow, or `as` follows it; any other
        // `:` line starts an activity action.
        let inner = &line[1..];
        let actor = inner.find(':').is_some_and(|i| {
            let after = inner[i + 1..].trim_start();
            !inner[..i].contains(';')
                && (after.is_empty()
                    // `..` is a dotted link, `...` is trailing text.
                    || after.starts_with(['-', '<'])
                    || after.starts_with("..") && !after.starts_with("...")
                    || after.get(..2).is_some_and(|w| w.eq_ignore_ascii_case("as"))
                        && word_len(after) == 2)
        });
        if actor && !line.ends_with(';') {
            return strong(Kind::Description);
        }
        return strong(Kind::Activity);
    }
    if line.starts_with('|') && !line.starts_with("||") {
        return strong(Kind::Activity);
    }
    if line.starts_with("==") || line.starts_with("...") || line.starts_with("||") {
        return strong(Kind::Sequence);
    }
    if line.starts_with("(*)") {
        // The older activity syntax, which the activity lowering names in its error.
        return strong(Kind::Activity);
    }
    if line.starts_with('(') && !line.starts_with("()") {
        return strong(Kind::Description);
    }
    if line.starts_with("()") {
        return strong(Kind::Description);
    }
    if let Some(inner) = line.strip_prefix('[') {
        // `[Component]`; `[->`, `[o->`, `[x->` and `[<-` are sequence edge messages.
        let first = inner.chars().next();
        if matches!(first, Some('-' | '<' | '/' | '\\'))
            || (matches!(first, Some('o' | 'x')) && inner[1..].starts_with(['-', '<']))
        {
            return strong(Kind::Sequence);
        }
        if let Some(close) = inner.find(']') {
            // `[Task] lasts 3 days` and the like are Gantt tasks.
            let after = inner[close + 1..].trim_start();
            let verb = after[..word_len(after)].to_ascii_lowercase();
            if ["lasts", "requires", "starts", "ends", "happens"].contains(&verb.as_str())
                || (verb == "as" && after[2..].trim_start().starts_with('['))
            {
                return strong(Kind::Gantt);
            }
            return strong(Kind::Description);
        }
    }
    if line.starts_with('@') && line[1..].starts_with(|c: char| c.is_ascii_digit() || c == '+') {
        return strong(Kind::Timing);
    }
    if word.eq_ignore_ascii_case("project") && rest[..word_len(rest)].eq_ignore_ascii_case("starts")
    {
        return strong(Kind::Gantt);
    }
    if word.eq_ignore_ascii_case("if") && rest.starts_with('(') {
        return strong(Kind::Activity);
    }
    if word.eq_ignore_ascii_case("end") {
        let next = &rest[..word_len(rest)];
        if ["fork", "split", "merge", "switch", "while"]
            .iter()
            .any(|w| w.eq_ignore_ascii_case(next))
        {
            return strong(Kind::Activity);
        }
    }
    if word.eq_ignore_ascii_case("note") && rest.to_ascii_lowercase().starts_with("over") {
        return strong(Kind::Sequence);
    }
    if let Some(&(_, k)) = STRONG.iter().find(|(w, _)| w.eq_ignore_ascii_case(word)) {
        return strong(k);
    }
    if description_link(line) {
        return strong(Kind::Description);
    }
    if super::link::link(line).is_some_and(|l| class_link(&l)) {
        return (None, Some(Kind::Class));
    }
    let weak = WEAK
        .iter()
        .find(|(w, _)| w.eq_ignore_ascii_case(word))
        .map(|&(_, k)| k);
    (None, weak)
}

/// A block whose body lines say nothing about the kind.
enum Body {
    /// Runs to a line that is one of these, ignoring case and spacing.
    Until(&'static [&'static str]),
    /// A `skinparam ... {` block, at this `{` depth.
    Braces(usize),
}

const POSITIONS: &[&str] = &["left", "right", "center", "top", "bottom"];

/// The body a line opens: a block `note`, `legend`, `title`, `header`, `footer`,
/// `skinparam ... {`, or `<style>`.
fn opens_body(line: &str) -> Option<Body> {
    let word = line[..word_len(line)].to_ascii_lowercase();
    let rest = line[word.len()..].trim_start();
    let positions_only = |s: &str| {
        s.split_whitespace()
            .all(|w| POSITIONS.contains(&w.to_ascii_lowercase().as_str()))
    };
    if line
        .get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case("<style>"))
    {
        let one_line = line.to_ascii_lowercase().contains("</style>");
        return (!one_line).then_some(Body::Until(&["</style>"]));
    }
    match word.as_str() {
        // `floating note ...` opens the same body as `note ...`.
        "floating" if word_len(rest) == 4 && rest[..4].eq_ignore_ascii_case("note") => {
            opens_body(rest)
        }
        // `note ... : text` and `note "text" as N` are one line.
        "note" | "hnote" | "rnote" if !line.contains(':') && !rest.starts_with('"') => {
            Some(Body::Until(&[
                "end note",
                "endnote",
                "end hnote",
                "endhnote",
                "end rnote",
                "endrnote",
            ]))
        }
        "legend" if positions_only(rest) => Some(Body::Until(&["end legend", "endlegend"])),
        "title" if rest.is_empty() => Some(Body::Until(&["end title", "endtitle"])),
        "header" if positions_only(rest) => Some(Body::Until(&["end header", "endheader"])),
        "footer" if positions_only(rest) => Some(Body::Until(&["end footer", "endfooter"])),
        "left" | "right" | "center" => {
            let next = rest[..word_len(rest)].to_ascii_lowercase();
            let tail = rest[next.len()..].trim();
            match next.as_str() {
                "header" if tail.is_empty() => Some(Body::Until(&["end header", "endheader"])),
                "footer" if tail.is_empty() => Some(Body::Until(&["end footer", "endfooter"])),
                _ => None,
            }
        }
        "skinparam" if line.ends_with('{') => Some(Body::Braces(1)),
        _ => None,
    }
}

/// Whether `line` is one of `closers`, ignoring case and spacing.
fn closes(line: &str, closers: &[&str]) -> bool {
    let squeezed = line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    closers.contains(&squeezed.as_str())
}

/// The statement lines of the first document, with comments, blank lines, and block bodies
/// (notes, legends, titles, headers, footers, skinparam blocks) dropped, and whether it began
/// with `@start` (and its word).
fn statement_lines(src: &str) -> (Option<&str>, Vec<&str>) {
    let mut start = None;
    let mut out = Vec::new();
    let mut in_comment = false;
    let mut body = None;
    let mut started = !super::is_plantuml(src);
    for line in src[scan::bom_len(src)..].lines() {
        let line = line.trim();
        if !started {
            if let Some(word) = line.strip_prefix("@start") {
                started = true;
                start = Some(&word[..word_len(word)]);
            }
            continue;
        }
        if let Some(b) = &mut body {
            if line
                .get(..4)
                .is_some_and(|p| p.eq_ignore_ascii_case("@end"))
            {
                break;
            }
            let done = match b {
                Body::Until(closers) => closes(line, closers),
                Body::Braces(depth) => {
                    *depth += line.matches('{').count();
                    *depth -= line.matches('}').count().min(*depth);
                    *depth == 0
                }
            };
            if done {
                body = None;
            }
            continue;
        }
        if in_comment {
            in_comment = !line.contains("'/");
            continue;
        }
        if let Some(rest) = line.strip_prefix("/'") {
            in_comment = !rest.contains("'/");
            continue;
        }
        if line
            .get(..4)
            .is_some_and(|p| p.eq_ignore_ascii_case("@end"))
        {
            break;
        }
        if line.is_empty() || line.starts_with('\'') {
            continue;
        }
        body = opens_body(line);
        out.push(line);
    }
    (start, out)
}

/// The kind of the first document in `src`: the `@start...` word, else the first statement
/// only one kind accepts, else the first hint, else a sequence diagram (PUML-1).
pub fn kind(src: &str) -> Kind {
    let (start, lines) = statement_lines(src);
    if let Some(k) = start.and_then(start_kind) {
        return k;
    }
    let mut hint = None;
    for line in lines {
        let (strong, weak) = classify(line);
        if let Some(k) = strong {
            return k;
        }
        if hint.is_none() {
            hint = weak;
        }
    }
    hint.unwrap_or(Kind::Sequence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k("A -> B"), Kind::Sequence);
        assert_eq!(k("actor A\nA -> B"), Kind::Sequence);
        assert_eq!(k("participant A"), Kind::Sequence);
        assert_eq!(k("[-> A"), Kind::Sequence);
        assert_eq!(k("start\n:a;"), Kind::Activity);
        assert_eq!(k(":a;\n:b;"), Kind::Activity);
        assert_eq!(k(":multi\nline;"), Kind::Activity);
        assert_eq!(k("if (x) then\nendif"), Kind::Activity);
        assert_eq!(k("|Lane|\n:a;"), Kind::Activity);
        assert_eq!(k("class A\nA -> B"), Kind::Class);
        assert_eq!(k("A -> B\ninterface I"), Kind::Class);
        assert_eq!(k("A <|-- B"), Kind::Class);
        assert_eq!(k("A ..> B"), Kind::Class);
        assert_eq!(k("A -- B"), Kind::Class);
        assert_eq!(k("A --> B\nA ->x B\nA o-> B"), Kind::Sequence);
        assert_eq!(k("[*] --> A"), Kind::State);
        assert_eq!(k("state A"), Kind::State);
        assert_eq!(k("actor U\nU -> (Login)"), Kind::Description);
        assert_eq!(k(":User: --> (Use)"), Kind::Description);
        assert_eq!(k("[Web] --> [Db]"), Kind::Description);
        assert_eq!(k("node N"), Kind::Description);
        assert_eq!(k("[Design] lasts 3 days"), Kind::Gantt);
        assert_eq!(k("Project starts 2026-01-05"), Kind::Gantt);
        assert_eq!(k("[A] as [X] lasts 1 day"), Kind::Gantt);
        assert_eq!(k("[A] -> [B]"), Kind::Description);
        // Sequence diagrams accept `database`, and PlantUML tries them first.
        assert_eq!(k("database D\nD -> E"), Kind::Sequence);
        assert_eq!(k("robust \"W\" as W\n@0\nW is On"), Kind::Timing);
        assert_eq!(k("analog \"V\" as V\nV is 3"), Kind::Timing);
        assert_eq!(k("nwdiag {\nnetwork a {\nb\n}\n}"), Kind::Other("nwdiag"));
        assert_eq!(k("' comment\n/' block\nclass '/\nstart"), Kind::Activity);
        assert_eq!(kind("@startmindmap\n* a\n@endmindmap\n"), Kind::Mindmap);
        assert_eq!(kind("@startsalt\n{}\n@endsalt\n"), Kind::Other("salt"));
        assert_eq!(kind("start\n:a;\n"), Kind::Activity);
        assert_eq!(kind("text\n@startuml\nstate A\n@enduml\n"), Kind::State);
    }

    #[test]
    fn block_bodies_say_nothing() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(
            k("Alice -> Bob : hi\nnote left\nstart the process\nend note"),
            Kind::Sequence
        );
        assert_eq!(k("A -> B\nhnote over A\nstop\nendnote"), Kind::Sequence);
        assert_eq!(k("note as N\nclass X\nEnd  Note\nstart"), Kind::Activity);
        // One-line notes have no body.
        assert_eq!(k("note left of A : x\nstart"), Kind::Activity);
        assert_eq!(k("note \"x\" as N\nstart"), Kind::Activity);
        assert_eq!(k("A -> B\nlegend right\nstart\nendlegend"), Kind::Sequence);
        assert_eq!(k("A -> B\ntitle\nstate x\nend title"), Kind::Sequence);
        assert_eq!(k("title start\nstate x"), Kind::State);
        assert_eq!(k("A -> B\nheader\nstop\nendheader"), Kind::Sequence);
        assert_eq!(k("A -> B\ncenter footer\nstop\nend footer"), Kind::Sequence);
        assert_eq!(
            k("A -> B\nskinparam x {\n a {\n start\n }\n class y\n}"),
            Kind::Sequence
        );
        assert_eq!(k("skinparam x {\n}\nstart"), Kind::Activity);
        assert_eq!(
            k("<style>\nnode {\n  BackgroundColor red\n}\n</STYLE>\nAlice -> Bob"),
            Kind::Sequence
        );
        assert_eq!(k("<style>x</style>\nnode N"), Kind::Description);
        // An unclosed body ends at `@end`.
        assert_eq!(
            kind("@startuml\nnote left\nstart\n@enduml\n@startuml\nstate A\n@enduml\n"),
            Kind::Sequence
        );
    }

    #[test]
    fn actor_or_action() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k(":Step 1: prepare\nthe data;"), Kind::Activity);
        assert_eq!(k(":Read file: x]"), Kind::Activity);
        assert_eq!(k(":User: --> (Use)"), Kind::Description);
        assert_eq!(k(":User:-->(Use)"), Kind::Description);
        assert_eq!(k(":User: as U"), Kind::Description);
        assert_eq!(k(":User:"), Kind::Description);
        assert_eq!(k(":Note: assume x\nmore;"), Kind::Activity);
    }

    #[test]
    fn colored_action() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k("#pink:a;\n#lightblue:b;"), Kind::Activity);
        assert_eq!(k("#:a;"), Kind::Sequence);
    }

    #[test]
    fn note_forms_without_colon() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        // A `#color` or `on link` header with no `:` opens a block.
        assert_eq!(
            k("A -> B\nnote right of A #pink\nstart\nend note"),
            Kind::Sequence
        );
        assert_eq!(k("A -> B\nnote on link\nstart\nend note"), Kind::Sequence);
        assert_eq!(
            k("A -> B\nnote left on link #red\nstart\nend note"),
            Kind::Sequence
        );
        assert_eq!(k("A -> B\nrnote right\nstart\nend rnote"), Kind::Sequence);
        assert_eq!(k("A -> B\nhnote left\nstart\nendhnote"), Kind::Sequence);
        // The same with a `:` is one line.
        assert_eq!(k("note right of A #pink : x\nstart"), Kind::Activity);
        assert_eq!(k("note on link : x\nstart"), Kind::Activity);
        assert_eq!(k("note left on link #red : x\nstart"), Kind::Activity);
        // Floating notes.
        assert_eq!(
            k("A -> B\nfloating note left\nstart\nend note"),
            Kind::Sequence
        );
        assert_eq!(
            k("A -> B\nFloating Note right of A\nstart\nend note"),
            Kind::Sequence
        );
        assert_eq!(k("floating note left : x\nstart"), Kind::Activity);
        assert_eq!(k("floating note \"x\" as N\nstart"), Kind::Activity);
        // `floating` alone, or before another word, opens nothing.
        assert_eq!(k("floating\nstart"), Kind::Activity);
        assert_eq!(k("floating notes\nstart"), Kind::Activity);
    }

    #[test]
    fn closers_match_exactly() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        // A closer with trailing text, or for another block, does not end the body.
        assert_eq!(
            k("A -> B\nnote left\nend note x\nstart\nend note"),
            Kind::Sequence
        );
        assert_eq!(
            k("A -> B\nlegend\nend note\nstart\nendlegend"),
            Kind::Sequence
        );
        assert_eq!(
            k("A -> B\nnote left\nendlegend\nstart\nendnote"),
            Kind::Sequence
        );
        assert_eq!(k("A -> B\nnote left\nend  NOTE\nstart"), Kind::Activity);
        assert_eq!(k("A -> B\nlegend\nEND\tLegend\nstart"), Kind::Activity);
        assert_eq!(k("A -> B\ntitle\nend title\nstart"), Kind::Activity);
        // A note's closer is not a statement of its own.
        assert_eq!(k("end note\nstate A"), Kind::State);
    }

    #[test]
    fn header_footer_with_text_are_one_line() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k("header Page 1\nstart"), Kind::Activity);
        assert_eq!(k("footer the end\nstate A"), Kind::State);
        assert_eq!(k("left footer text\nstart"), Kind::Activity);
        assert_eq!(k("title Plan\nstate A"), Kind::State);
        assert_eq!(k("legend\nstate x\nend legend\nstart"), Kind::Activity);
        assert_eq!(k("A -> B\nright header\nstop\nendheader"), Kind::Sequence);
        assert_eq!(k("A -> B\nheader right\nstop\nendheader"), Kind::Sequence);
        assert_eq!(
            k("A -> B\nlegend top left\nstart\nend legend"),
            Kind::Sequence
        );
        assert_eq!(k("legend bottom\nstart\nend legend\nstate A"), Kind::State);
        // `legend` with text on its line is not a block.
        assert_eq!(k("legend x\nstart"), Kind::Activity);
    }

    #[test]
    fn skinparam_bodies() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k("skinparam monochrome true\nstart"), Kind::Activity);
        assert_eq!(k("skinparam x { y }\nstart"), Kind::Activity);
        assert_eq!(
            k("A -> B\nskinparam x {\n a { b }\n start\n}\nstate"),
            Kind::State
        );
        // Stray closers do not underflow.
        assert_eq!(k("skinparam x {\n}\n}\n}\nstart"), Kind::Activity);
        // Unclosed: runs to `@end`.
        assert_eq!(k("skinparam x {\nstart"), Kind::Sequence);
        // The body ends on the line that balances it.
        assert_eq!(k("skinparam x {\n a {\n}\nstart\n}\n"), Kind::Sequence);
        assert_eq!(k("skinparam x {\n a {\n}\n}\nstart"), Kind::Activity);
    }

    #[test]
    fn actor_arrow_after_colon() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k(":A: ..> (B)"), Kind::Description);
        assert_eq!(k(":A: .. B"), Kind::Description);
        assert_eq!(k(":A: <-- B"), Kind::Description);
        assert_eq!(k(":A: AS B"), Kind::Description);
        // Trailing `...` and a word starting with `as` are action text.
        assert_eq!(k(":Wait: ... then\nmore;"), Kind::Activity);
        assert_eq!(k(":A: asx\nmore;"), Kind::Activity);
        assert_eq!(k(":A: as"), Kind::Description);
        // A `;` before the closing colon, or a final `;`, is an action.
        assert_eq!(k(":a; b: -> c"), Kind::Activity);
        assert_eq!(k(":A: --> B;"), Kind::Activity);
        // A lone colon is no action or actor.
        assert_eq!(k(":"), Kind::Sequence);
    }

    #[test]
    fn color_prefix_only_decides_with_a_colon() {
        let k = |s: &str| kind(&format!("@startuml\n{s}\n@enduml\n"));
        assert_eq!(k("#pink\nclass A"), Kind::Class);
        assert_eq!(k("#pink;line:red\nclass A"), Kind::Activity);
        // An earlier statement still wins.
        assert_eq!(k("class A {\n#prot : int\n}"), Kind::Class);
        assert_eq!(k("state A\n#pink:a;"), Kind::State);
        // A later one overrides a hint, but not a bare sequence line.
        assert_eq!(k("A <|-- B\n#pink:a;"), Kind::Activity);
        // Inside a skinparam or note body it says nothing.
        assert_eq!(k("skinparam x {\n#pink:a;\n}\nstate A"), Kind::State);
        assert_eq!(k("note left\n#pink:a;\nend note\nstate A"), Kind::State);
    }

    /// The first implementation: a forward scan that looks ahead for each opener's closer.
    fn description_link_reference(line: &str) -> bool {
        let head = line.split(':').next().unwrap_or(line);
        if !["--", "->", "<-", ".."].iter().any(|a| head.contains(a)) {
            return false;
        }
        let b = head.as_bytes();
        (0..b.len()).any(|i| {
            let after_gap = i == 0 || b[i - 1] == b' ' || b[i - 1] == b'>' || b[i - 1] == b'-';
            let opens = match b[i] {
                b'(' => Some(')'),
                b'[' if i == 0 || b[i - 1] == b' ' => Some(']'),
                _ => None,
            };
            after_gap
                && opens.is_some_and(|close| {
                    head[i + 1..]
                        .find(close)
                        .is_some_and(|j| j > 0 && !head[i + 1..i + 1 + j].starts_with(['-', '#']))
                })
        })
    }

    #[test]
    fn description_link_matches_reference() {
        let alphabet = ['(', ')', '[', ']', ' ', '-', '>', '#', 'a', ':'];
        let mut count = 0usize;
        let mut buf = Vec::new();
        fn walk(buf: &mut Vec<char>, left: usize, alphabet: &[char], count: &mut usize) {
            let s: String = buf.iter().collect();
            assert_eq!(
                description_link(&s),
                description_link_reference(&s),
                "{s:?}"
            );
            *count += 1;
            if left == 0 {
                return;
            }
            for &c in alphabet {
                buf.push(c);
                walk(buf, left - 1, alphabet, count);
                buf.pop();
            }
        }
        walk(&mut buf, 6, &alphabet, &mut count);
        assert!(count > 1_000_000);
    }

    #[test]
    fn description_link_is_linear() {
        let line = format!("a -> {}", "( ".repeat(20_000));
        let t = std::time::Instant::now();
        assert!(!description_link(&line));
        assert!(t.elapsed() < std::time::Duration::from_millis(500));
        assert!(description_link("a -> (b)"));
        assert!(description_link("[a] -> b"));
        assert!(!description_link("a -> ()"));
        assert!(!description_link("a --> [-x]"));
        assert!(!description_link("a -> b"));
    }
}
