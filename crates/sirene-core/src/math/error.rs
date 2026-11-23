//! The one error type of the typesetter (LMATH-13).

use std::{fmt, ops::Range};

/// A problem in a formula: a message naming what went wrong, and the byte range in the formula's
/// source it is about, so a diagram can report it at the offending command (LEMB-5). The message
/// is KaTeX's wording where KaTeX has the same error, without its `KaTeX parse error:` prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MathError {
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
