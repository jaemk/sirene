//! d3-format number specifiers, for `valueFormat` (MTREE-2): `[[fill]align][sign][symbol][0]
//! [width][,][.precision][~][type]`, in the en-US locale.
//!
//! Ported from d3-format (Copyright 2010-2021 Mike Bostock; ISC). See THIRD-PARTY-NOTICES.

/// A parsed d3-format specifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Format {
    fill: char,
    align: char,
    sign: char,
    symbol: Option<char>,
    zero: bool,
    width: usize,
    comma: bool,
    precision: Option<usize>,
    trim: bool,
    kind: char,
}

impl Default for Format {
    /// `,`: thousands separators and up to 12 significant digits.
    fn default() -> Self {
        Format::parse(",").expect("`,` is a valid format")
    }
}

const PREFIXES: [&str; 17] = [
    "y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y",
];

impl Format {
    /// Parses `spec`, or `None` when it is not a d3-format specifier. Mermaid's `$0,0` reads as
    /// `$,`.
    pub fn parse(spec: &str) -> Option<Format> {
        let spec = if spec == "$0,0" { "$," } else { spec };
        let c: Vec<char> = spec.chars().collect();
        let mut i = 0;
        let (mut fill, mut align) = (' ', '>');
        let is_align = |c: char| "<>=^".contains(c);
        if c.len() >= 2 && is_align(c[1]) {
            (fill, align) = (c[0], c[1]);
            i = 2;
        } else if c.first().is_some_and(|&a| is_align(a)) {
            align = c[0];
            i = 1;
        }
        let mut take = |set: &str| -> Option<char> {
            let ch = *c.get(i)?;
            set.contains(ch).then(|| {
                i += 1;
                ch
            })
        };
        let sign = take("+-( ").unwrap_or('-');
        let symbol = take("$#");
        let mut zero = take("0").is_some();
        let digits = |i: &mut usize| -> Option<usize> {
            let start = *i;
            while c.get(*i).is_some_and(char::is_ascii_digit) {
                *i += 1;
            }
            (*i > start).then(|| c[start..*i].iter().collect::<String>().parse().ok())?
        };
        let width = digits(&mut i).unwrap_or(0);
        let comma = c.get(i) == Some(&',') && {
            i += 1;
            true
        };
        let mut precision = None;
        if c.get(i) == Some(&'.') {
            i += 1;
            precision = Some(digits(&mut i)?);
        }
        let mut trim = c.get(i) == Some(&'~') && {
            i += 1;
            true
        };
        let mut kind = match c.get(i) {
            Some(&k) if k.is_ascii_alphabetic() || k == '%' => {
                i += 1;
                k
            }
            _ => ' ',
        };
        if i != c.len() {
            return None;
        }
        let mut comma = comma;
        if kind == 'n' {
            comma = true;
            kind = 'g';
        } else if !"efgprs%dxXob".contains(kind) {
            // No type, or an unknown one, is `.12~g`.
            precision = precision.or(Some(12));
            trim = true;
            kind = 'g';
        }
        if zero || (fill == '0' && align == '=') {
            zero = true;
            fill = '0';
            align = '=';
        }
        let precision = precision.map(|p| {
            if "gprs".contains(kind) {
                p.clamp(1, 21)
            } else {
                p.min(20)
            }
        });
        Some(Format {
            fill,
            align,
            sign,
            symbol,
            zero,
            width,
            comma,
            precision,
            trim,
            kind,
        })
    }

