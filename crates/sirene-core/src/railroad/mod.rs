//! Mermaid railroad diagrams (MRAIL-*): grammar rules written in EBNF, ABNF, PEG, or Mermaid's
//! constructor notation, drawn as syntax diagrams.

mod parser;

use std::collections::HashSet;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxElement, SyntaxNode};

/// The notation a railroad diagram's rules are written in, picked by the header keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Notation {
    /// `railroad-beta`: constructors such as `choice(a, b)`.
    #[default]
    Ir,
    /// `railroad-ebnf-beta`.
    Ebnf,
    /// `railroad-abnf-beta`.
    Abnf,
    /// `railroad-peg-beta`.
    Peg,
}

impl Notation {
    /// The notation of a header keyword.
    pub fn of_header(keyword: &str) -> Option<Notation> {
        Some(match keyword {
            "railroad-beta" => Notation::Ir,
            "railroad-ebnf-beta" => Notation::Ebnf,
            "railroad-abnf-beta" => Notation::Abnf,
            "railroad-peg-beta" => Notation::Peg,
            _ => return None,
        })
    }
}

/// A part of a rule's definition (MRAIL-2).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal, drawn in a rounded box.
    Terminal(String),
    /// A reference to a rule, drawn in a box.
    NonTerminal(String),
    /// Text drawn as `? text ?` in a dashed box.
    Special(String),
    Sequence(Vec<Expr>),
    Choice(Vec<Expr>),
    Optional(Box<Expr>),
    /// `min` or more of `expr`, and at most `max`; drawn with a bypass when `min` is 0.
    Repeat {
        expr: Box<Expr>,
        min: u32,
        max: Option<u32>,
    },
}

/// `name = definition ;`.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub name: String,
    pub definition: Expr,
}

/// A railroad diagram (MRAIL-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Railroad {
    pub notation: Notation,
    /// The `title` statement. Mermaid does not draw it.
    pub title: Option<String>,
    /// The rules in source order, each drawn on its own line.
    pub rules: Vec<Rule>,
}

