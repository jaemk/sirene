//! Track assignment for the cross-axis part of routes in a gap between ranks (LAYOUT-8).

/// A route segment through a gap, entering at cross position `top` and leaving at `bottom`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Jog {
    pub top: i32,
    pub bottom: i32,
}

impl Jog {
    fn lo(self) -> i32 {
        self.top.min(self.bottom)
    }

    fn hi(self) -> i32 {
        self.top.max(self.bottom)
    }

    fn strictly_inside(self, x: i32) -> bool {
        self.lo() < x && x < self.hi()
    }
}

/// Assigns each jog that changes position a track, numbered from the upper rank. Returns the
/// track per jog (`None` for straight ones) and the number of tracks.
///
/// A jog whose upper part shares a column with another's lower part turns first, so the two do
/// not overlap. Where possible a jog also turns before crossing another's cross-axis part.
pub(super) fn tracks(jogs: &[Jog]) -> (Vec<Option<usize>>, usize) {
    let moving: Vec<usize> = (0..jogs.len())
        .filter(|&i| jogs[i].top != jogs[i].bottom)
        .collect();
    let m = moving.len();
    // before[a][b]: moving[a] must take a lower track number than moving[b].
    let mut before = vec![vec![false; m]; m];
    for a in 0..m {
        for b in 0..m {
            let (ja, jb) = (jogs[moving[a]], jogs[moving[b]]);
            if a != b && ja.top == jb.bottom {
                before[a][b] = true;
            }
        }
    }
    for a in 0..m {
        for b in 0..m {
            let (ja, jb) = (jogs[moving[a]], jogs[moving[b]]);
            let prefer = a != b
                && (jb.strictly_inside(ja.top) || ja.strictly_inside(jb.bottom))
                && !(ja.strictly_inside(jb.top) || jb.strictly_inside(ja.bottom));
            if prefer && !before[b][a] {
                before[a][b] = true;
            }
        }
    }
    let order = topological(&before);
    let mut track = vec![None; jogs.len()];
    let mut rows: Vec<Vec<Jog>> = Vec::new();
    let mut assigned = vec![None; m];
    for &a in &order {
        let j = jogs[moving[a]];
        let min = (0..m)
            .filter(|&p| before[p][a])
            .filter_map(|p| assigned[p])
            .map(|t: usize| t + 1)
            .max()
            .unwrap_or(0);
        let t = (min..)
            .find(|&t| {
                rows.get(t)
                    .is_none_or(|row| row.iter().all(|o| o.hi() < j.lo() || j.hi() < o.lo()))
            })
            .expect("an empty track exists");
        if rows.len() <= t {
            rows.resize(t + 1, Vec::new());
        }
        rows[t].push(j);
        assigned[a] = Some(t);
        track[moving[a]] = Some(t);
    }
    (track, rows.len())
}

/// A topological order of `before`, taking the lowest index first; on a cycle, the remaining
/// node with the fewest remaining predecessors goes next.
fn topological(before: &[Vec<bool>]) -> Vec<usize> {
    let m = before.len();
    let mut done = vec![false; m];
    let mut order = Vec::with_capacity(m);
    while order.len() < m {
        let waiting = |b: usize| (0..m).filter(|&a| !done[a] && before[a][b]).count();
        let next = (0..m)
            .filter(|&b| !done[b])
            .min_by_key(|&b| (waiting(b), b))
            .expect("a remaining node");
        done[next] = true;
        order.push(next);
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jog(top: i32, bottom: i32) -> Jog {
        Jog { top, bottom }
    }

    #[test]
    fn straight_jogs_need_no_track() {
        assert_eq!(tracks(&[jog(3, 3)]), (vec![None], 0));
        assert_eq!(tracks(&[]), (vec![], 0));
    }

    #[test]
    fn disjoint_jogs_share_a_track() {
        assert_eq!(tracks(&[jog(0, 4), jog(6, 9)]), (vec![Some(0), Some(0)], 1));
    }

    #[test]
    fn overlapping_jogs_get_separate_tracks() {
        let (t, n) = tracks(&[jog(0, 6), jog(4, 9)]);
        assert_eq!(n, 2);
        assert_ne!(t[0], t[1]);
    }

    #[test]
    fn shared_column_turns_first() {
        // Jog 1 ends where jog 0 starts, so jog 0 must turn above it.
        let (t, _) = tracks(&[jog(5, 0), jog(9, 5)]);
        assert!(t[0] < t[1]);
        let (t, _) = tracks(&[jog(9, 5), jog(5, 0)]);
        assert!(t[1] < t[0]);
    }

    #[test]
    fn avoids_crossing_a_stub() {
        // Both go right: jog 1 starts inside jog 0's span, so it turns first.
        let (t, _) = tracks(&[jog(0, 4), jog(2, 8)]);
        assert!(t[1] < t[0]);
        // Both go left: jog 0 starts inside jog 1's span, so it turns first.
        let (t, _) = tracks(&[jog(4, 0), jog(8, 2)]);
        assert!(t[0] < t[1]);
        // Jogs that must cross get no preference.
        let (t, _) = tracks(&[jog(4, 2), jog(0, 8)]);
        assert_eq!(t, [Some(0), Some(1)]);
    }

    #[test]
    fn swapped_columns_still_get_tracks() {
        // A cycle of hard constraints: one overlap is unavoidable.
        let (t, n) = tracks(&[jog(0, 5), jog(5, 0)]);
        assert_eq!(n, 2);
        assert!(t[0].is_some() && t[1].is_some());
    }
}
