//! Track assignment for the cross-axis part of routes in a gap between ranks (LAYOUT-8).

use std::collections::{BTreeSet, HashMap};

/// A route segment through a gap, entering at cross position `top` and leaving at `bottom`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Jog {
    pub top: i32,
    pub bottom: i32,
    /// A flat edge's: it enters and leaves through the top of the gap, `bottom` being where it
    /// goes back up.
    pub flat: bool,
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
    let after = if m <= PREFER_LIMIT {
        preferred(jogs, &moving)
    } else {
        same_column(jogs, &moving)
    };
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); m];
    for (a, next) in after.iter().enumerate() {
        for &b in next {
            before[b].push(a);
        }
    }
    let order = topological(&after, &before);
    let mut track = vec![None; jogs.len()];
    let mut rows: Vec<Vec<Jog>> = Vec::new();
    let mut assigned = vec![None; m];
    for &a in &order {
        let j = jogs[moving[a]];
        let min = before[a]
            .iter()
            .filter_map(|&p| assigned[p])
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

/// The most moving jogs in one gap for which every pair is checked for crossings; past it only
/// shared columns order the tracks, so a gap costs time linear in its jogs and their overlaps.
const PREFER_LIMIT: usize = 1_000;

/// For each moving jog, the ones that must take a higher track: those whose lower part shares
/// a column with one of its upper parts. A flat jog has two upper parts and no lower part.
fn same_column(jogs: &[Jog], moving: &[usize]) -> Vec<Vec<usize>> {
    let mut by_bottom: HashMap<i32, Vec<usize>> = HashMap::new();
    for (b, &i) in moving.iter().enumerate() {
        if !jogs[i].flat {
            by_bottom.entry(jogs[i].bottom).or_default().push(b);
        }
    }
    let below = |x: i32| by_bottom.get(&x).into_iter().flatten().copied();
    moving
        .iter()
        .map(|&i| {
            let j = jogs[i];
            let mut next: Vec<usize> = below(j.top).collect();
            if j.flat {
                next.extend(below(j.bottom));
            }
            next
        })
        .collect()
}

/// [`same_column`], plus a jog turning before it crosses another's cross-axis part, unless
/// the other must already come first.
fn preferred(jogs: &[Jog], moving: &[usize]) -> Vec<Vec<usize>> {
    let m = moving.len();
    let mut before = vec![vec![false; m]; m];
    for (a, next) in same_column(jogs, moving).into_iter().enumerate() {
        for b in next {
            before[a][b] = true;
        }
    }
    for a in 0..m {
        for b in 0..m {
            let (ja, jb) = (jogs[moving[a]], jogs[moving[b]]);
            let prefer = a != b
                && !ja.flat
                && !jb.flat
                && (jb.strictly_inside(ja.top) || ja.strictly_inside(jb.bottom))
                && !(ja.strictly_inside(jb.top) || jb.strictly_inside(ja.bottom));
            if prefer && !before[b][a] {
                before[a][b] = true;
            }
        }
    }
    before
        .iter()
        .map(|row| (0..m).filter(|&b| row[b]).collect())
        .collect()
}

/// A topological order of the relation `after` (with `before`, its inverse), taking the lowest
/// index first; on a cycle, the remaining node with the fewest remaining predecessors goes next.
fn topological(after: &[Vec<usize>], before: &[Vec<usize>]) -> Vec<usize> {
    let m = after.len();
    let mut waiting: Vec<usize> = before.iter().map(Vec::len).collect();
    let mut queue: BTreeSet<(usize, usize)> = (0..m).map(|b| (waiting[b], b)).collect();
    let mut order = Vec::with_capacity(m);
    while let Some((_, next)) = queue.pop_first() {
        order.push(next);
        for &b in &after[next] {
            if queue.remove(&(waiting[b], b)) {
                waiting[b] -= 1;
                queue.insert((waiting[b], b));
            }
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jog(top: i32, bottom: i32) -> Jog {
        Jog {
            top,
            bottom,
            flat: false,
        }
    }

    #[test]
    fn flat_jogs_turn_above_lower_parts_in_their_columns() {
        let flat = Jog {
            top: 0,
            bottom: 6,
            flat: true,
        };
        // Jog 1 comes down in the flat jog's second column, so the flat jog turns first.
        let (t, _) = tracks(&[jog(9, 6), flat]);
        assert!(t[1] < t[0], "{t:?}");
        let (t, _) = tracks(&[jog(3, 0), flat]);
        assert!(t[1] < t[0], "{t:?}");
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
