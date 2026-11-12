//! PlantUML links (`A <|-- B`, `A "1" *-[#red]-> "many" B : label`), shared by the class,
//! state, and description diagrams (PUML-2).

use crate::flowchart::{Arrow, Stroke, Style};

use super::lines;

/// A link line, with its ends in drawing order: an `up` or `left` link is turned around so
/// that `from` ranks first.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Link<'a> {
    pub from: &'a str,
    pub to: &'a str,
    /// The quoted labels beside each end, such as multiplicities.
    pub from_label: Option<String>,
    pub to_label: Option<String>,
    /// The text after `:`.
    pub label: Option<String>,
    pub start: Option<Arrow>,
    pub end: Option<Arrow>,
    pub stroke: Stroke,
    /// The rank span: one less than the number of dashes, and 0 for `left` and `right`.
    pub length: u8,
    /// Cleared by `[norank]`.
    pub constraint: bool,
    pub styles: Vec<Style>,
    /// Heads and bracket options that are not drawn, for warnings.
    pub unknown: Vec<String>,
    /// The arrow as written, from its first head to its last.
    pub arrow: &'a str,
}

const LEFT_HEADS: &[u8] = b"<|*o#x}{+^";
const RIGHT_HEADS: &[u8] = b">|*o#x{}+^";

fn left_head(h: &str) -> Option<Option<Arrow>> {
    Some(match h {
        "" => None,
        "<" | "<<" => Some(Arrow::Point),
        "<|" | "^" => Some(Arrow::Triangle),
        "*" => Some(Arrow::Diamond),
        "o" => Some(Arrow::HollowDiamond),
        "x" => Some(Arrow::Cross),
        "+" => Some(Arrow::Circle),
        "}|" => Some(Arrow::OneMany),
        "}o" => Some(Arrow::ZeroMany),
        "|o" => Some(Arrow::ZeroOne),
        "||" => Some(Arrow::One),
        _ => return None,
    })
}

fn right_head(h: &str) -> Option<Option<Arrow>> {
    Some(match h {
        "" => None,
        ">" | ">>" => Some(Arrow::Point),
        "|>" | "^" => Some(Arrow::Triangle),
        "*" => Some(Arrow::Diamond),
        "o" => Some(Arrow::HollowDiamond),
        "x" => Some(Arrow::Cross),
        "+" => Some(Arrow::Circle),
        "|{" => Some(Arrow::OneMany),
        "o{" => Some(Arrow::ZeroMany),
        "o|" => Some(Arrow::ZeroOne),
        "||" => Some(Arrow::One),
        _ => return None,
    })
}

/// Whether a link's shaft starts at byte `i`.
fn shaft_starts(b: &[u8], i: usize) -> bool {
    let next = b.get(i + 1).copied();
    let prev = i.checked_sub(1).map(|p| b[p]);
    match b[i] {
        b'-' => true,
        b'.' => {
            let head = |p: u8| match p {
                // A head needs a name before it.
                b'o' | b'x' => i >= 2 && gap(Some(b[i - 2])),
                _ => LEFT_HEADS.contains(&p),
            };
            matches!(next, Some(b'.' | b'[' | b'>' | b'|'))
                || prev.is_some_and(|p| p.is_ascii_whitespace() || head(p))
        }
        // A bold `==` or dotted `~~` shaft, doubled so `a = 1` is not a link.
        // `=up=>` puts a direction word between the two.
        c @ (b'=' | b'~') => {
            let word = b[i + 1..]
                .iter()
                .take_while(|b| b.is_ascii_alphabetic())
                .count();
            next.is_some_and(|n| n == c || n == b'[')
                || (word > 0 && b.get(i + 1 + word) == Some(&c))
        }
        _ => false,
    }
}

