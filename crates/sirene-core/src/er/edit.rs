//! Source edits for changes made on an ER diagram's drawing (WYSIWY-24).
//!
//! Entities are named by their node id in the lowered chart (a quoted name without its quotes)
//! and relationships by their index among its edges, as
//! [`flowchart::edit`](crate::flowchart::edit) names them.

use super::lower::relationship;
use super::parser::id_len;
use crate::edits::{append, combine, fresh_id, label_source, normalize, span, statement_span};
use crate::flowchart::edit::TextEdit;
use crate::flowchart::{Arrow, Flowchart, Stroke, text};
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// The cardinalities an end can take, for menus: `(title, marker)`.
pub const CARDINALITIES: &[(&str, Arrow)] = &[
    ("Exactly One", Arrow::One),
    ("Zero or One", Arrow::ZeroOne),
    ("One or More", Arrow::OneMany),
    ("Zero or More", Arrow::ZeroMany),
];

/// A parsed source and the chart it lowers to.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    chart: Flowchart,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Self {
        let parse = super::parse(src);
        Doc {
            src,
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

    fn append(&self, line: &str) -> TextEdit {
        append(self.src, self.root.children().last().as_ref(), line)
    }
}

/// An entity reference's name and alias tokens.
fn ref_tokens(r: &SyntaxNode) -> (Option<SyntaxToken>, Option<SyntaxToken>) {
    let mut name = None;
    let mut alias = None;
    let mut in_alias = false;
    for t in r.children_with_tokens().filter_map(|e| e.into_token()) {
        match t.kind() {
            SHAPE_OPEN => in_alias = true,
            SHAPE_CLOSE | TRIPLE_COLON => in_alias = false,
            IDENT | STRING if in_alias => alias = Some(t),
            IDENT | STRING if name.is_none() => name = Some(t),
            _ => {}
        }
    }
    (name, alias)
}

/// The entity an `ENTITY_REF` names.
fn ref_id(r: &SyntaxNode) -> Option<String> {
    let t = ref_tokens(r).0?;
    Some(match t.kind() {
        STRING => text::unquote(t.text()).to_owned(),
        _ => t.text().to_owned(),
    })
}

/// The entity references in a statement.
fn refs(stmt: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    stmt.children().filter(|n| n.kind() == ENTITY_REF)
}

/// Entity `id` as source: quoted unless it is a plain name.
fn source_id(id: &str) -> String {
    if !id.is_empty() && id_len(id) == id.len() {
        id.to_owned()
    } else {
        format!("\"{}\"", id.replace('"', "'"))
    }
}

/// An alias for `text`, which is shown as written: on one line, without `"`.
fn alias(text: &str) -> String {
    format!(
        "\"{}\"",
        normalize(text).replace('\n', " ").replace('"', "'")
    )
}

/// The left-end symbol of `marker`; the right end reads it backwards.
fn symbol(marker: Arrow, left: bool) -> &'static str {
    match (marker, left) {
        (Arrow::ZeroOne, true) => "|o",
        (Arrow::ZeroOne, false) => "o|",
        (Arrow::ZeroMany, true) => "}o",
        (Arrow::ZeroMany, false) => "o{",
        (Arrow::OneMany, true) => "}|",
        (Arrow::OneMany, false) => "|{",
        _ => "||",
    }
}

/// Adds an entity shown as `label` (just its name when `label` is empty). Returns its id and
/// the edit.
pub fn add_entity(src: &str, label: &str) -> (String, TextEdit) {
    let doc = Doc::new(src);
    let id = fresh_id("E", doc.chart.nodes.iter().map(|n| n.id.as_str()));
    let text = normalize(label);
    let line = if text.is_empty() || text == id {
        id.clone()
    } else {
        format!("{id}[{}]", alias(&text))
    };
    (id, doc.append(&line))
}

/// Adds a relationship `FROM ||--o{ TO : ""` (one to zero or more) at the end. `None` when
/// either entity does not exist.
pub fn add_relationship(src: &str, from: &str, to: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    doc.chart.node(from)?;
    doc.chart.node(to)?;
    let line = format!("{} ||--o{{ {} : \"\"", source_id(from), source_id(to));
    Some(doc.append(&line))
}

