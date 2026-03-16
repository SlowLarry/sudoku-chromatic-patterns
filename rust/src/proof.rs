//! Automated proof generation for non-3-colorability of patterns.
//!
//! Strategy:
//! 1. Greedily find and apply diamond (K₄−e) reductions: if vertices
//!    a, b share two common neighbors u, v that are adjacent, then
//!    color(a) = color(b) in any 3-coloring. Merge them.
//! 2. After each reduction, check for K₄ (contradiction).
//! 3. If no gadget applies, branch (reverse Hajós): pick non-adjacent u, v,
//!    case-split on color(u)=color(v) [merge] vs color(u)≠color(v) [add edge].
//!    Both branches must reach contradiction.

use crate::sudoku_graph::NEIGHBORS_MASK;

/// Format a cell as "r{row}c{col}" (1-based).
fn cell_name(cell: u8) -> String {
    format!("r{}c{}", cell / 9 + 1, cell % 9 + 1)
}

// ── Working graph ───────────────────────────────────────────────────

/// Mutable graph supporting vertex merging and edge addition.
/// Vertices are indices 0..n (some may be inactive after merges).
/// Adjacency stored as u32 bitmasks (supports up to 32 vertices).
#[derive(Clone)]
struct ProofGraph {
    adj: Vec<u32>,
    labels: Vec<Vec<u8>>, // original cell indices per vertex
    active: u32,
}

impl ProofGraph {
    fn from_cells(cells: &[u8]) -> Self {
        let n = cells.len();
        assert!(n <= 30, "pattern too large for proof graph");
        let mut adj = vec![0u32; n];
        for i in 0..n {
            for j in (i + 1)..n {
                if NEIGHBORS_MASK[cells[i] as usize] & (1u128 << cells[j]) != 0 {
                    adj[i] |= 1 << j;
                    adj[j] |= 1 << i;
                }
            }
        }
        let labels: Vec<Vec<u8>> = cells.iter().map(|&c| vec![c]).collect();
        let active = (1u32 << n) - 1;
        ProofGraph { adj, labels, active }
    }

    fn vertex_name(&self, v: usize) -> String {
        let cells = &self.labels[v];
        if cells.len() == 1 {
            cell_name(cells[0])
        } else {
            let mut names: Vec<String> = cells.iter().map(|&c| cell_name(c)).collect();
            names.sort();
            format!("[{}]", names.join("="))
        }
    }

    fn active_verts(&self) -> Vec<usize> {
        let mut v = Vec::new();
        let mut m = self.active;
        while m != 0 {
            v.push(m.trailing_zeros() as usize);
            m &= m - 1;
        }
        v
    }

    /// Merge vertex `remove` into vertex `keep`.
    fn merge(&mut self, keep: usize, remove: usize) {
        debug_assert_ne!(keep, remove);
        debug_assert!(self.active & (1 << keep) != 0);
        debug_assert!(self.active & (1 << remove) != 0);

        let r_nbrs = self.adj[remove] & self.active & !(1 << keep) & !(1 << remove);
        self.adj[keep] |= r_nbrs;
        self.adj[keep] &= !(1 << remove) & !(1 << keep); // no self-loop

        let mut mask = r_nbrs;
        while mask != 0 {
            let nb = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            self.adj[nb] &= !(1 << remove);
            if nb != keep {
                self.adj[nb] |= 1 << keep;
            }
        }
        // also clear remove from keep's original neighbors
        // (already done by &= !(1 << remove) above)

        let r_labels = std::mem::take(&mut self.labels[remove]);
        self.labels[keep].extend(r_labels);
        self.active &= !(1 << remove);
        self.adj[remove] = 0;
    }

    fn add_edge(&mut self, u: usize, v: usize) {
        self.adj[u] |= 1 << v;
        self.adj[v] |= 1 << u;
    }

    /// Find K₄ among active vertices.
    fn find_k4(&self) -> Option<[usize; 4]> {
        let verts = self.active_verts();
        let n = verts.len();
        for i in 0..n {
            let a = verts[i];
            let na = self.adj[a] & self.active;
            for j in (i + 1)..n {
                let b = verts[j];
                if na & (1 << b) == 0 { continue; }
                let nab = na & self.adj[b] & self.active;
                for k in (j + 1)..n {
                    let c = verts[k];
                    if nab & (1 << c) == 0 { continue; }
                    let nabc = nab & self.adj[c] & self.active;
                    for l in (k + 1)..n {
                        let d = verts[l];
                        if nabc & (1 << d) != 0 {
                            return Some([a, b, c, d]);
                        }
                    }
                }
            }
        }
        None
    }

