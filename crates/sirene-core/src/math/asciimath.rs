//! AsciiMath to TeX (PCREO-2): the front end of `@startmath` and `<math>`.
//!
//! PlantUML reads AsciiMath by translating it to TeX with ASCIIMathTeXImg.js, a port of Peter
//! Jipsen's ASCIIMathML, and typesetting that. This module is that translation, so a formula
//! reads as it does in PlantUML, and the TeX it makes goes through sirene's own typesetter. It
//! follows the grammar of asciimath.org:
//!
//! ```text
//! v ::= [A-Za-z] | greek letters | numbers | other constant symbols
//! u ::= sqrt | text | bb | other unary symbols for font commands
//! b ::= frac | root | stackrel | other binary symbols
//! l ::= ( | [ | { | (: | {: | other left brackets
//! r ::= ) | ] | } | :) | :} | other right brackets
//! S ::= v | lEr | uS | bSS             Simple expression
//! I ::= S_S | S^S | S_S^S | S          Intermediate expression
//! E ::= IE | I/I                       Expression
//! ```
//!
//! The functions follow the JavaScript's `AMTparseSexpr`, `AMTparseIexpr`, and `AMTparseExpr`,
//! including its quirks, so the TeX is the same text for the same input. They work on pieces of
//! TeX that remember the source they came from, not on strings, which is how an error in the TeX
//! is reported on the AsciiMath that made it ([`Translation::source_span`]), and how the
//! matrices (`[[a,b],[c,d]]`) are found, where the JavaScript searches the text it has written.
//!
//! Where sirene differs from the JavaScript:
//!
//! - `text(...)` and `"..."` escape the characters that are TeX's (`\ { } % $ # & _ ^ ~`), so
//!   a literal reads as one.
//! - `mspace(...)` is `\mkern`, which KaTeX has, not `\mspace`, which it does not.
//! - The HTML entities `&lt;`, `&gt;`, and `&nbsp;` are not read.
//! - A row of a matrix that is not a bracketed group (`(a),x,(b)`) makes no matrix, where the
//!   JavaScript makes garbage.
//! - The translation never throws: where the JavaScript would, an empty group is read.
//! - A formula is bounded in size, nesting, and work (`MAX_FRAMES`, `steps`).
//!
//! Adapted from ASCIIMathTeXImg.js (asciimath-based/ in <https://github.com/asciimath/asciimathml>),
//! Copyright (c) Peter Jipsen and David Lippman, MIT licensed; see THIRD-PARTY-NOTICES.

mod table;
#[cfg(test)]
mod tests;

use std::ops::Range;

use super::error::MathError;
use super::limits;
use table::{Id, SYMS, Sym, Ty};

/// Parser frames (each `S`, `I`, and `E` being read) open at once.
const MAX_FRAMES: usize = limits::MAX_DEPTH * 4;

/// The work, in parser calls, a source of `n` bytes may take: the translator reads a binary
/// symbol's arguments again when its second one is missing, so nested ones could take time
/// exponential in their depth without this.
fn max_steps(n: usize) -> usize {
    16 * n + 4096
}

/// The longest input word of the symbol table (`twoheadrightarrowtail`), with room.
const LOOKAHEAD: usize = 32;

type Res<T> = Result<T, MathError>;

/// The inputs a lookup tries, by their first byte: the table's symbols, then the TeX names that
/// are inputs too (`Sym::alias`), each in table order. A symbol can only match text that starts
/// with its first byte, so a lookup reads one bucket, not the whole table twice.
fn candidates(first: u8) -> &'static [Sym] {
    static BY_FIRST: std::sync::OnceLock<Vec<Vec<Sym>>> = std::sync::OnceLock::new();
    BY_FIRST
        .get_or_init(|| {
            let mut by_first = vec![Vec::new(); 256];
            for s in SYMS
                .iter()
                .copied()
                .chain(SYMS.iter().filter_map(Sym::alias))
            {
                if let Some(bucket) = s
                    .input
                    .as_bytes()
                    .first()
                    .and_then(|b| by_first.get_mut(usize::from(*b)))
                {
                    bucket.push(s);
                }
            }
            by_first
        })
        .get(usize::from(first))
        .map_or(&[], Vec::as_slice)
}

/// The longest symbol `look` starts with and its length in bytes. On a tie the first wins, so
/// a TeX name never wins over a symbol.
fn longest_symbol(look: &str) -> (Option<Sym>, usize) {
    let mut best: Option<Sym> = None;
    let mut best_len = 0;
    let bucket = look.as_bytes().first().map_or(&[][..], |b| candidates(*b));
    for s in bucket {
        if s.input.len() > best_len && look.starts_with(s.input) {
            best = Some(*s);
            best_len = s.input.len();
        }
    }
    (best, best_len)
}

/// AsciiMath `source` as a TeX formula. Fails when the source is too long or too complex; any
/// other text translates (an unknown word is a run of italic letters, as in AsciiMath).
pub fn to_tex(source: &str) -> Res<String> {
    Ok(translate(source)?.tex)
}