    /// `v` formatted.
    pub fn apply(&self, v: f64) -> String {
        let p = self.precision.unwrap_or(6);
        let kind = self.kind;
        let mut prefix = match self.symbol {
            Some('$') => "$".to_owned(),
            Some('#') if "boxX".contains(kind) => format!("0{}", kind.to_ascii_lowercase()),
            _ => String::new(),
        };
        let mut suffix = if self.symbol != Some('$') && "%p".contains(kind) {
            "%".to_owned()
        } else {
            String::new()
        };
        let mut negative = v < 0.0 || (v == 0.0 && v.is_sign_negative());
        let a = v.abs();
        let mut si = "";
        let mut value = if v.is_nan() {
            "NaN".to_owned()
        } else {
            match kind {
                '%' => fixed(a * 100.0, p),
                'b' => format!("{:b}", a.round() as u64),
                'o' => format!("{:o}", a.round() as u64),
                'x' => format!("{:x}", a.round() as u64),
                'X' => format!("{:X}", a.round() as u64),
                'd' => fixed(a.round(), 0),
                'e' => exponential(a, p),
                'f' => fixed(a, p),
                'g' => precise(a, p),
                'p' => rounded(a * 100.0, p),
                'r' => rounded(a, p),
                's' => {
                    let (s, prefix) = si_prefix(a, p);
                    si = prefix;
                    s
                }
                _ => precise(a, p),
            }
        };
        if self.trim {
            value = trim_zeros(&value);
        }
        if negative && value.parse::<f64>().is_ok_and(|n| n == 0.0) && self.sign != '+' {
            negative = false;
        }
        let sign = if negative {
            if self.sign == '(' { "(" } else { "-" }
        } else if self.sign == '-' || self.sign == '(' {
            ""
        } else if self.sign == '+' {
            "+"
        } else {
            " "
        };
        prefix.insert_str(0, sign);
        suffix.insert_str(0, si);
        if negative && self.sign == '(' {
            suffix.push(')');
        }
        if "defgprs%".contains(kind)
            && let Some(at) = value.find(|c: char| !c.is_ascii_digit())
        {
            suffix.insert_str(0, &value[at..]);
            value.truncate(at);
        }
        if self.comma && !self.zero {
            value = group(&value, usize::MAX);
        }
        let len = prefix.chars().count() + value.chars().count() + suffix.chars().count();
        let mut padding: String =
            std::iter::repeat_n(self.fill, self.width.saturating_sub(len)).collect();
        if self.comma && self.zero {
            let digits = if padding.is_empty() {
                usize::MAX
            } else {
                self.width.saturating_sub(suffix.chars().count())
            };
            value = group(&format!("{padding}{value}"), digits);
            padding.clear();
        }
        match self.align {
            '<' => format!("{prefix}{value}{suffix}{padding}"),
            '=' => format!("{prefix}{padding}{value}{suffix}"),
            '^' => {
                let n = padding.chars().count();
                let left: String = padding.chars().take(n / 2).collect();
                let right: String = padding.chars().skip(n / 2).collect();
                format!("{left}{prefix}{value}{suffix}{right}")
            }
            _ => format!("{padding}{prefix}{value}{suffix}"),
        }
    }
}

/// `x.toFixed(p)`.
fn fixed(x: f64, p: usize) -> String {
    format!("{x:.p$}")
}

