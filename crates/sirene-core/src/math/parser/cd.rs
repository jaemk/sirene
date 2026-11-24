//! The `{CD}` environment: commutative diagrams, in the syntax of the amscd package.
//!
//! Adapted from KaTeX's `src/environments/cd.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and
//! other contributors). The body is read as rows of nodes, as `\\` ends them. Everything that is
//! not an arrow becomes a cell; `@>>>`, `@<<<`, `@VVV` and `@AAA` (each with two optional labels
//! between their three marks), `@=`, `@|` and `@.` become arrows. The result is the same table
//! KaTeX builds: even rows alternate cells and horizontal arrows, odd rows are the vertical arrows
//! with an empty cell between each, and a last empty row follows them all.

use std::ops::Range;

use super::{MathError, Mode, Name, Node, NodeKind, Parser, environments::symbol_text, token_text};
use crate::math::{
    ast::{AlignSpec, Array, CdSide, ColSeparation, MathClass, StyleName},
    symbols::AtomGroup,
};

/// An empty cell, which `{CD}` always sets in display style.
fn new_cell<'a>(mode: Mode, span: &Range<usize>) -> Node<'a> {
    Node {
        kind: NodeKind::Styling {
            style: StyleName::Display,
            body: Vec::new(),
            reset_font: true,
        },
        mode,
        span: span.clone(),
    }
}

/// Whether `node` is the `@` that starts an arrow.
fn is_start_of_arrow(node: &Node<'_>) -> bool {
    matches!(
        node.kind,
        NodeKind::Symbol {
            group: AtomGroup::TextOrd,
            name: Name::Char("@"),
        }
    )
}

/// Whether `node` is the mark `end` that closes an arrow's label (an `atom` or a `mathord`, as
/// KaTeX's types go).
fn is_label_end(node: &Node<'_>, end: &str) -> bool {
    matches!(
        node.kind,
        NodeKind::Symbol {
            group: AtomGroup::MathOrd
                | AtomGroup::Bin
                | AtomGroup::Close
                | AtomGroup::Inner
                | AtomGroup::Open
                | AtomGroup::Punct
                | AtomGroup::Rel,
            name: Name::Char(text),
        } if text == end
    )
}

fn group_of<'a>(body: Vec<Node<'a>>, mode: Mode, span: Range<usize>) -> Node<'a> {
    Node {
        kind: NodeKind::Group {
            body,
            semisimple: false,
        },
        mode,
        span,
    }
}

/// `\Big` and a delimiter symbol: the arrow of a vertical arrow or a double bar.
fn big_delimiter<'a>(delim: &str, mode: Mode, span: Range<usize>) -> Node<'a> {
    Node {
        kind: NodeKind::DelimSizing {
            size: 2,
            class: MathClass::Ord,
            delim: delim.to_string(),
        },
        mode,
        span,
    }
}

/// The arrow `arrow_char` makes, with its labels, as the function call KaTeX makes for it.
fn arrow<'a>(
    arrow_char: &str,
    labels: [Vec<Node<'a>>; 2],
    mode: Mode,
    span: Range<usize>,
) -> Node<'a> {
    let [first, second] = labels;
    let first = group_of(first, mode, span.clone());
    let second = group_of(second, mode, span.clone());
    match arrow_char {
        ">" | "<" => Node {
            kind: NodeKind::XArrow {
                label: if arrow_char == ">" {
                    "\\\\cdrightarrow"
                } else {
                    "\\\\cdleftarrow"
                }
                .to_string(),
                body: Box::new(first),
                below: Some(Box::new(second)),
            },
            mode,
            span,
        },
        "A" | "V" => {
            let side = |side, label| Node {
                kind: NodeKind::CdLabel {
                    side,
                    label: Box::new(label),
                },
                mode,
                span: span.clone(),
            };
            let delim = if arrow_char == "A" {
                "\\uparrow"
            } else {
                "\\downarrow"
            };
            let fragment = group_of(
                vec![
                    side(CdSide::Left, first),
                    big_delimiter(delim, mode, span.clone()),
                    side(CdSide::Right, second),
                ],
                mode,
                span.clone(),
            );
            Node {
                kind: NodeKind::CdLabelParent {
                    fragment: Box::new(fragment),
                },
                mode,
                span,
            }
        }
        // The long equals sign takes no arguments; its body is empty here.
        "=" => Node {
            kind: NodeKind::XArrow {
                label: "\\\\cdlongequal".to_string(),
                body: Box::new(group_of(Vec::new(), mode, span.clone())),
                below: None,
            },
            mode,
            span,
        },
        "|" => big_delimiter("\\Vert", mode, span),
        // No arrow: a space.
        _ => Node {
            kind: NodeKind::Symbol {
                group: AtomGroup::TextOrd,
                name: Name::Char(" "),
            },
            mode,
            span,
        },
    }
}

