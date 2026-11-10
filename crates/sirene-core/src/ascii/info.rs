//! Info diagrams as text (MINFO-4).

use super::Options;
use crate::info::Info;

/// Renders `info` as text: the title, if any, then sirene's version (MINFO-4).
pub fn render(info: &Info, _options: &Options) -> String {
    let mut out = String::new();
    if let Some(t) = info
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        out.push_str(t);
        out.push_str("\n\n");
    }
    out.push_str(&info.text());
    out.push('\n');
    out
}
