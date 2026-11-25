//! Formulas in Markdown, found beside the diagram blocks (MARKDO-5): fenced blocks named `math`,
//! `latex`, or `tex`, `$$...$$` display blocks, and inline `$...$`.

use std::ops::Range;

use super::{Block, Fenced, diagram_block, fence_end, fenced, info_word, lines};
use super::{list_item, quotes, spaces};
use crate::diagram::{self, Lang, Lowered};

/// How a formula is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathKind {
    /// A fenced block named `math`, `latex`, or `tex`: a TeX source, in display style.
    Fenced,
    /// `$$...$$`, in display style.
    Display,
    /// `$...$`, in text style.
    Inline,
}

/// One formula of a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathBlock {
    /// The formula as the typesetter reads it: for a fenced block the TeX source (with its
    /// wrappers, see [`crate::equation::Equation::tex`]), otherwise the text between the
    /// delimiters, with its lines joined by `\n` and the blockquote markers and list indentation
    /// of each taken off.
    pub source: String,
    pub kind: MathKind,
    /// The Markdown line of the first line of `source`, from 1.
    pub line: usize,
    /// The block's byte range in the Markdown: the fences, or the delimiters, included.
    pub range: Range<usize>,
    /// The columns before each line of `source` in its Markdown line, in characters.
    strips: Vec<usize>,
    /// The columns of a line `strips` has no entry for.
    indent: usize,
}

impl MathBlock {
    /// Where a position in [`MathBlock::source`] (line and column from 1) is in the Markdown.
    pub fn position(&self, line: usize, col: usize) -> (usize, usize) {
        let at = line.saturating_sub(1);
        let strip = self.strips.get(at).copied().unwrap_or(self.indent);
        (self.line + at, col + strip)
    }

    /// The formula lowered: its equation and the problems found reading it, with positions in
    /// [`MathBlock::source`].
    pub fn lower(&self) -> Lowered {
        match self.kind {
            MathKind::Fenced => diagram::lower_as(&self.source, Some(Lang::Latex)),
            MathKind::Display => diagram::lower_formula(&self.source, false),
            MathKind::Inline => diagram::lower_formula(&self.source, true),
        }
    }
}

/// A diagram or a formula of a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Diagram(Block),
    Math(MathBlock),
}

impl Part {
    /// The part's byte range in the Markdown.
    pub fn range(&self) -> Range<usize> {
        match self {
            Part::Diagram(b) => b.range(),
            Part::Math(m) => m.range.clone(),
        }
    }
}

/// The diagram blocks and formulas in `src`, in source order and not overlapping (MARKDO-5).
/// Math is not read inside a fenced block of any language, an indented code block, or a code
/// span.
pub fn parts(src: &str) -> Vec<Part> {
    let lines = lines(src);
    let offset = |i: usize| lines.get(i).map_or(src.len(), |(at, _)| *at);
    let mut covered = vec![false; lines.len()];
    let mut out = Vec::new();
    for b in fenced(src) {
        let end = match b.close {
            Some(j) => j + 1,
            None if b.stop != usize::MAX => b.stop,
            None => lines.len(),
        };
        for c in covered.iter_mut().take(end).skip(b.open) {
            *c = true;
        }
        if b.lang.is_some() {
            out.extend(diagram_block(b, &offset).map(Part::Diagram));
        } else if info_word(&b.info).is_some_and(|w| matches!(w.as_str(), "math" | "latex" | "tex"))
        {
            out.push(Part::Math(fenced_math(b, &offset)));
        }
    }
    text(&lines, &covered, &mut out);
    out.sort_by_key(|p| p.range().start);
    out
}

fn fenced_math(b: Fenced, offset: &impl Fn(usize) -> usize) -> MathBlock {
    let (_, end) = fence_end(&b, offset);
    MathBlock {
        source: b.code,
        kind: MathKind::Fenced,
        line: b.open + 2,
        range: b.runs.0..end.max(b.runs.0),
        strips: b.strips,
        indent: b.indent,
    }
}

/// A line of a paragraph.
struct Line<'a> {
    /// The line after its container's markers and indentation.
    text: &'a str,
    /// Where `text` starts in the Markdown.
    at: usize,
    /// The characters before `text` on its Markdown line.
    strip: usize,
    /// The Markdown line, from 1.
    line: usize,
}