impl<'a> Parser<'a> {
    /// The table of a `{CD}` environment, with the cursor just after `\begin{CD}`.
    pub(super) fn parse_cd(&mut self, name_span: &Range<usize>) -> Result<Node<'a>, MathError> {
        let mode = self.mode;
        let at = self.fetch()?.span;
        // `\\` ends a row, and `\cr` is `\\`.
        self.expander.begin_group(&at)?;
        self.expander.set_text_macro("cr", "\\\\\\relax", false)?;
        self.expander.begin_group(&at)?;
        let outer_color = self.current_color.clone();

        let mut parsed_rows: Vec<Vec<Node<'a>>> = Vec::new();
        loop {
            let nodes = self.parse_expression_until(false, Some("\\\\"))?;
            parsed_rows.push(nodes);
            self.expander.end_group(&at)?;
            self.expander.begin_group(&at)?;
            self.current_color = outer_color.clone();
            let token = self.fetch()?;
            match token_text(&token.kind).as_str() {
                // A tab character ends a row here too.
                "&" | "\\\\" => self.consume(),
                "\\end" => {
                    // The last row, if it is empty, was ended by a `\\`.
                    if parsed_rows.last().is_some_and(Vec::is_empty) {
                        parsed_rows.pop();
                    }
                    break;
                }
                _ => {
                    return Err(MathError::new("Expected \\\\ or \\cr or \\end", token.span));
                }
            }
        }
        // The row's group, then the table's.
        self.expander.end_group(&at)?;
        self.expander.end_group(&at)?;

        let mut body: Vec<Vec<Node<'a>>> = Vec::with_capacity(parsed_rows.len() + 1);
        for (i, nodes) in parsed_rows.into_iter().enumerate() {
            body.push(cd_row(i, nodes, mode, &at)?);
        }
        // The table ends in an empty row.
        body.push(Vec::new());

        let columns = body.first().map_or(0, Vec::len);
        let cols = vec![
            AlignSpec::Align {
                align: 'c',
                // The package puts an `\enskip` between columns: half of it on each side.
                pregap: Some(0.25),
                postgap: Some(0.25),
            };
            columns
        ];
        let hlines_before_row = vec![Vec::new(); body.len() + 1];
        Ok(Node {
            kind: NodeKind::Array(Box::new(Array {
                add_jot: true,
                arraystretch: 1.0,
                body,
                cols: Some(cols),
                row_gaps: vec![None],
                hskip_before_and_after: None,
                hlines_before_row,
                col_separation: Some(ColSeparation::Cd),
                tags: None,
                leqno: None,
            })),
            mode,
            span: name_span.start..name_span.start,
        })
    }
}

