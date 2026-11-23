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
