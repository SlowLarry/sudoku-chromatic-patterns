/// Exact 3-colorability solver for small graphs (DSATUR backtracking).
use crate::bitset::popcount16;

/// Return true if the graph (given as local adjacency masks) is 3-colorable.
///
/// Uses DSATUR heuristic: always color the uncolored vertex with the highest
/// saturation degree (most distinct neighbor colors), breaking ties by degree.
/// For patterns up to ~16 vertices this is very fast.
pub fn is_3_colorable(adj_masks: &[u16]) -> bool {
    let n = adj_masks.len();
    if n == 0 {
        return true;
    }
    debug_assert!(n <= 16);

    let mut colors = [u8::MAX; 16]; // MAX = uncolored
    let degrees: Vec<u32> = adj_masks.iter().map(|&m| popcount16(m)).collect();

    fn choose_vertex(
        n: usize,
        colors: &[u8; 16],
        adj_masks: &[u16],
        degrees: &[u32],
    ) -> (usize, u8) {
        let mut best = usize::MAX;
        let mut best_sat: i32 = -1;
        let mut best_deg: u32 = 0;
        let mut best_used: u8 = 0;
        for i in 0..n {
            if colors[i] != u8::MAX {
                continue;
            }
            let mut used: u8 = 0;
            let mut mask = adj_masks[i];
            while mask != 0 {
                let j = mask.trailing_zeros() as usize;
                mask &= mask - 1;
                if colors[j] != u8::MAX {
                    used |= 1 << colors[j];
                }
            }
            let sat = used.count_ones() as i32;
            if sat > best_sat || (sat == best_sat && degrees[i] > best_deg) {
                best = i;
                best_sat = sat;
                best_deg = degrees[i];
                best_used = used;
            }
        }
        (best, best_used)
    }

    fn backtrack(
        colored: usize,
        n: usize,
        colors: &mut [u8; 16],
        adj_masks: &[u16],
        degrees: &[u32],
    ) -> bool {
        if colored == n {
            return true;
        }
        let (v, used) = choose_vertex(n, colors, adj_masks, degrees);
        let available = (!used) & 0b111;
        if available == 0 {
            return false;
        }
        for color in 0..3u8 {
            if (available >> color) & 1 == 0 {
                continue;
            }
            colors[v] = color;
            if backtrack(colored + 1, n, colors, adj_masks, degrees) {
                return true;
            }
            colors[v] = u8::MAX;
        }
        false
    }

    backtrack(0, n, &mut colors, adj_masks, &degrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_colorable() {
        assert!(is_3_colorable(&[]));
    }

    #[test]
    fn single_vertex() {
        assert!(is_3_colorable(&[0]));
    }

    #[test]
    fn triangle_3colorable() {
        // K3: 0-1, 0-2, 1-2
        let adj = vec![0b110u16, 0b101, 0b011];
        assert!(is_3_colorable(&adj));
    }

    #[test]
    fn k4_not_3colorable() {
        // K4: all pairs adjacent among 4 vertices
        let adj = vec![0b1110u16, 0b1101, 0b1011, 0b0111];
        assert!(!is_3_colorable(&adj));
    }

    #[test]
    fn c5_3colorable() {
        // C5: 0-1, 1-2, 2-3, 3-4, 4-0
        let adj = vec![
            0b10010u16, // 0: adj to 1,4
            0b00101,    // 1: adj to 0,2
            0b01010,    // 2: adj to 1,3
            0b10100,    // 3: adj to 2,4
            0b01001,    // 4: adj to 3,0
        ];
        assert!(is_3_colorable(&adj));
    }

    #[test]
    fn w5_not_3colorable() {
        // W5: C4 + center vertex adjacent to all
        // Vertices: 0=center, 1,2,3,4 on rim
        // Center(0): adj 1,2,3,4
        // 1: adj 0,2,4
        // 2: adj 0,1,3
        // 3: adj 0,2,4
        // 4: adj 0,1,3
        let adj = vec![
            0b11110u16, // 0: 1,2,3,4
            0b10101,    // 1: 0,2,4
            0b01011,    // 2: 0,1,3
            0b10101,    // 3: 0,2,4
            0b01011,    // 4: 0,1,3
        ];
        // W5 (wheel on 5 vertices = K1 + C4): center needs a color,
        // C4 is bipartite and needs 2 colors, so total 3 colors suffice.
        // Actually W5 is 3-colorable. Let's verify:
        // 0=red, 1=blue, 2=green, 3=blue, 4=green ✓
        assert!(is_3_colorable(&adj));
    }

    #[test]
    fn grotzsch_not_3colorable() {
        // The Grötzsch graph is triangle-free and 4-chromatic (11 vertices).
        // Vertices 0-4: outer pentagon, 5-9: inner star, 10: center
        // Outer: 0-1, 1-2, 2-3, 3-4, 4-0
        // Star: 5→{1,2}, 6→{2,3}, 7→{3,4}, 8→{4,0}, 9→{0,1}
        // Center 10 → {5,6,7,8,9}
        let adj: Vec<u16> = vec![
            (1 << 1) | (1 << 4) | (1 << 8) | (1 << 9),           // 0
            (1 << 0) | (1 << 2) | (1 << 5) | (1 << 9),           // 1
            (1 << 1) | (1 << 3) | (1 << 5) | (1 << 6),           // 2
            (1 << 2) | (1 << 4) | (1 << 6) | (1 << 7),           // 3
            (1 << 3) | (1 << 0) | (1 << 7) | (1 << 8),           // 4
            (1 << 1) | (1 << 2) | (1 << 10),                     // 5
            (1 << 2) | (1 << 3) | (1 << 10),                     // 6
            (1 << 3) | (1 << 4) | (1 << 10),                     // 7
            (1 << 4) | (1 << 0) | (1 << 10),                     // 8
            (1 << 0) | (1 << 1) | (1 << 10),                     // 9
            (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 9), // 10
        ];
        assert!(!is_3_colorable(&adj));
    }
}
