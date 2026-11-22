//! Shared lossless-tree and recovery contracts for representative parsers (LANGS-10).

use proptest::prelude::*;
use sirene_core::Diagnostic;
use sirene_core::diagram::{self, Diagram, Kind, Lang};
use sirene_core::railroad::Notation;
use sirene_core::syntax::{SyntaxKind, SyntaxNode};
use sirene_core::theme::ThemeConfig;

#[path = "common/parser_registry.rs"]
mod parser_registry;
use parser_registry::{PARSERS, check_ranges};

#[test]
fn fuzz_selector_mapping_is_stable_and_unique() {
    let expected = [
        ("flowchart", "flowchart TD", true),
        ("architecture", "architecture-beta", false),
        ("block", "block-beta", false),
        ("c4", "C4Context", false),
        ("class", "classDiagram", false),
        ("cynefin", "cynefin-beta", false),
        ("dot", "digraph {", false),
        ("er", "erDiagram", false),
        ("eventmodel", "eventmodeling", false),
        ("gantt", "gantt", false),
        ("git", "gitGraph", false),
        ("info", "info", false),
        ("ishikawa", "ishikawa-beta", false),
        ("journey", "journey", false),
        ("kanban", "kanban", false),
        ("mindmap", "mindmap", false),
        ("packet", "packet-beta", false),
        ("pie", "pie", false),
        ("plantuml", "@startuml", true),
        ("quadrant", "quadrantChart", false),
        ("radar", "radar-beta", false),
        ("railroad", "railroad-beta", false),
        ("requirement", "requirementDiagram", false),
        ("sankey", "sankey-beta", false),
        ("sequence", "sequenceDiagram", false),
        ("state", "stateDiagram-v2", false),
        ("timeline", "timeline", false),
        ("treemap", "treemap-beta", false),
        ("treeview", "treeView-beta", false),
        ("usecase", "usecase-beta", false),
        ("venn", "venn-beta", false),
        ("wardley", "wardley-beta", false),
        ("xychart", "xychart-beta", false),
        ("zenuml", "zenuml", false),
    ];
    assert_eq!(PARSERS.len(), expected.len());
    let mut names = std::collections::HashSet::new();
    let mut headers = std::collections::HashSet::new();
    for (parser, &(name, header, normalizes)) in PARSERS.iter().zip(&expected) {
        assert_eq!(
            (parser.name, parser.header, parser.normalize_cr),
            (name, header, normalizes)
        );
        assert!(names.insert(parser.name));
        assert!(headers.insert(parser.header));
        assert_eq!(Kind::of(parser.header), parser.kind);
    }
}

fn check_contract(src: &str, tree: &SyntaxNode, diagnostics: &[Diagnostic]) {
    assert_eq!(tree.to_string(), src);
    check_ranges(src, diagnostics);
}

#[test]
fn carriage_returns_follow_each_parsers_contract() {
    for parser in PARSERS {
        let src = format!("{}\rcafé\r\nnext\r", parser.header);
        let (tree, diagnostics) = (parser.parse)(&src);
        check_contract(&parser.syntax_source(&src), &tree, &diagnostics);
    }
}

#[test]
fn valid_sources_and_recovery() {
    let cases = [
        (
            "pie\n\"café\": 2\ntitle Later\n",
            "pie\n\"café\": nope\ntitle Later\n",
            "expected a number",
        ),
        (
            "journey\ncafé: 2: Me\ntitle Later\n",
            "journey\ncafé\ntitle Later\n",
            "expected `:` and a score after the task name",
        ),
        (
            "timeline TD\ncafé : Event\ntitle Later\n",
            "timeline XY\ncafé : Event\ntitle Later\n",
            "expected the end of the line",
        ),
        (
            "xychart-beta\nbar [1, 2]\ntitle Later\n",
            "xychart-beta\nbar [1, café]\ntitle Later\n",
            "expected a number",
        ),
    ];
    for (name, (valid, malformed, message)) in ["pie", "journey", "timeline", "xychart"]
        .into_iter()
        .zip(cases)
    {
        let parse = PARSERS.iter().find(|p| p.name == name).unwrap().parse;
        let (tree, diagnostics) = parse(valid);
        check_contract(valid, &tree, &diagnostics);
        assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
        let (tree, diagnostics) = parse(malformed);
        check_contract(malformed, &tree, &diagnostics);
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>(),
            [message],
            "{name}"
        );
        let later = malformed.find("title Later").unwrap();
        assert!(
            tree.descendants()
                .any(|node| node.kind() == SyntaxKind::TITLE_STMT
                    && u32::from(node.text_range().start()) as usize == later),
            "{name}: {tree:#?}"
        );
    }
}

