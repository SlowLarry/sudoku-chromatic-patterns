/// Canonicalization using nauty via colored auxiliary graph.
///
/// Auxiliary graph (108 vertices):
///   - 81 cell vertices (0..80)
///   - 9 row nodes (81..89)
///   - 9 column nodes (90..98)
///   - 9 box nodes (99..107)
///
/// Edges: each cell connects to its row, column, and box node.
///
/// Vertex coloring (4 classes):
///   0: row + col nodes (merged for transpose symmetry)
///   1: box nodes
///   2: selected cells
///   3: unselected cells
///
/// This captures the full 3,359,232-element sudoku symmetry group.

use std::os::raw::c_int;

use nauty_Traces_sys::*;

use crate::bitset::iter_bits;

const AUX_N: usize = 108;
const ROW_OFFSET: usize = 81;
const COL_OFFSET: usize = 90;
const BOX_OFFSET: usize = 99;

/// Precomputed static adjacency for the auxiliary graph.
/// aux_adj[v] = list of neighbors of vertex v.
struct AuxAdj {
    /// For each vertex, its neighbors in the auxiliary graph.
    /// Cell vertices connect to 3 structure nodes; structure nodes connect to 9 cells.
    edges: [[u8; 9]; AUX_N],
    degrees: [u8; AUX_N],
}

impl AuxAdj {
    const fn build() -> Self {
        let mut edges = [[0u8; 9]; AUX_N];
        let mut degrees = [0u8; AUX_N];

        let mut cell = 0usize;
        while cell < 81 {
            let r = cell / 9;
            let c = cell % 9;
            let b = (r / 3) * 3 + c / 3;

            let row_node = ROW_OFFSET + r;
            let col_node = COL_OFFSET + c;
            let box_node = BOX_OFFSET + b;

            // Cell -> structure nodes
            edges[cell][0] = row_node as u8;
            edges[cell][1] = col_node as u8;
            edges[cell][2] = box_node as u8;
            degrees[cell] = 3;

            // Structure nodes -> cell
            let rd = degrees[row_node] as usize;
            edges[row_node][rd] = cell as u8;
            degrees[row_node] += 1;

            let cd = degrees[col_node] as usize;
            edges[col_node][cd] = cell as u8;
            degrees[col_node] += 1;

            let bd = degrees[box_node] as usize;
            edges[box_node][bd] = cell as u8;
            degrees[box_node] += 1;

            cell += 1;
        }
        AuxAdj { edges, degrees }
    }
}

static AUX: AuxAdj = AuxAdj::build();

/// Compute the number of setwords needed for n vertices.
#[inline]
fn setwords_needed(n: usize) -> usize {
    SETWORDSNEEDED(n)
}

/// Build the nauty dense graph + partition for a given chosen_mask.
/// Returns (graph, lab, ptn, m) where m = setwords per row.
fn build_nauty_graph(chosen_mask: u128) -> (Vec<setword>, Vec<c_int>, Vec<c_int>, usize) {
    let m = setwords_needed(AUX_N);
    let mut g = empty_graph(m, AUX_N);

    // Add edges from precomputed adjacency
    for v in 0..AUX_N {
        let deg = AUX.degrees[v] as usize;
        for i in 0..deg {
            let u = AUX.edges[v][i] as usize;
            if u > v {
                ADDONEEDGE(&mut g, v, u, m);
            }
        }
    }

    // Build partition: lab and ptn arrays.
    // Partition order: [selected cells | unselected cells | row+col nodes | box nodes]
    // Within each class, vertices sorted ascending.
    // ptn[i] = 1 within class, 0 at end of class.
    let mut lab = vec![0 as c_int; AUX_N];
    let mut ptn = vec![0 as c_int; AUX_N];

    let mut pos = 0;

    // Class 0: row + col nodes (81..98)
    let rc_start = pos;
    for v in ROW_OFFSET..BOX_OFFSET {
        lab[pos] = v as c_int;
        pos += 1;
    }
    // Set ptn: 1 within class, 0 at boundary
    for i in rc_start..pos - 1 {
        ptn[i] = 1;
    }
    ptn[pos - 1] = 0;

    // Class 1: box nodes (99..107)
    let box_start = pos;
    for v in BOX_OFFSET..AUX_N {
        lab[pos] = v as c_int;
        pos += 1;
    }
    for i in box_start..pos - 1 {
        ptn[i] = 1;
    }
    ptn[pos - 1] = 0;

    // Class 2: selected cells
    let sel_start = pos;
    for v in iter_bits(chosen_mask) {
        lab[pos] = v as c_int;
        pos += 1;
    }
    if pos > sel_start {
        for i in sel_start..pos - 1 {
            ptn[i] = 1;
        }
        ptn[pos - 1] = 0;
    }

    // Class 3: unselected cells
    let unsel_start = pos;
    let all_cells = (1u128 << 81) - 1;
    let unselected = all_cells & !chosen_mask;
    for v in iter_bits(unselected) {
        lab[pos] = v as c_int;
        pos += 1;
    }
    if pos > unsel_start {
        for i in unsel_start..pos - 1 {
            ptn[i] = 1;
        }
        ptn[pos - 1] = 0;
    }

    debug_assert_eq!(pos, AUX_N);

    (g, lab, ptn, m)
}

