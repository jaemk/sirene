//! Environments: `\begin{name} ... \end{name}`, and the tables they make.
//!
//! A table is rows of cells, rows ended by `\\` and cells by `&`. `parse_array` reads one, and
//! each environment is a set of options for it (`array`, the matrices, `cases`, the aligned
//! environments) with the checks and the wrapping KaTeX gives it. Equation tags (`\tag`, and the
//! numbering of `align`) are kept per row.

use super::{
    MathError, Mode, Name, Node, NodeKind, Parser, TokenKind, handlers::Arguments, token_text,
};
use crate::math::{
    ast::{AlignSpec, Array, ColSeparation, Measurement, RowTag, StyleName},
    functions::Function,
    lexer::Token,
};

/// What `parse_array` is told about the table it reads.
#[derive(Default)]
struct ArrayOptions {
    hskip_before_and_after: Option<bool>,
    add_jot: bool,
    cols: Option<Vec<AlignSpec>>,
    arraystretch: Option<f64>,
    col_separation: Option<ColSeparation>,
    /// `None`: no tags. `Some(true)`: numbered automatically. `Some(false)`: tags only by `\tag`.
    auto_tag: Option<bool>,
    single_row: bool,
    empty_single_row: bool,
    max_num_cols: Option<usize>,
    leqno: Option<bool>,
}

/// How many arguments the environment `name` takes, if there is such an environment.
pub(super) fn environment_args(name: &str) -> Option<usize> {
    Some(match name {
        "array" | "darray" | "subarray" | "alignat" | "alignat*" | "alignedat" => 1,
        "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "matrix*"
        | "pmatrix*" | "bmatrix*" | "Bmatrix*" | "vmatrix*" | "Vmatrix*" | "smallmatrix"
        | "cases" | "dcases" | "rcases" | "drcases" | "align" | "align*" | "aligned" | "split"
        | "gathered" | "gather" | "gather*" | "equation" | "equation*" | "CD" => 0,
        _ => return None,
    })
}

/// The function the table lists for an environment's arguments: all are plain groups.
pub(super) fn environment_function(args: usize) -> Function {
    Function {
        kind: "environment",
        num_args: args,
        num_optional_args: 0,
        allowed_in_text: false,
        allowed_in_math: true,
        allowed_in_argument: false,
        primitive: false,
        infix: false,
        arg_types: "o",
    }
}

/// The text of a group of characters, for an environment name or a column count.
pub(super) fn character_group(
    group: &Node<'_>,
    message: &str,
    allow_spaces: bool,
) -> Result<String, MathError> {
    let NodeKind::Group { body, .. } = &group.kind else {
        return Err(MathError::new(message, group.span.clone()));
    };
    let mut text = String::new();
    for node in body {
        match &node.kind {
            NodeKind::Symbol {
                group: crate::math::symbols::AtomGroup::TextOrd,
                name: Name::Char(c),
            } => text.push_str(c),
            NodeKind::Symbol {
                group: crate::math::symbols::AtomGroup::Spacing,
                name: Name::Char(" "),
            } if allow_spaces => text.push(' '),
            _ => return Err(MathError::new(message, group.span.clone())),
        }
    }
    Ok(text)
}

/// The text of a symbol node as the source writes it, or KaTeX's error for any other node.
pub(super) fn symbol_text(node: &Node<'_>) -> Result<String, MathError> {
    match &node.kind {
        NodeKind::Symbol { name, .. } => Ok(match name {
            Name::Char(text) => (*text).to_string(),
            Name::Control(control) => format!("\\{control}"),
        }),
        other => Err(MathError::new(
            format!(
                "Expected node of symbol group type, but got node of type {}",
                other.type_name()
            ),
            node.span.clone(),
        )),
    }
}

/// The number at the start of `text`, as JavaScript's `parseFloat` reads it.
fn parse_float(text: &str) -> Option<f64> {
    let text = text.trim_start();
    let bytes = text.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'-' | b'+')));
    let digits = |from: usize| {
        let mut i = from;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    };
    let whole = digits(end);
    let mut seen = whole > end;
    end = whole;
    if bytes.get(end) == Some(&b'.') {
        let frac = digits(end + 1);
        seen |= frac > end + 1;
        end = frac;
    }
    if !seen {
        return None;
    }
    text[..end].parse().ok()
}