fn body_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            4 => prop::sample::select("abc01 ;\n\r\t->{}[]()@%:/\"'".chars().collect::<Vec<_>>()),
            1 => any::<char>(),
        ],
        0..80,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn arbitrary_unicode_text_is_lossless_and_diagnostics_are_in_bounds(
        body in body_text(),
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
    ) {
        for parser in PARSERS {
            let src = format!("{}{nl}{body}", parser.header);
            let (tree, diagnostics) = (parser.parse)(&src);
            check_contract(&parser.syntax_source(&src), &tree, &diagnostics);
            let lowered = diagram::lower(&src);
            prop_assert_eq!(lowered.kind, parser.kind, "{}", parser.name);
            check_ranges(&src, &lowered.diagnostics);
        }
    }

    #[test]
    fn forced_languages_dispatch_and_keep_ordered_diagnostics(body in body_text()) {
        for lang in [Lang::Mermaid, Lang::Dot, Lang::PlantUml] {
            let lowered = diagram::lower_as(&body, Some(lang));
            prop_assert_eq!(lowered.kind, Kind::of_lang(&body, Some(lang)));
            prop_assert_eq!(lowered.kind.lang(), lang);
            prop_assert!(lowered.diagnostics.len() <= diagram::MAX_DIAGNOSTICS + 1);
            prop_assert!(lowered.diagnostics.windows(2).all(|d| d[0].range.start() <= d[1].range.start()));
            check_ranges(&body, &lowered.diagnostics);
            if lang == Lang::Mermaid {
                prop_assert_eq!(ThemeConfig::read(&body), lowered.theme);
            }
        }
    }

    #[test]
    fn plantuml_multiline_description_warnings_point_to_original_text(
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        indent in 0_usize..12,
        label in "[a-z]{1,12}",
        token in "[ࠀ-ࠫ𐀀-𐀝]{1,12}",
    ) {
        let padding = " ".repeat(indent);
        let src = format!("@startuml{nl}component \"{label}{nl}{padding}second line\" {token}{nl}@enduml");
        let lowered = diagram::lower(&src);
        prop_assert_eq!(lowered.diagnostics.len(), 1, "{:?}", lowered.diagnostics);
        let range = lowered.diagnostics[0].range;
        let at = src.find(&token).unwrap();
        prop_assert_eq!(usize::from(range.start()), at);
        prop_assert_eq!(usize::from(range.end()), at + token.len());
    }

    #[test]
    fn railroad_recovers_before_the_header_and_uses_its_notation(
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        name in "[a-z]{1,12}",
        terminal in "[a-z]{1,12}",
    ) {
        for (header, notation, definition) in [
            ("railroad-beta", Notation::Ir, format!("= terminal(\"{terminal}\")")),
            ("railroad-ebnf-beta", Notation::Ebnf, format!("= '{terminal}'")),
            ("railroad-abnf-beta", Notation::Abnf, format!("= \"{terminal}\"")),
            ("railroad-peg-beta", Notation::Peg, format!("<- '{terminal}'")),
        ] {
            let src = format!("%%{{unfinished{nl}{header}{nl}{name} {definition} ;{nl}");
            let lowered = diagram::lower(&src);
            prop_assert_eq!(lowered.diagnostics.len(), 1, "{:?}", lowered.diagnostics);
            prop_assert_eq!(&lowered.diagnostics[0].message, "unclosed directive: expected `}%%`");
            let Diagram::Railroad(chart) = lowered.diagram else {
                return Err(proptest::test_runner::TestCaseError::fail("expected a railroad diagram"));
            };
            prop_assert_eq!(chart.notation, notation);
            prop_assert_eq!(chart.rules.len(), 1);
            prop_assert_eq!(&chart.rules[0].name, &name);
        }
    }

    #[test]
    fn indented_frontmatter_content_cannot_change_dispatch(
        indent in 1_usize..9,
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        bom in prop::sample::select(vec!["", "\u{feff}"]),
        title in "[a-z ]{1,20}",
        theme in prop::sample::select(vec!["dark", "forest", "default"]),
        label in "[a-z]{1,10}",
        value in 1_u16..100,
    ) {
        let spaces = " ".repeat(indent);
        let src = format!(
            "{bom}---{nl}title: |{nl}{spaces}{title}{nl}{spaces}---{nl}{spaces}@startuml{nl}config:{nl}  theme: {theme}{nl}---{nl}pie{nl}\"{label}\": {value}{nl}"
        );
        prop_assert_eq!(Kind::of(&src), Kind::Pie);
        let lowered = diagram::lower(&src);
        prop_assert_eq!(lowered.kind, Kind::Pie);
        prop_assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        prop_assert_eq!(ThemeConfig::read(&src), lowered.theme.clone());
        let (name, range) = lowered.theme.name.unwrap();
        prop_assert_eq!(name.as_str(), theme);
        prop_assert_eq!(src.get(range), Some(theme));
        let Diagram::Pie(pie) = lowered.diagram else {
            return Err(proptest::test_runner::TestCaseError::fail("expected a pie chart"));
        };
        prop_assert_eq!(pie.slices.len(), 1);
        prop_assert_eq!(&pie.slices[0].label, &label);
    }

    #[test]
    fn theme_directives_commit_only_complete_settings(
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        key in "[a-zA-Z]{1,10}",
        first in "[a-z]{1,10}",
        last in "[a-z]{1,10}",
    ) {
        let malformed = r#"%%{init: {theme: 'neutral', themeVariables: {leak: red}, invalid: [1 2]}}%%"#;
        let valid = format!(
            r#"%%{{"initialize": {{theme: "f\u006frest", themeVariables: {{{key}: "{last}"}}, ignored: [{{value: "}}%%"}}]}}}}%%"#,
        );
        let src = [
            "---", "config:", "  theme: dark", "  themeVariables:",
            &format!("    {key}: '{first}'"), "---", "flowchart TD", malformed, &valid, "A",
        ].join(nl);
        let config = ThemeConfig::read(&src);
        prop_assert_eq!(&config, &diagram::lower(&src).theme);
        let (name, range) = config.name.unwrap();
        prop_assert_eq!(name, "forest");
        prop_assert_eq!(src.get(range), Some(r"f\u006frest"));
        prop_assert_eq!(config.variables, vec![(key.clone(), first), (key, last)]);
        prop_assert_eq!(config.unread.len(), 1);
        prop_assert_eq!(src.get(config.unread[0].clone()), Some(malformed));
    }
}
