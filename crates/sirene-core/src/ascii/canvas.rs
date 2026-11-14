//! A character grid where line segments merge into junctions (ASCIIR-4).

use unicode_width::UnicodeWidthChar;

use super::Charset;

pub(super) const UP: u8 = 1;
pub(super) const DOWN: u8 = 2;
pub(super) const LEFT: u8 = 4;
pub(super) const RIGHT: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Line {
    Light,
    Heavy,
    Double,
    Dotted,
}

/// A character with its Unicode and ASCII forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Glyph {
    pub unicode: char,
    pub ascii: char,
}

pub(super) const fn glyph(unicode: char, ascii: char) -> Glyph {
    Glyph { unicode, ascii }
}

#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    mask: u8,
    line: Option<Line>,
    /// Shown instead of the line character while the mask is unchanged.
    glyph: Option<(Glyph, u8)>,
    arrow: Option<Glyph>,
    text: Option<char>,
    /// The second cell of a wide character.
    tail: bool,
}

/// The most cells a canvas holds (ASCIIR-4). A larger drawing keeps its top rows.
const MAX_CELLS: usize = 1 << 22;

pub(super) struct Canvas {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
    /// The size asked for, when it was more than `MAX_CELLS` and the canvas is cut off.
    cut: Option<(i32, i32)>,
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Self {
        Self::with_limit(width, height, MAX_CELLS)
    }

    fn with_limit(width: i32, height: i32, limit: usize) -> Self {
        let (w, h) = (width.max(0) as usize, height.max(0) as usize);
        let columns = w.min(limit);
        let rows = h.min(limit / columns.max(1));
        Canvas {
            width: columns as i32,
            height: rows as i32,
            cells: vec![Cell::default(); columns * rows],
            cut: (columns < w || rows < h).then_some((width, height)),
        }
    }

    /// The columns kept, after any cut.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// The rows kept, after any cut.
    pub fn height(&self) -> i32 {
        self.height
    }

    fn cell(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get_mut((y * self.width + x) as usize)
    }

    /// Adds line directions to a cell. Lines of different styles meeting make a light junction.
    pub fn add(&mut self, x: i32, y: i32, bits: u8, line: Line) {
        if let Some(c) = self.cell(x, y) {
            c.mask |= bits;
            c.line = match c.line {
                Some(l) if l != line => Some(Line::Light),
                _ => Some(line),
            };
        }
    }