/// The result of parsing railroad diagram source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    notation: Notation,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Railroad`], with the parser's diagnostics.
    pub fn lower(&self) -> (Railroad, Vec<Diagnostic>) {
        let root = self.syntax();
        let mut diagnostics = self.diagnostics.clone();
        let mut chart = Railroad {
            notation: self.notation,
            ..Railroad::default()
        };
        let mut seen = HashSet::new();
        for n in root.children() {
            match n.kind() {
                TITLE_STMT => {
                    if let Some(t) = tokens(&n).find(|t| t.kind() == TEXT) {
                        chart.title = Some(self.unquote_title(t.text())).filter(|t| !t.is_empty());
                    }
                }
                RULE => {
                    let Some(name) = tokens(&n).find(|t| t.kind() == IDENT) else {
                        continue;
                    };
                    let body = n
                        .children()
                        .find(|c| matches!(c.kind(), RR_CHOICE | RR_CALL));
                    let Some(definition) = body.and_then(|b| self.expr(&b)) else {
                        continue;
                    };
                    let name = name.text().to_owned();
                    if !seen.insert(name.clone()) {
                        diagnostics.push(Diagnostic::warning(
                            n.text_range(),
                            format!("rule `{name}` is already defined; both are drawn"),
                        ));
                    }
                    chart.rules.push(Rule { name, definition });
                }
                _ => {}
            }
        }
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a railroad diagram: {} rules; {} diagnostics",
            chart.rules.len(),
            diagnostics.len()
        );
        (chart, diagnostics)
    }

    /// A title without its quotes, as Mermaid reads it.
    fn unquote_title(&self, t: &str) -> String {
        let t = t.trim();
        let quoted = t.len() >= 2
            && ((t.starts_with('"') && t.ends_with('"'))
                || (t.starts_with('\'') && t.ends_with('\'')));
        if !quoted {
            t.to_owned()
        } else if self.notation == Notation::Abnf {
            t[1..t.len() - 1].to_owned()
        } else {
            unescape(t)
        }
    }

    /// A string token's text, as Mermaid reads it: without its quotes, and with `\` escapes
    /// decoded except in ABNF.
    fn string(&self, s: &str) -> String {
        if self.notation == Notation::Abnf {
            s[1..s.len() - 1].to_owned()
        } else {
            unescape(s)
        }
    }

    fn expr(&self, n: &SyntaxNode) -> Option<Expr> {
        match n.kind() {
            RR_CHOICE => {
                let alts: Vec<Expr> = n
                    .children()
                    .filter(|c| c.kind() == RR_SEQ)
                    .map(|c| self.expr(&c))
                    .collect::<Option<_>>()?;
                collapse(alts, Expr::Choice)
            }
            RR_SEQ => {
                let items: Vec<Expr> = n
                    .children()
                    .filter(|c| c.kind() == RR_TERM)
                    .map(|c| self.expr(&c))
                    .collect::<Option<_>>()?;
                collapse(items, Expr::Sequence)
            }
            RR_TERM => self.term(n),
            RR_GROUP => {
                let inner = self.expr(&n.children().find(|c| c.kind() == RR_CHOICE)?)?;
                let open = tokens(n).next()?.kind();
                Some(match open {
                    L_BRACKET => Expr::Optional(Box::new(inner)),
                    L_BRACE => Expr::Repeat {
                        expr: Box::new(inner),
                        min: 0,
                        max: None,
                    },
                    _ => inner,
                })
            }
            RR_CALL => self.call(n),
            _ => None,
        }
    }

    /// An element with its operators.
    fn term(&self, n: &SyntaxNode) -> Option<Expr> {
        let parts: Vec<SyntaxElement> = n
            .children_with_tokens()
            .filter(|e| !e.kind().is_trivia())
            .collect();
        let mut i = 0;
        // A prefix: an ABNF repeat or a PEG predicate.
        let prefix = match parts.first()?.kind() {
            RR_REPEAT | RR_OP => {
                i = 1;
                parts
                    .first()
                    .and_then(|e| e.as_token())
                    .map(|t| t.text().to_owned())
            }
            _ => None,
        };
        let mut e = self.primary(parts.get(i)?)?;
        i += 1;
        while let Some(op) = parts.get(i) {
            i += 1;
            let op = op.as_token().map(|t| t.text()).unwrap_or_default();
            e = match op {
                "?" => Expr::Optional(Box::new(e)),
                "*" | "+" => Expr::Repeat {
                    expr: Box::new(e),
                    min: u32::from(op == "+"),
                    max: None,
                },
                "-" => {
                    let except = self.primary(parts.get(i)?)?;
                    i += 1;
                    Expr::Sequence(vec![e, Expr::Terminal("-".to_owned()), except])
                }
                _ => return None,
            };
        }
        Some(match prefix.as_deref() {
            None => e,
            Some(p @ ("&" | "!")) => Expr::Special(format!("{p}{}", label(&e))),
            Some(repeat) => {
                let (min, max) = repeat_bounds(repeat);
                if min == 0 && max == Some(1) {
                    Expr::Optional(Box::new(e))
                } else {
                    Expr::Repeat {
                        expr: Box::new(e),
                        min,
                        max,
                    }
                }
            }
        })
    }

    fn primary(&self, e: &SyntaxElement) -> Option<Expr> {
        if let Some(n) = e.as_node() {
            return self.expr(n);
        }
        let t = e.as_token()?;
        let s = t.text();
        Some(match t.kind() {
            STRING => Expr::Terminal(self.string(s)),
            IDENT => Expr::NonTerminal(s.to_owned()),
            RR_NUMVAL => Expr::Terminal(s.to_owned()),
            RR_SPECIAL => Expr::Special(s[1..s.len() - 1].trim().to_owned()),
            DOT => Expr::Special(".".to_owned()),
            _ => return None,
        })
    }

    /// An IR constructor call.
    fn call(&self, n: &SyntaxNode) -> Option<Expr> {
        let name = tokens(n).find(|t| t.kind() == RAILROAD_KW)?;
        let string = || {
            tokens(n)
                .find(|t| t.kind() == STRING)
                .map(|t| self.string(t.text()))
        };
        let args = || -> Option<Vec<Expr>> {
            n.children()
                .filter(|c| c.kind() == RR_CALL)
                .map(|c| self.call(&c))
                .collect()
        };
        let one = || -> Option<Box<Expr>> { args()?.into_iter().next().map(Box::new) };
        Some(match name.text() {
            "terminal" => Expr::Terminal(string()?),
            "nonterminal" => Expr::NonTerminal(string()?),
            "special" => Expr::Special(string()?),
            "sequence" => collapse(args()?, Expr::Sequence)?,
            "choice" => collapse(args()?, Expr::Choice)?,
            "optional" => Expr::Optional(one()?),
            "zeroOrMore" => Expr::Repeat {
                expr: one()?,
                min: 0,
                max: None,
            },
            "oneOrMore" => Expr::Repeat {
                expr: one()?,
                min: 1,
                max: None,
            },
            _ => return None,
        })
    }
}

fn tokens(n: &SyntaxNode) -> impl Iterator<Item = crate::syntax::SyntaxToken> + use<> {
    n.children_with_tokens().filter_map(|e| e.into_token())
}

/// One item as itself, more as `make` of them, none as `None`.
fn collapse(mut items: Vec<Expr>, make: fn(Vec<Expr>) -> Expr) -> Option<Expr> {
    match items.len() {
        0 => None,
        1 => items.pop(),
        _ => Some(make(items)),
    }
}

/// The text of a PEG predicate's operand, as Mermaid labels it.
fn label(e: &Expr) -> String {
    match e {
        Expr::Terminal(v) => format!("\"{v}\""),
        Expr::NonTerminal(n) => n.clone(),
        Expr::Special(t) => t.clone(),
        _ => "(...)".to_owned(),
    }
}

