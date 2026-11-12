//! The statement lines of the line-read PlantUML kinds (PUML-2), and the statements every kind
//! shares: titles, settings, preprocessor directives, and the `left to right direction` switch.

use rowan::{TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::flowchart::Direction;
use crate::syntax::{SyntaxKind, SyntaxNode};

/// One statement line, trimmed, with its range in the source.
#[derive(Debug, Clone)]
pub(crate) struct Line {
    pub text: String,
    pub range: TextRange,
}

impl Line {
    /// The range of `part`, a subslice of `self.text`.
    pub fn range_of(&self, part: &str) -> TextRange {
        let base = self.text.as_ptr() as usize;
        let at = (part.as_ptr() as usize)
            .checked_sub(base)
            .filter(|&at| at + part.len() <= self.text.len())
            .unwrap_or(0);
        let start = self.range.start() + TextSize::try_from(at).expect("line fits");
        TextRange::at(start, TextSize::try_from(part.len()).expect("line fits"))
    }

    /// The first word, lowercased.
    pub fn word(&self) -> String {
        word(&self.text).to_ascii_lowercase()
    }

    /// The text after the first word, trimmed.
    pub fn after_word(&self) -> &str {
        self.text[word(&self.text).len()..].trim()
    }

    /// The text lowercased with each run of whitespace as one space, to match closers such as
    /// `end note`.
    pub fn squeezed(&self) -> String {
        squeeze(&self.text)
    }
}

/// The word at the start of `s`: letters, digits, and `_`.
pub(crate) fn word(s: &str) -> &str {
    &s[..s
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())]
}

/// `s` lowercased with each run of whitespace as one space.
pub(crate) fn squeeze(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Whether `s` starts with the keyword `kw`, in any case, followed by a non-word character.
pub(crate) fn keyword<'a>(s: &'a str, kw: &str) -> Option<&'a str> {
    let w = word(s);
    w.eq_ignore_ascii_case(kw)
        .then(|| s[w.len()..].trim_start())
}

/// The statement lines of a line-read document.
pub(crate) fn read(root: &SyntaxNode) -> Vec<Line> {
    root.descendants()
        .filter(|n| n.kind() == SyntaxKind::TEXT_STMT)
        .map(|n| Line {
            text: n.text().to_string(),
            range: n.text_range(),
        })
        .collect()
}

/// A PlantUML color (`#red`, `#LightBlue`, `#ff8800`, `red`) as `#rrggbb`; a gradient such as
/// `#red/blue` or `#red|blue` gives its first color.
pub(crate) fn color(s: &str) -> Option<String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    let s = s.split(['/', '|', '\\', '-']).next().unwrap_or(s);
    if matches!(s.len(), 3 | 6 | 8) && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return crate::dot::lower::color(&format!("#{s}")).map(crate::dot::lower::hex);
    }
    crate::dot::lower::color(s).map(crate::dot::lower::hex)
}

/// `\n` in PlantUML text breaks the line; other escapes are kept as written.
pub(crate) fn text(s: &str) -> String {
    s.replace("\\n", "\n")
}

/// A `"quoted"` string's contents, or `s` itself.
pub(crate) fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}

/// The text of a `(...)` group at the start of `s`, and the rest after it, for `if (x)`,
/// `then (yes)`, and the like. Parentheses nest.
pub(crate) fn parens(s: &str) -> Option<(&str, &str)> {
    let body = s.strip_prefix('(')?;
    let mut depth = 1;
    for (i, c) in body.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&body[..i], body[i + 1..].trim_start()));
                }
            }
            _ => {}
        }
    }
    None
}

/// Where an unquoted name ends: at whitespace, a body, a color, a stereotype or generics, a
/// link, or a single `:` (`a::B` is one name).
pub(crate) fn name_end(s: &str) -> usize {
    let b = s.as_bytes();
    s.char_indices()
        .find(|&(i, c)| {
            c.is_whitespace()
                || matches!(c, '{' | '#' | '<' | '[')
                || (c == ':' && b.get(i + 1) != Some(&b':') && (i == 0 || b[i - 1] != b':'))
        })
        .map_or(s.len(), |(i, _)| i)
}

