//! PlantUML JSON and YAML documents (PDATA-*): the data read as a tree of objects, arrays, and
//! scalars, and drawn left to right as one table per object or array, each nested value
//! linked from its row.

use rowan::{TextRange, TextSize};

use super::lines;
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Direction, Edge, Flowchart, Label, Node, Row, Shape};
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;

/// A value of the document's data.
#[derive(Debug, Clone, PartialEq)]
enum Data {
    /// A string, number, boolean, or null, as drawn.
    Scalar(String),
    Object(Vec<(String, Value)>),
    Array(Vec<Value>),
}

#[derive(Debug, Clone, PartialEq)]
struct Value {
    data: Data,
    range: TextRange,
}

/// A problem at a byte offset.
type Error = (TextRange, String);

fn at(start: usize, len: usize) -> TextRange {
    let n = |x: usize| TextSize::try_from(x).expect("source fits");
    TextRange::at(n(start), n(len))
}

fn too_deep(start: usize) -> Error {
    (
        at(start, 1),
        format!("values nest more than {MAX_DEPTH} levels deep"),
    )
}

/// The document's data at its source offsets (`lines::body`). `<style>` blocks and `#highlight`
/// lines are blanked, with a warning.
fn body(root: &SyntaxNode, d: &mut Vec<Diagnostic>) -> String {
    let mut style: Option<TextRange> = None;
    let body = lines::body(root, |line| {
        let t = line.text.trim();
        if let Some(open) = style {
            if t.eq_ignore_ascii_case("</style>") {
                d.push(Diagnostic::warning(
                    open.cover(line.range),
                    "`<style>` is not drawn",
                ));
                style = None;
            }
            return false;
        }
        if t.eq_ignore_ascii_case("<style>") {
            style = Some(line.range);
            return false;
        }
        if lines::keyword(t.strip_prefix('#').unwrap_or(""), "highlight").is_some() {
            d.push(Diagnostic::warning(
                line.range,
                "`#highlight` is drawn without highlighting",
            ));
            return false;
        }
        true
    });
    if let Some(open) = style {
        d.push(Diagnostic::new(open, "`<style>` has no `</style>`"));
    }
    body
}

// ---- JSON ----

/// A JSON reader. At the first error it records it and stops, keeping what it read: the
/// collections still open are closed where the error is.
struct Json<'a> {
    s: &'a str,
    i: usize,
    depth: usize,
    error: Option<Error>,
}

