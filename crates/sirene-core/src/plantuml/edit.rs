//! Drawing edits to PlantUML use case, component, and deployment diagrams (WYSIWY-30): adding,
//! renaming, connecting, and deleting elements and links. Elements are named by their id and
//! links by their index in the lowered graph. Each edit is checked by lowering the result, and
//! refused when it would not read back.

use std::ops::Range;

use rowan::TextRange;

use super::Kind;
use super::description::{ELEMENTS, element};
use super::lines::name_end;
use super::link;
use crate::diagnostic::Severity;
use crate::diagram::Diagram;
use crate::edits::{combine, indent, line_start, newline, span, statement_span};
use crate::flowchart::edit::TextEdit;
use crate::flowchart::{Flowchart, Shape};

/// Whether `src` is a PlantUML diagram these edits apply to.
pub fn editable(src: &str) -> bool {
    super::parse(src).kind() == Kind::Description
}

/// The graph `src` lowers to and its number of errors; `None` for another kind.
fn lower(src: &str) -> Option<(Flowchart, usize)> {
    let parse = super::parse(src);
    if parse.kind() != Kind::Description {
        return None;
    }
    let (diagram, d) = parse.lower();
    let Diagram::Graph(chart) = diagram else {
        return None;
    };
    let errors = d.iter().filter(|d| d.severity == Severity::Error).count();
    Some((chart, errors))
}

/// The keyword that declares an element of `shape`.
fn keyword(shape: Shape) -> &'static str {
    match shape {
        Shape::Rect => "rectangle",
        _ => (ELEMENTS.iter())
            .find(|(_, s)| *s == shape)
            .map_or("rectangle", |(k, _)| k),
    }
}

/// `id` as written in a statement: bare when it is one name, else quoted.
fn reference(id: &str) -> Option<String> {
    if id.is_empty() || id.contains('"') {
        return None;
    }
    let plain = name_end(id) == id.len() && !id.starts_with(['(', ':', '"']);
    Some(if plain {
        id.to_owned()
    } else {
        format!("\"{id}\"")
    })
}

/// `text` as a quoted label: a line break written `\n`. `None` when it can't be written.
fn quoted(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains('"') || text.contains("\\n") {
        return None;
    }
    Some(format!(
        "\"{}\"",
        text.replace("\r\n", "\n").replace('\n', "\\n")
    ))
}

/// The source of a statement line holding `r`, trimmed.
fn line_of(src: &str, r: TextRange) -> Range<usize> {
    let start = line_start(src, usize::from(r.start()));
    let end = src[start..].find('\n').map_or(src.len(), |n| start + n);
    let text = &src[start..end];
    let lead = text.len() - text.trim_start().len();
    start + lead..start + text.trim_end().len()
}

/// The edit adding `line` on its own line before `@enduml`, or at the end.
fn append(src: &str, line: &str) -> TextEdit {
    let nl = newline(src);
    let mut at = src.len();
    let mut offset = 0;
    for l in src.split_inclusive('\n') {
        if l.trim_start().to_ascii_lowercase().starts_with("@enduml") {
            at = offset;
        }
        offset += l.len();
    }
    let before = src[..at].trim_end();
    let pad = match before.rfind('\n') {
        Some(n) if !before[n + 1..].trim_start().starts_with('@') => indent(src, n + 1).to_owned(),
        _ => String::new(),
    };
    let lead = if at == 0 || src[..at].ends_with('\n') {
        ""
    } else {
        nl
    };
    TextEdit {
        range: at..at,
        text: format!("{lead}{pad}{line}{nl}"),
    }
}

/// The graph's nodes as (id, shape) and edges as (from, to, label), sorted.
type Outline = (Vec<(String, Shape)>, Vec<(String, String, String)>);