/// Shows entity `id` as `label`: every alias it has is set, or the first mention gains one. An
/// empty label, or one equal to the name, removes them. `None` when there is no such entity or
/// nothing changes.
pub fn set_entity_label(src: &str, id: &str, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    doc.chart.node(id)?;
    let text = normalize(label);
    let plain = text.is_empty() || text == id;
    let mine: Vec<SyntaxNode> = doc
        .statements()
        .flat_map(|s| refs(&s).collect::<Vec<_>>())
        .filter(|r| ref_id(r).as_deref() == Some(id))
        .collect();
    let mut edits = Vec::new();
    for r in &mine {
        let (Some(name), alias_token) = ref_tokens(r) else {
            continue;
        };
        let Some(alias_token) = alias_token else {
            continue;
        };
        if plain {
            // From the end of the name through `]`.
            let close = r
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == SHAPE_CLOSE)
                .map_or(alias_token.text_range().end(), |t| t.text_range().end());
            let start = usize::from(name.text_range().end());
            edits.push((start..usize::from(close), String::new()));
        } else {
            edits.push((span(alias_token.text_range()), alias(&text)));
        }
    }
    if edits.is_empty() && !plain {
        let name = ref_tokens(mine.first()?).0?;
        let at = usize::from(name.text_range().end());
        edits.push((at..at, format!("[{}]", alias(&text))));
    }
    combine(src, edits)
}

/// Sets the label of relationship `edge`, quoted; an empty label leaves `""`. `None` when there
/// is no such relationship or nothing changes.
pub fn set_relationship_label(src: &str, edge: usize, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let stmt = doc.relationship(edge)?;
    let text = format!("\"{}\"", label_source(label).replace('"', "#quot;"));
    let edit = match crate::edits::token(&stmt, TEXT) {
        Some(t) => TextEdit {
            range: span(t.text_range()),
            text,
        },
        None => {
            let end = usize::from(stmt.text_range().end());
            let end = src[..end].trim_end().len();
            let colon = crate::edits::token(&stmt, COLON).is_some();
            let text = if colon {
                format!(" {text}")
            } else {
                format!(" : {text}")
            };
            TextEdit {
                range: end..end,
                text,
            }
        }
    };
    (edit.apply(src) != src).then_some(edit)
}

/// Rewrites relationship `edge` with `f` applied to its start marker, line, and end marker,
/// written as symbols.
fn set_relationship(
    src: &str,
    edge: usize,
    f: impl FnOnce(Arrow, Stroke, Arrow) -> (Arrow, Stroke, Arrow),
) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let stmt = doc.relationship(edge)?;
    let rel = crate::edits::token(&stmt, RELATION)?;
    let (a, stroke, b) = relationship(rel.text());
    let (a, stroke, b) = f(a, stroke, b);
    let line = if stroke == Stroke::Dotted { ".." } else { "--" };
    let edit = TextEdit {
        range: span(rel.text_range()),
        text: format!("{}{line}{}", symbol(a, true), symbol(b, false)),
    };
    (edit.apply(src) != src).then_some(edit)
}

/// Sets the cardinality of relationship `edge`'s start (or end) to `marker`, one of
/// [`CARDINALITIES`]. `None` when there is no such relationship or nothing changes.
pub fn set_cardinality(src: &str, edge: usize, start: bool, marker: Arrow) -> Option<TextEdit> {
    set_relationship(src, edge, |a, s, b| {
        if start {
            (marker, s, b)
        } else {
            (a, s, marker)
        }
    })
}

/// Makes relationship `edge` identifying (a solid line) or not (a dotted one). `None` when
/// there is no such relationship or nothing changes.
pub fn set_identifying(src: &str, edge: usize, identifying: bool) -> Option<TextEdit> {
    let stroke = if identifying {
        Stroke::Normal
    } else {
        Stroke::Dotted
    };
    set_relationship(src, edge, |a, _, b| (a, stroke, b))
}

