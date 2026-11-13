//! A strict JSON reader for use case JSON tables, flattened to key/value rows (MUSE-5).

use crate::lines::{MAX_DEPTH, too_deep};

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

/// The most key and value bytes the rows of one table may hold, since each row repeats its path.
const MAX_ROW_BYTES: usize = 1 << 24;

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.clone()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

struct Rows {
    rows: Vec<(String, String)>,
    bytes: usize,
}

impl Rows {
    fn push(&mut self, key: String, value: String) -> Result<(), String> {
        self.bytes += key.len() + value.len();
        if self.bytes > MAX_ROW_BYTES {
            return Err(format!(
                "a JSON table's rows may hold at most {MAX_ROW_BYTES} bytes"
            ));
        }
        self.rows.push((key, value));
        Ok(())
    }

    fn flatten(&mut self, prefix: &str, v: &Value) -> Result<(), String> {
        match v {
            Value::Object(entries) if entries.is_empty() => self.push(prefix.into(), "{}".into()),
            Value::Object(entries) => {
                for (k, v) in entries {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    self.flatten(&key, v)?;
                }
                Ok(())
            }
            Value::Array(items) if items.is_empty() => self.push(prefix.into(), "[]".into()),
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    match scalar(item) {
                        // A list of values puts the key on its first row only.
                        Some(s) => {
                            self.push(if i == 0 { prefix.into() } else { String::new() }, s)?
                        }
                        None => self.flatten(&format!("{prefix}[{i}]"), item)?,
                    }
                }
                Ok(())
            }
            v => self.push(prefix.into(), scalar(v).unwrap_or_default()),
        }
    }
}

/// The `{ ... }` object as `(key, value)` rows, or why it is not valid JSON.
pub(crate) fn rows(src: &str) -> Result<Vec<(String, String)>, String> {
    let mut r = Reader {
        s: src,
        pos: 0,
        depth: 0,
    };
    let v = r.value()?;
    r.ws();
    if r.pos != src.len() {
        return r.err("the end of the object");
    }
    if !matches!(v, Value::Object(_)) {
        return Err("a JSON table must be an object".into());
    }
    let mut out = Rows {
        rows: Vec::new(),
        bytes: 0,
    };
    out.flatten("", &v)?;
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