impl Json<'_> {
    fn space(&mut self) {
        let rest = &self.s[self.i..];
        self.i += rest.len() - rest.trim_start().len();
    }

    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    /// Records the first error and skips to the end, so every open collection ends.
    fn fail(&mut self, e: Error) {
        self.error.get_or_insert(e);
        self.i = self.s.len();
    }

    fn expected(&mut self, what: &str) {
        self.space();
        let e = match self.peek() {
            Some(c) => (
                at(self.i, c.len_utf8()),
                format!("expected {what}, found `{c}`"),
            ),
            None => (at(self.i, 0), format!("expected {what}")),
        };
        self.fail(e);
    }

    fn eat(&mut self, c: char) -> bool {
        self.space();
        let ok = self.peek() == Some(c);
        if ok {
            self.i += 1;
        }
        ok
    }

    /// The value at the cursor, or `None` after an error before any of it.
    fn value(&mut self) -> Option<Value> {
        self.space();
        let start = self.i;
        let data = match self.peek() {
            Some(open @ ('{' | '[')) => {
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    self.fail(too_deep(start));
                    return None;
                }
                self.i += 1;
                let data = if open == '{' {
                    Data::Object(self.object())
                } else {
                    Data::Array(self.array())
                };
                self.depth -= 1;
                data
            }
            Some('"') => Data::Scalar(self.string()?),
            Some(c) if c == '-' || c.is_ascii_alphanumeric() => {
                let rest = &self.s[self.i..];
                let len = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '.')))
                    .unwrap_or(rest.len());
                let word = &rest[..len];
                // Drawn as written, so a number out of `f64` range is still a number.
                let number = word.parse::<f64>().is_ok()
                    && !word.starts_with('+')
                    && !word.contains(|c: char| c.is_ascii_alphabetic() && c != 'e' && c != 'E');
                if !(number || matches!(word, "true" | "false" | "null")) {
                    let e = (
                        at(self.i, len),
                        format!("expected a JSON value, found `{word}`"),
                    );
                    self.fail(e);
                    return None;
                }
                self.i += len;
                Data::Scalar(word.to_owned())
            }
            _ => {
                self.expected("a JSON value");
                return None;
            }
        };
        Some(Value {
            data,
            range: at(start, self.i - start),
        })
    }

    /// An object's pairs, after its `{`.
    fn object(&mut self) -> Vec<(String, Value)> {
        let mut pairs = Vec::new();
        if self.eat('}') {
            return pairs;
        }
        loop {
            self.space();
            if self.peek() != Some('"') {
                self.expected("a `\"key\"`");
                break;
            }
            let Some(key) = self.string() else { break };
            if !self.eat(':') {
                self.expected("`:`");
                break;
            }
            let Some(v) = self.value() else { break };
            pairs.push((key, v));
            if self.eat('}') {
                break;
            }
            if !self.eat(',') {
                self.expected("`,` or `}`");
                break;
            }
        }
        pairs
    }

    /// An array's items, after its `[`.
    fn array(&mut self) -> Vec<Value> {
        let mut items = Vec::new();
        if self.eat(']') {
            return items;
        }
        while let Some(v) = self.value() {
            items.push(v);
            if self.eat(']') {
                break;
            }
            if !self.eat(',') {
                self.expected("`,` or `]`");
                break;
            }
        }
        items
    }

    /// A string at the cursor's `"`, unescaped.
    fn string(&mut self) -> Option<String> {
        let start = self.i;
        self.i += 1;
        let mut out = String::new();
        let mut chars = self.s[self.i..].char_indices();
        while let Some((k, c)) = chars.next() {
            match c {
                '"' => {
                    self.i += k + 1;
                    return Some(out);
                }
                '\n' => break,
                '\\' => {
                    let Some((_, e)) = chars.next() else { break };
                    match e {
                        'n' | 'r' | 't' => out.push(' '),
                        'b' | 'f' => {}
                        'u' => {
                            let mut c = hex4(&mut chars);
                            // A high surrogate joins the low one after it.
                            if let Some(high @ 0xd800..=0xdbff) = c {
                                let mut ahead = chars.clone();
                                if let (Some((_, '\\')), Some((_, 'u'))) =
                                    (ahead.next(), ahead.next())
                                    && let Some(low @ 0xdc00..=0xdfff) = hex4(&mut ahead)
                                {
                                    chars = ahead;
                                    c = Some(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00));
                                }
                            }
                            out.push(c.and_then(char::from_u32).unwrap_or('\u{fffd}'));
                        }
                        other => out.push(other),
                    }
                }
                c => out.push(c),
            }
        }
        self.fail((at(start, 1), "the string has no closing `\"`".into()));
        None
    }
}

/// The value of up to four hex digits after `\u`, taking only the digits; `None` unless there
/// are four.
fn hex4(chars: &mut std::str::CharIndices<'_>) -> Option<u32> {
    let mut v = 0;
    for _ in 0..4 {
        let mut ahead = chars.clone();
        let d = ahead.next().and_then(|(_, c)| c.to_digit(16))?;
        *chars = ahead;
        v = v * 16 + d;
    }
    Some(v)
}

/// The document's value, if any, and the first error.
fn json(s: &str) -> (Option<Value>, Option<Error>) {
    let mut p = Json {
        s,
        i: 0,
        depth: 0,
        error: None,
    };
    p.space();
    if p.peek().is_none() {
        return (None, None);
    }
    let v = p.value();
    p.space();
    if p.peek().is_some() {
        p.expected("the end of the document");
    }
    (v, p.error)
}

// ---- YAML ----

/// One YAML line: its indentation and its text with any comment removed.
#[derive(Debug, Clone)]
struct YLine {
    indent: usize,
    text: String,
    /// Offset of `text` in the source.
    start: usize,
}

impl YLine {
    fn range(&self) -> TextRange {
        at(self.start, self.text.len())
    }

    fn is_item(&self) -> bool {
        self.text == "-" || self.text.starts_with("- ")
    }
}

/// The offset of a `#` comment in `s`, outside quotes.
fn comment(s: &str) -> Option<usize> {
    let mut quote = None;
    let mut prev = ' ';
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && prev.is_whitespace() => return Some(i),
            None => {}
        }
        prev = c;
    }
    None
}

