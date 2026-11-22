//! A strict JSON reader for use case JSON tables, flattened to key/value rows (MUSE-5).

use crate::lines::{MAX_DEPTH, too_deep};

/// Separate budgets for the value slots read and the rows built from them.
const TABLE_BYTES: usize = 1 << 24;
const MAX_VALUES: usize = TABLE_BYTES / size_of::<Value>();

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    /// A number kept as written.
    Number(String),
    String(String),
    Array(Vec<Value>),
    /// Entries in first-appearance order; a repeated key takes the last value.
    Object(Vec<(String, Value)>),
}

struct Reader<'a> {
    s: &'a str,
    pos: usize,
    /// How many arrays and objects are open around the cursor (LANGS-8).
    depth: usize,
    remaining_values: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        let n = self.s[self.pos..]
            .find(|c: char| !matches!(c, ' ' | '\t' | '\n' | '\r'))
            .unwrap_or(self.s.len() - self.pos);
        self.pos += n;
    }

    fn eat(&mut self, c: char) -> bool {
        self.ws();
        if self.s[self.pos..].starts_with(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!(
            "invalid JSON at byte {}: expected {what}",
            self.pos
        ))
    }

    fn value(&mut self) -> Result<Value, String> {
        self.ws();
        self.remaining_values = self
            .remaining_values
            .checked_sub(1)
            .ok_or_else(|| format!("a JSON table may read at most {MAX_VALUES} values"))?;
        let r = &self.s[self.pos..];
        for (word, v) in [
            ("null", Value::Null),
            ("true", Value::Bool(true)),
            ("false", Value::Bool(false)),
        ] {
            if r.starts_with(word) {
                self.pos += word.len();
                return Ok(v);
            }
        }
        if r.starts_with(['{', '[']) {
            if self.depth >= MAX_DEPTH {
                return Err(format!(
                    "invalid JSON at byte {}: {}",
                    self.pos,
                    too_deep("values", MAX_DEPTH)
                ));
            }
            self.depth += 1;
            let v = if r.starts_with('{') {
                self.object()
            } else {
                self.array()
            };
            self.depth -= 1;
            return v;
        }
        match r.chars().next() {
            Some('"') => self.string().map(Value::String),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => self.err("a value"),
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let r = &self.s[self.pos..];
        let n = r
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')))
            .unwrap_or(r.len());
        let text = &r[..n];
        let valid = {
            let t = text.strip_prefix('-').unwrap_or(text);
            let (mantissa, exp) = match t.find(['e', 'E']) {
                Some(i) => (&t[..i], Some(&t[i + 1..])),
                None => (t, None),
            };
            let (int, frac) = match mantissa.split_once('.') {
                Some((i, f)) => (i, Some(f)),
                None => (mantissa, None),
            };
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
            digits(int)
                && (int == "0" || !int.starts_with('0'))
                && frac.is_none_or(digits)
                && exp.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)))
        };
        if !valid {
            return self.err("a number");
        }
        self.pos += n;
        Ok(Value::Number(text.to_owned()))
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        let mut chars = self.s[self.pos..].char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => {
                    self.pos += i + 1;
                    return Ok(out);
                }
                '\\' => {
                    let Some((_, e)) = chars.next() else { break };
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let Some(code) = hex4(&mut chars) else {
                                return self.err("four hex digits after `\\u`");
                            };
                            // A high surrogate joins a following `\uDC00`-`\uDFFF`.
                            let code = if (0xd800..0xdc00).contains(&code) {
                                let mut ahead = chars.clone();
                                let low = match (ahead.next(), ahead.next()) {
                                    (Some((_, '\\')), Some((_, 'u'))) => {
                                        hex4(&mut ahead).filter(|l| (0xdc00..0xe000).contains(l))
                                    }
                                    _ => None,
                                };
                                match low {
                                    Some(l) => {
                                        chars = ahead;
                                        0x10000 + ((code - 0xd800) << 10) + (l - 0xdc00)
                                    }
                                    None => code,
                                }
                            } else {
                                code
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return self.err("a valid escape"),
                    }
                }
                c if (c as u32) < 0x20 => return self.err("a closing `\"`"),
                c => out.push(c),
            }
        }
        self.pos = self.s.len();
        self.err("a closing `\"`")
    }

    fn array(&mut self) -> Result<Value, String> {
        self.pos += 1;
        let mut items = Vec::new();
        if self.eat(']') {
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            if self.eat(']') {
                return Ok(Value::Array(items));
            }
            if !self.eat(',') {
                return self.err("`,` or `]`");
            }
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.pos += 1;
        let mut entries: Vec<(String, Value)> = Vec::new();
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        if self.eat('}') {
            return Ok(Value::Object(entries));
        }
        loop {
            self.ws();
            if !self.s[self.pos..].starts_with('"') {
                return self.err("a quoted key");
            }
            let key = self.string()?;
            if !self.eat(':') {
                return self.err("`:`");
            }
            let v = self.value()?;
            match index.get(&key) {
                Some(&i) => entries[i].1 = v,
                None => {
                    index.insert(key.clone(), entries.len());
                    entries.push((key, v));
                }
            }
            if self.eat('}') {
                return Ok(Value::Object(entries));
            }
            if !self.eat(',') {
                return self.err("`,` or `}`");
            }
        }
    }
}

