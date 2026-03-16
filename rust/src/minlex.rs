/// Minlex canonicalization of patterns under the sudoku symmetry group.
///
/// The geometric sudoku symmetry group (no digit relabeling) has order
/// 2 × 6^4 × 6^4 = 3,359,232 and decomposes as:
///   - Transpose (2)
///   - Band permutations (3! = 6)
///   - Stack permutations (3! = 6)
///   - Row permutations within each band (3!^3 = 216)
///   - Col permutations within each stack (3!^3 = 216)
///
/// For each group element, we compose the cell permutation from this product
/// structure and apply it to the pattern, using early termination to skip
/// permutations that cannot improve the current minimum.

use std::sync::LazyLock;

/// All 6 permutations of {0, 1, 2}.
const PERMS3: [[u8; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// Precomputed inverse row/column maps for the sudoku symmetry group.
///
/// Each map is the inverse of a row (or column) permutation arising from
/// a band (or stack) permutation composed with per-band (or per-stack)
/// row (or column) permutations.
///
/// A forward map sends row r to: 3 * band_perm[r/3] + row_perm[r/3][r%3].
/// We store its inverse so that we can efficiently look up
/// "which original cell maps to output position w?" during minlexing.
///
/// Count: 6 (band perms) × 6^3 (row perms per band) = 1296 maps.
struct InvMaps {
    maps: Vec<[u8; 9]>,
}

static INV_MAPS: LazyLock<InvMaps> = LazyLock::new(|| {
    let mut maps = Vec::with_capacity(1296);
    for bp in &PERMS3 {
        for rp0 in &PERMS3 {
            for rp1 in &PERMS3 {
                for rp2 in &PERMS3 {
                    let rps = [rp0, rp1, rp2];
                    let mut fwd = [0u8; 9];
                    for r in 0..9usize {
                        let b = r / 3;
                        let i = r % 3;
                        fwd[r] = 3 * bp[b] + rps[b][i];
                    }
                    let mut inv = [0u8; 9];
                    for r in 0..9usize {
                        inv[fwd[r] as usize] = r as u8;
                    }
                    maps.push(inv);
                }
            }
        }
    }
    InvMaps { maps }
});

/// Compute the minlex (lexicographically smallest) canonical form of a pattern
/// under all 3,359,232 geometric sudoku symmetries.
///
/// The pattern is a u128 bitmask where bit i represents cell i
/// (row = i/9, col = i%9). Returns the lex-smallest morphed bitmask.
pub fn minlex_pattern(pattern: u128) -> u128 {
    let inv_maps = &INV_MAPS.maps;
    let mut min_mask: u128 = pattern;

    for transpose in 0..2u8 {
        for inv_rm in inv_maps {
            for inv_cm in inv_maps {
                if let Some(morphed) =
                    try_improve(pattern, min_mask, transpose, inv_rm, inv_cm)
                {
                    min_mask = morphed;
                }
            }
        }
    }

    min_mask
}

/// If applying the given symmetry produces a pattern lexicographically
/// smaller than `current_min`, return `Some(morphed_mask)`. Otherwise `None`.
///
/// Uses early termination: aborts as soon as the morphed pattern is
/// determined to be lex >= current_min at any position.
#[inline]
fn try_improve(
    pattern: u128,
    current_min: u128,
    transpose: u8,
    inv_rm: &[u8; 9],
    inv_cm: &[u8; 9],
) -> Option<u128> {
    let mut morphed: u128 = 0;
    let mut confirmed_better = false;

    for w in 0..81u8 {
        let wr = (w / 9) as usize;
        let wc = (w % 9) as usize;
        let original_cell = if transpose == 0 {
            9 * inv_rm[wr] + inv_cm[wc]
        } else {
            9 * inv_cm[wc] + inv_rm[wr]
        };
        let bit = (pattern >> original_cell) & 1;
        if bit != 0 {
            morphed |= 1u128 << w;
        }

        if !confirmed_better {
            let min_bit = (current_min >> w) & 1;
            if bit < min_bit {
                confirmed_better = true;
                // continue building the full morphed mask
            } else if bit > min_bit {
                return None; // worse, abort
            }
        }
    }

    if confirmed_better {
        Some(morphed)
    } else {
        None // equal to current min
    }
}

/// Convert a u128 bitmask to an 81-character '0'/'1' string (row-major).
pub fn mask_to_81str(mask: u128) -> String {
    let mut s = String::with_capacity(81);
    for i in 0..81 {
        if mask & (1u128 << i) != 0 {
            s.push('1');
        } else {
            s.push('0');
        }
    }
    s
}

/// Parse an 81-character '0'/'1' string into a u128 bitmask.
pub fn str81_to_mask(s: &str) -> Option<u128> {
    if s.len() != 81 {
        return None;
    }
    let mut mask: u128 = 0;
    for (i, ch) in s.chars().enumerate() {
        match ch {
            '1' => mask |= 1u128 << i,
            '0' => {}
            _ => return None,
        }
    }
    Some(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inv_maps_count() {
        assert_eq!(INV_MAPS.maps.len(), 1296);
    }

    #[test]
    fn test_inv_maps_are_permutations() {
        for inv in &INV_MAPS.maps {
            let mut seen = [false; 9];
            for &v in inv.iter() {
                assert!(v < 9);
                assert!(!seen[v as usize], "duplicate value in inv map");
                seen[v as usize] = true;
            }
        }
    }

    #[test]
    fn test_roundtrip_str() {
        let s = "110100000100000000000000000100100100000110000000000000010000000010000000010101000";
        assert_eq!(s.len(), 81);
        let mask = str81_to_mask(s).unwrap();
        let back = mask_to_81str(mask);
        assert_eq!(s, &back);
    }

    #[test]
    fn test_minlex_identity() {
        // The minlex of any pattern should be <= the pattern itself
        let pattern: u128 = (1 << 0) | (1 << 10) | (1 << 20);
        let ml = minlex_pattern(pattern);
        assert!(lex_le(ml, pattern));
    }

    #[test]
    fn test_minlex_symmetric_patterns() {
        // Transpose swaps (r,c) -> (c,r), so cell v=9r+c -> cell v'=9c+r.
        // Cells {0,1,3}: (0,0),(0,1),(0,3) -> under transpose -> (0,0),(1,0),(3,0) = cells {0,9,27}
        let p1: u128 = (1 << 0) | (1 << 1) | (1 << 3);
        let p2: u128 = (1 << 0) | (1 << 9) | (1 << 27);
        let ml1 = minlex_pattern(p1);
        let ml2 = minlex_pattern(p2);
        assert_eq!(ml1, ml2, "transposed patterns must have same minlex");
    }

    #[test]
    fn test_minlex_band_swap() {
        // Swap bands 0 and 1: rows 0-2 <-> rows 3-5
        // Cell (0,0)=0 -> (3,0)=27, cell (1,0)=9 -> (4,0)=36
        let p1: u128 = (1 << 0) | (1 << 9);  // cells 0, 9
        let p2: u128 = (1 << 27) | (1 << 36); // cells 27, 36
        let ml1 = minlex_pattern(p1);
        let ml2 = minlex_pattern(p2);
        assert_eq!(ml1, ml2, "band-swapped patterns must have same minlex");
    }

    #[test]
    fn test_minlex_is_canonical() {
        // Minlex of a minlex should be itself
        let pattern: u128 = (1 << 5) | (1 << 15) | (1 << 40) | (1 << 72);
        let ml = minlex_pattern(pattern);
        let ml2 = minlex_pattern(ml);
        assert_eq!(ml, ml2, "minlex must be idempotent");
    }

    fn lex_le(a: u128, b: u128) -> bool {
        if a == b {
            return true;
        }
        let diff = a ^ b;
        let first_diff = diff.trailing_zeros();
        (a >> first_diff) & 1 == 0
    }
}
