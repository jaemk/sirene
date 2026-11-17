//! Source edits for changes made on a class diagram's drawing (WYSIWY-23).
//!
//! Classes are named by their node id in the lowered chart (without backticks) and relations by
//! their index among its edges, as [`flowchart::edit`](crate::flowchart::edit) names them. A note
//! on its own is a node too, with an id starting `note:`.

use std::collections::HashSet;
use std::ops::Range;

use super::parser::id_len;
use crate::flowchart::Flowchart;
use crate::flowchart::edit::TextEdit;
use crate::sequence::edit::{indent, span, statement_span};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

/// Why [`add_relation`] refused a relation.
pub const RELATION_REFUSED: &str = "a relation joins two classes, not notes";

/// A parsed source and the chart it lowers to.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    chart: Flowchart,
    newline: &'static str,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Self {
        let parse = super::parse(src);
        Doc {
            src,
            root: parse.syntax(),
            chart: parse.lower().flowchart,
            newline: if src.contains("\r\n") { "\r\n" } else { "\n" },
        }
    }

    /// Every statement without a syntax error, at any depth, in source order.
    fn statements(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
        self.root.descendants().filter(|s| {
            matches!(
                s.kind(),
                CLASS_DECL
                    | MEMBER_STMT
                    | ANNOTATION_STMT
                    | RELATION_STMT
                    | NOTE_STMT
                    | CLICK_STMT
                    | STYLE_STMT
                    | CSSCLASS_STMT
            ) && !s.descendants().any(|n| n.kind() == ERROR)
        })
    }

    /// The statement of relation `edge`.
    fn relation(&self, edge: usize) -> Option<SyntaxNode> {
        let range = self.chart.edges.get(edge)?.range;
        self.root
            .descendants()
            .find(|s| s.kind() == RELATION_STMT && s.text_range() == range)
    }

    /// The statement of note `id` (one on its own, with an id starting `note:`).
    fn note(&self, id: &str) -> Option<SyntaxNode> {
        if !id.starts_with("note:") {
            return None;
        }
        let range = *self.chart.node(id)?.mentions.first()?;
        self.root
            .descendants()
            .find(|s| s.kind() == NOTE_STMT && s.text_range() == range)
    }

    /// Adds `line` at the end of the diagram, indented as its last statement.
    fn append(&self, line: &str) -> TextEdit {
        let src = self.src;
        let indent = self
            .root
            .children()
            .last()
            .map(|s| indent(src, usize::from(s.text_range().start())).to_owned())
            .filter(|i| !i.is_empty())
            .unwrap_or_else(|| "    ".to_owned());
        let sep = if src.is_empty() || src.ends_with('\n') {
            ""
        } else {
            self.newline
        };
        TextEdit {
            range: src.len()..src.len(),
            text: format!("{sep}{indent}{line}{}", self.newline),
        }
    }

    /// A class id not yet in use: `C1`, `C2`, ...
    fn fresh_id(&self) -> String {
        let used: HashSet<&str> = (self.chart.nodes.iter().map(|n| n.id.as_str()))
            .chain(self.chart.subgraphs.iter().map(|s| s.id.as_str()))
            .collect();
        // One of the first `used.len() + 1` is free.
        (1..=used.len() + 1)
            .map(|n| format!("C{n}"))
            .find(|id| !used.contains(id.as_str()))
            .unwrap_or_default()
    }

    /// Whether `id` is a class rather than a note.
    fn is_class(&self, id: &str) -> bool {
        !id.starts_with("note:") && self.chart.node(id).is_some()
    }
}

/// A class id without the backticks around it.
fn unticked(id: &str) -> &str {
    id.strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .unwrap_or(id)
}

