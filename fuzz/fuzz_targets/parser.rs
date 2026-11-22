#![no_main]

use libfuzzer_sys::fuzz_target;
use sirene_core::diagram;

#[path = "../../crates/sirene-core/tests/common/parser_registry.rs"]
mod parser_registry;
use parser_registry::{PARSERS, check_ranges};

// Preserve the selector and header so mutations reach the selected grammar.
fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    if data.len() > 4096 {
        return;
    }
    let parser = &PARSERS[usize::from(selector) % PARSERS.len()];
    let src = format!("{}\n{}", parser.header, String::from_utf8_lossy(body));
    let (tree, diagnostics) = (parser.parse)(&src);
    assert_eq!(
        tree.to_string(),
        parser.syntax_source(&src),
        "{}",
        parser.name
    );
    check_ranges(&src, &diagnostics);

    let lowered = diagram::lower(&src);
    assert_eq!(lowered.kind, parser.kind);
    check_ranges(&src, &lowered.diagnostics);
});
