//! Pure scanning helpers. Each takes the remaining source and returns a byte length.

/// Characters allowed anywhere in a node id (FLOWCH-7), besides the context-dependent `-`
/// and `=`.
fn is_id_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '_' | '.' | '!' | '$' | '\'' | '*' | '+' | '?' | '/' | '\\' | '`'
        )
}

/// Length of the node id at the start of `s`, or 0.
pub(crate) fn id_len(s: &str) -> usize {
    let mut chars = s.char_indices().peekable();
    let mut end = 0;
    while let Some((i, c)) = chars.next() {
        let next = chars.peek().map(|&(_, n)| n);
        let ok = match c {
            '-' => next.is_some_and(|n| !matches!(n, '-' | '.' | '>')),
            '=' => next != Some('='),
            _ => is_id_char(c),
        };
        if !ok {
            break;
        }
        end = i + c.len_utf8();
    }
    end
}

/// Whether `s` starts with keyword `kw` as a whole word.
pub(crate) fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && id_len(s) == kw.len()
}

/// Length of inline whitespace (not newlines) at the start of `s`.
pub(crate) fn ws_len(s: &str) -> usize {
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let newline = c == '\n' || (c == '\r' && chars.peek().is_some_and(|&(_, n)| n == '\n'));
        if newline || !c.is_whitespace() {
            return i;
        }
    }
    s.len()
}

/// Length of the newline at the start of `s` (`\n` or `\r\n`), or 0.
pub(crate) fn newline_len(s: &str) -> usize {
    if s.starts_with("\r\n") {
        2
    } else if s.starts_with('\n') {
        1
    } else {
        0
    }
}

/// Length up to (not including) the next newline.
pub(crate) fn line_len(s: &str) -> usize {
    match s.find('\n') {
        Some(i) if i > 0 && s.as_bytes()[i - 1] == b'\r' => i - 1,
        Some(i) => i,
        None => s.len(),
    }
}

/// Whether `s` starts at a statement separator or end of input.
pub(crate) fn is_stmt_end(s: &str) -> bool {
    s.is_empty() || s.starts_with(';') || newline_len(s) > 0
}

/// Length up to (not including) the next statement separator.
pub(crate) fn stmt_len(s: &str) -> usize {
    let line = line_len(s);
    s[..line].find(';').unwrap_or(line)
}

/// `len` with trailing inline whitespace of `s[..len]` removed.
pub(crate) fn trim_end_len(s: &str, len: usize) -> usize {
    s[..len].trim_end().len()
}

/// Length of a direction keyword valid in a header (FLOWCH-4), or 0.
pub(crate) fn header_dir_len(s: &str) -> usize {
    if dir_len(s) > 0 {
        2
    } else if s.starts_with(['>', '<', '^']) || keyword(s, "v") {
        1
    } else {
        0
    }
}

/// Length of a `direction` statement value (FLOWCH-11), or 0.
pub(crate) fn dir_len(s: &str) -> usize {
    let found = ["TB", "TD", "BT", "RL", "LR"].iter().any(|d| keyword(s, d));
    if found { 2 } else { 0 }
}

/// Length of the decimal number at the start of `s`, or 0.
pub(crate) fn number_len(s: &str) -> usize {
    s.bytes().take_while(u8::is_ascii_digit).count()
}

/// Length of a `click` link target (`_self`, `_blank`, `_parent`, `_top`), or 0.
pub(crate) fn link_target_len(s: &str) -> usize {
    ["_self", "_blank", "_parent", "_top"]
        .iter()
        .find(|t| keyword(s, t))
        .map_or(0, |t| t.len())
}

/// Length of a `"..."` or `` "`...`" `` string at the start of `s`, if closed.
pub(crate) fn string_len(s: &str) -> Option<usize> {
    if let Some(body) = s.strip_prefix("\"`") {
        body.find("`\"").map(|i| i + 4)
    } else {
        debug_assert!(s.starts_with('"'));
        s[1..].find('"').map(|i| i + 2)
    }
}

/// Length of a metadata body after `@{`, up to (not including) the closing `}`, if closed.
/// Braces inside double-quoted strings are ignored.
pub(crate) fn meta_body_len(s: &str) -> Option<usize> {
    let mut in_string = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_string = !in_string,
            '}' if !in_string => return Some(i),
            _ => {}
        }
    }
    None
}