    /// Find a diamond: four vertices where the two "tips" share two common
    /// neighbors (the "spine") that are adjacent, but the tips are not adjacent.
    /// Returns (tip_a, tip_b, spine_u, spine_v).
    fn find_diamond(&self) -> Option<(usize, usize, usize, usize)> {
        let verts = self.active_verts();
        let n = verts.len();
        for i in 0..n {
            let u = verts[i];
            let nu = self.adj[u] & self.active;
            for j in (i + 1)..n {
                let v = verts[j];
                if nu & (1 << v) == 0 { continue; }
                let common = nu & self.adj[v] & self.active & !((1 << u) | (1 << v));
                if common.count_ones() < 2 { continue; }
                // enumerate pairs in common
                let mut am = common;
                while am != 0 {
                    let a = am.trailing_zeros() as usize;
                    am &= am - 1;
                    let mut bm = am;
                    while bm != 0 {
                        let b = bm.trailing_zeros() as usize;
                        bm &= bm - 1;
                        if self.adj[a] & (1 << b) == 0 {
                            return Some((a, b, u, v));
                        }
                    }
                }
            }
        }
        None
    }

    /// Choose a non-adjacent pair for branching.
    /// Strongly prefer pairs where adding an edge creates K₄ (makes
    /// the "different color" branch resolve immediately).
    fn branch_candidates(&self) -> Vec<(usize, usize)> {
        let verts = self.active_verts();
        let n = verts.len();
        let mut pairs: Vec<(usize, usize, i32)> = Vec::new();

        for i in 0..n {
            let u = verts[i];
            for j in (i + 1)..n {
                let v = verts[j];
                if self.adj[u] & (1 << v) != 0 { continue; }
                let common = self.adj[u] & self.adj[v] & self.active;
                let common_count = common.count_ones() as i32;

                // Check if adding edge u-v creates K₄:
                // need two vertices in common that are also adjacent to each other
                let mut creates_k4 = false;
                let mut wm = common;
                while wm != 0 {
                    let w1 = wm.trailing_zeros() as usize;
                    wm &= wm - 1;
                    if self.adj[w1] & wm & common != 0 {
                        creates_k4 = true;
                        break;
                    }
                }

                let score = common_count + if creates_k4 { 1000 } else { 0 };
                pairs.push((u, v, score));
            }
        }

        // Sort by score descending (best candidates first → better pruning)
        pairs.sort_by(|a, b| b.2.cmp(&a.2));
        pairs.into_iter().map(|(u, v, _)| (u, v)).collect()
    }
}

// ── Proof tree ──────────────────────────────────────────────────────

/// A node in the proof tree.
pub enum ProofNode {
    /// Diamond found: tips must share color. Merge and continue.
    DiamondMerge {
        tip_a: String,
        tip_b: String,
        spine_u: String,
        spine_v: String,
        next: Box<ProofNode>,
    },
    /// Branch on two non-adjacent vertices (reverse Hajós).
    Branch {
        vertex_a: String,
        vertex_b: String,
        same_color: Box<ProofNode>,
        diff_color: Box<ProofNode>,
    },
    /// K₄ found — contradiction with 3-colorability.
    K4Contradiction {
        vertices: [String; 4],
    },
    /// Proof search exhausted depth limit.
    Failed,
}

impl ProofNode {
    pub fn is_complete(&self) -> bool {
        match self {
            ProofNode::K4Contradiction { .. } => true,
            ProofNode::DiamondMerge { next, .. } => next.is_complete(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.is_complete() && diff_color.is_complete()
            }
            ProofNode::Failed => false,
        }
    }

    pub fn depth(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::Failed => 0,
            ProofNode::DiamondMerge { next, .. } => 1 + next.depth(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                1 + same_color.depth().max(diff_color.depth())
            }
        }
    }

    pub fn branch_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::Failed => 0,
            ProofNode::DiamondMerge { next, .. } => next.branch_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                1 + same_color.branch_count() + diff_color.branch_count()
            }
        }
    }

    pub fn diamond_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::Failed => 0,
            ProofNode::DiamondMerge { next, .. } => 1 + next.diamond_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.diamond_count() + diff_color.diamond_count()
            }
        }
    }
    pub fn size(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } => 1,
            ProofNode::Failed => usize::MAX / 2,
            ProofNode::DiamondMerge { next, .. } => 1 + next.size(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                1 + same_color.size() + diff_color.size()
            }
        }
    }
}