/// The TeX of an AsciiMath formula and where in the source each part of it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation {
    /// The TeX.
    pub tex: String,
    /// The length of the AsciiMath source in bytes.
    len: usize,
    /// For each run of the TeX, its byte range and the range of the AsciiMath it came from.
    map: Vec<(Range<usize>, Option<Range<usize>>)>,
}

impl Translation {
    /// The byte range of the AsciiMath source that made the TeX in `tex`: the source of every
    /// run of TeX that `tex` touches, or, for TeX that came from no source (the braces around a
    /// group), of the run nearest before it, then nearest after, then the whole source.
    pub fn source_span(&self, tex: Range<usize>) -> Range<usize> {
        let end = tex.end.max(tex.start.saturating_add(1));
        let touched = |(r, _): &&(Range<usize>, Option<Range<usize>>)| {
            r.start < end && r.end > tex.start && r.start != r.end
        };
        let mut found: Option<Range<usize>> = None;
        for (_, src) in self.map.iter().filter(touched) {
            if let Some(s) = src {
                found = Some(match found {
                    None => s.clone(),
                    Some(f) => f.start.min(s.start)..f.end.max(s.end),
                });
            }
        }
        if let Some(f) = found {
            return f;
        }
        let before = self
            .map
            .iter()
            .rev()
            .find(|(r, s)| r.end <= tex.start && s.is_some());
        let after = self
            .map
            .iter()
            .find(|(r, s)| r.start >= tex.end && s.is_some());
        before
            .or(after)
            .and_then(|(_, s)| s.clone())
            .unwrap_or(0..self.len)
    }
}

/// Translates AsciiMath `source` to TeX, keeping the map from the TeX back to the source.
pub fn translate(source: &str) -> Res<Translation> {
    limits::check_source(source)?;
    if source.chars().all(char::is_whitespace) {
        return Ok(Translation {
            tex: String::new(),
            len: source.len(),
            map: Vec::new(),
        });
    }
    let chars: Vec<(char, usize)> = source.char_indices().map(|(i, c)| (c, i)).collect();
    let mut cur = Cur {
        chars: &chars,
        pos: 0,
        pending: Vec::new(),
        end: source.len(),
    };
    let mut p = Parser {
        nest: 0,
        prev: Ty::Unset,
        curr: Ty::Unset,
        steps: 0,
        max_steps: max_steps(source.len()),
        frames: 0,
    };
    cur.remove_blanks(0);
    let out = p.expr(&mut cur, false)?;
    let mut tex = String::new();
    let mut map = Vec::new();
    for piece in out.frag.0 {
        let start = tex.len();
        tex.push_str(&piece.text);
        map.push((start..tex.len(), piece.src));
    }
    Ok(Translation {
        tex,
        len: source.len(),
        map,
    })
}

/// A run of TeX and the AsciiMath it came from, if any.
#[derive(Clone, Debug)]
struct Piece {
    text: String,
    src: Option<Range<usize>>,
}

/// TeX written so far, in runs that remember their source.
#[derive(Clone, Debug, Default)]
struct Frag(Vec<Piece>);

impl Frag {
    fn one(text: impl Into<String>, src: Option<Range<usize>>) -> Frag {
        let mut f = Frag::default();
        f.push(text, src);
        f
    }

    /// Adds a run; an empty one adds nothing.
    fn push(&mut self, text: impl Into<String>, src: Option<Range<usize>>) {
        let text = text.into();
        if !text.is_empty() {
            self.0.push(Piece { text, src });
        }
    }

    fn extend(&mut self, other: Frag) {
        self.0.extend(other.0);
    }

    /// The first `n` characters.
    fn head(&self, n: usize) -> String {
        self.0.iter().flat_map(|p| p.text.chars()).take(n).collect()
    }

    /// The last `n` characters.
    fn tail(&self, n: usize) -> String {
        let mut rev: Vec<char> = self
            .0
            .iter()
            .rev()
            .flat_map(|p| p.text.chars().rev())
            .take(n)
            .collect();
        rev.reverse();
        rev.into_iter().collect()
    }

    fn contains(&self, c: char) -> bool {
        self.0.iter().any(|p| p.text.contains(c))
    }

    /// Drops `front` characters from the start and `back` from the end.
    fn trim(&mut self, front: usize, back: usize) {
        let mut left = front;
        for p in &mut self.0 {
            if left == 0 {
                break;
            }
            let n = p.text.chars().count();
            if n <= left {
                left -= n;
                p.text.clear();
            } else {
                p.text = p.text.chars().skip(left).collect();
                left = 0;
            }
        }
        let mut left = back;
        for p in self.0.iter_mut().rev() {
            if left == 0 {
                break;
            }
            let n = p.text.chars().count();
            if n <= left {
                left -= n;
                p.text.clear();
            } else {
                p.text = p.text.chars().take(n - left).collect();
                left = 0;
            }
        }
        self.0.retain(|p| !p.text.is_empty());
    }
}

/// A bracketed group `{\left( ... \right)}` of the form a matrix is made of, with where its
/// content and the top-level commas in it are among its runs.
#[derive(Clone, Debug)]
struct Group {
    left: char,
    right: char,
    content: Range<usize>,
    commas: Vec<usize>,
    /// The content is itself a matrix.
    array: bool,
}