/// Four hex digits, as in a `\u` escape.
fn hex4(chars: &mut std::str::CharIndices) -> Option<u32> {
    (0..4).try_fold(0, |n, _| Some(n * 16 + chars.next()?.1.to_digit(16)?))
}

fn scalar(v: &Value) -> Option<&str> {
    match v {
        Value::Null => Some("null"),
        Value::Bool(true) => Some("true"),
        Value::Bool(false) => Some("false"),
        Value::Number(s) | Value::String(s) => Some(s),
        _ => None,
    }
}

#[derive(Default)]
struct Rows {
    rows: Vec<(String, String)>,
    path: String,
    bytes: usize,
}

impl Rows {
    fn check_budget(&self, key_len: usize, value_len: usize) -> Result<usize, String> {
        let cost = size_of::<(String, String)>()
            .saturating_add(key_len)
            .saturating_add(value_len);
        if cost > TABLE_BYTES - self.bytes {
            return Err(format!(
                "a JSON table's rows may hold at most {TABLE_BYTES} bytes"
            ));
        }
        Ok(cost)
    }

    fn push(&mut self, include_path: bool, value: &str) -> Result<(), String> {
        let key = if include_path { &self.path } else { "" };
        self.bytes += self.check_budget(key.len(), value.len())?;
        self.rows.push((key.to_owned(), value.to_owned()));
        Ok(())
    }

    fn descend(&mut self, separator: &str, key: &str, value: &Value) -> Result<(), String> {
        let len = self.path.len();
        // Every subtree emits at least one row with its path. Bound the path before copying.
        self.check_budget(
            len.saturating_add(separator.len())
                .saturating_add(key.len()),
            0,
        )?;
        self.path.push_str(separator);
        self.path.push_str(key);
        let result = self.flatten(value);
        self.path.truncate(len);
        result
    }

    fn flatten(&mut self, v: &Value) -> Result<(), String> {
        match v {
            Value::Object(entries) if entries.is_empty() => self.push(true, "{}"),
            Value::Object(entries) => {
                for (k, v) in entries {
                    let separator = if self.path.is_empty() { "" } else { "." };
                    self.descend(separator, k, v)?;
                }
                Ok(())
            }
            Value::Array(items) if items.is_empty() => self.push(true, "[]"),
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    match scalar(item) {
                        // A list of values puts the key on its first row only.
                        Some(s) => self.push(i == 0, s)?,
                        None => self.descend("", &format!("[{i}]"), item)?,
                    }
                }
                Ok(())
            }
            v => self.push(true, scalar(v).unwrap_or_default()),
        }
    }
}

