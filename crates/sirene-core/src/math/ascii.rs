//! The ASCII renderer (LMATH-4): a formula as rows of text.
//!
//! It reads the parse tree, not the box tree, and builds blocks of characters with a baseline,
//! which are joined side by side (aligned on the baselines) or stacked. A fraction is a numerator,
//! a rule of `-` and a denominator; scripts use Unicode super- and subscript characters where
//! they exist and `^`/`_` forms where not; a radical is `√` under an overline row; a table is
//! aligned columns. Like the flowchart renderer's default character set it draws with Unicode;
//! a node it has no drawing for shows its source text. It never panics.

use super::{
    ast::{AlignSpec, Array, MathClass, Measurement, Node, NodeKind, OpContent, StyleName},
    error::MathError,
    formula::MathStyle,
    parser::{Settings, parse_with},
    symbols::{AtomGroup, Mode, Name, lookup},
};

/// Typesets `source`, a formula in TeX math mode (no `$` delimiters), as rows of text.
///
/// Trailing spaces and blank rows at either end are removed. Fails as `typeset` does.
pub fn ascii_formula(source: &str, style: MathStyle) -> Result<String, MathError> {
    Ok(ascii_formula_rows(source, style)?.0.join("\n"))
}

/// Like `ascii_formula`, but as the rows and the index of the row the baseline is on, so a
/// formula can sit in a line of text.
pub fn ascii_formula_rows(
    source: &str,
    style: MathStyle,
) -> Result<(Vec<String>, usize), MathError> {
    let nodes = parse_with(
        source,
        Settings {
            display_mode: style == MathStyle::Display,
        },
    )?;
    let renderer = Renderer {
        source,
        compact: false,
    };
    let block = renderer.list(&nodes, style == MathStyle::Display);
    Ok(block.finish_rows())
}

/// How a block's rows sit in a wider space.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Center,
    Right,
}

/// A rectangle of characters with the row the baseline is on.
#[derive(Clone, Debug, Default)]
struct Block {
    rows: Vec<Vec<char>>,
    base: usize,
    width: usize,
}

impl Block {
    /// The text as rows, the baseline on the first.
    fn text(s: &str) -> Block {
        let lines: Vec<Vec<char>> = s.split('\n').map(|l| l.chars().collect()).collect();
        let width = lines.iter().map(Vec::len).max().unwrap_or(0);
        if width == 0 && lines.len() <= 1 {
            return Block::default();
        }
        let rows = lines
            .into_iter()
            .map(|mut l| {
                l.resize(width, ' ');
                l
            })
            .collect();
        Block {
            rows,
            base: 0,
            width,
        }
    }

    fn filled(ch: char, width: usize, height: usize, base: usize) -> Block {
        Block {
            rows: vec![vec![ch; width]; height],
            base,
            width,
        }
    }

    fn height(&self) -> usize {
        self.rows.len()
    }

    fn below(&self) -> usize {
        self.rows.len().saturating_sub(self.base + 1)
    }

    fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The single row of the block, if it has one.
    fn one_row(&self) -> Option<String> {
        match self.rows.as_slice() {
            [row] => Some(row.iter().collect()),
            _ => None,
        }
    }

    /// Blocks side by side, their baselines on one row.
    fn hcat(parts: Vec<Block>) -> Block {
        let live = || parts.iter().filter(|p| !p.is_empty());
        if live().next().is_none() {
            return Block {
                width: parts.iter().map(|p| p.width).sum(),
                ..Block::default()
            };
        }
        let above = live().map(|p| p.base).max().unwrap_or(0);
        let below = live().map(Block::below).max().unwrap_or(0);
        let height = above + below + 1;
        let width = parts.iter().map(|p| p.width).sum();
        let mut rows: Vec<Vec<char>> = vec![Vec::with_capacity(width); height];
        for part in &parts {
            for (r, row) in rows.iter_mut().enumerate() {
                match (r + part.base)
                    .checked_sub(above)
                    .and_then(|i| part.rows.get(i))
                    .filter(|_| !part.is_empty())
                {
                    Some(cells) => row.extend(cells),
                    None => row.extend(std::iter::repeat_n(' ', part.width)),
                }
            }
        }
        Block {
            rows,
            base: above,
            width,
        }
    }

    /// Blocks one under another, the baseline that of part `base_part`.
    fn stack(parts: Vec<Block>, base_part: usize, align: Align) -> Block {
        let width = parts.iter().map(|p| p.width).max().unwrap_or(0);
        let base = parts
            .iter()
            .take(base_part)
            .map(Block::height)
            .sum::<usize>()
            + parts.get(base_part).map_or(0, |p| p.base);
        let mut rows = Vec::new();
        for part in &parts {
            let spare = width - part.width;
            let left = match align {
                Align::Left => 0,
                Align::Center => spare / 2,
                Align::Right => spare,
            };
            for cells in &part.rows {
                let mut row = vec![' '; left];
                row.extend(cells);
                row.resize(width, ' ');
                rows.push(row);
            }
        }
        Block { rows, base, width }
    }

    fn padded(self, width: usize, align: Align) -> Block {
        if self.width >= width {
            return self;
        }
        let filler = Block {
            width,
            ..Block::default()
        };
        let base = self.base;
        let mut out = Block::stack(vec![self, filler], 0, align);
        out.base = base;
        out
    }

    fn blank_like(&self) -> Block {
        Block {
            rows: vec![vec![' '; self.width]; self.height()],
            base: self.base,
            width: self.width,
        }
    }

    fn map_chars(mut self, f: impl Fn(char) -> char) -> Block {
        for row in &mut self.rows {
            for c in row.iter_mut() {
                *c = f(*c);
            }
        }
        self
    }

    /// The rows as text, trailing spaces and blank rows at either end cut, and the index of the
    /// baseline row among them.
    fn finish_rows(self) -> (Vec<String>, usize) {
        let lines: Vec<String> = self
            .rows
            .iter()
            .map(|r| r.iter().collect::<String>().trim_end().to_owned())
            .collect();
        let first = lines.iter().position(|l| !l.is_empty());
        let last = lines.iter().rposition(|l| !l.is_empty());
        match (first, last) {
            (Some(a), Some(b)) => {
                let rows = lines.get(a..=b).map(<[String]>::to_vec).unwrap_or_default();
                let base = self
                    .base
                    .saturating_sub(a)
                    .min(rows.len().saturating_sub(1));
                (rows, base)
            }
            _ => (Vec::new(), 0),
        }
    }
}

/// The spacing class of a node in a list, as far as the renderer spaces by it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ord,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Op,
}