/// What `S` and `I` read: TeX, and the group it is when it is only one.
#[derive(Clone, Debug, Default)]
struct Node {
    frag: Frag,
    group: Option<Group>,
}

impl Node {
    fn new(frag: Frag) -> Node {
        Node { frag, group: None }
    }
}

/// The reading position: the source, with TeX a definition expands to put in front of it.
#[derive(Clone)]
struct Cur<'a> {
    chars: &'a [(char, usize)],
    pos: usize,
    /// Characters to read before the source, last first, with the source they stand for.
    pending: Vec<(char, Range<usize>)>,
    end: usize,
}

impl Cur<'_> {
    fn get(&self, i: usize) -> Option<(char, Range<usize>)> {
        let n = self.pending.len();
        if i < n {
            return self.pending.get(n - 1 - i).cloned();
        }
        let &(c, at) = self.chars.get(self.pos + i - n)?;
        Some((c, at..at + c.len_utf8()))
    }

    fn peek(&self, i: usize) -> Option<char> {
        self.get(i).map(|(c, _)| c)
    }

    /// The number of characters left.
    fn remaining(&self) -> usize {
        self.pending.len() + self.chars.len().saturating_sub(self.pos)
    }

    fn pop(&mut self, n: usize) {
        for _ in 0..n {
            if self.pending.pop().is_none() {
                self.pos = (self.pos + 1).min(self.chars.len());
            }
        }
    }

    /// The next `n` characters, or fewer at the end.
    fn look(&self, n: usize) -> String {
        (0..n).map_while(|i| self.peek(i)).collect()
    }

    /// The source of the next `n` characters.
    fn span(&self, n: usize) -> Range<usize> {
        let Some((_, first)) = self.get(0) else {
            return self.end..self.end;
        };
        let last = n
            .checked_sub(1)
            .and_then(|i| self.get(i))
            .map_or(first.clone(), |(_, r)| r);
        first.start..last.end.max(first.start)
    }

    /// Where the next character is in the source.
    fn at(&self) -> usize {
        self.get(0).map_or(self.end, |(_, r)| r.start)
    }

    /// `AMremoveCharsAndBlanks`: drops `n` characters, a backslash before a name, and the
    /// blanks after.
    fn remove_blanks(&mut self, n: usize) {
        self.pop(n);
        if self.peek(0) == Some('\\') && !matches!(self.peek(1), Some('\\' | ' ')) {
            self.pop(1);
        }
        while self.peek(0).is_some_and(|c| u32::from(c) <= 32) {
            self.pop(1);
        }
    }

    /// Puts `text`, which stands for the source `src`, in front of what is left.
    fn inject(&mut self, text: &str, src: &Range<usize>) {
        for c in text.chars().rev() {
            self.pending.push((c, src.clone()));
        }
    }
}

/// What the symbol at the reading position is: a table symbol, or the one character (or
/// number) that is not in the table.
#[derive(Clone, Debug)]
struct Tok {
    sym: Sym,
    /// The text of a symbol that is not in the table.
    text: Option<String>,
    /// For such a symbol, whether the translator tags it an operator (not a letter or number).
    mo: bool,
    /// How many characters it is.
    len: usize,
}

impl Tok {
    fn ty(&self) -> Ty {
        self.sym.ttype
    }

    fn input(&self) -> &str {
        self.text.as_deref().unwrap_or(self.sym.input)
    }

    /// The end of the source (the empty symbol).
    fn eof(&self) -> bool {
        self.text.as_deref() == Some("")
    }

    /// `AMTgetTeXsymbol`.
    fn tex(&self) -> String {
        let pre = if self.sym.val { "" } else { "\\" };
        let name = self.sym.tex.unwrap_or_else(|| self.input());
        format!("{pre}{name}")
    }

    /// A closing symbol whose TeX is `\rbrace`, which does not make a matrix.
    fn is_rbrace(&self) -> bool {
        self.sym.input == "}" || self.sym.input == "rbrace"
    }
}

/// One item of an expression being read: where it is in the TeX so far, and what it is.
struct Item {
    start: usize,
    end: usize,
    group: Option<Group>,
    comma: bool,
}

/// What `E` reads.
struct Expr {
    /// The TeX, with the closing bracket as its last run if there is one.
    frag: Frag,
    /// How many runs come before the closing bracket.
    content: usize,
    /// Where the top-level commas are.
    commas: Vec<usize>,
    /// The content is a matrix.
    array: bool,
    /// The TeX of the closing bracket after `\right`, if there is one.
    closer: Option<String>,
}

struct Parser {
    /// The brackets open (`AMnestingDepth`).
    nest: usize,
    /// The types of the last two symbols looked up (`AMpreviousSymbol`, `AMcurrentSymbol`): a
    /// `-` after an infix symbol is a sign.
    prev: Ty,
    curr: Ty,
    steps: usize,
    max_steps: usize,
    frames: usize,
}

impl Parser {
    fn fail(&self, cur: &Cur<'_>, message: &str) -> MathError {
        let span = cur.get(0).map_or(cur.end..cur.end, |(_, r)| r);
        MathError::new(message, span)
    }