/// The `{ ... }` object as `(key, value)` rows, or why it is not valid JSON.
pub(crate) fn rows(src: &str) -> Result<Vec<(String, String)>, String> {
    let mut r = Reader {
        s: src,
        pos: 0,
        depth: 0,
        remaining_values: MAX_VALUES,
    };
    let v = r.value()?;
    r.ws();
    if r.pos != src.len() {
        return r.err("the end of the object");
    }
    if !matches!(v, Value::Object(_)) {
        return Err("a JSON table must be an object".into());
    }
    let mut out = Rows::default();
    out.flatten(&v)?;
    Ok(out.rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(src: &str) -> Vec<(String, String)> {
        rows(src).unwrap()
    }

    fn p(k: &str, v: &str) -> (String, String) {
        (k.into(), v.into())
    }

    #[test]
    fn flattens() {
        assert_eq!(
            pairs(
                r#"{ "name": "Ada", "age": 36, "x": null, "ok": true,
                     "a": { "b": 1.5e3 }, "tags": ["x", "y"], "items": [{ "n": "p" }],
                     "e": {}, "l": [], "name": "Bo" }"#
            ),
            [
                p("name", "Bo"),
                p("age", "36"),
                p("x", "null"),
                p("ok", "true"),
                p("a.b", "1.5e3"),
                p("tags", "x"),
                p("", "y"),
                p("items[0].n", "p"),
                p("e", "{}"),
                p("l", "[]"),
            ]
        );
        assert_eq!(pairs(r#"{"s": "a\"b\u0041\n"}"#), [p("s", "a\"bA\n")]);
    }

    #[test]
    fn surrogates() {
        let pair = ["{\"s\": \"\\", "ud83d\\", "ude00\"}"].concat();
        assert_eq!(pairs(&pair), [p("s", "\u{1f600}")]);
        let lone = ["{\"s\": \"\\", "ud83dx\"}"].concat();
        assert_eq!(pairs(&lone), [p("s", "\u{fffd}x")]);
    }

    #[test]
    fn row_bytes_are_bounded() {
        let key = "k".repeat(1 << 16);
        let inner: Vec<String> = (0..300).map(|i| format!("\"{i}\": 1")).collect();
        let src = format!("{{\"{key}\": {{{}}}}}", inner.join(","));
        assert!(rows(&src).unwrap_err().contains("at most"));
    }

    #[test]
    fn empty_cells_still_spend_row_storage() {
        let mut out = Rows {
            bytes: TABLE_BYTES - size_of::<(String, String)>(),
            ..Rows::default()
        };
        assert!(out.push(false, "").is_ok());
        assert_eq!(out.rows, [p("", "")]);
        assert!(out.push(false, "").is_err());
    }

    #[test]
    fn row_budget_is_checked_before_copying_a_path() {
        let mut out = Rows {
            bytes: TABLE_BYTES - size_of::<(String, String)>() - 3,
            ..Rows::default()
        };
        assert!(out.descend("", "café", &Value::Null).is_err());
        assert!(out.path.is_empty());
        assert_eq!(out.path.capacity(), 0);
    }

    #[test]
    fn reading_stops_at_the_value_limit() {
        let mut reader = Reader {
            s: "[null, null, null]",
            pos: 0,
            depth: 0,
            remaining_values: 2,
        };
        assert!(reader.value().unwrap_err().contains("values"));
        assert_eq!(&reader.s[reader.pos..], "null, null]");
    }

    #[test]
    fn rejects() {
        for bad in [
            "{a: 1}",
            "{\"a\": 01}",
            "{\"a\": 1,}",
            "{\"a\": 'x'}",
            "{\"a\": 1} x",
            "[1]",
            "{\"a\": \"\\q\"}",
            "{\"a\": tru}",
            "{\"a\": \"\\u+041\"}",
            "{\"a\": \"\\u00\"}",
        ] {
            assert!(rows(bad).is_err(), "{bad}");
        }
    }
}