fn class_of(n: &Node<'_>) -> Class {
    if n.mode == Mode::Text {
        return Class::Ord;
    }
    match &n.kind {
        NodeKind::Symbol { group, .. } => match group {
            AtomGroup::Bin => Class::Bin,
            AtomGroup::Rel => Class::Rel,
            AtomGroup::Open => Class::Open,
            AtomGroup::Close => Class::Close,
            AtomGroup::Punct => Class::Punct,
            AtomGroup::Op => Class::Op,
            _ => Class::Ord,
        },
        NodeKind::MClass { class, .. } => match class {
            MathClass::Bin => Class::Bin,
            MathClass::Rel => Class::Rel,
            MathClass::Open => Class::Open,
            MathClass::Close => Class::Close,
            MathClass::Punct => Class::Punct,
            _ => Class::Ord,
        },
        NodeKind::Op { .. } | NodeKind::OperatorName { .. } => Class::Op,
        NodeKind::XArrow { .. } => Class::Rel,
        NodeKind::HtmlMathml { html, .. } => html.first().map_or(Class::Ord, class_of),
        NodeKind::SupSub {
            base: Some(base), ..
        } => class_of(base),
        _ => Class::Ord,
    }
}

/// Whether a node is `\\not`, which strikes through the symbol after it (KaTeX's `\\@not`).
fn is_not(n: &Node<'_>) -> bool {
    match &n.kind {
        NodeKind::MClass { body, .. } | NodeKind::Group { body, .. } => {
            matches!(body.as_slice(), [only] if is_not(only))
        }
        NodeKind::HtmlMathml { html, .. } => html.first().is_some_and(is_not),
        NodeKind::Lap { body, .. } => is_not(body),
        NodeKind::Symbol { name, .. } => {
            lookup(Mode::Math, *name).is_some_and(|s| s.replace == "\u{e020}")
        }
        _ => false,
    }
}

/// A block with a slash through it: one character takes its negated form.
fn negated(block: Block) -> Block {
    let Some(row) = block.one_row() else {
        return block;
    };
    let mut chars = row.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return Block::hcat(vec![Block::text("/"), block]);
    };
    let n = match c {
        '=' => '\u{2260}',
        '<' => '\u{226e}',
        '>' => '\u{226f}',
        '\u{2208}' => '\u{2209}',
        '\u{220b}' => '\u{220c}',
        '\u{2264}' => '\u{2270}',
        '\u{2265}' => '\u{2271}',
        '\u{2282}' => '\u{2284}',
        '\u{2283}' => '\u{2285}',
        '\u{2286}' => '\u{2288}',
        '\u{2287}' => '\u{2289}',
        '\u{2261}' => '\u{2262}',
        '\u{223c}' => '\u{2241}',
        '\u{2248}' => '\u{2249}',
        '\u{2243}' => '\u{2244}',
        '\u{2245}' => '\u{2247}',
        '\u{2192}' => '\u{219b}',
        '\u{2190}' => '\u{219a}',
        '\u{21d2}' => '\u{21cf}',
        '\u{21d0}' => '\u{21cd}',
        '\u{2203}' => '\u{2204}',
        '\u{2223}' => '\u{2224}',
        '\u{2225}' => '\u{2226}',
        '\u{227a}' => '\u{2280}',
        '\u{227b}' => '\u{2281}',
        other => return Block::hcat(vec![Block::text("/"), Block::text(&other.to_string())]),
    };
    Block::text(&n.to_string())
}

/// Whether a node is a delimiter pair that the spacing after an operator should not split from.
fn is_open(n: &Node<'_>) -> bool {
    matches!(n.kind, NodeKind::LeftRight { .. }) || class_of(n) == Class::Open
}

struct Renderer<'s> {
    source: &'s str,
    /// Set inside scripts and limits' scripts: no spaces around operators.
    compact: bool,
}

