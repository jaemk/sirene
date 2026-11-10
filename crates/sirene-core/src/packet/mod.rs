//! Mermaid packet diagrams (MPKT-*): bit fields laid out in rows.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// The highest bit a packet draws, as in Mermaid.
pub const MAX_BIT: u32 = 9_999;

/// One field: bits `start` to `end`, inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub start: u32,
    pub end: u32,
    pub label: String,
}

impl Field {
    pub fn bits(&self) -> u32 {
        self.end - self.start + 1
    }
}

/// A field's part within one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub start: u32,
    pub end: u32,
    pub label: String,
}

impl Block {
    pub fn bits(&self) -> u32 {
        self.end - self.start + 1
    }
}

/// A packet diagram (MPKT-2).
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// Frontmatter `config.packet.bitsPerRow`, 32 unless set.
    pub bits_per_row: u32,
    /// Frontmatter `config.packet.showBits`, on unless set to `false`.
    pub show_bits: bool,
    /// Frontmatter `config.packet.bitOrder: descending`: each row's highest bit on the left.
    pub descending: bool,
    pub fields: Vec<Field>,
}

impl Default for Packet {
    fn default() -> Self {
        Packet {
            title: None,
            acc_title: None,
            acc_descr: None,
            bits_per_row: 32,
            show_bits: true,
            descending: false,
            fields: Vec::new(),
        }
    }
}

impl Packet {
    /// The rows from bit 0 to the last field, each with its fields' parts in it; a field that
    /// crosses a row's end continues on the next.
    pub fn rows(&self) -> Vec<Vec<Block>> {
        let per = self.bits_per_row.max(1);
        let n = self
            .fields
            .iter()
            .map(|f| f.end / per + 1)
            .max()
            .unwrap_or(0);
        let mut rows = vec![Vec::new(); n as usize];
        for f in &self.fields {
            let mut start = f.start;
            while start <= f.end {
                let row = start / per;
                let end = f.end.min((row + 1) * per - 1);
                rows[row as usize].push(Block {
                    start,
                    end,
                    label: f.label.clone(),
                });
                start = end + 1;
            }
        }
        rows
    }

    /// Where bit `b` sits within its row, counted from the left.
    pub fn column(&self, b: u32) -> u32 {
        let per = self.bits_per_row.max(1);
        let at = b % per;
        if self.descending { per - 1 - at } else { at }
    }
}

/// The result of parsing packet source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Packet`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Packet, Vec<Diagnostic>) {
        let (packet, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a packet: {} fields; {} diagnostics",
            packet.fields.len(),
            diagnostics.len()
        );
        (packet, diagnostics)
    }
}

/// Parses packet source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(start: u32, end: u32) -> Field {
        Field {
            start,
            end,
            label: format!("{start}"),
        }
    }

    #[test]
    fn rows_split_fields() {
        let p = Packet {
            bits_per_row: 8,
            fields: vec![field(0, 5), field(6, 17), field(20, 20)],
            ..Packet::default()
        };
        let rows: Vec<Vec<(u32, u32)>> = p
            .rows()
            .iter()
            .map(|r| r.iter().map(|b| (b.start, b.end)).collect())
            .collect();
        assert_eq!(
            rows,
            [
                vec![(0, 5), (6, 7)],
                vec![(8, 15)],
                vec![(16, 17), (20, 20)]
            ]
        );
        assert_eq!(p.column(9), 1);
        let d = Packet {
            descending: true,
            ..p
        };
        assert_eq!(d.column(9), 6);
        assert!(Packet::default().rows().is_empty());
    }
}