/// Canonical certificate for a chosen pattern (given as a bitmask of cells).
/// Two patterns are equivalent under sudoku symmetry iff their certificates match.
pub fn canonical_signature(chosen_mask: u128) -> Vec<setword> {
    let (mut g, mut lab, mut ptn, m) = build_nauty_graph(chosen_mask);
    let mut orbits = vec![0 as c_int; AUX_N];
    let mut canong = vec![0 as setword; m * AUX_N];

    let mut options = optionblk::default();
    options.getcanon = TRUE;
    options.defaultptn = FALSE;
    let mut stats = statsblk::default();

    unsafe {
        densenauty(
            g.as_mut_ptr(),
            lab.as_mut_ptr(),
            ptn.as_mut_ptr(),
            orbits.as_mut_ptr(),
            &mut options,
            &mut stats,
            m as c_int,
            AUX_N as c_int,
            canong.as_mut_ptr(),
        );
    }

    canong
}

/// Compute orbit representatives for candidate cells under the automorphism
/// group of the current chosen set.
///
/// Returns (reps, trivial) where:
///   - reps: one representative per orbit of candidates
///   - trivial: true if the automorphism group is trivial (order 1)
pub fn candidate_orbit_reps(chosen_mask: u128, candidates: &[u8]) -> (Vec<u8>, bool) {
    if candidates.is_empty() {
        return (vec![], true);
    }

    let (mut g, mut lab, mut ptn, m) = build_nauty_graph(chosen_mask);
    let mut orbits = vec![0 as c_int; AUX_N];

    let mut options = optionblk::default();
    options.getcanon = FALSE;
    options.defaultptn = FALSE;
    let mut stats = statsblk::default();

    unsafe {
        densenauty(
            g.as_mut_ptr(),
            lab.as_mut_ptr(),
            ptn.as_mut_ptr(),
            orbits.as_mut_ptr(),
            &mut options,
            &mut stats,
            m as c_int,
            AUX_N as c_int,
            std::ptr::null_mut(),
        );
    }

    let trivial = stats.grpsize1 == 1.0 && stats.grpsize2 == 0;

    let mut seen_orbits = std::collections::HashSet::new();
    let mut reps = Vec::new();
    for &v in candidates {
        let oid = orbits[v as usize];
        if seen_orbits.insert(oid) {
            reps.push(v);
        }
    }

    (reps, trivial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_deterministic() {
        // Same pattern always gives same certificate
        let mask = (1u128 << 0) | (1u128 << 1) | (1u128 << 9);
        let sig1 = canonical_signature(mask);
        let sig2 = canonical_signature(mask);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn equivalent_patterns_same_signature() {
        // Cell 0 = (0,0), Cell 10 = (1,1): both in box 0
        // {0, 1, 9} and {10, 11, 19}: should be equivalent under sudoku symmetry
        // Both are {(r,c), (r,c+1), (r+1,c)} in same box
        let sig1 = canonical_signature((1 << 0) | (1 << 1) | (1 << 9));
        let sig2 = canonical_signature((1 << 10) | (1 << 11) | (1 << 19));
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn different_patterns_different_signature() {
        // Very different patterns should differ
        let sig1 = canonical_signature(1 << 0);
        let sig2 = canonical_signature((1 << 0) | (1 << 1));
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn orbit_reps_single() {
        let mask = 1u128 << 0;
        let (reps, _) = candidate_orbit_reps(mask, &[1, 9]);
        assert!(!reps.is_empty());
    }
}
