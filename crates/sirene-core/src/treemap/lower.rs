//! Builds a treemap's items from the indentation of its lines (MTREE-2).

use std::collections::HashMap;

use super::{Format, Item, Treemap};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Style, text};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Treemap, Vec<Diagnostic>) {
    let src = root.to_string();
    let mut t = Treemap::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
            for (k, v) in text::frontmatter_config(tok.text(), "treemap") {
                if config(&mut t, &k, &v).is_none() {
                    diagnostics.push(Diagnostic::warning(
                        tok.text_range(),
                        format!("ignoring `{k}: {v}`: not a valid value"),
                    ));
                }
            }
        }
    }

    // Classes first, so an item may use one defined below it.
    let mut classes: HashMap<String, Vec<Style>> = HashMap::new();
    for stmt in root.children().filter(|s| s.kind() == CLASSDEF_STMT) {
        let (styles, _) = text::styles(&text_of(&stmt, STYLE_TEXT).unwrap_or_default());
        for name in stmt
            .children()
            .filter(|n| n.kind() == ID_LIST)
            .flat_map(|l| tokens(&l).filter(|t| t.kind() == IDENT).collect::<Vec<_>>())
        {
            classes
                .entry(name.text().to_owned())
                .or_default()
                .extend(styles.iter().cloned());
        }
    }

    // The open sections, each with its indentation.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    // Each item's line.
    let mut ranges = Vec::new();
    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            ACC_TITLE_STMT => t.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => t.acc_descr = text_of(&stmt, ACC_TEXT),
            NODE => {
                let Some(name) = token(&stmt, STRING) else {
                    continue;
                };
                let name = name.text()[1..name.text().len() - 1].to_owned();
                let value = match token(&stmt, NUMBER) {
                    Some(n) => match n.text().replace(',', "").parse::<f64>() {
                        Ok(v) if v.is_finite() => Some(v),
                        _ => {
                            diagnostics.push(Diagnostic::new(
                                n.text_range(),
                                format!("`{}` is not a number; ignoring `{name}`", n.text()),
                            ));
                            continue;
                        }
                    },
                    None if stmt.children().any(|c| c.kind() == ERROR) => continue,
                    None => None,
                };
                let class = stmt
                    .children()
                    .find(|c| c.kind() == CLASS_SHORTHAND)
                    .and_then(|c| token(&c, IDENT))
                    .map(|c| c.text().to_owned());
                let styles = match &class {
                    Some(c) => match classes.get(c) {
                        Some(s) => s.clone(),
                        None => {
                            diagnostics.push(Diagnostic::warning(
                                stmt.text_range(),
                                format!("class `{c}` has no `classDef`; ignoring it"),
                            ));
                            Vec::new()
                        }
                    },
                    None => Vec::new(),
                };
                let start = usize::from(stmt.text_range().start());
                let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
                let level = src[line_start..start].chars().count();
                while stack.last().is_some_and(|&(_, l)| l >= level) {
                    stack.pop();
                }
                let i = t.items.len();
                let parent = stack.last().map(|&(p, _)| p);
                if let Some(p) = parent {
                    t.items[p].children.push(i);
                }
                if value.is_none() {
                    stack.push((i, level));
                }
                ranges.push(stmt.text_range());
                t.items.push(Item {
                    name,
                    value,
                    class,
                    styles,
                    parent,
                    children: Vec::new(),
                });
            }
            _ => {}
        }
    }
    t.title = title;

    for (item, range) in t.items.iter().zip(ranges) {
        if item.is_section() && item.children.is_empty() {
            let name = &item.name;
            diagnostics.push(Diagnostic::warning(
                range,
                format!(
                    "section `{name}` holds no items, so it is not drawn; give it a value such \
                     as `\"{name}\": 1`, or indent items under it"
                ),
            ));
        }
    }
    (t, diagnostics)
}

/// Applies one `config.treemap` key, or returns `None` for a bad value. Unknown keys are kept
/// quiet.
fn config(t: &mut Treemap, k: &str, v: &str) -> Option<()> {
    let n = v.parse::<f32>().ok().filter(|n| n.is_finite());
    match k {
        "padding" => t.padding = n.filter(|&n| n >= 0.0)?,
        "diagramPadding" => t.diagram_padding = n.filter(|&n| n >= 0.0)?,
        "showValues" => t.show_values = v.parse().ok()?,
        "nodeWidth" => t.width = n.filter(|&n| n > 0.0)? * 10.0,
        "nodeHeight" => t.height = n.filter(|&n| n > 0.0)? * 10.0,
        "valueFormat" => t.value_format = Format::parse(v)?,
        _ => {}
    }
    Some(())
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}
