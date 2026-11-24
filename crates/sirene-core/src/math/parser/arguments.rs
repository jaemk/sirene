//! Arguments that are not formulas: sizes (`1em`, `-3mu`) and colors (`red`, `#ff0000`).
//!
//! They are read as the text of the tokens, then checked, as KaTeX does: a size is a number and a
//! two-letter unit, and a color is a name or a hex code. The result is a node of its own kind that
//! the function reading it takes apart.

use std::ops::Range;

use super::{MathError, Node, NodeKind, Parser, TokenKind, is_char};
use crate::math::{ast::Measurement, expander::token_text, limits};

/// The units a size may use: TeX's absolute units, and the relative `em`, `ex`, and `mu`.
const UNITS: &[&str] = &[
    "pt", "mm", "cm", "in", "bp", "pc", "dd", "cc", "nd", "nc", "sp", "px", "ex", "em", "mu",
];

impl<'a> Parser<'a> {
    /// The text of an argument's tokens, and the source range it came from. An optional argument
    /// is `[...]`, and is `None` when absent.
    pub(super) fn parse_string_group(
        &mut self,
        optional: bool,
    ) -> Result<Option<(String, Range<usize>)>, MathError> {
        let Some(arg) = self.expander.scan_argument(optional)? else {
            return Ok(None);
        };
        let mut text = String::new();
        loop {
            let token = self.fetch()?;
            if token.kind == TokenKind::Eof {
                break;
            }
            text.push_str(&token_text(&token.kind));
            self.consume();
        }
        // The end marker that `scan_argument` put after the argument.
        self.consume();
        Ok(Some((text, arg.span)))
    }

    /// The longest run of tokens whose text `accepts`, for an argument written without braces
    /// (`\kern1em`).
    fn parse_regex_group(
        &mut self,
        accepts: fn(&str) -> bool,
        what: &str,
    ) -> Result<(String, Range<usize>), MathError> {
        let first = self.fetch()?;
        let mut end = first.span.end;
        let mut text = String::new();
        loop {
            let token = self.fetch()?;
            if token.kind == TokenKind::Eof {
                break;
            }
            let candidate = format!("{text}{}", token_text(&token.kind));
            // A size is short; stopping at a bound keeps each test cheap.
            if candidate.len() > limits::MAX_SIZE_TEXT || !accepts(&candidate) {
                break;
            }
            text = candidate;
            end = token.span.end;
            self.consume();
        }
        if text.is_empty() {
            return Err(MathError::new(
                format!("Invalid {what}: '{}'", token_text(&first.kind)),
                first.span,
            ));
        }
        Ok((text, first.span.start..end))
    }

    /// A size argument as a `Size` node.
    pub(super) fn parse_size_group(
        &mut self,
        optional: bool,
    ) -> Result<Option<Node<'a>>, MathError> {
        // The braces are looked for before anything is expanded.
        self.expander.consume_spaces()?;
        let braced = is_char(self.expander.future()?, "{");
        let found = if !optional && !braced {
            Some(self.parse_regex_group(is_size_prefix, "size")?)
        } else {
            self.parse_string_group(optional)?
        };
        let Some((mut text, span)) = found else {
            return Ok(None);
        };
        let mut is_blank = false;
        if !optional && text.is_empty() {
            // `\above{}` and `\genfrac`: an empty size is zero.
            text = "0pt".to_string();
            is_blank = true;
        }
        let Some((number, unit)) = find_measurement(&text) else {
            return Err(MathError::new(
                format!("Invalid size: '{}'", limits::clip(&text)),
                span,
            ));
        };
        if !UNITS.contains(&unit) {
            return Err(MathError::new(format!("Invalid unit: '{unit}'"), span));
        }
        Ok(Some(Node {
            kind: NodeKind::Size {
                value: Measurement {
                    number,
                    unit: unit.to_string(),
                },
                is_blank,
            },
            mode: self.mode,
            span,
        }))
    }

    /// A color argument as a `ColorToken` node.
    pub(super) fn parse_color_group(
        &mut self,
        optional: bool,
    ) -> Result<Option<Node<'a>>, MathError> {
        let Some((text, span)) = self.parse_string_group(optional)? else {
            return Ok(None);
        };
        if !is_color(&text) {
            return Err(MathError::new(
                format!("Invalid color: '{}'", limits::clip(&text)),
                span,
            ));
        }
        // A six-digit hex code may leave out the `#`, as in xcolor's HTML model.
        let color = if text.len() == 6 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
            format!("#{text}")
        } else {
            text
        };
        Ok(Some(Node {
            kind: NodeKind::ColorToken(color),
            mode: self.mode,
            span,
        }))
    }
}

/// The checks `\includegraphics` makes of a size option: a bare number (in `bp`), or a number
/// and a unit as a size argument has.
pub(super) fn check_graphics_size(text: &str, span: &Range<usize>) -> Result<(), MathError> {
    if is_plain_number(text) {
        return Ok(());
    }
    let Some((_, unit)) = find_measurement(text) else {
        return Err(MathError::new(
            format!(
                "Invalid size: '{}' in \\includegraphics",
                limits::clip(text)
            ),
            span.clone(),
        ));
    };
    if !UNITS.contains(&unit) {
        return Err(MathError::new(
            format!("Invalid unit: '{unit}' in \\includegraphics."),
            span.clone(),
        ));
    }
    Ok(())
}