/// The offset of a mapping's `:` in `s`, outside quotes and brackets: one followed by a space or
/// the end.
fn colon(s: &str) -> Option<usize> {
    let mut quote = None;
    let mut depth = 0usize;
    let b = s.as_bytes();
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' if i == 0 => quote = Some(c),
                '[' | '{' => depth += 1,
                ']' | '}' => depth = depth.saturating_sub(1),
                ':' if depth == 0 && b.get(i + 1).is_none_or(|b| b.is_ascii_whitespace()) => {
                    return Some(i);
                }
                _ => {}
            },
        }
    }
    None
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return s[1..s.len() - 1].replace("\\\"", "\"").replace("\\n", " ");
    }
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return s[1..s.len() - 1].replace("''", "'");
    }
    s.to_owned()
}

/// The parts of a flow collection's inside, split at its top-level commas.
fn flow_items(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut from) = (0usize, None, 0);
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '[' | '{' => depth += 1,
                ']' | '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    out.push(s[from..i].trim());
                    from = i + 1;
                }
                _ => {}
            },
        }
    }
    out.push(s[from..].trim());
    out.retain(|p| !p.is_empty());
    out
}

/// A YAML reader over the document's lines. Like [`Json`], it stops at the first error and keeps
/// what it read.
struct Yaml {
    lines: Vec<YLine>,
    i: usize,
    depth: usize,
    error: Option<Error>,
}

impl Yaml {
    fn new(body: &str) -> Self {
        let mut lines = Vec::new();
        let mut offset = 0;
        for raw in body.split('\n') {
            let start = offset;
            offset += raw.len() + 1;
            let text = match comment(raw) {
                Some(c) => &raw[..c],
                None => raw,
            };
            let trimmed = text.trim();
            if trimmed.is_empty()
                || trimmed == "---"
                || trimmed == "..."
                || trimmed.starts_with('%')
            {
                continue;
            }
            let indent = text.len() - text.trim_start().len();
            lines.push(YLine {
                indent,
                text: trimmed.to_owned(),
                start: start + indent,
            });
        }
        Yaml {
            lines,
            i: 0,
            depth: 0,
            error: None,
        }
    }

    /// Records the first error and skips the remaining lines, so every open collection ends.
    fn fail(&mut self, range: TextRange, message: impl Into<String>) {
        self.error.get_or_insert((range, message.into()));
        self.i = self.lines.len();
    }

    /// The document's value, if any, and the first error.
    fn document(mut self) -> (Option<Value>, Option<Error>) {
        let Some(first) = self.lines.first() else {
            return (None, None);
        };
        let (first_is_item, indent) = (first.is_item(), first.indent);
        let v = self.node(indent);
        if let Some(l) = self.lines.get(self.i) {
            let what = if first_is_item {
                "`- item`"
            } else {
                "`key: value`"
            };
            let range = l.range();
            self.fail(range, format!("expected {what} at this indentation"));
        }
        (Some(v), self.error)
    }

    fn node(&mut self, indent: usize) -> Value {
        let l = &self.lines[self.i];
        let range = l.range();
        if self.depth >= MAX_DEPTH {
            let (r, m) = too_deep(l.start);
            self.fail(r, m);
            return Value {
                data: Data::Scalar(String::new()),
                range,
            };
        }
        self.depth += 1;
        let v = if l.is_item() {
            self.sequence(indent)
        } else if colon(&l.text).is_none() {
            // A lone scalar or flow collection, such as a document that is only `42`.
            let l = l.clone();
            self.i += 1;
            self.scalar(&l.text, &l, indent)
        } else {
            self.mapping(indent)
        };
        self.depth -= 1;
        v
    }

