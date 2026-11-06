//! Cycle removal and rank assignment (LAYOUT-4).

use std::collections::VecDeque;

/// An edge `from -> to` that must span at least `len` ranks.
#[derive(Debug, Clone, Copy)]
pub(super) struct RankEdge {
    pub from: usize,
    pub to: usize,
    pub len: i32,
}

/// Ranks `n` items. Returns whether each edge was reversed to break a cycle, and each item's
/// rank (the smallest is 0). Self-loops must not be passed in.
pub(super) fn rank(n: usize, edges: &[RankEdge]) -> (Vec<bool>, Vec<i32>) {
    let reversed = break_cycles(n, edges);
    let dag: Vec<RankEdge> = edges
        .iter()
        .zip(&reversed)
        .map(|(e, &r)| {
            if r {
                RankEdge {
                    from: e.to,
                    to: e.from,
                    len: e.len,
                }
            } else {
                *e
            }
        })
        .collect();
    let mut ranks = longest_path(n, &dag);
    tighten(&mut ranks, &dag);
    if let Some(&min) = ranks.iter().min() {
        for r in &mut ranks {
            *r -= min;
        }
    }
    (reversed, ranks)
}

/// Marks the back edges of a depth-first search that visits items and edges in input order.
fn break_cycles(n: usize, edges: &[RankEdge]) -> Vec<bool> {
    let mut out = vec![Vec::new(); n];
    for (i, e) in edges.iter().enumerate() {
        out[e.from].push(i);
    }
    let mut reversed = vec![false; edges.len()];
    // 0: unvisited, 1: on the stack, 2: done.
    let mut state = vec![0u8; n];
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        state[start] = 1;
        let mut stack = vec![(start, 0usize)];
        while let Some(top) = stack.last_mut() {
            let (v, i) = *top;
            match out[v].get(i) {
                Some(&e) => {
                    top.1 += 1;
                    let w = edges[e].to;
                    match state[w] {
                        0 => {
                            state[w] = 1;
                            stack.push((w, 0));
                        }
                        1 => reversed[e] = true,
                        _ => {}
                    }
                }
                None => {
                    state[v] = 2;
                    stack.pop();
                }
            }
        }
    }
    reversed
}

/// Ranks each item just below its lowest predecessor; sources get rank 0.
fn longest_path(n: usize, dag: &[RankEdge]) -> Vec<i32> {
    let mut indegree = vec![0usize; n];
    let mut out = vec![Vec::new(); n];
    for e in dag {
        indegree[e.to] += 1;
        out[e.from].push(*e);
    }
    let mut ranks = vec![0; n];
    let mut queue: VecDeque<usize> = (0..n).filter(|&v| indegree[v] == 0).collect();
    while let Some(v) = queue.pop_front() {
        for e in &out[v] {
            ranks[e.to] = ranks[e.to].max(ranks[v] + e.len);
            indegree[e.to] -= 1;
            if indegree[e.to] == 0 {
                queue.push_back(e.to);
            }
        }
    }
    ranks
}

/// Moves items to shorten edges: an item with more incoming than outgoing edges moves as far up
/// as its predecessors allow, and one with more outgoing edges as far down as its successors
/// allow. Each move shortens the total edge span, so this terminates.
fn tighten(ranks: &mut [i32], dag: &[RankEdge]) {
    let n = ranks.len();
    let mut incoming = vec![Vec::new(); n];
    let mut outgoing = vec![Vec::new(); n];
    for e in dag {
        incoming[e.to].push(*e);
        outgoing[e.from].push(*e);
    }
    loop {
        let mut changed = false;
        for v in 0..n {
            let (ins, outs) = (&incoming[v], &outgoing[v]);
            let target = if ins.len() > outs.len() {
                ins.iter().map(|e| ranks[e.from] + e.len).max()
            } else if outs.len() > ins.len() {
                outs.iter().map(|e| ranks[e.to] - e.len).min()
            } else {
                None
            };
            if let Some(t) = target {
                if t != ranks[v] {
                    ranks[v] = t;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(list: &[(usize, usize)]) -> Vec<RankEdge> {
        list.iter()
            .map(|&(from, to)| RankEdge { from, to, len: 2 })
            .collect()
    }

    #[test]
    fn chain() {
        let (rev, ranks) = rank(3, &edges(&[(0, 1), (1, 2)]));
        assert_eq!(rev, [false, false]);
        assert_eq!(ranks, [0, 2, 4]);
    }

    #[test]
    fn cycle_reverses_back_edge() {
        let (rev, ranks) = rank(3, &edges(&[(0, 1), (1, 2), (2, 0)]));
        assert_eq!(rev, [false, false, true]);
        assert_eq!(ranks, [0, 2, 4]);
    }

    #[test]
    fn source_is_pulled_down_to_its_target() {
        // 0 -> 1 -> 2 -> 3 and 4 -> 3: 4 sits just above 3.
        let (_, ranks) = rank(5, &edges(&[(0, 1), (1, 2), (2, 3), (4, 3)]));
        assert_eq!(ranks, [0, 2, 4, 6, 4]);
    }

    #[test]
    fn sink_is_pulled_up_to_its_source() {
        let (_, ranks) = rank(4, &edges(&[(0, 1), (1, 2), (0, 3)]));
        assert_eq!(ranks, [0, 2, 4, 2]);
    }

    #[test]
    fn minimum_lengths() {
        let e = [RankEdge {
            from: 0,
            to: 1,
            len: 6,
        }];
        assert_eq!(rank(2, &e).1, [0, 6]);
    }

    #[test]
    fn isolated_items_and_empty() {
        assert_eq!(rank(2, &[]).1, [0, 0]);
        assert_eq!(rank(0, &[]), (vec![], vec![]));
    }

    #[test]
    fn two_cycles_share_a_node() {
        let (rev, _) = rank(3, &edges(&[(0, 1), (1, 0), (1, 2), (2, 1)]));
        assert_eq!(rev, [false, true, false, true]);
    }
}