/// A name with an optional display string: `A`, `"Long name"`, `"Long name" as A`,
/// `A as "Long name"`, `Box<T>`. Returns the id, the label, and the rest of the line.
pub(crate) fn name(s: &str) -> Option<(String, String, &str)> {
    let s = s.trim_start();
    let (first, rest) = if let Some(body) = s.strip_prefix('"') {
        let close = body.find('"')?;
        (&s[..close + 2], body[close + 1..].trim_start())
    } else {
        let mut end = name_end(s);
        // Generics: `Box<T>` but not a stereotype `<<s>>`.
        if s[end..].starts_with('<') && !s[end..].starts_with("<<") {
            let mut depth = 0;
            for (i, c) in s[end..].char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            end += i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        (&s[..end], s[end..].trim_start())
    };
    if first.is_empty() {
        return None;
    }
    let display = unquote(first);
    if let Some(after) = keyword(rest, "as") {
        let (alias, rest) = match after.strip_prefix('"') {
            Some(body) => {
                let close = body.find('"')?;
                (&after[..close + 2], body[close + 1..].trim_start())
            }
            None => {
                let end = name_end(after);
                (&after[..end], after[end..].trim_start())
            }
        };
        // `"Display" as Id` or `Id as "Display"`.
        let (id, label) = if first.starts_with('"') || !alias.starts_with('"') {
            (unquote(alias), display)
        } else {
            (display, unquote(alias))
        };
        return Some((id.to_owned(), text(label), rest));
    }
    Some((id_of(display), text(display), rest))
}

/// The id a name refers to: the text without quotes or generics.
pub(crate) fn id_of(s: &str) -> String {
    let s = unquote(s.trim());
    match s.find('<') {
        Some(at) if at > 0 => s[..at].to_owned(),
        _ => s.to_owned(),
    }
}

/// A stereotype's name, without a `(S,#color)` spot.
pub(crate) fn stereotype(s: &str) -> String {
    let s = s.trim();
    let s = match s.strip_prefix('(') {
        Some(r) => r.find(')').map_or(s, |i| r[i + 1..].trim()),
        None => s,
    };
    s.to_owned()
}

/// Reads a statement's lines one at a time.
pub(crate) struct Cursor {
    lines: Vec<Line>,
    at: usize,
}

impl Cursor {
    pub fn new(lines: Vec<Line>) -> Self {
        Self { lines, at: 0 }
    }

    pub fn next(&mut self) -> Option<Line> {
        let line = self.lines.get(self.at).cloned();
        self.at += 1;
        line
    }

    /// The lines up to one whose squeezed text is one of `closers`, and that closer; `None` for
    /// the closer when the input ends first.
    pub fn until(&mut self, closers: &[&str]) -> (Vec<Line>, Option<Line>) {
        let mut body = Vec::new();
        while let Some(line) = self.next() {
            if closers.contains(&line.squeezed().as_str()) {
                return (body, Some(line));
            }
            body.push(line);
        }
        (body, None)
    }
}

/// What a statement every kind shares did.
pub(crate) enum Common {
    /// Not a shared statement.
    No,
    /// Read, with nothing for the model.
    Done,
    Title(String),
    Direction(Direction),
}

/// Reads a statement every kind accepts (PSEQ-6, PUML-4): `title`, the settings that are not
/// drawn (`header`, `footer`, `caption`, `legend`, `skinparam`, `scale`, ...), preprocessor
/// directives, and `left to right direction` / `top to bottom direction`.
pub(crate) fn common(line: &Line, cursor: &mut Cursor, d: &mut Vec<Diagnostic>) -> Common {
    let t = line.text.as_str();
    let w = line.word();
    if let Some(directive) = t.strip_prefix('!') {
        let name = word(directive).to_ascii_lowercase();
        let r = line.range_of(t);
        if [
            "include",
            "includeurl",
            "includesub",
            "import",
            "include_many",
            "include_once",
        ]
        .contains(&name.as_str())
        {
            d.push(Diagnostic::new(
                r,
                format!("`!{name}` reads other files and is not supported"),
            ));
        } else {
            d.push(Diagnostic::warning(
                r,
                format!("`!{name}` is not supported; ignoring the line"),
            ));
        }
        return Common::Done;
    }
    match line.squeezed().as_str() {
        "left to right direction" => return Common::Direction(Direction::LeftRight),
        "top to bottom direction" => return Common::Direction(Direction::TopBottom),
        _ => {}
    }
    match w.as_str() {
        "title" => {
            let rest = line.after_word();
            if rest.is_empty() {
                let (body, end) = cursor.until(&["end title", "endtitle"]);
                if end.is_none() {
                    d.push(Diagnostic::new(
                        line.range,
                        "unclosed `title`: expected `end title`",
                    ));
                }
                let lines: Vec<&str> = body.iter().map(|l| l.text.as_str()).collect();
                return Common::Title(lines.join("\n"));
            }
            Common::Title(text(rest.strip_prefix(':').unwrap_or(rest).trim()))
        }
        "header" | "footer" | "legend" | "caption" | "skinparam" | "scale" | "mainframe"
        | "skin" | "allowmixing" | "allow_mixing" | "set" | "hide" | "show" | "remove"
        | "restore" | "newpage" | "page" | "pragma" | "style" => {
            let block_closer = match w.as_str() {
                "legend" => Some(["end legend", "endlegend"]),
                "header" => Some(["end header", "endheader"]),
                "footer" => Some(["end footer", "endfooter"]),
                _ => None,
            };
            if let Some(closers) = block_closer
                && line.after_word().split_whitespace().all(|s| {
                    ["left", "right", "center", "top", "bottom"]
                        .contains(&s.to_ascii_lowercase().as_str())
                })
            {
                let (_, end) = cursor.until(&closers);
                if end.is_none() {
                    d.push(Diagnostic::new(
                        line.range,
                        format!("unclosed `{w}`: expected `{}`", closers[0]),
                    ));
                }
            } else if line.text.trim_end().ends_with('{') {
                // A `skinparam x {` or `<style>` block runs to its `}`.
                let mut depth = 1;
                while depth > 0 {
                    let Some(next) = cursor.next() else { break };
                    depth += next.text.matches('{').count();
                    depth -= next.text.matches('}').count().min(depth);
                }
            }
            d.push(Diagnostic::warning(
                line.range_of(word(t)),
                format!("`{w}` is not drawn yet; ignoring it"),
            ));
            Common::Done
        }
        _ if t.starts_with("<style>") => {
            let _ = cursor.until(&["</style>"]);
            d.push(Diagnostic::warning(
                line.range,
                "`<style>` is not drawn yet; ignoring it",
            ));
            Common::Done
        }
        _ => Common::No,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(parens("(a (b)) then"), Some(("a (b)", "then")));
        assert_eq!(parens("(a"), None);
        assert_eq!(keyword("If (x)", "if"), Some("(x)"));
        assert_eq!(keyword("iffy", "if"), None);
        assert_eq!(unquote("\"x y\""), "x y");
        assert_eq!(text("a\\nb"), "a\nb");
        let line = Line {
            text: "note left".into(),
            range: TextRange::at(10.into(), 9.into()),
        };
        assert_eq!(
            line.range_of(&line.text[5..]),
            TextRange::at(15.into(), 4.into())
        );
        assert_eq!(line.after_word(), "left");
    }
}
