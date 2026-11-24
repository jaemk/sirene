//! The one error type of the typesetter (LMATH-13).

use std::{fmt, ops::Range};

/// A problem in a formula: a message naming what went wrong, and the byte range in the formula's
/// source it is about, so a diagram can report it at the offending command (LEMB-5). The message
/// is KaTeX's wording where KaTeX has the same error, without its `KaTeX parse error:` prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathError {
    pub message: String,
    pub span: Range<usize>,
}

impl MathError {
    pub(crate) fn new(message: impl Into<String>, span: Range<usize>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for MathError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_the_message_and_the_span() {
        let e = MathError::new("Undefined control sequence: \\foo", 3..7);
        assert_eq!(e.to_string(), "Undefined control sequence: \\foo at 3..7");
        assert_eq!(MathError::new("", 0..0).to_string(), " at 0..0");
        let boxed: Box<dyn std::error::Error> = Box::new(e.clone());
        assert!(boxed.source().is_none());
        assert_eq!(boxed.to_string(), e.to_string());
    }
}
