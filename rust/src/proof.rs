//! Automated proof generation for non-3-colorability of patterns.
//!
//! Strategy:
//! 1. Check for K₄ or odd wheel — immediate contradiction (size 1).
//! 2. Try all available diamond reductions and all branch pairs as competing
//!    choices; keep the one producing the shortest total proof.
//! 3. Size-budget pruning ensures we never explore worse alternatives.
//! 4. Iterative deepening on branch count finds minimum-branch proofs.

use std::collections::VecDeque;
use crate::sudoku_graph::NEIGHBORS_MASK;

/// Format a cell as "r{row}c{col}" (1-based).
fn cell_name(cell: u8) -> String {
    format!("r{}c{}", cell / 9 + 1, cell % 9 + 1)
}

/// House type for SET equations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HouseType { Row, Col, Box }

/// A full house: a row/col/box with exactly 3 pattern cells.
#[derive(Clone, Debug)]
struct FullHouse {
    htype: HouseType,
    index: u8,             // row/col number or box index (0..8)
    cells: [usize; 3],     // vertex indices in ProofGraph
    orig_cells: [u8; 3],   // original cell indices (0..80)
}

/// A SET equation: k positive houses - k negative houses.
#[derive(Clone, Debug)]
struct SetEquation {
    positive: Vec<FullHouse>,
    negative: Vec<FullHouse>,
    lhs: Vec<usize>,       // remaining vertex indices on positive side
    rhs: Vec<usize>,       // remaining vertex indices on negative side
    m: usize,              // = len(lhs) = len(rhs)
}

/// Deduction from a SET equation.
#[derive(Clone, Debug)]
enum SetDeduction {
    /// m=1, cells are adjacent → contradiction
    Contradiction { lhs_v: usize, rhs_v: usize },
    /// m=1, cells not adjacent → merge
    Merge { lhs_v: usize, rhs_v: usize },
    /// m=2 or m=3 with cross-adjacency → add virtual edges within each side
    VirtualEdges { edges: Vec<(usize, usize)> },
}

/// Result from full-house coloring constraint analysis.
#[derive(Clone, Debug)]
enum HouseColoringResult {
    /// No valid 3-coloring of the house system exists — terminal contradiction.
    Contradiction,
    /// Two cells always have the same color → merge.
    ForcedSame(usize, usize),
    /// Two non-adjacent cells always have different colors → add edge.
    ForcedDiff(usize, usize),
}