    fn enter(&mut self, cur: &Cur<'_>) -> Res<()> {
        self.steps += 1;
        if self.steps > self.max_steps {
            return Err(self.fail(cur, "AsciiMath formula too complex"));
        }
        self.frames += 1;
        if self.frames > MAX_FRAMES {
            self.frames -= 1;
            return Err(self.fail(cur, "AsciiMath formula nested too deep"));
        }
        Ok(())
    }

    /// `AMgetSymbol`: the longest symbol of the table at the reading position, or a number, or
    /// the next character.
    fn symbol(&mut self, cur: &Cur<'_>) -> Tok {
        let look = cur.look(LOOKAHEAD);
        let (best, best_len) = longest_symbol(&look);
        self.prev = self.curr;
        if let Some(sym) = best {
            self.curr = sym.ttype;
            return Tok {
                sym,
                text: None,
                mo: true,
                len: best_len,
            };
        }
        self.curr = Ty::Const;
        let (text, mo) = number_or_char(cur);
        if text == "-" && cur.peek(1) != Some(' ') && self.prev == Ty::Infix {
            self.curr = Ty::Infix;
            let mut sym = base_sym();
            sym.ttype = Ty::Unary;
            sym.func = true;
            return Tok {
                sym,
                text: Some(text),
                mo,
                len: 1,
            };
        }
        let len = text.chars().count();
        Tok {
            sym: base_sym(),
            text: Some(text),
            mo,
            len,
        }
    }

    /// `AMTparseSexpr`: a simple expression: a symbol, a bracketed expression, or a command and
    /// its arguments. `None` where a closing bracket ends the enclosing expression.
    fn sexpr(&mut self, cur: &mut Cur<'_>) -> Res<Option<Node>> {
        self.enter(cur)?;
        let r = self.sexpr_inner(cur);
        self.frames -= 1;
        r
    }

    fn sexpr_inner(&mut self, cur: &mut Cur<'_>) -> Res<Option<Node>> {
        cur.remove_blanks(0);
        let mut sym = self.symbol(cur);
        if sym.ty() == Ty::RightBracket && self.nest > 0 {
            return Ok(None);
        }
        if sym.ty() == Ty::Definition {
            let src = cur.span(sym.len);
            cur.remove_blanks(sym.len);
            cur.inject(sym.sym.out, &src);
            sym = self.symbol(cur);
        }
        let src = cur.span(sym.len);
        Ok(Some(match sym.ty() {
            Ty::UnderOver | Ty::Const => {
                cur.remove_blanks(sym.len);
                let tex = sym.tex();
                if tex.starts_with('\\') || sym.mo {
                    Node::new(Frag::one(tex, Some(src)))
                } else {
                    Node::new(Frag::one(format!("{{{tex}}}"), Some(src)))
                }
            }
            Ty::LeftBracket => self.bracket(cur, &sym, src)?,
            Ty::Text => text(cur, &sym),
            Ty::Unary => self.unary(cur, &sym, src)?,
            Ty::Binary => self.binary(cur, &sym, src)?,
            Ty::Infix => {
                cur.remove_blanks(sym.len);
                Node::new(Frag::one(sym.input(), Some(src)))
            }
            Ty::Space => {
                cur.remove_blanks(sym.len);
                let word = sym.input();
                Node::new(Frag::one(
                    format!("{{\\quad\\text{{{word}}}\\quad}}"),
                    Some(src),
                ))
            }
            Ty::LeftRight => self.bars(cur, &sym, src)?,
            Ty::RightBracket | Ty::Definition | Ty::Unset => {
                cur.remove_blanks(sym.len);
                Node::new(Frag::one(format!("{{{}}}", sym.tex()), Some(src)))
            }
        }))
    }

    /// A bracket, and the expression up to its closing bracket.
    fn bracket(&mut self, cur: &mut Cur<'_>, sym: &Tok, src: Range<usize>) -> Res<Node> {
        self.nest += 1;
        cur.remove_blanks(sym.len);
        let ex = self.expr(cur, true);
        self.nest -= 1;
        let mut ex = ex?;
        // An empty group: `\right)` first.
        let head = ex.frag.head(13);
        let mut chop = 0;
        if let Some(rest) = head.strip_prefix("\\right") {
            match rest.chars().next() {
                Some(')' | ']' | '}') => chop = 6,
                Some('.') => chop = 7,
                _ if rest.starts_with("\\rbrace") => chop = 13,
                _ => {}
            }
        }
        let tex = sym.tex();
        let mut frag = Frag::default();
        let mut group = None;
        if chop > 0 {
            ex.frag.trim(chop, 0);
            if sym.sym.invisible {
                frag.push("{", Some(src));
            } else {
                frag.push(format!("{{{tex}"), Some(src));
            }
            frag.extend(ex.frag);
        } else {
            if sym.sym.invisible {
                frag.push("{\\left.", Some(src));
            } else {
                frag.push(format!("{{\\left{tex}"), Some(src));
            }
            let (left, right) = (tex.chars().next(), ex.closer.as_deref());
            if !sym.sym.invisible
                && let (Some(l @ ('(' | '[')), Some(r @ (")" | "]"))) = (left, right)
                && tex.len() == 1
            {
                group = Some(Group {
                    left: l,
                    right: r.chars().next().unwrap_or(')'),
                    content: 1..1 + ex.content,
                    commas: ex.commas.iter().map(|i| i + 1).collect(),
                    array: ex.array,
                });
            }
            frag.extend(ex.frag);
        }
        frag.push("}", None);
        Ok(Node { frag, group })
    }

