//! The commands the parser reads as functions: how many arguments each takes and where it may
//! appear (LMATH-2, step 3).
//!
//! The table is KaTeX's registry, generated into `functions_data.rs` by
//! `scripts/katex-functions.mjs`. It says what a name is, not what it does: the parser builds a
//! node only for the functions it has implemented, and reports the rest as not supported yet.

use super::{expander::Key, functions_data::FUNCTIONS};

/// A row of the generated table: name, node type, argument counts, and where it is allowed.
pub(super) type Row = (
    &'static str,
    &'static str,
    u8,
    u8,
    bool,
    bool,
    bool,
    bool,
    bool,
    &'static str,
);

/// What the registry says about a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Function {
    /// The name of the node KaTeX builds (`genfrac`, `sqrt`, ...).
    pub kind: &'static str,
    /// Arguments in braces, after any optional ones.
    pub num_args: usize,
    /// Optional arguments in brackets.
    pub num_optional_args: usize,
    pub allowed_in_text: bool,
    pub allowed_in_math: bool,
    /// Whether it may stand alone as the argument of another function.
    pub allowed_in_argument: bool,
    /// A primitive's arguments are read in its own mode, not a nested one.
    pub primitive: bool,
    /// An infix command (`\over`) splits the expression around it.
    pub infix: bool,
    /// How each argument is read, optional ones first: `o` original (a group in the current
    /// mode), `m` math, `t` text, `p` primitive (one group or symbol), `c` color, `s` size, `u`
    /// url, `h` hbox, `r` raw.
    pub arg_types: &'static str,
}

/// The function written `name`, with its backslash for a control sequence (`\frac`).
pub(crate) fn lookup(name: &str) -> Option<Function> {
    let at = FUNCTIONS.binary_search_by(|row| row.0.cmp(name)).ok()?;
    let (_, kind, args, optional, text, math, argument, primitive, infix, arg_types) =
        FUNCTIONS[at];
    Some(Function {
        kind,
        num_args: usize::from(args),
        num_optional_args: usize::from(optional),
        allowed_in_text: text,
        allowed_in_math: math,
        allowed_in_argument: argument,
        primitive,
        infix,
        arg_types,
    })
}

/// Whether `key`, a command or active character, is a function.
pub(crate) fn is_function(key: &Key<'_>) -> bool {
    let name = match key {
        Key::Word(word) => format!("\\{word}"),
        Key::Symbol(c) => format!("\\{c}"),
        Key::Char(text) => (*text).to_string(),
    };
    lookup(&name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_with_unique_names() {
        assert!(FUNCTIONS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn the_table_has_the_size_of_katex_s() {
        // As generated from KaTeX 0.19.1; a change here is a regeneration.
        assert_eq!(FUNCTIONS.len(), 312);
    }

    #[test]
    fn lookups() {
        let frac = lookup("\\frac").unwrap();
        assert_eq!((frac.kind, frac.num_args), ("genfrac", 2));
        let sqrt = lookup("\\sqrt").unwrap();
        assert_eq!((sqrt.num_args, sqrt.num_optional_args), (1, 1));
        assert!(lookup("\\over").unwrap().infix);
        assert!(lookup("\\text").unwrap().allowed_in_text);
        // The bare characters of the operators, and `$`, are functions too.
        assert!(lookup("\u{2211}").is_some());
        assert!(lookup("$").is_some());
        assert_eq!(lookup("\\alpha"), None);
        assert_eq!(lookup("frac"), None);
    }

    #[test]
    fn keys_are_looked_up_as_written() {
        assert!(is_function(&Key::Word("frac")));
        assert!(is_function(&Key::Char("$")));
        assert!(!is_function(&Key::Word("alpha")));
    }
}