fn shape_of(chart: &Flowchart) -> Outline {
    let mut nodes: Vec<_> = (chart.nodes.iter())
        .map(|n| (n.id.clone(), n.shape))
        .collect();
    nodes.sort_by(|a, b| a.0.cmp(&b.0));
    let mut edges: Vec<_> = (chart.edges.iter())
        .map(|e| {
            let label = e.label.as_ref().map(|l| l.text.clone()).unwrap_or_default();
            (e.from.clone(), e.to.clone(), label)
        })
        .collect();
    edges.sort();
    (nodes, edges)
}

/// `edit` applied to `src` when the result reads back with no new errors and `ok` holds for
/// the graphs before and after.
fn checked(
    src: &str,
    edit: TextEdit,
    ok: impl Fn(&Flowchart, &Flowchart) -> bool,
) -> Option<TextEdit> {
    let (before, errors) = lower(src)?;
    let (after, now) = lower(&edit.apply(src))?;
    (now <= errors && ok(&before, &after)).then_some(edit)
}

/// Shows element `id` as `text`: the name in its last declaration is rewritten, else a
/// declaration is added before `@enduml`.
pub fn set_node_label(src: &str, id: &str, text: &str) -> Option<TextEdit> {
    let (chart, _) = lower(src)?;
    let node = chart.node(id)?;
    if node.shape == Shape::Note || node.label.text == text {
        return None;
    }
    let label = quoted(text)?;
    let named = format!("{label} as {}", reference(id)?);
    let ok = |a: &Flowchart, b: &Flowchart| {
        shape_of(a) == shape_of(b)
            && (b.node(id)).is_some_and(|n| n.label.text.ends_with(text.trim()))
    };
    // The last line that declares it, its name between the keyword and what follows.
    for &m in node.mentions.iter().rev() {
        let line = line_of(src, m);
        if span(m) != line {
            continue;
        }
        let t = &src[line.clone()];
        if link::link(t).is_some() {
            continue;
        }
        let w = super::lines::word(t);
        let (start, after) = if ELEMENTS.iter().any(|(k, _)| k.eq_ignore_ascii_case(w)) {
            let after = t[w.len()..].trim_start();
            (t.len() - after.len(), after)
        } else {
            (0, t)
        };
        let Some((_, _, _, rest)) = element(after) else {
            continue;
        };
        let end = t.len() - rest.len();
        let kw = if start == 0 {
            format!("{} ", keyword(node.shape))
        } else {
            String::new()
        };
        let body = format!("{kw}{named}");
        let pad = if rest.is_empty() { "" } else { " " };
        let at = line.start + start;
        let e = TextEdit {
            range: at..line.start + end,
            text: format!("{body}{pad}"),
        };
        return checked(src, e, ok);
    }
    let e = append(src, &format!("{} {named}", keyword(node.shape)));
    checked(src, e, ok)
}

/// Shows link `i` as `text`, or without a label when `text` is empty.
pub fn set_edge_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let (chart, _) = lower(src)?;
    let edge = chart.edges.get(i)?;
    let line = line_of(src, edge.range);
    let t = &src[line.clone()];
    let l = link::link(t)?;
    let to_end = (l.to.as_ptr() as usize - t.as_ptr() as usize) + l.to.len();
    let text = text.trim();
    if text.contains('\n') || text.contains('"') {
        return None;
    }
    let new = if text.is_empty() {
        String::new()
    } else {
        format!(" : {text}")
    };
    let e = TextEdit {
        range: line.start + to_end..line.end,
        text: new,
    };
    let (from, to) = (edge.from.clone(), edge.to.clone());
    checked(src, e, |a, b| {
        a.nodes.len() == b.nodes.len()
            && b.edges.len() == a.edges.len()
            && (b.edges.get(i)).is_some_and(|e| {
                e.from == from
                    && e.to == to
                    && e.label.as_ref().map_or("", |l| l.text.as_str()) == text
            })
    })
}