    fn sequence(&mut self, indent: usize) -> Value {
        let first = self.lines[self.i].range();
        let mut items = Vec::new();
        let mut last = first;
        while let Some(l) = self.lines.get(self.i).cloned() {
            if l.indent < indent || !l.is_item() {
                break;
            }
            if l.indent > indent {
                self.fail(l.range(), "unexpected indentation");
                break;
            }
            last = l.range();
            let rest = l.text[1..].trim_start();
            if rest.is_empty() {
                self.i += 1;
                items.push(match self.lines.get(self.i) {
                    Some(n) if n.indent > indent => {
                        let at = n.indent;
                        self.node(at)
                    }
                    _ => Value {
                        data: Data::Scalar(String::new()),
                        range: l.range(),
                    },
                });
                continue;
            }
            let skip = l.text.len() - rest.len();
            let inner = YLine {
                indent: l.indent + skip,
                text: rest.to_owned(),
                start: l.start + skip,
            };
            if inner.is_item() || (colon(rest).is_some() && !rest.starts_with(['[', '{'])) {
                // `- key: value` or `- - item`: a collection starting on the item's line.
                self.lines[self.i] = inner.clone();
                items.push(self.node(inner.indent));
            } else {
                self.i += 1;
                items.push(self.scalar(rest, &inner, indent));
            }
        }
        Value {
            data: Data::Array(items),
            range: first.cover(last),
        }
    }

    fn mapping(&mut self, indent: usize) -> Value {
        let first = self.lines[self.i].range();
        let mut pairs = Vec::new();
        let mut last = first;
        while let Some(l) = self.lines.get(self.i).cloned() {
            if l.indent < indent || (l.indent == indent && l.is_item()) {
                break;
            }
            if l.indent > indent {
                self.fail(l.range(), "unexpected indentation");
                break;
            }
            let Some(c) = colon(&l.text) else {
                self.fail(l.range(), "expected `key: value`");
                break;
            };
            last = l.range();
            let key = unquote(&l.text[..c]);
            let rest = l.text[c + 1..].trim();
            self.i += 1;
            let value = if rest.is_empty() {
                match self.lines.get(self.i) {
                    Some(n) if n.indent > indent || (n.indent == indent && n.is_item()) => {
                        let at = n.indent;
                        self.node(at)
                    }
                    _ => Value {
                        data: Data::Scalar(String::new()),
                        range: l.range(),
                    },
                }
            } else {
                self.scalar(rest, &l, indent)
            };
            pairs.push((key, value));
        }
        Value {
            data: Data::Object(pairs),
            range: first.cover(last),
        }
    }

    /// A value written on its line: a block scalar's indicator, a flow collection, or text.
    fn scalar(&mut self, s: &str, l: &YLine, indent: usize) -> Value {
        let range = l.range();
        if matches!(s.chars().next(), Some('|' | '>')) && s.len() <= 3 {
            // `|` or `>` and the more indented lines after it.
            let mut parts = Vec::new();
            while let Some(n) = self.lines.get(self.i).filter(|n| n.indent > indent) {
                parts.push(n.text.clone());
                self.i += 1;
            }
            return Value {
                data: Data::Scalar(parts.join(" ")),
                range,
            };
        }
        if s.starts_with(['[', '{']) {
            return self.flow(s, range);
        }
        Value {
            data: Data::Scalar(unquote(s)),
            range,
        }
    }

    /// A flow collection such as `[a, b]` or `{a: 1}`; one with no closing bracket is drawn as
    /// its text, with an error.
    fn flow(&mut self, s: &str, range: TextRange) -> Value {
        let close = if s.starts_with('[') { ']' } else { '}' };
        let inner = s[1..].strip_suffix(close);
        let Some(inner) = inner.filter(|_| self.depth < MAX_DEPTH) else {
            if self.depth >= MAX_DEPTH {
                let (r, m) = too_deep(range.start().into());
                self.fail(r, m);
            } else {
                self.fail(
                    range,
                    format!("the flow collection has no closing `{close}`"),
                );
            }
            return Value {
                data: Data::Scalar(s.to_owned()),
                range,
            };
        };
        self.depth += 1;
        let data = if close == ']' {
            Data::Array(
                flow_items(inner)
                    .into_iter()
                    .map(|p| self.flow_value(p, range))
                    .collect(),
            )
        } else {
            Data::Object(
                flow_items(inner)
                    .into_iter()
                    .map(|p| {
                        let (k, v) = match colon(p) {
                            Some(c) => (&p[..c], p[c + 1..].trim()),
                            None => (p, ""),
                        };
                        (unquote(k), self.flow_value(v, range))
                    })
                    .collect(),
            )
        };
        self.depth -= 1;
        Value { data, range }
    }