    /// A horizontal line from `x0` to `x1` inclusive.
    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, line: Line) {
        for x in x0.max(0)..=x1.min(self.width - 1) {
            let bits = if x > x0 { LEFT } else { 0 } | if x < x1 { RIGHT } else { 0 };
            self.add(x, y, bits, line);
        }
    }

    /// A vertical line from `y0` to `y1` inclusive.
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, line: Line) {
        for y in y0.max(0)..=y1.min(self.height - 1) {
            let bits = if y > y0 { UP } else { 0 } | if y < y1 { DOWN } else { 0 };
            self.add(x, y, bits, line);
        }
    }

    /// Empties the cells from `(x0, y0)` to `(x1, y1)` inclusive, so what is drawn there next
    /// covers what was under it.
    pub fn clear(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0.max(0)..=y1.min(self.height - 1) {
            for x in x0.max(0)..=x1.min(self.width - 1) {
                if let Some(c) = self.cell(x, y) {
                    *c = Cell::default();
                }
            }
        }
    }

    /// Shows `g` at a cell until more lines join it.
    pub fn glyph(&mut self, x: i32, y: i32, g: Glyph) {
        if let Some(c) = self.cell(x, y) {
            c.glyph = Some((g, c.mask));
        }
    }

    pub fn arrow(&mut self, x: i32, y: i32, g: Glyph) {
        if let Some(c) = self.cell(x, y) {
            c.arrow = Some(g);
        }
    }

    /// Writes text from `(x, y)`; wide characters take two cells and zero-width ones are
    /// dropped. A wide character that does not fit whole is left out. Returns the next column.
    pub fn text(&mut self, mut x: i32, y: i32, s: &str) -> i32 {
        for ch in s.chars() {
            let w = ch.width().unwrap_or(0) as i32;
            if w == 0 {
                continue;
            }
            let fits = x >= 0 && x.saturating_add(w) <= self.width;
            if fits {
                for k in 0..w {
                    self.release(x + k, y);
                }
                if let Some(c) = self.cell(x, y) {
                    c.text = Some(ch);
                    c.tail = false;
                }
                if w == 2
                    && let Some(c) = self.cell(x + 1, y)
                {
                    c.text = None;
                    c.tail = true;
                }
            }
            x = x.saturating_add(w);
        }
        x
    }

    /// Splits the wide character covering `(x, y)`, if any, so the cell can be written alone:
    /// its other half becomes a space.
    fn release(&mut self, x: i32, y: i32) {
        let Some(c) = self.cell(x, y) else {
            return;
        };
        let (tail, head) = (c.tail, c.text.is_some_and(|t| t.width() == Some(2)));
        if tail && let Some(h) = self.cell(x - 1, y) {
            h.text = Some(' ');
        }
        if head && let Some(t) = self.cell(x + 1, y) {
            t.tail = false;
            t.text = Some(' ');
        }
    }

    pub fn render(&self, charset: Charset) -> String {
        let mut out = String::new();
        for row in self.cells.chunks(self.width.max(1) as usize) {
            let mut line = String::new();
            for c in row {
                if c.tail {
                    continue;
                }
                line.push(char_of(c, charset));
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        if let Some((width, height)) = self.cut {
            out.push_str(&format!(
                "... cut off: the drawing is {width} columns by {height} rows\n"
            ));
        }
        out
    }
}

fn pick(g: Glyph, charset: Charset) -> char {
    match charset {
        Charset::Unicode => g.unicode,
        Charset::Ascii => g.ascii,
    }
}

fn char_of(c: &Cell, charset: Charset) -> char {
    if let Some(t) = c.text {
        return t;
    }
    if let Some(a) = c.arrow {
        return pick(a, charset);
    }
    match c.glyph {
        Some((g, mask)) if mask == c.mask => return pick(g, charset),
        _ => {}
    }
    if c.mask == 0 {
        return ' ';
    }
    let line = c.line.unwrap_or(Line::Light);
    match charset {
        Charset::Unicode => unicode(c.mask, line),
        Charset::Ascii => ascii(c.mask, line),
    }
}

fn unicode(mask: u8, line: Line) -> char {
    if mask & (UP | DOWN) == 0 {
        return match line {
            Line::Light => '─',
            Line::Heavy => '━',
            Line::Double => '═',
            Line::Dotted => '┄',
        };
    }
    if mask & (LEFT | RIGHT) == 0 {
        return match line {
            Line::Light => '│',
            Line::Heavy => '┃',
            Line::Double => '║',
            Line::Dotted => '┆',
        };
    }
    let table: [char; 9] = match line {
        Line::Light | Line::Dotted => ['┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼'],
        Line::Heavy => ['┏', '┓', '┗', '┛', '┣', '┫', '┳', '┻', '╋'],
        Line::Double => ['╔', '╗', '╚', '╝', '╠', '╣', '╦', '╩', '╬'],
    };
    let i = match mask {
        m if m == DOWN | RIGHT => 0,
        m if m == DOWN | LEFT => 1,
        m if m == UP | RIGHT => 2,
        m if m == UP | LEFT => 3,
        m if m == UP | DOWN | RIGHT => 4,
        m if m == UP | DOWN | LEFT => 5,
        m if m == LEFT | RIGHT | DOWN => 6,
        m if m == LEFT | RIGHT | UP => 7,
        _ => 8,
    };
    table[i]
}

fn ascii(mask: u8, line: Line) -> char {
    if mask & (UP | DOWN) == 0 {
        return match line {
            Line::Light => '-',
            Line::Heavy | Line::Double => '=',
            Line::Dotted => '.',
        };
    }
    if mask & (LEFT | RIGHT) == 0 {
        return match line {
            Line::Dotted => ':',
            _ => '|',
        };
    }
    '+'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(c: &Canvas) -> (String, String) {
        (c.render(Charset::Unicode), c.render(Charset::Ascii))
    }

    #[test]
    fn boxes_and_junctions() {
        let mut c = Canvas::new(5, 3);
        c.hline(0, 4, 0, Line::Light);
        c.hline(0, 4, 2, Line::Light);
        c.vline(0, 0, 2, Line::Light);
        c.vline(4, 0, 2, Line::Light);
        c.vline(2, 0, 2, Line::Light);
        assert_eq!(
            show(&c),
            (
                "┌─┬─┐\n│ │ │\n└─┴─┘\n".to_owned(),
                "+-+-+\n| | |\n+-+-+\n".to_owned()
            )
        );
        c.hline(0, 4, 1, Line::Light);
        assert_eq!(c.render(Charset::Unicode), "┌─┬─┐\n├─┼─┤\n└─┴─┘\n");
    }

    #[test]
    fn line_styles() {
        let mut c = Canvas::new(3, 5);
        c.hline(0, 2, 0, Line::Heavy);
        c.hline(0, 2, 1, Line::Double);
        c.hline(0, 2, 2, Line::Dotted);
        c.vline(0, 3, 4, Line::Light);
        assert_eq!(c.render(Charset::Unicode), "━━━\n═══\n┄┄┄\n│\n│\n");
        assert_eq!(c.render(Charset::Ascii), "===\n===\n...\n|\n|\n");

        let mut c = Canvas::new(3, 3);
        c.vline(0, 0, 2, Line::Heavy);
        c.vline(1, 0, 2, Line::Double);
        c.vline(2, 0, 2, Line::Dotted);
        assert_eq!(c.render(Charset::Unicode), "┃║┆\n┃║┆\n┃║┆\n");
        assert_eq!(c.render(Charset::Ascii), "||:\n||:\n||:\n");
    }

    #[test]
    fn junction_tables() {
        for (line, expected) in [
            (Line::Heavy, "┏┳┓\n┣╋┫\n┗┻┛\n"),
            (Line::Double, "╔╦╗\n╠╬╣\n╚╩╝\n"),
            (Line::Dotted, "┌┬┐\n├┼┤\n└┴┘\n"),
        ] {
            let mut c = Canvas::new(3, 3);
            for i in 0..3 {
                c.hline(0, 2, i, line);
                c.vline(i, 0, 2, line);
            }
            assert_eq!(c.render(Charset::Unicode), expected);
        }
    }

    #[test]
    fn mixed_styles_make_light_junctions() {
        let mut c = Canvas::new(3, 3);
        c.hline(0, 2, 1, Line::Heavy);
        c.vline(1, 0, 2, Line::Double);
        assert_eq!(c.render(Charset::Unicode), " ║\n━┼━\n ║\n");
    }

    #[test]
    fn glyphs_yield_to_new_lines() {
        let mut c = Canvas::new(3, 2);
        c.hline(0, 2, 0, Line::Light);
        c.glyph(0, 0, glyph('╭', '.'));
        c.glyph(2, 0, glyph('╮', '.'));
        assert_eq!(c.render(Charset::Unicode), "╭─╮\n\n");
        c.add(2, 0, DOWN, Line::Light);
        c.add(2, 1, UP, Line::Light);
        assert_eq!(c.render(Charset::Unicode), "╭─┐\n  │\n");
    }

    #[test]
    fn text_arrows_and_bounds() {
        let mut c = Canvas::new(4, 1);
        c.hline(0, 3, 0, Line::Light);
        c.arrow(3, 0, glyph('►', '>'));
        assert_eq!(c.render(Charset::Ascii), "--->\n");
        assert_eq!(c.text(0, 0, "a\u{301}b"), 2);
        assert_eq!(c.render(Charset::Unicode), "ab─►\n");
        assert_eq!(c.text(1, 0, "日"), 3);
        assert_eq!(c.render(Charset::Unicode), "a日►\n");
        // Off-canvas writes are ignored.
        c.add(-1, 0, LEFT, Line::Light);
        c.glyph(9, 9, glyph('x', 'x'));
        c.arrow(9, 9, glyph('x', 'x'));
        // A wide character that does not fit whole is left out.
        assert_eq!(c.text(3, 0, "日"), 5);
        assert_eq!(c.text(-1, 0, "日"), 1);
        assert_eq!(c.render(Charset::Unicode), "a日►\n");
    }

    /// Writing over half of a wide character keeps every row its width.
    #[test]
    fn overwriting_wide_characters() {
        let mut c = Canvas::new(6, 1);
        c.text(0, 0, "日本語");
        c.text(1, 0, "x");
        assert_eq!(c.render(Charset::Unicode), " x本語\n");
        c.text(3, 0, "月");
        assert_eq!(c.render(Charset::Unicode), " x 月\n");
        c.text(2, 0, "y");
        assert_eq!(c.render(Charset::Unicode), " xy月\n");
    }

    #[test]
    fn empty_canvas() {
        assert_eq!(Canvas::new(0, 0).render(Charset::Unicode), "");
        assert_eq!(Canvas::new(-1, 3).render(Charset::Unicode), "");
    }

    #[test]
    fn cuts_off_past_the_limit() {
        // Three rows of four cells fit in 12; the fourth row is cut, and drawing there is ignored.
        let mut c = Canvas::with_limit(4, 5, 12);
        c.vline(0, -2, i32::MAX - 1, Line::Light);
        c.hline(i32::MIN + 1, i32::MAX - 1, 1, Line::Light);
        c.clear(2, 2, i32::MAX - 1, i32::MAX - 1);
        c.text(0, 4, "x");
        assert_eq!(
            c.render(Charset::Unicode),
            "│\n┼───\n│\n... cut off: the drawing is 4 columns by 5 rows\n"
        );
        // Wider than the limit: one row of `limit` cells.
        let mut c = Canvas::with_limit(9, 2, 4);
        c.hline(0, 8, 0, Line::Light);
        assert_eq!(
            c.render(Charset::Ascii),
            "----\n... cut off: the drawing is 9 columns by 2 rows\n"
        );
        // A size whose product does not fit in an `i32`.
        let c = Canvas::new(70_000, 70_000);
        assert_eq!(c.cells.len(), 59 * 70_000);
        assert!(c.cut.is_some());
        assert!(Canvas::new(2_048, 2_048).cut.is_none());
    }
}
