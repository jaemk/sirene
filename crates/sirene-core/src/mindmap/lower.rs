//! Builds a mindmap's tree from the indentation of its node lines (MMIND-2).

use super::{Mindmap, Node, Shape};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Mindmap, Vec<Diagnostic>) {
    let src = root.to_string();
    let mut map = Mindmap::default();
    let mut diagnostics = Vec::new();
    // Each node's indentation.
    let mut levels: Vec<usize> = Vec::new();
    // The node `::icon` and `:::` lines apply to; `None` after a node that was left out.
    let mut last: Option<usize> = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            map.title = text::frontmatter_title(tok.text());
        }
    }
    for stmt in root.children() {
        match stmt.kind() {
            NODE => {
                let start = usize::from(stmt.text_range().start());
                let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
                let level = src[line_start..start].chars().count();
                let mut node = node(&stmt);
                if !map.nodes.is_empty() {
                    let Some(parent) = (0..map.nodes.len()).rev().find(|&j| levels[j] < level)
                    else {
                        diagnostics.push(Diagnostic::new(
                            stmt.text_range(),
                            format!(
                                "`{}` has no parent: a mindmap has one root, so indent this \
                                 line under a node above it",
                                node.label.text.replace('\n', " ")
                            ),
                        ));
                        last = None;
                        continue;
                    };
                    let p = &map.nodes[parent];
                    node.parent = Some(parent);
                    node.depth = p.depth + 1;
                    node.section = p.section.or(Some(p.children.len()));
                }
                let i = map.nodes.len();
                if let Some(p) = node.parent {
                    map.nodes[p].children.push(i);
                }
                map.nodes.push(node);
                levels.push(level);
                last = Some(i);
            }
            ICON_STMT | CLASS_SHORTHAND => {
                let Some(value) = text_of(&stmt, TEXT).filter(|t| !t.is_empty()) else {
                    continue;
                };
                let what = if stmt.kind() == ICON_STMT {
                    "`::icon(...)`"
                } else {
                    "`:::`"
                };
                if map.nodes.is_empty() {
                    diagnostics.push(Diagnostic::new(
                        stmt.text_range(),
                        format!("{what} must follow the node it applies to"),
                    ));
                    continue;
                }
                let Some(i) = last else { continue };
                let node = &mut map.nodes[i];
                if stmt.kind() == ICON_STMT {
                    diagnostics.push(Diagnostic::warning(
                        stmt.text_range(),
                        format!("icons are not drawn; ignoring `{value}`"),
                    ));
                    node.icon = Some(value);
                } else {
                    node.classes
                        .extend(value.split_whitespace().map(str::to_owned));
                }
            }
            _ => {}
        }
    }
    (map, diagnostics)
}

/// A node line's id, shape, and label.
fn node(stmt: &SyntaxNode) -> Node {
    let id = text_of(stmt, IDENT).filter(|s| !s.is_empty());
    let shape_node = stmt.children().find(|n| n.kind() == SHAPE);
    let shape = shape_node
        .as_ref()
        .and_then(|s| token(s, SHAPE_OPEN))
        .map_or(Shape::Default, |t| Shape::of(t.text()));
    let label = shape_node.as_ref().and_then(|s| {
        s.children_with_tokens()
            .filter_map(|e| e.into_token())
            .find_map(|t| match t.kind() {
                MD_STRING => Some(text::label(text::unquote(t.text()), true)),
                STRING => Some(text::label(text::unquote(t.text()), false)),
                TEXT => Some(text::label(t.text(), false)),
                _ => None,
            })
    });
    let label = match label {
        Some(l) if !l.text.is_empty() => l,
        _ => text::label(id.as_deref().unwrap_or(""), false),
    };
    Node {
        id,
        label,
        shape,
        icon: None,
        classes: Vec::new(),
        parent: None,
        children: Vec::new(),
        depth: 0,
        section: None,
    }
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}
