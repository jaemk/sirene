//! Builds a kanban board's columns and cards from the indentation of its node lines (MKAN-2).

use super::{Card, Column, Kanban, Priority};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::mindmap;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

/// What `::icon` and `:::` lines apply to.
#[derive(Clone, Copy)]
enum Last {
    Column(usize),
    Card(usize, usize),
}

pub(crate) fn lower(root: &SyntaxNode) -> (Kanban, Vec<Diagnostic>) {
    let src = root.to_string();
    let mut board = Kanban::default();
    let mut diagnostics = Vec::new();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            board.title = text::frontmatter_title(tok.text());
            for (k, v) in text::frontmatter_config(tok.text(), "kanban") {
                if k == "ticketBaseUrl" && !v.is_empty() {
                    board.ticket_base_url = Some(v);
                }
            }
        }
    }
    // The first node's indentation, which every column has.
    let mut column_level: Option<usize> = None;
    let mut last: Option<Last> = None;
    for stmt in root.children() {
        match stmt.kind() {
            NODE => {
                let start = usize::from(stmt.text_range().start());
                let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
                let level = src[line_start..start].chars().count();
                let node = mindmap::lower::node(&stmt);
                let meta = metadata(&stmt, &mut diagnostics);
                let at = *column_level.get_or_insert(level);
                if level == at {
                    let mut column = Column {
                        id: node.id,
                        label: node.label,
                        ..Column::default()
                    };
                    for (k, v, range) in meta {
                        match k.as_str() {
                            "label" => column.label = text::label(&v, false),
                            _ => diagnostics.push(Diagnostic::warning(
                                range,
                                format!("ignoring `{k}`: a column's metadata is `label`"),
                            )),
                        }
                    }
                    board.columns.push(column);
                    last = Some(Last::Column(board.columns.len() - 1));
                } else if level > at {
                    let mut card = Card {
                        id: node.id,
                        label: node.label,
                        ..Card::default()
                    };
                    for (k, v, range) in meta {
                        match k.as_str() {
                            "label" => card.label = text::label(&v, false),
                            "assigned" => card.assigned = Some(v),
                            "ticket" => card.ticket = Some(v),
                            "icon" => {
                                diagnostics.push(icon_warning(range, &v));
                                card.icon = Some(v);
                            }
                            "priority" => match Priority::of(&v) {
                                Some(p) => card.priority = Some(p),
                                None => diagnostics.push(Diagnostic::warning(
                                    range,
                                    format!(
                                        "ignoring priority `{v}`: expected `Very High`, `High`, \
                                         `Low`, or `Very Low`"
                                    ),
                                )),
                            },
                            _ => diagnostics.push(Diagnostic::warning(
                                range,
                                format!(
                                    "ignoring `{k}`: a card's metadata is `assigned`, `ticket`, \
                                     `priority`, `label`, and `icon`"
                                ),
                            )),
                        }
                    }
                    let c = board.columns.len() - 1;
                    board.columns[c].cards.push(card);
                    last = Some(Last::Card(c, board.columns[c].cards.len() - 1));
                } else {
                    diagnostics.push(Diagnostic::new(
                        stmt.text_range(),
                        format!(
                            "`{}` is indented less than the first column; indent columns alike \
                             and their cards under them",
                            node.label.text.replace('\n', " ")
                        ),
                    ));
                    last = None;
                }
            }
            ICON_STMT | CLASS_SHORTHAND => {
                let Some(value) = text_of(&stmt, TEXT).filter(|t| !t.is_empty()) else {
                    continue;
                };
                let icon = stmt.kind() == ICON_STMT;
                if board.columns.is_empty() {
                    let what = if icon { "`::icon(...)`" } else { "`:::`" };
                    diagnostics.push(Diagnostic::new(
                        stmt.text_range(),
                        format!("{what} must follow the column or card it applies to"),
                    ));
                    continue;
                }
                if icon {
                    diagnostics.push(icon_warning(stmt.text_range(), &value));
                }
                let classes = value.split_whitespace().map(str::to_owned);
                match last {
                    Some(Last::Column(c)) => {
                        let column = &mut board.columns[c];
                        if icon {
                            column.icon = Some(value);
                        } else {
                            column.classes.extend(classes);
                        }
                    }
                    Some(Last::Card(c, k)) => {
                        let card = &mut board.columns[c].cards[k];
                        if icon {
                            card.icon = Some(value);
                        } else {
                            card.classes.extend(classes);
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
    (board, diagnostics)
}

fn icon_warning(range: rowan::TextRange, name: &str) -> Diagnostic {
    Diagnostic::warning(range, format!("icons are not drawn; ignoring `{name}`"))
}

/// A node's `@{ ... }` entries with the metadata's range; entries without `:` are warnings.
fn metadata(
    stmt: &SyntaxNode,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, String, rowan::TextRange)> {
    // An unclosed `@{` is already an error; its text is not read.
    let Some(m) = stmt
        .children()
        .find(|n| n.kind() == METADATA)
        .filter(|m| token(m, META_CLOSE).is_some())
    else {
        return Vec::new();
    };
    let body = text_of(&m, META_BODY).unwrap_or_default();
    let (entries, invalid) = text::metadata(&body);
    for bad in invalid {
        diagnostics.push(Diagnostic::warning(
            m.text_range(),
            format!("ignoring `{bad}`: expected `key: value`"),
        ));
    }
    entries
        .into_iter()
        .map(|(k, v)| (k, v, m.text_range()))
        .collect()
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}