/// Class `id` as source: in backticks unless it is a plain id.
fn source_id(id: &str) -> String {
    if !id.is_empty() && id_len(id) == id.len() && !id.starts_with('`') {
        id.to_owned()
    } else {
        format!("`{id}`")
    }
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

/// The class a `CLASS_REF` names.
fn ref_id(r: &SyntaxNode) -> Option<String> {
    token(r, IDENT).map(|t| unticked(t.text()).to_owned())
}

/// The class ids a statement names, besides those in `style` and `cssClass` lists.
fn ids(stmt: &SyntaxNode) -> Vec<String> {
    match stmt.kind() {
        NOTE_STMT => token(stmt, IDENT)
            .map(|t| unticked(t.text()).to_owned())
            .into_iter()
            .collect(),
        CLICK_STMT => {
            let text = token(stmt, TEXT).map(|t| t.text().trim().to_owned());
            let text = text.unwrap_or_default();
            let id = match text.strip_prefix('`') {
                Some(t) => t.split('`').next().unwrap_or(""),
                None => text.split_whitespace().next().unwrap_or(""),
            };
            vec![id.to_owned()]
        }
        _ => stmt
            .children()
            .filter(|n| n.kind() == CLASS_REF)
            .filter_map(|r| ref_id(&r))
            .collect(),
    }
}

/// Label text as the editor gives it: control characters as spaces and each line trimmed.
fn normalize(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Label source for `text`: lines joined with `<br>`, `#` as an entity code.
fn label_source(text: &str) -> String {
    normalize(text).replace('#', "#35;").replace('\n', "<br>")
}

/// A quoted label for `text`.
fn quoted(text: &str) -> String {
    format!("\"{}\"", label_source(text).replace('"', "#quot;"))
}

/// Applies `edits`, which must not overlap, and returns the change as one edit.
fn combine(src: &str, mut edits: Vec<(Range<usize>, String)>) -> Option<TextEdit> {
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for (r, text) in edits {
        if r.start < at {
            continue;
        }
        out.push_str(&src[at..r.start]);
        out.push_str(&text);
        at = r.end;
    }
    out.push_str(&src[at..]);
    TextEdit::between(src, &out)
}

/// The label of class or note `id` as the editor shows it: without a class's annotation.
pub fn label(src: &str, id: &str) -> Option<String> {
    let doc = Doc::new(src);
    let text = doc.chart.node(id)?.label.text.clone();
    let annotated = doc.is_class(id)
        && doc.statements().any(|s| {
            matches!(s.kind(), ANNOTATION_STMT | CLASS_DECL)
                && token(&s, STEREOTYPE).is_some()
                && ids(&s).first().is_some_and(|i| i == id)
        });
    Some(if annotated {
        text.split_once('\n')
            .map_or(text.clone(), |(_, l)| l.to_owned())
    } else {
        text
    })
}

/// Adds a class shown as `label` (just its id when `label` is empty). Returns its id and the
/// edit.
pub fn add_class(src: &str, label: &str) -> (String, TextEdit) {
    let doc = Doc::new(src);
    let id = doc.fresh_id();
    let text = normalize(label);
    let line = if text.is_empty() || text == id {
        format!("class {id}")
    } else {
        format!("class {id}[{}]", quoted(&text))
    };
    (id, doc.append(&line))
}

/// Adds an association (`-->`) from class `from` to class `to`, at the end of the diagram. One
/// at a note, or at a class that does not exist, is refused with `None`.
pub fn add_relation(src: &str, from: &str, to: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    if !doc.is_class(from) || !doc.is_class(to) {
        return None;
    }
    Some(doc.append(&format!("{} --> {}", source_id(from), source_id(to))))
}

/// Shows class `id` as `label`, in the `["..."]` of its first declaration, with any later
/// labels removed; a declaration without `class` gains it. An empty label, or one equal to the
/// id, removes them. A note on its own takes `label` as its text. `None` when there is no such
/// class or nothing changes.
pub fn set_class_label(src: &str, id: &str, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    if let Some(note) = doc.note(id) {
        let string = token(&note, STRING)?;
        let edit = TextEdit {
            range: span(string.text_range()),
            text: quoted(label),
        };
        return (edit.apply(src) != src).then_some(edit);
    }
    if !doc.is_class(id) {
        return None;
    }
    let text = normalize(label);
    let plain = text.is_empty() || text == id;
    let decls: Vec<SyntaxNode> = doc
        .statements()
        .filter(|s| s.kind() == CLASS_DECL && ids(s).first().is_some_and(|i| i == id))
        .collect();
    let labeled: Vec<&SyntaxNode> = decls
        .iter()
        .filter(|s| token(s, STRING).is_some())
        .collect();
    let mut edits = Vec::new();
    for (k, s) in labeled.iter().enumerate() {
        let (open, close) = (token(s, SHAPE_OPEN)?, token(s, SHAPE_CLOSE)?);
        if k == 0 && !plain {
            edits.push((span(token(s, STRING)?.text_range()), quoted(&text)));
        } else {
            let range =
                usize::from(open.text_range().start())..usize::from(close.text_range().end());
            edits.push((range, String::new()));
        }
    }
    if labeled.is_empty() && !plain {
        let line = format!("[{}]", quoted(&text));
        match decls.first() {
            Some(d) => {
                let r = d.children().find(|n| n.kind() == CLASS_REF)?;
                if token(d, CLASS_KW).is_none() {
                    let at = usize::from(r.text_range().start());
                    edits.push((at..at, "class ".to_owned()));
                }
                let at = usize::from(r.text_range().end());
                edits.push((at..at, line));
            }
            None => {
                return Some(doc.append(&format!("class {}{line}", source_id(id))));
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    combine(src, edits)
}

/// Sets the label of relation `edge`; an empty label removes it. `None` when there is no such
/// relation or nothing changes.
pub fn set_relation_label(src: &str, edge: usize, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let stmt = doc.relation(edge)?;
    let text = label_source(label);
    let end = usize::from(stmt.text_range().end());
    let edit = match token(&stmt, COLON) {
        Some(colon) if text.is_empty() => {
            let start = src[..usize::from(colon.text_range().start())]
                .trim_end()
                .len();
            TextEdit {
                range: start..end,
                text: String::new(),
            }
        }
        Some(colon) => TextEdit {
            range: usize::from(colon.text_range().end())..end,
            text: format!(" {text}"),
        },
        None if text.is_empty() => return None,
        None => {
            let to = stmt.children().filter(|n| n.kind() == CLASS_REF).last()?;
            let at = usize::from(to.text_range().end());
            TextEdit {
                range: at..at,
                text: format!(" : {text}"),
            }
        }
    };
    (edit.apply(src) != src).then_some(edit)
}

/// Removes relations `edges` and classes `classes`, with every statement naming one of them
/// (and their ids from `style` and `cssClass` lists). A note on its own is removed by its id.
/// `None` when nothing is removed.
pub fn delete(src: &str, classes: &[&str], edges: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut doomed: Vec<rowan::TextRange> = edges
        .iter()
        .filter_map(|&i| doc.chart.edges.get(i).map(|e| e.range))
        .collect();
    doomed.extend(
        classes
            .iter()
            .filter_map(|id| doc.note(id))
            .map(|n| n.text_range()),
    );
    let named: Vec<&str> = classes
        .iter()
        .copied()
        .filter(|id| doc.is_class(id))
        .collect();
    let mut edits = Vec::new();
    for s in doc.statements() {
        match s.kind() {
            STYLE_STMT => {
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
                    .filter(|id| !named.contains(&unticked(id)))
                    .collect();
                if kept.is_empty() {
                    doomed.push(s.text_range());
                } else if kept.len() < all.len() {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            CSSCLASS_STMT => {
                let Some(string) = token(&s, STRING) else {
                    continue;
                };
                let all: Vec<&str> = crate::flowchart::text::unquote(string.text())
                    .split(',')
                    .map(str::trim)
                    .collect();
                let kept: Vec<&str> = all
                    .iter()
                    .copied()
                    .filter(|id| !named.contains(id))
                    .collect();
                if kept.is_empty() {
                    doomed.push(s.text_range());
                } else if kept.len() < all.len() {
                    edits.push((span(string.text_range()), format!("\"{}\"", kept.join(","))));
                }
            }
            _ if ids(&s).iter().any(|i| named.contains(&i.as_str())) => {
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

    const SRC: &str = "classDiagram\n    Animal <|-- Duck : is\n    class Duck {\n        +swim()\n    }\n    namespace Pond {\n        class Fish\n    }\n";

    #[test]
    fn adds_classes() {
        let (id, e) = add_class(SRC, "New \"one\"");
        assert_eq!(id, "C1");
        let out = e.apply(SRC);
        assert!(
            out.ends_with("    }\n    class C1[\"New #quot;one#quot;\"]\n"),
            "{out}"
        );
        assert_eq!(chart(&out).node("C1").unwrap().label.text, "New \"one\"");
        let one = "classDiagram\n    class C1\n";
        let (id, e) = add_class(one, "");
        assert_eq!(id, "C2");
        assert_eq!(e.apply(one), "classDiagram\n    class C1\n    class C2\n");
    }

    #[test]
    fn adds_relations() {
        let out = apply(SRC, add_relation(SRC, "Fish", "Duck"));
        assert!(out.ends_with("    }\n    Fish --> Duck\n"), "{out}");
        let c = chart(&out);
        assert_eq!(c.edges.len(), 2);
        // The class stays in its namespace.
        let pond = c.subgraphs.iter().find(|s| s.id == "Pond").unwrap();
        assert!(pond.members.iter().any(|m| m == "Fish"));
        // An id that is not plain goes in backticks.
        let ticked = "classDiagram\n    class `A B`\n    C\n";
        let out = apply(ticked, add_relation(ticked, "A B", "C"));
        assert!(out.ends_with("    `A B` --> C\n"), "{out}");
        assert_eq!(chart(&out).edges[0].from, "A B");
        // Notes and unknown classes: refused.
        let note = "classDiagram\n    A\n    note \"n\"\n";
        let n = chart(note).nodes[1].id.clone();
        assert!(add_relation(note, "A", &n).is_none());
        assert!(add_relation(note, "A", "Z").is_none());
    }

    #[test]
    fn labels_classes() {
        let out = apply(SRC, set_class_label(SRC, "Duck", "Mallard #1"));
        assert!(
            out.contains("    class Duck[\"Mallard #35;1\"] {\n"),
            "{out}"
        );
        assert_eq!(chart(&out).node("Duck").unwrap().label.text, "Mallard #1");
        // A bare declaration gains `class`, and the class stays in its namespace.
        let bare = "classDiagram\n    namespace N {\n        A\n    }\n";
        let out = apply(bare, set_class_label(bare, "A", "x"));
        assert_eq!(
            out,
            "classDiagram\n    namespace N {\n        class A[\"x\"]\n    }\n"
        );
        assert!(chart(&out).subgraphs[0].members.iter().any(|m| m == "A"));
        // Only in a relation: a declaration is added.
        let out = apply(SRC, set_class_label(SRC, "Animal", "Beast"));
        assert!(out.ends_with("    class Animal[\"Beast\"]\n"), "{out}");
        // The first label is replaced and later ones removed; an empty label removes all.
        let two = "classDiagram\n    class A[\"one\"]\n    class A[\"two\"]\n";
        assert_eq!(
            apply(two, set_class_label(two, "A", "three")),
            "classDiagram\n    class A[\"three\"]\n    class A\n"
        );
        assert_eq!(
            apply(two, set_class_label(two, "A", "A")),
            "classDiagram\n    class A\n    class A\n"
        );
        // A note's text.
        let note = "classDiagram\n    note \"old\"\n";
        let n = chart(note).nodes[0].id.clone();
        let out = apply(note, set_class_label(note, &n, "new\nline"));
        assert_eq!(out, "classDiagram\n    note \"new<br>line\"\n");
        assert_eq!(chart(&out).nodes[0].label.text, "new\nline");
        assert!(set_class_label(SRC, "Duck", "Duck").is_none());
        assert!(set_class_label(SRC, "Nope", "x").is_none());
    }

    #[test]
    fn editor_labels() {
        let src =
            "classDiagram\n    class A {\n        <<interface>>\n    }\n    <<enum>> B\n    C\n";
        assert_eq!(label(src, "A").as_deref(), Some("A"));
        assert_eq!(label(src, "B").as_deref(), Some("B"));
        assert_eq!(label(src, "C").as_deref(), Some("C"));
        assert!(label(src, "Z").is_none());
    }

    #[test]
    fn labels_relations() {
        assert_eq!(
            apply(SRC, set_relation_label(SRC, 0, "was")),
            SRC.replace(": is", ": was")
        );
        assert_eq!(
            apply(SRC, set_relation_label(SRC, 0, "")),
            SRC.replace(" : is", "")
        );
        let bare = "classDiagram\n    A --> B\n";
        assert_eq!(
            apply(bare, set_relation_label(bare, 0, "uses")),
            "classDiagram\n    A --> B : uses\n"
        );
        assert!(set_relation_label(bare, 0, "").is_none());
        assert!(set_relation_label(bare, 9, "x").is_none());
    }

    #[test]
    fn deletes() {
        // A class takes every statement naming it, and its id from style lists.
        let src = "classDiagram\n    A <|-- B\n    class A {\n        +id\n    }\n    A : +name\n    <<interface>> A\n    note for A \"n\"\n    click A href \"u\"\n    style A,B fill:#f00\n    cssClass \"A\" hot\n    B --> C\n";
        assert_eq!(
            apply(src, delete(src, &["A"], &[])),
            "classDiagram\n    style B fill:#f00\n    B --> C\n"
        );
        // A relation alone, a class in a namespace, and a note on its own.
        assert_eq!(
            apply(src, delete(src, &[], &[1])),
            src.replace("    B --> C\n", "")
        );
        assert_eq!(
            apply(SRC, delete(SRC, &["Fish"], &[])),
            SRC.replace("        class Fish\n", "")
        );
        let note = "classDiagram\n    A\n    note \"n\"\n";
        let n = chart(note).nodes[1].id.clone();
        assert_eq!(
            apply(note, delete(note, &[&n], &[])),
            "classDiagram\n    A\n"
        );
        assert!(delete(src, &["Z"], &[]).is_none());
    }
}
