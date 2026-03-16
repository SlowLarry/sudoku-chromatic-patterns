/// T&E (Trial & Error) depth computation for 3-coloring of small graphs.
///
/// T&E depth measures the minimum nesting depth of tentative color assignments
/// needed to prove a graph is not 3-colorable:
///
///   T&E(0): Constraint propagation alone reaches contradiction.
///   T&E(n): By tentatively assigning colors and showing contradiction via
///           T&E(n-1), we can eliminate colors. Repeating at each nesting
///           level eventually eliminates all possible 3-colorings.
///
/// Propagation includes:
///   - Naked singles: vertex with one color left → remove from neighbors
///   - Triangle all-different: in any 3-clique with 3 colors, each vertex
///     gets a distinct color. This gives hidden singles and naked pairs.

use crate::sudoku_graph;

/// Domain bitmask: bit 0 = color 0, bit 1 = color 1, bit 2 = color 2.
/// Full domain = 0b111 = 7.
type Domain = u8;

const FULL: Domain = 7;

#[inline]
fn dom_count(d: Domain) -> u32 {
    (d & 7).count_ones()
}

/// Unit propagation for 3-coloring domains.
/// When a vertex has exactly one color left (singleton), remove that color
/// from all neighbors. Repeat until fixpoint.
/// Returns true if a contradiction (empty domain) is found.
#[inline]
fn propagate_singles(doms: &mut [Domain], adj: &[u16]) -> bool {
    let n = doms.len();
    let mut queue: u16 = 0;
    for v in 0..n {
        if doms[v] != 0 && dom_count(doms[v]) == 1 {
            queue |= 1 << v;
        }
    }

    while queue != 0 {
        let v = queue.trailing_zeros() as usize;
        queue &= queue - 1;
        let d = doms[v];
        if d == 0 {
            return true;
        }
        if dom_count(d) != 1 {
            continue;
        }
        let mut nbrs = adj[v];
        while nbrs != 0 {
            let u = nbrs.trailing_zeros() as usize;
            nbrs &= nbrs - 1;
            if doms[u] & d != 0 {
                doms[u] &= !d;
                if doms[u] == 0 {
                    return true;
                }
                if dom_count(doms[u]) == 1 {
                    queue |= 1 << u;
                }
            }
        }
    }
    false
}

/// Full constraint propagation: naked singles + triangle all-different.
/// Loops until joint fixpoint.
/// Returns true if a contradiction is found.
fn propagate(doms: &mut [Domain], adj: &[u16], triangles: &[[usize; 3]]) -> bool {
    loop {
        if propagate_singles(doms, adj) {
            return true;
        }

        let mut changed = false;
        for tri in triangles {
            let [a, b, c] = *tri;

            // Hidden singles: if a color can only go in one vertex → assign
            for color in [1u8, 2, 4] {
                let in_a = (doms[a] & color) != 0;
                let in_b = (doms[b] & color) != 0;
                let in_c = (doms[c] & color) != 0;
                let count = in_a as u8 + in_b as u8 + in_c as u8;

                if count == 0 {
                    return true; // no vertex can take this color → contradiction
                }
                if count == 1 {
                    let v = if in_a { a } else if in_b { b } else { c };
                    if dom_count(doms[v]) > 1 {
                        doms[v] = color;
                        changed = true;
                    }
                }
            }

            // Naked pairs: two vertices share same 2-element domain → third gets remainder
            let verts = [a, b, c];
            for i in 0..3 {
                let j = (i + 1) % 3;
                let k = (i + 2) % 3;
                let vi = verts[i];
                let vj = verts[j];
                let vk = verts[k];
                if dom_count(doms[vi]) == 2 && doms[vi] == doms[vj] {
                    let remainder = FULL & !doms[vi];
                    let new_dom = doms[vk] & remainder;
                    if new_dom != doms[vk] {
                        doms[vk] = new_dom;
                        changed = true;
                        if new_dom == 0 {
                            return true;
                        }
                    }
                }
            }
        }

        if !changed {
            return false;
        }
    }
}

/// Find all triangles in the induced subgraph.
fn find_triangles(adj: &[u16]) -> Vec<[usize; 3]> {
    let n = adj.len();
    let mut tris = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            if adj[i] & (1 << j) == 0 {
                continue;
            }
            // k must be adjacent to both i and j
            let common = adj[i] & adj[j];
            let mut mask = common >> (j + 1);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                tris.push([i, j, j + 1 + bit]);
                mask &= mask - 1;
            }
        }
    }
    tris
}

/// Test whether non-3-colorability can be proved at T&E nesting depth `depth`.
fn te_solve(doms: &mut [Domain], adj: &[u16], triangles: &[[usize; 3]], depth: u32) -> bool {
    if propagate(doms, adj, triangles) {
        return true;
    }
    if depth == 0 {
        return false;
    }

    let n = doms.len();
    loop {
        let mut changed = false;

        for v in 0..n {
            if dom_count(doms[v]) <= 1 {
                continue;
            }

            for color in [1u8, 2, 4] {
                if doms[v] & color == 0 {
                    continue;
                }
                let mut trial: [Domain; 16] = [0; 16];
                trial[..n].copy_from_slice(&doms[..n]);
                trial[v] = color;
                if te_solve(&mut trial[..n], adj, triangles, depth - 1) {
                    doms[v] &= !color;
                    changed = true;
                    if doms[v] == 0 {
                        return true;
                    }
                    if propagate(doms, adj, triangles) {
                        return true;
                    }
                }
            }
        }

        if !changed {
            return false;
        }
    }
}

/// Compute the T&E depth of the induced subgraph on the given cells.
/// Returns the minimum depth d such that T&E(d) proves non-3-colorability.
/// Returns -1 if max_depth is exceeded.
pub fn compute_te_depth(cells: &[u8], max_depth: u32) -> i32 {
    let adj = sudoku_graph::induced_subgraph(cells);
    let n = adj.len();
    let triangles = find_triangles(&adj);
    let base_doms = [FULL; 16];

    for d in 0..=max_depth {
        let mut doms = [0u8; 16];
        doms[..n].copy_from_slice(&base_doms[..n]);
        if te_solve(&mut doms[..n], &adj, &triangles, d) {
            return d as i32;
        }
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_is_3colorable() {
        let adj: Vec<u16> = vec![0b110, 0b101, 0b011];
        let triangles = find_triangles(&adj);
        let mut doms = [FULL; 3];
        assert!(!te_solve(&mut doms, &adj, &triangles, 5));
    }

    #[test]
    fn test_k4_needs_te1() {
        let adj: Vec<u16> = vec![0b1110, 0b1101, 0b1011, 0b0111];
        let triangles = find_triangles(&adj);
        let n = 4;
        // T&E(0): all domains full, no propagation triggers.
        let mut doms0 = [FULL; 4];
        assert!(!te_solve(&mut doms0[..n], &adj, &triangles, 0));
        // T&E(1): try a color, propagation cascades to contradiction.
        let mut doms1 = [FULL; 4];
        assert!(te_solve(&mut doms1[..n], &adj, &triangles, 1));
    }
}