/// The bounds of an ABNF repeat: `*` is 0 or more, `1*` 1 or more, `2*4` 2 to 4, and `3`
/// exactly 3.
fn repeat_bounds(s: &str) -> (u32, Option<u32>) {
    let num = |s: &str| s.parse::<u32>().ok();
    match s.split_once('*') {
        Some((lo, hi)) => (num(lo).unwrap_or(0), num(hi)),
        None => {
            let n = num(s).unwrap_or(0);
            (n, Some(n))
        }
    }
}

/// A quoted string's contents with `\n`, `\r`, `\t`, and `\x` for any other `x` decoded.
fn unescape(s: &str) -> String {
    let body = &s[1..s.len() - 1];
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// Parses railroad diagram source in the notation its header names. Never fails; problems
/// are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let notation = Notation::of_header(crate::diagram::header(src)).unwrap_or_default();
    let (green, diagnostics) = parser::parse(src, notation);
    Parse {
        green,
        notation,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(src: &str) -> Vec<Rule> {
        let (r, d) = parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        r.rules
    }

    fn t(s: &str) -> Expr {
        Expr::Terminal(s.to_owned())
    }

    fn nt(s: &str) -> Expr {
        Expr::NonTerminal(s.to_owned())
    }

    fn rep(e: Expr, min: u32, max: Option<u32>) -> Expr {
        Expr::Repeat {
            expr: Box::new(e),
            min,
            max,
        }
    }

    #[test]
    fn ebnf() {
        let r = rules(
            "railroad-ebnf-beta\ntitle \"A \\\"b\\\"\"\n\
             a = \"x\" , 'y\\n' | b? c* d+ ;\n\
             e ::= [ f ] { g } ( h ) ? any ? ;\n\
             i = j - k ;\n",
        );
        assert_eq!(
            r[0].definition,
            Expr::Choice(vec![
                Expr::Sequence(vec![t("x"), t("y\n")]),
                Expr::Sequence(vec![
                    Expr::Optional(Box::new(nt("b"))),
                    rep(nt("c"), 0, None),
                    rep(nt("d"), 1, None),
                ]),
            ])
        );
        assert_eq!(
            r[1].definition,
            Expr::Sequence(vec![
                Expr::Optional(Box::new(nt("f"))),
                rep(nt("g"), 0, None),
                nt("h"),
                Expr::Special("any".to_owned()),
            ])
        );
        assert_eq!(
            r[2].definition,
            Expr::Sequence(vec![nt("j"), t("-"), nt("k")])
        );
        let (chart, _) = parse("railroad-ebnf-beta\ntitle \"A \\\"b\\\"\"\n").lower();
        assert_eq!(chart.title.as_deref(), Some("A \"b\""));
        assert_eq!(chart.notation, Notation::Ebnf);
    }

    #[test]
    fn abnf() {
        let r = rules(
            "railroad-abnf-beta\ntitle 'T'\n\
             a = *b 1*c 2*4d 3e *1f [ g / \"h\\\" ] %x30-39 ;\n",
        );
        assert_eq!(
            r[0].definition,
            Expr::Sequence(vec![
                rep(nt("b"), 0, None),
                rep(nt("c"), 1, None),
                rep(nt("d"), 2, Some(4)),
                rep(nt("e"), 3, Some(3)),
                Expr::Optional(Box::new(nt("f"))),
                Expr::Optional(Box::new(Expr::Choice(vec![nt("g"), t("h\\")]))),
                t("%x30-39"),
            ])
        );
    }

    #[test]
    fn peg() {
        let r = rules("railroad-peg-beta\nA <- !B C? / &\"d\" . / !(e f) ;\n");
        assert_eq!(
            r[0].definition,
            Expr::Choice(vec![
                Expr::Sequence(vec![
                    Expr::Special("!B".to_owned()),
                    Expr::Optional(Box::new(nt("C"))),
                ]),
                Expr::Sequence(vec![
                    Expr::Special("&\"d\"".to_owned()),
                    Expr::Special(".".to_owned()),
                ]),
                Expr::Special("!(...)".to_owned()),
            ])
        );
    }

    #[test]
    fn ir() {
        let r = rules(
            "railroad-beta\nr = sequence(terminal(\"a\"), choice(nonterminal('b')), \
             optional(special(\"s\")), zeroOrMore(terminal(\"c\")), oneOrMore(terminal(\"d\"))) ;\n",
        );
        assert_eq!(
            r[0].definition,
            Expr::Sequence(vec![
                t("a"),
                nt("b"),
                Expr::Optional(Box::new(Expr::Special("s".to_owned()))),
                rep(t("c"), 0, None),
                rep(t("d"), 1, None),
            ])
        );
    }

    #[test]
    fn duplicates_and_errors() {
        let (chart, d) = parse("railroad-ebnf-beta\na = b ;\na = c ;\nd = ( ;\n").lower();
        assert_eq!(chart.rules.len(), 2);
        let msgs: Vec<&str> = d.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            [
                "rule `a` is already defined; both are drawn",
                "expected a string, a rule name, `(`, `[`, `{`, or `? text ?`"
            ]
        );
    }
}