// ── Proof search ────────────────────────────────────────────────────

/// Search for the shortest complete proof with at most `branches_left` branch
/// nodes and at most `size_budget` total proof nodes.
/// Diamonds are applied greedily; all branch-pair candidates are tried and the
/// one producing the smallest complete proof is kept.
fn find_best_proof(
    graph: &ProofGraph,
    branches_left: usize,
    depth_remaining: usize,
    size_budget: usize,
) -> ProofNode {
    if size_budget == 0 {
        return ProofNode::Failed;
    }

    // Terminal: K₄ found
    if let Some(k4) = graph.find_k4() {
        return ProofNode::K4Contradiction {
            vertices: k4.map(|v| graph.vertex_name(v)),
        };
    }

    if depth_remaining == 0 {
        return ProofNode::Failed;
    }

    // Greedy: apply diamond reduction
    if let Some((a, b, u, v)) = graph.find_diamond() {
        let tip_a = graph.vertex_name(a);
        let tip_b = graph.vertex_name(b);
        let spine_u = graph.vertex_name(u);
        let spine_v = graph.vertex_name(v);

        let mut g = graph.clone();
        let (keep, remove) = (a.min(b), a.max(b));
        g.merge(keep, remove);

        let next = find_best_proof(&g, branches_left, depth_remaining - 1, size_budget - 1);
        return ProofNode::DiamondMerge {
            tip_a,
            tip_b,
            spine_u,
            spine_v,
            next: Box::new(next),
        };
    }

    // Branch: try all non-adjacent pairs, keep the one with smallest complete proof
    if branches_left == 0 {
        return ProofNode::Failed;
    }

    let pairs = graph.branch_candidates();
    if pairs.is_empty() {
        return ProofNode::Failed;
    }

    let mut best: Option<ProofNode> = None;
    let mut best_total = size_budget;

    for (u, v) in pairs {
        let sub_budget = best_total - 1; // 1 for the branch node itself

        // Case 1: same color → merge
        let mut g_same = graph.clone();
        let (keep, remove) = (u.min(v), u.max(v));
        g_same.merge(keep, remove);
        let same_proof = find_best_proof(
            &g_same,
            branches_left - 1,
            depth_remaining - 1,
            sub_budget,
        );

        if !same_proof.is_complete() {
            continue;
        }

        let same_size = same_proof.size();
        let diff_budget = sub_budget.saturating_sub(same_size);
        if diff_budget == 0 {
            continue;
        }

        // Case 2: different colors → add edge
        let mut g_diff = graph.clone();
        g_diff.add_edge(u, v);
        let diff_proof = find_best_proof(
            &g_diff,
            branches_left - 1,
            depth_remaining - 1,
            diff_budget,
        );

        if !diff_proof.is_complete() {
            continue;
        }

        let total = 1 + same_size + diff_proof.size();
        if total < best_total {
            best_total = total;
            let name_a = graph.vertex_name(u);
            let name_b = graph.vertex_name(v);
            best = Some(ProofNode::Branch {
                vertex_a: name_a,
                vertex_b: name_b,
                same_color: Box::new(same_proof),
                diff_color: Box::new(diff_proof),
            });
        }
    }

    best.unwrap_or(ProofNode::Failed)
}

// ── Formatting ──────────────────────────────────────────────────────