impl Renderer<'_> {
    /// Nodes in a row, spaced as TeX spaces atoms; a line break stacks lines.
    fn list(&self, nodes: &[Node<'_>], display: bool) -> Block {
        if nodes.iter().any(|n| matches!(n.kind, NodeKind::Cr { .. })) {
            let lines: Vec<Block> = nodes
                .split(|n| matches!(n.kind, NodeKind::Cr { .. }))
                .map(|line| self.line(line, display))
                .collect();
            return Block::stack(lines, 0, Align::Left);
        }
        self.line(nodes, display)
    }

    fn line(&self, nodes: &[Node<'_>], display: bool) -> Block {
        // Each node drawn, with its class and whether it opens; `\not` joins the node after it.
        let mut items: Vec<(Class, bool, Block)> = Vec::new();
        let mut at = 0;
        while let Some(node) = nodes.get(at) {
            at += 1;
            let (node, negate) = match nodes.get(at) {
                Some(next) if is_not(node) => {
                    at += 1;
                    (next, true)
                }
                _ => (node, false),
            };
            let block = self.node(node, display);
            let block = if negate { negated(block) } else { block };
            // `\notin` strikes the symbol before it with a slash laid over it.
            if matches!(
                &node.kind,
                NodeKind::Lap {
                    alignment: "llap",
                    ..
                }
            ) && block.one_row().is_some_and(|r| r.trim() == "/")
                && let Some(last) = items.last_mut()
            {
                last.2 = negated(std::mem::take(&mut last.2));
                continue;
            }
            items.push((class_of(node), is_open(node), block));
        }
        let mut pieces: Vec<Block> = Vec::new();
        let mut last_space = false;
        let space = |pieces: &mut Vec<Block>, last_space: &mut bool| {
            if !pieces.is_empty() && !*last_space {
                pieces.push(Block::text(" "));
                *last_space = true;
            }
        };
        let count = items.len();
        let classes: Vec<Class> = items.iter().map(|i| i.0).collect();
        let nodes_open: Vec<bool> = items.iter().map(|i| i.1).collect();
        for (i, (class, _, block)) in items.into_iter().enumerate() {
            let prev = i.checked_sub(1).and_then(|j| classes.get(j)).copied();
            let next = classes.get(i + 1).copied();
            let binary = class == Class::Bin
                && prev.is_some_and(|p| {
                    !matches!(
                        p,
                        Class::Bin | Class::Rel | Class::Open | Class::Punct | Class::Op
                    )
                })
                && next.is_some_and(|n| !matches!(n, Class::Rel | Class::Close | Class::Punct));
            let blank = block.is_empty() && block.width == 0;
            let has_next = i + 1 < count;
            if self.compact {
                pieces.push(block);
                continue;
            }
            match class {
                Class::Rel | Class::Bin if class == Class::Rel || binary => {
                    space(&mut pieces, &mut last_space);
                    pieces.push(block);
                    last_space = false;
                    if has_next {
                        space(&mut pieces, &mut last_space);
                    }
                }
                Class::Punct => {
                    pieces.push(block);
                    last_space = false;
                    if has_next {
                        space(&mut pieces, &mut last_space);
                    }
                }
                Class::Op => {
                    pieces.push(block);
                    last_space = false;
                    let opens = nodes_open.get(i + 1).copied().unwrap_or(true);
                    if has_next && !opens && next != Some(Class::Close) {
                        space(&mut pieces, &mut last_space);
                    }
                }
                _ => {
                    pieces.push(block);
                    if !blank {
                        last_space = false;
                    }
                }
            }
        }
        Block::hcat(pieces)
    }

    /// The source of a node, for a node with no drawing.
    fn source_of(&self, n: &Node<'_>) -> Block {
        self.source
            .get(n.span.clone())
            .map(Block::text)
            .unwrap_or_default()
    }

    fn node(&self, n: &Node<'_>, d: bool) -> Block {
        match &n.kind {
            NodeKind::Symbol { group, name } => self.symbol(n, *group, *name),
            NodeKind::Group { body, .. }
            | NodeKind::Color { body, .. }
            | NodeKind::Sizing { body, .. }
            | NodeKind::Pmb { body, .. }
            | NodeKind::HBox { body }
            | NodeKind::Text { body, .. }
            | NodeKind::MClass { body, .. } => self.list(body, d),
            NodeKind::Styling { style, body, .. } => self.list(body, *style == StyleName::Display),
            NodeKind::SupSub { base, sup, sub } => {
                self.supsub(base.as_deref(), sup.as_deref(), sub.as_deref(), d)
            }
            NodeKind::Frac {
                numer,
                denom,
                bar,
                left,
                right,
                ..
            } => self.frac(numer, denom, *bar, left.as_deref(), right.as_deref(), d),
            NodeKind::Infix { .. } => self.source_of(n),
            NodeKind::Sqrt { body, index } => self.sqrt(body, index.as_deref(), d),
            NodeKind::Font { font, body } => {
                let block = self.node(body, d);
                if *font == "mathbb" {
                    block.map_chars(double_struck)
                } else {
                    block
                }
            }
            NodeKind::Overline { body } => {
                let body = self.node(body, d);
                let bar = Block::filled('\u{af}', body.width, 1, 0);
                let base = body.base + 1;
                let mut out = Block::stack(vec![bar, body], 1, Align::Left);
                out.base = base;
                out
            }
            NodeKind::Underline { body } => {
                let body = self.node(body, d);
                let bar = Block::filled('_', body.width, 1, 0);
                Block::stack(vec![body, bar], 0, Align::Left)
            }
            NodeKind::DelimSizing { delim, .. } | NodeKind::Middle { delim } => {
                self.delim(delim, 0, 0)
            }
            NodeKind::LeftRight {
                body, left, right, ..
            } => self.left_right(body, left, right, d),
            NodeKind::Accent { label, base, .. } => self.accent(label, base, d),
            NodeKind::AccentUnder { label, base } => self.accent_under(label, base, d),
            NodeKind::Op { content, .. } => match content {
                OpContent::Name(name) => Block::text(&op_text(name)),
                OpContent::Body(body) => self.list(body, d),
            },
            NodeKind::OperatorName { body, .. } => self.list(body, false),
            NodeKind::Kern { dimension } => Block::text(&" ".repeat(cells(dimension))),
            NodeKind::Rule { width, height, .. } => {
                if em_of(height) > em_of(width) {
                    Block::filled('\u{2502}', 1, cells(height).max(1), 0)
                } else {
                    Block::filled('\u{2500}', cells(width).max(1), 1, 0)
                }
            }
            NodeKind::Phantom { body } => self.list(body, d).blank_like(),
            NodeKind::VPhantom { body } => {
                let b = self.node(body, d);
                Block {
                    rows: vec![Vec::new(); b.height()],
                    base: b.base,
                    width: 0,
                }
            }
            NodeKind::Smash { body, .. }
            | NodeKind::Lap { body, .. }
            | NodeKind::RaiseBox { body, .. }
            | NodeKind::VCenter { body }
            | NodeKind::ReflectBox { body } => self.node(body, d),
            NodeKind::Array(array) => self.array(array, d),
            NodeKind::Tag { body, tag } => Block::hcat(vec![
                self.list(body, d),
                Block::text("  ("),
                self.list(tag, false),
                Block::text(")"),
            ]),
            NodeKind::Enclose { label, body, .. } => {
                let body = self.node(body, d);
                if matches!(label.as_str(), "\\fbox" | "\\fcolorbox" | "\\boxed") {
                    boxed(body)
                } else {
                    body
                }
            }
            NodeKind::MathChoice { display, text, .. } => {
                self.list(if d { display } else { text }, d)
            }
            NodeKind::Verb { body, .. } => Block::text(body),
            NodeKind::HtmlMathml { html, .. } => self.list(html, d),
            NodeKind::CharCode(c) => Block::text(&c.to_string()),
            NodeKind::XArrow { label, body, below } => self.xarrow(label, body, below.as_deref()),
            NodeKind::HorizBrace {
                label,
                is_over,
                base,
            } => {
                let base = self.node(base, d);
                let brace = label.contains("brace");
                let w = base.width;
                let line = bracket_line(w, *is_over, brace);
                if *is_over {
                    let at = base.base + 1;
                    let mut out = Block::stack(vec![line, base], 1, Align::Left);
                    out.base = at;
                    out
                } else {
                    Block::stack(vec![base, line], 0, Align::Left)
                }
            }
            NodeKind::CdLabel { label, .. } => self.node(label, d),
            NodeKind::CdLabelParent { fragment } => self.node(fragment, d),
            NodeKind::Raw { .. }
            | NodeKind::Url { .. }
            | NodeKind::Size { .. }
            | NodeKind::ColorToken(_)
            | NodeKind::Internal
            | NodeKind::Cr { .. }
            | NodeKind::Environment { .. } => Block::default(),
        }
    }

    fn symbol(&self, n: &Node<'_>, group: AtomGroup, name: Name<'_>) -> Block {
        if group == AtomGroup::Spacing {
            let width = match name {
                Name::Control("quad" | "enspace") => 2,
                Name::Control("qquad") => 4,
                Name::Control("!" | "negthinspace" | "negmedspace" | "negthickspace") => 0,
                _ => 1,
            };
            return Block::text(&" ".repeat(width));
        }
        match lookup(n.mode, name) {
            Some(symbol) => Block::text(&plain_space(symbol.replace)),
            None => match name {
                Name::Char(text) => Block::text(text),
                Name::Control(_) => self.source_of(n),
            },
        }
    }

    fn supsub(
        &self,
        base: Option<&Node<'_>>,
        sup: Option<&Node<'_>>,
        sub: Option<&Node<'_>>,
        d: bool,
    ) -> Block {
        let base_block = base.map(|b| self.node(b, d)).unwrap_or_default();
        if base.is_some_and(|b| stacked(b, d)) {
            let mut parts = Vec::new();
            let mut at = 0;
            if let Some(s) = sup {
                parts.push(self.node(s, false));
                at = 1;
            }
            parts.push(base_block);
            if let Some(s) = sub {
                parts.push(self.node(s, false));
            }
            return Block::stack(parts, at, Align::Center);
        }
        let small = Renderer {
            source: self.source,
            compact: true,
        };
        let sup = sup.map(|s| script(small.node(s, false), true));
        let sub = sub.map(|s| script(small.node(s, false), false));
        // Unicode scripts read as `x_i^2`; the `^` and `_` forms as `x^2_i`.
        let all_unicode = sup.iter().chain(&sub).all(|(_, unicode)| *unicode);
        let mut parts = vec![base_block];
        let (first, second) = if all_unicode { (sub, sup) } else { (sup, sub) };
        parts.extend(first.into_iter().chain(second).map(|(b, _)| b));
        Block::hcat(parts)
    }

    fn frac(
        &self,
        numer: &Node<'_>,
        denom: &Node<'_>,
        bar: bool,
        left: Option<&str>,
        right: Option<&str>,
        d: bool,
    ) -> Block {
        let numer = self.node(numer, d);
        let denom = self.node(denom, d);
        let width = numer.width.max(denom.width) + if bar { 2 } else { 0 };
        let mut parts = vec![numer];
        let mut base = parts.first().map_or(0, |p| p.height().saturating_sub(1));
        if bar {
            parts.push(Block::filled('-', width, 1, 0));
            base += 1;
        }
        parts.push(denom);
        let mut body = Block::stack(parts, 0, Align::Center);
        body.base = base;
        let (above, below) = (body.base, body.below());
        Block::hcat(vec![
            left.map(|l| self.delim(l, above, below))
                .unwrap_or_default(),
            body,
            right
                .map(|r| self.delim(r, above, below))
                .unwrap_or_default(),
        ])
    }

    fn sqrt(&self, body: &Node<'_>, index: Option<&Node<'_>>, d: bool) -> Block {
        let body = self.node(body, d);
        let index = index.map(|i| {
            let b = self.node(i, false);
            match script_string(&b, true) {
                Some(s) => s,
                None => format!("({})", b.one_row().unwrap_or_default()),
            }
        });
        let lead: Vec<char> = index.unwrap_or_default().chars().collect();
        let pad = lead.len();
        let mut rows = Vec::new();
        let mut top = vec![' '; pad + 1];
        top.extend(std::iter::repeat_n('\u{af}', body.width));
        rows.push(top);
        for (r, cells) in body.rows.iter().enumerate() {
            let mut row = vec![' '; pad + 1];
            if r == body.base {
                row = lead.clone();
                row.push('\u{221a}');
            }
            row.extend(cells);
            rows.push(row);
        }
        Block {
            rows,
            base: body.base + 1,
            width: pad + 1 + body.width,
        }
    }

    /// A delimiter `above` rows above the baseline and `below` rows below it, as one character
    /// if it is one row and pieces if it is more.
    fn delim(&self, delim: &str, above: usize, below: usize) -> Block {
        let name = delim.strip_prefix('\\');
        let found = match name {
            Some(word) => lookup(Mode::Math, Name::Control(word)),
            None => lookup(Mode::Math, Name::Char(delim)),
        };
        let shown = found.map_or(delim.strip_prefix('\\').unwrap_or(delim), |s| s.replace);
        let Some(c) = shown.chars().next().filter(|c| *c != '.') else {
            return Block::default();
        };
        let height = above + below + 1;
        let mut rows = Vec::with_capacity(height);
        for r in 0..height {
            let at = if height == 1 {
                Piece::Whole
            } else if r == 0 {
                Piece::Top
            } else if r + 1 == height {
                Piece::Bottom
            } else if height >= 3 && r == height / 2 {
                Piece::Middle
            } else {
                Piece::Extender
            };
            rows.push(vec![delim_char(c, at)]);
        }
        Block {
            rows,
            base: above,
            width: 1,
        }
    }

    fn left_right(&self, body: &[Node<'_>], left: &str, right: &str, d: bool) -> Block {
        // The body in segments split at `\middle`, with the delimiters between them.
        let mut segments: Vec<Block> = Vec::new();
        let mut middles: Vec<&str> = Vec::new();
        let mut start = 0;
        for (i, node) in body.iter().enumerate() {
            if let NodeKind::Middle { delim } = &node.kind {
                if let Some(chunk) = body.get(start..i) {
                    segments.push(self.list(chunk, d));
                }
                middles.push(delim);
                start = i + 1;
            }
        }
        if let Some(chunk) = body.get(start..) {
            segments.push(self.list(chunk, d));
        }
        let live = || segments.iter().filter(|s| !s.is_empty());
        let above = live().map(|s| s.base).max().unwrap_or(0);
        let below = live().map(Block::below).max().unwrap_or(0);
        let mut parts = vec![self.delim(left, above, below)];
        let mut middles = middles.into_iter();
        for segment in segments {
            parts.push(segment);
            if let Some(m) = middles.next() {
                parts.push(self.delim(m, above, below));
            }
        }
        parts.push(self.delim(right, above, below));
        Block::hcat(parts)
    }

    fn accent(&self, label: &str, base: &Node<'_>, d: bool) -> Block {
        let block = self.node(base, d);
        let Some(mark) = accent_mark(label) else {
            return block;
        };
        let mut row = vec![' '; block.width];
        let at = block.width.saturating_sub(1) / 2;
        for (k, c) in mark.chars().enumerate() {
            if let Some(cell) = row.get_mut(at + k) {
                *cell = c;
            }
        }
        let mut top = Block {
            rows: vec![row],
            base: 0,
            width: block.width,
        };
        if top.width == 0 {
            top = Block::default();
        }
        let base = block.base + usize::from(!top.is_empty());
        let mut out = Block::stack(vec![top, block], 1, Align::Left);
        out.base = base;
        out
    }

    fn accent_under(&self, label: &str, base: &Node<'_>, d: bool) -> Block {
        let block = self.node(base, d);
        let w = block.width.max(1);
        let mut line = vec!['\u{2500}'; w];
        if label.contains("rightarrow") {
            if let Some(last) = line.last_mut() {
                *last = '\u{2192}';
            }
        } else if label.contains("leftarrow")
            && let Some(first) = line.first_mut()
        {
            *first = '\u{2190}';
        }
        let row = Block {
            rows: vec![line],
            base: 0,
            width: w,
        };
        Block::stack(vec![block, row], 0, Align::Left)
    }

    fn xarrow(&self, label: &str, body: &Node<'_>, below: Option<&Node<'_>>) -> Block {
        let over = self.node(body, false);
        let under = below.map(|b| self.node(b, false)).unwrap_or_default();
        let n = over.width.max(under.width) + 2;
        let double = label.contains("Rightarrow")
            || label.contains("Leftarrow")
            || label.contains("Leftrightarrow")
            || label.contains("longequal");
        let line = if double { '\u{2550}' } else { '\u{2500}' };
        let mut cells = vec![line; n];
        let has_right = label.contains("ightarrow") || label.contains("ightleft");
        let has_left = label.contains("eftarrow") || label.contains("leftright");
        let (lh, rh) = if double {
            ('\u{21d0}', '\u{21d2}')
        } else {
            ('\u{2190}', '\u{2192}')
        };
        if has_right && let Some(c) = cells.last_mut() {
            *c = rh;
        }
        if has_left && let Some(c) = cells.first_mut() {
            *c = lh;
        }
        let arrow = Block {
            rows: vec![cells],
            base: 0,
            width: n,
        };
        let at = over.height();
        Block::stack(vec![over, arrow, under], at, Align::Center)
    }

    fn array(&self, array: &Array<'_>, d: bool) -> Block {
        let cells: Vec<Vec<Block>> = array
            .body
            .iter()
            .map(|row| row.iter().map(|c| self.node(c, d)).collect())
            .collect();
        let columns = cells.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> = (0..columns)
            .map(|j| {
                cells
                    .iter()
                    .filter_map(|row| row.get(j))
                    .map(|c| c.width)
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        // The column specs split into alignments and the rules between the columns.
        let specs = array.cols.as_deref().unwrap_or(&[]);
        let mut aligns: Vec<(Align, Option<f64>, Option<f64>)> = Vec::new();
        let mut rules: Vec<bool> = vec![false; columns + 1];
        for spec in specs {
            match spec {
                AlignSpec::Align {
                    align,
                    pregap,
                    postgap,
                } => aligns.push((
                    match align {
                        'l' => Align::Left,
                        'r' => Align::Right,
                        _ => Align::Center,
                    },
                    *pregap,
                    *postgap,
                )),
                AlignSpec::Separator(_) => {
                    if let Some(r) = rules.get_mut(aligns.len()) {
                        *r = true;
                    }
                }
            }
        }
        let tall = cells.iter().flatten().any(|c| c.height() > 1);
        let spacing = |j: usize| -> usize {
            let post = aligns.get(j).and_then(|a| a.2);
            let pre = aligns.get(j + 1).and_then(|a| a.1);
            let rule = usize::from(rules.get(j + 1).copied().unwrap_or(false));
            if post.is_none() && pre.is_none() {
                2 + rule
            } else {
                ((post.unwrap_or(0.0) + pre.unwrap_or(0.0)) * 2.0)
                    .round()
                    .max(0.0) as usize
                    + rule
            }
        };
        let total: usize = widths.iter().sum::<usize>()
            + (0..columns.saturating_sub(1)).map(&spacing).sum::<usize>();
        let mut rows: Vec<Block> = Vec::new();
        let mut table_width = 0;
        let hline = |at: usize| {
            array
                .hlines_before_row
                .get(at)
                .is_some_and(|h| !h.is_empty())
        };
        for (i, row) in cells.into_iter().enumerate() {
            let mut parts: Vec<Block> = Vec::new();
            for (j, cell) in row.into_iter().enumerate() {
                let align = aligns.get(j).map_or(Align::Center, |a| a.0);
                let w = widths.get(j).copied().unwrap_or(0);
                if j > 0 {
                    parts.push(Block::text(&" ".repeat(spacing(j - 1))));
                }
                parts.push(cell.padded(w, align));
            }
            let mut block = Block::hcat(parts);
            if rules.iter().any(|r| *r) {
                block = with_rules(block, &widths, &rules, &spacing);
            }
            table_width = table_width.max(block.width);
            if i > 0 && tall {
                rows.push(Block {
                    rows: vec![Vec::new()],
                    base: 0,
                    width: 0,
                });
            }
            if hline(i) {
                rows.push(Block::filled('\u{2500}', total, 1, 0));
            }
            rows.push(block);
        }
        if hline(array.body.len()) {
            rows.push(Block::filled('\u{2500}', total, 1, 0));
        }
        let mut table = Block::stack(rows, 0, Align::Left);
        // A table sits on the middle row.
        table.base = table.height() / 2;
        table.width = table.width.max(table_width);
        table
    }
}

/// Vertical rules between the columns of one row, drawn the height of the row.
fn with_rules(
    mut row: Block,
    widths: &[usize],
    rules: &[bool],
    spacing: &dyn Fn(usize) -> usize,
) -> Block {
    let mut x = 0;
    for (j, w) in widths.iter().enumerate() {
        let gap = spacing(j);
        if rules.get(j + 1).copied().unwrap_or(false) {
            let at = x + w + gap / 2;
            for cells in &mut row.rows {
                if let Some(c) = cells.get_mut(at) {
                    *c = '\u{2502}';
                }
            }
        }
        x += w + gap;
    }
    row
}

/// Whether a node's scripts go above and below it.
fn stacked(base: &Node<'_>, display: bool) -> bool {
    match &base.kind {
        NodeKind::Op {
            limits,
            always_handle_supsub,
            ..
        }
        | NodeKind::OperatorName {
            limits,
            always_handle_supsub,
            ..
        } => *limits && (display || *always_handle_supsub),
        NodeKind::HorizBrace { .. } | NodeKind::XArrow { .. } => true,
        _ => false,
    }
}

/// A script as a block: Unicode characters if there are some for all of it, else the `^` or `_`
/// form, in parentheses if it is more than a character.
fn script(block: Block, sup: bool) -> (Block, bool) {
    if block.is_empty() {
        return (block, true);
    }
    if sup
        && let Some(row) = block.one_row()
        && row
            .chars()
            .all(|c| matches!(c, '\u{2032}' | '\u{2033}' | '\u{2034}'))
    {
        return (block, true);
    }
    if let Some(s) = script_string(&block, sup) {
        return (Block::text(&s), true);
    }
    let marker = Block::text(if sup { "^" } else { "_" });
    if block.width <= 1 && block.height() == 1 {
        return (Block::hcat(vec![marker, block]), false);
    }
    (
        Block::hcat(vec![marker, Block::text("("), block, Block::text(")")]),
        false,
    )
}

/// The one row of a block in super- or subscript characters, if there is one row and each
/// character has such a form.
fn script_string(block: &Block, sup: bool) -> Option<String> {
    let row = block.one_row()?;
    row.chars()
        .map(|c| if sup { sup_char(c) } else { sub_char(c) })
        .collect()
}

fn sup_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '\u{2070}',
        '1' => '\u{b9}',
        '2' => '\u{b2}',
        '3' => '\u{b3}',
        '4'..='9' => char::from_u32(0x2070 + (c as u32 - '0' as u32))?,
        '+' => '\u{207a}',
        '-' | '\u{2212}' => '\u{207b}',
        '=' => '\u{207c}',
        '(' => '\u{207d}',
        ')' => '\u{207e}',
        'a' => '\u{1d43}',
        'b' => '\u{1d47}',
        'c' => '\u{1d9c}',
        'd' => '\u{1d48}',
        'e' => '\u{1d49}',
        'f' => '\u{1da0}',
        'g' => '\u{1d4d}',
        'h' => '\u{2b0}',
        'i' => '\u{2071}',
        'j' => '\u{2b2}',
        'k' => '\u{1d4f}',
        'l' => '\u{2e1}',
        'm' => '\u{1d50}',
        'n' => '\u{207f}',
        'o' => '\u{1d52}',
        'p' => '\u{1d56}',
        'r' => '\u{2b3}',
        's' => '\u{2e2}',
        't' => '\u{1d57}',
        'u' => '\u{1d58}',
        'v' => '\u{1d5b}',
        'w' => '\u{2b7}',
        'x' => '\u{2e3}',
        'y' => '\u{2b8}',
        'z' => '\u{1dbb}',
        'A' => '\u{1d2c}',
        'B' => '\u{1d2e}',
        'D' => '\u{1d30}',
        'E' => '\u{1d31}',
        'G' => '\u{1d33}',
        'H' => '\u{1d34}',
        'I' => '\u{1d35}',
        'J' => '\u{1d36}',
        'K' => '\u{1d37}',
        'L' => '\u{1d38}',
        'M' => '\u{1d39}',
        'N' => '\u{1d3a}',
        'O' => '\u{1d3c}',
        'P' => '\u{1d3e}',
        'R' => '\u{1d3f}',
        'T' => '\u{1d40}',
        'U' => '\u{1d41}',
        'V' => '\u{2c7d}',
        'W' => '\u{1d42}',
        '\u{3b2}' => '\u{1d5d}',
        '\u{3b3}' => '\u{1d5e}',
        '\u{3b4}' => '\u{1d5f}',
        '\u{3b8}' => '\u{1dbf}',
        '\u{3c6}' => '\u{1d60}',
        '\u{3c7}' => '\u{1d61}',
        _ => return None,
    })
}

fn sub_char(c: char) -> Option<char> {
    Some(match c {
        '0'..='9' => char::from_u32(0x2080 + (c as u32 - '0' as u32))?,
        '+' => '\u{208a}',
        '-' | '\u{2212}' => '\u{208b}',
        '=' => '\u{208c}',
        '(' => '\u{208d}',
        ')' => '\u{208e}',
        'a' => '\u{2090}',
        'e' => '\u{2091}',
        'h' => '\u{2095}',
        'i' => '\u{1d62}',
        'j' => '\u{2c7c}',
        'k' => '\u{2096}',
        'l' => '\u{2097}',
        'm' => '\u{2098}',
        'n' => '\u{2099}',
        'o' => '\u{2092}',
        'p' => '\u{209a}',
        'r' => '\u{1d63}',
        's' => '\u{209b}',
        't' => '\u{209c}',
        'u' => '\u{1d64}',
        'v' => '\u{1d65}',
        'x' => '\u{2093}',
        '\u{3b2}' => '\u{1d66}',
        '\u{3b3}' => '\u{1d67}',
        '\u{3c1}' => '\u{1d68}',
        '\u{3c6}' => '\u{1d69}',
        '\u{3c7}' => '\u{1d6a}',
        _ => return None,
    })
}

/// A large operator's character, or the name of a text operator, from its command.
fn op_text(name: &str) -> String {
    let word = name.strip_prefix('\\').unwrap_or(name);
    match lookup(Mode::Math, Name::Control(word)) {
        Some(symbol) => plain_space(symbol.replace),
        None => word.to_owned(),
    }
}

/// Spaces of any width as a plain space.
fn plain_space(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{a0}' | '\u{2002}'..='\u{200a}' | '\u{202f}' | '\u{205f}' => ' ',
            c => c,
        })
        .collect()
}

