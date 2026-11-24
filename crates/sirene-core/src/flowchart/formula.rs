//! Formulas in label text (LEMB-1, LEMB-2), shared by every diagram that has them: the runs of
//! a line and the diagnostics for formulas that do not typeset (LEMB-4, LEMB-5). Mermaid writes
//! a formula `$$...$$` and PlantUML `<latex>...</latex>` (TeX) or `<math>...</math>`
//! (AsciiMath): a [`Delim`] says which.

use super::model::math_line;
use super::{Delim, Span};

/// Whether `line` holds a formula in `delim`.
pub fn has(line: &str, delim: Delim) -> bool {
    !delim.ranges(line).is_empty()
}

/// `line` as a row of text runs and formula runs. Without the `math` feature, or without a
/// formula, one plain run.
pub fn runs(line: &str, delim: Delim) -> Vec<Span> {
    if cfg!(feature = "math") && has(line, delim) {
        math_line(line, delim)
    } else {
        vec![Span::plain(line)]
    }
}

/// The offset in `body` of byte `at` of `body` with `\\` read as `\`.
#[cfg(feature = "math")]
fn uncollapsed(body: &str, at: usize) -> usize {
    let bytes = body.as_bytes();
    let (mut i, mut seen) = (0, 0);
    while i < bytes.len() && seen < at {
        i += if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'\\') {
            2
        } else {
            1
        };
        seen += 1;
    }
    i
}

/// Each formula in `lines` that does not typeset, in order: its body as written and the error,
/// whose span is in the TeX source ([`Delim::source`] of the body).
#[cfg(feature = "math")]
pub(crate) fn failures(
    lines: impl IntoIterator<Item = String>,
    delim: Delim,
) -> Vec<(String, crate::math::MathError)> {
    let mut out = Vec::new();
    for line in lines {
        for r in delim.ranges(&line) {
            let text = &line[r.clone()];
            let (open, close) = delim.lens(text);
            let body = &text[open..text.len() - close];
            if let Err(e) = check(delim, text, body) {
                out.push((body.to_owned(), e));
            }
        }
    }
    out
}

/// Typesets the formula `body` of the run `text` (delimiters included), as its delimiters say.
#[cfg(feature = "math")]
fn check(delim: Delim, text: &str, body: &str) -> Result<(), crate::math::MathError> {
    use crate::math::{typeset, typeset_asciimath};
    let (src, style) = (delim.source(body), delim.style());
    if delim.asciimath(text) {
        typeset_asciimath(&src, style).map(drop)
    } else {
        typeset(&src, style).map(drop)
    }
}

/// A diagnostic for each formula in `lines` that does not typeset, in order.
///
/// `raw` is the source text the lines came from, starting at `start`: a formula is found in it
/// by its body, and the diagnostic is on the failing span inside the delimiters. A formula not
/// found as written (entity codes, say) is reported on `fallback`.
#[cfg(feature = "math")]
pub(crate) fn errors(
    start: rowan::TextSize,
    raw: &str,
    lines: impl IntoIterator<Item = String>,
    fallback: rowan::TextRange,
    delim: Delim,
) -> Vec<crate::diagnostic::Diagnostic> {
    use crate::diagnostic::Diagnostic;
    use rowan::TextRange;
    let mut out = Vec::new();
    let mut cursor = 0;
    for line in lines {
        for r in delim.ranges(&line) {
            let text = &line[r.clone()];
            let (open, close) = delim.lens(text);
            let body = &text[open..text.len() - close];
            // Where the body is in the source text, when it is there as written.
            let found = raw
                .get(cursor..)
                .and_then(|rest| rest.find(body))
                .map(|i| cursor + i);
            if let Some(p) = found {
                cursor = p + body.len();
            }
            let Err(e) = check(delim, text, body) else {
                continue;
            };
            let range = match found {
                Some(p) => {
                    let (from, to) = match delim {
                        Delim::Dollar => {
                            let from = uncollapsed(body, e.span.start);
                            (from, uncollapsed(body, e.span.end).max(from))
                        }
                        Delim::Latex => (e.span.start, e.span.end.max(e.span.start)),
                    };
                    let at =
                        |o: usize| start + rowan::TextSize::new((p + o.min(body.len())) as u32);
                    TextRange::new(at(from), at(to))
                }
                None => fallback,
            };
            out.push(Diagnostic::new(range, e.message));
        }
    }
    out
}

#[cfg(not(feature = "math"))]
pub(crate) fn errors(
    _: rowan::TextSize,
    _: &str,
    _: impl IntoIterator<Item = String>,
    _: rowan::TextRange,
    _: Delim,
) -> Vec<crate::diagnostic::Diagnostic> {
    Vec::new()
}