    /// A unary symbol and its argument: `sqrt`, an accent, a font, a function, `abs`.
    fn unary(&mut self, cur: &mut Cur<'_>, sym: &Tok, src: Range<usize>) -> Res<Node> {
        cur.remove_blanks(sym.len);
        let after = cur.clone();
        let next = cur.peek(0);
        let Some(arg) = self.sexpr(cur)? else {
            return Ok(Node::new(Frag::one(
                format!("{{{}}}", sym.tex()),
                Some(src),
            )));
        };
        if sym.sym.func {
            let name = sym.input();
            let single = name.chars().count() == 1
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if matches!(next, Some('^' | '_' | '/' | '|' | ',')) || (single && next != Some('(')) {
                *cur = after;
                return Ok(Node::new(Frag::one(
                    format!("{{{}}}", sym.tex()),
                    Some(src),
                )));
            }
            let mut frag = Frag::one(format!("{{{}{{", sym.tex()), Some(src));
            frag.extend(arg.frag);
            frag.push("}}", None);
            return Ok(Node::new(frag));
        }
        let arg = remove_brackets(arg).frag;
        let mut frag = Frag::default();
        if sym.sym.id == Id::Sqrt {
            frag.push("\\sqrt{", Some(src));
        } else if sym.sym.id == Id::Cancel {
            frag.push("\\cancel{", Some(src));
        } else if let Some((l, r)) = sym.sym.lr {
            frag.push(format!("{{\\left{l}"), Some(src.clone()));
            frag.extend(arg);
            frag.push(format!("\\right{r}}}"), Some(src));
            return Ok(Node::new(frag));
        } else if sym.sym.acc {
            frag.push(format!("{}{{", sym.tex()), Some(src));
        } else {
            frag.push(format!("{{{}{{", sym.tex()), Some(src));
            frag.extend(arg);
            frag.push("}}", None);
            return Ok(Node::new(frag));
        }
        frag.extend(arg);
        frag.push("}", None);
        Ok(Node::new(frag))
    }

    /// A binary symbol and its two arguments: `frac`, `root`, `color`, `stackrel`.
    fn binary(&mut self, cur: &mut Cur<'_>, sym: &Tok, src: Range<usize>) -> Res<Node> {
        cur.remove_blanks(sym.len);
        let after = cur.clone();
        let alone =
            |src: Range<usize>| Node::new(Frag::one(format!("{{{}}}", sym.tex()), Some(src)));
        let Some(first) = self.sexpr(cur)? else {
            return Ok(alone(src));
        };
        let first = remove_brackets(first);
        let Some(second) = self.sexpr(cur)? else {
            *cur = after;
            return Ok(alone(src));
        };
        let second = remove_brackets(second);
        let mut frag = Frag::default();
        match sym.sym.id {
            Id::Color => {
                let name: String = first
                    .frag
                    .0
                    .iter()
                    .flat_map(|p| p.text.chars())
                    .filter(|c| !matches!(c, '{' | '}'))
                    .collect();
                frag.push(format!("{{\\color{{{name}}}"), Some(src));
                frag.extend(second.frag);
            }
            Id::Root => {
                frag.push("{\\sqrt[", Some(src));
                frag.extend(first.frag);
                frag.push("]{", None);
                frag.extend(second.frag);
                frag.push("}", None);
            }
            _ => {
                frag.push(format!("{{{}{{", sym.tex()), Some(src));
                frag.extend(first.frag);
                frag.push("}{", None);
                frag.extend(second.frag);
                frag.push("}", None);
            }
        }
        frag.push("}", None);
        Ok(Node::new(frag))
    }

    /// `|`: the start of an absolute value, or a divider (`|` before a comma, or with no
    /// closing `|`).
    fn bars(&mut self, cur: &mut Cur<'_>, sym: &Tok, src: Range<usize>) -> Res<Node> {
        self.nest += 1;
        cur.remove_blanks(sym.len);
        let after = cur.clone();
        let ex = self.expr(cur, false);
        self.nest -= 1;
        let ex = ex?;
        let closed = ex.frag.tail(1) == "|";
        if closed && after.peek(0) != Some(',') {
            let mut frag = Frag::one("{\\left|", Some(src));
            frag.extend(ex.frag);
            frag.push("}", None);
            Ok(Node::new(frag))
        } else {
            *cur = after;
            Ok(Node::new(Frag::one("{\\mid}", Some(src))))
        }
    }

    /// `AMTparseIexpr`: a simple expression and the subscript and superscript after it.
    fn iexpr(&mut self, cur: &mut Cur<'_>) -> Res<Option<Node>> {
        self.enter(cur)?;
        let r = self.iexpr_inner(cur);
        self.frames -= 1;
        r
    }

