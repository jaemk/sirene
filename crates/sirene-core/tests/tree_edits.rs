//! Property tests for mindmap and timeline drawing edits: each edit that is made does what it
//! says to the lowered model and leaves everything else as it was, and plain text is never
//! refused.

use proptest::prelude::*;
use sirene_core::{mindmap, timeline};

/// Text as the editors read it back: trimmed lines, trimmed as a whole.
fn normalize(text: &str) -> String {
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Plain text that every editor can write: letters, digits, and inner spaces.
fn plain(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ')
}

fn label() -> impl Strategy<Value = String> {
    prop_oneof![
        "[A-Za-z0-9 ]{1,8}",
        "[A-Za-z0-9 _#\"'<>&;:{}()|*/\\[\\]\\\\.,=`%\n-]{0,10}",
    ]
}

#[derive(Debug, Clone)]
enum Op {
    Add(Option<usize>, String),
    AddEvent(usize, String),
    Rename(usize, String),
    RenameEvent(usize, String),
    Delete(Vec<usize>, Vec<usize>),
}

fn op() -> impl Strategy<Value = Op> {
    let ix = 0..12usize;
    prop_oneof![
        (prop::option::of(ix.clone()), label()).prop_map(|(i, l)| Op::Add(i, l)),
        (ix.clone(), label()).prop_map(|(i, l)| Op::AddEvent(i, l)),
        (ix.clone(), label()).prop_map(|(i, l)| Op::Rename(i, l)),
        (ix.clone(), label()).prop_map(|(i, l)| Op::RenameEvent(i, l)),
        (
            prop::collection::vec(ix.clone(), 0..3),
            prop::collection::vec(ix, 0..3)
        )
            .prop_map(|(a, b)| Op::Delete(a, b)),
    ]
}

// Mindmaps

/// A mindmap's nodes as (label, depth), depth first.
fn tree(src: &str) -> Vec<(String, usize)> {
    let map = mindmap::parse(src).lower().0;
    map.nodes
        .iter()
        .map(|n| (n.label.text.clone(), n.depth))
        .collect()
}

/// A mindmap: a root and nodes of mixed shapes, each at most one deeper than the one before,
/// with some icon and class lines.
fn mindmap_doc() -> impl Strategy<Value = String> {
    let node = (1..4usize, 0..4usize, "[A-Za-z][a-z0-9]{0,3}", 0..4usize);
    prop::collection::vec(node, 0..8).prop_map(|nodes| {
        let mut s = String::from("mindmap\n  root((Root))\n");
        let mut depth = 0;
        for (want, shape, text, extra) in nodes {
            depth = want.min(depth + 1);
            let pad = "  ".repeat(depth + 1);
            let node = match shape {
                0 => text.clone(),
                1 => format!("{text}[Sq {text}]"),
                2 => format!("{text}(Round)"),
                _ => format!("{text}{{{{Hex}}}}"),
            };
            s.push_str(&format!("{pad}{node}\n"));
            match extra {
                0 => s.push_str(&format!("{pad}::icon(fa fa-{text})\n")),
                1 => s.push_str(&format!("{pad}:::urgent\n")),
                _ => {}
            }
        }
        s
    })
}

fn mindmap_step(src: &str, op: &Op) -> Result<String, TestCaseError> {
    let before = tree(src);
    let n = before.len();
    let out = match op {
        Op::Add(parent, text) => {
            let parent = parent.filter(|&p| p < n || n == 0).map(|p| p % n.max(1));
            let parent = if n == 0 { None } else { parent.or(Some(0)) };
            let Some((i, e)) = mindmap::edit::add_node(src, parent, text) else {
                prop_assert!(!plain(text), "refused {text:?} under {parent:?}:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let mut after = tree(&out);
            let want = match normalize(text) {
                t if t.is_empty() => "New idea".to_owned(),
                t => t,
            };
            let depth = parent.map_or(0, |p| before[p].1 + 1);
            prop_assert_eq!(after.get(i), Some(&(want, depth)), "{}", out);
            after.remove(i);
            prop_assert_eq!(&after, &before, "{}", out);
            out
        }
        Op::Rename(i, text) if *i < n => {
            let Some(e) = mindmap::edit::set_label(src, *i, text) else {
                let same = normalize(text) == before[*i].0;
                prop_assert!(!plain(text) || same, "refused {text:?} for {i}:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let mut want = before.clone();
            want[*i].0 = normalize(text);
            prop_assert_eq!(&tree(&out), &want, "{}", out);
            out
        }
        Op::Delete(nodes, _) => {
            let Some(e) = mindmap::edit::delete(src, nodes) else {
                prop_assert!(nodes.iter().all(|&i| i >= n), "nothing deleted:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            // Each deleted node goes with the nodes deeper than it that follow it.
            let mut gone = vec![false; n];
            for &i in nodes.iter().filter(|&&i| i < n) {
                gone[i] = true;
                for j in i + 1..n {
                    if before[j].1 <= before[i].1 {
                        break;
                    }
                    gone[j] = true;
                }
            }
            let want: Vec<_> = (before.iter().zip(&gone))
                .filter(|(_, g)| !**g)
                .map(|(n, _)| n.clone())
                .collect();
            prop_assert_eq!(&tree(&out), &want, "{}", out);
            out
        }
        _ => src.to_owned(),
    };
    Ok(out)
}

// Timelines

type Periods = Vec<(Vec<String>, Vec<Vec<String>>)>;

fn periods(src: &str) -> Periods {
    let t = timeline::parse(src).lower().0;
    t.periods()
        .map(|(_, p)| (p.label.clone(), p.events.clone()))
        .collect()
}

fn lines(text: &str) -> Vec<String> {
    normalize(text).lines().map(str::to_owned).collect()
}

/// A timeline: periods in sections, with events on their line and on `: EVENT` lines.
fn timeline_doc() -> impl Strategy<Value = String> {
    let event = "[A-Za-z][a-z0-9 ]{0,4}[a-z0-9]";
    let period = (
        any::<bool>(),
        "[0-9]{4}",
        prop::collection::vec(event, 0..3),
        prop::collection::vec(event, 0..3),
    );
    (any::<bool>(), prop::collection::vec(period, 0..5)).prop_map(|(title, periods)| {
        let mut s = String::from("timeline\n");
        if title {
            s.push_str("    title History\n");
        }
        for (section, label, own, more) in periods {
            if section {
                s.push_str("    section Era\n");
            }
            s.push_str(&format!("    {label}"));
            for e in own {
                s.push_str(&format!(" : {e}"));
            }
            s.push('\n');
            if !more.is_empty() {
                s.push_str("         ");
                for e in more {
                    s.push_str(&format!(" : {e}"));
                }
                s.push('\n');
            }
        }
        s
    })
}

fn timeline_step(src: &str, op: &Op) -> Result<String, TestCaseError> {
    let before = periods(src);
    let n = before.len();
    let flat: Vec<(usize, usize)> = (before.iter().enumerate())
        .flat_map(|(i, (_, es))| (0..es.len()).map(move |k| (i, k)))
        .collect();
    let out = match op {
        Op::Add(after, text) => {
            let after = after.filter(|&a| a < n);
            let Some((i, e)) = timeline::edit::add_period(src, after, text) else {
                prop_assert!(!plain(text), "refused {text:?} after {after:?}:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let mut want = before.clone();
            want.insert(i, (lines(text), Vec::new()));
            prop_assert_eq!(&periods(&out), &want, "{}", out);
            out
        }
        Op::AddEvent(i, text) if *i < n => {
            let Some((k, e)) = timeline::edit::add_event(src, *i, text) else {
                prop_assert!(!plain(text), "refused {text:?} in {i}:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let mut want = before.clone();
            want[*i].1.push(lines(text));
            let at = before[..=*i].iter().map(|p| p.1.len()).sum::<usize>();
            prop_assert_eq!(k, at);
            prop_assert_eq!(&periods(&out), &want, "{}", out);
            out
        }
        Op::Rename(i, text) if *i < n => {
            let Some(e) = timeline::edit::set_period_label(src, *i, text) else {
                let same = lines(text) == before[*i].0;
                prop_assert!(!plain(text) || same, "refused {text:?} for {i}:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let mut want = before.clone();
            want[*i].0 = lines(text);
            prop_assert_eq!(&periods(&out), &want, "{}", out);
            out
        }
        Op::RenameEvent(m, text) if *m < flat.len() => {
            let (i, k) = flat[*m];
            let Some(e) = timeline::edit::set_event_text(src, *m, text) else {
                let same = lines(text) == before[i].1[k];
                prop_assert!(!plain(text) || same, "refused {text:?} for {m}:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let mut want = before.clone();
            want[i].1[k] = lines(text);
            prop_assert_eq!(&periods(&out), &want, "{}", out);
            out
        }
        Op::Delete(ps, es) => {
            let any = ps.iter().any(|&i| i < n) || es.iter().any(|&m| m < flat.len());
            let Some(e) = timeline::edit::delete(src, ps, es) else {
                prop_assert!(!any, "nothing deleted:\n{src}");
                return Ok(src.to_owned());
            };
            let out = e.apply(src);
            let want: Periods = (before.iter().enumerate())
                .filter(|(i, _)| !ps.contains(i))
                .map(|(i, (label, events))| {
                    let kept = (events.iter().enumerate())
                        .filter(|(k, _)| {
                            let m = flat.iter().position(|&f| f == (i, *k));
                            !m.is_some_and(|m| es.contains(&m))
                        })
                        .map(|(_, e)| e.clone())
                        .collect();
                    (label.clone(), kept)
                })
                .collect();
            prop_assert_eq!(&periods(&out), &want, "{}", out);
            out
        }
        _ => src.to_owned(),
    };
    Ok(out)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn mindmap_edits(src in mindmap_doc(), ops in prop::collection::vec(op(), 1..8)) {
        let mut src = src;
        for op in &ops {
            src = mindmap_step(&src, op)?;
        }
    }

    #[test]
    fn timeline_edits(src in timeline_doc(), ops in prop::collection::vec(op(), 1..8)) {
        let mut src = src;
        for op in &ops {
            src = timeline_step(&src, op)?;
        }
    }
}
