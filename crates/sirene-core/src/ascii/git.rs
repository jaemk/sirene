//! Git graphs as text, one commit per row (MGIT-5).

use super::Options;
use super::canvas::{Canvas, Glyph, Line, glyph};
use super::width as columns;
use crate::git::{Commit, CommitType, Direction, GitGraph};

fn commit_glyph(c: &Commit) -> Glyph {
    match c.kind {
        CommitType::Reverse => glyph('⊗', 'x'),
        CommitType::Highlight => glyph('■', '#'),
        CommitType::Normal if c.is_merge() => glyph('◉', '@'),
        CommitType::Normal => glyph('●', '*'),
    }
}

/// The text after a commit's lanes: its id and tags.
fn caption(c: &Commit) -> String {
    let mut s = c.id.clone();
    for t in &c.tags {
        s.push_str(" [");
        s.push_str(t);
        s.push(']');
    }
    s
}

/// Renders `graph` as text (MGIT-5).
pub fn render(graph: &GitGraph, options: &Options) -> String {
    let lanes = graph.branches.len() as i32;
    let rows = graph.commits.len() as i32;
    let x = |k: usize| 2 * k as i32;
    let text_x = 2 * (lanes - 1).max(0) + 3;
    let width = graph
        .branches
        .iter()
        .enumerate()
        .map(|(k, b)| x(k) + columns(&b.name))
        .chain(graph.commits.iter().map(|c| text_x + columns(&caption(c))))
        .max()
        .unwrap_or(0);
    let height = lanes + rows;
    // Rows top to bottom; bottom to top flips them.
    let flip = graph.direction == Direction::BottomTop;
    let y = |row: i32| if flip { height - 1 - row } else { row };
    let commit_row = |i: usize| lanes + i as i32;
    let mut canvas = Canvas::new(width, height);

    // Each lane runs from below its name to the last row that uses it.
    let mut last = vec![-1; graph.branches.len()];
    for (i, c) in graph.commits.iter().enumerate() {
        last[c.branch] = last[c.branch].max(commit_row(i));
        for &p in &c.parents {
            let b = graph.commits[p].branch;
            last[b] = last[b].max(commit_row(i));
        }
    }
    for (k, &end) in last.iter().enumerate() {
        let from = k as i32 + 1;
        if end >= from {
            canvas.vline(x(k), y(from).min(y(end)), y(from).max(y(end)), Line::Light);
        }
    }
    // A parent on another lane joins across the child's row.
    for (i, c) in graph.commits.iter().enumerate() {
        for &p in &c.parents {
            let b = graph.commits[p].branch;
            if b != c.branch {
                let (a, z) = (x(b).min(x(c.branch)), x(b).max(x(c.branch)));
                canvas.hline(a, z, y(commit_row(i)), Line::Light);
            }
        }
    }
    for (i, c) in graph.commits.iter().enumerate() {
        let row = y(commit_row(i));
        canvas.arrow(x(c.branch), row, commit_glyph(c));
        canvas.text(text_x, row, &caption(c));
    }
    for (k, b) in graph.branches.iter().enumerate() {
        canvas.text(x(k), y(k as i32), &b.name);
    }
    let body = canvas.render(options.charset);
    match graph
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        // The title centered above, as for flowcharts.
        Some(t) => {
            let pad = (width - columns(t)).max(0) / 2;
            format!("{}{t}\n\n{body}", " ".repeat(pad as usize))
        }
        None => body,
    }
}