    fn iexpr_inner(&mut self, cur: &mut Cur<'_>) -> Res<Option<Node>> {
        cur.remove_blanks(0);
        let sym1 = self.symbol(cur);
        let node = self.sexpr(cur)?;
        let symbol = self.symbol(cur);
        if symbol.ty() != Ty::Infix || symbol.sym.id == Id::Slash {
            return Ok(node);
        }
        let mut node = node.unwrap_or_default().frag;
        let src = cur.span(symbol.len);
        cur.remove_blanks(symbol.len);
        let script = match self.sexpr(cur)? {
            None => Frag::one("{}", None),
            Some(n) => remove_brackets(n).frag,
        };
        if symbol.sym.id == Id::Sub {
            let sym2 = self.symbol(cur);
            if sym2.sym.id == Id::Sup && sym2.ty() == Ty::Infix {
                let src2 = cur.span(sym2.len);
                cur.remove_blanks(sym2.len);
                let sup = match self.sexpr(cur)? {
                    None => Frag::one("{}", None),
                    Some(n) => remove_brackets(n).frag,
                };
                let mut f = Frag::one("{", None);
                f.extend(node);
                f.push("_{", Some(src));
                f.extend(script);
                f.push("}^{", Some(src2));
                f.extend(sup);
                f.push("}}", None);
                node = f;
            } else {
                node.push("_{", Some(src));
                node.extend(script);
                node.push("}", None);
            }
        } else {
            node.push("^{", Some(src));
            node.extend(script);
            node.push("}", None);
        }
        if sym1.sym.func {
            let sym2 = self.symbol(cur);
            if sym2.ty() != Ty::Infix
                && sym2.ty() != Ty::RightBracket
                && (sym1.len > 1 || sym2.ty() == Ty::LeftBracket)
            {
                let rest = self.iexpr(cur)?.unwrap_or_default();
                let mut f = Frag::one("{", None);
                f.extend(node);
                f.extend(rest.frag);
                f.push("}", None);
                node = f;
            }
        }
        Ok(Some(Node::new(node)))
    }

    /// `AMTparseExpr`: expressions up to the end of the source or the bracket that closes the
    /// enclosing one, with a fraction made of two simple expressions around a `/`, and a matrix
    /// made of bracketed groups.
    fn expr(&mut self, cur: &mut Cur<'_>, rightbracket: bool) -> Res<Expr> {
        self.enter(cur)?;
        let r = self.expr_inner(cur, rightbracket);
        self.frames -= 1;
        r
    }

    fn expr_inner(&mut self, cur: &mut Cur<'_>, rightbracket: bool) -> Res<Expr> {
        let mut frag = Frag::default();
        let mut items: Vec<Item> = Vec::new();
        let mut symbol;
        loop {
            cur.remove_blanks(0);
            let node = self.iexpr(cur)?;
            symbol = self.symbol(cur);
            let start = frag.0.len();
            if symbol.ty() == Ty::Infix && symbol.sym.id == Id::Slash {
                let src = cur.span(symbol.len);
                cur.remove_blanks(symbol.len);
                let right = match self.iexpr(cur)? {
                    None => Frag::one("{}", None),
                    Some(n) => remove_brackets(n).frag,
                };
                let left = remove_brackets(node.unwrap_or_default()).frag;
                frag.push("\\frac{", Some(src));
                frag.extend(left);
                frag.push("}{", None);
                frag.extend(right);
                frag.push("}", None);
                items.push(Item {
                    start,
                    end: frag.0.len(),
                    group: None,
                    comma: false,
                });
                symbol = self.symbol(cur);
            } else if let Some(n) = node {
                let comma = n.frag.0.len() == 1 && n.frag.0.first().is_some_and(|p| p.text == ",");
                frag.extend(n.frag);
                items.push(Item {
                    start,
                    end: frag.0.len(),
                    group: n.group,
                    comma,
                });
            }
            let go = (symbol.ty() != Ty::RightBracket
                && (symbol.ty() != Ty::LeftRight || rightbracket))
                || self.nest == 0;
            if !go || symbol.eof() {
                break;
            }
        }
        let mut commas: Vec<usize> = items.iter().filter(|i| i.comma).map(|i| i.start).collect();
        let mut array = false;
        let mut closer = None;
        if symbol.ty() == Ty::RightBracket || symbol.ty() == Ty::LeftRight {
            if let Some(m) = matrix(&frag, &items, symbol.is_rbrace()) {
                frag = m;
                array = true;
                commas.clear();
            }
            let content = frag.0.len();
            let src = cur.span(symbol.len);
            cur.remove_blanks(symbol.len);
            if symbol.sym.invisible {
                frag.push("\\right.", Some(src));
                closer = Some(".".to_owned());
            } else {
                let tex = symbol.tex();
                frag.push(format!("\\right{tex}"), Some(src));
                closer = Some(tex);
            }
            return Ok(Expr {
                frag,
                content,
                commas,
                array,
                closer,
            });
        }
        let content = frag.0.len();
        if self.nest > 0 {
            frag.push("\\right.", None);
            closer = Some(".".to_owned());
        }
        Ok(Expr {
            frag,
            content,
            commas,
            array,
            closer,
        })
    }
}

/// The symbol that is not in the table: one character, or a number.
fn base_sym() -> Sym {
    table::plain()
}

