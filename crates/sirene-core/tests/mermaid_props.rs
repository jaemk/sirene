//! Property test for the Mermaid parsers: any text after any header lowers and draws without
//! panicking, and every diagnostic lies within the source.

use proptest::prelude::*;
use sirene_core::TextSize;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram;
use sirene_core::scene::{Approx, Theme};

#[rustfmt::skip]
const HEADERS: &[&str] = &[
    "flowchart TD", "graph LR", "sequenceDiagram", "classDiagram", "stateDiagram-v2",
    "erDiagram", "gantt", "pie", "journey", "gitGraph", "mindmap", "timeline", "quadrantChart",
    "requirementDiagram", "C4Context", "sankey-beta", "xychart-beta", "block-beta", "packet",
    "architecture-beta", "kanban", "radar-beta", "treemap-beta", "zenuml", "venn-beta",
    "treeView-beta", "usecase-beta", "railroad-beta", "wardley-beta", "ishikawa-beta",
    "cynefin-beta", "eventmodeling",
];

/// Pieces biased toward Mermaid syntax, with non-ASCII text where keywords may follow.
fn body() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    let pieces = prop::sample::select(vec![
        "\n", "\r\n", " ", " ", "\t", "  ", "A", "B", "x1", "é", "日本", "🐟", "\"", "\"q\"", ":",
        ";", ",", "[", "]", "(", ")", "{", "}", "<", ">", "<<", ">>", "[[", "]]", "|", "-", "--",
        "-->", "->>", "-x", "--o", "..", "==", "o{", "}|", "|o", "<|--", "*--", "%%", "%% c",
        "%%{", "}%%", ":::", "@{", "#", "+", "1", "-2", "3.5", "100%", "as ", "of ", "left ",
        "right ", "over ", "note ", "end", "state ", "class ", "classDef ", "style ", "direction ",
        "subgraph ", "participant ", "actor ", "loop ", "alt ", "else", "box ", "activate ",
        "autonumber", "section ", "title ", "dateFormat ", "accTitle: ", "accDescr {", "<br>",
        "service ", "group ", "in ", "commit", "branch ", "checkout ", "merge ", "x-axis ",
        "y-axis ", "bar ", "line ", "columns ", "space", "axis ", "curve ", "set ", "union ",
    ]);
    prop::collection::vec(pieces, 0..40).prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    #[test]
    fn any_text_lowers_and_draws(header in prop::sample::select(HEADERS), body in body()) {
        let src = format!("{header}\n{body}");
        let lowered = diagram::lower(&src);
        for d in &lowered.diagnostics {
            prop_assert!(d.range.end() <= TextSize::of(src.as_str()), "{d:?}");
        }
        for charset in [Charset::Unicode, Charset::Ascii] {
            lowered.diagram.ascii(&Options { charset });
        }
        lowered.diagram.scene(&Approx, &Theme::LIGHT);
    }
}