    fn flow_value(&mut self, s: &str, range: TextRange) -> Value {
        if s.starts_with(['[', '{']) {
            self.flow(s, range)
        } else {
            Value {
                data: Data::Scalar(unquote(s)),
                range,
            }
        }
    }
}

// ---- Lowering ----

/// A nested value's text in its parent's row: a scalar's text, `{}` or `[]` for an empty
/// collection, or `None` for a collection drawn as its own table.
fn inline(v: &Value) -> Option<String> {
    match &v.data {
        Data::Scalar(s) => Some(s.replace('\n', " ")),
        Data::Object(p) if p.is_empty() => Some("{}".into()),
        Data::Array(a) if a.is_empty() => Some("[]".into()),
        _ => None,
    }
}

/// Adds `v`'s table and its nested tables to `chart`, returning the table's id.
fn table<'a>(chart: &mut Flowchart, v: &'a Value) -> String {
    let id = format!("data:{}", chart.nodes.len());
    let mut node = Node::new(&id, Label::plain(""), Shape::Rect);
    node.mentions.push(v.range);
    chart.nodes.push(node);
    let at = chart.nodes.len() - 1;
    let mut rows = Vec::new();
    let mut nested = Vec::new();
    let mut row = |cells: Vec<String>, child: Option<&'a Value>| {
        if let Some(c) = child {
            nested.push((rows.len(), c));
        }
        rows.push(Row {
            cells,
            ..Row::default()
        });
    };
    match &v.data {
        Data::Object(pairs) => {
            for (k, c) in pairs {
                match inline(c) {
                    Some(t) => row(vec![k.clone(), t], None),
                    None => row(vec![k.clone(), String::new()], Some(c)),
                }
            }
        }
        Data::Array(items) => {
            for c in items {
                match inline(c) {
                    Some(t) => row(vec![t], None),
                    None => row(vec![String::new()], Some(c)),
                }
            }
        }
        Data::Scalar(_) => {}
    }
    if rows.is_empty() {
        rows.push(Row::plain(inline(v).unwrap_or_default()));
    }
    chart.nodes[at].sections = vec![rows];
    for (k, child) in nested {
        let to = table(chart, child);
        let mut edge = Edge::new(&id, to, child.range);
        edge.end = None;
        edge.rows = [Some(k), None];
        chart.edges.push(edge);
    }
    id
}

fn lower(root: &SyntaxNode, yaml: bool) -> (Flowchart, Vec<Diagnostic>) {
    let mut d = Vec::new();
    let body = body(root, &mut d);
    let (value, error) = if yaml {
        Yaml::new(&body).document()
    } else {
        json(&body)
    };
    let mut chart = Flowchart {
        direction: Direction::LeftRight,
        ..Flowchart::default()
    };
    match value {
        Some(v) => {
            table(&mut chart, &v);
        }
        None if error.is_none() => {
            let end = TextSize::try_from(body.trim_end().len()).expect("source fits");
            let what = if yaml { "YAML" } else { "JSON" };
            d.push(Diagnostic::new(
                TextRange::empty(end),
                format!("the document has no {what} data"),
            ));
        }
        None => {}
    }
    if let Some((range, message)) = error {
        d.push(Diagnostic::new(range, message));
    }
    (chart, d)
}

pub(crate) fn lower_json(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    lower(root, false)
}