fn double_struck(c: char) -> char {
    match c {
        'C' => '\u{2102}',
        'H' => '\u{210d}',
        'N' => '\u{2115}',
        'P' => '\u{2119}',
        'Q' => '\u{211a}',
        'R' => '\u{211d}',
        'Z' => '\u{2124}',
        'A'..='Z' => char::from_u32(0x1d538 + (c as u32 - 'A' as u32)).unwrap_or(c),
        'a'..='z' => char::from_u32(0x1d552 + (c as u32 - 'a' as u32)).unwrap_or(c),
        '0'..='9' => char::from_u32(0x1d7d8 + (c as u32 - '0' as u32)).unwrap_or(c),
        c => c,
    }
}

/// The mark an accent shows over its base, from the command.
fn accent_mark(label: &str) -> Option<&'static str> {
    Some(match label.strip_prefix('\\').unwrap_or(label) {
        "hat" | "widehat" | "^" => "^",
        "tilde" | "widetilde" | "utilde" | "~" => "~",
        "bar" | "=" | "overline" => "\u{af}",
        "acute" | "'" => "\u{b4}",
        "grave" | "`" => "`",
        "dot" | "." => ".",
        "ddot" | "\"" => "\u{a8}",
        "breve" | "u" => "\u{2d8}",
        "check" | "v" => "\u{2c7}",
        "mathring" | "r" => "\u{2da}",
        "H" => "\u{2dd}",
        "vec" | "overrightarrow" | "Overrightarrow" => "\u{2192}",
        "overleftarrow" => "\u{2190}",
        "overleftrightarrow" => "\u{2194}",
        _ => return None,
    })
}

