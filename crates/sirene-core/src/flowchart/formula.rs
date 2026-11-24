//! `$$...$$` formulas in label text (LEMB-1), shared by every diagram that has them: the runs of
//! a line and the diagnostics for formulas that do not typeset (LEMB-4, LEMB-5).

use super::Span;
use super::model::{formulas, math_line};

/// Whether `line` holds a `$$...$$` formula.
pub fn has(line: &str) -> bool {
    !formulas(line).is_empty()
}

/// `line` as a row of text runs and formula runs. Without the `math` feature, or without a
/// formula, one plain run.
pub fn runs(line: &str) -> Vec<Span> {
    if cfg!(feature = "math") && has(line) {
        math_line(line)
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
) -> Vec<crate::diagnostic::Diagnostic> {
    use crate::diagnostic::Diagnostic;
    use crate::math::{MathStyle, typeset};
    use rowan::TextRange;
    let mut out = Vec::new();
    let mut cursor = 0;
    for line in lines {
        for r in formulas(&line) {
            let body = &line[r.start + 2..r.end - 2];
            // Where the body is in the source text, when it is there as written.
            let found = raw
                .get(cursor..)
                .and_then(|rest| rest.find(body))
                .map(|i| cursor + i);
            if let Some(p) = found {
                cursor = p + body.len();
            }
            let Err(e) = typeset(&body.replace("\\\\", "\\"), MathStyle::Display) else {
                continue;
            };
            let range = match found {
                Some(p) => {
                    let from = uncollapsed(body, e.span.start);
                    let to = uncollapsed(body, e.span.end).max(from);
                    let at = |o: usize| start + rowan::TextSize::new((p + o) as u32);
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
) -> Vec<crate::diagnostic::Diagnostic> {
    Vec::new()
}
