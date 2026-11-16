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

    /// The 1-based line and column (in characters) where the diagnostic starts in `src`. For
    /// several diagnostics in one source, [`LineIndex`] reads the source once.
    pub fn line_col(&self, src: &str) -> (usize, usize) {
        LineIndex::new(src).line_col(self)
    }
}

/// The line starts of a source, for finding many diagnostics' lines and columns without
/// reading it again for each.
#[derive(Debug)]
pub struct LineIndex<'a> {
    src: &'a str,
    /// The byte offset of each line's start, the first at 0.
    starts: Vec<usize>,
    /// The last position found: its line start, offset, and column less one, so a later
    /// position on the same line counts characters from there.
    last: std::cell::Cell<(usize, usize, usize)>,
}

impl<'a> LineIndex<'a> {
    pub fn new(src: &'a str) -> Self {
        let bytes = src.as_bytes();
        // A lone `\r` breaks the line too, as editors show it; `\r\n` is one break.
        let starts = std::iter::once(0)
            .chain(
                (bytes.iter().enumerate())
                    .filter(|&(i, &b)| {
                        b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n'))
                    })
                    .map(|(i, _)| i + 1),
            )
            .collect();
        LineIndex {
            src,
            starts,
            last: std::cell::Cell::new((0, 0, 0)),
        }
    }

    /// The 1-based line and column (in characters) where `diagnostic` starts.
    pub fn line_col(&self, diagnostic: &Diagnostic) -> (usize, usize) {
        let src = self.src;
        let mut offset = usize::from(diagnostic.range.start()).min(src.len());
        while !src.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|&s| s <= offset);
        // Between a `\r` and its `\n`, the column counts from the `\r`.
        let start = if offset > 0 && src.as_bytes()[offset - 1] == b'\r' {
            offset
        } else {
            self.starts[line - 1]
        };
        let (last_start, last_offset, last_col) = self.last.get();
        let col = if last_start == start && (start..=offset).contains(&last_offset) {
            last_col + src[last_offset..offset].chars().count()
        } else {
            src[start..offset].chars().count()
        };
        self.last.set((start, offset, col));
        (line, col + 1)
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
        assert_eq!(at(2).line_col(src), (1, 1));
    }

    /// One index answers like a fresh scan per diagnostic, in any order.
    #[test]
    fn index_matches_single_lookups() {
        let src = "ab\r\nçd\re\n\nlong line ç ç\r\n";
        let index = LineIndex::new(src);
        let offsets = (0..=src.len() as u32 + 2).chain((0..=src.len() as u32).rev());
        for o in offsets.chain([12, 14, 13, 20, 16, 1]) {
            let d = at(o);
            let mut off = usize::from(d.range.start()).min(src.len());
            while !src.is_char_boundary(off) {
                off -= 1;
            }
            let before = &src[..off];
            let line_start = before.rfind(['\n', '\r']).map_or(0, |i| i + 1);
            let line = before.matches('\n').count()
                + before
                    .match_indices('\r')
                    .filter(|&(i, _)| src.as_bytes().get(i + 1) != Some(&b'\n'))
                    .count()
                + 1;
            let scanned = (line, before[line_start..].chars().count() + 1);
            assert_eq!(index.line_col(&d), scanned, "offset {o}");
        }
    }
}