/// The byte where the shaft starts, outside quotes, brackets, and parentheses, before any `:`.
fn find_shaft(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    // A `:actor:` end.
    if b.first() == Some(&b':') {
        i = 1 + s[1..].find(':')? + 1;
    }
    let mut depth = 0usize;
    let mut quoted = false;
    while i < b.len() {
        let c = b[i];
        if quoted {
            quoted = c != b'"';
        } else if c == b'"' {
            quoted = true;
        } else if matches!(c, b'(' | b'[') {
            depth += 1;
        } else if matches!(c, b')' | b']') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 {
            if c == b':' {
                if b.get(i + 1) == Some(&b':') {
                    i += 2;
                    continue;
                }
                return None;
            }
            if i > 0 && shaft_starts(b, i) {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Whether byte `c` may come right before or after a head `o` or `x`, which otherwise ends or
/// starts a name.
fn gap(c: Option<u8>) -> bool {
    c.is_none_or(|c| c.is_ascii_whitespace() || matches!(c, b'"' | b')' | b']' | b'(' | b'['))
}

/// The byte index of the label's `:` in `s`, outside quotes and brackets, skipping a leading
/// `:actor:`.
fn label_colon(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    if b.first() == Some(&b':') {
        i = 1 + s[1..].find(':')? + 1;
    }
    let (mut depth, mut quoted) = (0usize, false);
    while i < b.len() {
        match b[i] {
            b'"' => quoted = !quoted,
            b'(' | b'[' if !quoted => depth += 1,
            b')' | b']' if !quoted => depth = depth.saturating_sub(1),
            b':' if !quoted && depth == 0 => {
                if b.get(i + 1) == Some(&b':') {
                    i += 2;
                    continue;
                }
                return Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// A trailing `"label"` of `s`, when text is left before it.
fn trailing_quote(s: &str) -> (&str, Option<String>) {
    let s = s.trim_end();
    if let Some(body) = s.strip_suffix('"')
        && let Some(open) = body.rfind('"')
        && !body[..open].trim().is_empty()
    {
        return (
            body[..open].trim_end(),
            Some(lines::text(&body[open + 1..])),
        );
    }
    (s, None)
}

/// A leading `"label"` of `s`, when text is left after it.
fn leading_quote(s: &str) -> (&str, Option<String>) {
    let s = s.trim_start();
    if let Some(body) = s.strip_prefix('"')
        && let Some(close) = body.find('"')
        && !body[close + 1..].trim().is_empty()
    {
        return (
            body[close + 1..].trim_start(),
            Some(lines::text(&body[..close])),
        );
    }
    (s, None)
}

/// Reads a link line, or `None` when `s` has no arrow between two ends.
pub(crate) fn link(s: &str) -> Option<Link<'_>> {
    let b = s.as_bytes();
    let p = find_shaft(s)?;
    // Heads before the shaft, at most two, and an `o` or `x` only after a gap.
    let mut left = p;
    while left > 0 && p - left < 2 && LEFT_HEADS.contains(&b[left - 1]) {
        left -= 1;
    }
    while left < p && matches!(b[left], b'o' | b'x') && (left == 0 || !gap(Some(b[left - 1]))) {
        left += 1;
    }
    // The shaft: dashes or dots, one `[options]`, and direction words between them.
    let mut q = p;
    let (mut dashes, mut dots, mut bold) = (0u8, false, false);
    let mut options = None;
    let mut direction = None;
    while q < b.len() {
        match b[q] {
            b'-' => dashes = dashes.saturating_add(1),
            b'.' | b'~' => {
                dashes = dashes.saturating_add(1);
                dots = true;
            }
            b'=' => {
                dashes = dashes.saturating_add(1);
                bold = true;
            }
            b'[' => {
                let close = q + s[q..].find(']')?;
                options = Some(&s[q + 1..close]);
                q = close;
            }
            c if c.is_ascii_alphabetic() => {
                let w = lines::word(&s[q..]);
                let after = b.get(q + w.len()).copied();
                let dir = w.to_ascii_lowercase();
                let known = [
                    "up", "u", "down", "d", "do", "left", "l", "le", "right", "r", "ri",
                ];
                if !known.contains(&dir.as_str())
                    || !matches!(after, Some(b'-' | b'.' | b'=' | b'~' | b'['))
                {
                    break;
                }
                direction = Some(dir);
                q += w.len();
                continue;
            }
            _ => break,
        }
        q += 1;
    }
    // Heads after the shaft.
    let mut right = q;
    while right < b.len() && right - q < 2 && RIGHT_HEADS.contains(&b[right]) {
        right += 1;
    }
    while right > q
        && matches!(b[right - 1], b'o' | b'x')
        && (right == b.len() || !gap(Some(b[right])))
    {
        right -= 1;
    }
    let mut unknown = Vec::new();
    let head = |h: &str, f: fn(&str) -> Option<Option<Arrow>>, unknown: &mut Vec<String>| {
        f(h).unwrap_or_else(|| {
            unknown.push(format!("head `{h}`"));
            None
        })
    };
    let start = head(&s[left..p], left_head, &mut unknown);
    let end = head(&s[q..right], right_head, &mut unknown);

    let (from, from_label) = trailing_quote(&s[..left]);
    let rest = s[right..].trim_start();
    let (rest, label) = match label_colon(rest) {
        Some(at) => (&rest[..at], Some(lines::text(rest[at + 1..].trim()))),
        None => (rest, None),
    };
    let (to, to_label) = leading_quote(rest);
    let (from, to) = (from.trim(), to.trim());
    if from.is_empty() || to.is_empty() {
        return None;
    }

    let mut link = Link {
        from,
        to,
        from_label,
        to_label,
        label: label.filter(|l| !l.is_empty()),
        start,
        end,
        stroke: if bold {
            Stroke::Thick
        } else if dots {
            Stroke::Dotted
        } else {
            Stroke::Normal
        },
        length: dashes.saturating_sub(1).min(10),
        constraint: true,
        styles: Vec::new(),
        unknown,
        arrow: &s[left..right],
    };
    for option in options.into_iter().flat_map(|o| o.split([',', ';'])) {
        let o = option.trim();
        match o.to_ascii_lowercase().as_str() {
            "" | "plain" => {}
            "dashed" | "dotted" => link.stroke = Stroke::Dotted,
            "bold" => link.stroke = Stroke::Thick,
            "hidden" => link.stroke = Stroke::Invisible,
            "norank" => link.constraint = false,
            t if t.starts_with("thickness=") => {
                if t["thickness=".len()..]
                    .parse::<f32>()
                    .is_ok_and(|w| w >= 2.0)
                {
                    link.stroke = Stroke::Thick;
                }
            }
            _ if o.starts_with('#') => match lines::color(o) {
                Some(c) => link.styles.push(Style {
                    property: "stroke".into(),
                    value: c,
                }),
                None => link.unknown.push(format!("color `{o}`")),
            },
            _ => link.unknown.push(format!("style `{o}`")),
        }
    }
    match direction.as_deref() {
        Some("up" | "u") => link.turn(),
        Some("left" | "l" | "le") => {
            link.turn();
            link.length = 0;
        }
        Some("right" | "r" | "ri") => link.length = 0,
        _ => {}
    }
    Some(link)
}

impl Link<'_> {
    /// Swaps the ends, keeping what is drawn.
    fn turn(&mut self) {
        std::mem::swap(&mut self.from, &mut self.to);
        std::mem::swap(&mut self.from_label, &mut self.to_label);
        std::mem::swap(&mut self.start, &mut self.end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Ends<'a> = (&'a str, &'a str, Option<Arrow>, Option<Arrow>, u8);

    fn ends(s: &str) -> Option<Ends<'_>> {
        link(s).map(|l| (l.from, l.to, l.start, l.end, l.length))
    }

    #[test]
    fn heads_and_lengths() {
        use Arrow::*;
        assert_eq!(ends("A <|-- B"), Some(("A", "B", Some(Triangle), None, 1)));
        assert_eq!(ends("A<|--B"), Some(("A", "B", Some(Triangle), None, 1)));
        assert_eq!(ends("Foo--Bar"), Some(("Foo", "Bar", None, None, 1)));
        assert_eq!(ends("A *--- B"), Some(("A", "B", Some(Diamond), None, 2)));
        assert_eq!(
            ends("A o-- B"),
            Some(("A", "B", Some(HollowDiamond), None, 1))
        );
        assert_eq!(
            ends("A --o B"),
            Some(("A", "B", None, Some(HollowDiamond), 1))
        );
        assert_eq!(ends("A -> B"), Some(("A", "B", None, Some(Point), 0)));
        assert_eq!(ends("A ..|> B"), Some(("A", "B", None, Some(Triangle), 1)));
        assert_eq!(
            ends("A |o--o{ B"),
            Some(("A", "B", Some(ZeroOne), Some(ZeroMany), 1))
        );
        assert_eq!(
            ends("[*] --> S1"),
            Some(("[*]", "S1", None, Some(Point), 1))
        );
        assert_eq!(
            ends("(Use) <.. :Actor:"),
            Some(("(Use)", ":Actor:", Some(Point), None, 1))
        );
        assert_eq!(
            ends("[A-1] --> [B]"),
            Some(("[A-1]", "[B]", None, Some(Point), 1))
        );
        assert_eq!(
            ends("a.b.C ..> D"),
            Some(("a.b.C", "D", None, Some(Point), 1))
        );
        assert_eq!(ends("x-->o"), Some(("x", "o", None, Some(Point), 1)));
        assert_eq!(
            ends("x.y.Z <|-- o.A"),
            Some(("x.y.Z", "o.A", Some(Triangle), None, 1))
        );
        // `up` and `left` turn the link around.
        assert_eq!(ends("A -up-> B"), Some(("B", "A", Some(Point), None, 1)));
        assert_eq!(ends("A -left-> B"), Some(("B", "A", Some(Point), None, 0)));
        assert_eq!(ends("A -r-> B"), Some(("A", "B", None, Some(Point), 0)));
        assert_eq!(ends("A : x-y"), None);
        assert_eq!(ends("A -->"), None);
        assert_eq!(ends("class A"), None);
    }

    #[test]
    fn labels_and_options() {
        let l = link("A \"1\" *-[#red,dashed]-> \"many\" B : has >").unwrap();
        assert_eq!((l.from, l.to), ("A", "B"));
        assert_eq!(l.from_label.as_deref(), Some("1"));
        assert_eq!(l.to_label.as_deref(), Some("many"));
        assert_eq!(l.label.as_deref(), Some("has >"));
        assert_eq!(l.stroke, Stroke::Dotted);
        assert_eq!(l.styles[0].value, "#ff0000");
        assert_eq!(l.arrow, "*-[#red,dashed]->");
        let l = link("\"Long name\" -[hidden,norank,wavy]- B").unwrap();
        assert_eq!(l.from, "\"Long name\"");
        assert_eq!(l.stroke, Stroke::Invisible);
        assert!(!l.constraint);
        assert_eq!(l.unknown, ["style `wavy`"]);
        let l = link("A ::: B").map(|l| l.from);
        assert_eq!(l, None);
        assert_eq!(link("A::x --> B").map(|l| l.from), Some("A::x"));
    }

    #[test]
    fn bold_and_dotted_shafts() {
        let l = link("A ==> B").unwrap();
        assert_eq!(
            (l.from, l.to, l.stroke, l.length),
            ("A", "B", Stroke::Thick, 1)
        );
        let l = link("A ~~> B").unwrap();
        assert_eq!((l.from, l.to, l.stroke), ("A", "B", Stroke::Dotted));
        let l = link("A =up=> B").unwrap();
        assert_eq!((l.from, l.to, l.stroke), ("B", "A", Stroke::Thick));
        let l = link("A =[#red]=> B").unwrap();
        assert_eq!(l.stroke, Stroke::Thick);
        assert_eq!(link("A <|== B").map(|l| l.stroke), Some(Stroke::Thick));
        // A single `=` is not a shaft.
        assert_eq!(link("a = 1").map(|l| l.from), None);
        assert_eq!(link("== title ==").map(|l| l.from), None);
    }
}