/// Shape opening delimiters (FLOWCH-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opener {
    DoubleCircle,
    Circle,
    Stadium,
    Ellipse,
    Round,
    Subroutine,
    Cylinder,
    Slash,
    Backslash,
    Rect,
    Hexagon,
    Diamond,
    Odd,
}

/// Openers ordered so that longer delimiters win.
const OPENERS: &[(&str, Opener)] = &[
    ("(((", Opener::DoubleCircle),
    ("((", Opener::Circle),
    ("([", Opener::Stadium),
    ("(-", Opener::Ellipse),
    ("(", Opener::Round),
    ("[[", Opener::Subroutine),
    ("[(", Opener::Cylinder),
    ("[/", Opener::Slash),
    ("[\\", Opener::Backslash),
    ("[", Opener::Rect),
    ("{{", Opener::Hexagon),
    ("{", Opener::Diamond),
    (">", Opener::Odd),
];

/// The shape opener at the start of `s`, with its length.
pub(crate) fn shape_open(s: &str) -> Option<(usize, Opener)> {
    OPENERS
        .iter()
        .find(|(text, _)| s.starts_with(text))
        .map(|&(text, opener)| (text.len(), opener))
}

/// Valid closing delimiters for `opener`; the first is the canonical one.
pub(crate) fn closers(opener: Opener) -> &'static [&'static str] {
    match opener {
        Opener::DoubleCircle => &[")))"],
        Opener::Circle => &["))"],
        Opener::Stadium => &["])"],
        Opener::Ellipse => &["-)"],
        Opener::Round => &[")"],
        Opener::Subroutine => &["]]"],
        Opener::Cylinder => &[")]"],
        Opener::Slash => &["/]", "\\]"],
        Opener::Backslash => &["\\]", "/]"],
        Opener::Rect | Opener::Odd => &["]"],
        Opener::Hexagon => &["}}"],
        Opener::Diamond => &["}"],
    }
}

/// The closer for `opener` at the start of `s`, with its length.
pub(crate) fn shape_close(s: &str, opener: Opener) -> Option<usize> {
    closers(opener)
        .iter()
        .find(|c| s.starts_with(**c))
        .map(|c| c.len())
}

/// Length of unquoted label text (FLOWCH-8) inside a shape opened by `opener`.
pub(crate) fn shape_text_len(s: &str, opener: Opener) -> usize {
    let extra: &[&str] = match opener {
        Opener::Ellipse => &["-)"],
        Opener::Slash | Opener::Backslash => &["/]", "\\]"],
        _ => &[],
    };
    text_len(s, extra)
}

/// Length of unquoted label text: stops at `[](){}|"` or any of `extra_stops`.
pub(crate) fn text_len(s: &str, extra_stops: &[&str]) -> usize {
    for (i, c) in s.char_indices() {
        if matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | '|' | '"')
            || extra_stops.iter().any(|stop| s[i..].starts_with(stop))
        {
            return i;
        }
    }
    s.len()
}

/// Stroke family of an inline-label edge (FLOWCH-9). Invisible edges have no inline form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrokeKind {
    Normal,
    Thick,
    Dotted,
}

/// An edge stroke at the start of the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stroke {
    /// A complete stroke such as `-->`, with its length.
    Full(usize),
    /// The opening of an inline-label edge such as `-- `, with its length.
    Open(usize, StrokeKind),
}

fn run(b: &[u8], i: usize, c: u8) -> usize {
    b.get(i..)
        .map_or(0, |t| t.iter().take_while(|&&x| x == c).count())
}

/// A complete `--+[-xo>]` (or `==+[=xo>]`) stroke at `i`, without a start marker.
fn line_stroke(b: &[u8], i: usize, c: u8) -> Option<usize> {
    let n = run(b, i, c);
    if n < 2 {
        return None;
    }
    match b.get(i + n) {
        Some(b'x' | b'o' | b'>') => Some(n + 1),
        _ if n >= 3 => Some(n),
        _ => None,
    }
}