/// Whether an environment's name makes it an AMS environment that needs display mode.
fn ams_numbering(name: &str) -> Option<bool> {
    // `aligned` and its relatives (`gathered`, `alignedat`) have "ed" in them and take no tags.
    if name.contains("ed") {
        None
    } else {
        Some(!name.contains('*'))
    }
}

/// The style a cell is set in: display for the environments named `d...`, else text.
fn cell_style(name: &str) -> StyleName {
    if name.starts_with('d') {
        StyleName::Display
    } else {
        StyleName::Text
    }
}

impl<'a> Parser<'a> {
    /// The table the environment `name` makes, from its arguments, with the cursor just after
    /// `\begin{name}` and its arguments.
    pub(super) fn build_environment(
        &mut self,
        name: &str,
        name_span: &std::ops::Range<usize>,
        mut args: Arguments<'a>,
    ) -> Result<Node<'a>, MathError> {
        let mode = self.mode;
        let start = name_span.start;
        let ams = |parser: &Parser<'_>| -> Result<(), MathError> {
            if parser.display_mode {
                Ok(())
            } else {
                Err(MathError::new(
                    format!("{{{name}}} can be used only in display mode."),
                    name_span.clone(),
                ))
            }
        };
        let array = match name {
            "array" | "darray" => {
                let cols = column_specs(args.required.pop(), "lcr", true)?;
                let options = ArrayOptions {
                    max_num_cols: Some(cols.len()),
                    cols: Some(cols),
                    hskip_before_and_after: Some(true),
                    ..ArrayOptions::default()
                };
                self.parse_array(options, Some(cell_style(name)))?
            }
            "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "matrix*"
            | "pmatrix*" | "bmatrix*" | "Bmatrix*" | "vmatrix*" | "Vmatrix*" => {
                return self.matrix(name, name_span);
            }
            "smallmatrix" => {
                let options = ArrayOptions {
                    arraystretch: Some(0.5),
                    ..ArrayOptions::default()
                };
                let mut array = self.parse_array(options, Some(StyleName::Script))?;
                array.col_separation = Some(ColSeparation::Small);
                array
            }
            "subarray" => {
                let cols = column_specs(args.required.pop(), "lc", false)?;
                if cols.len() > 1 {
                    return Err(MathError::new(
                        "{subarray} can contain only one column",
                        name_span.clone(),
                    ));
                }
                let options = ArrayOptions {
                    cols: Some(cols),
                    hskip_before_and_after: Some(false),
                    arraystretch: Some(0.5),
                    ..ArrayOptions::default()
                };
                let array = self.parse_array(options, Some(StyleName::Script))?;
                if array.body.first().is_some_and(|row| row.len() > 1) {
                    return Err(MathError::new(
                        "{subarray} can contain only one column",
                        name_span.clone(),
                    ));
                }
                array
            }
            "cases" | "dcases" | "rcases" | "drcases" => {
                let column = |postgap| AlignSpec::Align {
                    align: 'l',
                    pregap: Some(0.0),
                    postgap: Some(postgap),
                };
                let options = ArrayOptions {
                    arraystretch: Some(1.2),
                    cols: Some(vec![column(1.0), column(0.0)]),
                    ..ArrayOptions::default()
                };
                let array = self.parse_array(options, Some(cell_style(name)))?;
                let (left, right) = if name.contains('r') {
                    (".", "\\}")
                } else {
                    ("\\{", ".")
                };
                return Ok(self.wrapped(array, left, right, start, mode));
            }
            "align" | "align*" | "aligned" | "split" | "alignat" | "alignat*" | "alignedat" => {
                if !name.contains("ed") {
                    ams(self)?;
                }
                return self.aligned(name, args.required.pop(), name_span);
            }
            "gathered" | "gather" | "gather*" => {
                if matches!(name, "gather" | "gather*") {
                    ams(self)?;
                }
                let options = ArrayOptions {
                    cols: Some(vec![AlignSpec::Align {
                        align: 'c',
                        pregap: None,
                        postgap: None,
                    }]),
                    add_jot: true,
                    col_separation: Some(ColSeparation::Gather),
                    auto_tag: ams_numbering(name),
                    empty_single_row: true,
                    leqno: Some(false),
                    ..ArrayOptions::default()
                };
                self.parse_array(options, Some(StyleName::Display))?
            }
            "equation" | "equation*" => {
                ams(self)?;
                let options = ArrayOptions {
                    auto_tag: ams_numbering(name),
                    empty_single_row: true,
                    single_row: true,
                    max_num_cols: Some(1),
                    leqno: Some(false),
                    ..ArrayOptions::default()
                };
                self.parse_array(options, Some(StyleName::Display))?
            }
            // Commutative diagrams are not built yet.
            _ => {
                return Err(MathError::new(
                    format!("{{{name}}} is not supported yet"),
                    name_span.clone(),
                ));
            }
        };
        Ok(Node {
            kind: NodeKind::Array(Box::new(array)),
            mode,
            span: start..start,
        })
    }

    /// A table in delimiters, as the matrix and `cases` environments are.
    fn wrapped(
        &self,
        array: Array<'a>,
        left: &str,
        right: &str,
        start: usize,
        mode: Mode,
    ) -> Node<'a> {
        let span = start..start;
        let inner = Node {
            kind: NodeKind::Array(Box::new(array)),
            mode,
            span: span.clone(),
        };
        Node {
            kind: NodeKind::LeftRight {
                body: vec![inner],
                left: left.to_string(),
                right: right.to_string(),
                // A `\color` before the table does not reach into it.
                right_color: None,
            },
            mode,
            span,
        }
    }

    /// `matrix`, `pmatrix`, ..., and the starred forms that take `[l]`, `[c]`, or `[r]`.
    fn matrix(
        &mut self,
        name: &str,
        name_span: &std::ops::Range<usize>,
    ) -> Result<Node<'a>, MathError> {
        let mode = self.mode;
        let delimiters = match name.trim_end_matches('*') {
            "pmatrix" => Some(("(", ")")),
            "bmatrix" => Some(("[", "]")),
            "Bmatrix" => Some(("\\{", "\\}")),
            "vmatrix" => Some(("|", "|")),
            "Vmatrix" => Some(("\\Vert", "\\Vert")),
            _ => None,
        };
        let mut align = 'c';
        if name.ends_with('*') {
            // The mathtools forms take the alignment in brackets.
            self.consume_spaces()?;
            if token_text(&self.fetch()?.kind) == "[" {
                self.consume();
                self.consume_spaces()?;
                let token = self.fetch()?;
                align = match token_text(&token.kind).as_str() {
                    "l" => 'l',
                    "c" => 'c',
                    "r" => 'r',
                    _ => {
                        return Err(MathError::new("Expected l or c or r", token.span));
                    }
                };
                self.consume();
                self.consume_spaces()?;
                self.expect("]")?;
            }
        }
        let spec = AlignSpec::Align {
            align,
            pregap: None,
            postgap: None,
        };
        let options = ArrayOptions {
            hskip_before_and_after: Some(false),
            cols: Some(vec![spec.clone()]),
            ..ArrayOptions::default()
        };
        let mut array = self.parse_array(options, Some(cell_style(name)))?;
        // One column spec for each column the rows turned out to have.
        let columns = array.body.iter().map(Vec::len).max().unwrap_or(0);
        array.cols = Some(vec![spec; columns]);
        match delimiters {
            Some((left, right)) => Ok(self.wrapped(array, left, right, name_span.start, mode)),
            None => Ok(Node {
                kind: NodeKind::Array(Box::new(array)),
                mode,
                span: name_span.start..name_span.start,
            }),
        }
    }

    /// `align`, `aligned`, `alignat`, and `split`: right-aligned and left-aligned columns in
    /// pairs, with the empty group that makes a leading operator binary.
    fn aligned(
        &mut self,
        name: &str,
        count: Option<Node<'a>>,
        name_span: &std::ops::Range<usize>,
    ) -> Result<Node<'a>, MathError> {
        let mode = self.mode;
        let is_split = name == "split";
        let options = ArrayOptions {
            cols: Some(Vec::new()),
            add_jot: true,
            auto_tag: if is_split { None } else { ams_numbering(name) },
            empty_single_row: true,
            col_separation: Some(if name.contains("at") {
                ColSeparation::Alignat
            } else {
                ColSeparation::Align
            }),
            max_num_cols: if is_split { Some(2) } else { None },
            leqno: Some(false),
            ..ArrayOptions::default()
        };
        let mut array = self.parse_array(options, Some(StyleName::Display))?;

        // The first argument of `alignat` is the number of column pairs; without it the
        // number is the most cells any row has.
        let mut num_maths = 0;
        let mut num_cols = 0;
        if let Some(count) = &count
            && matches!(count.kind, NodeKind::Group { .. })
        {
            let message = "Number of columns should be a positive integer";
            let text = character_group(count, message, false)?;
            let number = text.parse::<usize>().ok().filter(|n| *n >= 1);
            let Some(number) = number.filter(|_| text.bytes().all(|b| b.is_ascii_digit())) else {
                return Err(MathError::new(message, count.span.clone()));
            };
            num_maths = number;
            num_cols = number * 2;
        }
        let is_aligned = num_cols == 0;
        for row in &mut array.body {
            for cell in row.iter_mut().skip(1).step_by(2) {
                if let NodeKind::Styling { body, .. } = &mut cell.kind
                    && let Some(Node {
                        kind: NodeKind::Group { body: inner, .. },
                        ..
                    }) = body.first_mut()
                {
                    inner.insert(
                        0,
                        Node {
                            kind: NodeKind::Group {
                                body: Vec::new(),
                                semisimple: false,
                            },
                            mode,
                            span: name_span.clone(),
                        },
                    );
                }
            }
            if !is_aligned {
                // A pair is two cells; an odd last cell counts as half.
                if num_maths * 2 < row.len() {
                    let pairs = if row.len() % 2 == 0 {
                        (row.len() / 2).to_string()
                    } else {
                        format!("{}.5", row.len() / 2)
                    };
                    return Err(MathError::new(
                        format!("Too many math in a row: expected {num_maths}, but got {pairs}"),
                        row.first().map_or(0..0, |c| c.span.clone()),
                    ));
                }
            } else if num_cols < row.len() {
                num_cols = row.len();
            }
        }
        // In aligned mode one quad separates the pairs; otherwise nothing does.
        let cols = (0..num_cols)
            .map(|i| {
                let (align, pregap) = if i % 2 == 1 {
                    ('l', 0.0)
                } else if i > 0 && is_aligned {
                    ('r', 1.0)
                } else {
                    ('r', 0.0)
                };
                AlignSpec::Align {
                    align,
                    pregap: Some(pregap),
                    postgap: Some(0.0),
                }
            })
            .collect();
        array.cols = Some(cols);
        array.col_separation = Some(if is_aligned {
            ColSeparation::Align
        } else {
            ColSeparation::Alignat
        });
        Ok(Node {
            kind: NodeKind::Array(Box::new(array)),
            mode,
            span: name_span.start..name_span.start,
        })
    }

    /// The `\hline` and `\hdashline` rules before a row: for each, whether it is dashed.
    fn get_hlines(&mut self) -> Result<Vec<bool>, MathError> {
        let mut lines = Vec::new();
        self.consume_spaces()?;
        let mut next = token_text(&self.fetch()?.kind);
        // `\relax` is left by the `\cr` macro.
        if next == "\\relax" {
            self.consume();
            self.consume_spaces()?;
            next = token_text(&self.fetch()?.kind);
        }
        while next == "\\hline" || next == "\\hdashline" {
            self.consume();
            lines.push(next == "\\hdashline");
            self.consume_spaces()?;
            next = token_text(&self.fetch()?.kind);
        }
        Ok(lines)
    }

    /// The rows of a table: cells separated by `&`, rows by `\\`, up to `\end`.
    fn parse_array(
        &mut self,
        options: ArrayOptions,
        style: Option<StyleName>,
    ) -> Result<Array<'a>, MathError> {
        let at = self.fetch()?.span;
        self.expander.begin_group(&at)?;
        if !options.single_row {
            // `\cr` is `\\` without the optional size.
            self.expander.set_text_macro("cr", "\\\\\\relax", false)?;
        }
        let arraystretch = match options.arraystretch {
            Some(stretch) => stretch,
            None => match self.expander.macro_text("arraystretch") {
                None => 1.0,
                Some(text) => match parse_float(&text) {
                    Some(stretch) if stretch > 0.0 => stretch,
                    _ => {
                        return Err(MathError::new(
                            format!("Invalid \\arraystretch: {text}"),
                            at,
                        ));
                    }
                },
            },
        };
        // A group for the first cell.
        self.expander.begin_group(&at)?;
        let outer_color = self.current_color.clone();

        let mut body: Vec<Vec<Node<'a>>> = vec![Vec::new()];
        let mut row_gaps = Vec::new();
        let mut hlines_before_row = Vec::new();
        let mut tags: Option<Vec<RowTag<'a>>> = options.auto_tag.map(|_| Vec::new());
        self.begin_row(options.auto_tag)?;
        hlines_before_row.push(self.get_hlines()?);

        loop {
            // Each cell is read in a group of its own.
            let until = if options.single_row { "\\end" } else { "\\\\" };
            let cell_body = self.parse_expression_until(false, Some(until))?;
            self.expander.end_group(&at)?;
            self.expander.begin_group(&at)?;
            self.current_color = outer_color.clone();
            let span = cell_body
                .first()
                .zip(cell_body.last())
                .map_or(at.clone(), |(a, b)| a.span.start..b.span.end);
            let mut cell = Node {
                kind: NodeKind::Group {
                    body: cell_body,
                    semisimple: false,
                },
                mode: self.mode,
                span: span.clone(),
            };
            if let Some(style) = style {
                cell = Node {
                    kind: NodeKind::Styling {
                        style,
                        body: vec![cell],
                        reset_font: true,
                    },
                    mode: self.mode,
                    span,
                };
            }
            let row = body.len() - 1;
            body[row].push(cell);
            let token = self.fetch()?;
            match token_text(&token.kind).as_str() {
                "&" => {
                    if options.max_num_cols == Some(body[row].len())
                        && (options.single_row || options.col_separation.is_some())
                    {
                        return Err(MathError::new("Too many tab characters: &", token.span));
                    }
                    self.consume();
                }
                "\\end" => {
                    self.end_row(&mut tags, options.auto_tag)?;
                    // A last row that is empty is dropped (the `\\` before `\end` makes it),
                    // unless an environment keeps it: it is the only row, has its own tag, or
                    // is numbered and was ended by `\\` on purpose.
                    let last_cell_empty = matches!(
                        body[row].last().map(|n| &n.kind),
                        Some(NodeKind::Styling { body: inner, .. })
                            if matches!(inner.as_slice(), [Node { kind: NodeKind::Group { body, .. }, .. }] if body.is_empty())
                    );
                    let last_tag = tags.as_ref().and_then(|t| t.last());
                    if body[row].len() == 1
                        && last_cell_empty
                        && (body.len() > 1 || !options.empty_single_row)
                        && !matches!(last_tag, Some(RowTag::Custom(_)))
                        && !(options.auto_tag == Some(true)
                            && matches!(last_tag, Some(RowTag::Numbered(true))))
                    {
                        body.pop();
                    }
                    if hlines_before_row.len() < body.len() + 1 {
                        hlines_before_row.push(Vec::new());
                    }
                    break;
                }
                "\\\\" => {
                    self.consume();
                    // A size in brackets may follow, unless a space does.
                    let size = if self.expander.future()?.kind == TokenKind::Space {
                        None
                    } else {
                        self.parse_size_group(true)?
                    };
                    row_gaps.push(match size {
                        Some(Node {
                            kind: NodeKind::Size { value, .. },
                            ..
                        }) => Some(value),
                        _ => None,
                    });
                    self.end_row(&mut tags, options.auto_tag)?;
                    // Rules may follow the row separator.
                    hlines_before_row.push(self.get_hlines()?);
                    body.push(Vec::new());
                    self.begin_row(options.auto_tag)?;
                }
                _ => {
                    return Err(MathError::new(
                        "Expected & or \\\\ or \\cr or \\end",
                        token.span,
                    ));
                }
            }
        }
        // The cell's group, then the table's.
        self.expander.end_group(&at)?;
        self.expander.end_group(&at)?;
        Ok(Array {
            add_jot: options.add_jot,
            arraystretch,
            body,
            cols: options.cols,
            row_gaps,
            hskip_before_and_after: options.hskip_before_and_after,
            hlines_before_row,
            col_separation: options.col_separation,
            tags,
            leqno: options.leqno,
        })
    }

    /// A numbered environment starts each row numbered, until `\nonumber` says otherwise.
    fn begin_row(&mut self, auto_tag: Option<bool>) -> Result<(), MathError> {
        if auto_tag == Some(true) {
            self.expander.set_text_macro("@eqnsw", "1", true)?;
        }
        Ok(())
    }

    /// Records the finished row's tag: its `\tag`, or whether it is numbered.
    fn end_row(
        &mut self,
        tags: &mut Option<Vec<RowTag<'a>>>,
        auto_tag: Option<bool>,
    ) -> Result<(), MathError> {
        let Some(tags) = tags else {
            return Ok(());
        };
        if self.expander.has_macro("df@tag") {
            let nodes = self.subparse(vec![Token {
                kind: TokenKind::ControlWord("df@tag"),
                span: 0..0,
            }])?;
            tags.push(RowTag::Custom(nodes));
            self.expander.clear_macro("df@tag");
        } else {
            let numbered = auto_tag == Some(true)
                && self.expander.macro_text("@eqnsw").as_deref() == Some("1");
            tags.push(RowTag::Numbered(numbered));
        }
        Ok(())
    }
}