/// The text and kind of the symbol at `cur` that is not in the table: a number
/// (`[0-9]+(\.[0-9]+)?`, taking a decimal point that is followed by anything as part of it, as
/// the translator does), or one character.
fn number_or_char(cur: &Cur<'_>) -> (String, bool) {
    let len = cur.remaining();
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    let mut k = 1;
    let mut st = cur.peek(0);
    let mut integ = true;
    let mut used_decimal = false;
    while digit(st) && k <= len {
        st = cur.peek(k);
        k += 1;
    }
    if st == Some('.') {
        st = cur.peek(k);
        if k > 1 && k < len {
            k += 1;
            used_decimal = true;
        }
        if digit(st) {
            integ = false;
            if !used_decimal {
                k += 1;
            }
            while digit(st) && k <= len {
                st = cur.peek(k);
                k += 1;
            }
        }
    }
    if (integ && k > 1) || k > 2 {
        return (cur.look(k - 1), false);
    }
    let c = cur.peek(0);
    let letter = c.is_some_and(|c| c.is_ascii_alphabetic());
    (c.map(String::from).unwrap_or_default(), !letter)
}

/// `text(...)`, `mbox(...)`, `"..."`, and `mspace(...)`.
fn text(cur: &mut Cur<'_>, sym: &Tok) -> Node {
    let src_start = cur.at();
    if sym.sym.id != Id::Quote {
        cur.remove_blanks(sym.len);
    }
    let close = match cur.peek(0) {
        Some('{') => Some('}'),
        Some('(') => Some(')'),
        Some('[') => Some(']'),
        _ => None,
    };
    // The index of the closing character; 0 where there is none to look for.
    let mut i = match (close, sym.sym.id) {
        (Some(c), _) => (0..cur.remaining()).find(|&k| cur.peek(k) == Some(c)),
        (None, Id::Quote) => (1..cur.remaining())
            .find(|&k| cur.peek(k) == Some('"'))
            .or(Some(0)),
        (None, _) => Some(0),
    };
    // A closing character that is missing: the text runs to the end.
    let len = cur.remaining();
    let i = *i.get_or_insert(len);
    let body: String = (1..i).map_while(|k| cur.peek(k)).collect();
    let end = cur.span(i + 1).end;
    let src = src_start..end.max(src_start);
    if sym.sym.id == Id::Mspace {
        cur.remove_blanks(i + 1);
        return Node::new(Frag::one(mspace(&body), Some(src)));
    }
    let mut tex = String::new();
    if body.starts_with(' ') {
        tex.push_str("\\ ");
    }
    tex.push_str("\\text{");
    for c in body.chars() {
        match c {
            '\\' => tex.push_str("\\textbackslash{}"),
            '{' | '}' | '%' | '$' | '#' | '&' | '_' => {
                tex.push('\\');
                tex.push(c);
            }
            '^' => tex.push_str("\\textasciicircum{}"),
            '~' => tex.push_str("\\textasciitilde{}"),
            _ => tex.push(c),
        }
    }
    tex.push('}');
    if body.ends_with(' ') {
        tex.push_str("\\ ");
    }
    cur.remove_blanks(i + 1);
    Node::new(Frag::one(tex, Some(src)))
}

/// `mspace(1em)`: a space of that many `mu` (an `em` is 18) or `em`, as `\mkern`; nothing for
/// anything else.
fn mspace(body: &str) -> String {
    let s = body.strip_prefix('-').unwrap_or(body);
    let negative = s.len() != body.len();
    let digits = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (number, rest) = s.split_at(digits);
    let unit = rest.trim_start();
    if number.is_empty() || !matches!(unit, "" | "em" | "mu") {
        return String::new();
    }
    // `parseFloat`: the longest prefix that is a number.
    let mut end = 0;
    let mut dot = false;
    for (i, c) in number.char_indices() {
        if c == '.' {
            if dot {
                break;
            }
            dot = true;
        }
        end = i + 1;
    }
    let Ok(mut value) = number[..end].parse::<f64>() else {
        return String::new();
    };
    if negative {
        value = -value;
    }
    if unit == "em" {
        value *= 16.0;
    }
    if !value.is_finite() {
        return String::new();
    }
    format!("\\mkern{{{value}mu}}")
}

/// `AMTremoveBrackets`: the brackets of a group that is all of an argument (`sqrt(x)`, `x^(2)`)
/// are not drawn.
fn remove_brackets(mut node: Node) -> Node {
    node.group = None;
    let head: Vec<char> = node.frag.head(14).chars().collect();
    if head.first() != Some(&'{') || node.frag.tail(1) != "}" {
        return node;
    }
    let at =
        |from: usize, to: usize| -> String { head.iter().skip(from).take(to - from).collect() };
    let mut chop = 0;
    if at(1, 6) == "\\left" {
        if matches!(head.get(6), Some('(' | '[' | '{')) {
            chop = 7;
        } else if at(6, 13) == "\\lbrace" {
            chop = 13;
        }
    } else if matches!(head.get(1), Some('(' | '[')) {
        chop = 2;
    }
    if chop == 0 {
        return node;
    }
    let tail = node.frag.tail(8);
    let back = if matches!(tail.as_str(), "\\right)}" | "\\right]}" | "\\right.}") {
        8
    } else if tail == "\\rbrace}" {
        14
    } else {
        return node;
    };
    node.frag.trim(chop, back);
    let mut frag = Frag::one("{", None);
    frag.extend(node.frag);
    frag.push("}", None);
    node.frag = frag;
    node
}