/// A number with an optional sign and no unit: `5`, `-2.`, `+ .5`.
fn is_plain_number(text: &str) -> bool {
    let rest = text
        .strip_prefix(['-', '+'])
        .unwrap_or(text)
        .trim_start_matches(' ');
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    match rest.split_once('.') {
        None => !rest.is_empty() && digits(rest),
        Some((whole, fraction)) => {
            digits(whole) && digits(fraction) && !(whole.is_empty() && fraction.is_empty())
        }
    }
}

/// Whether `text` could be the start of a size: an optional sign, a number with at most one
/// point, and up to two lowercase letters, with spaces allowed between the parts.
fn is_size_prefix(text: &str) -> bool {
    let bytes = text.as_bytes();
    let spaces = |mut i: usize| {
        while bytes.get(i) == Some(&b' ') {
            i += 1;
        }
        i
    };
    let mut i = 0;
    if matches!(bytes.first(), Some(b'-' | b'+')) {
        i += 1;
    }
    i = spaces(i);
    if i == bytes.len() {
        return true;
    }
    let digits = |mut i: usize| {
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    };
    let after_whole = digits(i);
    let mut j = after_whole;
    if bytes.get(j) == Some(&b'.') {
        j = digits(j + 1);
    } else if after_whole == i {
        // No digits and no point: not a number.
        return false;
    }
    j = spaces(j);
    let mut letters = 0;
    while bytes.get(j).is_some_and(u8::is_ascii_lowercase) && letters < 2 {
        j += 1;
        letters += 1;
    }
    spaces(j) == bytes.len()
}

/// The first number and two-letter unit in `text`: `1.5 em` is `(1.5, "em")`.
fn find_measurement(text: &str) -> Option<(f64, &str)> {
    // A longer text is not a size, and searching it for one at every offset is quadratic.
    if text.len() > limits::MAX_SIZE_TEXT {
        return None;
    }
    (0..text.len())
        .filter(|&start| text.is_char_boundary(start))
        .find_map(|start| measurement_at(&text[start..]))
}

/// A measurement at the very start of `text`: optional sign, spaces, a number, spaces, and two
/// lowercase letters.
fn measurement_at(text: &str) -> Option<(f64, &str)> {
    let bytes = text.as_bytes();
    let negative = bytes.first() == Some(&b'-');
    let mut i = usize::from(matches!(bytes.first(), Some(b'-' | b'+')));
    while bytes.get(i) == Some(&b' ') {
        i += 1;
    }
    let start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let whole = i > start;
    if bytes.get(i) == Some(&b'.') {
        let point = i;
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        // `.` alone is not a number; `5.` is.
        if !whole && i == point + 1 {
            return None;
        }
    } else if !whole {
        return None;
    }
    let magnitude = &text[start..i];
    while bytes.get(i) == Some(&b' ') {
        i += 1;
    }
    let unit = text.get(i..i + 2)?;
    if !unit.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let number: f64 = magnitude.parse().ok()?;
    Some((if negative { -number } else { number }, unit))
}

/// A color name, or a hex code with or without `#`: `red`, `#f00`, `#ff0000`, `ff0000`.
fn is_color(text: &str) -> bool {
    let hex = |s: &str| s.bytes().all(|b| b.is_ascii_hexdigit());
    match text.strip_prefix('#') {
        Some(code) => matches!(code.len(), 3 | 4 | 6 | 8) && hex(code),
        None => {
            !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphabetic())
                || (text.len() == 6 && hex(text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_prefixes() {
        for ok in [
            "", "-", "+ ", "1", "1.", ".", "1.5", "1.5e", "1.5em", "1.5 em", "- 2 pt ",
        ] {
            assert!(is_size_prefix(ok), "{ok:?}");
        }
        for bad in ["a", "1.5.", "1emm", "1 2", "--1", "e1"] {
            assert!(!is_size_prefix(bad), "{bad:?}");
        }
    }

    #[test]
    fn measurements() {
        assert_eq!(find_measurement("1em"), Some((1.0, "em")));
        assert_eq!(find_measurement("-0.17em"), Some((-0.17, "em")));
        assert_eq!(find_measurement("+ 3 mu"), Some((3.0, "mu")));
        assert_eq!(find_measurement(".5pt"), Some((0.5, "pt")));
        assert_eq!(find_measurement("5.pt"), Some((5.0, "pt")));
        assert_eq!(find_measurement("1"), None);
        assert_eq!(find_measurement("em"), None);
        // The search is not anchored, as KaTeX's.
        assert_eq!(find_measurement("x 2em"), Some((2.0, "em")));
    }

    #[test]
    fn colors() {
        for ok in [
            "red",
            "#f00",
            "#ff00",
            "#ff0000",
            "#ff000080",
            "ff0000",
            "Red",
        ] {
            assert!(is_color(ok), "{ok:?}");
        }
        for bad in ["", "#", "#ff", "#fffff", "red1", "#ggg", "12345"] {
            assert!(!is_color(bad), "{bad:?}");
        }
    }
}
