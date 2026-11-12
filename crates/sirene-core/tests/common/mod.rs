//! Checks the property tests share: the invariants of a lowered graph model and of its
//! drawings.

use proptest::prelude::*;
use sirene_core::ascii::{self, Charset, Options};
use sirene_core::flowchart::Flowchart;
use sirene_core::scene::{self, Approx, Primitive, Pt, Segment, Theme};

/// Rendering never panics and ends every line with a newline (ASCIIR-1).
pub fn check_render(c: &Flowchart) -> Result<(), TestCaseError> {
    for charset in [Charset::Unicode, Charset::Ascii] {
        let out = ascii::render(c, &Options { charset });
        prop_assert!(out.is_empty() || out.ends_with('\n'));
        prop_assert!(
            !out.lines().any(|l| l.ends_with(' ')),
            "trailing space in\n{}",
            out
        );
    }
    check_scene(c)
}

/// Every coordinate in a drawing is finite and inside the scene (DISPLA-1).
pub fn check_scene(c: &Flowchart) -> Result<(), TestCaseError> {
    let s = scene::flowchart(c, &Approx, &Theme::LIGHT);
    let inside = |p: &Pt| {
        p.x.is_finite()
            && p.y.is_finite()
            && (-0.01..=s.width + 0.01).contains(&p.x)
            && (-0.01..=s.height + 0.01).contains(&p.y)
    };
    for e in &s.elements {
        match &e.primitive {
            Primitive::Path { segments, .. } => {
                for seg in segments {
                    let ok = match seg {
                        Segment::Move(p) | Segment::Line(p) => inside(p),
                        Segment::Arc { to, .. } => inside(to),
                        Segment::Cubic { c1, c2, to } => inside(c1) && inside(c2) && inside(to),
                        Segment::Close => true,
                    };
                    prop_assert!(ok, "{:?} outside {}x{}", seg, s.width, s.height);
                }
            }
            Primitive::Text { center, .. } => prop_assert!(inside(center), "{:?}", center),
        }
    }
    Ok(())
}

/// Structural invariants of any lowered model (FLOWMO-4, FLOWMO-8, FLOWMO-10).
pub fn check_model(c: &Flowchart) -> Result<(), TestCaseError> {
    let mut ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.extend(c.subgraphs.iter().map(|s| s.id.as_str()));
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    prop_assert_eq!(ids.len(), count, "node and subgraph ids must be unique");
    let known = |id: &str| ids.binary_search(&id).is_ok();
    for e in &c.edges {
        prop_assert!(known(&e.from) && known(&e.to), "dangling edge {:?}", e);
        prop_assert!(e.length <= 10);
    }
    for (id, _) in &c.ordering {
        prop_assert!(known(id), "ordering names a missing node {:?}", id);
    }
    let ends = c.end_ranks.iter().map(|(_, g)| g);
    for group in c.same_rank.iter().chain(ends).chain(&c.groups) {
        for id in group {
            prop_assert!(known(id), "rank group names a missing node {:?}", id);
        }
    }
    let mut members = Vec::new();
    for s in &c.subgraphs {
        for m in &s.members {
            prop_assert!(known(m), "unknown member {} of {}", m, s.id);
            prop_assert_ne!(m, &s.id);
            members.push(m.as_str());
        }
    }
    let count = members.len();
    members.sort_unstable();
    members.dedup();
    prop_assert_eq!(members.len(), count, "a member belongs to one subgraph");
    for s in &c.subgraphs {
        let mut current = c.parent(&s.id);
        let mut depth = 0;
        while let Some(p) = current {
            prop_assert_ne!(&p.id, &s.id, "subgraph nesting has a cycle");
            depth += 1;
            prop_assert!(depth <= c.subgraphs.len(), "subgraph nesting has a cycle");
            current = c.parent(&p.id);
        }
    }
    Ok(())
}