/// Adds an element shown as `text`: a use case in a diagram with use cases, else a rectangle,
/// with the first free id `nK`. Returns its id and the edit.
pub fn add_node(src: &str, text: &str) -> Option<(String, TextEdit)> {
    let (chart, _) = lower(src)?;
    let id = (1..)
        .map(|k| format!("n{k}"))
        .find(|id| chart.node(id).is_none() && !chart.subgraphs.iter().any(|s| &s.id == id))?;
    let kw = if chart.nodes.iter().any(|n| n.shape == Shape::UseCase) {
        "usecase"
    } else {
        "rectangle"
    };
    let e = append(src, &format!("{kw} {} as {id}", quoted(text)?));
    let e = checked(src, e, |a, b| {
        b.nodes.len() == a.nodes.len() + 1
            && b.edges.len() == a.edges.len()
            && b.node(&id).is_some()
    })?;
    Some((id, e))
}

/// Adds a link from `from` to `to`.
pub fn add_edge(src: &str, from: &str, to: &str) -> Option<TextEdit> {
    let (chart, _) = lower(src)?;
    chart.node(from)?;
    chart.node(to)?;
    let e = append(src, &format!("{} --> {}", reference(from)?, reference(to)?));
    checked(src, e, |a, b| {
        a.nodes.len() == b.nodes.len()
            && b.edges.len() == a.edges.len() + 1
            && (b.edges.last()).is_some_and(|e| e.from == from && e.to == to)
    })
}

