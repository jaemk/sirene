//! Structured inputs around parser nesting limits (LANGS-11).
//!
//! Deep-input stress tests live in `limits.rs`; these vary the boundary itself and check that
//! skipping an over-deep block does not consume a following statement.

use std::fmt::Write as _;

use proptest::prelude::*;
use sirene_core::syntax::{SyntaxKind, SyntaxNode};
use sirene_core::{Diagnostic, block, flowchart, state};

type Parse = fn(&str) -> (SyntaxNode, Vec<Diagnostic>);

fn check(src: &str, parse: Parse, limit: usize, depth: usize, message: &str) {
    let (tree, diagnostics) = parse(src);
    assert_eq!(tree.to_string(), src);
    let end = src.find("after").unwrap();
    let mut matching = diagnostics.iter().filter(|d| d.message == message);
    if depth > limit {
        let diagnostic = matching.next().expect("missing nesting diagnostic");
        assert!(u32::from(diagnostic.range.start()) as usize <= end);
        assert!(
            matching.next().is_none(),
            "nesting limit was reported twice"
        );
    } else {
        assert!(matching.next().is_none(), "unexpected nesting diagnostic");
    }
    assert!(
        tree.descendants_with_tokens()
            .any(|item| item.into_token().is_some_and(|t| {
                t.kind() == SyntaxKind::IDENT
                    && t.text() == "after"
                    && u32::from(t.text_range().start()) as usize == end
            })),
        "parser did not recover to the following statement: {diagnostics:?}"
    );
    for d in diagnostics {
        let start = u32::from(d.range.start()) as usize;
        let end = u32::from(d.range.end()) as usize;
        assert!(start <= end && end <= src.len());
        assert!(src.is_char_boundary(start) && src.is_char_boundary(end));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn nested_blocks_preserve_following_statements(
        depth_offset in -3_i32..=5,
        comments in any::<bool>(),
        crlf in any::<bool>(),
    ) {
        let depth = (64 + depth_offset) as usize;
        let newline = if crlf { "\r\n" } else { "\n" };
        for (header, opener, closer, trailing, limit, actual_depth, message, parse) in [
            ("flowchart TD", "subgraph s", "end", "after --> done", 64, depth,
                "subgraphs nested more than 64 deep", (|s: &str| { let p = flowchart::parse(s); (p.syntax(), p.diagnostics().to_vec()) }) as Parse),
            ("stateDiagram-v2", "state s", "}", "after --> done", 64, depth,
                "composite states nested more than 64 deep", (|s: &str| { let p = state::parse(s); (p.syntax(), p.diagnostics().to_vec()) }) as Parse),
            ("block-beta", "block:b", "end", "after", 8, (8 + depth_offset) as usize,
                "blocks nested more than 8 deep", (|s: &str| { let p = block::parse(s); (p.syntax(), p.diagnostics().to_vec()) }) as Parse),
        ] {
            let mut src = format!("{header}{newline}");
            for i in 0..actual_depth {
                let _ = write!(src, "{opener}{i}{}{newline}", if header == "stateDiagram-v2" { " {" } else { "" });
            }
            if comments { let _ = write!(src, "%% end }} café{newline}"); }
            let _ = write!(src, "a{newline}");
            for _ in 0..actual_depth { let _ = write!(src, "{closer}{newline}"); }
            let _ = write!(src, "{trailing}{newline}");
            check(&src, parse, limit, actual_depth, message);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn metadata_budget_reports_once_and_keeps_following_nodes(
        count in 253_usize..=260,
        crlf in any::<bool>(),
    ) {
        let newline = if crlf { "\r\n" } else { "\n" };
        let keys = (0..count).map(|i| format!("k{i}: v")).collect::<Vec<_>>().join(", ");
        let src = format!("flowchart LR{newline}a@{{ {keys} }}{newline}b@{{ {keys} }}{newline}after --> done{newline}");
        let parsed = flowchart::parse(&src);
        prop_assert_eq!(parsed.syntax().to_string(), src.as_str());
        let result = parsed.lower();
        let warnings: Vec<_> = result.diagnostics.iter()
            .filter(|d| d.message == "an element has more than 256 metadata keys; leaving out the rest")
            .collect();
        prop_assert_eq!(warnings.len(), usize::from(count > 256));
        prop_assert!(result.flowchart.nodes.iter().any(|node| node.id == "after"));
        for d in &result.diagnostics {
            let start = u32::from(d.range.start()) as usize;
            let end = u32::from(d.range.end()) as usize;
            prop_assert!(start <= end && end <= src.len());
            prop_assert!(src.is_char_boundary(start) && src.is_char_boundary(end));
        }
    }
}
