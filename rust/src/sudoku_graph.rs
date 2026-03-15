/// Sudoku graph: 81 vertices, each with degree 20.
/// Cell v = 9*r + c. Adjacent iff same row, column, or 3x3 box.
use crate::bitset::iter_bits;
/// Row index for each cell.
pub const ROW_OF: [u8; 81] = {
    let mut arr = [0u8; 81];
    let mut v = 0;
    while v < 81 {
        arr[v] = (v / 9) as u8;
        v += 1;
    }
    arr
};

/// Column index for each cell.
pub const COL_OF: [u8; 81] = {
    let mut arr = [0u8; 81];
    let mut v = 0;
    while v < 81 {
        arr[v] = (v % 9) as u8;
        v += 1;
    }
    arr
};

/// Box index (0..9) for each cell.
pub const BOX_OF: [u8; 81] = {
    let mut arr = [0u8; 81];
    let mut v = 0;
    while v < 81 {
        let r = v / 9;
        let c = v % 9;
        arr[v] = ((r / 3) * 3 + c / 3) as u8;
        v += 1;
    }
    arr
};

/// Precomputed neighbor bitmasks. neighbors_mask[v] has bit i set iff cell i
/// is adjacent to cell v in the sudoku graph.
pub static NEIGHBORS_MASK: [u128; 81] = {
    // First build row/col/box groups, then compute masks.
    let mut masks = [0u128; 81];

    // For each pair of cells, check adjacency.
    let mut u = 0;
    while u < 81 {
        let mut v = u + 1;
        while v < 81 {
            let same_row = (u / 9) == (v / 9);
            let same_col = (u % 9) == (v % 9);
            let same_box = ((u / 9) / 3 == (v / 9) / 3) && ((u % 9) / 3 == (v % 9) / 3);
            if same_row || same_col || same_box {
                masks[u] |= 1u128 << v;
                masks[v] |= 1u128 << u;
            }
            v += 1;
        }
        u += 1;
    }
    masks
};

/// Return (row, col, box) for a cell index 0..80.
#[inline]
pub fn cell_to_rcb(v: u8) -> (u8, u8, u8) {
    (ROW_OF[v as usize], COL_OF[v as usize], BOX_OF[v as usize])
}

/// Check whether two cells are adjacent in the sudoku graph.
#[inline]
pub fn is_adjacent(u: u8, v: u8) -> bool {
    (NEIGHBORS_MASK[u as usize] >> v) & 1 != 0
}

/// Build local adjacency masks for an induced subgraph.
///
/// Given a list of global cell indices, returns a vector of u16 masks
/// where bit j is set iff local vertex j is adjacent to local vertex i
/// in the induced subgraph. Local indices follow the order of `vertices`.
pub fn induced_subgraph(vertices: &[u8]) -> Vec<u16> {
    let n = vertices.len();
    debug_assert!(n <= 16, "induced_subgraph supports at most 16 vertices");
    let mut adj = vec![0u16; n];
    for i in 0..n {
        let vi = vertices[i] as usize;
        for j in (i + 1)..n {
            let vj = vertices[j] as usize;
            if (NEIGHBORS_MASK[vi] >> vj) & 1 != 0 {
                adj[i] |= 1 << j;
                adj[j] |= 1 << i;
            }
        }
    }
    adj
}

/// Build the global mask for a set of cells.
#[inline]
pub fn cells_to_mask(vertices: &[u8]) -> u128 {
    let mut mask = 0u128;
    for &v in vertices {
        mask |= 1u128 << v;
    }
    mask
}

/// Convert a global 81-cell mask to a list of cell indices.
pub fn mask_to_cells(mask: u128) -> Vec<u8> {
    iter_bits(mask).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitset::popcount;

    #[test]
    fn cell_0_has_degree_20() {
        assert_eq!(popcount(NEIGHBORS_MASK[0]), 20);
    }

    #[test]
    fn all_cells_degree_20() {
        for v in 0..81 {
            assert_eq!(
                popcount(NEIGHBORS_MASK[v]),
                20,
                "cell {} has degree {}",
                v,
                popcount(NEIGHBORS_MASK[v])
            );
        }
    }

    #[test]
    fn total_edges() {
        let total: u32 = (0..81).map(|v| popcount(NEIGHBORS_MASK[v])).sum();
        // Each edge counted twice: 810 edges * 2 = 1620
        assert_eq!(total, 1620);
    }

    #[test]
    fn adjacency_symmetric() {
        for u in 0..81u8 {
            for v in 0..81u8 {
                assert_eq!(
                    is_adjacent(u, v),
                    is_adjacent(v, u),
                    "asymmetric: ({}, {})",
                    u,
                    v
                );
            }
        }
    }

    #[test]
    fn no_self_loops() {
        for v in 0..81u8 {
            assert!(!is_adjacent(v, v));
        }
    }

    #[test]
    fn induced_triangle() {
        // Cells 0, 1, 2 are in the same row and same box -> all adjacent
        let adj = induced_subgraph(&[0, 1, 2]);
        assert_eq!(adj, vec![0b110, 0b101, 0b011]);
    }

    #[test]
    fn box_of_correct() {
        // Cell 0 = (0,0) -> box 0
        assert_eq!(BOX_OF[0], 0);
        // Cell 80 = (8,8) -> box 8
        assert_eq!(BOX_OF[80], 8);
        // Cell 30 = (3,3) -> box 4
        assert_eq!(BOX_OF[30], 4);
    }
}
