//! Use case metadata scanning remains bounded for repeated unclosed openers (LANGS-8).

use sirene_core::{syntax::SyntaxKind, usecase};

#[test]
fn unclosed_metadata_recovers_to_later_statement() {
    let count = 20_000;
    let src = format!(
        "usecase-beta\n{}actor Later\n",
        "actor A@{ bad\n".repeat(count)
    );
    let parsed = usecase::parse(&src);
    assert_eq!(parsed.syntax().to_string(), src);
    assert_eq!(parsed.diagnostics().len(), count);
    assert!(
        parsed
            .diagnostics()
            .iter()
            .all(|d| d.message == "unclosed `@{`: expected `}`")
    );
    assert!(parsed.syntax().descendants_with_tokens().any(|e| {
        e.into_token()
            .is_some_and(|t| t.kind() == SyntaxKind::IDENT && t.text() == "Later")
    }));
}

#[test]
fn metadata_closers_respect_quotes_and_multiple_searches() {
    let src = "usecase-beta\nactor A@{ color: \"café }\" }\nactor B@{ type: hollow }\nactor C@{ text: \"unclosed\nactor D@{ text: \"other\nactor Later\n";
    let parsed = usecase::parse(src);
    assert_eq!(parsed.syntax().to_string(), src);
    let bodies: Vec<_> = parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == SyntaxKind::META_BODY)
        .map(|t| t.text().to_owned())
        .collect();
    assert_eq!(bodies, [" color: \"café }\" ", " type: hollow "]);
    assert_eq!(parsed.diagnostics().len(), 2);
    assert!(
        parsed
            .diagnostics()
            .iter()
            .all(|d| d.message == "unclosed `@{`: expected `}`")
    );
}
