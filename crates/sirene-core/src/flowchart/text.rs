//! Text decoding helpers for lowering: labels, styles, metadata, and click arguments.

use super::model::{Label, Style};

/// Content of a `STRING` or `MD_STRING` token, without its quotes (which may be missing when
/// the string is unclosed).
pub(crate) fn unquote(token: &str) -> &str {
    let (open, close) = if token.starts_with("\"`") {
        ("\"`", "`\"")
    } else {
        ("\"", "\"")
    };
    let s = token.strip_prefix(open).unwrap_or(token);
    s.strip_suffix(close).unwrap_or(s)
}

/// Decodes label text (FLOWMO-6): `<br>` line breaks, trimmed lines, and entity codes.
pub(crate) fn label(raw: &str, markdown: bool) -> Label {
    let text = replace_br(raw)
        .trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    Label {
        text: decode_entities(&text),
        markdown,
    }
}

fn replace_br(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        match br_len(tail) {
            Some(n) => {
                out.push('\n');
                rest = &tail[n..];
            }
            None => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Length of a `<br>`, `<br/>`, or `<br />` tag (any case) at the start of `s`.
fn br_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() < 4 || !b[1..3].eq_ignore_ascii_case(b"br") {
        return None;
    }
    let mut i = 3;
    while b.get(i) == Some(&b' ') {
        i += 1;
    }
    if b.get(i) == Some(&b'/') {
        i += 1;
    }
    (b.get(i) == Some(&b'>')).then_some(i + 1)
}

/// Decodes Mermaid's `#name;` and `#NNN;` entity codes, and HTML's `&name;` and `&#NNN;`;
/// unknown codes are kept verbatim.
pub(crate) fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['#', '&']) {
        out.push_str(&rest[..i]);
        let mark = &rest[i..i + 1];
        let tail = &rest[i + 1..];
        let decoded = tail.find(';').and_then(|end| {
            let code = &tail[..end];
            let code = match mark {
                "&" => code.strip_prefix('#').unwrap_or(code),
                _ => code,
            };
            let c = match code {
                "quot" => Some('"'),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ if !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()) => {
                    code.parse().ok().and_then(char::from_u32)
                }
                _ => None,
            };
            c.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push_str(mark);
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Splits style text into declarations (FLOWMO-12). Returns the declarations and the trimmed
/// text of any declaration without a `:`.
pub(crate) fn styles(text: &str) -> (Vec<Style>, Vec<String>) {
    let mut decls = Vec::new();
    let mut invalid = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    let mut flush = |current: &mut String| {
        let decl = current.trim();
        if !decl.is_empty() {
            match decl.split_once(':') {
                Some((p, v)) => decls.push(Style {
                    property: p.trim().to_owned(),
                    value: v.trim().to_owned(),
                }),
                None => invalid.push(decl.to_owned()),
            }
        }
        current.clear();
    };
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&',') => {
                chars.next();
                current.push(',');
            }
            ',' => flush(&mut current),
            _ => current.push(c),
        }
    }
    flush(&mut current);
    (decls, invalid)
}

/// Splits `s` at `sep` characters outside quotes. `'` quotes count only when `single` is set.
fn split_unquoted(s: &str, is_sep: impl Fn(char) -> bool, single: bool) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' && q == '"' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || (single && c == '\'') => quote = Some(c),
            None if is_sep(c) => {
                parts.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            None => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Parses a metadata body (FLOWMO-7). Returns the entries, with later keys replacing earlier
/// ones, and the trimmed text of any entry without a `:`.
pub(crate) fn metadata(body: &str) -> (Vec<(String, String)>, Vec<String>) {
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut invalid = Vec::new();
    for part in split_unquoted(body, |c| c == ',' || c == '\n', true) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, value)) = part.split_once(':') else {
            invalid.push(part.to_owned());
            continue;
        };
        let key = key.trim().to_owned();
        let value = yaml_scalar(value.trim());
        match entries.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => entries.push((key, value)),
        }
    }
    (entries, invalid)
}

/// One entry of a metadata body, with byte ranges into the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetaEntry {
    pub key: String,
    /// The entry without surrounding whitespace.
    pub entry: std::ops::Range<usize>,
    /// The value without surrounding whitespace.
    pub value: std::ops::Range<usize>,
}

