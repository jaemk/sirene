//! Source edits for changes made on a requirement diagram's drawing (WYSIWY-25).
//!
//! Requirements and elements are named by their name (a quoted one without its quotes), which is
//! their node id in the lowered chart, and relationships by their index among its edges.

use super::parser::RELATIONS;
use crate::edits::{append, combine, indent, newline, normalize, span, statement_span, token};
use crate::flowchart::Flowchart;
use crate::flowchart::edit::TextEdit;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// The relationship types, for menus: `(title, keyword)`.
pub const RELATIONSHIPS: &[(&str, &str)] = &[
    ("Contains", "contains"),
    ("Copies", "copies"),
    ("Derives", "derives"),
    ("Satisfies", "satisfies"),
    ("Verifies", "verifies"),
    ("Refines", "refines"),
    ("Traces", "traces"),
];

/// A parsed source and the chart it lowers to.
struct Doc {
    root: SyntaxNode,
    chart: Flowchart,
}

impl Doc {
    fn new(src: &str) -> Self {
        let parse = super::parse(src);
        Doc {
            root: parse.syntax(),
            chart: parse.lower().flowchart,
        }
    }

    /// Every statement without a syntax error, in source order.
    fn statements(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
        self.root
            .children()
            .filter(|s| !s.descendants().any(|n| n.kind() == ERROR))
    }

    /// The statement of relationship `edge`.
    fn relationship(&self, edge: usize) -> Option<SyntaxNode> {
        let range = self.chart.edges.get(edge)?.range;
        self.root
            .children()
            .find(|s| s.kind() == RELATION_STMT && s.text_range() == range)
    }

    /// `name`, or `name 2`, `name 3`, ...: one no node has.
    fn unused(&self, name: &str) -> String {
        (1..=self.chart.nodes.len() + 1)
            .map(|n| match n {
                1 => name.to_owned(),
                n => format!("{name} {n}"),
            })
            .find(|n| self.chart.node(n).is_none())
            .unwrap_or_default()
    }
}

/// A name or value token's text: quotes removed from a quoted one, trimmed.
fn value(t: &SyntaxToken) -> String {
    match t.kind() {
        STRING => text::unquote(t.text()).trim().to_owned(),
        _ => t.text().trim().to_owned(),
    }
}

/// The name tokens a statement mentions: a definition's, a relationship's two, a `name:::class`
/// statement's, or those in a `class` or `style` list.
fn names(stmt: &SyntaxNode) -> Vec<SyntaxToken> {
    let direct = || {
        stmt.children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| matches!(t.kind(), IDENT | STRING))
    };
    match stmt.kind() {
        REQUIREMENT => direct().take(1).collect(),
        RELATION_STMT => direct().collect(),
        CLASS_STMT | STYLE_STMT => match stmt.children().find(|n| n.kind() == ID_LIST) {
            Some(list) => list
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| t.kind() == IDENT)
                .collect(),
            None => direct().take(1).collect(),
        },
        _ => Vec::new(),
    }
}

/// `name` as source: as written when it is a plain word, otherwise quoted.
fn source_name(name: &str) -> String {
    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', "'"))
    }
}

/// A name for `text`: on one line, trimmed.
fn name_of(text: &str) -> String {
    normalize(text).replace('\n', " ")
}

/// Adds a requirement named `name` (made unique with a number), with no fields. Returns its
/// name and the edit.
pub fn add_requirement(src: &str, name: &str) -> (String, TextEdit) {
    let doc = Doc::new(src);
    let name = doc.unused(&name_of(name));
    let last = doc.root.children().last();
    // `}` goes under `requirement`, at the indent `append` gives the line.
    let under = last
        .as_ref()
        .map(|s| indent(src, usize::from(s.text_range().start())))
        .filter(|i| !i.is_empty())
        .unwrap_or("    ");
    let line = format!(
        "requirement {} {{{}{under}}}",
        source_name(&name),
        newline(src)
    );
    let edit = append(src, last.as_ref(), &line);
    (name, edit)
}

/// Adds a relationship `FROM - traces -> TO` at the end. `None` when either does not exist.
pub fn add_relationship(src: &str, from: &str, to: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    doc.chart.node(from)?;
    doc.chart.node(to)?;
    let line = format!("{} - traces -> {}", source_name(from), source_name(to));
    Some(append(src, doc.root.children().last().as_ref(), &line))
}

/// The name of `id` as the editor shows it: without its stereotype.
pub fn label(src: &str, id: &str) -> Option<String> {
    Doc::new(src).chart.node(id).map(|_| id.to_owned())
}

/// Renames requirement or element `id` to `name` wherever it is named. `None` when there is no
/// such node, the name is empty or taken, or nothing changes.
pub fn rename(src: &str, id: &str, name: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let name = name_of(name);
    doc.chart.node(id)?;
    if name.is_empty() || name == id || doc.chart.node(&name).is_some() {
        return None;
    }
    let mut edits = Vec::new();
    for s in doc.statements() {
        for t in names(&s) {
            if value(&t) != id {
                continue;
            }
            // A list takes only a plain word.
            let listed = t.parent().is_some_and(|p| p.kind() == ID_LIST);
            let text = source_name(&name);
            if listed && text.starts_with('"') {
                return None;
            }
            edits.push((span(t.text_range()), text));
        }
    }
    combine(src, edits)
}