fn format_node(node: &ProofNode, step: &mut usize, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match node {
        ProofNode::K4Contradiction { vertices } => {
            *step += 1;
            format!(
                "{}{}.  K₄ on {{{}, {}, {}, {}}}. Contradiction.\n",
                pad, step, vertices[0], vertices[1], vertices[2], vertices[3],
            )
        }
        ProofNode::DiamondMerge { tip_a, tip_b, spine_u, spine_v, next } => {
            *step += 1;
            let mut s = format!(
                "{}{}.  Diamond {{{}, {}, {}, {}}} (spine {}—{}).\n",
                pad, step, tip_a, spine_u, spine_v, tip_b, spine_u, spine_v,
            );
            s += &format!(
                "{}    → color({}) = color({}). Identify.\n",
                pad, tip_a, tip_b,
            );
            s += &format_node(next, step, indent);
            s
        }
        ProofNode::Branch { vertex_a, vertex_b, same_color, diff_color } => {
            *step += 1;
            let my_step = *step;
            let mut s = format!(
                "{}{}.  Branch on {}, {}:\n",
                pad, my_step, vertex_a, vertex_b,
            );
            s += &format!(
                "{}    Case A: color({}) = color({}). Identify.\n",
                pad, vertex_a, vertex_b,
            );
            s += &format_node(same_color, step, indent + 3);
            s += &format!(
                "{}    Case B: color({}) ≠ color({}). Add edge.\n",
                pad, vertex_a, vertex_b,
            );
            s += &format_node(diff_color, step, indent + 3);
            s += &format!("{}    Both cases contradict 3-colorability.\n", pad);
            s
        }
        ProofNode::Failed => {
            format!("{}    (proof search failed — depth limit reached)\n", pad)
        }
    }
}

pub fn format_proof(proof: &ProofNode) -> String {
    let mut step = 0;
    let mut s = String::from("Proof of non-3-colorability:\n");
    s += "  Assume for contradiction it is 3-colorable.\n\n";
    s += &format_node(proof, &mut step, 1);
    if proof.is_complete() {
        s += "\n  Therefore the pattern is not 3-colorable. □\n";
    }
    s
}

// ── Public API ──────────────────────────────────────────────────────

pub struct ProofResult {
    pub proof: ProofNode,
    pub text: String,
}

impl ProofResult {
    pub fn summary(&self) -> String {
        format!(
            "depth={} diamonds={} branches={} complete={}",
            self.proof.depth(),
            self.proof.diamond_count(),
            self.proof.branch_count(),
            self.proof.is_complete(),
        )
    }
}

