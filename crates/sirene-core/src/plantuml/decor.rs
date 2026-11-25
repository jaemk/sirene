//! `header`, `footer`, `caption`, and `legend` statements (PCOM-1), read by every kind that
//! reads the shared statements and by the sequence kind, and the diagnostics for their
//! formulas.
//!
//! A statement is read where it is found and recorded in a list kept for the lowering in
//! progress; [`apply`] then wraps the lowered diagram in them. The lowering runs on one thread
//! at a time and [`Parse::lower`](super::Parse::lower) clears the list before and takes it after.

use std::cell::RefCell;
use std::collections::HashSet;

use rowan::TextRange;

use super::lines::{Cursor, Line, word};
use crate::decor::{Banner, Decorated, Decorations, HAlign, Legend, VAlign};
use crate::diagnostic::Diagnostic;
use crate::diagram::Diagram;
use crate::flowchart::Delim;
use crate::syntax::SyntaxNode;

/// Which decoration a statement is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Header,
    Footer,
    Caption,
    Legend,
}

/// A decoration statement read: its lines of text, each with the source range it was read
/// from, and its position.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    pub slot: Slot,
    pub lines: Vec<(String, TextRange)>,
    pub horizontal: HAlign,
    pub vertical: VAlign,
}

thread_local! {
    static READ: RefCell<Vec<Item>> = const { RefCell::new(Vec::new()) };
}

/// Starts a lowering with no decorations read.
pub(crate) fn reset() {
    READ.with_borrow_mut(Vec::clear);
}

/// Records a statement read.
pub(crate) fn record(item: Item) {
    READ.with_borrow_mut(|items| items.push(item));
}

/// The statements read since [`reset`].
pub(crate) fn take() -> Vec<Item> {
    READ.with_borrow_mut(std::mem::take)
}

fn position(word: &str) -> Option<Result<HAlign, VAlign>> {
    match word.to_ascii_lowercase().as_str() {
        "left" => Some(Ok(HAlign::Left)),
        "right" => Some(Ok(HAlign::Right)),
        "center" => Some(Ok(HAlign::Center)),
        "top" => Some(Err(VAlign::Top)),
        "bottom" => Some(Err(VAlign::Bottom)),
        _ => None,
    }
}

/// The positions in `words`, when every word is one: the last horizontal and the last
/// vertical.
fn positions(rest: &str) -> Option<(Option<HAlign>, Option<VAlign>)> {
    let (mut h, mut v) = (None, None);
    for w in rest.split_whitespace() {
        match position(w)? {
            Ok(a) => h = Some(a),
            Err(a) => v = Some(a),
        }
    }
    Some((h, v))
}

/// The closers of a block `slot` opens.
fn closers(slot: Slot) -> Option<[&'static str; 2]> {
    match slot {
        Slot::Header => Some(["end header", "endheader"]),
        Slot::Footer => Some(["end footer", "endfooter"]),
        Slot::Legend => Some(["end legend", "endlegend"]),
        Slot::Caption => None,
    }
}

fn name(slot: Slot) -> &'static str {
    match slot {
        Slot::Header => "header",
        Slot::Footer => "footer",
        Slot::Caption => "caption",
        Slot::Legend => "legend",
    }
}

