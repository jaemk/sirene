//! The baseline of KaTeX's own answers (LMATH-13) as the stages come to exist. The data is
//! `tests/fixtures/math/katex-oracle.json`, written by `scripts/katex-fixtures.mjs` from KaTeX's
//! test suite (MIT, Khan Academy and other contributors; see THIRD-PARTY-NOTICES).

use serde_json::Value;

use super::lexer::{Lexer, TokenKind};

/// KaTeX's answer for each distinct input: the input, and the oracle entry for it.
fn oracle() -> Vec<(String, Value)> {
    let data: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("../../tests/fixtures/math/katex-oracle.json")).unwrap();
    data.into_iter()
        .map(|(key, entry)| {
            let (input, _display, _macros): (String, bool, Value) =
                serde_json::from_str(&key).unwrap();
            (input, entry)
        })
        .collect()
}

/// The symbol leaves of a KaTeX parse tree: `(mode, text, group)` for each node that is one
/// symbol of the table, where `group` is KaTeX's name for its atom group.
fn symbol_leaves(node: &Value, out: &mut Vec<(String, String, String)>) {
    match node {
        Value::Array(items) => items.iter().for_each(|n| symbol_leaves(n, out)),
        Value::Object(fields) => {
            let kind = fields.get("type").and_then(Value::as_str);
            let mode = fields.get("mode").and_then(Value::as_str);
            let text = fields.get("text").and_then(Value::as_str);
            if let (Some(kind), Some(mode), Some(text)) = (kind, mode, text) {
                let group = match kind {
                    "atom" => fields.get("family").and_then(Value::as_str),
                    "mathord" | "textord" | "spacing" => Some(kind),
                    _ => None,
                };
                if let Some(group) = group {
                    out.push((mode.into(), text.into(), group.into()));
                }
            }
            fields.values().for_each(|n| symbol_leaves(n, out));
        }
        _ => {}
    }
}