/// The row of a bracket or brace over or under a base `width` wide.
fn bracket_line(width: usize, over: bool, brace: bool) -> Block {
    let (left, mid, right) = match (over, brace) {
        (true, true) => ('\u{256d}', '\u{2534}', '\u{256e}'),
        (false, true) => ('\u{2570}', '\u{252c}', '\u{256f}'),
        (true, false) => ('\u{250c}', '\u{2500}', '\u{2510}'),
        (false, false) => ('\u{2514}', '\u{2500}', '\u{2518}'),
    };
    let mut cells = vec!['\u{2500}'; width.max(1)];
    if width >= 2 {
        if let Some(c) = cells.first_mut() {
            *c = left;
        }
        if let Some(c) = cells.last_mut() {
            *c = right;
        }
        if brace && width >= 3 {
            let at = width / 2;
            if let Some(c) = cells.get_mut(at) {
                *c = mid;
            }
        }
    }
    Block {
        width: cells.len(),
        rows: vec![cells],
        base: 0,
    }
}

/// A frame around a block.
fn boxed(body: Block) -> Block {
    let w = body.width;
    let mut rows = Vec::with_capacity(body.height() + 2);
    let mut top = vec!['\u{250c}'];
    top.extend(std::iter::repeat_n('\u{2500}', w));
    top.push('\u{2510}');
    rows.push(top);
    for cells in &body.rows {
        let mut row = vec!['\u{2502}'];
        row.extend(cells);
        row.push('\u{2502}');
        rows.push(row);
    }
    let mut bottom = vec!['\u{2514}'];
    bottom.extend(std::iter::repeat_n('\u{2500}', w));
    bottom.push('\u{2518}');
    rows.push(bottom);
    Block {
        rows,
        base: body.base + 1,
        width: w + 2,
    }
}

