//! Parse and validation diagnostics.

use rowan::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Mermaid would fail to render the diagram.
    Error,
    /// Mermaid would ignore or guess; the diagram still renders.
    Warning,
}

/// A problem found in the source, located by byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: TextRange,
    pub message: String,
    pub severity: Severity,
}

impl Diagnostic {
    /// An error diagnostic.
    pub fn new(range: TextRange, message: impl Into<String>) -> Self {
        Self {
            range,
            message: message.into(),
            severity: Severity::Error,
        }
    }

    pub fn warning(range: TextRange, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::new(range, message)
        }
    }

    /// The 1-based line and column (in characters) where the diagnostic starts in `src`.
    pub fn line_col(&self, src: &str) -> (usize, usize) {
        let mut offset = usize::from(self.range.start()).min(src.len());
        while !src.is_char_boundary(offset) {
            offset -= 1;
        }
        // A lone `\r` breaks the line too, as editors show it; `\r\n` is one break.
        let before = &src[..offset];
        let line_start = before.rfind(['\n', '\r']).map_or(0, |i| i + 1);
        let line = before.matches('\n').count()
            + before
                .match_indices('\r')
                .filter(|&(i, _)| src.as_bytes().get(i + 1) != Some(&b'\n'))
                .count()
            + 1;
        (line, before[line_start..].chars().count() + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::TextSize;

    fn at(offset: u32) -> Diagnostic {
        Diagnostic::new(TextRange::empty(TextSize::new(offset)), "m")
    }

    #[test]
    fn line_col() {
        let src = "ab\nçd\n";
        assert_eq!(at(0).line_col(src), (1, 1));
        assert_eq!(at(2).line_col(src), (1, 3));
        assert_eq!(at(3).line_col(src), (2, 1));
        assert_eq!(at(5).line_col(src), (2, 2));
        assert_eq!(at(7).line_col(src), (3, 1));
        assert_eq!(at(99).line_col(src), (3, 1));
        // Inside a multi-byte character.
        assert_eq!(at(4).line_col(src), (2, 1));
        // `\r\n` is one break, and a lone `\r` is one too.
        let src = "a\r\nb\rcd";
        assert_eq!(at(3).line_col(src), (2, 1));
        assert_eq!(at(6).line_col(src), (3, 2));
    }
}
