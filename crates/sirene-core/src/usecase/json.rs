//! A strict JSON reader for use case JSON tables, flattened to key/value rows (MUSE-5).

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
        match r.chars().next() {
            Some('{') => self.object(),
            Some('[') => self.array(),
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
                            let hex: String =
                                (0..4).filter_map(|_| chars.next()).map(|x| x.1).collect();
                            let Some(code) = u32::from_str_radix(&hex, 16)
                                .ok()
                                .filter(|_| hex.len() == 4)
                            else {
                                return self.err("four hex digits after `\\u`");
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
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 = v,
                None => entries.push((key, v)),
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

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.clone()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn flatten(prefix: &str, v: &Value, rows: &mut Vec<(String, String)>) {
    match v {
        Value::Object(entries) if entries.is_empty() => rows.push((prefix.into(), "{}".into())),
        Value::Object(entries) => {
            for (k, v) in entries {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, v, rows);
            }
        }
        Value::Array(items) if items.is_empty() => rows.push((prefix.into(), "[]".into())),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                match scalar(item) {
                    // A list of values puts the key on its first row only.
                    Some(s) => rows.push((if i == 0 { prefix.into() } else { String::new() }, s)),
                    None => flatten(&format!("{prefix}[{i}]"), item, rows),
                }
            }
        }
        v => rows.push((prefix.into(), scalar(v).unwrap_or_default())),
    }
}

/// The `{ ... }` object as `(key, value)` rows, or why it is not valid JSON.
pub(crate) fn rows(src: &str) -> Result<Vec<(String, String)>, String> {
    let mut r = Reader { s: src, pos: 0 };
    let v = r.value()?;
    r.ws();
    if r.pos != src.len() {
        return r.err("the end of the object");
    }
    if !matches!(v, Value::Object(_)) {
        return Err("a JSON table must be an object".into());
    }
    let mut out = Vec::new();
    flatten("", &v, &mut out);
    Ok(out)
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
        ] {
            assert!(rows(bad).is_err(), "{bad}");
        }
    }
}
