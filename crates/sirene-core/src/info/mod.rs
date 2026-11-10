//! Mermaid info diagrams (MINFO-*): the version of the tool drawing them.

mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

/// sirene's version, the text an info diagram draws.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// An info diagram (MINFO-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Info {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// `showInfo`: read, and drawn the same as without it.
    pub show_info: bool,
}

impl Info {
    /// The drawn text: `sirene v` and the version.
    pub fn text(&self) -> String {
        format!("sirene v{VERSION}")
    }
}

/// The result of parsing info diagram source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to an [`Info`], with the parser's diagnostics.
    pub fn lower(&self) -> (Info, Vec<Diagnostic>) {
        let root = self.syntax();
        let mut info = Info::default();
        for el in root.descendants_with_tokens() {
            let Some(tok) = el.as_token() else { continue };
            let parent = tok.parent().map(|p| p.kind());
            match tok.kind() {
                FRONTMATTER => info.title = text::frontmatter_title(tok.text()),
                SHOW_INFO_KW => info.show_info = true,
                TEXT if parent == Some(TITLE_STMT) => {
                    let t = tok.text().trim();
                    if !t.is_empty() {
                        info.title = Some(t.to_owned());
                    }
                }
                ACC_TEXT if parent == Some(ACC_TITLE_STMT) => {
                    info.acc_title = Some(tok.text().trim().to_owned());
                }
                ACC_TEXT if parent == Some(ACC_DESCR_STMT) => {
                    info.acc_descr = Some(tok.text().trim().to_owned());
                }
                _ => {}
            }
        }
        log::debug!("lowered an info diagram");
        (info, self.diagnostics.clone())
    }
}

/// Parses info diagram source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