/// `x.toExponential(p)`: `1.50e+3`.
fn exponential(x: f64, p: usize) -> String {
    let s = format!("{x:.p$e}");
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

/// `x`'s first `p` significant digits and its decimal exponent.
fn decimal_parts(x: f64, p: usize) -> (String, i32) {
    let s = format!("{x:.*e}", p.saturating_sub(1));
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    (m.replace('.', ""), e.parse().unwrap_or(0))
}

/// `x.toPrecision(p)`.
fn precise(x: f64, p: usize) -> String {
    let (_, e) = decimal_parts(x, p);
    if e < -6 || e >= p as i32 {
        exponential(x, p - 1)
    } else {
        fixed(x, (p as i32 - 1 - e).max(0) as usize)
    }
}

/// `x` to `p` significant digits, never in exponent form.
fn rounded(x: f64, p: usize) -> String {
    let (c, e) = decimal_parts(x, p);
    if e < 0 {
        format!("0.{}{c}", "0".repeat((-e - 1) as usize))
    } else if c.len() > e as usize + 1 {
        format!("{}.{}", &c[..=e as usize], &c[e as usize + 1..])
    } else {
        format!("{c}{}", "0".repeat(e as usize + 1 - c.len()))
    }
}

/// `x` to `p` significant digits scaled to an SI prefix, and the prefix.
fn si_prefix(x: f64, p: usize) -> (String, &'static str) {
    let (c, e) = decimal_parts(x, p);
    let k = (e.div_euclid(3)).clamp(-8, 8);
    let i = e - k * 3 + 1;
    let n = c.len() as i32;
    let s = if i == n {
        c
    } else if i > n {
        format!("{c}{}", "0".repeat((i - n) as usize))
    } else if i > 0 {
        format!("{}.{}", &c[..i as usize], &c[i as usize..])
    } else {
        let more = (p as i32 + i - 1).max(0) as usize;
        format!(
            "0.{}{}",
            "0".repeat((1 - i) as usize),
            decimal_parts(x, more).0
        )
    };
    (s, PREFIXES[(8 + k) as usize])
}

/// `s` without insignificant trailing zeros: `1.500e+3` -> `1.5e+3`, `2.0` -> `2`.
fn trim_zeros(s: &str) -> String {
    let Some(dot) = s.find('.') else {
        return s.to_owned();
    };
    let end = s[dot..]
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .map_or(s.len(), |i| dot + i);
    let digits = s[dot + 1..end].trim_end_matches('0');
    let head = if digits.is_empty() {
        s[..dot].to_owned()
    } else {
        format!("{}.{digits}", &s[..dot])
    };
    format!("{head}{}", &s[end..])
}

/// `digits` grouped in threes with `,`, keeping at most `width` characters.
fn group(digits: &str, width: usize) -> String {
    let chars: Vec<char> = digits.chars().collect();
    let mut groups: Vec<String> = Vec::new();
    let mut end = chars.len();
    let mut len = 0;
    while end > 0 {
        let start = end.saturating_sub(3);
        let mut g: String = chars[start..end].iter().collect();
        if len + g.len() + 1 > width {
            let keep = width.saturating_sub(len).max(1);
            g = g.chars().skip(g.len().saturating_sub(keep)).collect();
            groups.push(g);
            break;
        }
        len += g.len() + 1;
        groups.push(g);
        end = start;
    }
    groups.reverse();
    groups.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(spec: &str, v: f64) -> String {
        Format::parse(spec)
            .unwrap_or_else(|| panic!("bad spec {spec}"))
            .apply(v)
    }

    #[test]
    fn d3_examples() {
        assert_eq!(f(",", 1234.5), "1,234.5");
        assert_eq!(f(",", 1234567.0), "1,234,567");
        assert_eq!(f(".1f", 4.26), "4.3");
        assert_eq!(f(".1%", 0.123), "12.3%");
        assert_eq!(f("$.2f", 3.5), "$3.50");
        assert_eq!(f("$,.2f", 1234.5), "$1,234.50");
        assert_eq!(f("$0,0", 1234.5), "$1,234.5");
        assert_eq!(f("d", 2.6), "3");
        assert_eq!(f(".2e", 1500.0), "1.50e+3");
        assert_eq!(f(".3s", 1500.0), "1.50k");
        assert_eq!(f("~s", 1500.0), "1.5k");
        assert_eq!(f(".2r", 1234.0), "1200");
        assert_eq!(f(".3g", 0.000012345), "0.0000123");
        assert_eq!(f("+.1f", 2.0), "+2.0");
        assert_eq!(f("(.1f", -2.0), "(2.0)");
        assert_eq!(f("08.2f", -1.5), "-0001.50");
        assert_eq!(f("*^9d", 42.0), "***42****");
        assert_eq!(f("010,d", 1234567.0), "01,234,567");
        assert_eq!(f("#x", 255.0), "0xff");
        assert_eq!(f("", 0.1 + 0.2), "0.3");
    }

    #[test]
    fn bad_specs() {
        assert!(Format::parse("abc").is_none());
        assert!(Format::parse(".f").is_none());
        assert!(Format::parse("$$").is_none());
    }

    #[test]
    fn helpers() {
        assert_eq!(trim_zeros("1.500e+3"), "1.5e+3");
        assert_eq!(trim_zeros("2.000"), "2");
        assert_eq!(trim_zeros("120"), "120");
        assert_eq!(group("1234567", usize::MAX), "1,234,567");
        assert_eq!(rounded(0.0123, 2), "0.012");
    }
}
