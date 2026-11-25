//! The page decorations of a PlantUML diagram (PCOM-1): `header`, `footer`, `caption`, and
//! `legend`, drawn around a diagram of any kind.
//!
//! The text is Creole as far as sirene reads it (PCREO-1): `**bold**`, `//italic//`, `<b>`,
//! `<i>`, `<color:...>`, `== heading` lines, bullet and numbered lists, and `<latex>` and
//! `<math>` formulas (LEMB-2). Each line of text is kept as source; [`lines`] reads it as runs.

use crate::diagram::Diagram;
use crate::flowchart::{Delim, Span, formula};

/// Where a decoration sits across the diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// Where a legend sits down the diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    Top,
    #[default]
    Bottom,
}

/// A `header`, `footer`, or `caption`: lines of Creole text and where they sit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    pub lines: Vec<String>,
    pub align: HAlign,
}

/// A `legend`: lines of Creole text in a bordered box at a corner or edge of the diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Legend {
    pub lines: Vec<String>,
    pub horizontal: HAlign,
    pub vertical: VAlign,
}

/// The decorations a document has. The last of each kind in the document is kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decorations {
    pub header: Option<Banner>,
    pub footer: Option<Banner>,
    pub caption: Option<Banner>,
    pub legend: Option<Legend>,
}

impl Decorations {
    pub fn is_empty(&self) -> bool {
        self.header.is_none()
            && self.footer.is_none()
            && self.caption.is_none()
            && self.legend.is_none()
    }
}

/// A diagram with its decorations around it. Only a diagram that has some is wrapped.
#[derive(Debug, Clone, PartialEq)]
pub struct Decorated {
    pub diagram: Diagram,
    pub decor: Decorations,
}

/// How a line of a decoration starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    None,
    /// A bullet at a nesting level, from 1.
    Bullet(usize),
    /// A number at a nesting level, from 1, and the number.
    Number(usize, usize),
}

impl Marker {
    /// The text a line starts with: indentation and the bullet or number. `bullet` is the
    /// character a bullet is drawn with.
    pub fn prefix(self, bullet: &str) -> String {
        match self {
            Marker::None => String::new(),
            Marker::Bullet(level) => format!("{}{bullet} ", "  ".repeat(level.saturating_sub(1))),
            Marker::Number(level, n) => format!("{}{n}. ", "  ".repeat(level.saturating_sub(1))),
        }
    }
}

/// A run of a line: a span and its `<color:...>`, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub span: Span,
    pub color: Option<String>,
}

/// A line of a decoration read as Creole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub marker: Marker,
    pub runs: Vec<Run>,
}

/// Reads the source `lines` as Creole lines. A list line is `*` or `#` (repeated for a deeper
/// level) and a space, then text; `== text` is a bold heading. Bold, italic, and color carry
/// from one run to the next within a line, and formulas are runs of their own.
pub fn lines(lines: &[String]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut numbers: Vec<usize> = Vec::new();
    for src in lines {
        let (marker, text) = marker(src.trim(), &mut numbers);
        let (text, heading) = heading(text);
        out.push(Line {
            marker,
            runs: runs(text, heading),
        });
    }
    out
}

/// The list marker at the start of `line` and the rest. `numbers` counts each open numbered
/// level, and is cleared by a line that is not a list item.
fn marker<'a>(line: &'a str, numbers: &mut Vec<usize>) -> (Marker, &'a str) {
    let first = line.chars().next();
    if let Some(c @ ('*' | '#')) = first {
        let level = line.chars().take_while(|&d| d == c).count();
        if let Some(rest) = line[level..].strip_prefix(' ') {
            if c == '*' {
                numbers.clear();
                return (Marker::Bullet(level), rest.trim_start());
            }
            numbers.truncate(level);
            numbers.resize(level, 0);
            numbers[level - 1] += 1;
            return (Marker::Number(level, numbers[level - 1]), rest.trim_start());
        }
    }
    numbers.clear();
    (Marker::None, line)
}

/// `== text ==` as its text, and whether it was a heading.
fn heading(line: &str) -> (&str, bool) {
    let eq = line.chars().take_while(|&c| c == '=').count();
    match line[eq..].strip_prefix(' ') {
        Some(rest) if eq >= 2 => (rest.trim_end().trim_end_matches('=').trim_end(), true),
        _ => (line, false),
    }
}

/// The text and formula runs of one line.
fn runs(line: &str, bold_all: bool) -> Vec<Run> {
    let mut state = State {
        bold: bold_all,
        ..State::default()
    };
    let mut out = Vec::new();
    for piece in formula::runs(line, Delim::Latex) {
        if piece.math {
            out.push(Run {
                span: piece,
                color: state.color.clone(),
            });
            continue;
        }
        state.read(&piece.text, bold_all, &mut out);
    }
    out
}

#[derive(Default)]
struct State {
    bold: bool,
    italic: bool,
    color: Option<String>,
}

impl State {
    /// Reads `text`, adding its runs to `out` and changing the state at each marker.
    fn read(&mut self, text: &str, bold_all: bool, out: &mut Vec<Run>) {
        let mut buf = String::new();
        let mut i = 0;
        while i < text.len() {
            let rest = &text[i..];
            let n = self.marker(rest, text[..i].chars().next_back(), bold_all, |s| {
                if !buf.is_empty() {
                    s.push(&mut buf, out);
                }
            });
            match n {
                Some(len) => i += len,
                None => {
                    let c = rest.chars().next().map_or(1, char::len_utf8);
                    buf.push_str(&rest[..c]);
                    i += c;
                }
            }
        }
        self.push(&mut buf, out);
    }