/// Removes relationships `edges` and entities `entities`, with every statement naming one of
/// them (and their names from `class` and `style` lists). `None` when nothing is removed.
pub fn delete(src: &str, entities: &[&str], edges: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut doomed: Vec<rowan::TextRange> = edges
        .iter()
        .filter_map(|&i| doc.chart.edges.get(i).map(|e| e.range))
        .collect();
    let mut edits = Vec::new();
    for s in doc.statements() {
        match s.kind() {
            CLASS_STMT | STYLE_STMT => {
                let Some(list) = s.children().find(|n| n.kind() == ID_LIST) else {
                    continue;
                };
                let all: Vec<String> = list
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| t.kind() == IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                let kept: Vec<&str> = all
                    .iter()
                    .map(String::as_str)
                    .filter(|id| !entities.contains(id))
                    .collect();
                if kept.is_empty() {
                    doomed.push(s.text_range());
                } else if kept.len() < all.len() {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            ENTITY | RELATION_STMT
                if refs(&s)
                    .any(|r| ref_id(&r).is_some_and(|id| entities.contains(&id.as_str()))) =>
            {
                doomed.push(s.text_range());
            }
            _ => {}
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

    fn apply(src: &str, e: Option<TextEdit>) -> String {
        e.expect("an edit").apply(src)
    }

    const SRC: &str =
        "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER {\n        int id PK\n    }\n";

    #[test]
    fn adds_entities() {
        let (id, e) = add_entity(SRC, "Line \"item\"");
        assert_eq!(id, "E1");
        let out = e.apply(SRC);
        assert!(out.ends_with("    }\n    E1[\"Line 'item'\"]\n"), "{out}");
        assert_eq!(chart(&out).node("E1").unwrap().label.text, "Line 'item'");
        let (_, e) = add_entity(SRC, "");
        assert!(e.apply(SRC).ends_with("    }\n    E1\n"));
    }

    #[test]
    fn adds_relationships() {
        let out = apply(SRC, add_relationship(SRC, "ORDER", "CUSTOMER"));
        assert!(out.ends_with("    ORDER ||--o{ CUSTOMER : \"\"\n"), "{out}");
        let c = chart(&out);
        assert_eq!(c.edges.len(), 2);
        assert!(c.edges[1].label.is_none());
        let quoted = "erDiagram\n    \"A B\"\n    C\n";
        let out = apply(quoted, add_relationship(quoted, "A B", "C"));
        assert!(out.ends_with("    \"A B\" ||--o{ C : \"\"\n"), "{out}");
        assert_eq!(chart(&out).edges[0].from, "A B");
        assert!(add_relationship(SRC, "ORDER", "Z").is_none());
    }

    #[test]
    fn labels_entities() {
        let out = apply(SRC, set_entity_label(SRC, "ORDER", "Order"));
        assert_eq!(
            out,
            SRC.replace("CUSTOMER ||--o{ ORDER", "CUSTOMER ||--o{ ORDER[\"Order\"]")
        );
        assert_eq!(chart(&out).node("ORDER").unwrap().label.text, "Order");
        let aliased = "erDiagram\n    p[Person] ||--o{ q : x\n    p[\"Who\"]\n";
        assert_eq!(
            apply(aliased, set_entity_label(aliased, "p", "Someone")),
            "erDiagram\n    p[\"Someone\"] ||--o{ q : x\n    p[\"Someone\"]\n"
        );
        assert_eq!(
            apply(aliased, set_entity_label(aliased, "p", "")),
            "erDiagram\n    p ||--o{ q : x\n    p\n"
        );
        assert!(set_entity_label(SRC, "ORDER", "ORDER").is_none());
        assert!(set_entity_label(SRC, "Z", "x").is_none());
    }

    #[test]
    fn labels_relationships() {
        let out = apply(SRC, set_relationship_label(SRC, 0, "buys #1"));
        assert!(out.contains("ORDER : \"buys #35;1\"\n"), "{out}");
        assert_eq!(chart(&out).edges[0].label.as_ref().unwrap().text, "buys #1");
        let bare = "erDiagram\n    A ||--o{ B\n";
        assert_eq!(
            apply(bare, set_relationship_label(bare, 0, "has")),
            "erDiagram\n    A ||--o{ B : \"has\"\n"
        );
        assert!(set_relationship_label(SRC, 9, "x").is_none());
    }

    #[test]
    fn sets_cardinalities_and_lines() {
        let out = apply(SRC, set_cardinality(SRC, 0, false, Arrow::OneMany));
        assert!(out.contains("CUSTOMER ||--|{ ORDER"), "{out}");
        assert_eq!(chart(&out).edges[0].end, Some(Arrow::OneMany));
        let out = apply(SRC, set_cardinality(SRC, 0, true, Arrow::ZeroOne));
        assert!(out.contains("CUSTOMER |o--o{ ORDER"), "{out}");
        let out = apply(SRC, set_identifying(SRC, 0, false));
        assert!(out.contains("CUSTOMER ||..o{ ORDER"), "{out}");
        assert_eq!(chart(&out).edges[0].stroke, Stroke::Dotted);
        // Words become symbols.
        let words = "erDiagram\n    A one or more to zero or many B : x\n";
        assert_eq!(
            apply(words, set_cardinality(words, 0, true, Arrow::One)),
            "erDiagram\n    A ||--o{ B : x\n"
        );
        assert!(set_identifying(SRC, 0, true).is_none());
        assert!(set_cardinality(SRC, 0, true, Arrow::One).is_none());
    }

    #[test]
    fn deletes() {
        let src = "erDiagram\n    A ||--o{ B : x\n    A {\n        int id\n    }\n    B }|..|{ C : y\n    class A,B hot\n    style A fill:#f00\n";
        assert_eq!(
            apply(src, delete(src, &["A"], &[])),
            "erDiagram\n    B }|..|{ C : y\n    class B hot\n"
        );
        assert_eq!(
            apply(src, delete(src, &[], &[1])),
            src.replace("    B }|..|{ C : y\n", "")
        );
        assert!(delete(src, &["Z"], &[]).is_none());
    }
}
