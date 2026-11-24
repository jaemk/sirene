//! The bounds on a formula (LMATH-6, as LANGS-8 bounds every language).

use super::MathError;

/// A formula's source is at most 64 KiB.
pub(crate) const MAX_SOURCE: usize = 64 * 1024;

/// The TeX an AsciiMath source translates to is at most this long (the TeX lexer's own bound).
pub(crate) const MAX_TRANSLATED_TEX: usize = MAX_SOURCE;

/// Groups, scripts, and environments nest at most this deep (shared with every other language).
pub(crate) const MAX_DEPTH: usize = crate::lines::MAX_DEPTH;

/// Macro expansion runs at most this many steps.
pub(crate) const MAX_EXPANSIONS: usize = 10_000;

/// Macro expansion produces at most this many bytes of tokens.
pub(crate) const MAX_EXPANDED: usize = 1024 * 1024;

/// The tokens that must follow one parameter of a `\def` are at most this many. Reading a
/// delimited argument compares the end of what it has read with them at every token, so this
/// bounds that work at the size of the argument times this.
pub(crate) const MAX_DELIMITERS: usize = 256;

/// The text of a size argument written without braces (`\\kern1em`) is read up to this many
/// bytes; a longer run is not a size.
pub(crate) const MAX_SIZE_TEXT: usize = 64;

/// The number of column pairs `alignat` may be told to expect: it makes a column spec for each, even
/// for columns no row has.
pub(crate) const MAX_COLUMNS: usize = 256;

/// A length the source gives (`\kern`, `\rule`, `\raisebox`) is at most this many em either way,
/// as KaTeX's `maxSize` setting caps it from above; KaTeX's default has no cap.
pub(crate) const MAX_LENGTH_EM: f64 = 1000.0;

/// Source text echoed in an error message is cut to this many characters.
pub(crate) const MAX_ECHO: usize = 64;

#[cfg(feature = "math")]
/// A formula's display list holds at most this many items (glyphs, rules, paths).
pub(crate) const MAX_ITEMS: usize = 100_000;

#[cfg(feature = "math")]
/// A block of ASCII rows covers at most this many cells (rows times width).
pub(crate) const MAX_ASCII_CELLS: usize = 4 * 1024 * 1024;

/// An array or matrix has at most this many rows.
pub(crate) const MAX_ROWS: usize = 1024;

/// An array or matrix has at most this many cells in all.
pub(crate) const MAX_CELLS: usize = 16 * 1024;

#[cfg(feature = "math")]
/// A formula's width, ascent, or descent is at most this many em, and never negative.
pub(crate) const MAX_EXTENT_EM: f64 = 10_000.0;

/// `text` cut to `MAX_ECHO` characters, with an ASCII `...` after a cut.
pub(crate) fn clip(text: &str) -> String {
    match text.char_indices().nth(MAX_ECHO) {
        Some((at, _)) => format!("{}...", &text[..at]),
        None => text.to_string(),
    }
}

/// The error for a source past `MAX_SOURCE`; the span is the first byte over the limit.
pub(crate) fn check_source(src: &str) -> Result<(), MathError> {
    if src.len() <= MAX_SOURCE {
        return Ok(());
    }
    // Report at a character boundary at or before the limit.
    let mut at = MAX_SOURCE;
    while !src.is_char_boundary(at) {
        at -= 1;
    }
    Err(MathError::new(
        format!("formula longer than {MAX_SOURCE} bytes"),
        at..src.len(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_at_the_limit_passes() {
        assert!(check_source(&"x".repeat(MAX_SOURCE)).is_ok());
    }

    #[test]
    fn a_source_past_the_limit_fails_on_a_char_boundary() {
        let src = format!("{}{}", "x".repeat(MAX_SOURCE - 1), "\u{e9}\u{e9}");
        let err = check_source(&src).unwrap_err();
        assert!(src.is_char_boundary(err.span.start), "{err}");
        assert_eq!(err.span.end, src.len());
        assert_eq!(err.message, "formula longer than 65536 bytes");
    }
}
