//! Drawing edits to mindmap source (WYSIWY-26): adding, renaming, and deleting nodes. Nodes are
//! named by their index in [`Mindmap::nodes`], as lowering gives them.

use super::{Mindmap, lower};
use crate::edits::{combine, indent, line_start, newline, normalize, span, token};
use crate::flowchart::edit::TextEdit;
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

/// A parsed source, its mindmap, and the statement of each node.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    map: Mindmap,
    statements: Vec<SyntaxNode>,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Doc<'a> {
        let root = super::parse(src).syntax();
        let (map, _, statements) = lower::lower_ranges(&root);
        Doc {
            src,
            root,
            map,
            statements,
        }
    }

    /// The byte after node `i`'s subtree: past its last descendant's line and the `::icon` and
    /// `:::` lines after it.
    fn subtree_end(&self, i: usize) -> usize {
        let last = i + self.map.descendants(i).len();
        let mut end = usize::from(self.statements[last].text_range().end());
        for s in self.root.children() {
            if usize::from(s.text_range().start()) < end {
                continue;
            }
            if !matches!(s.kind(), ICON_STMT | CLASS_SHORTHAND) {
                break;
            }
            end = usize::from(s.text_range().end());
        }
        match self.src[end..].find('\n') {
            Some(n) => end + n + 1,
            None => self.src.len(),
        }
    }
}

/// Source text for a label: lines joined with `<br>`, and `#`, `<`, and `"` as entity codes.
fn text_source(text: &str) -> String {
    normalize(text)
        .replace('#', "#35;")
        .replace('<', "#lt;")
        .replace('"', "#quot;")
        .replace('\n', "<br>")
}

/// Nodes for `text`, to try in order: the text alone when it may read back as written, then
/// quoted in `[...]`.
fn node_sources(text: &str) -> Vec<String> {
    let s = text_source(text);
    let bare = !s.contains(['(', ')', '[', ']', '{', '}'])
        && !s.contains("@{")
        && !s.starts_with(':')
        && !s.contains("<br>");
    let quoted = format!("[\"{s}\"]");
    if bare { vec![s, quoted] } else { vec![quoted] }
}

/// Whether node `i` of `src` reads as `want`, with `count` nodes in all.
fn reads(src: &str, i: usize, want: &str, count: usize) -> bool {
    let (map, _, _) = lower::lower_ranges(&super::parse(src).syntax());
    map.nodes.len() == count && map.nodes.get(i).is_some_and(|n| n.label.text == want)
}

/// The label of node `i`.
pub fn label(src: &str, i: usize) -> Option<String> {
    Some(Doc::new(src).map.nodes.get(i)?.label.text.clone())
}

/// The source of node `i`: its line.
pub fn node_range(src: &str, i: usize) -> Option<std::ops::Range<usize>> {
    Doc::new(src)
        .statements
        .get(i)
        .map(|s| span(s.text_range()))
}

/// Adds a node shown as `text` (`New idea` when empty) as the last child of node `parent`, or
/// as the root of an empty mindmap. Returns its index and the edit; `None` when there is no such
/// parent, or no parent is given and the mindmap has a root.
pub fn add_node(src: &str, parent: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let doc = Doc::new(src);
    let text = if normalize(text).is_empty() {
        "New idea"
    } else {
        text
    };
    let nl = newline(src);
    let want = normalize(text);
    let count = doc.map.nodes.len() + 1;
    let Some(p) = parent else {
        if !doc.map.nodes.is_empty() {
            return None;
        }
        let at = src.len();
        let lead = if src.ends_with('\n') { "" } else { nl };
        return node_sources(text).into_iter().find_map(|line| {
            let edit = TextEdit {
                range: at..at,
                text: format!("{lead}  {line}{nl}"),
            };
            reads(&edit.apply(src), 0, &want, count).then_some((0, edit))
        });
    };
    let node = doc.map.nodes.get(p)?;
    let start = usize::from(doc.statements[p].text_range().start());
    let inner = match node.children.first() {
        Some(&c) => indent(src, usize::from(doc.statements[c].text_range().start())).to_owned(),
        None => {
            let own = indent(src, start);
            let step = if own.contains('\t') { "\t" } else { "  " };
            format!("{own}{step}")
        }
    };
    let at = doc.subtree_end(p);
    let lead = if src[..at].ends_with('\n') { "" } else { nl };
    let i = p + doc.map.descendants(p).len() + 1;
    node_sources(text).into_iter().find_map(|line| {
        let edit = TextEdit {
            range: at..at,
            text: format!("{lead}{inner}{line}{nl}"),
        };
        // The new node must lower under `p` as written, with every other node kept.
        let out = edit.apply(src);
        let (after, _, _) = lower::lower_ranges(&super::parse(&out).syntax());
        let kept =
            after.nodes.get(i).and_then(|n| n.parent) == Some(p) && reads(&out, i, &want, count);
        kept.then_some((i, edit))
    })
}