/// The columns an `array`'s argument names: one letter of `allowed` each (and `|` or `:` rules
/// where `rules` is set).
fn column_specs(
    arg: Option<Node<'_>>,
    allowed: &str,
    rules: bool,
) -> Result<Vec<AlignSpec>, MathError> {
    let Some(arg) = arg else {
        return Err(MathError::new("Missing arguments for \\begin", 0..0));
    };
    // The argument is a group of symbols, or a lone symbol.
    let nodes = match arg.kind {
        NodeKind::Group { body, .. } => body,
        NodeKind::Symbol { .. } => vec![arg],
        other => {
            return Err(MathError::new(
                format!(
                    "Expected node of type ordgroup, but got node of type {}",
                    other.type_name()
                ),
                arg.span,
            ));
        }
    };
    nodes
        .iter()
        .map(|node| {
            let text = symbol_text(node)?;
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if allowed.contains(c) => Ok(AlignSpec::Align {
                    align: c,
                    pregap: None,
                    postgap: None,
                }),
                (Some(c @ ('|' | ':')), None) if rules => Ok(AlignSpec::Separator(c)),
                _ => Err(MathError::new(
                    format!("Unknown column alignment: {text}"),
                    node.span.clone(),
                )),
            }
        })
        .collect()
}

/// A measurement for a `Cr` node from a size argument.
pub(super) fn size_value(node: Option<Node<'_>>) -> Option<Measurement> {
    match node {
        Some(Node {
            kind: NodeKind::Size { value, .. },
            ..
        }) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_float_reads_a_prefix() {
        assert_eq!(parse_float("1.5"), Some(1.5));
        assert_eq!(parse_float("  2"), Some(2.0));
        assert_eq!(parse_float("1.5pt"), Some(1.5));
        assert_eq!(parse_float(".5"), Some(0.5));
        assert_eq!(parse_float("-3"), Some(-3.0));
        assert_eq!(parse_float("x"), None);
        assert_eq!(parse_float("."), None);
        assert_eq!(parse_float(""), None);
    }

    #[test]
    fn numbering_follows_the_name() {
        assert_eq!(ams_numbering("align"), Some(true));
        assert_eq!(ams_numbering("align*"), Some(false));
        assert_eq!(ams_numbering("aligned"), None);
        assert_eq!(ams_numbering("gathered"), None);
        assert_eq!(ams_numbering("equation"), Some(true));
    }
}