impl std::fmt::Display for HouseType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HouseType::Row => write!(f, "row"),
            HouseType::Col => write!(f, "col"),
            HouseType::Box => write!(f, "box"),
        }
    }
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

    /// Find all diamonds: four vertices where the two "tips" share two common
    /// neighbors (the "spine") that are adjacent, but the tips are not adjacent.
    /// Returns Vec of (tip_a, tip_b, spine_u, spine_v), deduplicated by merge
    /// pair (min(tip_a, tip_b), max(tip_a, tip_b)) since different spines for
    /// the same tip pair produce the same merged graph.
    fn find_all_diamonds(&self) -> Vec<(usize, usize, usize, usize)> {
        let verts = self.active_verts();
        let n = verts.len();
        let mut seen: Vec<(usize, usize)> = Vec::new();
        let mut results = Vec::new();
        for i in 0..n {
            let u = verts[i];
            let nu = self.adj[u] & self.active;
            for j in (i + 1)..n {
                let v = verts[j];
                if nu & (1 << v) == 0 { continue; }
                let common = nu & self.adj[v] & self.active & !((1 << u) | (1 << v));
                if common.count_ones() < 2 { continue; }
                let mut am = common;
                while am != 0 {
                    let a = am.trailing_zeros() as usize;
                    am &= am - 1;
                    let mut bm = am;
                    while bm != 0 {
                        let b = bm.trailing_zeros() as usize;
                        bm &= bm - 1;
                        if self.adj[a] & (1 << b) == 0 {
                            let pair = (a.min(b), a.max(b));
                            if !seen.contains(&pair) {
                                seen.push(pair);
                                results.push((a, b, u, v));
                            }
                        }
                    }
                }
            }
        }
        results
    }

    /// Find an odd wheel: a hub vertex whose neighbors contain an induced odd
    /// cycle (length 5, 7, or 9). Returns (hub, cycle_verts_in_order).
    fn find_odd_wheel(&self) -> Option<(usize, Vec<usize>)> {
        let verts = self.active_verts();
        for &hub in &verts {
            let nbrs = self.adj[hub] & self.active & !(1 << hub);
            let nbr_count = nbrs.count_ones();
            if nbr_count < 5 { continue; }

            // Collect hub's neighbors
            let mut nbr_list = Vec::new();
            let mut m = nbrs;
            while m != 0 {
                nbr_list.push(m.trailing_zeros() as usize);
                m &= m - 1;
            }

            // Try to find an odd cycle among these neighbors (length 5, 7, 9).
            // Use DFS/backtracking to find a cycle in the subgraph induced on nbr_list.
            // We need: each consecutive pair adjacent, last-first adjacent,
            // and no other adjacencies among cycle vertices (to be an induced cycle).
            for cycle_len in [5usize, 7, 9] {
                if nbr_list.len() < cycle_len { continue; }
                if let Some(cycle) = self.find_induced_odd_cycle(&nbr_list, cycle_len) {
                    return Some((hub, cycle));
                }
            }
        }
        None
    }

    /// Search for an induced cycle of exactly `target_len` among `candidates`.
    fn find_induced_odd_cycle(&self, candidates: &[usize], target_len: usize) -> Option<Vec<usize>> {
        if candidates.len() < target_len { return None; }

        // Build a bitmask of candidates for fast lookup
        let mut cand_mask: u32 = 0;
        for &v in candidates { cand_mask |= 1 << v; }

        let mut path = Vec::with_capacity(target_len);

        // Try each candidate as starting vertex
        for &start in candidates {
            path.clear();
            path.push(start);

            if self.find_cycle_dfs(
                &mut path, 1 << start, start, target_len, cand_mask,
            ) {
                return Some(path.clone());
            }
        }
        None
    }

    /// DFS to extend `path` to a cycle of `target_len` among vertices in `cand_mask`.
    /// `start` is the first vertex (must close the cycle).
    fn find_cycle_dfs(
        &self,
        path: &mut Vec<usize>,
        used: u32,
        start: usize,
        target_len: usize,
        cand_mask: u32,
    ) -> bool {
        let cur = *path.last().unwrap();

        if path.len() == target_len {
            // Check: last vertex adjacent to start?
            if self.adj[cur] & (1 << start) == 0 { return false; }
            // Check induced: no chords (non-consecutive adjacencies within the cycle)
            return self.is_induced_cycle(path);
        }

        // Extend: try neighbors of `cur` that are in cand_mask and not used
        let mut next_mask = self.adj[cur] & cand_mask & self.active & !used;
        // Prune: only consider vertices > start to avoid duplicate cycles
        // (we fix the start as the smallest vertex in the cycle)
        if path.len() == 1 {
            // Second vertex must be > start to avoid mirror duplicates
            next_mask &= !((1u32 << (start + 1)) - 1);
        }

        while next_mask != 0 {
            let v = next_mask.trailing_zeros() as usize;
            next_mask &= next_mask - 1;

            path.push(v);
            if self.find_cycle_dfs(path, used | (1 << v), start, target_len, cand_mask) {
                return true;
            }
            path.pop();
        }
        false
    }

    /// Check that a cycle path has no chords (is an induced cycle).
    fn is_induced_cycle(&self, path: &[usize]) -> bool {
        let n = path.len();
        for i in 0..n {
            for j in i + 2..n {
                if j == n - 1 && i == 0 { continue; } // last-first is the closing edge
                if self.adj[path[i]] & (1 << path[j]) != 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Find all 3-prisms (circular ladders) with at least 2 satellites from
    /// distinct rungs where at least one new edge can be added.
    /// A 3-prism is two vertex-disjoint triangles connected by 3 rungs.
    /// In any 3-coloring each rung misses a distinct color, so a satellite
    /// (vertex adjacent to both endpoints of a rung) is forced to that color.
    /// Satellites on different rungs have different colors → add edges.
    fn find_all_circular_ladders(&self) -> Vec<([(usize, usize); 3], Vec<(usize, usize)>)> {
        let verts = self.active_verts();
        let n = verts.len();
        if n < 8 { return Vec::new(); } // need 6 (prism) + 2 (satellites)

        // Collect all triangles
        let mut triangles: Vec<[usize; 3]> = Vec::new();
        for i in 0..n {
            let a = verts[i];
            let na = self.adj[a] & self.active;
            for j in (i + 1)..n {
                let b = verts[j];
                if na & (1 << b) == 0 { continue; }
                let nab = na & self.adj[b] & self.active;
                for k in (j + 1)..n {
                    let c = verts[k];
                    if nab & (1 << c) != 0 {
                        triangles.push([a, b, c]);
                    }
                }
            }
        }

        let perms: [[usize; 3]; 6] = [
            [0,1,2],[0,2,1],[1,0,2],[1,2,0],[2,0,1],[2,1,0],
        ];
        let mut results = Vec::new();
        let mut seen_sat_sets: Vec<Vec<usize>> = Vec::new();

        for ti in 0..triangles.len() {
            let ta = triangles[ti];
            let mask_a: u32 = ta.iter().fold(0u32, |m, &v| m | (1 << v));
            for tj in (ti + 1)..triangles.len() {
                let tb = triangles[tj];
                let mask_b: u32 = tb.iter().fold(0u32, |m, &v| m | (1 << v));
                if mask_a & mask_b != 0 { continue; } // not disjoint

                for perm in &perms {
                    let rungs = [
                        (ta[0], tb[perm[0]]),
                        (ta[1], tb[perm[1]]),
                        (ta[2], tb[perm[2]]),
                    ];
                    if rungs.iter().any(|&(u, v)| self.adj[u] & (1 << v) == 0) {
                        continue;
                    }

                    // Found a prism. Find one satellite per rung.
                    let prism_mask = mask_a | mask_b;
                    let mut satellites: Vec<(usize, usize)> = Vec::new();
                    for (ri, &(u, v)) in rungs.iter().enumerate() {
                        let sats = self.adj[u] & self.adj[v] & self.active & !prism_mask;
                        if sats != 0 {
                            satellites.push((ri, sats.trailing_zeros() as usize));
                        }
                    }
                    if satellites.len() < 2 { continue; }

                    // Check at least one new edge can be added
                    let has_new = (0..satellites.len()).any(|si| {
                        ((si + 1)..satellites.len()).any(|sj| {
                            self.adj[satellites[si].1] & (1 << satellites[sj].1) == 0
                        })
                    });
                    if !has_new { continue; }

                    // Dedup by satellite vertex set
                    let mut sv: Vec<usize> = satellites.iter().map(|&(_, s)| s).collect();
                    sv.sort();
                    if seen_sat_sets.contains(&sv) { continue; }
                    seen_sat_sets.push(sv);

                    results.push((rungs, satellites));
                }
            }
        }
        results
    }

    /// Find a bridged hexagon: an induced C₆ with 3 bridge pairs on opposite
    /// edges, forcing a terminal contradiction.
    ///
    /// In any 3-coloring of C₆, each edge has a "missing" color. A bridge pair
    /// on opposite edges (eᵢ, eᵢ₊₃) consists of a satellite on each edge that
    /// are mutually adjacent, forcing the two edges to miss different colors.
    /// Every 3-coloring of C₆ has at least one pair of opposite edges with the
    /// same missing color, so 3 bridge pairs → contradiction.
    fn find_bridged_hexagon(&self) -> Option<([usize; 6], [(usize, usize); 3])> {
        let verts = self.active_verts();
        if verts.len() < 9 { return None; } // need 6 ring + at least 3 bridge vertices

        // Only consider ring candidates with degree ≥ 3 (need ≥ 2 ring neighbors)
        let mut cand_mask: u32 = 0;
        for &v in &verts {
            if (self.adj[v] & self.active).count_ones() >= 3 {
                cand_mask |= 1 << v;
            }
        }
        if cand_mask.count_ones() < 6 { return None; }

        let mut cand_list = Vec::new();
        let mut m = cand_mask;
        while m != 0 {
            cand_list.push(m.trailing_zeros() as usize);
            m &= m - 1;
        }

        let mut path = Vec::with_capacity(6);
        let mut counter: u64 = 0;
        const MAX_NODES: u64 = 500_000;

        for &start in &cand_list {
            path.clear();
            path.push(start);
            if let Some(result) = self.find_bridged_hex_dfs(
                &mut path, 1 << start, start, cand_mask, &mut counter, MAX_NODES,
            ) {
                return Some(result);
            }
            if counter >= MAX_NODES { return None; }
        }
        None
    }

    /// DFS to find an induced C₆ with valid bridge pairs.
    fn find_bridged_hex_dfs(
        &self,
        path: &mut Vec<usize>,
        used: u32,
        start: usize,
        cand_mask: u32,
        counter: &mut u64,
        max_nodes: u64,
    ) -> Option<([usize; 6], [(usize, usize); 3])> {
        *counter += 1;
        if *counter >= max_nodes { return None; }

        let cur = *path.last().unwrap();

        if path.len() == 6 {
            // Check closing edge and induced property
            if self.adj[cur] & (1 << start) == 0 { return None; }
            if !self.is_induced_cycle(path) { return None; }
            return self.check_hex_bridges(path);
        }

        let mut next_mask = self.adj[cur] & cand_mask & self.active & !used;
        if path.len() == 1 {
            // Second vertex > start to avoid duplicate cycles
            next_mask &= !((1u32 << (start + 1)) - 1);
        }

        while next_mask != 0 {
            let v = next_mask.trailing_zeros() as usize;
            next_mask &= next_mask - 1;

            path.push(v);
            if let Some(result) = self.find_bridged_hex_dfs(
                path, used | (1 << v), start, cand_mask, counter, max_nodes,
            ) {
                return Some(result);
            }
            path.pop();
            if *counter >= max_nodes { return None; }
        }
        None
    }

    /// Check whether an induced C₆ has 3 bridge pairs on opposite edges.
    fn check_hex_bridges(&self, ring: &[usize]) -> Option<([usize; 6], [(usize, usize); 3])> {
        debug_assert_eq!(ring.len(), 6);
        let ring_mask: u32 = ring.iter().fold(0u32, |m, &v| m | (1 << v));

        let mut bridges = [(0usize, 0usize); 3];
        for i in 0..3 {
            // Opposite edges: (ring[i], ring[i+1]) and (ring[i+3], ring[(i+4)%6])
            let (u1, u2) = (ring[i], ring[i + 1]);
            let (w1, w2) = (ring[i + 3], ring[(i + 4) % 6]);

            // Satellites: vertices adjacent to both endpoints, not in ring
            let sats_a = self.adj[u1] & self.adj[u2] & self.active & !ring_mask;
            let sats_b = self.adj[w1] & self.adj[w2] & self.active & !ring_mask;

            if sats_a == 0 || sats_b == 0 { return None; }

            // Find a pair (s1 on edge_i, s2 on edge_{i+3}) that are adjacent
            let mut found = false;
            let mut ma = sats_a;
            while ma != 0 {
                let s1 = ma.trailing_zeros() as usize;
                ma &= ma - 1;
                let cands = self.adj[s1] & sats_b & !(1 << s1);
                if cands != 0 {
                    let s2 = cands.trailing_zeros() as usize;
                    bridges[i] = (s1, s2);
                    found = true;
                    break;
                }
            }
            if !found { return None; }
        }

        let ring_arr = [ring[0], ring[1], ring[2], ring[3], ring[4], ring[5]];
        Some((ring_arr, bridges))
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

    // ── SET Equivalence Theory ──────────────────────────────────────

    /// Identify full houses (row/col/box with exactly 3 active vertices).
    fn find_full_houses(&self) -> Vec<FullHouse> {
        let mut houses: Vec<FullHouse> = Vec::new();
        // Count original cells per house, mapping to current vertices.
        // A house is "full" if exactly 3 distinct original cells from the
        // pattern land in it AND those cells map to 3 distinct active vertices.
        //
        // After merges, two original cells might map to the same vertex;
        // we still track houses by original cell positions.

        // Gather all (original_cell, current_vertex) pairs for active vertices
        let mut cell_vertex: Vec<(u8, usize)> = Vec::new();
        let verts = self.active_verts();
        for &v in &verts {
            for &c in &self.labels[v] {
                cell_vertex.push((c, v));
            }
        }

        // Group by house
        let mut by_row: [Vec<(u8, usize)>; 9] = Default::default();
        let mut by_col: [Vec<(u8, usize)>; 9] = Default::default();
        let mut by_box: [Vec<(u8, usize)>; 9] = Default::default();

        for &(c, v) in &cell_vertex {
            let r = c / 9;
            let col = c % 9;
            let b = (r / 3) * 3 + col / 3;
            by_row[r as usize].push((c, v));
            by_col[col as usize].push((c, v));
            by_box[b as usize].push((c, v));
        }

        for (idx, group) in by_row.iter().enumerate() {
            if group.len() == 3 {
                houses.push(FullHouse {
                    htype: HouseType::Row,
                    index: idx as u8,
                    cells: [group[0].1, group[1].1, group[2].1],
                    orig_cells: [group[0].0, group[1].0, group[2].0],
                });
            }
        }
        for (idx, group) in by_col.iter().enumerate() {
            if group.len() == 3 {
                houses.push(FullHouse {
                    htype: HouseType::Col,
                    index: idx as u8,
                    cells: [group[0].1, group[1].1, group[2].1],
                    orig_cells: [group[0].0, group[1].0, group[2].0],
                });
            }
        }
        for (idx, group) in by_box.iter().enumerate() {
            if group.len() == 3 {
                houses.push(FullHouse {
                    htype: HouseType::Box,
                    index: idx as u8,
                    cells: [group[0].1, group[1].1, group[2].1],
                    orig_cells: [group[0].0, group[1].0, group[2].0],
                });
            }
        }

        houses
    }

    /// Find all SET equations with remainder size m <= max_m.
    /// Tries all pairs of house types (positive vs negative), all k-sized
    /// subsets, and returns equations with distinct LHS/RHS after cancellation.
    fn find_set_equations(&self, max_m: usize) -> Vec<SetEquation> {
        let houses = self.find_full_houses();
        if houses.is_empty() { return Vec::new(); }

        // Group by type
        let mut rows: Vec<&FullHouse> = Vec::new();
        let mut cols: Vec<&FullHouse> = Vec::new();
        let mut boxes: Vec<&FullHouse> = Vec::new();
        for h in &houses {
            match h.htype {
                HouseType::Row => rows.push(h),
                HouseType::Col => cols.push(h),
                HouseType::Box => boxes.push(h),
            }
        }

        let groups: [(HouseType, &[&FullHouse]); 3] = [
            (HouseType::Row, &rows),
            (HouseType::Col, &cols),
            (HouseType::Box, &boxes),
        ];

        let mut equations: Vec<SetEquation> = Vec::new();
        // Use a set to deduplicate by (lhs_mask, rhs_mask) of vertex indices
        let mut seen: Vec<(u32, u32)> = Vec::new();

        // Try all (type_A positive, type_B negative) pairs
        for i in 0..3 {
            for j in 0..3 {
                if i == j { continue; }
                let pos_group = groups[i].1;
                let neg_group = groups[j].1;
                let max_k = pos_group.len().min(neg_group.len());

                for k in 1..=max_k {
                    // Enumerate k-subsets of positive
                    for pos_sel in combinations(pos_group, k) {
                        for neg_sel in combinations(neg_group, k) {
                            if let Some(eq) = self.build_set_equation(&pos_sel, &neg_sel, max_m, &mut seen) {
                                equations.push(eq);
                            }
                        }
                    }
                }
            }
        }

        equations
    }

    /// Build a SET equation from selected positive and negative houses.
    /// Returns None if remainder is too large or equation is trivial.
    fn build_set_equation(
        &self,
        positive: &[&FullHouse],
        negative: &[&FullHouse],
        max_m: usize,
        seen: &mut Vec<(u32, u32)>,
    ) -> Option<SetEquation> {
        // Collect original cell sets for each side
        let mut pos_cells: u128 = 0;
        for h in positive {
            for &c in &h.orig_cells {
                pos_cells |= 1u128 << c;
            }
        }
        let mut neg_cells: u128 = 0;
        for h in negative {
            for &c in &h.orig_cells {
                neg_cells |= 1u128 << c;
            }
        }

        let intersection = pos_cells & neg_cells;
        let lhs_cells = pos_cells & !intersection;
        let rhs_cells = neg_cells & !intersection;

        let m_lhs = lhs_cells.count_ones() as usize;
        let m_rhs = rhs_cells.count_ones() as usize;

        if m_lhs == 0 || m_lhs != m_rhs || m_lhs > max_m {
            return None;
        }

        // Map original cells to current vertices
        let lhs_verts = self.orig_cells_to_vertices(lhs_cells);
        let rhs_verts = self.orig_cells_to_vertices(rhs_cells);

        // Deduplicate by vertex mask pair
        let mut lhs_mask: u32 = 0;
        for &v in &lhs_verts { lhs_mask |= 1 << v; }
        let mut rhs_mask: u32 = 0;
        for &v in &rhs_verts { rhs_mask |= 1 << v; }

        let key = if lhs_mask <= rhs_mask { (lhs_mask, rhs_mask) } else { (rhs_mask, lhs_mask) };
        if seen.contains(&key) { return None; }
        seen.push(key);

        Some(SetEquation {
            positive: positive.iter().map(|h| (*h).clone()).collect(),
            negative: negative.iter().map(|h| (*h).clone()).collect(),
            lhs: lhs_verts,
            rhs: rhs_verts,
            m: m_lhs,
        })
    }

    /// Map a bitmask of original cells (0..80) to current vertex indices.
    fn orig_cells_to_vertices(&self, cell_mask: u128) -> Vec<usize> {
        let mut result = Vec::new();
        let verts = self.active_verts();
        let mut remaining = cell_mask;
        for &v in &verts {
            for &c in &self.labels[v] {
                if remaining & (1u128 << c) != 0 {
                    if !result.contains(&v) {
                        result.push(v);
                    }
                    remaining &= !(1u128 << c);
                }
            }
        }
        result
    }

    /// Derive a deduction from a SET equation, if possible.
    fn derive_set_deduction(&self, eq: &SetEquation) -> Option<SetDeduction> {
        if eq.m == 1 {
            let l = eq.lhs[0];
            let r = eq.rhs[0];
            if l == r {
                return None; // Same vertex after merge — trivial
            }
            if self.adj[l] & (1 << r) != 0 {
                return Some(SetDeduction::Contradiction { lhs_v: l, rhs_v: r });
            } else {
                return Some(SetDeduction::Merge { lhs_v: l, rhs_v: r });
            }
        }

        // m == 2 or m == 3: check cross-adjacency forcing all-distinct
        if !self.check_cross_adjacency(&eq.lhs, &eq.rhs) {
            return None;
        }

        // Both sides forced all-distinct.  Add virtual edges for pairs
        // within each side that aren't already adjacent.
        let mut edges = Vec::new();
        for i in 0..eq.lhs.len() {
            for j in (i + 1)..eq.lhs.len() {
                let a = eq.lhs[i];
                let b = eq.lhs[j];
                if a != b && self.adj[a] & (1 << b) == 0 {
                    edges.push((a.min(b), a.max(b)));
                }
            }
        }
        for i in 0..eq.rhs.len() {
            for j in (i + 1)..eq.rhs.len() {
                let a = eq.rhs[i];
                let b = eq.rhs[j];
                if a != b && self.adj[a] & (1 << b) == 0 {
                    edges.push((a.min(b), a.max(b)));
                }
            }
        }

        if edges.is_empty() {
            return None; // All pairs already adjacent — no new info
        }

        // Deduplicate edges
        edges.sort();
        edges.dedup();

        Some(SetDeduction::VirtualEdges { edges })
    }

    /// Check if cross-adjacency between LHS and RHS forces all-distinct colors.
    /// For m=2: need at least 1 cross-edge.
    /// For m=3: for every pair on LHS, their combined RHS neighborhood must
    ///          have >= 2 vertices.
    fn check_cross_adjacency(&self, lhs: &[usize], rhs: &[usize]) -> bool {
        let m = lhs.len();
        if m == 1 { return true; }

        if m == 2 {
            for &l in lhs {
                for &r in rhs {
                    if self.adj[l] & (1 << r) != 0 {
                        return true;
                    }
                }
            }
            return false;
        }

        if m == 3 {
            // For every pair of LHS vertices, combined RHS neighborhood >= 2
            for i in 0..lhs.len() {
                for j in (i + 1)..lhs.len() {
                    let combined = (self.adj[lhs[i]] | self.adj[lhs[j]]) & self.active;
                    let mut rhs_count = 0;
                    for &r in rhs {
                        if combined & (1 << r) != 0 {
                            rhs_count += 1;
                        }
                    }
                    if rhs_count < 2 { return false; }
                }
            }
            return true;
        }

        false
    }

    /// Find all SET deductions available on the current graph.
    /// Returns list of (equation, deduction) pairs prioritized:
    /// contradictions first, then merges, then virtual edges.
    fn find_set_deductions(&self) -> Vec<(SetEquation, SetDeduction)> {
        let equations = self.find_set_equations(3);
        let mut results: Vec<(SetEquation, SetDeduction)> = Vec::new();

        for eq in equations {
            if let Some(ded) = self.derive_set_deduction(&eq) {
                results.push((eq, ded));
            }
        }

        // Sort: contradictions first, then merges, then virtual edges
        results.sort_by_key(|(_, d)| match d {
            SetDeduction::Contradiction { .. } => 0,
            SetDeduction::Merge { .. } => 1,
            SetDeduction::VirtualEdges { .. } => 2,
        });

        results
    }

    // ── Parity Transport (Trivalue Oddagon) ─────────────────────────

    /// Sign of a permutation of 3 elements: +1 for even, -1 for odd.
    fn permutation_sign_3(perm: [usize; 3]) -> i8 {
        let inversions =
            (perm[0] > perm[1]) as i8 +
            (perm[0] > perm[2]) as i8 +
            (perm[1] > perm[2]) as i8;
        if inversions % 2 == 0 { 1 } else { -1 }
    }

    /// Detect a parity transport contradiction (trivalue oddagon).
    ///
    /// Finds full houses (row/col/box with 3 cells mapping to 3 distinct
    /// active vertices), builds a graph of 1-1 connections between pairs of
    /// disjoint houses, and checks for an odd parity cycle via BFS 2-coloring.
    fn find_parity_transport(&self) -> Option<ProofNode> {
        let all_houses = self.find_full_houses();

        // Filter to houses with 3 distinct active vertices
        let houses: Vec<&FullHouse> = all_houses.iter()
            .filter(|h| {
                h.cells[0] != h.cells[1] &&
                h.cells[0] != h.cells[2] &&
                h.cells[1] != h.cells[2]
            })
            .collect();
        let n = houses.len();
        if n < 2 { return None; }

        // Build parity graph: edges labeled with permutation sign
        // adj[i] = list of (neighbor_house_index, parity_sign)
        let mut adj: Vec<Vec<(usize, i8)>> = vec![vec![]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                // Check disjoint original cells
                let mut overlap = false;
                'outer: for &ci in &houses[i].orig_cells {
                    for &cj in &houses[j].orig_cells {
                        if ci == cj { overlap = true; break 'outer; }
                    }
                }
                if overlap { continue; }

                // Check disjoint vertices (a merged vertex might span both)
                let mut vert_overlap = false;
                'outer2: for &vi in &houses[i].cells {
                    for &vj in &houses[j].cells {
                        if vi == vj { vert_overlap = true; break 'outer2; }
                    }
                }
                if vert_overlap { continue; }

                // Check 1-1 vertex adjacency: each cell in H_i sees exactly
                // one cell in H_j, and vice versa
                let mut matching: [Option<usize>; 3] = [None; 3];
                let mut valid = true;

                for (ci, &vi) in houses[i].cells.iter().enumerate() {
                    let mut count = 0;
                    let mut matched_idx = 0;
                    for (cj, &vj) in houses[j].cells.iter().enumerate() {
                        if self.adj[vi] & (1 << vj) != 0 {
                            count += 1;
                            matched_idx = cj;
                        }
                    }
                    if count != 1 { valid = false; break; }
                    matching[ci] = Some(matched_idx);
                }
                if !valid { continue; }

                // Verify reverse direction
                for &vj in &houses[j].cells {
                    let mut count = 0;
                    for &vi in &houses[i].cells {
                        if self.adj[vj] & (1 << vi) != 0 {
                            count += 1;
                        }
                    }
                    if count != 1 { valid = false; break; }
                }
                if !valid { continue; }

                let perm = [matching[0].unwrap(), matching[1].unwrap(), matching[2].unwrap()];
                let parity = Self::permutation_sign_3(perm);

                adj[i].push((j, parity));
                adj[j].push((i, parity));
            }
        }

        // BFS 2-coloring to detect odd cycle
        let mut label: Vec<Option<i8>> = vec![None; n];
        let mut parent: Vec<Option<usize>> = vec![None; n];

        for start in 0..n {
            if label[start].is_some() || adj[start].is_empty() { continue; }

            label[start] = Some(1);
            let mut queue = VecDeque::new();
            queue.push_back(start);

            while let Some(u) = queue.pop_front() {
                for &(v, parity) in &adj[u] {
                    let expected = label[u].unwrap() * parity;
                    if let Some(lv) = label[v] {
                        if lv != expected {
                            // Odd cycle found — extract and return
                            let cycle = Self::extract_bfs_cycle(u, v, &parent);
                            return Some(self.build_parity_transport_node(
                                &cycle, &houses, &adj,
                            ));
                        }
                    } else {
                        label[v] = Some(expected);
                        parent[v] = Some(u);
                        queue.push_back(v);
                    }
                }
            }
        }

        None
    }

    /// Extract a cycle from BFS tree when edge (u, v) creates a conflict.
    fn extract_bfs_cycle(u: usize, v: usize, parent: &[Option<usize>]) -> Vec<usize> {
        // Build paths from u and v to the BFS root
        let path_to_root = |start: usize| -> Vec<usize> {
            let mut path = vec![start];
            let mut cur = start;
            while let Some(p) = parent[cur] {
                path.push(p);
                cur = p;
            }
            path
        };

        let path_u = path_to_root(u);
        let path_v = path_to_root(v);

        // Find LCA (lowest common ancestor)
        let set_u: std::collections::HashSet<usize> =
            path_u.iter().copied().collect();
        let mut lca = *path_v.last().unwrap();
        let mut v_prefix_len = path_v.len();
        for (i, &node) in path_v.iter().enumerate() {
            if set_u.contains(&node) {
                lca = node;
                v_prefix_len = i + 1;
                break;
            }
        }

        // Cycle: LCA → ... → u  (then edge u→v)  v → ... → LCA
        let u_prefix_len = path_u.iter().position(|&n| n == lca).unwrap() + 1;
        let mut cycle: Vec<usize> = path_u[..u_prefix_len].to_vec();
        cycle.reverse(); // LCA, ..., u

        // Append v path back to (but not including) LCA
        for &node in &path_v[..v_prefix_len - 1] {
            cycle.push(node);
        }

        cycle
    }

    /// Build a ProofNode::ParityTransport from a cycle of house indices.
    fn build_parity_transport_node(
        &self,
        cycle: &[usize],
        houses: &[&FullHouse],
        adj: &[Vec<(usize, i8)>],
    ) -> ProofNode {
        let len = cycle.len();
        let mut house_descs: Vec<(String, [String; 3])> = Vec::new();
        let mut conn_descs: Vec<String> = Vec::new();

        for idx in 0..len {
            let hi = cycle[idx];
            let h = houses[hi];
            let hname = format_house(h);
            let cell_names = [
                self.vertex_name(h.cells[0]),
                self.vertex_name(h.cells[1]),
                self.vertex_name(h.cells[2]),
            ];
            house_descs.push((hname, cell_names));

            // Describe connection to next house
            let hj = cycle[(idx + 1) % len];
            // Find the parity for this edge
            let parity = adj[hi].iter()
                .find(|&&(nb, _)| nb == hj)
                .map(|&(_, p)| p)
                .unwrap_or(1);

            // Determine what connects these houses (rows/cols/boxes)
            let h_next = houses[hj];
            let mut via_rows: Vec<u8> = Vec::new();
            let mut via_cols: Vec<u8> = Vec::new();
            let mut via_boxes: Vec<u8> = Vec::new();

            for &ci in &h.orig_cells {
                for &cj in &h_next.orig_cells {
                    if NEIGHBORS_MASK[ci as usize] & (1u128 << cj) != 0 {
                        let ri = ci / 9;
                        let rj = cj / 9;
                        let coli = ci % 9;
                        let colj = cj % 9;
                        let bi = (ri / 3) * 3 + coli / 3;
                        let bj = (rj / 3) * 3 + colj / 3;
                        if ri == rj { via_rows.push(ri + 1); }
                        else if coli == colj { via_cols.push(coli + 1); }
                        else if bi == bj { via_boxes.push(bi + 1); }
                    }
                }
            }

            let parity_str = if parity == 1 { "even" } else { "odd" };
            let via_str = if via_rows.len() >= via_cols.len() && via_rows.len() >= via_boxes.len() && !via_rows.is_empty() {
                via_rows.sort();
                via_rows.dedup();
                let nums: Vec<String> = via_rows.iter().map(|n| n.to_string()).collect();
                format!("via rows {{{}}}", nums.join(", "))
            } else if via_cols.len() >= via_boxes.len() && !via_cols.is_empty() {
                via_cols.sort();
                via_cols.dedup();
                let nums: Vec<String> = via_cols.iter().map(|n| n.to_string()).collect();
                format!("via cols {{{}}}", nums.join(", "))
            } else if !via_boxes.is_empty() {
                via_boxes.sort();
                via_boxes.dedup();
                let nums: Vec<String> = via_boxes.iter().map(|n| n.to_string()).collect();
                format!("via boxes {{{}}}", nums.join(", "))
            } else {
                "adjacent".to_string()
            };

            conn_descs.push(format!("{} [{}]", via_str, parity_str));
        }

        ProofNode::ParityTransport {
            houses: house_descs,
            connections: conn_descs,
        }
    }
    /// Detect a parity transport contradiction via pigeonhole on permutation
    /// parity classes.
    ///
    /// Finds rows (or columns) with exactly 3 pattern cells, one per stack
    /// (or band). If ≥4 such rows are connected by parallel 1-1 adjacency
    /// (all 3 stack-pairs adjacent → same permutation parity forced), and
    /// every pair of rows has at least one adjacent stack-pair (→ distinct
    /// permutations required), then pigeonhole gives a contradiction:
    /// 4 distinct same-parity permutations needed, but only 3 exist in each
    /// parity class of S₃.
    fn find_parity_chain(&self) -> Option<ProofNode> {
        let all_houses = self.find_full_houses();

        for is_row in [true, false] {
            // Collect full triple houses: rows (or columns) with exactly 3
            // distinct active vertices.  Cells are sorted by column (for rows)
            // or by row (for columns) to give a canonical position ordering.
            let mut triples: Vec<(u8, [usize; 3], [u8; 3])> = Vec::new();

            for h in &all_houses {
                if is_row && h.htype != HouseType::Row { continue; }
                if !is_row && h.htype != HouseType::Col { continue; }
                if h.cells[0] == h.cells[1] || h.cells[0] == h.cells[2]
                    || h.cells[1] == h.cells[2] { continue; }

                // Sort cells by column (for rows) or by row (for columns)
                let sort_keys: [u8; 3] = if is_row {
                    [h.orig_cells[0] % 9, h.orig_cells[1] % 9, h.orig_cells[2] % 9]
                } else {
                    [h.orig_cells[0] / 9, h.orig_cells[1] / 9, h.orig_cells[2] / 9]
                };

                let mut order: [(u8, usize); 3] =
                    [(sort_keys[0], 0), (sort_keys[1], 1), (sort_keys[2], 2)];
                order.sort_by_key(|&(k, _)| k);

                triples.push((
                    h.index,
                    [h.cells[order[0].1], h.cells[order[1].1], h.cells[order[2].1]],
                    [h.orig_cells[order[0].1], h.orig_cells[order[1].1], h.orig_cells[order[2].1]],
                ));
            }

            let n = triples.len();
            if n < 4 { continue; }

            // Parallel graph: edge iff all 3 same-position pairs are adjacent
            let mut par_adj: Vec<u32> = vec![0; n];
            for i in 0..n {
                for j in (i + 1)..n {
                    let all = (0..3).all(|k|
                        self.adj[triples[i].1[k]] & (1 << triples[j].1[k]) != 0
                    );
                    if all {
                        par_adj[i] |= 1 << j;
                        par_adj[j] |= 1 << i;
                    }
                }
            }

            // Conflict graph: edge iff ≥1 same-position pair is adjacent
            let mut conf_adj: Vec<u32> = vec![0; n];
            for i in 0..n {
                for j in (i + 1)..n {
                    let any = (0..3).any(|k|
                        self.adj[triples[i].1[k]] & (1 << triples[j].1[k]) != 0
                    );
                    if any {
                        conf_adj[i] |= 1 << j;
                        conf_adj[j] |= 1 << i;
                    }
                }
            }

            // Find connected components in parallel graph
            let mut visited = 0u32;
            for start in 0..n {
                if visited & (1 << start) != 0 { continue; }
                if par_adj[start] == 0 { visited |= 1 << start; continue; }

                let mut comp = 1u32 << start;
                let mut queue = VecDeque::new();
                queue.push_back(start);
                visited |= 1 << start;

                while let Some(u) = queue.pop_front() {
                    let mut nbrs = par_adj[u] & !visited;
                    while nbrs != 0 {
                        let v = nbrs.trailing_zeros() as usize;
                        nbrs &= nbrs - 1;
                        visited |= 1 << v;
                        comp |= 1 << v;
                        queue.push_back(v);
                    }
                }

                let comp_size = comp.count_ones();
                if comp_size < 4 { continue; }

                // Collect component members
                let mut members = Vec::new();
                let mut m = comp;
                while m != 0 {
                    members.push(m.trailing_zeros() as usize);
                    m &= m - 1;
                }

                // Check if all pairs in component have ≥1 adjacent group
                let all_conflict = members.iter().all(|&i|
                    (conf_adj[i] & comp) == (comp & !(1u32 << i))
                );

                if all_conflict {
                    return Some(self.build_chain_node(
                        &members, &triples, &par_adj, is_row,
                    ));
                }

                // Try 4-subsets of the component
                let nm = members.len();
                for a in 0..nm {
                    for b in (a + 1)..nm {
                        for c in (b + 1)..nm {
                            for d in (c + 1)..nm {
                                let sub = [members[a], members[b],
                                           members[c], members[d]];
                                let sub_mask: u32 = sub.iter()
                                    .fold(0u32, |m, &i| m | (1 << i));

                                // Must be connected in parallel graph
                                if !Self::is_mask_connected(sub_mask, &par_adj) {
                                    continue;
                                }

                                // All 6 pairs must conflict
                                let all_conf = sub.iter().all(|&i|
                                    (conf_adj[i] & sub_mask) == (sub_mask & !(1u32 << i))
                                );
                                if all_conf {
                                    return Some(self.build_chain_node(
                                        &sub.to_vec(), &triples, &par_adj, is_row,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Check whether a bitmask of vertices is connected in an adjacency list.
    fn is_mask_connected(mask: u32, adj: &[u32]) -> bool {
        if mask == 0 || mask.count_ones() == 1 { return true; }
        let start = mask.trailing_zeros() as usize;
        let mut reached = 1u32 << start;
        let mut frontier = reached;
        loop {
            let mut new_frontier = 0u32;
            let mut f = frontier;
            while f != 0 {
                let v = f.trailing_zeros() as usize;
                f &= f - 1;
                new_frontier |= adj[v] & mask & !reached;
            }
            if new_frontier == 0 { break; }
            reached |= new_frontier;
            frontier = new_frontier;
        }
        reached == mask
    }

    /// Build a ProofNode::ParityChain from selected full triple houses.
    fn build_chain_node(
        &self,
        indices: &[usize],
        triples: &[(u8, [usize; 3], [u8; 3])],
        par_adj: &[u32],
        is_row: bool,
    ) -> ProofNode {
        let house_type = if is_row { "row" } else { "col" };

        let mut house_names = Vec::new();
        let mut cells = Vec::new();

        for &idx in indices {
            let (house_idx, ref vert_cells, _) = triples[idx];
            house_names.push(format!("{} {}", house_type, house_idx + 1));
            cells.push([
                self.vertex_name(vert_cells[0]),
                self.vertex_name(vert_cells[1]),
                self.vertex_name(vert_cells[2]),
            ]);
        }

        // Describe parallel links
        let mut links = Vec::new();
        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                if par_adj[indices[i]] & (1 << indices[j]) != 0 {
                    let (hi, _, ref oc_i) = triples[indices[i]];
                    let (hj, _, ref oc_j) = triples[indices[j]];

                    let via: Vec<&str> = (0..3).map(|k| {
                        if is_row {
                            if oc_i[k] % 9 == oc_j[k] % 9 { "col" }
                            else { "box" }
                        } else {
                            if oc_i[k] / 9 == oc_j[k] / 9 { "row" }
                            else { "box" }
                        }
                    }).collect();

                    links.push(format!(
                        "{} {}\u{2194}{} {} ({})",
                        house_type, hi + 1,
                        house_type, hj + 1,
                        via.join(", "),
                    ));
                }
            }
        }

        ProofNode::ParityChain {
            house_type: house_type.to_string(),
            house_names,
            cells,
            links,
        }
    }

    /// Find a forced deduction from full-house coloring constraints.
    ///
    /// Enumerates all valid 3-colorings of the full-house cell system
    /// (each house = one S₃ permutation), filtered by adjacency constraints
    /// and non-house cell availability. Returns:
    /// - Contradiction: no valid coloring exists (terminal)
    /// - ForcedSame(a,b): cells a,b always same color → merge
    /// - ForcedDiff(a,b): cells a,b always differ and not adjacent → add edge
    fn find_house_coloring_deduction(&self) -> Option<HouseColoringResult> {
        let all_houses = self.find_full_houses();
        // Only houses with 3 distinct active vertices
        let houses: Vec<&FullHouse> = all_houses.iter()
            .filter(|h| {
                h.cells[0] != h.cells[1] &&
                h.cells[0] != h.cells[2] &&
                h.cells[1] != h.cells[2]
            })
            .collect();

        if houses.len() < 2 { return None; }
        if houses.len() > 8 { return None; } // complexity limit: 6^8 ≈ 1.7M

        // Collect unique vertices across all houses
        let mut cell_set: Vec<usize> = Vec::new();
        for h in &houses {
            for &c in &h.cells {
                if !cell_set.contains(&c) {
                    cell_set.push(c);
                }
            }
        }
        cell_set.sort();
        let n_cells = cell_set.len();

        // Map vertex → local index
        let cell_to_local = |v: usize| -> usize {
            cell_set.binary_search(&v).unwrap()
        };

        // Map each house's cells to local indices
        let house_local: Vec<[usize; 3]> = houses.iter()
            .map(|h| [
                cell_to_local(h.cells[0]),
                cell_to_local(h.cells[1]),
                cell_to_local(h.cells[2]),
            ])
            .collect();

        // Non-house active vertices (for availability checking)
        let non_house: Vec<usize> = self.active_verts().into_iter()
            .filter(|v| cell_set.binary_search(v).is_err())
            .collect();

        // Adjacency between non-house cells and house cells (bitmask over local indices)
        let nh_adj_masks: Vec<u32> = non_house.iter()
            .map(|&nhc| {
                let mut mask = 0u32;
                for (li, &hc) in cell_set.iter().enumerate() {
                    if self.adj[nhc] & (1 << hc) != 0 {
                        mask |= 1u32 << li;
                    }
                }
                mask
            })
            .collect();

        const PERMS: [[u8; 3]; 6] = [
            [0,1,2], [0,2,1], [1,0,2], [1,2,0], [2,0,1], [2,1,0]
        ];

        let n_houses = houses.len();
        let n_combs = 6_usize.pow(n_houses as u32);

        // Track possible relationships: flat matrix [i * n_cells + j]
        // Bits: same_possible, diff_possible
        let matrix_size = n_cells * n_cells;
        let mut same_possible = vec![false; matrix_size];
        let mut diff_possible = vec![false; matrix_size];
        let mut any_valid = false;
        // Early termination: count how many pairs are still "undecided"
        // (only same or only diff seen so far)
        let n_check_pairs = n_cells * (n_cells - 1) / 2;
        let mut decided_count = 0usize;

        'combo: for combo in 0..n_combs {
            let mut colors = [255u8; 30]; // max 30 cells
            let mut c = combo;
            let mut valid = true;

            // Assign colors from each house's permutation
            for hi in 0..n_houses {
                let pi = c % 6;
                c /= 6;
                let perm = &PERMS[pi];
                for k in 0..3 {
                    let local = house_local[hi][k];
                    let color = perm[k];
                    if colors[local] == 255 {
                        colors[local] = color;
                        // Check adjacency with already-assigned cells
                        for other in 0..n_cells {
                            if other == local { continue; }
                            if colors[other] == 255 { continue; }
                            if self.adj[cell_set[local]] & (1 << cell_set[other]) != 0 {
                                if colors[other] == color {
                                    valid = false;
                                    break;
                                }
                            }
                        }
                        if !valid { break; }
                    } else if colors[local] != color {
                        valid = false;
                        break;
                    }
                }
                if !valid { break; }
            }
            if !valid { continue; }

            // Check non-house cell availability
            for (ni, &_nhc) in non_house.iter().enumerate() {
                let mask = nh_adj_masks[ni];
                let mut avail: u8 = 0b111; // colors 0,1,2
                let mut bits = mask;
                while bits != 0 {
                    let li = bits.trailing_zeros() as usize;
                    if colors[li] != 255 {
                        avail &= !(1u8 << colors[li]);
                    }
                    bits &= bits - 1;
                }
                // Also check adjacency with OTHER non-house cells? No — we'd
                // need to enumerate their colors too. Just check house-adjacent.
                if avail == 0 {
                    // This non-house cell can't be colored
                    // Also check if it has adjacency to non-house cells that
                    // might rescue it — but without enumerating, we can't.
                    // However, avail==0 means it conflicts with 3 house cells
                    // of 3 different colors. This is a genuine conflict.
                    continue 'combo;
                }
            }

            any_valid = true;

            // Record relationships
            for i in 0..n_cells {
                for j in (i+1)..n_cells {
                    let idx = i * n_cells + j;
                    let was_only_same = same_possible[idx] && !diff_possible[idx];
                    let was_only_diff = !same_possible[idx] && diff_possible[idx];

                    if colors[i] == colors[j] {
                        same_possible[idx] = true;
                    } else {
                        diff_possible[idx] = true;
                    }

                    // Track when a pair becomes "both possible" (no longer useful)
                    if was_only_same && diff_possible[idx] {
                        decided_count += 1;
                    } else if was_only_diff && same_possible[idx] {
                        decided_count += 1;
                    }
                }
            }

            // Early exit if all pairs have both same and diff possible
            if decided_count >= n_check_pairs {
                return None;
            }
        }

        if !any_valid {
            // No valid coloring exists — house system is contradictory
            return Some(HouseColoringResult::Contradiction);
        }

        // Find forced relationships — prefer same-color (merge) over diff-color (edge)
        for i in 0..n_cells {
            for j in (i+1)..n_cells {
                let idx = i * n_cells + j;
                if same_possible[idx] && !diff_possible[idx] {
                    return Some(HouseColoringResult::ForcedSame(cell_set[i], cell_set[j]));
                }
            }
        }
        for i in 0..n_cells {
            for j in (i+1)..n_cells {
                let idx = i * n_cells + j;
                if diff_possible[idx] && !same_possible[idx] {
                    if self.adj[cell_set[i]] & (1 << cell_set[j]) == 0 {
                        return Some(HouseColoringResult::ForcedDiff(cell_set[i], cell_set[j]));
                    }
                }
            }
        }

        None
    }

    // ── Pigeonhole X-wing ───────────────────────────────────────────

    /// Propagate forced-color constraints through full houses.
    ///
    /// Given cells forced to share a color X (seeds), propagates:
    /// - In a full house, if one cell is X → others are not-X
    /// - In a full house, if two cells are not-X → third is X
    ///
    /// Returns the first pair of adjacent forced-X cells (contradiction),
    /// or None if propagation reaches a fixed point without contradiction.
    fn propagate_forced_color(
        seeds: u32,
        houses: &[&FullHouse],
        adj: &[u32],
    ) -> Option<(usize, usize)> {
        let mut forced_x = seeds;
        let mut forced_not_x: u32 = 0;

        loop {
            let old_x = forced_x;
            let old_not = forced_not_x;

            for h in houses {
                let c = [h.cells[0], h.cells[1], h.cells[2]];
                let h_mask = (1u32 << c[0]) | (1u32 << c[1]) | (1u32 << c[2]);

                let x_in_h = (forced_x & h_mask).count_ones();
                let nx_in_h = (forced_not_x & h_mask).count_ones();

                // Two X cells in same house → contradiction
                if x_in_h >= 2 {
                    let mut bits = forced_x & h_mask;
                    let a = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let b = bits.trailing_zeros() as usize;
                    return Some((a, b));
                }

                // One X cell → others are not-X
                if x_in_h == 1 {
                    forced_not_x |= h_mask & !forced_x;
                }

                // Two not-X cells, no X cell yet → third is X
                if nx_in_h >= 2 && x_in_h == 0 {
                    let unknown = h_mask & !forced_x & !forced_not_x;
                    if unknown.count_ones() == 1 {
                        forced_x |= unknown;
                    }
                }
            }

            // Check: any two forced-X cells adjacent?
            let mut bits = forced_x;
            while bits != 0 {
                let a = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let clash = adj[a] & forced_x;
                if clash != 0 {
                    let b = clash.trailing_zeros() as usize;
                    return Some((a, b));
                }
            }

            if forced_x == old_x && forced_not_x == old_not {
                break;
            }
        }

        None
    }

    /// Detect a pigeonhole X-wing contradiction.
    ///
    /// Finds a chordless 4-cycle (induced C₄). By pigeonhole, any
    /// 3-coloring forces at least one diagonal to share a color.
    /// Propagates forced-X / forced-not-X through full houses for each
    /// diagonal. If both diagonals independently lead to a contradiction
    /// (two forced-X cells in the same house), the graph is not
    /// 3-colorable.
    fn find_pigeonhole_xwing(&self) -> Option<ProofNode> {
        let all_houses = self.find_full_houses();
        let houses: Vec<&FullHouse> = all_houses.iter()
            .filter(|h| {
                h.cells[0] != h.cells[1] &&
                h.cells[0] != h.cells[2] &&
                h.cells[1] != h.cells[2]
            })
            .collect();

        if houses.len() < 2 { return None; }

        let active = self.active_verts();
        let n = active.len();

        // For each pair of non-adjacent vertices (potential diagonal)
        for i in 0..n {
            let a = active[i];
            for j in (i + 1)..n {
                let c = active[j];
                if self.adj[a] & (1 << c) != 0 { continue; }

                // Common neighbors of a and c in the active graph
                let common = self.adj[a] & self.adj[c] & self.active;
                if common.count_ones() < 2 { continue; }

                let mut common_verts = Vec::new();
                let mut cbits = common;
                while cbits != 0 {
                    common_verts.push(cbits.trailing_zeros() as usize);
                    cbits &= cbits - 1;
                }

                for bi in 0..common_verts.len() {
                    let b = common_verts[bi];
                    for di in (bi + 1)..common_verts.len() {
                        let d = common_verts[di];
                        if self.adj[b] & (1 << d) != 0 { continue; }

                        // Chordless 4-cycle a-b-c-d-a
                        // Diagonal 1: (a,c), Diagonal 2: (b,d)
                        let clash1 = Self::propagate_forced_color(
                            (1 << a) | (1 << c), &houses, &self.adj,
                        );
                        let clash2 = Self::propagate_forced_color(
                            (1 << b) | (1 << d), &houses, &self.adj,
                        );

                        if let (Some((c1a, c1b)), Some((c2a, c2b))) = (clash1, clash2) {
                            return Some(ProofNode::PigeonholeXwing {
                                cycle: [
                                    self.vertex_name(a),
                                    self.vertex_name(b),
                                    self.vertex_name(c),
                                    self.vertex_name(d),
                                ],
                                clash_1: (self.vertex_name(c1a), self.vertex_name(c1b)),
                                clash_2: (self.vertex_name(c2a), self.vertex_name(c2b)),
                            });
                        }
                    }
                }
            }
        }

        None
    }
}

/// Generate all k-element combinations from a slice.
fn combinations<T: Clone>(items: &[T], k: usize) -> Vec<Vec<T>> {
    if k == 0 { return vec![vec![]]; }
    if k > items.len() { return vec![]; }
    if k == items.len() { return vec![items.to_vec()]; }

    let mut result = Vec::new();
    for i in 0..=items.len() - k {
        for mut rest in combinations(&items[i + 1..], k - 1) {
            let mut combo = vec![items[i].clone()];
            combo.append(&mut rest);
            result.push(combo);
        }
    }
    result
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
    /// Odd wheel found: hub forces rim to 2 colors, but rim is an odd cycle.
    OddWheel {
        hub: String,
        rim: Vec<String>,
    },
    /// Circular ladder (3-prism): satellites on different rungs forced to
    /// distinct colors. Add edges between them and continue.
    CircularLadder {
        rungs: [(String, String); 3],
        satellites: Vec<(usize, String)>, // (rung_index, vertex_name)
        next: Box<ProofNode>,
    },
    /// Bridged hexagon: induced C₆ with 3 bridge pairs on opposite edges.
    /// Each bridge forces opposite edges to miss different colors, but every
    /// 3-coloring of C₆ has a pair of opposite edges with the same missing
    /// color. Terminal contradiction.
    BridgedHexagon {
        ring: [String; 6],
        bridges: [(String, String); 3], // (sat on edge_i, sat on edge_{i+3})
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
    /// SET Equivalence Theory deduction.
    /// Combines k full houses of one type as positive with k of another type
    /// as negative. After canceling intersections, remaining cells (LHS = RHS)
    /// must have equal color multisets.
    /// Terminal when m=1 with adjacent cells (contradiction).
    /// Non-terminal when m=1 merge or m=2/3 virtual edges (has next).
    SetEquivalence {
        equation: String,      // e.g. "rows {3,6,9} − cols {3,6,9}"
        lhs: Vec<String>,      // remaining LHS cell names
        rhs: Vec<String>,      // remaining RHS cell names
        deduction: String,     // human-readable deduction
        is_contradiction: bool,
        next: Option<Box<ProofNode>>,
    },
    /// Parity transport (trivalue oddagon).
    /// A cycle of full houses where adjacent houses have 1-1 cell connections.
    /// Going around the cycle, the composition of connecting permutations has
    /// odd parity, which contradicts 3-colorability (the derangements of S₃
    /// are both even permutations, so net parity around any valid cycle must
    /// be even).
    ParityTransport {
        /// Houses in the cycle: (house_description, [cell names; 3]).
        houses: Vec<(String, [String; 3])>,
        /// For each link i → (i+1) mod len: description string.
        connections: Vec<String>,
    },
    /// Parity transport (pigeonhole on permutation parity classes).
    /// ≥4 rows (or columns) each span all 3 stacks (or bands), connected
    /// by parallel 1-1 links forcing same permutation parity. Every pair
    /// shares visibility at ≥1 stack → distinct permutations required.
    /// 4 distinct same-parity permutations from only 3 available →
    /// pigeonhole contradiction.
    ParityChain {
        house_type: String,         // "row" or "col"
        house_names: Vec<String>,   // e.g. ["row 5", "row 6", "row 8", "row 9"]
        cells: Vec<[String; 3]>,    // cells per house sorted by stack/band
        links: Vec<String>,         // parallel link descriptions
    },
    /// Non-terminal deduction from full-house coloring constraint analysis.
    /// All valid 3-colorings of the full-house system force two cells to have
    /// the same or different color → merge or add edge, then continue.
    ParityTransportDeduction {
        houses: Vec<String>,
        cell_a: String,
        cell_b: String,
        forced_same: bool,
        next: Box<ProofNode>,
    },
    /// Terminal contradiction from full-house coloring constraint analysis.
    /// No valid 3-coloring of the house system exists.
    HouseColoringContradiction {
        houses: Vec<String>,
    },
    /// Pigeonhole X-wing: chordless 4-cycle (induced C₄) where by
    /// pigeonhole both diag­onals sharing a color lead to contradiction
    /// via forced-color propagation through full houses.
    PigeonholeXwing {
        cycle: [String; 4],           // a-b-c-d in cycle order
        clash_1: (String, String),     // contradicting pair for diagonal (a,c)
        clash_2: (String, String),     // contradicting pair for diagonal (b,d)
    },
    /// Proof search exhausted depth limit.
    Failed,
}

impl ProofNode {
    pub fn is_complete(&self) -> bool {
        match self {
            ProofNode::K4Contradiction { .. } => true,
            ProofNode::OddWheel { .. } => true,
            ProofNode::BridgedHexagon { .. } => true,
            ProofNode::ParityTransport { .. } => true,
            ProofNode::ParityChain { .. } => true,
            ProofNode::HouseColoringContradiction { .. } => true,
            ProofNode::PigeonholeXwing { .. } => true,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { true } else { next.as_ref().unwrap().is_complete() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.is_complete(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.is_complete() && diff_color.is_complete()
            }
            ProofNode::Failed => false,
        }
    }

    pub fn depth(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. } | ProofNode::ParityTransport { .. }
            | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { 1 + next.as_ref().unwrap().depth() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => 1 + next.depth(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                1 + same_color.depth().max(diff_color.depth())
            }
        }
    }

    pub fn branch_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. } | ProofNode::ParityTransport { .. }
            | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().branch_count() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.branch_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                1 + same_color.branch_count() + diff_color.branch_count()
            }
        }
    }

    pub fn diamond_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. } | ProofNode::ParityTransport { .. }
            | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().diamond_count() }
            }
            ProofNode::DiamondMerge { next, .. } => 1 + next.diamond_count(),
            ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.diamond_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.diamond_count() + diff_color.diamond_count()
            }
        }
    }
    pub fn odd_wheel_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::BridgedHexagon { .. }
            | ProofNode::ParityTransport { .. } | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::OddWheel { .. } => 1,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().odd_wheel_count() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.odd_wheel_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.odd_wheel_count() + diff_color.odd_wheel_count()
            }
        }
    }
    pub fn circular_ladder_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. } | ProofNode::ParityTransport { .. }
            | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().circular_ladder_count() }
            }
            ProofNode::CircularLadder { next, .. } => 1 + next.circular_ladder_count(),
            ProofNode::DiamondMerge { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.circular_ladder_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.circular_ladder_count() + diff_color.circular_ladder_count()
            }
        }
    }
    pub fn bridged_hexagon_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::ParityTransport { .. } | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::BridgedHexagon { .. } => 1,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().bridged_hexagon_count() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.bridged_hexagon_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.bridged_hexagon_count() + diff_color.bridged_hexagon_count()
            }
        }
    }
    pub fn set_equivalence_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. } | ProofNode::ParityTransport { .. }
            | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                1 + if *is_contradiction { 0 } else { next.as_ref().unwrap().set_equivalence_count() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.set_equivalence_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.set_equivalence_count() + diff_color.set_equivalence_count()
            }
        }
    }
    pub fn parity_transport_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::PigeonholeXwing { .. }
            | ProofNode::Failed => 0,
            ProofNode::ParityTransport { .. } => 1,
            ProofNode::ParityChain { .. } => 1,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().parity_transport_count() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. } => next.parity_transport_count(),
            ProofNode::ParityTransportDeduction { next, .. } => 1 + next.parity_transport_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.parity_transport_count() + diff_color.parity_transport_count()
            }
        }
    }
    pub fn pigeonhole_xwing_count(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } | ProofNode::OddWheel { .. }
            | ProofNode::BridgedHexagon { .. } | ProofNode::ParityTransport { .. }
            | ProofNode::ParityChain { .. }
            | ProofNode::HouseColoringContradiction { .. }
            | ProofNode::Failed => 0,
            ProofNode::PigeonholeXwing { .. } => 1,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 0 } else { next.as_ref().unwrap().pigeonhole_xwing_count() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => next.pigeonhole_xwing_count(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                same_color.pigeonhole_xwing_count() + diff_color.pigeonhole_xwing_count()
            }
        }
    }
    pub fn size(&self) -> usize {
        match self {
            ProofNode::K4Contradiction { .. } => 1,
            ProofNode::OddWheel { .. } => 1,
            ProofNode::BridgedHexagon { .. } => 1,
            ProofNode::ParityTransport { .. } => 1,
            ProofNode::ParityChain { .. } => 1,
            ProofNode::HouseColoringContradiction { .. } => 1,
            ProofNode::PigeonholeXwing { .. } => 1,
            ProofNode::Failed => usize::MAX / 2,
            ProofNode::SetEquivalence { is_contradiction, next, .. } => {
                if *is_contradiction { 1 } else { 1 + next.as_ref().unwrap().size() }
            }
            ProofNode::DiamondMerge { next, .. } | ProofNode::CircularLadder { next, .. }
            | ProofNode::ParityTransportDeduction { next, .. } => 1 + next.size(),
            ProofNode::Branch { same_color, diff_color, .. } => {
                1 + same_color.size() + diff_color.size()
            }
        }
    }
}

