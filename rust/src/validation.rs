/// Validators for minimal 4-chromatic patterns.
use crate::bitset::{popcount16, iter_bits};
use crate::coloring::is_3_colorable;
use crate::sudoku_graph::{induced_subgraph, NEIGHBORS_MASK};

/// BFS connectivity test on local adjacency masks.
fn is_connected(adj_masks: &[u16]) -> bool {
    let n = adj_masks.len();
    if n <= 1 {
        return true;
    }
    let mut seen: u16 = 1; // start from vertex 0
    let mut frontier: u16 = 1;
    while frontier != 0 {
        let mut next: u16 = 0;
        let mut mask = frontier;
        while mask != 0 {
            let i = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            next |= adj_masks[i];
        }
        next &= !seen;
        seen |= next;
        frontier = next;
    }
    popcount16(seen) == n as u32
}

/// Check that the local graph contains no K4 (clique of size 4).
fn is_k4_free(adj_masks: &[u16]) -> bool {
    let n = adj_masks.len();
    for u in 0..n {
        let nbrs_u = adj_masks[u];
        // Only check v > u to avoid double-counting
        let mut vmask = nbrs_u & !((1u16 << (u + 1)) - 1);
        while vmask != 0 {
            let v = vmask.trailing_zeros() as usize;
            vmask &= vmask - 1;
            // Common neighbors of u and v
            let common = nbrs_u & adj_masks[v];
            if popcount16(common) < 2 {
                continue;
            }
            // Check if any two vertices in common are adjacent (forming K4)
            let mut cmask = common;
            while cmask != 0 {
                let w = cmask.trailing_zeros() as usize;
                cmask &= cmask - 1;
                // Any vertex in common that is > w and adjacent to w?
                let rest = cmask & adj_masks[w];
                if rest != 0 {
                    return false;
                }
            }
        }
    }
    true
}

/// Validate that a set of cells forms a minimal 4-chromatic K4-free pattern.
///
/// Checks (in order):
/// 1. Connected
/// 2. K4-free
/// 3. Minimum degree >= 3
/// 4. Not 3-colorable
/// 5. Deletion-critical: removing any one vertex makes it 3-colorable
pub fn is_valid_minimal_4chromatic_pattern(vertices: &[u8]) -> bool {
    let adj_masks = induced_subgraph(vertices);
    let n = adj_masks.len();
    if n == 0 {
        return false;
    }
    if !is_connected(&adj_masks) {
        return false;
    }
    if !is_k4_free(&adj_masks) {
        return false;
    }
    // Min degree >= 3
    if adj_masks.iter().any(|&m| popcount16(m) < 3) {
        return false;
    }
    // Must NOT be 3-colorable
    if is_3_colorable(&adj_masks) {
        return false;
    }
    // Deletion-critical: each single-vertex deletion must be 3-colorable
    for i in 0..n {
        let sub: Vec<u8> = vertices.iter().enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_j, &v)| v)
            .collect();
        let sub_adj = induced_subgraph(&sub);
        if !is_3_colorable(&sub_adj) {
            return false;
        }
    }
    true
}

/// Quick K4-free check: does adding vertex v to chosen_mask create a K4?
/// A K4 appears iff v's neighbors in chosen contain a triangle.
pub fn has_triangle_in_mask(mask: u128) -> bool {
    for u in iter_bits(mask) {
        let nbrs_u = NEIGHBORS_MASK[u as usize] & mask;
        if nbrs_u.count_ones() < 2 {
            continue;
        }
        for v in iter_bits(nbrs_u) {
            let others = nbrs_u & !((1u128) << v);
            if NEIGHBORS_MASK[v as usize] & others != 0 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudoku_graph::induced_subgraph;

    #[test]
    fn connected_triangle() {
        let adj = induced_subgraph(&[0, 1, 2]);
        assert!(is_connected(&adj));
    }

    #[test]
    fn disconnected() {
        // Cells 0 and 40 are not adjacent (different row, col, and box)
        let adj = induced_subgraph(&[0, 40]);
        assert!(!is_connected(&adj));
    }

    #[test]
    fn k4_free_triangle() {
        let adj = induced_subgraph(&[0, 1, 2]);
        assert!(is_k4_free(&adj));
    }

    #[test]
    fn not_k4_free() {
        // Cells 0,1,2,3 are all in row 0 -> all pairwise adjacent -> K4
        let adj = induced_subgraph(&[0, 1, 2, 3]);
        assert!(!is_k4_free(&adj));
    }

    #[test]
    fn known_n10_pattern() {
        // First pattern from results_n10_final.txt:
        // 110100000100010000000010000100000000011010000000000000000000000000000000000000000
        let bitstring = "110100000100010000000010000100000000011010000000000000000000000000000000000000000";
        let verts: Vec<u8> = bitstring
            .chars()
            .enumerate()
            .filter(|&(_, c)| c == '1')
            .map(|(i, _)| i as u8)
            .collect();
        assert_eq!(verts.len(), 10);
        assert!(is_valid_minimal_4chromatic_pattern(&verts));
    }
}
