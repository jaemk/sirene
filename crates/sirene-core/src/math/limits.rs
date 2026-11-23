//! The bounds on a formula (LMATH-6, as LANGS-8 bounds every language).

use super::MathError;

/// A formula's source is at most 64 KiB.
pub(crate) const MAX_SOURCE: usize = 64 * 1024;

/// Groups, scripts, and environments nest at most this deep (shared with every other language).
pub(crate) const MAX_DEPTH: usize = crate::lines::MAX_DEPTH;

/// Macro expansion runs at most this many steps.
pub(crate) const MAX_EXPANSIONS: usize = 10_000;

/// Macro expansion produces at most this many bytes of tokens.
pub(crate) const MAX_EXPANDED: usize = 1024 * 1024;

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
