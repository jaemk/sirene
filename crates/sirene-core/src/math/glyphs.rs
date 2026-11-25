//! The characters the text renderer draws formulas with (LMATH-4), by character set.
//!
//! Many terminal fonts draw a character they lack from a fallback font, often wider than one
//! cell, which shifts the rest of its row. A formula is drawn with one of three sets:
//!
//! - `UnicodeMath`: any character the symbol table gives (the radical, super- and subscript
//!   letters, bracket pieces, mathematical operators).
//! - `Unicode`: only the characters in [`ALLOWED`], which DejaVu Sans Mono, Menlo, Consolas,
//!   Cascadia Mono, and Courier New draw in one cell.
//! - `Ascii`: printable ASCII (`' '..='~'`).
//!
//! Every glyph the renderer builds goes through [`fold`] or [`text`], so a block's width is that of
//! the text drawn.

use std::borrow::Cow;

use crate::ascii::Charset;

#[cfg(test)]
pub(super) mod tests;

/// The characters (inclusive ranges) of the `Unicode` set, other than the ones of `UnicodeMath`:
/// printable ASCII, the Latin-1 supplement (letters, `\u{b0}`, `\u{b1}`, `\u{b7}`, `\u{d7}`,
/// `\u{f7}`, `\u{b2}`, `\u{b3}`, `\u{b9}` and the rest, but not the no-break space, the soft
/// hyphen, the macron `\u{af}`, the acute accent, or the cedilla), Greek letters (`\u{391}` to
/// `\u{3a9}`, `\u{3b1}` to `\u{3c9}`), and the box-drawing block, which the rest of the text
/// output draws with.
pub(crate) const ALLOWED: &[(char, char)] = &[
    (' ', '~'),
    ('\u{a1}', '\u{ac}'),
    ('\u{ae}', '\u{ae}'),
    ('\u{b0}', '\u{b3}'),
    ('\u{b5}', '\u{b7}'),
    ('\u{b9}', '\u{ff}'),
    ('\u{391}', '\u{3a1}'),
    ('\u{3a3}', '\u{3a9}'),
    ('\u{3b1}', '\u{3c9}'),
    ('\u{2500}', '\u{257f}'),
];

/// Whether the `Unicode` set has `c`.
pub(crate) fn is_allowed(c: char) -> bool {
    ALLOWED.iter().any(|&(lo, hi)| lo <= c && c <= hi)
}

/// Whether `c` is printable ASCII, the `Ascii` set.
pub(crate) fn is_ascii_cell(c: char) -> bool {
    (' '..='~').contains(&c)
}

/// Whether `charset` draws `c` as itself.
fn has(c: char, charset: Charset) -> bool {
    match charset {
        Charset::UnicodeMath => true,
        Charset::Unicode => is_allowed(c),
        Charset::Ascii => is_ascii_cell(c),
    }
}

/// `c` as drawn in `charset`, or `None` when it has no form there. A character with a form in
/// the set is itself; otherwise the table's form, which for `Unicode` may be an allowed Greek
/// letter or Latin-1 character and else is ASCII.
pub(super) fn fold(c: char, charset: Charset) -> Option<Cow<'static, str>> {
    if has(c, charset) {
        return Some(Cow::Owned(c.to_string()));
    }
    if charset == Charset::Unicode
        && let Some(&(_, to)) = UNICODE_FORMS.iter().find(|(from, _)| *from == c)
    {
        return Some(Cow::Owned(to.to_string()));
    }
    ascii_form(c)
}

/// `s` as drawn in `charset`, `?` for each character with no form. A character of zero width (a
/// combining mark, a joiner) is dropped, as the renderer drops it from its cells.
pub(super) fn text(s: &str, charset: Charset) -> Cow<'_, str> {
    if s.chars().all(|c| has(c, charset)) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if unicode_width::UnicodeWidthChar::width(c) == Some(0) && c != '\0' {
            continue;
        }
        match fold(c, charset) {
            Some(form) => out.push_str(&form),
            None => out.push('?'),
        }
    }
    Cow::Owned(out)
}