/// Shows node `i` as `text`, keeping its shape and id. `None` when there is no such node, the
/// text is empty, or nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    if normalize(text).is_empty() {
        return None;
    }
    let doc = Doc::new(src);
    let stmt = doc.statements.get(i)?;
    let want = normalize(text);
    if doc.map.nodes[i].label.text == want {
        return None;
    }
    let (range, sources) = match stmt.children().find(|n| n.kind() == SHAPE) {
        Some(shape) => {
            let content = [MD_STRING, STRING, TEXT]
                .into_iter()
                .find_map(|k| token(&shape, k))?;
            let quoted = format!("\"{}\"", text_source(text));
            (span(content.text_range()), vec![quoted])
        }
        None => (span(token(stmt, IDENT)?.text_range()), node_sources(text)),
    };
    let count = doc.map.nodes.len();
    sources.into_iter().find_map(|text| {
        let edit = TextEdit {
            range: range.clone(),
            text,
        };
        reads(&edit.apply(src), i, &want, count).then_some(edit)
    })
}

/// Removes nodes `nodes`, each with its subtree. `None` when nothing is removed.
pub fn delete(src: &str, nodes: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let edits = nodes
        .iter()
        .filter(|&&i| i < doc.statements.len())
        .map(|&i| {
            let start = line_start(src, usize::from(doc.statements[i].text_range().start()));
            (start..doc.subtree_end(i), String::new())
        })
        .collect();
    combine(src, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "mindmap\n  root((Root))\n    A[Square]\n      a1\n      ::icon(x)\n    B\n";

    fn map(src: &str) -> Mindmap {
        super::super::parse(src).lower().0
    }

    fn labels(src: &str) -> Vec<String> {
        map(src).nodes.into_iter().map(|n| n.label.text).collect()
    }

    #[test]
    fn adds_nodes() {
        let (i, e) = add_node(SRC, Some(1), "two").unwrap();
        let out = e.apply(SRC);
        assert_eq!(
            out,
            "mindmap\n  root((Root))\n    A[Square]\n      a1\n      ::icon(x)\n      two\n    B\n"
        );
        assert_eq!(i, 3);
        assert_eq!(map(&out).nodes[3].parent, Some(1));
        let (i, e) = add_node(SRC, Some(3), "").unwrap();
        let out = e.apply(SRC);
        assert!(out.ends_with("    B\n      New idea\n"), "{out}");
        assert_eq!(labels(&out)[i], "New idea");
        let (i, e) = add_node(SRC, Some(0), "x (y) #1\nz").unwrap();
        let out = e.apply(SRC);
        assert!(
            out.ends_with("    B\n    [\"x (y) #35;1<br>z\"]\n"),
            "{out}"
        );
        assert_eq!(labels(&out)[i], "x (y) #1\nz");
        assert!(add_node(SRC, None, "r").is_none());
        assert!(add_node(SRC, Some(9), "r").is_none());
        let (i, e) = add_node("mindmap\n", None, "").unwrap();
        assert_eq!(i, 0);
        assert_eq!(e.apply("mindmap\n"), "mindmap\n  New idea\n");
        let (_, e) = add_node("mindmap\n  r", Some(0), "c").unwrap();
        assert_eq!(e.apply("mindmap\n  r"), "mindmap\n  r\n    c\n");
    }

    #[test]
    fn labels_nodes() {
        let out = set_label(SRC, 0, "Top \"one\"")
            .map(|e| e.apply(SRC))
            .unwrap();
        assert!(out.contains("root((\"Top #quot;one#quot;\"))"), "{out}");
        assert_eq!(labels(&out)[0], "Top \"one\"");
        let out = set_label(SRC, 2, "a (1)").map(|e| e.apply(SRC)).unwrap();
        assert!(out.contains("      [\"a (1)\"]\n"), "{out}");
        assert_eq!(labels(&out)[2], "a (1)");
        let out = set_label(SRC, 3, "Bee").map(|e| e.apply(SRC)).unwrap();
        assert!(out.ends_with("    Bee\n"), "{out}");
        assert!(set_label(SRC, 3, "B").is_none());
        assert!(set_label(SRC, 3, " ").is_none());
        assert!(set_label(SRC, 9, "x").is_none());
        assert_eq!(label(SRC, 1).as_deref(), Some("Square"));
    }

    #[test]
    fn quotes_text_read_otherwise() {
        for text in ["%% x", "::icon(y)", ":::c"] {
            let out = set_label(SRC, 3, text).unwrap().apply(SRC);
            assert_eq!(labels(&out)[3], text, "{out}");
            let (i, e) = add_node("mindmap\n", None, text).unwrap();
            assert_eq!(labels(&e.apply("mindmap\n"))[i], text);
            let (i, e) = add_node(SRC, Some(3), text).unwrap();
            let out = e.apply(SRC);
            assert_eq!(labels(&out)[i], text, "{out}");
            assert_eq!(map(&out).nodes[i].parent, Some(3));
        }
    }

    #[test]
    fn deletes_nodes() {
        let out = delete(SRC, &[1]).unwrap().apply(SRC);
        assert_eq!(out, "mindmap\n  root((Root))\n    B\n");
        let out = delete(SRC, &[1, 2, 3]).unwrap().apply(SRC);
        assert_eq!(out, "mindmap\n  root((Root))\n");
        assert_eq!(delete(SRC, &[0]).unwrap().apply(SRC), "mindmap\n");
        assert!(delete(SRC, &[9]).is_none());
    }
}