pub(crate) fn lower_yaml(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    lower(root, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(v: &Value) -> &str {
        match &v.data {
            Data::Scalar(s) => s,
            other => panic!("not a scalar: {other:?}"),
        }
    }

    fn pairs(v: &Value) -> Vec<(&str, &Value)> {
        match &v.data {
            Data::Object(p) => p.iter().map(|(k, v)| (k.as_str(), v)).collect(),
            other => panic!("not an object: {other:?}"),
        }
    }

    fn items(v: &Value) -> &[Value] {
        match &v.data {
            Data::Array(a) => a,
            other => panic!("not an array: {other:?}"),
        }
    }

    #[test]
    fn json_values() {
        let (v, e) = json(r#" {"a": 1, "b": [true, null, "x\"y"], "c": {}, "d": -2.5e3} "#);
        assert_eq!(e, None);
        let v = v.unwrap();
        let p = pairs(&v);
        assert_eq!(p[0].0, "a");
        assert_eq!(scalar(p[0].1), "1");
        let b = items(p[1].1);
        assert_eq!(
            b.iter().map(scalar).collect::<Vec<_>>(),
            ["true", "null", "x\"y"]
        );
        assert!(pairs(p[2].1).is_empty());
        assert_eq!(scalar(p[3].1), "-2.5e3");
        assert_eq!(json("  "), (None, None));
        assert_eq!(scalar(&json("\"\\u0041\"").0.unwrap()), "A");
    }

    #[test]
    fn json_escapes_and_big_numbers() {
        let s = |src: &str| scalar(&json(src).0.unwrap()).to_owned();
        // A surrogate pair is one character; a lone surrogate is a replacement character.
        assert_eq!(s(r#""\ud83d\ude00""#), "\u{1f600}");
        assert_eq!(s(r#""\ud83dx""#), "\u{fffd}x");
        assert_eq!(s(r#""\ud83d\u0041""#), "\u{fffd}A");
        // A short `\u` escape stops at the closing quote.
        assert_eq!(s(r#""\u12""#), "\u{fffd}");
        assert_eq!(s(r#""\u12g""#), "\u{fffd}g");
        assert_eq!(s("1e999999"), "1e999999");
    }

    #[test]
    fn json_errors() {
        let err = |s: &str| json(s).1.unwrap().1;
        assert_eq!(err("{\"a\" 1}"), "expected `:`, found `1`");
        assert_eq!(err("[1 2]"), "expected `,` or `]`, found `2`");
        assert_eq!(err("{a: 1}"), "expected a `\"key\"`, found `a`");
        assert_eq!(err("[1,"), "expected a JSON value");
        assert_eq!(err("\"open"), "the string has no closing `\"`");
        assert_eq!(err("nope"), "expected a JSON value, found `nope`");
        assert_eq!(err("1 2"), "expected the end of the document, found `2`");
        let deep = "[".repeat(MAX_DEPTH + 1);
        assert!(err(&deep).contains("nest more than"));
        // The error's range is the offending character.
        assert_eq!(json("[1 2]").1.unwrap().0, at(3, 1));
        // What was read before the error is kept.
        let (v, _) = json(r#"{"a": [1, 2 3], "b": 4}"#);
        let p = pairs(v.as_ref().unwrap());
        assert_eq!(p.len(), 1);
        assert_eq!(items(p[0].1).len(), 2);
    }

    fn yaml(s: &str) -> Value {
        let (v, e) = Yaml::new(s).document();
        assert_eq!(e, None);
        v.unwrap()
    }

    #[test]
    fn yaml_mappings_and_sequences() {
        let v = yaml(
            "name: sirene # a comment\nlangs:\n  - rust\n  - 'mermaid'\nowner:\n  name: J\n  \
             tags: [a, b]\nlist:\n- x: 1\n  y: 2\n- plain\nempty:\nnote: |\n  one\n  two\n",
        );
        let p = pairs(&v);
        assert_eq!(
            p.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            ["name", "langs", "owner", "list", "empty", "note"]
        );
        assert_eq!(scalar(p[0].1), "sirene");
        assert_eq!(
            items(p[1].1).iter().map(scalar).collect::<Vec<_>>(),
            ["rust", "mermaid"]
        );
        let owner = pairs(p[2].1);
        assert_eq!(scalar(owner[0].1), "J");
        assert_eq!(items(owner[1].1).len(), 2);
        let list = items(p[3].1);
        assert_eq!(pairs(&list[0]).len(), 2);
        assert_eq!(scalar(&list[1]), "plain");
        assert_eq!(scalar(p[4].1), "");
        assert_eq!(scalar(p[5].1), "one two");
    }

    #[test]
    fn yaml_top_level_sequence_and_flow() {
        let v = yaml("- - a\n  - b\n- {k: v, n: [1, 2]}\n- url: http://x\n");
        let top = items(&v);
        assert_eq!(items(&top[0]).len(), 2);
        let flow = pairs(&top[1]);
        assert_eq!(flow[0], ("k", flow[0].1));
        assert_eq!(scalar(flow[0].1), "v");
        assert_eq!(items(flow[1].1).len(), 2);
        assert_eq!(scalar(pairs(&top[2])[0].1), "http://x");
    }

    #[test]
    fn yaml_errors() {
        let err = |s: &str| Yaml::new(s).document().1.unwrap().1;
        assert_eq!(err("a: 1\n    b: 2\n"), "unexpected indentation");
        assert_eq!(err("a: 1\njust text\n"), "expected `key: value`");
        assert_eq!(
            err("a: 1\n- b\n"),
            "expected `key: value` at this indentation"
        );
        assert_eq!(err("- a\nb: 1\n"), "expected `- item` at this indentation");
        assert_eq!(
            err("- a: 1\nb: 2\n"),
            "expected `- item` at this indentation"
        );
        assert_eq!(err("a: [1, 2\n"), "the flow collection has no closing `]`");
        assert_eq!(Yaml::new("# only\n---\n").document(), (None, None));
        // What was read before the error is kept.
        let (v, _) = Yaml::new("a: 1\nb: 2\n    c: 3\n").document();
        assert_eq!(pairs(v.as_ref().unwrap()).len(), 2);
    }

    #[test]
    fn yaml_indented_first_item_reports_item_error() {
        let err = |s: &str| Yaml::new(s).document().1.unwrap().1;
        // The first line is an item at indent 2, so a shallower line expects `- item`.
        assert_eq!(
            err("  - a\nb: 1\n"),
            "expected `- item` at this indentation"
        );
        assert_eq!(err("  - a\n- b\n"), "expected `- item` at this indentation");
        // An indented first mapping key keeps the mapping wording.
        assert_eq!(
            err("  a: 1\n- b\n"),
            "expected `key: value` at this indentation"
        );
        // A well-formed indented sequence is clean.
        let v = yaml("  - a\n  - b\n");
        assert_eq!(items(&v).len(), 2);
    }

    #[test]
    fn mixed_nested_tables_render() {
        let v = json(r#"{"a": [[1, 2], {"k": [3]}, "s"], "b": {"c": [], "d": {"e": [{"f": 1}]}}}"#)
            .0
            .unwrap();
        let mut chart = Flowchart::default();
        table(&mut chart, &v);
        assert!(!chart.nodes.is_empty());
        // Every edge joins two nodes that exist.
        for e in &chart.edges {
            assert!(chart.nodes.iter().any(|n| n.id == e.from), "{}", e.from);
            assert!(chart.nodes.iter().any(|n| n.id == e.to), "{}", e.to);
        }
        // The root table lists both keys.
        let root: Vec<_> = chart.nodes[0].sections[0]
            .iter()
            .map(|r| r.cells[0].clone())
            .collect();
        assert_eq!(root, ["a", "b"]);
        // Rendering the same value again gives the same shape.
        let mut again = Flowchart::default();
        table(&mut again, &v);
        assert_eq!(again.nodes.len(), chart.nodes.len());
        assert_eq!(again.edges.len(), chart.edges.len());
    }

    #[test]
    fn deep_table_with_large_leaf_completes() {
        let depth = 30;
        let leaf = "x".repeat(200_000);
        let src = format!(
            "{}\"{leaf}\"{}",
            "{\"a\":[".repeat(depth / 2),
            "]}".repeat(depth / 2)
        );
        let v = json(&src).0.unwrap();
        let mut chart = Flowchart::default();
        table(&mut chart, &v);
        assert_eq!(chart.nodes.len(), depth);
    }

    #[test]
    fn tables() {
        let v = json(r#"{"a": 1, "b": {"c": [1, {"d": 2}]}, "e": []}"#)
            .0
            .unwrap();
        let mut chart = Flowchart::default();
        table(&mut chart, &v);
        let ids: Vec<_> = chart.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["data:0", "data:1", "data:2", "data:3"]);
        let cells = |n: usize| -> Vec<Vec<String>> {
            chart.nodes[n].sections[0]
                .iter()
                .map(|r| r.cells.clone())
                .collect()
        };
        assert_eq!(cells(0), [vec!["a", "1"], vec!["b", ""], vec!["e", "[]"]]);
        assert_eq!(cells(2), [vec!["1"], vec![""]]);
        assert!(chart.nodes.iter().all(Node::untitled));
        let edges: Vec<_> = chart
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.rows[0], e.end))
            .collect();
        assert_eq!(
            edges,
            [
                ("data:2", "data:3", Some(1), None),
                ("data:1", "data:2", Some(0), None),
                ("data:0", "data:1", Some(1), None),
            ]
        );
    }
}
