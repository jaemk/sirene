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
    assert_eq!((read, failures.len()), (437, 12));
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

/// A parse tree as KaTeX's JSON writes it, so the two can be compared (`loc` is left out of the
/// oracle).
fn katex_json(node: &super::ast::Node<'_>) -> Value {
    use super::ast::NodeKind;
    use super::symbols::{AtomGroup, Mode, Name};
    use serde_json::json;

    let mode = match node.mode {
        Mode::Math => "math",
        Mode::Text => "text",
    };
    match &node.kind {
        NodeKind::Symbol { group, name } => {
            let text = match name {
                Name::Char(text) => (*text).to_string(),
                Name::Control(name) => format!("\\{name}"),
            };
            let family = |f: &str| json!({"type": "atom", "mode": mode, "family": f, "text": text});
            let plain = |t: &str| json!({"type": t, "mode": mode, "text": text});
            match group {
                AtomGroup::Bin => family("bin"),
                AtomGroup::Close => family("close"),
                AtomGroup::Inner => family("inner"),
                AtomGroup::Open => family("open"),
                AtomGroup::Punct => family("punct"),
                AtomGroup::Rel => family("rel"),
                AtomGroup::MathOrd => plain("mathord"),
                AtomGroup::TextOrd => plain("textord"),
                AtomGroup::Spacing => plain("spacing"),
                AtomGroup::Op => plain("op-token"),
                AtomGroup::Accent => plain("accent-token"),
            }
        }
        NodeKind::Group { body, semisimple } => {
            let mut fields = serde_json::Map::new();
            fields.insert("type".into(), "ordgroup".into());
            fields.insert("mode".into(), mode.into());
            fields.insert(
                "body".into(),
                body.iter().map(katex_json).collect::<Vec<_>>().into(),
            );
            if *semisimple {
                fields.insert("semisimple".into(), true.into());
            }
            Value::Object(fields)
        }
        NodeKind::Frac {
            numer,
            denom,
            continued,
            bar,
            bar_size,
            left,
            right,
        } => json!({
            "type": "genfrac", "mode": mode,
            "numer": katex_json(numer), "denom": katex_json(denom),
            "continued": continued, "hasBarLine": bar,
            "leftDelim": left, "rightDelim": right,
            "barSize": bar_size.as_ref().map(measurement_json),
        }),
        NodeKind::Infix { replace_with, size } => json!({
            "type": "infix", "mode": mode, "replaceWith": replace_with,
            "size": size.as_ref().map(measurement_json),
        }),
        NodeKind::Sqrt { body, index } => json!({
            "type": "sqrt", "mode": mode, "body": katex_json(body),
            "index": index.as_deref().map(katex_json),
        }),
        NodeKind::Styling {
            style,
            body,
            reset_font,
        } => {
            use super::ast::StyleName;
            let style = match style {
                StyleName::Display => "display",
                StyleName::Text => "text",
                StyleName::Script => "script",
                StyleName::ScriptScript => "scriptscript",
            };
            let mut value = json!({"type": "styling", "mode": mode, "style": style,
                "body": body.iter().map(katex_json).collect::<Vec<_>>()});
            if *reset_font {
                value["resetFont"] = true.into();
            }
            value
        }
        NodeKind::Text { body, font } => json!({
            "type": "text", "mode": mode, "font": format!("\\{font}"),
            "body": body.iter().map(katex_json).collect::<Vec<_>>(),
        }),
        NodeKind::Font { font, body } => {
            json!({"type": "font", "mode": mode, "font": font, "body": katex_json(body)})
        }
        NodeKind::MClass {
            class,
            body,
            character_box,
        } => {
            json!({"type": "mclass", "mode": mode, "mclass": class_json(*class),
                "body": body.iter().map(katex_json).collect::<Vec<_>>(),
                "isCharacterBox": character_box})
        }
        NodeKind::Enclose {
            label,
            background_color,
            border_color,
            body,
        } => {
            let mut value = json!({"type": "enclose", "mode": mode, "label": label,
                "body": katex_json(body)});
            if let Some(color) = background_color {
                value["backgroundColor"] = color.clone().into();
            }
            if let Some(color) = border_color {
                value["borderColor"] = color.clone().into();
            }
            value
        }
        NodeKind::Pmb { class, body } => json!({"type": "pmb", "mode": mode,
            "mclass": class_json(*class), "body": nodes_json(body)}),
        NodeKind::HBox { body } => {
            json!({"type": "hbox", "mode": mode, "body": nodes_json(body)})
        }
        NodeKind::VCenter { body } => {
            json!({"type": "vcenter", "mode": mode, "body": katex_json(body)})
        }
        NodeKind::MathChoice {
            display,
            text,
            script,
            script_script,
        } => json!({"type": "mathchoice", "mode": mode, "display": nodes_json(display),
            "text": nodes_json(text), "script": nodes_json(script),
            "scriptscript": nodes_json(script_script)}),
        NodeKind::ReflectBox { body } => {
            json!({"type": "reflectbox", "mode": mode, "body": katex_json(body)})
        }
        NodeKind::Overline { body } => {
            json!({"type": "overline", "mode": mode, "body": katex_json(body)})
        }
        NodeKind::Underline { body } => {
            json!({"type": "underline", "mode": mode, "body": katex_json(body)})
        }
        NodeKind::DelimSizing { size, class, delim } => {
            use super::ast::MathClass;
            let class = match class {
                MathClass::Ord => "mord",
                MathClass::Rel => "mrel",
                MathClass::Open => "mopen",
                MathClass::Close => "mclose",
                _ => "other",
            };
            json!({"type": "delimsizing", "mode": mode, "size": size, "mclass": class,
                "delim": delim})
        }
        NodeKind::LeftRight {
            body,
            left,
            right,
            right_color,
        } => {
            let mut value = json!({
                "type": "leftright", "mode": mode, "left": left, "right": right,
                "body": body.iter().map(katex_json).collect::<Vec<_>>(),
            });
            if let Some(color) = right_color {
                value["rightColor"] = color.as_str().into();
            }
            value
        }
        NodeKind::Accent {
            label,
            stretchy,
            shifty,
            base,
        } => json!({"type": "accent", "mode": mode, "label": label, "isStretchy": stretchy,
            "isShifty": shifty, "base": katex_json(base)}),
        NodeKind::AccentUnder { label, base } => {
            json!({"type": "accentUnder", "mode": mode, "label": label, "base": katex_json(base)})
        }
        NodeKind::Op {
            limits,
            always_handle_supsub,
            symbol,
            content,
        } => {
            use super::ast::OpContent;
            let mut fields = serde_json::Map::new();
            fields.insert("type".into(), "op".into());
            fields.insert("mode".into(), mode.into());
            fields.insert("limits".into(), (*limits).into());
            fields.insert("parentIsSupSub".into(), false.into());
            fields.insert("symbol".into(), (*symbol).into());
            match content {
                OpContent::Name(name) => fields.insert("name".into(), name.as_str().into()),
                OpContent::Body(body) => fields.insert(
                    "body".into(),
                    body.iter().map(katex_json).collect::<Vec<_>>().into(),
                ),
            };
            if *always_handle_supsub {
                fields.insert("alwaysHandleSupSub".into(), true.into());
            }
            Value::Object(fields)
        }
        NodeKind::OperatorName {
            body,
            always_handle_supsub,
            limits,
        } => json!({"type": "operatorname", "mode": mode,
            "body": body.iter().map(katex_json).collect::<Vec<_>>(),
            "alwaysHandleSupSub": always_handle_supsub, "limits": limits,
            "parentIsSupSub": false}),
        NodeKind::Kern { dimension } => {
            json!({"type": "kern", "mode": mode, "dimension": measurement_json(dimension)})
        }
        NodeKind::Rule {
            shift,
            width,
            height,
        } => json!({"type": "rule", "mode": mode,
            "shift": shift.as_ref().map(measurement_json),
            "width": measurement_json(width), "height": measurement_json(height)}),
        NodeKind::Color { color, body } => json!({"type": "color", "mode": mode, "color": color,
            "body": body.iter().map(katex_json).collect::<Vec<_>>()}),
        NodeKind::Phantom { body } => json!({"type": "phantom", "mode": mode,
            "body": body.iter().map(katex_json).collect::<Vec<_>>()}),
        NodeKind::VPhantom { body } => {
            json!({"type": "vphantom", "mode": mode, "body": katex_json(body)})
        }
        NodeKind::Smash {
            body,
            height,
            depth,
        } => json!({"type": "smash", "mode": mode, "body": katex_json(body),
            "smashHeight": height, "smashDepth": depth}),
        NodeKind::Lap { alignment, body } => {
            json!({"type": "lap", "mode": mode, "alignment": alignment, "body": katex_json(body)})
        }
        NodeKind::RaiseBox { dy, body } => json!({"type": "raisebox", "mode": mode,
            "dy": measurement_json(dy), "body": katex_json(body)}),
        NodeKind::Sizing { size, body } => json!({"type": "sizing", "mode": mode, "size": size,
            "body": body.iter().map(katex_json).collect::<Vec<_>>()}),
        NodeKind::Size { value, is_blank } => json!({"type": "size", "mode": mode,
            "value": measurement_json(value), "isBlank": is_blank}),
        NodeKind::ColorToken(color) => json!({"type": "color-token", "mode": mode, "color": color}),
        NodeKind::Internal => json!({"type": "internal", "mode": mode}),
        NodeKind::Array(array) => {
            use super::ast::{AlignSpec, ColSeparation, RowTag};
            let nodes =
                |body: &[super::ast::Node<'_>]| body.iter().map(katex_json).collect::<Vec<_>>();
            let mut fields = serde_json::Map::new();
            fields.insert("type".into(), "array".into());
            fields.insert("mode".into(), mode.into());
            fields.insert("arraystretch".into(), number_json(array.arraystretch));
            fields.insert(
                "body".into(),
                array
                    .body
                    .iter()
                    .map(|row| nodes(row))
                    .collect::<Vec<_>>()
                    .into(),
            );
            fields.insert(
                "rowGaps".into(),
                array
                    .row_gaps
                    .iter()
                    .map(|gap| gap.as_ref().map_or(Value::Null, measurement_json))
                    .collect::<Vec<_>>()
                    .into(),
            );
            fields.insert("hLinesBeforeRow".into(), json!(array.hlines_before_row));
            if array.add_jot {
                fields.insert("addJot".into(), true.into());
            }
            if let Some(cols) = &array.cols {
                let cols: Vec<Value> = cols
                    .iter()
                    .map(|col| match col {
                        AlignSpec::Align {
                            align,
                            pregap,
                            postgap,
                        } => {
                            let mut c = json!({"type": "align", "align": align.to_string()});
                            if let Some(gap) = pregap {
                                c["pregap"] = number_json(*gap);
                            }
                            if let Some(gap) = postgap {
                                c["postgap"] = number_json(*gap);
                            }
                            c
                        }
                        AlignSpec::Separator(c) => {
                            json!({"type": "separator", "separator": c.to_string()})
                        }
                    })
                    .collect();
                fields.insert("cols".into(), cols.into());
            }
            if let Some(hskip) = array.hskip_before_and_after {
                fields.insert("hskipBeforeAndAfter".into(), hskip.into());
            }
            if let Some(kind) = array.col_separation {
                let kind = match kind {
                    ColSeparation::Align => "align",
                    ColSeparation::Alignat => "alignat",
                    ColSeparation::Gather => "gather",
                    ColSeparation::Small => "small",
                };
                fields.insert("colSeparationType".into(), kind.into());
            }
            if let Some(tags) = &array.tags {
                let tags: Vec<Value> = tags
                    .iter()
                    .map(|tag| match tag {
                        RowTag::Numbered(numbered) => Value::from(*numbered),
                        RowTag::Custom(body) => nodes(body).into(),
                    })
                    .collect();
                fields.insert("tags".into(), tags.into());
            }
            if let Some(leqno) = array.leqno {
                fields.insert("leqno".into(), leqno.into());
            }
            Value::Object(fields)
        }
        NodeKind::Cr { new_line, size } => json!({"type": "cr", "mode": mode,
            "newLine": new_line, "size": size.as_ref().map(measurement_json)}),
        NodeKind::Environment { name } => {
            json!({"type": "environment", "mode": mode, "name": name})
        }
        NodeKind::Tag { body, tag } => json!({"type": "tag", "mode": mode,
            "body": body.iter().map(katex_json).collect::<Vec<_>>(),
            "tag": tag.iter().map(katex_json).collect::<Vec<_>>()}),
        NodeKind::Middle { delim } => json!({"type": "middle", "mode": mode, "delim": delim}),
        NodeKind::SupSub { base, sup, sub } => {
            let mut fields = serde_json::Map::new();
            fields.insert("type".into(), "supsub".into());
            fields.insert("mode".into(), mode.into());
            fields.insert(
                "base".into(),
                base.as_deref().map_or(Value::Null, katex_json),
            );
            if let Some(sup) = sup {
                fields.insert("sup".into(), katex_json(sup));
            }
            if let Some(sub) = sub {
                fields.insert("sub".into(), katex_json(sub));
            }
            Value::Object(fields)
        }
    }
}

/// A number as KaTeX's JSON writes it: a whole number has no decimal point.
fn nodes_json(nodes: &[super::ast::Node<'_>]) -> Value {
    nodes.iter().map(katex_json).collect::<Vec<_>>().into()
}

fn class_json(class: super::ast::MathClass) -> &'static str {
    use super::ast::MathClass;
    match class {
        MathClass::Ord => "mord",
        MathClass::Bin => "mbin",
        MathClass::Rel => "mrel",
        MathClass::Open => "mopen",
        MathClass::Close => "mclose",
        MathClass::Punct => "mpunct",
        MathClass::Inner => "minner",
    }
}

fn number_json(number: f64) -> Value {
    if number.fract() == 0.0 && number.abs() < 1e15 {
        Value::from(number as i64)
    } else {
        Value::from(number)
    }
}

/// A length as KaTeX's JSON writes it.
fn measurement_json(m: &super::ast::Measurement) -> Value {
    serde_json::json!({"number": number_json(m.number), "unit": m.unit})
}

/// The inputs of the oracle that need no macro settings, with whether they were read in display
/// mode and KaTeX's answer for each.
fn plain_oracle() -> Vec<(String, bool, Value)> {
    let data: serde_json::Map<String, Value> =
        serde_json::from_str(include_str!("../../tests/fixtures/math/katex-oracle.json")).unwrap();
    data.into_iter()
        .filter_map(|(key, entry)| {
            let (input, display, macros): (String, bool, Value) =
                serde_json::from_str(&key).unwrap();
            macros.is_null().then_some((input, display, entry))
        })
        .collect()
}

/// The parser builds the tree KaTeX builds, for every baseline input it supports, and fails with
/// KaTeX's message where KaTeX fails. An input that reaches a command the parser lacks is counted
/// as not yet supported; the count falls as the parser grows.
#[test]
fn the_parser_agrees_with_katex() {
    use super::parser::{Settings, parse_with};

    let (mut same, mut unsupported) = (0, 0);
    let mut differences = Vec::new();
    // Inputs where the design differs on purpose (the deviations table in spec/latex-math.md):
    // KaTeX runs `\def` as a function of the parser, so it rejects a `\def` as the argument of a
    // script ("Got function '\def' with no arguments as superscript"); here a definition is a
    // macro the expander runs, and the script reads what follows it.
    const DEVIATIONS: &[&str] = &["2^\\def\\foo{1}2"];
    for (input, display, entry) in plain_oracle() {
        if DEVIATIONS.contains(&input.as_str()) {
            continue;
        }
        let parsed = parse_with(
            &input,
            Settings {
                display_mode: display,
            },
        );
        match (&parsed, entry.get("tree"), entry.get("parse_error")) {
            (Err(e), _, _) if e.message.ends_with("supported yet") => unsupported += 1,
            (Ok(nodes), Some(tree), _) => {
                let ours = Value::Array(nodes.iter().map(katex_json).collect());
                if ours == *tree {
                    same += 1;
                } else {
                    differences.push(format!("tree differs  <-  {input:?}"));
                }
            }
            (Err(e), None, Some(Value::String(expected))) => {
                // The size of the expansion limit is not KaTeX's, and the message says so.
                let same_message = e.message == *expected
                    || (e.message.starts_with("Too many expansions")
                        && expected.starts_with("Too many expansions"));
                if same_message {
                    same += 1;
                } else {
                    differences.push(format!(
                        "{:?} vs KaTeX's {expected:?}  <-  {input:?}",
                        e.message
                    ));
                }
            }
            (Ok(_), None, _) => differences.push(format!("accepted what KaTeX rejects: {input:?}")),
            (Err(e), Some(_), _) => differences.push(format!("{:?}  <-  {input:?}", e.message)),
            (Err(_), None, _) => differences.push(format!("no oracle answer: {input:?}")),
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    // This count falls as the parser gains the functions.
    assert_eq!((same, unsupported), (488, 127));
}

/// A development aid, not a check: which unsupported commands stop the most baseline inputs, to
/// pick the next ones to build. Run with `cargo test -p sirene-core --lib unsupported_commands --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn unsupported_commands_by_inputs_stopped() {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for (input, _, _) in plain_oracle() {
        if let Err(e) = super::parser::parse(&input)
            && e.message.ends_with("supported yet")
        {
            *counts.entry(e.message).or_default() += 1;
        }
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.1));
    for (message, count) in rows.iter().take(50) {
        println!("{count:4} {message}");
    }
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