/// Sets the type of relationship `edge` to `kind`, one of the keywords in [`RELATIONSHIPS`].
/// `None` when there is no such relationship or type, or nothing changes.
pub fn set_relationship_type(src: &str, edge: usize, kind: &str) -> Option<TextEdit> {
    if !RELATIONS.contains(&kind) {
        return None;
    }
    let doc = Doc::new(src);
    let rel = token(&doc.relationship(edge)?, RELATION)?;
    let edit = TextEdit {
        range: span(rel.text_range()),
        text: kind.to_owned(),
    };
    (!rel.text().eq_ignore_ascii_case(kind)).then_some(edit)
}

/// The type of relationship `edge`, lowercase.
pub fn relationship_type(src: &str, edge: usize) -> Option<String> {
    let doc = Doc::new(src);
    let rel = token(&doc.relationship(edge)?, RELATION)?;
    Some(rel.text().to_ascii_lowercase())
}

/// Removes relationships `edges` and requirements or elements `ids`, with every statement
/// naming one of them (and their names from `class` and `style` lists). `None` when nothing is
/// removed.
pub fn delete(src: &str, ids: &[&str], edges: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut doomed: Vec<rowan::TextRange> = edges
        .iter()
        .filter_map(|&i| doc.chart.edges.get(i).map(|e| e.range))
        .collect();
    let mut edits = Vec::new();
    for s in doc.statements() {
        let all = names(&s);
        let gone = |t: &SyntaxToken| ids.contains(&value(t).as_str());
        if !all.iter().any(gone) {
            continue;
        }
        match s.children().find(|n| n.kind() == ID_LIST) {
            Some(list) => {
                let kept: Vec<String> = all.iter().filter(|t| !gone(t)).map(value).collect();
                if kept.is_empty() {
                    doomed.push(s.text_range());
                } else {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            None => doomed.push(s.text_range()),
        }
    }
    doomed.sort_by_key(|r| r.start());
    doomed.dedup();
    edits.extend(
        doomed
            .into_iter()
            .map(|r| (statement_span(src, r), String::new())),
    );
    if edits.is_empty() {
        return None;
    }
    combine(src, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(src: &str) -> Flowchart {
        super::super::parse(src).lower().flowchart
    }

    fn diagnostics(src: &str) -> usize {
        super::super::parse(src).lower().diagnostics.len()
    }

    fn apply(src: &str, e: Option<TextEdit>) -> String {
        e.expect("an edit").apply(src)
    }

    const SRC: &str = "requirementDiagram\n    requirement req {\n        id: 1\n    }\n    element sys {\n        type: system\n    }\n    sys - satisfies -> req\n    style req fill:#f00\n";

    #[test]
    fn adds_requirements() {
        let (name, e) = add_requirement(SRC, "New requirement");
        assert_eq!(name, "New requirement");
        let out = e.apply(SRC);
        assert!(
            out.ends_with("    requirement \"New requirement\" {\n    }\n"),
            "{out}"
        );
        assert!(chart(&out).node("New requirement").is_some());
        assert_eq!(diagnostics(&out), 0, "{out}");
        let (name, _) = add_requirement(&out, "New requirement");
        assert_eq!(name, "New requirement 2");
        let (name, e) = add_requirement("requirementDiagram\n", "r");
        assert_eq!(name, "r");
        assert_eq!(
            e.apply("requirementDiagram\n"),
            "requirementDiagram\n    requirement r {\n    }\n"
        );
    }

    #[test]
    fn adds_relationships() {
        let out = apply(SRC, add_relationship(SRC, "req", "sys"));
        assert!(out.ends_with("    req - traces -> sys\n"), "{out}");
        assert_eq!(chart(&out).edges.len(), 2);
        assert!(add_relationship(SRC, "req", "nope").is_none());
    }

    #[test]
    fn renames() {
        let out = apply(SRC, rename(SRC, "req", "speed"));
        assert_eq!(
            out,
            SRC.replace("req {", "speed {")
                .replace("-> req", "-> speed")
                .replace("style req", "style speed")
        );
        assert!(chart(&out).node("speed").is_some());
        // A name that is not a word is quoted, and cannot go in a list.
        let plain = "requirementDiagram\n    requirement a {\n    }\n    a - traces -> a\n";
        assert_eq!(
            apply(plain, rename(plain, "a", "Top speed")),
            "requirementDiagram\n    requirement \"Top speed\" {\n    }\n    \"Top speed\" - traces -> \"Top speed\"\n"
        );
        assert!(rename(SRC, "req", "Top speed").is_none());
        assert!(rename(SRC, "req", "sys").is_none());
        assert!(rename(SRC, "req", "").is_none());
        assert!(rename(SRC, "nope", "x").is_none());
    }

    #[test]
    fn sets_relationship_types() {
        assert_eq!(relationship_type(SRC, 0).as_deref(), Some("satisfies"));
        let out = apply(SRC, set_relationship_type(SRC, 0, "verifies"));
        assert!(out.contains("sys - verifies -> req"), "{out}");
        assert!(set_relationship_type(SRC, 0, "satisfies").is_none());
        assert!(set_relationship_type(SRC, 0, "bogus").is_none());
        assert!(set_relationship_type(SRC, 5, "copies").is_none());
    }

    #[test]
    fn deletes() {
        assert_eq!(
            apply(SRC, delete(SRC, &["req"], &[])),
            "requirementDiagram\n    element sys {\n        type: system\n    }\n"
        );
        assert_eq!(
            apply(SRC, delete(SRC, &[], &[0])),
            SRC.replace("    sys - satisfies -> req\n", "")
        );
        let listed = "requirementDiagram\n    element a {\n    }\n    element b {\n    }\n    class a,b hot\n";
        assert_eq!(
            apply(listed, delete(listed, &["a"], &[])),
            "requirementDiagram\n    element b {\n    }\n    class b hot\n"
        );
        assert!(delete(SRC, &["nope"], &[]).is_none());
    }
}