/// The `key: value` entries of a metadata body in source order, each with where it is.
pub(crate) fn metadata_entries(body: &str) -> Vec<MetaEntry> {
    let base = body.as_ptr() as usize;
    split_unquoted(body, |c| c == ',' || c == '\n', true)
        .into_iter()
        .filter_map(|part| {
            let start = part.as_ptr() as usize - base;
            let lead = part.len() - part.trim_start().len();
            let trimmed = part.trim();
            let (key, value) = trimmed.split_once(':')?;
            let entry = start + lead..start + lead + trimmed.len();
            let value_start = entry.start + key.len() + 1;
            let value_lead = value.len() - value.trim_start().len();
            let value = value_start + value_lead..value_start + value_lead + value.trim().len();
            Some(MetaEntry {
                key: key.trim().to_owned(),
                entry,
                value,
            })
        })
        .collect()
}

/// A YAML scalar: double-quoted (with `\` escapes), single-quoted (with `''`), or plain.
fn yaml_scalar(v: &str) -> String {
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let mut out = String::new();
        let mut chars = v[1..v.len() - 1].chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        v[1..v.len() - 1].replace("''", "'")
    } else {
        v.to_owned()
    }
}

/// Splits callback arguments at commas outside double quotes, trimming and unquoting each
/// (FLOWMO-14).
pub(crate) fn callback_args(s: &str) -> Vec<String> {
    if s.trim().is_empty() {
        return Vec::new();
    }
    split_unquoted(s, |c| c == ',', false)
        .into_iter()
        .map(|a| {
            let a = a.trim();
            if a.len() >= 2 && a.starts_with('"') && a.ends_with('"') {
                a[1..a.len() - 1].to_owned()
            } else {
                a.to_owned()
            }
        })
        .collect()
}

/// The top-level `title:` value of a frontmatter block, including its `---` lines (FLOWMO-3).
pub(crate) fn frontmatter_title(block: &str) -> Option<String> {
    block.lines().find_map(|line| {
        let value = line.strip_prefix("title:")?;
        Some(yaml_scalar(value.trim())).filter(|t| !t.is_empty())
    })
}

