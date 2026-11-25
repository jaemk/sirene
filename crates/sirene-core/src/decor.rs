//! The page decorations of a PlantUML diagram (PCOM-1): `header`, `footer`, `caption`, and
//! `legend`, drawn around a diagram of any kind.
//!
//! The text is Creole as far as sirene reads it (PCREO-1): `**bold**`, `//italic//`, `<b>`,
//! `<i>`, `<color:...>`, `== heading` lines, bullet and numbered lists, and `<latex>` and
//! `<math>` formulas (LEMB-2). Each line of text is kept as source; [`lines`] reads it as runs.

use crate::diagram::Diagram;
use crate::flowchart::Span;
use crate::plantuml::creole;

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

/// A run of a line: a span and the Creole style it is read in. The color and background are as
/// written (`<color:red>`, `<back:#ff0>`), the size is `<size:N>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Run {
    pub span: Span,
    pub color: Option<String>,
    pub underline: bool,
    pub strike: bool,
    pub mono: bool,
    pub size: Option<u32>,
    pub back: Option<String>,
}

/// A horizontal rule line: `----`, `====`, `____`, or `....` (four or more of one character).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Single,
    Double,
    Thick,
    Dotted,
}

impl Rule {
    /// The character a rule is drawn with as text.
    pub fn ch(self) -> char {
        match self {
            Rule::Single => '-',
            Rule::Double => '=',
            Rule::Thick => '_',
            Rule::Dotted => '.',
        }
    }
}

/// A line of a decoration read as Creole: a rule, a row of table cells, or text runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub marker: Marker,
    pub runs: Vec<Run>,
    pub rule: Option<Rule>,
    /// The cells of a table row `|a|b|`; a `|=` cell is a bold heading. Empty for other lines.
    pub cells: Vec<Vec<Run>>,
}

/// Reads the source `lines` as Creole lines. A list line is `*` or `#` (repeated for a deeper
/// level) and a space, then text; `== text` is a bold heading; `----` is a rule; `|a|b|` is a
/// table row. Runs are read by [`creole::styled`]: bold, italic, and the other styles carry from
/// one run to the next within a line, and formulas are runs of their own.
pub fn lines(lines: &[String]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut numbers: Vec<usize> = Vec::new();
    for src in lines {
        let src = src.trim();
        let line = |marker, runs, rule, cells| Line {
            marker,
            runs,
            rule,
            cells,
        };
        if let Some(rule) = rule(src) {
            numbers.clear();
            out.push(line(Marker::None, Vec::new(), Some(rule), Vec::new()));
            continue;
        }
        if let Some(cells) = table_row(src) {
            numbers.clear();
            out.push(line(Marker::None, Vec::new(), None, cells));
            continue;
        }
        let (marker, text) = marker(src, &mut numbers);
        let (text, heading) = heading(text);
        out.push(line(
            marker,
            creole::styled(text, heading),
            None,
            Vec::new(),
        ));
    }
    out
}

/// The rule `line` is: four or more of `-`, `=`, `_`, or `.`.
fn rule(line: &str) -> Option<Rule> {
    let c = line.chars().next()?;
    if line.chars().count() < 4 || !line.chars().all(|d| d == c) {
        return None;
    }
    match c {
        '-' => Some(Rule::Single),
        '=' => Some(Rule::Double),
        '_' => Some(Rule::Thick),
        '.' => Some(Rule::Dotted),
        _ => None,
    }
}

/// The cells of `|a|b|`, each a heading when it starts `=`.
fn table_row(line: &str) -> Option<Vec<Vec<Run>>> {
    let inner = line.strip_prefix('|')?;
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    // A `|` inside a formula (`|x|`) is the formula's.
    let formulas = creole::latex_tags(inner);
    let mut cells = Vec::new();
    let mut from = 0;
    let mut cut = |end: usize, next: usize| {
        let cell = inner[from..end].trim();
        cells.push(match cell.strip_prefix('=') {
            Some(head) => creole::styled(head.trim(), true),
            None => creole::styled(cell, false),
        });
        from = next;
    };
    for (i, c) in inner.char_indices() {
        if c == '|' && !formulas.iter().any(|f| f.contains(&i)) {
            cut(i, i + 1);
        }
    }
    cut(inner.len(), inner.len());
    Some(cells)
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

    fn runs_of(src: &str) -> Vec<Run> {
        lines(&[src.to_owned()]).remove(0).runs
    }

    #[test]
    fn underline_strike_and_monospace_markers_style_their_text() {
        let r = runs_of("a __b__ --c-- \"\"d\"\" e");
        let styled: Vec<(&str, bool, bool, bool)> = r
            .iter()
            .map(|r| (r.span.text.as_str(), r.underline, r.strike, r.mono))
            .collect();
        assert_eq!(
            styled,
            [
                ("a ", false, false, false),
                ("b", true, false, false),
                (" ", false, false, false),
                ("c", false, true, false),
                (" ", false, false, false),
                ("d", false, false, true),
                (" e", false, false, false),
            ]
        );
    }

    #[test]
    fn tags_set_underline_strike_size_and_background() {
        let r = runs_of("<u>a</u><s>b</s><size:20>c</size><back:#ff0>d</back>e");
        assert!(r[0].underline && !r[1].underline);
        assert!(r[1].strike && !r[2].strike);
        assert_eq!(r[2].size, Some(20));
        assert_eq!(r[3].back.as_deref(), Some("ff0"));
        assert_eq!((r[4].size, r[4].back.clone()), (None, None));
    }

    #[test]
    fn a_marker_with_no_partner_or_a_space_after_it_is_text() {
        for src in [
            "A --> B",
            "a__b",
            "x -- y -- z",
            "-- x",
            "__ x __",
            "\"\" x",
        ] {
            let r = runs_of(src);
            let text: String = r.iter().map(|r| r.span.text.as_str()).collect();
            assert_eq!(text, src, "{src}");
            assert!(
                r.iter().all(|r| !r.underline && !r.strike && !r.mono),
                "{src}"
            );
        }
        // A bad size is text, not a style.
        assert_eq!(runs_of("<size:big>x")[0].size, None);
    }

    #[test]
    fn styles_carry_across_a_formula() {
        let r = runs_of("__a <latex>x</latex> b__");
        assert_eq!(r.len(), 3);
        assert!(r[0].underline && r[2].underline);
    }

    #[test]
    fn four_of_a_character_is_a_rule() {
        for (src, want) in [
            ("----", Rule::Single),
            ("=====", Rule::Double),
            ("____", Rule::Thick),
            ("....", Rule::Dotted),
        ] {
            let l = &lines(&[src.into()])[0];
            assert_eq!(l.rule, Some(want), "{src}");
            assert!(l.runs.is_empty());
        }
        for src in ["---", "--x--", "-=-=", "== h =="] {
            assert_eq!(lines(&[src.into()])[0].rule, None, "{src}");
        }
    }

    #[test]
    fn a_table_row_has_cells_and_headings() {
        let l = &lines(&[
            "|= Name |= Qty |".into(),
            "| a | <latex>|x|</latex> |".into(),
        ]);
        assert_eq!(l[0].cells.len(), 2);
        assert!(l[0].cells[0][0].span.bold);
        assert_eq!(l[0].cells[1][0].span.text, "Qty");
        assert_eq!(l[1].cells.len(), 2);
        assert_eq!(l[1].cells[0][0].span.text, "a");
        // The bars of the formula are not cell breaks.
        assert!(l[1].cells[1][0].span.text.contains("|x|"));
        assert!(l[0].runs.is_empty());
    }
}