/// `s` as drawn in `charset`, for text from the source (a character of a `\text`, a `\verb`).
/// `Unicode` keeps it as written, since a letter of another script has no form in the set and is
/// the author's own; `Ascii` folds it.
pub(super) fn literal(s: &str, charset: Charset) -> Cow<'_, str> {
    match charset {
        Charset::Unicode => Cow::Borrowed(s),
        _ => text(s, charset),
    }
}

/// `s` as drawn in `charset` if every character has a form there.
pub(super) fn exact(s: &str, charset: Charset) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push_str(&fold(c, charset)?);
    }
    Some(out)
}

/// A character outside the `Unicode` set and a character of it that stands for it there.
const UNICODE_FORMS: &[(char, char)] = &[
    ('\u{3d1}', '\u{3b8}'),
    ('\u{3d5}', '\u{3c6}'),
    ('\u{3d6}', '\u{3c0}'),
    ('\u{3f0}', '\u{3ba}'),
    ('\u{3f1}', '\u{3c1}'),
    ('\u{3f5}', '\u{3b5}'),
    ('\u{2219}', '\u{b7}'),
    ('\u{22c5}', '\u{b7}'),
    ('\u{2212}', '-'),
];

/// The ASCII form of `c`: its table entry, a letter without its accent or style, or `None`.
fn ascii_form(c: char) -> Option<Cow<'static, str>> {
    if let Some(&(_, form)) = FORMS.iter().find(|(from, _)| *from == c) {
        return Some(Cow::Borrowed(form));
    }
    let code = c as u32;
    // Mathematical alphanumerics: the plain letter or digit.
    let plain = match code {
        0x1d400..=0x1d6a3 => {
            let k = (code - 0x1d400) % 52;
            let base = if k < 26 { b'A' } else { b'a' };
            Some(char::from(base + (k % 26) as u8))
        }
        0x1d7ce..=0x1d7ff => Some(char::from(b'0' + ((code - 0x1d7ce) % 10) as u8)),
        _ => None,
    };
    if let Some(p) = plain {
        return Some(Cow::Owned(p.to_string()));
    }
    latin1_letter(c).map(Cow::Borrowed)
}

/// A Latin-1 letter without its accent.
fn latin1_letter(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{c0}'..='\u{c5}' => "A",
        '\u{c6}' => "AE",
        '\u{c7}' => "C",
        '\u{c8}'..='\u{cb}' => "E",
        '\u{cc}'..='\u{cf}' => "I",
        '\u{d0}' => "D",
        '\u{d1}' => "N",
        '\u{d2}'..='\u{d6}' | '\u{d8}' => "O",
        '\u{d9}'..='\u{dc}' => "U",
        '\u{dd}' => "Y",
        '\u{de}' => "Th",
        '\u{df}' => "ss",
        '\u{e0}'..='\u{e5}' => "a",
        '\u{e6}' => "ae",
        '\u{e7}' => "c",
        '\u{e8}'..='\u{eb}' => "e",
        '\u{ec}'..='\u{ef}' => "i",
        '\u{f0}' => "d",
        '\u{f1}' => "n",
        '\u{f2}'..='\u{f6}' | '\u{f8}' => "o",
        '\u{f9}'..='\u{fc}' => "u",
        '\u{fd}' | '\u{ff}' => "y",
        '\u{fe}' => "th",
        _ => return None,
    })
}