/// Removes links `edges`, and elements `nodes` with every statement naming them. An element
/// named only on a removed link is kept, declared in its place.
pub fn delete(src: &str, nodes: &[&str], edges: &[usize]) -> Option<TextEdit> {
    let (chart, _) = lower(src)?;
    let gone = |id: &str| nodes.contains(&id);
    let mut lines: Vec<Range<usize>> = Vec::new();
    for n in nodes {
        lines.extend(chart.node(n)?.mentions.iter().map(|&m| line_of(src, m)));
    }
    let removed: Vec<usize> = (0..chart.edges.len())
        .filter(|i| {
            let e = &chart.edges[*i];
            edges.contains(i) || gone(&e.from) || gone(&e.to)
        })
        .collect();
    lines.extend(removed.iter().map(|&i| line_of(src, chart.edges[i].range)));
    lines.sort_by_key(|r| r.start);
    lines.dedup();
    if lines.is_empty() {
        return None;
    }
    // Elements whose every mention is on a removed line, and that stay.
    let on = |r: TextRange| lines.contains(&line_of(src, r));
    let kept: Vec<_> = (chart.nodes.iter())
        .filter(|n| !gone(&n.id) && n.mentions.iter().all(|&m| on(m)))
        .collect();
    let mut out = Vec::new();
    for l in &lines {
        let mut text = String::new();
        for n in kept
            .iter()
            .filter(|n| n.mentions.iter().any(|&m| line_of(src, m) == *l))
        {
            let r = reference(&n.id)?;
            let decl = match n.shape {
                Shape::UseCase => format!("({})", n.id),
                Shape::Actor => format!(":{}:", n.id),
                _ if n.shape == Shape::Rect && !n.id.contains(']') => format!("[{}]", n.id),
                s => format!("{} {r}", keyword(s)),
            };
            if !text.is_empty() {
                text.push_str(newline(src));
                text.push_str(indent(src, l.start));
            }
            text.push_str(&decl);
        }
        if text.is_empty() {
            let r = TextRange::new((l.start as u32).into(), (l.end as u32).into());
            out.push((statement_span(src, r), String::new()));
        } else {
            out.push((l.clone(), text));
        }
    }
    let e = combine(src, out)?;
    let want = {
        let (n, mut e) = shape_of(&chart);
        // An actor's shape follows whether the diagram has use cases, so only ids are compared.
        let n: Vec<_> = (n.into_iter())
            .filter(|(id, _)| !gone(id))
            .map(|(id, _)| id)
            .collect();
        let mut drop: Vec<_> = (removed.iter())
            .map(|&i| {
                let x = &chart.edges[i];
                let label = x.label.as_ref().map(|l| l.text.clone()).unwrap_or_default();
                (x.from.clone(), x.to.clone(), label)
            })
            .collect();
        drop.sort();
        for d in drop {
            if let Some(at) = e.iter().position(|x| *x == d) {
                e.remove(at);
            }
        }
        (n, e)
    };
    checked(src, e, |_, b| {
        let (n, e) = shape_of(b);
        (n.into_iter().map(|(id, _)| id).collect(), e) == want
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(src: &str, e: Option<TextEdit>) -> String {
        e.expect("an edit").apply(src)
    }

    const SRC: &str = "@startuml\nactor User\n(Login) as L\nUser --> L\n@enduml\n";

    #[test]
    fn renames() {
        assert!(editable(SRC) && !editable("@startuml\nA -> B : hi\n@enduml\n"));
        assert_eq!(
            apply(SRC, set_node_label(SRC, "User", "Customer")),
            "@startuml\nactor \"Customer\" as User\n(Login) as L\nUser --> L\n@enduml\n"
        );
        assert_eq!(
            apply(SRC, set_node_label(SRC, "L", "Sign in")),
            "@startuml\nactor User\nusecase \"Sign in\" as L\nUser --> L\n@enduml\n"
        );
        assert_eq!(set_node_label(SRC, "L", "a \"b\""), None);
        let linked = "@startuml\n[A] --> [B]\n@enduml\n";
        assert_eq!(
            apply(linked, set_node_label(linked, "B", "Two\nlines")),
            "@startuml\n[A] --> [B]\nrectangle \"Two\\nlines\" as B\n@enduml\n"
        );
        let e = apply(SRC, set_edge_label(SRC, 0, "uses"));
        assert_eq!(
            e,
            "@startuml\nactor User\n(Login) as L\nUser --> L : uses\n@enduml\n"
        );
        assert_eq!(apply(&e, set_edge_label(&e, 0, "")), SRC);
    }

    #[test]
    fn adds() {
        let (id, e) = add_node(SRC, "Logout").unwrap();
        assert_eq!(id, "n1");
        let out = e.apply(SRC);
        assert_eq!(
            out,
            "@startuml\nactor User\n(Login) as L\nUser --> L\nusecase \"Logout\" as n1\n@enduml\n"
        );
        assert_eq!(
            apply(&out, add_edge(&out, "User", "n1")),
            "@startuml\nactor User\n(Login) as L\nUser --> L\nusecase \"Logout\" as n1\n\
             User --> n1\n@enduml\n"
        );
        let spaced = "@startuml\n(Use case) --> (Other)\n@enduml\n";
        assert_eq!(
            apply(spaced, add_edge(spaced, "Other", "Use case")),
            "@startuml\n(Use case) --> (Other)\nOther --> \"Use case\"\n@enduml\n"
        );
    }

    #[test]
    fn deletes() {
        let two = "@startuml\nactor User\n(Login) as L\n(Help)\nUser --> L\n@enduml\n";
        assert_eq!(
            apply(two, delete(two, &["L"], &[])),
            "@startuml\nactor User\n(Help)\n@enduml\n"
        );
        // A lone `actor` reads as a sequence diagram, so the delete is refused.
        assert_eq!(delete(SRC, &["L"], &[]), None);
        assert_eq!(
            apply(SRC, delete(SRC, &[], &[0])),
            "@startuml\nactor User\n(Login) as L\n@enduml\n"
        );
        // An element named only on a removed link stays.
        let linked = "@startuml\n(A) --> (B)\n(B) --> (C)\n@enduml\n";
        assert_eq!(
            apply(linked, delete(linked, &["B"], &[])),
            "@startuml\n(A)\n(C)\n@enduml\n"
        );
        assert_eq!(delete(SRC, &[], &[]), None);
    }
}
