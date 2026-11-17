//! Source edits for changes made on a class diagram's drawing (WYSIWY-23).
//!
//! Classes are named by their node id in the lowered chart (without backticks) and relations by
//! their index among its edges, as [`flowchart::edit`](crate::flowchart::edit) names them. A note
//! on its own is a node too, with an id starting `note:`.

use super::parser::id_len;
use crate::edits::{
    append, combine, fresh_id, indent, label_source, line_start, newline, normalize, quoted, span,
    statement_span, token,
};
use crate::flowchart::Flowchart;
use crate::flowchart::edit::TextEdit;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// Why [`add_relation`] refused a relation.
pub const RELATION_REFUSED: &str = "a relation joins two classes, not notes";

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
        append(self.src, self.root.children().last().as_ref(), line)
    }

    /// A class id not yet in use: `C1`, `C2`, ...
    fn fresh_id(&self) -> String {
        let chart = &self.chart;
        let nodes = chart.nodes.iter().map(|n| n.id.as_str());
        fresh_id(
            "C",
            nodes.chain(chart.subgraphs.iter().map(|s| s.id.as_str())),
        )
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

/// The member tokens of class `id`, in the order its box shows them.
fn member_tokens(doc: &Doc, id: &str) -> Vec<SyntaxToken> {
    doc.statements()
        .filter(|s| {
            matches!(s.kind(), CLASS_DECL | MEMBER_STMT) && ids(s).first().is_some_and(|i| i == id)
        })
        .flat_map(|s| {
            s.children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| t.kind() == MEMBER)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The members of class `id` as written, in the order its box shows them.
pub fn members(src: &str, id: &str) -> Vec<String> {
    let doc = Doc::new(src);
    member_tokens(&doc, id)
        .iter()
        .map(|t| t.text().trim().to_owned())
        .collect()
}

/// Adds member `text` (an attribute, or a method with `(...)`) to class `id`: on a new line
/// before the `}` of its last body when that is on its own line, otherwise as `ID : text` at the
/// end. `None` for an empty member or one that is not a class.
pub fn add_member(src: &str, id: &str, text: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let text = normalize(text).replace('\n', " ");
    if text.is_empty() || !doc.is_class(id) {
        return None;
    }
    let body = doc
        .statements()
        .filter(|s| s.kind() == CLASS_DECL && ids(s).first().is_some_and(|i| i == id))
        .filter_map(|s| token(&s, R_BRACE).map(|close| (s, close)))
        .last();
    // An edit counts only when the class gains exactly that member (a line such as `%%` reads
    // as a comment instead).
    let count = members(src, id).len();
    let adds = |e: &TextEdit| {
        let after = members(&e.apply(src), id);
        after.len() == count + 1 && after.contains(&text)
    };
    // In a body, `}` would close it and `<<` start an annotation.
    let fits = !text.contains('}') && !text.starts_with("<<");
    if let Some((decl, close)) = body.filter(|_| fits) {
        let close = usize::from(close.text_range().start());
        let at = line_start(src, close);
        if src[at..close].trim().is_empty() {
            let indent = match member_tokens(&doc, id)
                .iter()
                .find(|t| t.parent().as_ref() == Some(&decl))
            {
                Some(m) => indent(src, usize::from(m.text_range().start())).to_owned(),
                None => format!("{}    ", indent(src, close)),
            };
            let edit = TextEdit {
                range: at..at,
                text: format!("{indent}{text}{}", newline(src)),
            };
            if adds(&edit) {
                return Some(edit);
            }
        }
    }
    Some(doc.append(&format!("{} : {text}", source_id(id)))).filter(adds)
}

/// Removes member `n` (in the order [`members`] gives) of class `id`. `None` when there is no
/// such member.
pub fn remove_member(src: &str, id: &str, n: usize) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let member = member_tokens(&doc, id).into_iter().nth(n)?;
    let parent = member.parent()?;
    if parent.kind() != MEMBER_STMT {
        return Some(TextEdit {
            range: statement_span(src, member.text_range()),
            text: String::new(),
        });
    }
    // Removing the only statement naming the class would remove the class.
    let elsewhere = doc
        .statements()
        .any(|s| s != parent && ids(&s).iter().any(|i| i == id));
    Some(if elsewhere {
        TextEdit {
            range: statement_span(src, parent.text_range()),
            text: String::new(),
        }
    } else {
        TextEdit {
            range: span(parent.text_range()),
            text: format!("class {}", source_id(id)),
        }
    })
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

/// The relation kinds, for menus: `(title, line, marker)`. A relation is written with its marker
/// on the side it had (the right when it had none), e.g. `--|>` or `<|--`.
pub const RELATION_KINDS: &[(&str, &str, &str)] = &[
    ("Association", "--", ">"),
    ("Inheritance", "--", "|>"),
    ("Composition", "--", "*"),
    ("Aggregation", "--", "o"),
    ("Dependency", "..", ">"),
    ("Realization", "..", "|>"),
    ("Link", "--", ""),
    ("Dashed Link", "..", ""),
];

/// A relation token's parts: left end, line, right end, with `<` and `<|` as `>` and `|>`.
fn relation_parts(rel: &str) -> Option<(&str, &str, &str)> {
    let at = rel.find(['-', '.'])?;
    let (left, rest) = rel.split_at(at);
    let (line, right) = rest.split_at(2.min(rest.len()));
    // `*`, `o`, and `()` read the same from either side.
    let left = match left {
        "<" => ">",
        "<|" => "|>",
        other => other,
    };
    Some((left, line, right))
}

/// The kind of relation `edge`, as a title in [`RELATION_KINDS`]. `None` when there is no such
/// relation or it has markers at both ends or one no kind has.
pub fn relation_kind(src: &str, edge: usize) -> Option<&'static str> {
    let doc = Doc::new(src);
    let rel = token(&doc.relation(edge)?, RELATION)?;
    let (left, line, right) = relation_parts(rel.text())?;
    let marker = match (left, right) {
        (m, "") | ("", m) => m,
        _ => return None,
    };
    RELATION_KINDS
        .iter()
        .find(|(_, l, m)| *l == line && *m == marker)
        .map(|(title, _, _)| *title)
}

/// Sets the kind of relation `edge` to `kind`, a title in [`RELATION_KINDS`], keeping the side
/// its marker is on. `None` when there is no such relation or kind, or nothing changes.
pub fn set_relation_kind(src: &str, edge: usize, kind: &str) -> Option<TextEdit> {
    let &(_, line, marker) = RELATION_KINDS.iter().find(|(t, _, _)| *t == kind)?;
    let doc = Doc::new(src);
    let rel = token(&doc.relation(edge)?, RELATION)?;
    let (left, _, right) = relation_parts(rel.text())?;
    let text = if !left.is_empty() && right.is_empty() {
        let marker = match marker {
            ">" => "<",
            "|>" => "<|",
            m => m,
        };
        format!("{marker}{line}")
    } else {
        format!("{line}{marker}")
    };
    // `o` at the right is a marker only with whitespace or a quote after it.
    let after = &src[usize::from(rel.text_range().end())..];
    let touching = after.starts_with(|c: char| !c.is_whitespace() && c != '"');
    let text = if text.ends_with('o') && touching {
        format!("{text} ")
    } else {
        text
    };
    let edit = TextEdit {
        range: span(rel.text_range()),
        text,
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
    fn members_of_classes() {
        assert_eq!(members(SRC, "Duck"), ["+swim()"]);
        // Into the body, at its members' indent.
        let out = apply(SRC, add_member(SRC, "Duck", "+int age"));
        assert!(
            out.contains("class Duck {\n        +swim()\n        +int age\n    }\n"),
            "{out}"
        );
        assert_eq!(members(&out, "Duck"), ["+swim()", "+int age"]);
        assert_eq!(chart(&out).node("Duck").unwrap().sections.len(), 2);
        // A class without a body gets `ID : member`; so does a member with `}`.
        let out = apply(SRC, add_member(SRC, "Fish", "+fin()"));
        assert!(out.ends_with("    Fish : +fin()\n"), "{out}");
        let out = apply(SRC, add_member(SRC, "Duck", "+map() Map{}"));
        assert!(out.ends_with("    Duck : +map() Map{}\n"), "{out}");
        // An empty body.
        let empty = "classDiagram\n    class A {\n    }\n";
        assert_eq!(
            apply(empty, add_member(empty, "A", "+x")),
            "classDiagram\n    class A {\n        +x\n    }\n"
        );
        // `%%` starts a comment in a body; as `A : %%` it is a member.
        let out = apply(empty, add_member(empty, "A", "%%"));
        assert_eq!(members(&out, "A"), ["%%"], "{out}");
        assert!(add_member(SRC, "Duck", "  ").is_none());
        assert!(add_member(SRC, "Nope", "+x").is_none());

        // Removing one in a body, and one in a member statement.
        let two = "classDiagram\n    class A {\n        +x\n    }\n    A : +y()\n";
        assert_eq!(members(two, "A"), ["+x", "+y()"]);
        assert_eq!(
            apply(two, remove_member(two, "A", 0)),
            "classDiagram\n    class A {\n    }\n    A : +y()\n"
        );
        assert_eq!(
            apply(two, remove_member(two, "A", 1)),
            "classDiagram\n    class A {\n        +x\n    }\n"
        );
        assert!(remove_member(two, "A", 2).is_none());
        // A class named only by its member keeps a declaration.
        let only = "classDiagram\n    A : +x\n";
        assert_eq!(
            apply(only, remove_member(only, "A", 0)),
            "classDiagram\n    class A\n"
        );
    }

    #[test]
    fn sets_relation_kinds() {
        assert_eq!(relation_kind(SRC, 0), Some("Inheritance"));
        // The marker stays on its side.
        let out = apply(SRC, set_relation_kind(SRC, 0, "Composition"));
        assert!(out.contains("Animal *-- Duck : is"), "{out}");
        assert_eq!(relation_kind(&out, 0), Some("Composition"));
        let out = apply(SRC, set_relation_kind(SRC, 0, "Dependency"));
        assert!(out.contains("Animal <.. Duck : is"), "{out}");
        let out = apply(SRC, set_relation_kind(SRC, 0, "Dashed Link"));
        assert!(out.contains("Animal .. Duck : is"), "{out}");
        // With no marker, or one on the right, it goes on the right.
        let plain = "classDiagram\n    A -- B\n    A-->B\n";
        let out = apply(plain, set_relation_kind(plain, 0, "Realization"));
        assert!(out.contains("A ..|> B"), "{out}");
        assert_eq!(relation_kind(&out, 0), Some("Realization"));
        // `o` touching the next class gets a space, so it stays a marker.
        let out = apply(plain, set_relation_kind(plain, 1, "Aggregation"));
        assert!(out.contains("A--o B"), "{out}");
        assert_eq!(relation_kind(&out, 1), Some("Aggregation"));
        assert!(set_relation_kind(SRC, 0, "Inheritance").is_none());
        assert!(set_relation_kind(SRC, 0, "Bogus").is_none());
        assert!(set_relation_kind(SRC, 9, "Link").is_none());
        let both = "classDiagram\n    A <|--|> B\n";
        assert_eq!(relation_kind(both, 0), None);
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