/// Characters of the symbol table, and the ASCII text that stands for each.
const FORMS: &[(char, &str)] = &[
    // Spaces.
    ('\u{a0}', " "),
    ('\u{2002}', " "),
    ('\u{2003}', " "),
    ('\u{2009}', " "),
    ('\u{202f}', " "),
    // Latin-1 symbols.
    ('\u{a1}', "!"),
    ('\u{a2}', "c"),
    ('\u{a3}', "L"),
    ('\u{a4}', "o"),
    ('\u{a5}', "Y"),
    ('\u{a6}', "|"),
    ('\u{a7}', "S"),
    ('\u{a8}', ".."),
    ('\u{a9}', "(c)"),
    ('\u{aa}', "a"),
    ('\u{ab}', "<<"),
    ('\u{ac}', "!"),
    ('\u{ae}', "(R)"),
    ('\u{af}', "-"),
    ('\u{b0}', "deg"),
    ('\u{b1}', "+/-"),
    ('\u{b2}', "^2"),
    ('\u{b3}', "^3"),
    ('\u{b4}', "'"),
    ('\u{b5}', "u"),
    ('\u{b6}', "P"),
    ('\u{b7}', "*"),
    ('\u{b8}', ","),
    ('\u{b9}', "^1"),
    ('\u{ba}', "o"),
    ('\u{bb}', ">>"),
    ('\u{bc}', "1/4"),
    ('\u{bd}', "1/2"),
    ('\u{be}', "3/4"),
    ('\u{bf}', "?"),
    ('\u{d7}', "x"),
    ('\u{f7}', "/"),
    // Greek capitals (`A` for Alpha and the others that look like a Latin letter).
    ('\u{391}', "A"),
    ('\u{392}', "B"),
    ('\u{393}', "Gamma"),
    ('\u{394}', "Delta"),
    ('\u{395}', "E"),
    ('\u{396}', "Z"),
    ('\u{397}', "H"),
    ('\u{398}', "Theta"),
    ('\u{399}', "I"),
    ('\u{39a}', "K"),
    ('\u{39b}', "Lambda"),
    ('\u{39c}', "M"),
    ('\u{39d}', "N"),
    ('\u{39e}', "Xi"),
    ('\u{39f}', "O"),
    ('\u{3a0}', "Pi"),
    ('\u{3a1}', "P"),
    ('\u{3a3}', "Sigma"),
    ('\u{3a4}', "T"),
    ('\u{3a5}', "Upsilon"),
    ('\u{3a6}', "Phi"),
    ('\u{3a7}', "X"),
    ('\u{3a8}', "Psi"),
    ('\u{3a9}', "Omega"),
    // Greek small letters and their variants.
    ('\u{3b1}', "alpha"),
    ('\u{3b2}', "beta"),
    ('\u{3b3}', "gamma"),
    ('\u{3b4}', "delta"),
    ('\u{3b5}', "epsilon"),
    ('\u{3b6}', "zeta"),
    ('\u{3b7}', "eta"),
    ('\u{3b8}', "theta"),
    ('\u{3b9}', "iota"),
    ('\u{3ba}', "kappa"),
    ('\u{3bb}', "lambda"),
    ('\u{3bc}', "mu"),
    ('\u{3bd}', "nu"),
    ('\u{3be}', "xi"),
    ('\u{3bf}', "o"),
    ('\u{3c0}', "pi"),
    ('\u{3c1}', "rho"),
    ('\u{3c2}', "sigma"),
    ('\u{3c3}', "sigma"),
    ('\u{3c4}', "tau"),
    ('\u{3c5}', "upsilon"),
    ('\u{3c6}', "phi"),
    ('\u{3c7}', "chi"),
    ('\u{3c8}', "psi"),
    ('\u{3c9}', "omega"),
    ('\u{3d1}', "theta"),
    ('\u{3d5}', "phi"),
    ('\u{3d6}', "pi"),
    ('\u{3dd}', "digamma"),
    ('\u{3f0}', "kappa"),
    ('\u{3f1}', "rho"),
    ('\u{3f5}', "epsilon"),
    // Modifier letters of accents.
    ('\u{2c7}', "v"),
    ('\u{2c9}', "-"),
    ('\u{2ca}', "'"),
    ('\u{2cb}', "`"),
    ('\u{2d8}', "u"),
    ('\u{2d9}', "."),
    ('\u{2da}', "o"),
    ('\u{2dc}', "~"),
    ('\u{2dd}', "\""),
    // Punctuation.
    ('\u{2010}', "-"),
    ('\u{2011}', "-"),
    ('\u{2013}', "-"),
    ('\u{2014}', "--"),
    ('\u{2018}', "'"),
    ('\u{2019}', "'"),
    ('\u{201c}', "\""),
    ('\u{201d}', "\""),
    ('\u{2020}', "+"),
    ('\u{2021}', "++"),
    ('\u{2022}', "*"),
    ('\u{2026}', "..."),
    ('\u{2032}', "'"),
    ('\u{2033}', "''"),
    ('\u{2034}', "'''"),
    ('\u{2035}', "`"),
    ('\u{2039}', "<"),
    ('\u{203a}', ">"),
    ('\u{2057}', "''''"),
    // Letterlike symbols.
    ('\u{2102}', "C"),
    ('\u{2107}', "e"),
    ('\u{210d}', "H"),
    ('\u{210f}', "hbar"),
    ('\u{2111}', "Im"),
    ('\u{2113}', "l"),
    ('\u{2115}', "N"),
    ('\u{2118}', "wp"),
    ('\u{2119}', "P"),
    ('\u{211a}', "Q"),
    ('\u{211c}', "Re"),
    ('\u{211d}', "R"),
    ('\u{2124}', "Z"),
    ('\u{2127}', "mho"),
    ('\u{2132}', "Finv"),
    ('\u{2135}', "aleph"),
    ('\u{2136}', "beth"),
    ('\u{2137}', "gimel"),
    ('\u{2138}', "daleth"),
    ('\u{2141}', "Game"),
    // Arrows.
    ('\u{2190}', "<-"),
    ('\u{2191}', "up"),
    ('\u{2192}', "->"),
    ('\u{2193}', "down"),
    ('\u{2194}', "<->"),
    ('\u{2195}', "updown"),
    ('\u{2196}', "nwarrow"),
    ('\u{2197}', "nearrow"),
    ('\u{2198}', "searrow"),
    ('\u{2199}', "swarrow"),
    ('\u{219a}', "!<-"),
    ('\u{219b}', "!->"),
    ('\u{219e}', "<<-"),
    ('\u{21a0}', "->>"),
    ('\u{21a2}', ">-"),
    ('\u{21a3}', "->"),
    ('\u{21a6}', "|->"),
    ('\u{21a9}', "<-"),
    ('\u{21aa}', "->"),
    ('\u{21ae}', "!<->"),
    ('\u{21bc}', "<-"),
    ('\u{21bd}', "<-"),
    ('\u{21c0}', "->"),
    ('\u{21c1}', "->"),
    ('\u{21cb}', "<=>"),
    ('\u{21cc}', "<=>"),
    ('\u{21cd}', "!<="),
    ('\u{21ce}', "!<=>"),
    ('\u{21cf}', "!=>"),
    ('\u{21d0}', "<="),
    ('\u{21d1}', "Up"),
    ('\u{21d2}', "=>"),
    ('\u{21d3}', "Down"),
    ('\u{21d4}', "<=>"),
    ('\u{21d5}', "Updown"),
    ('\u{21dd}', "~>"),
    // Mathematical operators.
    ('\u{2200}', "forall"),
    ('\u{2201}', "complement"),
    ('\u{2202}', "partial"),
    ('\u{2203}', "exists"),
    ('\u{2204}', "!exists"),
    ('\u{2205}', "empty"),
    ('\u{2207}', "nabla"),
    ('\u{2208}', "in"),
    ('\u{2209}', "!in"),
    ('\u{220b}', "ni"),
    ('\u{220c}', "!ni"),
    ('\u{220d}', "ni"),
    ('\u{220f}', "prod"),
    ('\u{2210}', "coprod"),
    ('\u{2211}', "sum"),
    ('\u{2212}', "-"),
    ('\u{2213}', "-/+"),
    ('\u{2214}', "+"),
    ('\u{2216}', "\\"),
    ('\u{2217}', "*"),
    ('\u{2218}', "o"),
    ('\u{2219}', "*"),
    ('\u{221a}', "sqrt"),
    ('\u{221d}', "propto"),
    ('\u{221e}', "inf"),
    ('\u{2220}', "angle"),
    ('\u{2221}', "angle"),
    ('\u{2222}', "angle"),
    ('\u{2223}', "|"),
    ('\u{2224}', "!|"),
    ('\u{2225}', "||"),
    ('\u{2226}', "!||"),
    ('\u{2227}', "and"),
    ('\u{2228}', "or"),
    ('\u{2229}', "cap"),
    ('\u{222a}', "cup"),
    ('\u{222b}', "int"),
    ('\u{222c}', "iint"),
    ('\u{222d}', "iiint"),
    ('\u{222e}', "oint"),
    ('\u{222f}', "oiint"),
    ('\u{2230}', "oiiint"),
    ('\u{2234}', "therefore"),
    ('\u{2235}', "because"),
    ('\u{223c}', "~"),
    ('\u{223d}', "~"),
    ('\u{2240}', "wr"),
    ('\u{2241}', "!~"),
    ('\u{2242}', "~"),
    ('\u{2243}', "~="),
    ('\u{2244}', "!~="),
    ('\u{2245}', "~="),
    ('\u{2246}', "!~="),
    ('\u{2247}', "!~="),
    ('\u{2248}', "~="),
    ('\u{2249}', "!~="),
    ('\u{224a}', "~="),
    ('\u{224d}', "~"),
    ('\u{224e}', "=="),
    ('\u{224f}', "=="),
    ('\u{2250}', "=."),
    ('\u{2251}', "=."),
    ('\u{2252}', "=."),
    ('\u{2253}', "=."),
    ('\u{2256}', "=o"),
    ('\u{2257}', "=o"),
    ('\u{225c}', "=^"),
    ('\u{2260}', "!="),
    ('\u{2261}', "=="),
    ('\u{2262}', "!=="),
    ('\u{2264}', "<="),
    ('\u{2265}', ">="),
    ('\u{2266}', "<="),
    ('\u{2267}', ">="),
    ('\u{2268}', "<"),
    ('\u{2269}', ">"),
    ('\u{226a}', "<<"),
    ('\u{226b}', ">>"),
    ('\u{226c}', "between"),
    ('\u{226e}', "!<"),
    ('\u{226f}', "!>"),
    ('\u{2270}', "!<="),
    ('\u{2271}', "!>="),
    ('\u{2272}', "<~"),
    ('\u{2273}', ">~"),
    ('\u{2276}', "<>"),
    ('\u{2277}', "><"),
    ('\u{227a}', "prec"),
    ('\u{227b}', "succ"),
    ('\u{227c}', "preceq"),
    ('\u{227d}', "succeq"),
    ('\u{227e}', "prec~"),
    ('\u{227f}', "succ~"),
    ('\u{2280}', "!prec"),
    ('\u{2281}', "!succ"),
    ('\u{2282}', "subset"),
    ('\u{2283}', "supset"),
    ('\u{2284}', "!subset"),
    ('\u{2285}', "!supset"),
    ('\u{2286}', "subseteq"),
    ('\u{2287}', "supseteq"),
    ('\u{2288}', "!subseteq"),
    ('\u{2289}', "!supseteq"),
    ('\u{228a}', "subsetneq"),
    ('\u{228b}', "supsetneq"),
    ('\u{228e}', "uplus"),
    ('\u{228f}', "sqsubset"),
    ('\u{2290}', "sqsupset"),
    ('\u{2291}', "sqsubseteq"),
    ('\u{2292}', "sqsupseteq"),
    ('\u{2293}', "sqcap"),
    ('\u{2294}', "sqcup"),
    ('\u{2295}', "(+)"),
    ('\u{2296}', "(-)"),
    ('\u{2297}', "(x)"),
    ('\u{2298}', "(/)"),
    ('\u{2299}', "(.)"),
    ('\u{229a}', "(o)"),
    ('\u{229b}', "(*)"),
    ('\u{229d}', "(-)"),
    ('\u{229e}', "[+]"),
    ('\u{229f}', "[-]"),
    ('\u{22a0}', "[x]"),
    ('\u{22a1}', "[.]"),
    ('\u{22a2}', "|-"),
    ('\u{22a3}', "-|"),
    ('\u{22a4}', "T"),
    ('\u{22a5}', "_|_"),
    ('\u{22a8}', "|="),
    ('\u{22a9}', "|-"),
    ('\u{22ac}', "!|-"),
    ('\u{22ad}', "!|="),
    ('\u{22b2}', "<|"),
    ('\u{22b3}', "|>"),
    ('\u{22b4}', "<|="),
    ('\u{22b5}', "|>="),
    ('\u{22ba}', "T"),
    ('\u{22bb}', "xor"),
    ('\u{22bc}', "nand"),
    ('\u{22c0}', "bigwedge"),
    ('\u{22c1}', "bigvee"),
    ('\u{22c2}', "bigcap"),
    ('\u{22c3}', "bigcup"),
    ('\u{22c4}', "<>"),
    ('\u{22c5}', "*"),
    ('\u{22c6}', "*"),
    ('\u{22c8}', "|X|"),
    ('\u{22c9}', "x|"),
    ('\u{22ca}', "|x"),
    ('\u{22ce}', "v"),
    ('\u{22cf}', "^"),
    ('\u{22d2}', "cap"),
    ('\u{22d3}', "cup"),
    ('\u{22d8}', "<<<"),
    ('\u{22d9}', ">>>"),
    ('\u{22ee}', ":"),
    ('\u{22ef}', "..."),
    ('\u{22f1}', "..."),
    // Technical and geometric symbols.
    ('\u{2308}', "|'"),
    ('\u{2309}', "'|"),
    ('\u{230a}', "|_"),
    ('\u{230b}', "_|"),
    ('\u{2322}', "^"),
    ('\u{2323}', "v"),
    ('\u{25a0}', "[#]"),
    ('\u{25a1}', "[]"),
    ('\u{25b2}', "^"),
    ('\u{25b3}', "/\\"),
    ('\u{25b6}', ">"),
    ('\u{25b9}', ">"),
    ('\u{25bc}', "v"),
    ('\u{25bd}', "\\/"),
    ('\u{25c0}', "<"),
    ('\u{25c3}', "<"),
    ('\u{25ca}', "<>"),
    ('\u{25ef}', "O"),
    ('\u{2605}', "*"),
    ('\u{2660}', "spades"),
    ('\u{2661}', "hearts"),
    ('\u{2662}', "diamonds"),
    ('\u{2663}', "clubs"),
    ('\u{266d}', "b"),
    ('\u{266e}', "n"),
    ('\u{266f}', "#"),
    ('\u{2713}', "v"),
    ('\u{2720}', "+"),
    // Delimiters and long arrows.
    ('\u{27e8}', "<"),
    ('\u{27e9}', ">"),
    ('\u{27ee}', "("),
    ('\u{27ef}', ")"),
    ('\u{27f5}', "<--"),
    ('\u{27f6}', "-->"),
    ('\u{27f7}', "<-->"),
    ('\u{27f8}', "<=="),
    ('\u{27f9}', "==>"),
    ('\u{27fa}', "<==>"),
    ('\u{27fc}', "|-->"),
    // Large operators, supplemental.
    ('\u{29eb}', "<>"),
    ('\u{2a00}', "bigodot"),
    ('\u{2a01}', "bigoplus"),
    ('\u{2a02}', "bigotimes"),
    ('\u{2a04}', "biguplus"),
    ('\u{2a06}', "bigsqcup"),
    ('\u{2a3f}', "coprod"),
    ('\u{2a5e}', "barwedge"),
    ('\u{2a7d}', "<="),
    ('\u{2a7e}', ">="),
    ('\u{2a85}', "<~"),
    ('\u{2a86}', ">~"),
    ('\u{2a87}', "<"),
    ('\u{2a88}', ">"),
    ('\u{2aaf}', "preceq"),
    ('\u{2ab0}', "succeq"),
    ('\u{2ac5}', "subseteq"),
    ('\u{2ac6}', "supseteq"),
    // The `\not` slash, which the renderer draws before the symbol it strikes.
    ('\u{e020}', "/"),
];