/// Every symbol KaTeX's parser read in its own test suite is in the table, with the same atom
/// group. The one exception is a `textord` KaTeX's parser made without the table: a character of
/// a script it supports (Cyrillic, CJK, Arabic digits), a character from `\char`, or a
/// backslash in `\href` text. Those belong to the parser stage, and are counted here so that
/// the count shows when the table or the parser changes.
#[test]
fn the_symbol_table_agrees_with_katex_on_every_baseline_symbol() {
    use super::symbols::{AtomGroup, Mode, Name, lookup};

    let mut checked = 0;
    let mut parser_textords = 0;
    let mut disagreements = Vec::new();
    for (input, entry) in oracle() {
        let Some(tree) = entry.get("tree") else {
            continue;
        };
        let mut leaves = Vec::new();
        symbol_leaves(tree, &mut leaves);
        for (mode, text, group) in leaves {
            let mode = if mode == "math" {
                Mode::Math
            } else {
                Mode::Text
            };
            let name = match text.strip_prefix('\\') {
                Some(control) => Name::Control(control),
                None => Name::Char(&text),
            };
            let expected = match group.as_str() {
                "accent-token" => AtomGroup::Accent,
                "bin" => AtomGroup::Bin,
                "close" => AtomGroup::Close,
                "inner" => AtomGroup::Inner,
                "mathord" => AtomGroup::MathOrd,
                "op-token" => AtomGroup::Op,
                "open" => AtomGroup::Open,
                "punct" => AtomGroup::Punct,
                "rel" => AtomGroup::Rel,
                "spacing" => AtomGroup::Spacing,
                "textord" => AtomGroup::TextOrd,
                other => panic!("unknown group {other} in {input:?}"),
            };
            checked += 1;
            match lookup(mode, name) {
                Some(symbol) if symbol.group == expected => {}
                _ if expected == AtomGroup::TextOrd => parser_textords += 1,
                found => disagreements.push(format!("{input:?}: {text:?} {found:?} vs {group}")),
            }
        }
    }
    assert!(checked > 4000, "{checked}");
    assert_eq!(
        parser_textords, 87,
        "textords the parser makes without the table"
    );
    assert!(
        disagreements.is_empty(),
        "{} of {checked}:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}

/// The macro expander reads every input KaTeX parses, to its end. Without a parser the expander
/// sees no groups or modes, so it can only fail where it lacks a macro, or where an input uses a
/// definition's scope or a mode; those are the failures listed here, and the list shows what the
/// parser stage and the remaining macros must cover.
#[test]
fn the_expander_reads_what_katex_parses() {
    use super::expander::Expander;
    use super::symbols::Mode;

    let mut failures = Vec::new();
    let mut read = 0;
    for (input, entry) in oracle() {
        if entry.get("tree").is_none() {
            continue;
        }
        let mut expander = Expander::new(&input, Mode::Math).unwrap();
        let outcome = loop {
            match expander.expand_next_token() {
                Err(e) => break Err(e.message),
                Ok(token) if token.kind == TokenKind::Eof => break Ok(()),
                Ok(_) => {}
            }
        };
        match outcome {
            Ok(()) => read += 1,
            Err(message) => failures.push(format!("{message}  <-  {input:?}")),
        }
    }
    // Every failure is a macro that is not implemented yet. This count falls as they are.
    assert!(
        failures.iter().all(|f| f.contains("is not supported yet")),
        "{}",
        failures.join("\n")
    );
    assert_eq!((read, failures.len()), (436, 13));
}

/// The errors KaTeX's expander raises are the expander's errors too: for each input KaTeX
/// rejects with one of these messages, reading the input to its end produces the same message.
#[test]
fn the_expander_fails_where_katex_fails_in_expansion() {
    use super::expander::Expander;
    use super::symbols::Mode;

    // "Unexpected end of input in a macro argument" is not here: KaTeX raises it for a function's
    // missing argument (`\frac 1`), which the parser reads, so the parser stage checks it.
    let expansion_errors = [
        "Extra }",
        "Use of the macro doesn't match its definition",
        "Too many expansions",
        "Expected a control sequence",
        "Expected a macro definition",
        "Invalid number of arguments",
        "Invalid argument number",
        "Argument number",
        "Incomplete placeholder at end of macro body",
        "Not a valid argument number",
        "\\newcommand",
        "\\renewcommand",
    ];
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for (input, entry) in oracle() {
        let Some(message) = entry.get("parse_error").and_then(Value::as_str) else {
            continue;
        };
        if !expansion_errors.iter().any(|e| message.starts_with(e)) {
            continue;
        }
        checked += 1;
        let mut expander = Expander::new(&input, Mode::Math).unwrap();
        let ours = loop {
            match expander.expand_next_token() {
                Err(e) => break Some(e.message),
                Ok(token) if token.kind == TokenKind::Eof => break None,
                Ok(_) => {}
            }
        };
        // The limit's wording differs on purpose (KaTeX names its `maxExpand` setting).
        let same = ours.as_deref().is_some_and(|o| {
            o == message
                || (message.starts_with("Too many expansions")
                    && o.starts_with("Too many expansions"))
        });
        if !same {
            mismatches.push(format!("KaTeX: {message:?}, here: {ours:?}  <-  {input:?}"));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert_eq!(checked, 22);
}

#[test]
fn the_oracle_is_present() {
    let oracle = oracle();
    assert!(oracle.len() > 600, "{}", oracle.len());
    assert!(oracle.iter().any(|(_, e)| e.get("tree").is_some()));
    assert!(oracle.iter().any(|(_, e)| e.get("parse_error").is_some()));
}

/// The lexer agrees with KaTeX's on every input of its test suite: where KaTeX's own lexer
/// rejects a character, this one does too, and otherwise it reads the whole input, in order.
/// (No input of the suite has a character KaTeX's lexer rejects, so rejection is covered by the
/// lexer's own tests.)
#[test]
fn the_lexer_agrees_with_katex_on_every_baseline_input() {
    for (input, entry) in oracle() {
        let katex_rejects = entry
            .get("parse_error")
            .and_then(Value::as_str)
            .is_some_and(|m| m.starts_with("Unexpected character"));
        let mut lexer = Lexer::new(&input).unwrap();
        let mut end = 0;
        let outcome = loop {
            match lexer.next_token() {
                Err(e) => break Err(e),
                Ok(token) => {
                    assert!(token.span.start >= end, "{input:?}: {token:?}");
                    assert!(input.get(token.span.clone()).is_some(), "{input:?}");
                    end = token.span.end;
                    if token.kind == TokenKind::Eof {
                        break Ok(());
                    }
                }
            }
        };
        match (katex_rejects, outcome) {
            (true, Err(_)) | (false, Ok(())) => {}
            (true, Ok(())) => panic!("KaTeX rejects a character in {input:?}, the lexer does not"),
            (false, Err(e)) => panic!("the lexer rejects {input:?} ({e}), KaTeX does not"),
        }
    }
}