    /// If `rest` starts with a marker, flushes with `flush`, applies it, and gives its length.
    fn marker(
        &mut self,
        rest: &str,
        before: Option<char>,
        bold_all: bool,
        mut flush: impl FnMut(&Self),
    ) -> Option<usize> {
        let lower = rest
            .get(..rest.len().min(8))
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let tag = |t: &str| lower.starts_with(t).then_some(t.len());
        let found = if rest.starts_with("**") {
            Some((2, Change::Bold(!self.bold)))
        } else if rest.starts_with("//") && before != Some(':') {
            Some((2, Change::Italic(!self.italic)))
        } else if let Some(n) = tag("<b>") {
            Some((n, Change::Bold(true)))
        } else if let Some(n) = tag("</b>") {
            Some((n, Change::Bold(bold_all)))
        } else if let Some(n) = tag("<i>") {
            Some((n, Change::Italic(true)))
        } else if let Some(n) = tag("</i>") {
            Some((n, Change::Italic(false)))
        } else if let Some(n) = tag("</color>") {
            Some((n, Change::Color(None)))
        } else if lower.starts_with("<color:") {
            let close = rest.find('>')?;
            let name = rest[7..close].trim();
            // `<color:#red>` and `<color:red>` are both a color.
            Some((close + 1, Change::Color(Some(name.to_owned()))))
        } else {
            None
        };
        let (len, change) = found?;
        flush(self);
        match change {
            Change::Bold(b) => self.bold = b,
            Change::Italic(b) => self.italic = b,
            Change::Color(c) => self.color = c,
        }
        Some(len)
    }

    fn push(&self, buf: &mut String, out: &mut Vec<Run>) {
        if buf.is_empty() {
            return;
        }
        out.push(Run {
            span: Span {
                text: std::mem::take(buf),
                bold: self.bold,
                italic: self.italic,
                ..Span::default()
            },
            color: self.color.clone(),
        });
    }
}

enum Change {
    Bold(bool),
    Italic(bool),
    Color(Option<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Vec<(String, bool, bool, Option<String>, bool)> {
        lines(&[src.to_owned()])[0]
            .runs
            .iter()
            .map(|r| {
                (
                    r.span.text.clone(),
                    r.span.bold,
                    r.span.italic,
                    r.color.clone(),
                    r.span.math,
                )
            })
            .collect()
    }

    #[test]
    fn plain_text_is_one_run() {
        assert_eq!(one("hello"), [("hello".into(), false, false, None, false)]);
    }

    #[test]
    fn bold_and_italic_toggle() {
        assert_eq!(
            one("a **b** //c// d"),
            [
                ("a ".into(), false, false, None, false),
                ("b".into(), true, false, None, false),
                (" ".into(), false, false, None, false),
                ("c".into(), false, true, None, false),
                (" d".into(), false, false, None, false),
            ]
        );
    }

    #[test]
    fn tags_set_bold_italic_and_color() {
        assert_eq!(
            one("<b>x</b><I>y</I><color:red>z</color>w"),
            [
                ("x".into(), true, false, None, false),
                ("y".into(), false, true, None, false),
                ("z".into(), false, false, Some("red".into()), false),
                ("w".into(), false, false, None, false),
            ]
        );
    }

    #[test]
    fn a_url_scheme_is_not_italic() {
        assert_eq!(
            one("http://x.org"),
            [("http://x.org".into(), false, false, None, false)]
        );
    }

    #[test]
    fn style_carries_across_a_formula() {
        let r = one("**a <latex>x</latex> b**");
        assert_eq!(r.len(), 3);
        assert!(r[0].1 && r[2].1);
        assert!(r[1].4);
    }

    #[test]
    fn lists_have_levels_and_numbers() {
        let l = lines(&[
            "* one".into(),
            "** two".into(),
            "# a".into(),
            "# b".into(),
            "## c".into(),
            "plain".into(),
            "# again".into(),
        ]);
        let m: Vec<Marker> = l.iter().map(|l| l.marker).collect();
        assert_eq!(
            m,
            [
                Marker::Bullet(1),
                Marker::Bullet(2),
                Marker::Number(1, 1),
                Marker::Number(1, 2),
                Marker::Number(2, 1),
                Marker::None,
                Marker::Number(1, 1),
            ]
        );
        assert_eq!(Marker::Bullet(2).prefix("*"), "  * ");
        assert_eq!(Marker::Number(1, 3).prefix("*"), "3. ");
    }

    #[test]
    fn bold_text_is_not_a_list() {
        let l = lines(&["**bold** text".into()]);
        assert_eq!(l[0].marker, Marker::None);
        assert!(l[0].runs[0].span.bold);
    }

    #[test]
    fn a_heading_is_bold() {
        let l = lines(&["== Title ==".into()]);
        assert_eq!(l[0].runs.len(), 1);
        assert_eq!(l[0].runs[0].span.text, "Title");
        assert!(l[0].runs[0].span.bold);
    }
}