/// A complete `-?\.+-[xo>]?` stroke at `i`, without a start marker.
fn dotted_stroke(b: &[u8], i: usize) -> Option<usize> {
    let mut k = i;
    if b.get(k) == Some(&b'-') {
        k += 1;
    }
    let dots = run(b, k, b'.');
    if dots == 0 {
        return None;
    }
    k += dots;
    if b.get(k) != Some(&b'-') {
        return None;
    }
    k += 1;
    if matches!(b.get(k), Some(b'x' | b'o' | b'>')) {
        k += 1;
    }
    Some(k - i)
}

/// The edge stroke at the start of `s`, including an optional start marker.
pub(crate) fn stroke(s: &str) -> Option<Stroke> {
    let b = s.as_bytes();
    if b.starts_with(b"~~~") {
        return Some(Stroke::Full(run(b, 0, b'~')));
    }
    let i = usize::from(matches!(b.first(), Some(b'<' | b'x' | b'o')));
    if let Some(n) = line_stroke(b, i, b'-')
        .or_else(|| line_stroke(b, i, b'='))
        .or_else(|| dotted_stroke(b, i))
    {
        return Some(Stroke::Full(i + n));
    }
    let rest = &b[i..];
    let kind = if rest.starts_with(b"--") {
        StrokeKind::Normal
    } else if rest.starts_with(b"==") {
        StrokeKind::Thick
    } else if rest.starts_with(b"-.") {
        StrokeKind::Dotted
    } else {
        return None;
    };
    Some(Stroke::Open(i + 2, kind))
}

/// A closing stroke for an inline label of `kind` at the start of `s`, with its length.
pub(crate) fn closing_stroke(s: &str, kind: StrokeKind) -> Option<usize> {
    let b = s.as_bytes();
    match kind {
        StrokeKind::Normal => line_stroke(b, 0, b'-'),
        StrokeKind::Thick => line_stroke(b, 0, b'='),
        StrokeKind::Dotted => dotted_stroke(b, 0),
    }
}

/// Length of inline edge label text of `kind`: up to where a closing stroke may begin, or the
/// end of the line.
pub(crate) fn inline_edge_text_len(s: &str, kind: StrokeKind) -> usize {
    let line = &s[..line_len(s)];
    for (i, _) in line.char_indices() {
        let r = &line[i..];
        let stop = match kind {
            StrokeKind::Normal => r.starts_with("--"),
            StrokeKind::Thick => r.starts_with("=="),
            StrokeKind::Dotted => closing_stroke(r, kind).is_some(),
        };
        if stop {
            return i;
        }
    }
    line.len()
}

/// Length of an edge id at the start of `s` (`id` in `id@-->`), if one is there.
pub(crate) fn edge_id_len(s: &str) -> Option<usize> {
    let n = id_len(s);
    let after = s[n..].strip_prefix('@')?;
    (n > 0 && stroke(after).is_some()).then_some(n)
}