/// Finds the formulas of the paragraphs outside the `covered` lines.
fn text(lines: &[(usize, &str)], covered: &[bool], out: &mut Vec<Part>) {
    let mut para: Vec<Line> = Vec::new();
    // The content columns of the open list items, and the blockquote depth they are in.
    let mut items: Vec<usize> = Vec::new();
    let mut depth = 0;
    for (i, &(at, line)) in lines.iter().enumerate() {
        if covered.get(i).copied().unwrap_or(false) {
            flush(&mut para, out);
            continue;
        }
        let (q, qlen) = quotes(line, usize::MAX);
        if q != depth {
            items.clear();
            depth = q;
            flush(&mut para, out);
        }
        let rest = &line[qlen..];
        if rest.trim().is_empty() {
            flush(&mut para, out);
            continue;
        }
        let mut pos = spaces(rest);
        while items.last().is_some_and(|col| pos < *col) {
            items.pop();
        }
        let mut base = items.last().copied().unwrap_or(0);
        while pos - base <= 3
            && let Some(col) = list_item(&rest[pos..])
        {
            flush(&mut para, out);
            base = pos + col;
            items.push(base);
            pos = base + spaces(&rest[base..]);
        }
        let t = rest[pos..].trim_end();
        // Indented code, unless it continues a paragraph.
        if pos - base >= 4 && para.is_empty() {
            continue;
        }
        if is_break(t) {
            flush(&mut para, out);
            continue;
        }
        let heading = heading(t);
        if heading {
            flush(&mut para, out);
        }
        para.push(Line {
            text: t,
            at: at + qlen + pos,
            strip: line[..qlen + pos].chars().count(),
            line: i + 1,
        });
        if heading {
            flush(&mut para, out);
        }
    }
    flush(&mut para, out);
}

/// Whether `t` is a thematic break or a setext heading's underline.
fn is_break(t: &str) -> bool {
    let t = t.trim();
    let Some(c) = t
        .chars()
        .next()
        .filter(|c| matches!(c, '-' | '*' | '_' | '='))
    else {
        return false;
    };
    let marks = t.chars().filter(|x| *x == c).count();
    t.chars().all(|x| x == c || x == ' ' || x == '\t') && (marks >= 3 || (c == '=' && marks >= 1))
}

/// Whether `t` is an ATX heading (one to six `#` and a space).
fn heading(t: &str) -> bool {
    let hashes = t.len() - t.trim_start_matches('#').len();
    (1..=6).contains(&hashes) && (t.len() == hashes || t[hashes..].starts_with([' ', '\t']))
}

fn flush(para: &mut Vec<Line>, out: &mut Vec<Part>) {
    if !para.is_empty() {
        scan(para, out);
        para.clear();
    }
}

/// The number of `c` bytes at `at`.
fn run(b: &[u8], at: usize, c: u8) -> usize {
    b[at..].iter().take_while(|x| **x == c).count()
}

/// Finds the code spans, `$$...$$`, and `$...$` of one paragraph, in order.
fn scan(para: &[Line], out: &mut Vec<Part>) {
    let mut text = String::new();
    let mut starts = Vec::with_capacity(para.len());
    for (k, l) in para.iter().enumerate() {
        if k > 0 {
            text.push('\n');
        }
        starts.push(text.len());
        text.push_str(l.text);
    }
    let b = text.as_bytes();
    let mut p = 0;
    // The byte before which no inline formula opens: a scan for a closer ended there.
    let mut dead = 0;
    while p < b.len() {
        match b[p] {
            b'\\' => p += if escapes(b, p) { 2 } else { 1 },
            b'`' => p = skip_code(b, p),
            b'$' => {
                let n = run(b, p, b'$');
                if n == 2
                    && let Some(q) = display_close(b, p + 2)
                    && !text[p + 2..q].trim().is_empty()
                {
                    emit(
                        MathKind::Display,
                        para,
                        &starts,
                        &text,
                        p..q + 2,
                        p + 2..q,
                        out,
                    );
                    p = q + 2;
                } else if n == 1 && p >= dead {
                    match inline_close(&text, p) {
                        Ok(q) => {
                            emit(
                                MathKind::Inline,
                                para,
                                &starts,
                                &text,
                                p..q + 1,
                                p + 1..q,
                                out,
                            );
                            p = q + 1;
                        }
                        Err(scanned) => {
                            // A closer valid for a later `$` is valid for this one, so no `$`
                            // before where this scan ended has one (a scan per `$` is quadratic).
                            dead = scanned.unwrap_or(dead);
                            p += n;
                        }
                    }
                } else {
                    p += n;
                }
            }
            _ => p += 1,
        }
    }
}

