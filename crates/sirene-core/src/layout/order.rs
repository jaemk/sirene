//! Ordering items within ranks to reduce crossings (LAYOUT-5).

const SWEEPS: usize = 12;

/// Reorders `layers` (element ids per layer) in place. `segments` connect an element to one in
/// the next layer, as `(upper, lower)`; `elements` is the number of element ids.
pub(super) fn order(layers: &mut [Vec<usize>], segments: &[(usize, usize)], elements: usize) {
    let mut up = vec![Vec::new(); elements];
    let mut down = vec![Vec::new(); elements];
    for &(u, v) in segments {
        down[u].push(v);
        up[v].push(u);
    }
    let mut pos = vec![0usize; elements];
    index(layers, &mut pos);
    let mut best = layers.to_vec();
    let mut best_crossings = crossings(layers, &down, &pos);
    for sweep in 0..SWEEPS {
        if best_crossings == 0 {
            break;
        }
        if sweep % 2 == 0 {
            for layer in layers.iter_mut().skip(1) {
                reorder(layer, &up, &mut pos);
            }
        } else {
            for layer in layers.iter_mut().rev().skip(1) {
                reorder(layer, &down, &mut pos);
            }
        }
        let c = crossings(layers, &down, &pos);
        if c < best_crossings {
            best_crossings = c;
            best = layers.to_vec();
        }
    }
    layers.clone_from_slice(&best);
}

fn index(layers: &[Vec<usize>], pos: &mut [usize]) {
    for layer in layers {
        for (i, &e) in layer.iter().enumerate() {
            pos[e] = i;
        }
    }
}

/// Sorts a layer by the mean position of each element's neighbors. Elements without neighbors
/// keep their slot.
fn reorder(layer: &mut [usize], adj: &[Vec<usize>], pos: &mut [usize]) {
    let mut movable: Vec<(f64, usize, usize)> = layer
        .iter()
        .enumerate()
        .filter(|(_, e)| !adj[**e].is_empty())
        .map(|(i, &e)| {
            let sum: usize = adj[e].iter().map(|&n| pos[n]).sum();
            (sum as f64 / adj[e].len() as f64, i, e)
        })
        .collect();
    movable.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut next = movable.into_iter();
    for slot in layer.iter_mut() {
        if !adj[*slot].is_empty() {
            *slot = next.next().expect("one movable element per movable slot").2;
        }
    }
    for (i, &e) in layer.iter().enumerate() {
        pos[e] = i;
    }
}

/// Number of segment crossings between consecutive layers.
fn crossings(layers: &[Vec<usize>], down: &[Vec<usize>], pos: &[usize]) -> usize {
    let mut total = 0;
    for layer in layers {
        let pairs: Vec<(usize, usize)> = layer
            .iter()
            .flat_map(|&u| down[u].iter().map(move |&v| (u, v)))
            .map(|(u, v)| (pos[u], pos[v]))
            .collect();
        for (i, a) in pairs.iter().enumerate() {
            for b in &pairs[i + 1..] {
                if (a.0 < b.0 && a.1 > b.1) || (a.0 > b.0 && a.1 < b.1) {
                    total += 1;
                }
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untangles_a_cross() {
        // 0 1 / 2 3 with 0->3 and 1->2.
        let mut layers = vec![vec![0, 1], vec![2, 3]];
        order(&mut layers, &[(0, 3), (1, 2)], 4);
        assert_eq!(layers, [vec![0, 1], vec![3, 2]]);
    }

    #[test]
    fn keeps_order_without_crossings() {
        let mut layers = vec![vec![0, 1], vec![2, 3]];
        order(&mut layers, &[(0, 2), (1, 3)], 4);
        assert_eq!(layers, [vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn unconnected_elements_keep_their_slot() {
        // 4 has no neighbors and stays in the middle.
        let mut layers = vec![vec![0, 1], vec![2, 4, 3]];
        order(&mut layers, &[(0, 3), (1, 2)], 5);
        assert_eq!(layers[1][1], 4);
        assert_eq!(layers, [vec![0, 1], vec![3, 4, 2]]);
    }

    #[test]
    fn upward_sweep_reorders_top_layer() {
        // Reordering the lower layer cannot remove the crossing; reordering the top one can.
        let mut layers = vec![vec![0, 1, 2], vec![3, 4]];
        order(&mut layers, &[(0, 4), (1, 3), (2, 4)], 5);
        assert_eq!(layers, [vec![1, 0, 2], vec![3, 4]]);
    }
}