/// A matrix, when `frag`, the items of an expression that a closing bracket ends, are bracketed
/// groups with commas between them: `[[a,b],[c,d]]`, `((a),(b))`, `{(a,b),(c,d):}`. Each group
/// is a row; a lone `|` in a row (`[[a,b,|,c]]`) is a vertical rule in that place.
fn matrix(frag: &Frag, items: &[Item], closing_rbrace: bool) -> Option<Frag> {
    if !frag.contains(',') || frag.head(1) != "{" || items.len().is_multiple_of(2) {
        return None;
    }
    let first = items.first()?.group.as_ref()?;
    let last = items.last()?.group.as_ref()?;
    let (l, r) = (first.left, last.right);
    if !((l == '(' && r == ')' && !closing_rbrace) || (l == '[' && r == ']')) {
        return None;
    }
    let mut rows = Vec::new();
    for (k, item) in items.iter().enumerate() {
        if k % 2 == 1 {
            if !item.comma {
                return None;
            }
            continue;
        }
        let g = item.group.as_ref()?;
        if g.left != l || g.right != r || g.array {
            return None;
        }
        let pieces = frag.0.get(item.start..item.end)?;
        rows.push((pieces, g));
    }
    let mut aligns = String::new();
    let mut out_rows: Vec<Vec<Vec<Piece>>> = Vec::new();
    let mut count: Option<usize> = None;
    for (i, (pieces, g)) in rows.iter().enumerate() {
        // The elements: the runs between the commas.
        let mut elements: Vec<Vec<Piece>> = Vec::new();
        let mut from = g.content.start;
        for &comma in g.commas.iter().chain(std::iter::once(&g.content.end)) {
            elements.push(pieces.get(from..comma)?.to_vec());
            from = comma + 1;
        }
        for j in (0..elements.len()).rev() {
            let bar = elements
                .get(j)
                .is_some_and(|e| e.len() == 1 && e.first().is_some_and(|p| p.text == "{\\mid}"));
            if bar {
                if i == 0 {
                    aligns.insert(0, '|');
                }
                elements.remove(j);
            } else if i == 0 {
                aligns.insert(0, 'c');
            }
        }
        match count {
            Some(n) if n > 0 && elements.len() != n => return None,
            None => count = Some(elements.len()),
            _ => {}
        }
        out_rows.push(elements);
    }
    let mut out = Frag::one(format!("\\begin{{array}}{{{aligns}}} "), None);
    for (i, row) in out_rows.into_iter().enumerate() {
        if i > 0 {
            out.push("\\\\", None);
        }
        for (j, element) in row.into_iter().enumerate() {
            if j > 0 {
                out.push("&", None);
            }
            out.0.extend(element);
        }
    }
    out.push("\\end{array}", None);
    Some(out)
}

/// The error of the TeX of a translation, on the AsciiMath that made it.
#[cfg(feature = "math")]
fn remap(t: &Translation, e: MathError) -> MathError {
    MathError::new(e.message, t.source_span(e.span))
}

/// Typesets AsciiMath `source` in `style`: [`to_tex`], then `typeset`. An error's span is a byte
/// range of `source` (see [`Translation::source_span`]).
#[cfg(feature = "math")]
pub fn typeset_asciimath(
    source: &str,
    style: super::formula::MathStyle,
) -> Res<super::formula::Formula> {
    let t = translate(source)?;
    check_tex(&t)?;
    super::formula::typeset(&t.tex, style).map_err(|e| remap(&t, e))
}

/// The error for translated TeX past the TeX limit, which is named as such: the AsciiMath source
/// passed its own limit in `translate`, so the length is one the user never wrote.
#[cfg(feature = "math")]
fn check_tex(t: &Translation) -> Res<()> {
    if t.tex.len() <= limits::MAX_TRANSLATED_TEX {
        return Ok(());
    }
    Err(MathError::new(
        format!(
            "translated TeX longer than {} bytes",
            limits::MAX_TRANSLATED_TEX
        ),
        t.source_span(limits::MAX_TRANSLATED_TEX..t.tex.len()),
    ))
}

/// The AsciiMath `source` as rows of text and the row of the baseline (see
/// `ascii_formula_rows`); errors as [`typeset_asciimath`] does.
#[cfg(feature = "math")]
pub fn ascii_asciimath_rows(
    source: &str,
    style: super::formula::MathStyle,
) -> Res<(Vec<String>, usize)> {
    let t = translate(source)?;
    check_tex(&t)?;
    super::ascii::ascii_formula_rows(&t.tex, style).map_err(|e| remap(&t, e))
}

/// The AsciiMath `source` as text; see [`ascii_asciimath_rows`].
#[cfg(feature = "math")]
pub fn ascii_asciimath(source: &str, style: super::formula::MathStyle) -> Res<String> {
    Ok(ascii_asciimath_rows(source, style)?.0.join("\n"))
}