/// Reads the statement at `line` when it is a `header`, `footer`, `caption`, or `legend` that
/// is drawn, with its block. The forms:
///
/// - `header text`, `header: text`, `left|right|center header text`;
/// - `header`, `header left|right|center`, `left|right|center header`, and the lines up to
///   `end header` (the same for `footer`);
/// - `caption text`;
/// - `legend [top|bottom] [left|right|center]` and the lines up to `end legend`.
///
/// Anything else is not read: `None`, with nothing consumed.
pub(crate) fn statement(line: &Line, cursor: &mut Cursor, d: &mut Vec<Diagnostic>) -> Option<Item> {
    let t = line.text.as_str();
    let first = word(t);
    let rest = t[first.len()..].trim_start();
    let (prefix, slot, rest) = match first.to_ascii_lowercase().as_str() {
        "header" => (None, Slot::Header, rest),
        "footer" => (None, Slot::Footer, rest),
        "caption" => (None, Slot::Caption, rest),
        "legend" => (None, Slot::Legend, rest),
        side @ ("left" | "right" | "center") => {
            let side = position(side)?.ok()?;
            let second = word(rest);
            let slot = match second.to_ascii_lowercase().as_str() {
                "header" => Slot::Header,
                "footer" => Slot::Footer,
                _ => return None,
            };
            (Some(side), slot, rest[second.len()..].trim_start())
        }
        _ => return None,
    };
    let default = match slot {
        Slot::Header => HAlign::Right,
        _ => HAlign::Center,
    };
    let mut item = Item {
        slot,
        lines: Vec::new(),
        horizontal: prefix.unwrap_or(default),
        vertical: VAlign::Bottom,
    };
    // A block: nothing, or only positions, after the keyword. A header or footer has no
    // vertical position, and one with a position before its keyword takes none after it.
    let pos = positions(rest).filter(|(_, v)| slot == Slot::Legend || v.is_none());
    let block = if slot == Slot::Caption || (prefix.is_some() && !rest.is_empty()) {
        None
    } else {
        pos
    };
    if let Some((h, v)) = block {
        if let Some(h) = h {
            item.horizontal = h;
        }
        if let Some(v) = v {
            item.vertical = v;
        }
        let closers = closers(slot)?;
        let (body, end) = cursor.until(&closers);
        if end.is_none() {
            d.push(Diagnostic::new(
                line.range,
                format!("unclosed `{}`: expected `{}`", name(slot), closers[0]),
            ));
        }
        for l in body {
            push_lines(&mut item, &l.text, l.range);
        }
        return Some(item);
    }
    if slot == Slot::Legend {
        return None;
    }
    let text = rest.strip_prefix(':').unwrap_or(rest).trim();
    if text.is_empty() {
        return None;
    }
    push_lines(&mut item, text, line.range);
    Some(item)
}

/// Adds the lines of `text`, where `\n` breaks a line outside a formula, read from `range`.
fn push_lines(item: &mut Item, text: &str, range: TextRange) {
    for l in super::creole::newlines(text).split('\n') {
        item.lines.push((l.to_owned(), range));
    }
}

/// `diagram` with the decorations in `items` around it, and the diagnostics for their
/// formulas. A diagram with none is as it was. Of each kind the last statement is kept.
pub(crate) fn apply(
    diagram: Diagram,
    items: Vec<Item>,
    root: &SyntaxNode,
) -> (Diagram, Vec<Diagnostic>) {
    let mut decor = Decorations::default();
    let text = |item: &Item| {
        item.lines
            .iter()
            .map(|(l, _)| l.clone())
            .collect::<Vec<_>>()
    };
    for item in &items {
        let banner = || Banner {
            lines: text(item),
            align: item.horizontal,
        };
        match item.slot {
            Slot::Header => decor.header = Some(banner()),
            Slot::Footer => decor.footer = Some(banner()),
            Slot::Caption => decor.caption = Some(banner()),
            Slot::Legend => {
                decor.legend = Some(Legend {
                    lines: text(item),
                    horizontal: item.horizontal,
                    vertical: item.vertical,
                });
            }
        }
    }
    if decor.is_empty() || matches!(diagram, Diagram::Equation(_)) {
        return (diagram, Vec::new());
    }
    let diagnostics = diagnose(&items, &decor, root);
    let wrapped = Diagram::Decorated(Box::new(Decorated { diagram, decor }));
    (wrapped, diagnostics)
}