/// Whether the backslash at `at` escapes the (ASCII punctuation) byte after it.
fn escapes(b: &[u8], at: usize) -> bool {
    b.get(at + 1).is_some_and(u8::is_ascii_punctuation)
}

/// Where the backticks at `at` leave off: after the code span they open, or after the run when no
/// run of the same length closes it.
fn skip_code(b: &[u8], at: usize) -> usize {
    let n = run(b, at, b'`');
    code_span_end(b, at + n, n).unwrap_or(at + n)
}

/// Where a code span opened by a run of `n` backticks ends, searching from `from`: just after
/// the next run of exactly `n`.
fn code_span_end(b: &[u8], from: usize, n: usize) -> Option<usize> {
    let mut q = from;
    while q < b.len() {
        if b[q] == b'`' {
            let m = run(b, q, b'`');
            if m == n {
                return Some(q + m);
            }
            q += m;
        } else {
            q += 1;
        }
    }
    None
}

/// The start of the `$$` that closes a display formula whose body starts at `from`.
fn display_close(b: &[u8], from: usize) -> Option<usize> {
    let mut q = from;
    while q < b.len() {
        match b[q] {
            b'\\' => q += if escapes(b, q) { 2 } else { 1 },
            b'`' => q = skip_code(b, q),
            b'$' if b.get(q + 1) == Some(&b'$') => return Some(q),
            _ => q += 1,
        }
    }
    None
}

/// The position of the `$` that closes the inline formula opened by the one at `open`, by
/// GitHub's rules: the opening `$` is not followed by whitespace, the closing one is not
/// preceded by whitespace nor followed by a digit, and the formula stays on one line.
///
/// An error is `None` when `open` cannot open a formula, and otherwise the byte the search ended at
/// (the end of the line) without finding a closer.
fn inline_close(text: &str, open: usize) -> Result<usize, Option<usize>> {
    let b = text.as_bytes();
    if text[open + 1..]
        .chars()
        .next()
        .is_none_or(char::is_whitespace)
    {
        return Err(None);
    }
    let mut q = open + 1;
    while q < b.len() {
        match b[q] {
            b'\\' => q += if escapes(b, q) { 2 } else { 1 },
            b'`' => q = skip_code(b, q),
            b'\n' => return Err(Some(q)),
            b'$' => {
                let before = text[..q]
                    .chars()
                    .next_back()
                    .is_some_and(|c| !c.is_whitespace());
                let digit = b.get(q + 1).is_some_and(u8::is_ascii_digit);
                if before && !digit {
                    return Ok(q);
                }
                q += 1;
            }
            _ => q += 1,
        }
    }
    Err(Some(b.len()))
}

/// The paragraph line that holds byte `at` of the joined text.
fn line_of(starts: &[usize], at: usize) -> usize {
    starts.partition_point(|s| *s <= at).saturating_sub(1)
}

/// Adds the formula whose body is `text[body]` and whose delimiters end at `outer`.
fn emit(
    kind: MathKind,
    para: &[Line],
    starts: &[usize],
    text: &str,
    outer: Range<usize>,
    body: Range<usize>,
    out: &mut Vec<Part>,
) {
    let source = &text[body.clone()];
    let k = line_of(starts, body.start);
    let Some(first) = para.get(k) else {
        return;
    };
    let md = |at: usize| {
        let k = line_of(starts, at);
        para.get(k).map_or(0, |l| l.at + at - starts[k])
    };
    let mut strips = vec![first.strip + text[starts[k]..body.start].chars().count()];
    for j in 1..=source.matches('\n').count() {
        strips.push(para.get(k + j).map_or(0, |l| l.strip));
    }
    out.push(Part::Math(MathBlock {
        source: source.to_owned(),
        kind,
        line: first.line,
        range: md(outer.start)..md(outer.end),
        strips,
        indent: 0,
    }));
}