pub fn prove_pattern(cells: &[u8]) -> ProofResult {
    let graph = ProofGraph::from_cells(cells);
    // Iterative deepening on branch count to find minimum-branch proof
    for max_br in 0..=10 {
        let proof = find_best_proof(&graph, max_br, 50, usize::MAX);
        if proof.is_complete() {
            let text = format_proof(&proof);
            return ProofResult { proof, text };
        }
    }
    // Fallback (should not happen for valid 4-chromatic patterns)
    let proof = ProofNode::Failed;
    let text = format_proof(&proof);
    ProofResult { proof, text }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells_from_bitstring(s: &str) -> Vec<u8> {
        s.chars()
            .enumerate()
            .filter(|&(_, c)| c == '1')
            .map(|(i, _)| i as u8)
            .collect()
    }

    #[test]
    fn test_proof_graph_from_cells() {
        // Triangle: cells 0, 1, 2 (row 0, cols 0-2, same row+box)
        let cells = vec![0, 1, 2];
        let g = ProofGraph::from_cells(&cells);
        assert_eq!(g.active.count_ones(), 3);
        assert!(g.adj[0] & (1 << 1) != 0); // 0-1 adjacent (same row)
        assert!(g.adj[0] & (1 << 2) != 0); // 0-2 adjacent (same row)
        assert!(g.adj[1] & (1 << 2) != 0); // 1-2 adjacent (same row)
    }

    #[test]
    fn test_find_k4() {
        // Cells 0,1,2,3 (row 0, cols 0-3): 0-1-2 in same box, 3 in same row.
        // 0-1: same row+box ✓, 0-2: same row+box ✓, 0-3: same row ✓
        // 1-2: same row+box ✓, 1-3: same row ✓, 2-3: same row ✓
        // All pairs adjacent → K₄
        let cells = vec![0, 1, 2, 3];
        let g = ProofGraph::from_cells(&cells);
        assert!(g.find_k4().is_some());
    }

    #[test]
    fn test_find_diamond() {
        // Need 4 cells where two share two common neighbors but aren't adjacent.
        // Cells: (0,0)=0, (0,1)=1, (1,0)=9, (1,1)=10
        // 0-1: same row+box ✓, 0-9: same col+box ✓, 0-10: same box ✓
        // 1-9: same box ✓, 1-10: same row? No (rows 0,1). Same col? No (cols 1,1→yes!
        // Wait: cell 1 = (0,1), cell 10 = (1,1). Same column? col 1 = col 1 → yes.
        // 9-10: same row+box ✓
        // So: 0-1✓ 0-9✓ 0-10✓ 1-9✓ 1-10✓ 9-10✓ → K₄ again.
        // Need cells where exactly one pair is non-adjacent.
        // Try: (0,0)=0, (0,3)=3, (1,0)=9, (1,3)=12
        // 0-3: same row ✓, 0-9: same col+box ✓, 0-12: diff row/col/box? row0≠row1, col0≠col3, box0≠box1 → NOT adjacent
        // 3-9: diff row/col, box? (0,3)→box1, (1,0)→box0 → NOT adjacent
        // 3-12: same col ✓, 9-12: same row ✓
        // Adj pairs: 0-3, 0-9, 3-12, 9-12. Missing: 0-12 and 3-9.
        // That's K₄ minus TWO edges. Not a diamond.
        //
        // Let me try: (0,0)=0, (0,1)=1, (0,2)=2, (1,0)=9
        // 0-1✓ 0-2✓ 1-2✓ 0-9✓ 1-9(box)✓ 2-9(box)✓ → K₄
        //
        // Try: (0,0)=0, (0,1)=1, (1,0)=9, (3,1)=28
        // 0-1: row ✓, 0-9: col+box ✓, 0-28: col0≠col1, row0≠row3, box? no → NOT adj
        // 1-9: box ✓, 1-28: col1=col1 → col ✓, 9-28: col0≠col1, row1≠row3, box? no → NOT adj
        // Adj: {0-1, 0-9, 1-9, 1-28}. Common neighbors of 0 and 28: only 1. Not enough.
        //
        // Let me try: (0,0)=0, (0,3)=3, (3,0)=27, (3,3)=30
        // 0-3: row ✓, 0-27: col ✓, 0-30: diff → NOT adj
        // 3-27: diff → NOT adj, 3-30: col3=col3 → col ✓
        // 27-30: row ✓
        // Adj: 0-3, 0-27, 3-30, 27-30. That's a 4-cycle, not a diamond.
        //
        // Need diamond: 5 edges out of 6. Let me add a diagonal.
        // (0,0)=0, (0,1)=1, (1,0)=9, (3,0)=27
        // 0-1: row ✓, 0-9: col+box ✓, 0-27: col ✓
        // 1-9: box ✓, 1-27: ? row0≠3, col1≠0, box? (0,1)→b0, (3,0)→b3 → NOT adj
        // 9-27: col ✓
        // Adj: 0-1, 0-9, 0-27, 1-9, 9-27. Missing: 1-27.
        // So spine = 0-9 (edge). Common neighbors of 0 and 9 that include 1 and 27?
        // N(0)∩N(9) includes 1(box) and... does it include 27? N(9) includes col 0: 0,18,27,36,45,54,63,72 → 27 ✓
        // And N(0) includes 27 (col). So common = {1, 27, ...}
        // 1-27: NOT adj → diamond with tips 1, 27, spine 0, 9.
        let cells = vec![0, 1, 9, 27];
        let g = ProofGraph::from_cells(&cells);
        assert!(g.find_k4().is_none(), "should not have K₄");
        let d = g.find_diamond();
        assert!(d.is_some(), "should find diamond");
    }

    #[test]
    fn test_merge() {
        let cells = vec![0, 1, 9, 27];
        let mut g = ProofGraph::from_cells(&cells);
        // vertices: 0=cell0, 1=cell1, 2=cell9, 3=cell27
        // Diamond tips: 1(cell1) and 3(cell27), spine: 0(cell0) and 2(cell9)
        let d = g.find_diamond().unwrap();
        let (a, b, _, _) = d;
        let (keep, remove) = (a.min(b), a.max(b));
        g.merge(keep, remove);
        assert_eq!(g.active.count_ones(), 3);
        assert!(g.labels[keep].len() == 2);
    }

    #[test]
    fn test_known_n10_proof() {
        let s = "110100000100010000000010000100000000011010000000000000000000000000000000000000000";
        let cells = cells_from_bitstring(s);
        assert_eq!(cells.len(), 10);
        let result = prove_pattern(&cells);
        assert!(result.proof.is_complete(), "proof should be complete");
        assert!(result.text.contains("Contradiction"));
    }

    #[test]
    fn test_k4_immediate() {
        // 4 cells all in row 0 → K₄ → immediate contradiction
        let cells = vec![0, 1, 2, 3];
        let result = prove_pattern(&cells);
        assert!(result.proof.is_complete());
        assert_eq!(result.proof.depth(), 0);
        assert_eq!(result.proof.branch_count(), 0);
    }
}