/// A diagnostic for each formula of the decorations that is drawn and does not typeset (and
/// without the `math` feature, for each one), on the formula where it is written.
fn diagnose(items: &[Item], decor: &Decorations, root: &SyntaxNode) -> Vec<Diagnostic> {
    let src = root.text().to_string();
    let mut used = HashSet::new();
    let mut out = Vec::new();
    // The statements that were not replaced by a later one of their kind.
    let last = |slot: Slot| items.iter().rev().find(|i| i.slot == slot);
    let kept = [
        (Slot::Header, decor.header.is_some()),
        (Slot::Footer, decor.footer.is_some()),
        (Slot::Caption, decor.caption.is_some()),
        (Slot::Legend, decor.legend.is_some()),
    ];
    for (slot, present) in kept {
        let Some(item) = last(slot).filter(|_| present) else {
            continue;
        };
        for (line, range) in &item.lines {
            out.extend(crate::flowchart::formula::errors_among(
                &src,
                [line.clone()],
                &[*range],
                &mut used,
                root.text_range(),
                Delim::Latex,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(s: &str) -> Line {
        Line {
            text: s.to_owned(),
            range: TextRange::default(),
        }
    }

    fn read(first: &str, rest: &[&str]) -> (Option<Item>, Vec<String>, Option<String>) {
        let mut cursor = Cursor::new(rest.iter().map(|s| line(s)).collect());
        let mut d = Vec::new();
        let item = statement(&line(first), &mut cursor, &mut d);
        let msgs = d.into_iter().map(|d| d.message).collect();
        (item, msgs, cursor.next().map(|l| l.text))
    }

    fn texts(item: &Item) -> Vec<&str> {
        item.lines.iter().map(|(l, _)| l.as_str()).collect()
    }

    #[test]
    fn one_line_forms_have_default_positions() {
        let (h, ..) = read("header Page 1", &[]);
        let h = h.unwrap();
        assert_eq!((h.slot, h.horizontal), (Slot::Header, HAlign::Right));
        assert_eq!(texts(&h), ["Page 1"]);
        let (f, ..) = read("footer: the end", &[]);
        let f = f.unwrap();
        assert_eq!((f.slot, f.horizontal), (Slot::Footer, HAlign::Center));
        assert_eq!(texts(&f), ["the end"]);
        let (c, ..) = read("caption Figure 1", &[]);
        assert_eq!(c.unwrap().horizontal, HAlign::Center);
    }

    #[test]
    fn a_position_before_the_keyword_sets_the_alignment() {
        for (src, want) in [
            ("left header x", HAlign::Left),
            ("Center Header x", HAlign::Center),
            ("right footer x", HAlign::Right),
        ] {
            let (item, ..) = read(src, &[]);
            let item = item.unwrap();
            assert_eq!(item.horizontal, want, "{src}");
            assert_eq!(texts(&item), ["x"], "{src}");
        }
    }

    #[test]
    fn blocks_run_to_their_closer() {
        let (h, msgs, next) = read("header", &["a", "b", "end header", "after"]);
        let h = h.unwrap();
        assert_eq!(texts(&h), ["a", "b"]);
        assert!(msgs.is_empty());
        assert_eq!(next.as_deref(), Some("after"));
        let (f, ..) = read("left footer", &["x", "endfooter"]);
        let f = f.unwrap();
        assert_eq!((f.horizontal, texts(&f)), (HAlign::Left, vec!["x"]));
        let (f, ..) = read("footer right", &["x", "End  Footer"]);
        assert_eq!(f.unwrap().horizontal, HAlign::Right);
    }

    #[test]
    fn an_unclosed_block_is_one_error() {
        let (item, msgs, _) = read("legend", &["a"]);
        assert!(item.is_some());
        assert_eq!(msgs, ["unclosed `legend`: expected `end legend`"]);
    }

    #[test]
    fn legend_positions() {
        for (src, h, v) in [
            ("legend", HAlign::Center, VAlign::Bottom),
            ("legend left", HAlign::Left, VAlign::Bottom),
            ("legend right", HAlign::Right, VAlign::Bottom),
            ("legend top", HAlign::Center, VAlign::Top),
            ("legend top left", HAlign::Left, VAlign::Top),
            ("legend bottom right", HAlign::Right, VAlign::Bottom),
            ("LEGEND Top Right", HAlign::Right, VAlign::Top),
            ("legend center", HAlign::Center, VAlign::Bottom),
        ] {
            let (item, ..) = read(src, &["x", "endlegend"]);
            let item = item.unwrap_or_else(|| panic!("{src}"));
            assert_eq!((item.horizontal, item.vertical), (h, v), "{src}");
            assert_eq!(texts(&item), ["x"], "{src}");
        }
    }

    #[test]
    fn a_legend_with_text_on_its_line_is_not_read() {
        let (item, _, next) = read("legend hello", &["x"]);
        assert!(item.is_none());
        assert_eq!(next.as_deref(), Some("x"));
    }

    #[test]
    fn a_backslash_n_breaks_a_line_outside_a_formula() {
        let (h, ..) = read("header a\\nb <latex>x \\neq y</latex>", &[]);
        assert_eq!(texts(&h.unwrap()), ["a", "b <latex>x \\neq y</latex>"]);
    }

    #[test]
    fn other_statements_are_not_read() {
        for s in ["title x", "left x", "note left", "headers x", "caption"] {
            assert!(read(s, &[]).0.is_none(), "{s}");
        }
        assert!(read("left header", &["x", "end header"]).0.is_some());
        // `top` is text for a header, which has no vertical position.
        let (h, ..) = read("header top", &[]);
        assert_eq!(texts(&h.unwrap()), ["top"]);
    }
}