/// The `key: value` lines under `config:` and then `section:` (such as `gantt:`) in a
/// frontmatter block, with quotes removed.
pub(crate) fn frontmatter_config(block: &str, section: &str) -> Vec<(String, String)> {
    let indent = |line: &str| line.len() - line.trim_start().len();
    let unquote = |s: &str| {
        let s = s.trim();
        for q in ['"', '\''] {
            if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
                return s[1..s.len() - 1].to_owned();
            }
        }
        s.to_owned()
    };
    let header = format!("{section}:");
    let mut out = Vec::new();
    let mut in_config = false;
    let mut section_at: Option<usize> = None;
    for line in block.lines() {
        if line.trim().is_empty() || line.trim() == "---" {
            continue;
        }
        let at = indent(line);
        if at == 0 {
            in_config = line.trim_end() == "config:";
            section_at = None;
            continue;
        }
        if !in_config {
            continue;
        }
        match section_at {
            Some(s) if at > s => {
                if let Some((k, v)) = line.trim().split_once(':') {
                    out.push((unquote(k), unquote(v)));
                }
            }
            _ => section_at = (line.trim() == header).then_some(at),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(p: &str, v: &str) -> Style {
        Style {
            property: p.into(),
            value: v.into(),
        }
    }

    #[test]
    fn metadata_entry_ranges() {
        let body = r#" shape: doc ,label: "a, b"
  bad
  x:1"#;
        let entries = metadata_entries(body);
        let parts: Vec<(&str, &str, &str)> = entries
            .iter()
            .map(|e| {
                (
                    e.key.as_str(),
                    &body[e.entry.clone()],
                    &body[e.value.clone()],
                )
            })
            .collect();
        assert_eq!(
            parts,
            [
                ("shape", "shape: doc", "doc"),
                ("label", r#"label: "a, b""#, r#""a, b""#),
                ("x", "x:1", "1"),
            ]
        );
        assert!(metadata_entries("").is_empty());
    }

    #[test]
    fn unquotes_strings() {
        assert_eq!(unquote("\"abc\""), "abc");
        assert_eq!(unquote("\"`md`\""), "md");
        assert_eq!(unquote("\"open"), "open");
        assert_eq!(unquote("\"`open"), "open");
        assert_eq!(unquote("\""), "");
    }

    #[test]
    fn decodes_labels() {
        assert_eq!(label("  hi  ", false).text, "hi");
        assert_eq!(label("a<br>b<BR/>c<br />d", false).text, "a\nb\nc\nd");
        assert_eq!(label("a <br> b", false).text, "a\nb");
        assert_eq!(label("one\n    two\n", true).text, "one\ntwo");
        assert_eq!(label("a<b>c</b> <br", false).text, "a<b>c</b> <br");
        assert_eq!(label("<bx>", false).text, "<bx>");
        assert!(label("x", true).markdown);
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("#quot;hi#quot;"), "\"hi\"");
        assert_eq!(decode_entities("#amp;#lt;#gt;#apos;#nbsp;"), "&<>'\u{a0}");
        assert_eq!(decode_entities("I #9829; you"), "I \u{2665} you");
        assert_eq!(decode_entities("#nope; #; # #12"), "#nope; #; # #12");
        assert_eq!(decode_entities("#99999999999;"), "#99999999999;");
        assert_eq!(decode_entities("#lt;br#gt;"), "<br>");
        assert_eq!(decode_entities("&nbsp;&amp;&#9829;"), "\u{a0}&\u{2665}");
        assert_eq!(decode_entities("a & b; &nope; &"), "a & b; &nope; &");
    }

    #[test]
    fn splits_styles() {
        let (decls, invalid) = styles("fill:#f9f, stroke : #333 ,stroke-width:4px,,");
        assert_eq!(
            decls,
            [
                style("fill", "#f9f"),
                style("stroke", "#333"),
                style("stroke-width", "4px")
            ]
        );
        assert!(invalid.is_empty());

        let (decls, invalid) = styles(r"font-family:a\,b,bogus,x:y:z");
        assert_eq!(decls, [style("font-family", "a,b"), style("x", "y:z")]);
        assert_eq!(invalid, ["bogus"]);

        let (decls, _) = styles(r"a:b\c");
        assert_eq!(decls, [style("a", r"b\c")]);
    }

    #[test]
    fn parses_metadata() {
        let (entries, invalid) = metadata(r#" shape: rect, label: "a, b: \"c\"\\" "#);
        assert_eq!(
            entries,
            [
                ("shape".to_owned(), "rect".to_owned()),
                ("label".to_owned(), "a, b: \"c\"\\".to_owned())
            ]
        );
        assert!(invalid.is_empty());

        let (entries, invalid) =
            metadata("\n  shape: hex\n  label: 'it''s'\n  shape: diam\n bad\n");
        assert_eq!(
            entries,
            [
                ("shape".to_owned(), "diam".to_owned()),
                ("label".to_owned(), "it's".to_owned())
            ]
        );
        assert_eq!(invalid, ["bad"]);

        let (entries, _) = metadata(r#"a: "x\ny\tz\"#);
        assert_eq!(entries, [("a".to_owned(), r#""x\ny\tz\"#.to_owned())]);
        let (entries, _) = metadata(r#"a: "\n\t\q\""#);
        assert_eq!(entries, [("a".to_owned(), "\n\tq\\".to_owned())]);
    }

    #[test]
    fn splits_callback_args() {
        assert!(callback_args("  ").is_empty());
        assert_eq!(callback_args(r#""a, b", c , "d""#), ["a, b", "c", "d"]);
        assert_eq!(callback_args(r#""x\"y""#), [r#"x\"y"#]);
    }

    #[test]
    fn reads_frontmatter_title() {
        let block = "---\ntitle: \"My chart\"\nconfig:\n  title: nested\n---";
        assert_eq!(frontmatter_title(block).as_deref(), Some("My chart"));
        assert_eq!(
            frontmatter_title("---\ntitle: plain\n---").as_deref(),
            Some("plain")
        );
        assert_eq!(frontmatter_title("---\ntitle:\n---"), None);
        assert_eq!(frontmatter_title("---\nconfig: {}\n---"), None);
    }
}