/// Which row of a tall delimiter.
#[derive(Clone, Copy)]
enum Piece {
    Whole,
    Top,
    Middle,
    Bottom,
    Extender,
}

/// The character of one row of a delimiter `c`, of the left side or the right.
fn delim_char(c: char, at: Piece) -> char {
    use Piece::*;
    match (c, at) {
        (_, Whole) => c,
        ('(', Top) => '\u{239b}',
        ('(', Extender | Middle) => '\u{239c}',
        ('(', Bottom) => '\u{239d}',
        (')', Top) => '\u{239e}',
        (')', Extender | Middle) => '\u{239f}',
        (')', Bottom) => '\u{23a0}',
        ('[' | '\u{2308}', Top) => '\u{23a1}',
        ('[' | '\u{2308}' | '\u{230a}', Extender | Middle) | ('\u{230a}', Top) => '\u{23a2}',
        ('[' | '\u{230a}', Bottom) => '\u{23a3}',
        ('\u{2308}', Bottom) => '\u{23a2}',
        (']' | '\u{2309}', Top) => '\u{23a4}',
        (']' | '\u{2309}' | '\u{230b}', Extender | Middle) | ('\u{230b}', Top) => '\u{23a5}',
        (']' | '\u{230b}', Bottom) => '\u{23a6}',
        ('\u{2309}', Bottom) => '\u{23a5}',
        ('{', Top) => '\u{23a7}',
        ('{', Middle) => '\u{23a8}',
        ('{', Extender) => '\u{23aa}',
        ('{', Bottom) => '\u{23a9}',
        ('}', Top) => '\u{23ab}',
        ('}', Middle) => '\u{23ac}',
        ('}', Extender) => '\u{23aa}',
        ('}', Bottom) => '\u{23ad}',
        ('\u{27e8}', Top) => '\u{2571}',
        ('\u{27e8}', Bottom) => '\u{2572}',
        ('\u{27e9}', Top) => '\u{2572}',
        ('\u{27e9}', Bottom) => '\u{2571}',
        ('\u{27e8}' | '\u{27e9}', _) => '\u{2502}',
        ('|' | '\u{2223}', _) => '\u{2502}',
        ('\u{2225}' | '\u{2016}', _) => '\u{2551}',
        _ => c,
    }
}