/// Whether an edge (optionally with an id) starts at `s`.
pub(crate) fn edge_start(s: &str) -> bool {
    stroke(s).is_some() || edge_id_len(s).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(id_len("abc def"), 3);
        assert_eq!(id_len("A-->B"), 1);
        assert_eq!(id_len("A-.->B"), 1);
        assert_eq!(id_len("A==>B"), 1);
        assert_eq!(id_len("a-b"), 3);
        assert_eq!(id_len("a="), 2);
        assert_eq!(id_len("a-"), 1);
        assert_eq!(id_len("x.y/z_1"), 7);
        assert_eq!(id_len("ñandú["), 7);
        assert_eq!(id_len("A:::c"), 1);
        assert_eq!(id_len("A@{"), 1);
        assert_eq!(id_len("-->"), 0);
        assert_eq!(id_len(""), 0);
    }

    #[test]
    fn keywords() {
        assert!(keyword("end", "end"));
        assert!(keyword("end\n", "end"));
        assert!(!keyword("endpoint", "end"));
        assert!(keyword("end-->", "end"));
        assert!(!keyword("classDef", "class"));
        assert!(keyword("flowchart-elk LR", "flowchart-elk"));
        assert!(!keyword("flowchart-elk", "flowchart"));
    }

    #[test]
    fn whitespace_and_lines() {
        assert_eq!(ws_len("  \tA"), 3);
        assert_eq!(ws_len(" \nA"), 1);
        assert_eq!(ws_len(" \r\nA"), 1);
        assert_eq!(ws_len(" \rA"), 2);
        assert_eq!(ws_len("   "), 3);
        assert_eq!(newline_len("\r\nx"), 2);
        assert_eq!(newline_len("\nx"), 1);
        assert_eq!(newline_len("x"), 0);
        assert_eq!(line_len("ab\ncd"), 2);
        assert_eq!(line_len("ab\r\ncd"), 2);
        assert_eq!(line_len("\nab"), 0);
        assert_eq!(line_len("ab"), 2);
        assert!(is_stmt_end(""));
        assert!(is_stmt_end(";x"));
        assert!(is_stmt_end("\r\n"));
        assert!(!is_stmt_end(" ;"));
        assert_eq!(stmt_len("a b;c"), 3);
        assert_eq!(stmt_len("a b\nc;"), 3);
        assert_eq!(trim_end_len("ab  \t;", 5), 2);
    }

    #[test]
    fn directions() {
        assert_eq!(header_dir_len("LR"), 2);
        assert_eq!(header_dir_len("TD;"), 2);
        assert_eq!(header_dir_len(">"), 1);
        assert_eq!(header_dir_len("<"), 1);
        assert_eq!(header_dir_len("^"), 1);
        assert_eq!(header_dir_len("v"), 1);
        assert_eq!(header_dir_len("vx"), 0);
        assert_eq!(header_dir_len("LRX"), 0);
        assert_eq!(dir_len(">"), 0);
        assert_eq!(dir_len("BT"), 2);
    }

    #[test]
    fn small_tokens() {
        assert_eq!(number_len("12,3"), 2);
        assert_eq!(number_len("x"), 0);
        assert_eq!(link_target_len("_blank"), 6);
        assert_eq!(link_target_len("_blanket"), 0);
        assert_eq!(link_target_len("_top x"), 4);
    }

    #[test]
    fn strings() {
        assert_eq!(string_len("\"abc\" x"), Some(5));
        assert_eq!(string_len("\"\""), Some(2));
        assert_eq!(string_len("\"abc"), None);
        assert_eq!(string_len("\"`a *b*`\" x"), Some(9));
        assert_eq!(string_len("\"`abc\""), None);
    }

    #[test]
    fn metadata_bodies() {
        assert_eq!(meta_body_len(" shape: rect }"), Some(13));
        assert_eq!(meta_body_len("label: \"a}b\" }x"), Some(13));
        assert_eq!(meta_body_len("label: x"), None);
    }

    #[test]
    fn shapes() {
        assert_eq!(shape_open("(((a)))"), Some((3, Opener::DoubleCircle)));
        assert_eq!(shape_open("((a))"), Some((2, Opener::Circle)));
        assert_eq!(shape_open("([a])"), Some((2, Opener::Stadium)));
        assert_eq!(shape_open("(-a-)"), Some((2, Opener::Ellipse)));
        assert_eq!(shape_open("(a)"), Some((1, Opener::Round)));
        assert_eq!(shape_open("[[a]]"), Some((2, Opener::Subroutine)));
        assert_eq!(shape_open("[(a)]"), Some((2, Opener::Cylinder)));
        assert_eq!(shape_open("[/a/]"), Some((2, Opener::Slash)));
        assert_eq!(shape_open("[\\a\\]"), Some((2, Opener::Backslash)));
        assert_eq!(shape_open("[a]"), Some((1, Opener::Rect)));
        assert_eq!(shape_open("{{a}}"), Some((2, Opener::Hexagon)));
        assert_eq!(shape_open("{a}"), Some((1, Opener::Diamond)));
        assert_eq!(shape_open(">a]"), Some((1, Opener::Odd)));
        assert_eq!(shape_open("a"), None);
        assert_eq!(shape_close("/]", Opener::Backslash), Some(2));
        assert_eq!(shape_close(")", Opener::Circle), None);
    }

    #[test]
    fn label_text() {
        assert_eq!(text_len("abc]", &[]), 3);
        assert_eq!(text_len("a|b", &[]), 1);
        assert_eq!(text_len("a\"b", &[]), 1);
        assert_eq!(text_len("a\nb", &[]), 3);
        assert_eq!(shape_text_len("a-b-)", Opener::Ellipse), 3);
        assert_eq!(shape_text_len("a/b/]", Opener::Slash), 3);
        assert_eq!(shape_text_len("a/b\\]", Opener::Backslash), 3);
        assert_eq!(shape_text_len("a/b]", Opener::Rect), 3);
    }

    #[test]
    fn full_strokes() {
        use Stroke::Full;
        assert_eq!(stroke("-->B"), Some(Full(3)));
        assert_eq!(stroke("---B"), Some(Full(3)));
        assert_eq!(stroke("---->B"), Some(Full(5)));
        assert_eq!(stroke("--xB"), Some(Full(3)));
        assert_eq!(stroke("--o B"), Some(Full(3)));
        assert_eq!(stroke("<-->B"), Some(Full(4)));
        assert_eq!(stroke("x--x B"), Some(Full(4)));
        assert_eq!(stroke("o--o B"), Some(Full(4)));
        assert_eq!(stroke("==>B"), Some(Full(3)));
        assert_eq!(stroke("===B"), Some(Full(3)));
        assert_eq!(stroke("<==>B"), Some(Full(4)));
        assert_eq!(stroke("-.->B"), Some(Full(4)));
        assert_eq!(stroke("-.-B"), Some(Full(3)));
        assert_eq!(stroke("-..->B"), Some(Full(5)));
        assert_eq!(stroke(".->B"), Some(Full(3)));
        assert_eq!(stroke("<-.->B"), Some(Full(5)));
        assert_eq!(stroke("~~~B"), Some(Full(3)));
        assert_eq!(stroke("~~~~~B"), Some(Full(5)));
    }

    #[test]
    fn open_strokes() {
        use Stroke::Open;
        assert_eq!(stroke("-- text -->"), Some(Open(2, StrokeKind::Normal)));
        assert_eq!(stroke("<-- text -->"), Some(Open(3, StrokeKind::Normal)));
        assert_eq!(stroke("== text ==>"), Some(Open(2, StrokeKind::Thick)));
        assert_eq!(stroke("-. text .->"), Some(Open(2, StrokeKind::Dotted)));
    }

    #[test]
    fn not_strokes() {
        assert_eq!(stroke("-"), None);
        assert_eq!(stroke("~~"), None);
        assert_eq!(stroke("xyz"), None);
        assert_eq!(stroke("<"), None);
        assert_eq!(stroke("=x"), None);
        assert_eq!(stroke("-.x"), Some(Stroke::Open(2, StrokeKind::Dotted)));
        assert_eq!(stroke(""), None);
    }

    #[test]
    fn closing_strokes() {
        assert_eq!(closing_stroke("-->B", StrokeKind::Normal), Some(3));
        assert_eq!(closing_stroke("---B", StrokeKind::Normal), Some(3));
        assert_eq!(closing_stroke("-- B", StrokeKind::Normal), None);
        assert_eq!(closing_stroke("==>B", StrokeKind::Thick), Some(3));
        assert_eq!(closing_stroke(".->B", StrokeKind::Dotted), Some(3));
        assert_eq!(closing_stroke("..-B", StrokeKind::Dotted), Some(3));
        assert_eq!(closing_stroke("-.->B", StrokeKind::Dotted), Some(4));
        assert_eq!(closing_stroke("~~~", StrokeKind::Normal), None);
    }

    #[test]
    fn inline_edge_text() {
        assert_eq!(inline_edge_text_len(" text -->B", StrokeKind::Normal), 6);
        assert_eq!(inline_edge_text_len(" a-b -->B", StrokeKind::Normal), 5);
        assert_eq!(inline_edge_text_len(" text ==>B", StrokeKind::Thick), 6);
        assert_eq!(inline_edge_text_len(" t.x .->B", StrokeKind::Dotted), 5);
        assert_eq!(inline_edge_text_len(" no close\nB", StrokeKind::Normal), 9);
        assert_eq!(inline_edge_text_len(" x", StrokeKind::Thick), 2);
    }

    #[test]
    fn edge_ids() {
        assert_eq!(edge_id_len("e1@-->B"), Some(2));
        assert_eq!(edge_id_len("e1@{ x }"), None);
        assert_eq!(edge_id_len("e1@B"), None);
        assert_eq!(edge_id_len("e1"), None);
        assert_eq!(edge_id_len("@-->"), None);
        assert!(edge_start("-->"));
        assert!(edge_start("e1@-->"));
        assert!(!edge_start("B"));
    }
}