/// The cells and arrows of row `i` of the diagram, from the nodes read for it.
fn cd_row<'a>(
    i: usize,
    nodes: Vec<Node<'a>>,
    mode: Mode,
    at: &Range<usize>,
) -> Result<Vec<Node<'a>>, MathError> {
    let mut row: Vec<Node<'a>> = Vec::new();
    let mut cell = new_cell(mode, at);
    let mut nodes = nodes.into_iter();
    while let Some(node) = nodes.next() {
        if !is_start_of_arrow(&node) {
            // Whatever is not an arrow goes into a cell.
            if let NodeKind::Styling { body, .. } = &mut cell.kind {
                body.push(node);
            }
            continue;
        }
        // The `@` starts an arrow, so the cell before it is done.
        row.push(cell);
        let Some(mark) = nodes.next() else {
            return Err(MathError::new(
                "Expected node of symbol group type, but got undefined",
                node.span,
            ));
        };
        let arrow_char = symbol_text(&mark)?;
        let mut labels: [Vec<Node<'a>>; 2] = [Vec::new(), Vec::new()];
        let mut last = mark.span.clone();
        match arrow_char.as_str() {
            // These arrows take no labels.
            "=" | "|" | "." => {}
            "<" | ">" | "A" | "V" => {
                let missing = |at: Range<usize>| {
                    MathError::new(
                        format!("Missing a {arrow_char} character to complete a CD arrow."),
                        at,
                    )
                };
                for label in &mut labels {
                    loop {
                        let Some(next) = nodes.next() else {
                            return Err(missing(last));
                        };
                        if is_label_end(&next, &arrow_char) {
                            last = next.span;
                            break;
                        }
                        if is_start_of_arrow(&next) {
                            return Err(missing(next.span));
                        }
                        label.push(next);
                    }
                }
            }
            _ => {
                return Err(MathError::new(
                    "Expected one of \"<>AV=|.\" after @",
                    mark.span,
                ));
            }
        }
        let span = node.span.start..last.end;
        row.push(Node {
            kind: NodeKind::Styling {
                style: StyleName::Display,
                body: vec![arrow(&arrow_char, labels, mode, span.clone())],
                reset_font: true,
            },
            mode,
            span,
        });
        // The cells are implicit: what follows the arrow is the next cell.
        cell = new_cell(mode, at);
    }
    if i.is_multiple_of(2) {
        // Even rows alternate cells and arrows, and end in a cell.
        row.push(cell);
    } else if !row.is_empty() {
        // Odd rows are vertical arrows, with the empty cell before the first one removed.
        row.remove(0);
    }
    Ok(row)
}

#[cfg(test)]
mod tests {
    use crate::math::{
        ast::{AlignSpec, Array, CdSide, ColSeparation},
        parser::{Settings, parse, parse_with},
    };

    use super::{Name, Node, NodeKind};