/// The length of a measurement in em.
fn em_of(m: &Measurement) -> f64 {
    match m.unit.as_str() {
        "em" => m.number,
        "ex" => m.number * 0.43,
        "mu" => m.number / 18.0,
        _ => m.number / 10.0,
    }
}

/// The length of a measurement in cells (two to the em).
fn cells(m: &Measurement) -> usize {
    let n = em_of(m) * 2.0;
    // A thin space is one cell, not none.
    let n = if n > 0.25 && n < 1.0 { 1.0 } else { n.round() };
    if n.is_finite() && n > 0.0 {
        (n as usize).min(40)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(src: &str) -> String {
        ascii_formula(src, MathStyle::Text).unwrap_or_else(|e| format!("error: {e}"))
    }

    /// Source, whether it is set as a display, and the expected rows.
    const CASES: &[(&str, bool, &str)] = &[
        (r"\frac{1}{2}", false, " 1\n---\n 2"),
        (r"\frac{a+b}{c}", false, " a + b\n-------\n   c"),
        (
            r"\frac{1}{1+\frac{1}{x}}",
            false,
            "    1\n---------\n      1\n 1 + ---\n      x",
        ),
        (
            r"x=\frac{-b}{2a}",
            false,
            "     \u{2212}b\nx = ----\n     2a",
        ),
        (
            r"\binom{n}{k}",
            false,
            "\u{239b}n\u{239e}\n\u{239d}k\u{23a0}",
        ),
        (
            r"\frac{\frac{a}{b}}{\frac{c}{d}}",
            false,
            "  a\n ---\n  b\n-----\n  c\n ---\n  d",
        ),
        ("x^2", false, "x\u{b2}"),
        ("x_i", false, "x\u{1d62}"),
        ("x_i^2", false, "x\u{1d62}\u{b2}"),
        ("a_{n+1}", false, "a\u{2099}\u{208a}\u{2081}"),
        ("x^{a+b}", false, "x\u{1d43}\u{207a}\u{1d47}"),
        ("x^q", false, "x^q"),
        ("x_{ab}^{qr}", false, "x^(qr)_(ab)"),
        ("e^{-x^2}", false, "e^(\u{2212}x\u{b2})"),
        ("f'(x)", false, "f\u{2032}(x)"),
        (r"x^{\frac{1}{2}}", false, "    1\nx^(---)\n    2"),
        (r"\sqrt{x}", false, " \u{af}\n\u{221a}x"),
        (
            r"\sqrt{x+1}",
            false,
            " \u{af}\u{af}\u{af}\u{af}\u{af}\n\u{221a}x + 1",
        ),
        (r"\sqrt[3]{x}", false, "  \u{af}\n\u{b3}\u{221a}x"),
        (
            r"\sqrt{\frac{a}{b}}",
            false,
            " \u{af}\u{af}\u{af}\n  a\n\u{221a}---\n  b",
        ),
        (
            r"\sum_{i=0}^{n} i",
            false,
            "\u{2211}\u{1d62}\u{208c}\u{2080}\u{207f} i",
        ),
        (r"\sum_{i=0}^{n} i", true, "  n\n  \u{2211}   i\ni = 0"),
        (r"\int_0^1 x\,dx", false, "\u{222b}\u{2080}\u{b9} x dx"),
        (r"\int_0^1 x\,dx", true, "\u{222b}\u{2080}\u{b9} x dx"),
        (r"\lim_{x\to 0} f(x)", true, " lim  f(x)\nx \u{2192} 0"),
        (r"\prod_{k=1}^{n} k", true, "  n\n  \u{220f}   k\nk = 1"),
        (
            r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
            false,
            "\u{239b}a  b\u{239e}\n\u{239d}c  d\u{23a0}",
        ),
        (
            r"\begin{bmatrix} 1 & 0 & 0 \\ 0 & 1 & 0 \\ 0 & 0 & 1 \end{bmatrix}",
            false,
            "\u{23a1}1  0  0\u{23a4}\n\u{23a2}0  1  0\u{23a5}\n\u{23a3}0  0  1\u{23a6}",
        ),
        (
            r"\begin{vmatrix} a & b \\ c & d \end{vmatrix}",
            false,
            "\u{2502}a  b\u{2502}\n\u{2502}c  d\u{2502}",
        ),
        (
            r"\begin{matrix} x & yy \\ zzz & w \end{matrix}",
            false,
            " x   yy\nzzz  w",
        ),
        (
            r"\begin{cases} x & x > 0 \\ -x & x \le 0 \end{cases}",
            false,
            "\u{23a7}x   x > 0\n\u{23a9}\u{2212}x  x \u{2264} 0",
        ),
        (
            r"\begin{aligned} a &= b + c \\ d &= e \end{aligned}",
            false,
            "a = b + c\nd = e",
        ),
        (r"\hat{x}", false, "^\nx"),
        (r"\vec{v} + \bar{y}", false, "\u{2192}   \u{af}\nv + y"),
        (r"\overline{ab}", false, "\u{af}\u{af}\nab"),
        (r"\underline{ab}", false, "ab\n__"),
        (r"\text{if } x > 0", false, "if x > 0"),
        (r"x \text{ and } y", false, "x and y"),
        (r"\mathbb{R}^n", false, "\u{211d}\u{207f}"),
        (
            r"\left( \frac{a}{b} \right)",
            false,
            "\u{239b} a \u{239e}\n\u{239c}---\u{239f}\n\u{239d} b \u{23a0}",
        ),
        (
            r"\left[ \frac{a}{b} \right]",
            false,
            "\u{23a1} a \u{23a4}\n\u{23a2}---\u{23a5}\n\u{23a3} b \u{23a6}",
        ),
        (
            r"\left\{ \frac{a}{b} \middle| c \right.",
            false,
            "\u{23a7} a \u{2502}\n\u{23a8}---\u{2502}c\n\u{23a9} b \u{2502}",
        ),
        (
            r"\alpha + \beta \cdot \gamma \times \delta",
            false,
            "\u{3b1} + \u{3b2} \u{22c5} \u{3b3} \u{d7} \u{3b4}",
        ),
        (
            r"a \neq b \Rightarrow c \approx d",
            false,
            "a \u{2260} b \u{21d2} c \u{2248} d",
        ),
        (r"\sin x + \cos(2y)", false, "sin x + cos(2y)"),
        (r"\operatorname{rank} A", false, "rank A"),
        (
            r"\boxed{x}",
            false,
            "\u{250c}\u{2500}\u{2510}\n\u{2502}x\u{2502}\n\u{2514}\u{2500}\u{2518}",
        ),
        (r"a \\ b", false, "a\nb"),
        (r"a \notin B", false, "a \u{2209} B"),
        (r"a \not< b", false, "a \u{226e} b"),
        (
            r"\begin{array}{c|c} a & b \\ \hline c & d \end{array}",
            false,
            "a \u{2502} b\n\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\nc \u{2502} d",
        ),
        (
            r"\left\langle \frac{a}{b} \right\rangle",
            false,
            "\u{2571} a \u{2572}\n\u{2502}---\u{2502}\n\u{2572} b \u{2571}",
        ),
        (r"\mathrm{d}x \; \mathbf{v}", false, "dx v"),
        (
            r"\frac{\sqrt{x}}{2}^2",
            false,
            "  \u{af}\n \u{221a}x\n----\u{b2}\n 2",
        ),
        (
            r"\overbrace{a+b}^{n}",
            false,
            "  n\n\u{256d}\u{2500}\u{2534}\u{2500}\u{256e}\na + b",
        ),
        (
            r"\xrightarrow{f} B",
            false,
            " f\n\u{2500}\u{2500}\u{2192} B",
        ),
        (r"e^{i\pi} + 1 = 0", false, "e^(i\u{3c0}) + 1 = 0"),
        (
            r"\frac{1}{2} + \frac{3}{4}",
            false,
            " 1     3\n--- + ---\n 2     4",
        ),
        (
            r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
            true,
            "           \u{af}\u{af}\u{af}\u{af}\u{af}\u{af}\u{af}\u{af}\n     \u{2212}b \u{b1} \u{221a}b\u{b2} \u{2212} 4ac\nx = ----------------\n           2a",
        ),
    ];

    #[test]
    fn snapshots() {
        let mut bad = String::new();
        for (src, is_display, expected) in CASES {
            let style = if *is_display {
                MathStyle::Display
            } else {
                MathStyle::Text
            };
            let got = ascii_formula(src, style).unwrap_or_else(|e| format!("error: {e}"));
            if got != *expected {
                bad.push_str(&format!("{src} (display {is_display})\n{got}\n===\n"));
            }
        }
        assert!(bad.is_empty(), "\n{bad}");
    }

    #[test]
    fn a_bad_formula_is_an_error_and_an_empty_one_is_empty() {
        assert!(ascii_formula(r"\frac{1}", MathStyle::Text).is_err());
        assert!(ascii_formula(r"\nosuchcommand", MathStyle::Display).is_err());
        assert_eq!(text(""), "");
        assert_eq!(text("{}"), "");
    }

    #[test]
    fn nodes_without_a_drawing_do_not_panic() {
        for src in [
            r"\rule{1em}{2em}",
            r"\kern1em x \mskip3mu",
            r"\phantom{abc}x \vphantom{\frac12}",
            r"\smash{x}\mathllap{y}\raisebox{1em}{z}",
            r"\href{http://example.com}{x}",
            r"\color{red} x",
            r"\textcolor{red}{x}\colorbox{red}{y}\cancel{z}",
            r"\mathchoice{a}{b}{c}{d}",
            r"\verb|a b|",
            r"\underbrace{x}_{n}\overleftarrow{ab}\underrightarrow{c}",
            r"\xleftrightarrow[b]{a}",
            r"\tag{1} x",
            r"\begin{CD} A @>f>> B \\ @VVV @VVV \\ C @>>> D \end{CD}",
            r"\begin{array}{l|r:c} a & b & c \\ \hline d & e & f \end{array}",
            r"\sqrt[\frac12]{x}",
            r"\left. \frac12 \right|_0^1",
            r"\mathbb{abcXYZ019}\mathcal{A}",
            r"\sideset{_a}{_b}\sum",
            r"{}^{14}_{6}C",
        ] {
            for style in [MathStyle::Text, MathStyle::Display] {
                if let Ok(out) = ascii_formula(src, style) {
                    assert!(!out.contains('\r'), "{src}");
                }
            }
        }
    }

    #[test]
    fn rows_of_mixed_height_share_a_baseline() {
        // The relation sits on the fraction bar, and the tall scripts hang from it.
        assert_eq!(
            text(r"1 + \frac{a}{b} = \frac{c}{d}"),
            "     a     c\n1 + --- = ---\n     b     d"
        );
        assert_eq!(
            text(r"\sqrt{\frac{a}{b}} + y"),
            " \u{af}\u{af}\u{af}\n  a\n\u{221a}--- + y\n  b"
        );
    }

    #[test]
    fn plain_text_and_symbols() {
        assert_eq!(text("x"), "x");
        assert_eq!(text("a+b"), "a + b");
        assert_eq!(text("a=b"), "a = b");
        assert_eq!(text("-x"), "\u{2212}x");
        assert_eq!(text(r"\alpha+\beta"), "\u{3b1} + \u{3b2}");
        assert_eq!(text(r"a\leq b"), "a \u{2264} b");
        assert_eq!(text(r"\infty"), "\u{221e}");
        assert_eq!(text(r"x\in A"), "x \u{2208} A");
        assert_eq!(text("f(x,y)"), "f(x, y)");
        assert_eq!(text(r"a\,b\quad c"), "a b  c");
    }
}
