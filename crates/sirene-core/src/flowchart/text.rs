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

/// Decodes label text (FLOWMO-6): `<br>` line breaks, trimmed lines, and entity codes. In a
/// label that is not markdown, `<b>`/`<strong>` and `<i>`/`<em>` pairs become markdown bold and
/// italic (making the label markdown), and other HTML tags are dropped.
pub(crate) fn label(raw: &str, markdown: bool) -> Label {
    let (text, emphasis) = replace_html(raw, !markdown);
    decode_lines(&text, markdown || emphasis)
}

/// Like [`label`], but `<br>` is the only tag read: the label's text as written.
pub(crate) fn plain_label(raw: &str, markdown: bool) -> Label {
    let (text, _) = replace_html(raw, false);
    decode_lines(&text, markdown)
}

fn decode_lines(text: &str, markdown: bool) -> Label {
    let text = text
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

/// HTML tags dropped from a label (the emphasis tags `b`, `strong`, `i`, and `em` are read
/// separately).
const DROPPED_TAGS: &[&str] = &[
    "a", "abbr", "big", "center", "cite", "code", "del", "div", "font", "h1", "h2", "h3", "h4",
    "h5", "h6", "ins", "kbd", "li", "mark", "ol", "p", "pre", "q", "s", "samp", "small", "span",
    "strike", "sub", "sup", "table", "td", "th", "tr", "tt", "u", "ul", "var",
];

/// Length and name (lower case) of an HTML tag at the start of `s`, and whether it closes.
fn tag(s: &str) -> Option<(usize, String, bool)> {
    // The `>` before any other `<`, so a run of `<` is looked at once.
    let end = 1 + s.get(1..)?.find(['<', '>'])?;
    if s.as_bytes()[end] != b'>' {
        return None;
    }
    let inner = &s[1..end];
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name_len = inner
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(inner.len());
    let (name, after) = inner.split_at(name_len);
    // Attributes follow after a space; anything else glued to the name is not a tag.
    let ok = after.is_empty() || after.starts_with([' ', '\t', '/', '\n']);
    (ok && !name.is_empty()).then(|| (end + 1, name.to_ascii_lowercase(), closing))
}

/// Replaces `<br>` with a line break. With `html`, also turns `<b>`/`<strong>` and `<i>`/`<em>`
/// pairs into `**` and `*` markers (and reports whether it did) and drops [`DROPPED_TAGS`].
fn replace_html(s: &str, html: bool) -> (String, bool) {
    let mut out = String::with_capacity(s.len());
    let mut emphasis = false;
    // Open emphasis tags: bold, italic.
    let mut open = [0usize; 2];
    // Where the last closing bold and italic tags start, to tell whether one follows a point.
    let lower = s.to_ascii_lowercase();
    let last_close = [["</b>", "</strong>"], ["</i>", "</em>"]]
        .map(|partners| partners.iter().filter_map(|p| lower.rfind(p)).max());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(n) = br_len(tail) {
            out.push('\n');
            rest = &tail[n..];
            continue;
        }
        let known = html
            .then(|| tag(tail))
            .flatten()
            .and_then(|(n, name, closing)| {
                let kind = match name.as_str() {
                    "b" | "strong" => Some(0),
                    "i" | "em" => Some(1),
                    _ => None,
                };
                match kind {
                    Some(k) if closing => {
                        if open[k] > 0 {
                            open[k] -= 1;
                            out.push_str(["**", "*"][k]);
                        }
                        Some(n)
                    }
                    Some(k) => {
                        // Only a tag with a partner after it is emphasis.
                        let after = s.len() - tail.len() + n;
                        if last_close[k].is_some_and(|c| c >= after) {
                            open[k] += 1;
                            emphasis = true;
                            out.push_str(["**", "*"][k]);
                        }
                        Some(n)
                    }
                    None => DROPPED_TAGS.contains(&name.as_str()).then_some(n),
                }
            });
        match known {
            Some(n) => rest = &tail[n..],
            None => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    (out, emphasis)
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

/// Decodes Mermaid's `#name;` and `#NNN;` entity codes, and HTML's `&name;`, `&#NNN;`, and
/// `&#xHH;`; unknown codes are kept verbatim.
pub(crate) fn decode_entities(s: &str) -> String {
    decode(s, &['#', '&'])
}

/// Decodes HTML's `&name;`, `&#NNN;`, and `&#xHH;`; unknown codes are kept verbatim.
pub(crate) fn decode_html_entities(s: &str) -> String {
    decode(s, &['&'])
}

/// The character a named entity, or (with `numeric`) a decimal code, stands for.
fn entity(code: &str, numeric: bool) -> Option<char> {
    if numeric && !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()) {
        return code.parse().ok().and_then(char::from_u32);
    }
    super::entities::lookup(code)
}

/// The longest entity code read, in bytes: HTML's longest name is 31, and a code point is at
/// most 7 digits after `#` or `#x`, so this leaves room for leading zeros.
const MAX_ENTITY: usize = 40;

fn decode(s: &str, marks: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(marks) {
        out.push_str(&rest[..i]);
        let mark = &rest[i..i + 1];
        let tail = &rest[i + 1..];
        // Entity names and codes are short, so `;` is looked for only a little way ahead.
        let window = &tail.as_bytes()[..tail.len().min(MAX_ENTITY + 1)];
        let decoded = window.iter().position(|&b| b == b';').and_then(|end| {
            let code = &tail[..end];
            let c = match (mark, code.strip_prefix('#')) {
                ("&", Some(num)) => match num.strip_prefix(['x', 'X']) {
                    Some(hex) if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                        u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                    }
                    Some(_) => None,
                    None => entity(num, true),
                },
                ("&", None) => entity(code, false),
                _ => entity(code, true),
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
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut invalid = Vec::new();
    for part in metadata_parts(body) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, value)) = part.split_once(':') else {
            invalid.push(part.to_owned());
            continue;
        };
        let key = key.trim().to_owned();
        let value = yaml_value(value);
        match index.get(&key) {
            Some(&i) => entries[i].1 = value,
            None => {
                index.insert(key.clone(), entries.len());
                entries.push((key, value));
            }
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
    metadata_parts(body)
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

/// The entries of a metadata body, as Mermaid reads it: a body on one line is a YAML flow
/// mapping, split at commas outside quotes; a body over several lines is a block mapping, split
/// at lines, where lines indented more than an entry's (and blank lines among them) continue its
/// value.
fn metadata_parts(body: &str) -> Vec<&str> {
    if !body.contains('\n') {
        return split_unquoted(body, |c| c == ',', true);
    }
    let mut parts = Vec::new();
    // A body that starts with an entry (trimmed by the caller) does not show that entry's
    // indent: it is taken as the next entry line's.
    let indent_of = |l: &str| l.len() - l.trim_start().len();
    let first = body.lines().next().unwrap_or("");
    let first_indent = if first.trim().is_empty() {
        0
    } else {
        body.lines()
            .skip(1)
            .find(|l| {
                let key = l.trim_start().split(':').next().unwrap_or("");
                l.contains(':')
                    && !key.is_empty()
                    && key
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            })
            .map_or(indent_of(first), indent_of)
    };
    // The current entry's start and indent, and the end of its last non-blank line.
    let mut entry: Option<(usize, usize)> = None;
    let mut end = 0;
    let mut at = 0;
    for line in body.split_inclusive('\n') {
        let text = line.trim_end_matches(['\n', '\r']);
        let indent = if at == 0 {
            first_indent
        } else {
            indent_of(text)
        };
        let blank = text.trim().is_empty();
        match entry {
            _ if blank => {}
            Some((_, base)) if indent > base => {}
            _ => {
                if let Some((start, _)) = entry {
                    parts.push(&body[start..end]);
                }
                entry = Some((at, indent));
            }
        }
        if !blank {
            end = at + text.len();
        }
        at += line.len();
    }
    if let Some((start, _)) = entry {
        parts.push(&body[start..end]);
    }
    parts
}

/// A metadata value, the text after its key's `:`: a `|` (literal) or `>` (folded) block
/// scalar from the lines after it, without the final line break; or a scalar whose lines are
/// joined with spaces.
fn yaml_value(v: &str) -> String {
    let mut lines = v.lines();
    let head = lines.next().unwrap_or("").trim();
    let Some(style) = head.chars().next().filter(|c| *c == '|' || *c == '>') else {
        let folded: Vec<&str> = v.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        return yaml_scalar(&folded.join(" "));
    };
    let lines: Vec<&str> = lines.collect();
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let lines: Vec<&str> = lines
        .into_iter()
        .map(|l| l.get(indent..).unwrap_or("").trim_end())
        .collect();
    let mut out = String::new();
    if style == '|' {
        out = lines.join("\n");
    } else {
        // Folded: lines join with a space; a blank line is a line break.
        let mut gap = false;
        for line in lines {
            if line.is_empty() {
                out.push('\n');
                gap = true;
            } else {
                if !out.is_empty() && !gap {
                    out.push(' ');
                }
                out.push_str(line);
                gap = false;
            }
        }
    }
    out.trim_end_matches('\n').to_owned()
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
        fn parts(body: &str) -> Vec<(&str, &str, &str)> {
            metadata_entries(body)
                .into_iter()
                .map(|e| (&body[e.entry.clone()], &body[e.value.clone()]))
                .map(|(entry, value)| (entry.split(':').next().unwrap().trim(), entry, value))
                .collect()
        }
        assert_eq!(
            parts(r#" shape: doc ,label: "a, b", bad,x:1"#),
            [
                ("shape", "shape: doc", "doc"),
                ("label", r#"label: "a, b""#, r#""a, b""#),
                ("x", "x:1", "1"),
            ]
        );
        // Over several lines, an entry runs on through the lines indented more.
        assert_eq!(
            parts("\n  shape: doc\n  label: |\n    one\n\n    two\n  x: 1\n"),
            [
                ("shape", "shape: doc", "doc"),
                (
                    "label",
                    "label: |\n    one\n\n    two",
                    "|\n    one\n\n    two"
                ),
                ("x", "x: 1", "1"),
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
        assert_eq!(label("a<b>c</b> <br", false).text, "a**c** <br");
        assert_eq!(label("<bx>", false).text, "<bx>");
        assert!(label("x", true).markdown);
        assert!(!label("x", false).markdown);
    }

    #[test]
    fn reads_html_tags() {
        let l = label("<b>bold</b> and <i>it</i>", false);
        assert_eq!(l.text, "**bold** and *it*");
        assert!(l.markdown);
        assert_eq!(l.lines(), ["bold and it"]);
        let spans = &l.spans()[0];
        assert!(spans[0].bold && spans[2].italic);

        let l = label("<STRONG>a</Strong><em>b</em>", false);
        assert_eq!(l.text, "**a***b*");
        // Other tags are dropped, with their attributes.
        let l = label("x<span style=\"color:red\">y</span><u>z</u>", false);
        assert_eq!(l.text, "xyz");
        assert!(!l.markdown);
        // Without a partner, an emphasis tag is dropped; unknown tags are kept as written.
        assert_eq!(label("<b>x", false).text, "x");
        assert_eq!(label("a</b>", false).text, "a");
        assert!(!label("<b>x", false).markdown);
        assert_eq!(label("List<T> a < b > c", false).text, "List<T> a < b > c");
        // Markdown labels keep their text.
        assert_eq!(label("<b>x</b>", true).text, "<b>x</b>");
        // Escaped tags are text.
        assert_eq!(label("#lt;b#gt;", false).text, "<b>");
        // The editor reads only `<br>`.
        assert_eq!(plain_label("<b>x</b><br>y", false).text, "<b>x</b>\ny");
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
        assert_eq!(
            decode_entities("&#x2665;&#X41;&#x;&#xZ;"),
            "\u{2665}A&#x;&#xZ;"
        );
        // The HTML 4 names, in both forms.
        assert_eq!(
            decode_entities("#copy; 2024 #hearts; &rarr; &mdash; &Alpha;&alpha;"),
            "\u{a9} 2024 \u{2665} \u{2192} \u{2014} \u{391}\u{3b1}"
        );
        assert_eq!(decode_html_entities("&eacute;&euro;"), "\u{e9}\u{20ac}");
        // A numeral after `&` needs its `#`.
        assert_eq!(decode_entities("&9829;"), "&9829;");
    }

    #[test]
    fn long_runs_are_kept() {
        assert_eq!(tag("<<<<"), None);
        assert_eq!(tag("<b<b>"), None);
        let lt = "<".repeat(10_000);
        assert_eq!(label(&lt, false).text, lt);
        let open = "<b>".repeat(1000) + "x";
        assert_eq!(label(&open, false).text, "x");
        assert_eq!(label("<b>a</b><b>b</b>", false).text, "**a****b**");
        let amp = "&".repeat(10_000);
        assert_eq!(decode_entities(&amp), amp);
        let long = format!("&{};", "a".repeat(MAX_ENTITY + 5));
        assert_eq!(decode_entities(&long), long);
        assert_eq!(decode_entities("&amp;&amp;"), "&&");
    }

    #[test]
    fn decodes_html_entities_only() {
        assert_eq!(decode_html_entities("#amp; &amp; #9829;"), "#amp; & #9829;");
        assert_eq!(decode_html_entities("&amp;lt;"), "&lt;");
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

        // Over several lines (a YAML block mapping), commas are text, and `|` and `>` read the
        // lines after them.
        let (entries, invalid) = metadata(
            "\n  label: Hello, world\n  a: |\n    one\n      two\n\n    three\n  b: >\n    one\n    two\n\n    three\n  c: long\n    plain\n",
        );
        assert_eq!(
            entries,
            [
                ("label".to_owned(), "Hello, world".to_owned()),
                ("a".to_owned(), "one\n  two\n\nthree".to_owned()),
                ("b".to_owned(), "one two\nthree".to_owned()),
                ("c".to_owned(), "long plain".to_owned()),
            ]
        );
        assert!(invalid.is_empty());
        // A body trimmed before it is read: the first entry is at the next one's level.
        let (entries, _) = metadata("a: 1\n      b: 2\n      c: |\n        x\n    ");
        assert_eq!(
            entries,
            [
                ("a".to_owned(), "1".to_owned()),
                ("b".to_owned(), "2".to_owned()),
                ("c".to_owned(), "x".to_owned()),
            ]
        );
        let (entries, _) = metadata("label: |\n  one\n  two");
        assert_eq!(entries, [("label".to_owned(), "one\ntwo".to_owned())]);

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
