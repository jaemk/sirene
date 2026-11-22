#![no_main]

use libfuzzer_sys::fuzz_target;
use sirene_core::{Diagnostic, diagram};

#[path = "../../crates/sirene-core/tests/common/parser_registry.rs"]
mod parser_registry;

// A selector chooses a parser and supplies its header, so mutations reach grammar rules rather
// than spending most executions failing at the header. The remaining bytes are arbitrary UTF-8
// (invalid sequences become U+FFFD). Keep the bound small enough for routine fuzz campaigns.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 4096 {
        return;
    }
    let body = String::from_utf8_lossy(&data[1..]);
    let parser = &parser_registry::PARSERS[usize::from(data[0]) % parser_registry::PARSERS.len()];
    let src = format!("{}\n{body}", parser.header);
    let (tree, diagnostics) = (parser.parse)(&src);
    check(
        parser.name,
        &src,
        tree.to_string(),
        &diagnostics,
        parser.normalize_cr,
    );
    let lowered = diagram::lower(&src);
    check_ranges(&src, &lowered.diagnostics);
});

fn check(name: &str, src: &str, actual: String, diagnostics: &[Diagnostic], normalize_cr: bool) {
    let expected = if normalize_cr {
        normalize(src)
    } else {
        src.to_owned()
    };
    assert_eq!(actual, expected, "{name}");
    check_ranges(src, diagnostics);
}

fn normalize(src: &str) -> String {
    let mut chars = src.chars().peekable();
    let mut out = String::with_capacity(src.len());
    while let Some(c) = chars.next() {
        out.push(if c == '\r' && chars.peek() != Some(&'\n') {
            '\n'
        } else {
            c
        });
    }
    out
}

fn check_ranges(src: &str, diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        let start = u32::from(d.range.start()) as usize;
        let end = u32::from(d.range.end()) as usize;
        assert!(
            start <= end
                && end <= src.len()
                && src.is_char_boundary(start)
                && src.is_char_boundary(end),
            "{d:?}"
        );
    }
}