// ── SET helpers ─────────────────────────────────────────────────────

/// Format a house identifier for display (1-based).
fn format_house(h: &FullHouse) -> String {
    match h.htype {
        HouseType::Row => format!("row {}", h.index + 1),
        HouseType::Col => format!("col {}", h.index + 1),
        HouseType::Box => format!("box {}", h.index + 1),
    }
}

/// Format a SET equation string.
fn format_set_equation(eq: &SetEquation, graph: &ProofGraph) -> (String, Vec<String>, Vec<String>, String) {
    let pos_str: Vec<String> = eq.positive.iter().map(|h| format_house(h)).collect();
    let neg_str: Vec<String> = eq.negative.iter().map(|h| format_house(h)).collect();
    let equation = format!(
        "{{{}}}\u{2009}\u{2212}\u{2009}{{{}}}",
        pos_str.join(", "),
        neg_str.join(", "),
    );
    let lhs_names: Vec<String> = eq.lhs.iter().map(|&v| graph.vertex_name(v)).collect();
    let rhs_names: Vec<String> = eq.rhs.iter().map(|&v| graph.vertex_name(v)).collect();

    let deduction = match eq.m {
        1 => {
            if graph.adj[eq.lhs[0]] & (1 << eq.rhs[0]) != 0 {
                format!(
                    "color({}) = color({}), but they share a house. Contradiction.",
                    lhs_names[0], rhs_names[0]
                )
            } else {
                format!(
                    "color({}) = color({}). Identify.",
                    lhs_names[0], rhs_names[0]
                )
            }
        }
        _ => {
            format!(
                "Both sides forced all-distinct. Virtual edges added.",
            )
        }
    };

    (equation, lhs_names, rhs_names, deduction)
}