    /// A node as a short string: cells in `[]`, groups in `{}`, an arrow by its label.
    fn show(node: &Node<'_>) -> String {
        let all = |nodes: &[Node<'_>]| nodes.iter().map(show).collect::<Vec<_>>().join(" ");
        match &node.kind {
            NodeKind::Styling { body, .. } => format!("[{}]", all(body)),
            NodeKind::Group { body, .. } => format!("{{{}}}", all(body)),
            NodeKind::Symbol { name, .. } => match name {
                Name::Char(text) => (*text).to_string(),
                Name::Control(name) => format!("\\{name}"),
            },
            NodeKind::XArrow { label, body, below } => match below {
                Some(below) => format!("{label}({}|{})", show(body), show(below)),
                None => format!("{label}({})", show(body)),
            },
            NodeKind::DelimSizing { size, delim, .. } => format!("big{size}{delim}"),
            NodeKind::CdLabel { side, label } => match side {
                CdSide::Left => format!("left{}", show(label)),
                CdSide::Right => format!("right{}", show(label)),
            },
            NodeKind::CdLabelParent { fragment } => format!("vertical<{}>", show(fragment)),
            other => format!("?{}", other.type_name()),
        }
    }

    fn array_of(src: &str) -> Array<'_> {
        let nodes = parse_with(src, Settings { display_mode: true }).unwrap();
        let [
            Node {
                kind: NodeKind::Array(array),
                ..
            },
        ] = <[Node<'_>; 1]>::try_from(nodes).unwrap()
        else {
            panic!("one table");
        };
        *array
    }

    /// The rows of a diagram, each cell as `show` writes it.
    fn rows(src: &str) -> Vec<Vec<String>> {
        array_of(src)
            .body
            .iter()
            .map(|row| row.iter().map(show).collect())
            .collect()
    }

    fn error(src: &str) -> String {
        parse_with(src, Settings { display_mode: true })
            .unwrap_err()
            .message
    }

    const RIGHT: &str = "\\\\cdrightarrow";
    const LEFT: &str = "\\\\cdleftarrow";

    #[test]
    fn right_arrows_take_labels_above_and_below() {
        assert_eq!(
            rows("\\begin{CD}A @>a>b> B\\end{CD}"),
            [
                vec![
                    "[A]".to_string(),
                    format!("[{RIGHT}({{a}}|{{b}})]"),
                    "[B]".into()
                ],
                vec![],
            ]
        );
        // Without labels the groups are empty, as KaTeX's are.
        assert_eq!(
            rows("\\begin{CD}A @>>> B\\end{CD}")[0][1],
            format!("[{RIGHT}({{}}|{{}})]")
        );
    }

    #[test]
    fn left_arrows_take_labels_above_and_below() {
        assert_eq!(
            rows("\\begin{CD}A @<a<b< B\\end{CD}")[0][1],
            format!("[{LEFT}({{a}}|{{b}})]")
        );
    }

    #[test]
    fn vertical_arrows_take_labels_beside_them() {
        assert_eq!(
            rows("\\begin{CD}A @>>> B\\\\@VaVbV @AcAdA\\\\C @= D\\end{CD}")[1],
            [
                "[vertical<{left{a} big2\\downarrow right{b}}>]",
                "[]",
                "[vertical<{left{c} big2\\uparrow right{d}}>]",
            ]
        );
    }

    #[test]
    fn bars_equals_and_blanks_take_no_labels() {
        assert_eq!(
            rows("\\begin{CD}A @= B @| C @. D\\end{CD}")[0],
            [
                "[A]",
                "[\\\\cdlongequal({})]",
                "[B]",
                "[big2\\Vert]",
                "[C]",
                "[ ]",
                "[D]"
            ]
        );
        // The vertical bar in an odd row has no cell before it.
        assert_eq!(
            rows("\\begin{CD}A @= B\\\\@| @.\\end{CD}")[1],
            ["[big2\\Vert]", "[]", "[ ]"]
        );
    }

    #[test]
    fn labels_hold_any_nodes_and_a_braced_mark_does_not_end_one() {
        assert_eq!(
            rows("\\begin{CD}A @>x + y>{>}> B\\end{CD}")[0][1],
            format!("[{RIGHT}({{x + y}}|{{{{>}}}})]")
        );
    }

    #[test]
    fn the_table_is_what_katex_builds() {
        let array = array_of("\\begin{CD}A @>>> B @>>> C\\\\@VVV @. @VVV\\\\D @= E @= F\\end{CD}");
        // The rows, and an empty one after them.
        assert_eq!(array.body.len(), 4);
        assert!(array.body[3].is_empty());
        assert_eq!(array.hlines_before_row.len(), 5);
        assert!(array.hlines_before_row.iter().all(Vec::is_empty));
        assert_eq!(array.row_gaps.len(), 1);
        assert_eq!(array.arraystretch, 1.0);
        assert!(array.add_jot);
        assert_eq!(array.col_separation, Some(ColSeparation::Cd));
        // A column for each cell and arrow of the first row, set apart by half an enskip each side.
        let cols = array.cols.unwrap();
        assert_eq!(cols.len(), 5);
        assert!(cols.iter().all(|col| *col
            == AlignSpec::Align {
                align: 'c',
                pregap: Some(0.25),
                postgap: Some(0.25)
            }));
    }

    #[test]
    fn a_last_empty_row_and_cr_are_read_as_row_ends() {
        let plain = rows("\\begin{CD}A @>>> B\\end{CD}");
        assert_eq!(rows("\\begin{CD}A @>>> B\\\\\\end{CD}"), plain);
        assert_eq!(
            rows("\\begin{CD}A\\cr @VVV\\cr B\\end{CD}"),
            rows("\\begin{CD}A\\\\@VVV\\\\ B\\end{CD}")
        );
        // An odd row keeps only its arrows: the text of one without any is dropped.
        assert_eq!(
            rows("\\begin{CD}A\\\\ B\\\\ C\\end{CD}")[1],
            Vec::<String>::new()
        );
        // Whitespace around the marks does not matter.
        assert_eq!(rows("\\begin{CD}A@>>>B\\end{CD}"), plain);
    }

    #[test]
    fn an_empty_diagram_is_a_table_of_one_empty_row() {
        let array = array_of("\\begin{CD}\\end{CD}");
        assert_eq!(array.body, [Vec::new()]);
        assert_eq!(array.cols, Some(Vec::new()));
        assert_eq!(array.hlines_before_row.len(), 2);
    }

    #[test]
    fn the_diagram_needs_display_mode() {
        let err = parse("\\begin{CD}A\\end{CD}").unwrap_err();
        assert_eq!(err.message, "{CD} can be used only in display mode.");
    }

    #[test]
    fn broken_arrows_fail_as_katex_does() {
        let src = "\\begin{CD}A @>a B\\end{CD}";
        let err = parse_with(src, Settings { display_mode: true }).unwrap_err();
        assert_eq!(err.message, "Missing a > character to complete a CD arrow.");
        // At the arrow's mark.
        assert_eq!(&src[err.span.clone()], ">");
        // The second label is missing its end.
        assert_eq!(
            error("\\begin{CD}A @Va V b\\end{CD}"),
            "Missing a V character to complete a CD arrow."
        );
        // Another arrow starts inside a label.
        let src = "\\begin{CD}A @>a @>b> B\\end{CD}";
        let err = parse_with(src, Settings { display_mode: true }).unwrap_err();
        assert_eq!(err.message, "Missing a > character to complete a CD arrow.");
        assert_eq!(&src[err.span], "@");
        assert_eq!(
            error("\\begin{CD}A @x B\\end{CD}"),
            "Expected one of \"<>AV=|.\" after @"
        );
        assert_eq!(
            error("\\begin{CD}A @\\end{CD}"),
            "Expected node of symbol group type, but got undefined"
        );
        assert_eq!(
            error("\\begin{CD}A @\\alpha B\\end{CD}"),
            "Expected one of \"<>AV=|.\" after @"
        );
        assert_eq!(
            error("\\begin{CD}A @{>} B\\end{CD}"),
            "Expected node of symbol group type, but got node of type ordgroup"
        );
    }

    #[test]
    fn a_diagram_that_does_not_end_fails() {
        assert_eq!(
            error("\\begin{CD}A @>>> B"),
            "Expected \\\\ or \\cr or \\end"
        );
        assert!(error("\\begin{CD}A @>>> B\\end{matrix}").starts_with("Mismatch"));
    }

    #[test]
    fn a_huge_diagram_is_read_in_bounds() {
        let row = "A @>>> B ".repeat(2000);
        let src = format!(
            "\\begin{{CD}}{row}\\\\{}\\end{{CD}}",
            "@VVV @. ".repeat(2000)
        );
        let array = array_of(&src);
        assert_eq!(array.body.len(), 3);
        assert_eq!(array.body[0].len(), 2000 * 2 + 1);
        // Many rows, and a long run of marks that never close.
        let many = format!("\\begin{{CD}}{}\\end{{CD}}", "A\\\\".repeat(5000));
        assert_eq!(array_of(&many).body.len(), 5001);
        let open = format!("\\begin{{CD}}{}\\end{{CD}}", "@>".repeat(5000));
        assert!(parse_with(&open, Settings { display_mode: true }).is_err());
    }
}
