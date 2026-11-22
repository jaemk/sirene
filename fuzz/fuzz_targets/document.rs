#![no_main]

use libfuzzer_sys::fuzz_target;
use sirene_core::diagram::{self, Diagram, Kind, Lang, Lowered};
use sirene_core::theme::ThemeConfig;

#[path = "../../crates/sirene-core/tests/common/parser_registry.rs"]
#[allow(dead_code)]
mod parser_registry;
use parser_registry::{PARSERS, check_ranges};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 4096 {
        return;
    }
    let raw = String::from_utf8_lossy(data);
    check(&raw, None);

    let newline = ["\n", "\r\n", "\r"][byte(data, 3) % 3];
    match byte(data, 6) % 3 {
        0 => known_document(data, newline),
        1 => {
            let lang = [Lang::Mermaid, Lang::Dot, Lang::PlantUml][byte(data, 5) % 3];
            assert_eq!(check(&raw, Some(lang)).kind.lang(), lang);
        }
        _ => directive_recovery(data, newline),
    }
});

fn byte(data: &[u8], at: usize) -> usize {
    usize::from(data.get(at).copied().unwrap_or_default())
}

fn known_document(data: &[u8], newline: &str) {
    let parser = &PARSERS[byte(data, 0) % PARSERS.len()];
    let aliases = [
        ("graph TD", Kind::Flowchart),
        ("flowchart-elk LR", Kind::Flowchart),
        ("stateDiagram", Kind::State),
        ("classDiagram-v2", Kind::Class),
        ("swimlane-beta", Kind::Swimlane),
        ("agentflow-beta", Kind::Agentflow),
        ("railroad-ebnf-beta", Kind::Railroad),
        ("railroad-abnf-beta", Kind::Railroad),
        ("railroad-peg-beta", Kind::Railroad),
        ("@startjson", Kind::PlantUml),
        ("@startyaml", Kind::PlantUml),
        ("@startdot", Kind::PlantUml),
    ];
    let (header, expected) = if byte(data, 1) & 1 != 0 {
        aliases[byte(data, 2) % aliases.len()]
    } else {
        (parser.header, parser.kind)
    };
    let prefixes = [
        "",
        "\u{feff}",
        "%% comment\n",
        "%%{init: {\n 'theme': 'default'\n}}%%\n",
        "---\ntitle: |\n  ---\n  @startuml\n---\n",
        "%%{init: {theme: 'unfinished\n",
    ];
    let prefix = if expected.lang() == Lang::Mermaid {
        prefixes[byte(data, 4) % prefixes.len()].replace('\n', newline)
    } else {
        String::new()
    };
    // The header is fixed, while mutations reach the grammar below it.
    let body = String::from_utf8_lossy(data.get(7..).unwrap_or_default());
    let src = format!("{prefix}{header}{newline}{body}");
    assert_eq!(check(&src, None).kind, expected, "{src:?}");
}

fn directive_recovery(data: &[u8], newline: &str) {
    let (header, statement, expected) = [
        ("flowchart TD", "A[\"kept\"]", Kind::Flowchart),
        ("usecase-beta", "actor A", Kind::UseCase),
        ("railroad-ebnf-beta", "r = 'kept';", Kind::Railroad),
    ][byte(data, 0) % 3];
    let broken = ["%%{unfinished", "%%{init: {theme: 'unfinished"][byte(data, 1) % 2];
    let (encoded, theme) = [(r"\u0064ark", "dark"), ("forest", "forest")][byte(data, 2) % 2];
    let valid = format!(r#"%%{{init: {{unused: ["}}%%"], theme: "{encoded}"}}}}%%"#);
    let src = [header, broken, statement, &valid, ""].join(newline);
    let lowered = check(&src, None);
    assert_eq!(lowered.kind, expected, "{src:?}");
    assert_eq!(
        lowered.theme.name.as_ref().map(|n| n.0.as_str()),
        Some(theme)
    );
    assert_eq!(lowered.diagnostics.len(), 1, "{src:?}");
    let error = &lowered.diagnostics[0];
    assert_eq!(error.message, "unclosed directive: expected `}%%`");
    let opener = header.len() + newline.len();
    assert_eq!(usize::from(error.range.start()), opener);
    assert_eq!(usize::from(error.range.end()), opener + 3);
    match &lowered.diagram {
        Diagram::Graph(graph) => assert!(graph.nodes.iter().any(|node| node.id == "A")),
        Diagram::Railroad(chart) => assert!(chart.rules.iter().any(|rule| rule.name == "r")),
        _ => panic!("expected graph or railroad: {src:?}"),
    }
}

fn check(src: &str, lang: Option<Lang>) -> Lowered {
    let lowered = diagram::lower_as(src, lang);
    assert_eq!(lowered.kind, Kind::of_lang(src, lang), "{src:?}");
    assert!(lowered.diagnostics.len() <= diagram::MAX_DIAGNOSTICS + 1);
    assert!(
        lowered
            .diagnostics
            .windows(2)
            .all(|d| d[0].range.start() <= d[1].range.start())
    );
    check_ranges(src, &lowered.diagnostics);
    if lowered.kind.lang() == Lang::Mermaid {
        assert_eq!(ThemeConfig::read(src), lowered.theme);
        if let Some((_, range)) = &lowered.theme.name {
            assert!(src.get(range.clone()).is_some(), "{range:?}");
        }
        for range in &lowered.theme.unread {
            assert!(src.get(range.clone()).is_some(), "{range:?}");
        }
    }
    lowered
}