// ── Proof search ────────────────────────────────────────────────────

/// Search for the shortest complete proof with at most `branches_left` branch
/// nodes and at most `size_budget` total proof nodes.
/// K₄, odd wheels, and bridged hexagons are terminal (size 1) and returned
/// immediately. Diamonds and branches are both treated as choices: all
/// available diamonds and branch pairs are tried, and the one producing the
/// smallest complete proof is kept.
fn find_best_proof(
    graph: &ProofGraph,
    branches_left: usize,
    depth_remaining: usize,
    size_budget: usize,
) -> ProofNode {
    if size_budget == 0 {
        return ProofNode::Failed;
    }

    // Terminal: K₄ found — contradiction, size 1, can't be beaten
    if let Some(k4) = graph.find_k4() {
        return ProofNode::K4Contradiction {
            vertices: k4.map(|v| graph.vertex_name(v)),
        };
    }

    // Terminal: odd wheel found — contradiction, size 1, can't be beaten
    if let Some((hub, rim)) = graph.find_odd_wheel() {
        return ProofNode::OddWheel {
            hub: graph.vertex_name(hub),
            rim: rim.iter().map(|&v| graph.vertex_name(v)).collect(),
        };
    }

    // Terminal: parity transport (trivalue oddagon)
    if let Some(pt) = graph.find_parity_transport() {
        return pt;
    }

    // Terminal: parity transport (pigeonhole chain)
    if let Some(pc) = graph.find_parity_chain() {
        return pc;
    }

    // Terminal: bridged hexagon
    if let Some((ring, bridges)) = graph.find_bridged_hexagon() {
        return ProofNode::BridgedHexagon {
            ring: ring.map(|v| graph.vertex_name(v)),
            bridges: bridges.map(|(s1, s2)| (graph.vertex_name(s1), graph.vertex_name(s2))),
        };
    }

    // Terminal: pigeonhole X-wing
    if let Some(xw) = graph.find_pigeonhole_xwing() {
        return xw;
    }

    if depth_remaining == 0 {
        return ProofNode::Failed;
    }

    let mut best: Option<ProofNode> = None;
    let mut best_size = size_budget;

    // Try all diamond reductions
    let diamonds = graph.find_all_diamonds();
    for (a, b, u, v) in &diamonds {
        let tip_a_name = graph.vertex_name(*a);
        let tip_b_name = graph.vertex_name(*b);
        let spine_u_name = graph.vertex_name(*u);
        let spine_v_name = graph.vertex_name(*v);

        let mut g = graph.clone();
        let (keep, remove) = ((*a).min(*b), (*a).max(*b));
        g.merge(keep, remove);

        let next = find_best_proof(&g, branches_left, depth_remaining - 1, best_size - 1);
        if next.is_complete() {
            let total = 1 + next.size();
            if total < best_size {
                best_size = total;
                best = Some(ProofNode::DiamondMerge {
                    tip_a: tip_a_name,
                    tip_b: tip_b_name,
                    spine_u: spine_u_name,
                    spine_v: spine_v_name,
                    next: Box::new(next),
                });
            }
        }
    }

    // Try all circular ladder reductions
    let ladders = graph.find_all_circular_ladders();
    for (rungs, satellites) in &ladders {
        let rung_names: [(String, String); 3] = [
            (graph.vertex_name(rungs[0].0), graph.vertex_name(rungs[0].1)),
            (graph.vertex_name(rungs[1].0), graph.vertex_name(rungs[1].1)),
            (graph.vertex_name(rungs[2].0), graph.vertex_name(rungs[2].1)),
        ];
        let sat_names: Vec<(usize, String)> = satellites.iter()
            .map(|&(ri, s)| (ri, graph.vertex_name(s)))
            .collect();

        let mut g = graph.clone();
        for si in 0..satellites.len() {
            for sj in (si + 1)..satellites.len() {
                g.add_edge(satellites[si].1, satellites[sj].1);
            }
        }

        let next = find_best_proof(&g, branches_left, depth_remaining - 1, best_size - 1);
        if next.is_complete() {
            let total = 1 + next.size();
            if total < best_size {
                best_size = total;
                best = Some(ProofNode::CircularLadder {
                    rungs: rung_names,
                    satellites: sat_names,
                    next: Box::new(next),
                });
            }
        }
    }

    // Try all SET equivalence deductions
    let set_deds = graph.find_set_deductions();
    for (eq, ded) in &set_deds {
        let (equation, lhs_names, rhs_names, ded_text) = format_set_equation(eq, graph);

        match ded {
            SetDeduction::Contradiction { .. } => {
                // Terminal — size 1
                if 1 < best_size {
                    best_size = 1;
                    best = Some(ProofNode::SetEquivalence {
                        equation,
                        lhs: lhs_names,
                        rhs: rhs_names,
                        deduction: ded_text,
                        is_contradiction: true,
                        next: None,
                    });
                }
            }
            SetDeduction::Merge { lhs_v, rhs_v } => {
                let mut g = graph.clone();
                let (keep, remove) = ((*lhs_v).min(*rhs_v), (*lhs_v).max(*rhs_v));
                g.merge(keep, remove);

                let sub = find_best_proof(&g, branches_left, depth_remaining - 1, best_size - 1);
                if sub.is_complete() {
                    let total = 1 + sub.size();
                    if total < best_size {
                        best_size = total;
                        best = Some(ProofNode::SetEquivalence {
                            equation,
                            lhs: lhs_names,
                            rhs: rhs_names,
                            deduction: ded_text,
                            is_contradiction: false,
                            next: Some(Box::new(sub)),
                        });
                    }
                }
            }
            SetDeduction::VirtualEdges { edges } => {
                let mut g = graph.clone();
                for &(a, b) in edges {
                    g.add_edge(a, b);
                }

                let sub = find_best_proof(&g, branches_left, depth_remaining - 1, best_size - 1);
                if sub.is_complete() {
                    let total = 1 + sub.size();
                    if total < best_size {
                        best_size = total;
                        best = Some(ProofNode::SetEquivalence {
                            equation,
                            lhs: lhs_names,
                            rhs: rhs_names,
                            deduction: ded_text,
                            is_contradiction: false,
                            next: Some(Box::new(sub)),
                        });
                    }
                }
            }
        }
    }

    // Try branches (only if allowed)
    if branches_left > 0 {
        let pairs = graph.branch_candidates();

        for (u, v) in pairs {
            let sub_budget = best_size - 1; // 1 for the branch node itself
            if sub_budget == 0 { break; }

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
            if total < best_size {
                best_size = total;
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
    }

    best.unwrap_or(ProofNode::Failed)
}

/// Greedy/hierarchical proof search: applies techniques in strict priority
/// order (K₄ → diamond → SET → circular ladder → odd wheel → branch).
/// This doesn't find the shortest proof, but classifies difficulty: a pattern
/// that needs branching under this strategy is genuinely harder than one
/// solvable by diamonds alone.
fn find_greedy_proof(
    graph: &ProofGraph,
    branches_left: usize,
    depth_remaining: usize,
) -> ProofNode {
    // Terminal: K₄
    if let Some(k4) = graph.find_k4() {
        return ProofNode::K4Contradiction {
            vertices: k4.map(|v| graph.vertex_name(v)),
        };
    }

    // Terminal: parity transport (trivalue oddagon)
    if let Some(pt) = graph.find_parity_transport() {
        return pt;
    }

    // Terminal: parity transport (pigeonhole chain)
    if let Some(pc) = graph.find_parity_chain() {
        return pc;
    }

    if depth_remaining == 0 {
        return ProofNode::Failed;
    }

    // Priority 1: diamond (first found, apply greedily)
    // Diamonds are the simplest deduction; SET generalizes diamonds over
    // multiple multiset pairs, so diamonds should always be tried first.
    let diamonds = graph.find_all_diamonds();
    if let Some(&(a, b, u, v)) = diamonds.first() {
        let tip_a = graph.vertex_name(a);
        let tip_b = graph.vertex_name(b);
        let spine_u = graph.vertex_name(u);
        let spine_v = graph.vertex_name(v);

        let mut g = graph.clone();
        let (keep, remove) = (a.min(b), a.max(b));
        g.merge(keep, remove);

        let next = find_greedy_proof(&g, branches_left, depth_remaining - 1);
        return ProofNode::DiamondMerge {
            tip_a,
            tip_b,
            spine_u,
            spine_v,
            next: Box::new(next),
        };
    }

    // Priority 2: SET equivalence (after diamonds exhausted)
    let set_deds = graph.find_set_deductions();
    if let Some((eq, ded)) = set_deds.into_iter().next() {
        let (equation, lhs_names, rhs_names, ded_text) = format_set_equation(&eq, graph);

        match ded {
            SetDeduction::Contradiction { .. } => {
                return ProofNode::SetEquivalence {
                    equation,
                    lhs: lhs_names,
                    rhs: rhs_names,
                    deduction: ded_text,
                    is_contradiction: true,
                    next: None,
                };
            }
            SetDeduction::Merge { lhs_v, rhs_v } => {
                let mut g = graph.clone();
                let (keep, remove) = (lhs_v.min(rhs_v), lhs_v.max(rhs_v));
                g.merge(keep, remove);
                let sub = find_greedy_proof(&g, branches_left, depth_remaining - 1);
                return ProofNode::SetEquivalence {
                    equation,
                    lhs: lhs_names,
                    rhs: rhs_names,
                    deduction: ded_text,
                    is_contradiction: false,
                    next: Some(Box::new(sub)),
                };
            }
            SetDeduction::VirtualEdges { edges } => {
                let mut g = graph.clone();
                for (a, b) in edges {
                    g.add_edge(a, b);
                }
                let sub = find_greedy_proof(&g, branches_left, depth_remaining - 1);
                return ProofNode::SetEquivalence {
                    equation,
                    lhs: lhs_names,
                    rhs: rhs_names,
                    deduction: ded_text,
                    is_contradiction: false,
                    next: Some(Box::new(sub)),
                };
            }
        }
    }

    // Priority 3: circular ladder (after diamonds and SET exhausted)
    let ladders = graph.find_all_circular_ladders();
    if let Some((rungs, satellites)) = ladders.into_iter().next() {
        let rung_names: [(String, String); 3] = [
            (graph.vertex_name(rungs[0].0), graph.vertex_name(rungs[0].1)),
            (graph.vertex_name(rungs[1].0), graph.vertex_name(rungs[1].1)),
            (graph.vertex_name(rungs[2].0), graph.vertex_name(rungs[2].1)),
        ];
        let sat_names: Vec<(usize, String)> = satellites.iter()
            .map(|&(ri, s)| (ri, graph.vertex_name(s)))
            .collect();

        let mut g = graph.clone();
        for si in 0..satellites.len() {
            for sj in (si + 1)..satellites.len() {
                g.add_edge(satellites[si].1, satellites[sj].1);
            }
        }

        let next = find_greedy_proof(&g, branches_left, depth_remaining - 1);
        return ProofNode::CircularLadder {
            rungs: rung_names,
            satellites: sat_names,
            next: Box::new(next),
        };
    }

    // Priority 4: bridged hexagon
    if let Some((ring, bridges)) = graph.find_bridged_hexagon() {
        return ProofNode::BridgedHexagon {
            ring: ring.map(|v| graph.vertex_name(v)),
            bridges: bridges.map(|(s1, s2)| (graph.vertex_name(s1), graph.vertex_name(s2))),
        };
    }

    // Priority 5: pigeonhole X-wing
    if let Some(xw) = graph.find_pigeonhole_xwing() {
        return xw;
    }

    // Priority 6: odd wheel (after diamonds, SET, and ladders exhausted)
    if let Some((hub, rim)) = graph.find_odd_wheel() {
        return ProofNode::OddWheel {
            hub: graph.vertex_name(hub),
            rim: rim.iter().map(|&v| graph.vertex_name(v)).collect(),
        };
    }

    // Priority 5: branch (last resort — try all pairs, pick best)
    if branches_left == 0 {
        return ProofNode::Failed;
    }

    let pairs = graph.branch_candidates();
    let mut best: Option<ProofNode> = None;
    let mut best_size = usize::MAX;

    for (u, v) in pairs {
        let mut g_same = graph.clone();
        let (keep, remove) = (u.min(v), u.max(v));
        g_same.merge(keep, remove);
        let same_proof = find_greedy_proof(&g_same, branches_left - 1, depth_remaining - 1);
        if !same_proof.is_complete() { continue; }

        let mut g_diff = graph.clone();
        g_diff.add_edge(u, v);
        let diff_proof = find_greedy_proof(&g_diff, branches_left - 1, depth_remaining - 1);
        if !diff_proof.is_complete() { continue; }

        let total = 1 + same_proof.size() + diff_proof.size();
        if total < best_size {
            best_size = total;
            best = Some(ProofNode::Branch {
                vertex_a: graph.vertex_name(u),
                vertex_b: graph.vertex_name(v),
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
        ProofNode::OddWheel { hub, rim } => {
            *step += 1;
            let rim_str = rim.join(", ");
            format!(
                "{}{}.  Odd wheel: hub {} forces rim to 2 colors.\n\
                 {}    Bivalue oddagon {{{}}} (length {}) cannot be 2-colored. Contradiction.\n",
                pad, step, hub,
                pad, rim_str, rim.len(),
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
        ProofNode::CircularLadder { rungs, satellites, next } => {
            *step += 1;
            let rung_strs: Vec<String> = rungs.iter()
                .map(|(a, b)| format!("{}\u{2014}{}", a, b))
                .collect();
            let sat_strs: Vec<String> = satellites.iter()
                .map(|(ri, name)| format!("{} (rung {})", name, ri + 1))
                .collect();
            let action = if satellites.len() >= 3 { "Add triangle" } else { "Add edge" };
            let mut s = format!(
                "{}{}.  Circular ladder {{{}}}:\n",
                pad, step, rung_strs.join(", "),
            );
            s += &format!(
                "{}    Satellites {} forced to distinct colors. {}.\n",
                pad, sat_strs.join(", "), action,
            );
            s += &format_node(next, step, indent);
            s
        }
        ProofNode::BridgedHexagon { ring, bridges } => {
            *step += 1;
            let ring_str = ring.iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let bridge_strs: Vec<String> = bridges.iter().enumerate()
                .map(|(i, (s1, s2))| {
                    let e1 = format!("{}\u{2014}{}", ring[i], ring[(i + 1) % 6]);
                    let e3 = format!("{}\u{2014}{}", ring[i + 3], ring[(i + 4) % 6]);
                    format!("{}\u{2014}{} (edges {} \u{2194} {})", s1, s2, e1, e3)
                })
                .collect();
            format!(
                "{}{}.  Bridged hexagon: ring {{{}}}\n\
                 {}    Bridges: {}.\n\
                 {}    Each bridge forces opposite edges to miss different colors,\n\
                 {}    but every 3-coloring of C\u{2086} requires at least one matching pair. Contradiction.\n",
                pad, step, ring_str,
                pad, bridge_strs.join(", "),
                pad,
                pad,
            )
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
        ProofNode::SetEquivalence { equation, lhs, rhs, deduction, is_contradiction, next } => {
            *step += 1;
            let lhs_str = format!("{{{}}}", lhs.join(", "));
            let rhs_str = format!("{{{}}}", rhs.join(", "));
            let mut s = format!(
                "{}{}.  SET: {}.\n\
                 {}    Remainder: {} = {}.\n\
                 {}    → {}\n",
                pad, step, equation,
                pad, lhs_str, rhs_str,
                pad, deduction,
            );
            if !is_contradiction {
                if let Some(n) = next {
                    s += &format_node(n, step, indent);
                }
            }
            s
        }
        ProofNode::HouseColoringContradiction { houses } => {
            *step += 1;
            let houses_str = houses.join(", ");
            format!(
                "{}{}.  House coloring constraint ({{{}}}):\n\
                 {}    No valid 3-coloring of these houses exists. Contradiction.\n",
                pad, step, houses_str, pad,
            )
        }
        ProofNode::ParityTransportDeduction { houses, cell_a, cell_b, forced_same, next } => {
            *step += 1;
            let houses_str = houses.join(", ");
            let action = if *forced_same {
                format!("color({}) = color({}). Identify.", cell_a, cell_b)
            } else {
                format!("color({}) \u{2260} color({}). Add edge.", cell_a, cell_b)
            };
            let mut s = format!(
                "{}{}.  House coloring constraint ({{{}}}):\n",
                pad, step, houses_str,
            );
            s += &format!(
                "{}    All valid 3-colorings force {}.\n",
                pad, action,
            );
            s += &format_node(next, step, indent);
            s
        }
        ProofNode::ParityTransport { houses, connections } => {
            *step += 1;
            let len = houses.len();
            let mut s = format!(
                "{}{}.  Trivalue oddagon:\n",
                pad, step,
            );
            for i in 0..len {
                let (ref hname, ref cells) = houses[i];
                s += &format!(
                    "{}    {} {{{}}}\n",
                    pad, hname, cells.join(", "),
                );
                s += &format!(
                    "{}      \u{2192} {}\n",
                    pad, connections[i],
                );
            }
            // Compute total parity from connection descriptions
            let odd_count = connections.iter()
                .filter(|c| c.contains("[odd]"))
                .count();
            let total = if odd_count % 2 == 1 { "odd" } else { "even" };
            s += &format!(
                "{}    Cycle parity: {}. Contradiction.\n",
                pad, total,
            );
            s
        }
        ProofNode::ParityChain { house_type, house_names, cells, links } => {
            *step += 1;
            let vis_name = if house_type == "row" { "column" } else { "row" };
            let mut s = format!(
                "{}{}.  Parity transport:\n",
                pad, step,
            );
            for i in 0..house_names.len() {
                s += &format!(
                    "{}    {} {{{}}}\n",
                    pad, house_names[i], cells[i].join(", "),
                );
            }
            s += &format!(
                "{}    Parallel links: {}\n",
                pad, links.join("; "),
            );
            s += &format!(
                "{}    \u{2192} same permutation parity.\n",
                pad,
            );
            s += &format!(
                "{}    Each pair shares a {} \u{2192} distinct permutations.\n",
                pad, vis_name,
            );
            s += &format!(
                "{}    {} same-parity permutations from 3 available \u{2192} pigeonhole contradiction.\n",
                pad, house_names.len(),
            );
            s
        }
        ProofNode::PigeonholeXwing { cycle, clash_1, clash_2 } => {
            *step += 1;
            let mut s = format!(
                "{}{}.  Pigeonhole X-wing on {{{}, {}, {}, {}}}:\n",
                pad, step, cycle[0], cycle[1], cycle[2], cycle[3],
            );
            s += &format!(
                "{}    Diagonals: {{{}, {}}} and {{{}, {}}} (non-adjacent).\n",
                pad, cycle[0], cycle[2], cycle[1], cycle[3],
            );
            s += &format!(
                "{}    By pigeonhole, one diagonal must share a color.\n",
                pad,
            );
            s += &format!(
                "{}    Case 1: color({}) = color({}) \u{2192} forces {} = {} (adjacent). Contradiction.\n",
                pad, cycle[0], cycle[2], clash_1.0, clash_1.1,
            );
            s += &format!(
                "{}    Case 2: color({}) = color({}) \u{2192} forces {} = {} (adjacent). Contradiction.\n",
                pad, cycle[1], cycle[3], clash_2.0, clash_2.1,
            );
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
    /// Difficulty metrics from the greedy/hierarchical proof.
    pub greedy_branches: usize,
    pub greedy_odd_wheels: usize,
    pub greedy_circular_ladders: usize,
    pub greedy_bridged_hexagons: usize,
    pub greedy_set_equivalences: usize,
    pub greedy_parity_transports: usize,
    pub greedy_pigeonhole_xwings: usize,
}

impl ProofResult {
    pub fn summary(&self) -> String {
        format!(
            "depth={} diamonds={} odd_wheels={} circular_ladders={} bridged_hexagons={} set_equivalences={} parity_transports={} pigeonhole_xwings={} branches={} complete={} greedy_branches={} greedy_odd_wheels={} greedy_circular_ladders={} greedy_bridged_hexagons={} greedy_set_equivalences={} greedy_parity_transports={} greedy_pigeonhole_xwings={}",
            self.proof.depth(),
            self.proof.diamond_count(),
            self.proof.odd_wheel_count(),
            self.proof.circular_ladder_count(),
            self.proof.bridged_hexagon_count(),
            self.proof.set_equivalence_count(),
            self.proof.parity_transport_count(),
            self.proof.pigeonhole_xwing_count(),
            self.proof.branch_count(),
            self.proof.is_complete(),
            self.greedy_branches,
            self.greedy_odd_wheels,
            self.greedy_circular_ladders,
            self.greedy_bridged_hexagons,
            self.greedy_set_equivalences,
            self.greedy_parity_transports,
            self.greedy_pigeonhole_xwings,
        )
    }
}

pub fn prove_pattern(cells: &[u8]) -> ProofResult {
    let graph = ProofGraph::from_cells(cells);

    // Optimal proof: iterative deepening on branch count, trying all choices
    let mut proof = ProofNode::Failed;
    for max_br in 0..=10 {
        proof = find_best_proof(&graph, max_br, 50, usize::MAX);
        if proof.is_complete() { break; }
    }

    // Greedy proof: strict priority order
    let mut greedy = ProofNode::Failed;
    for max_br in 0..=10 {
        greedy = find_greedy_proof(&graph, max_br, 50);
        if greedy.is_complete() { break; }
    }

    let text = format_proof(&proof);
    ProofResult {
        greedy_branches: greedy.branch_count(),
        greedy_odd_wheels: greedy.odd_wheel_count(),
        greedy_circular_ladders: greedy.circular_ladder_count(),
        greedy_bridged_hexagons: greedy.bridged_hexagon_count(),
        greedy_set_equivalences: greedy.set_equivalence_count(),
        greedy_parity_transports: greedy.parity_transport_count(),
        greedy_pigeonhole_xwings: greedy.pigeonhole_xwing_count(),
        proof,
        text,
    }
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
        let d = g.find_all_diamonds();
        assert!(!d.is_empty(), "should find diamond");
    }

    #[test]
    fn test_merge() {
        let cells = vec![0, 1, 9, 27];
        let mut g = ProofGraph::from_cells(&cells);
        // vertices: 0=cell0, 1=cell1, 2=cell9, 3=cell27
        // Diamond tips: 1(cell1) and 3(cell27), spine: 0(cell0) and 2(cell9)
        let d = g.find_all_diamonds();
        let (a, b, _, _) = d[0];
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
