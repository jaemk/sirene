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
    if line.starts_with(':') && line.len() > 1 {
        // `:Actor:` is a use case actor; any other `:` line starts an activity action.
        let inner = &line[1..];
        if inner.find(':').is_some_and(|i| !inner[..i].contains(';')) && !line.ends_with(';') {
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

/// The statement lines of the first document, with comments and blank lines dropped, and
/// whether it began with `@start` (and its word).
fn statement_lines(src: &str) -> (Option<&str>, Vec<&str>) {
    let mut start = None;
    let mut out = Vec::new();
    let mut in_comment = false;
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
}
