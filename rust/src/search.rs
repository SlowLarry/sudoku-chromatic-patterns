/// Recursive backtracking search for minimal 4-chromatic patterns.

use std::collections::HashSet;
use std::time::Instant;

use crate::bitset::{iter_bits, popcount};
use crate::canonical::{canonical_signature, candidate_orbit_reps};
use crate::sudoku_graph::NEIGHBORS_MASK;
use crate::symmetry::orbit_reps_custom;
use crate::validation::{has_triangle_in_mask, is_valid_minimal_4chromatic_pattern};

use nauty_Traces_sys::setword;

/// Search statistics.
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    pub nodes: u64,
    pub k4_prunes: u64,
    pub degree_prunes: u64,
    pub symmetry_prunes: u64,
    pub orbit_prunes: u64,
    pub leaves: u64,
    pub solutions: u64,
    pub stopped: bool,
    pub stop_reason: Option<String>,
    pub elapsed_seconds: f64,
    pub progress_pct: f64,
}

/// Configuration for the search.
pub struct SearchConfig {
    pub target_size: usize,
    pub roots: Vec<u8>,
    pub limit: Option<u64>,
    pub symmetry_prune: bool,
    pub max_nodes: Option<u64>,
    pub max_seconds: Option<f64>,
    pub progress_seconds: Option<f64>,
    pub known_signatures: HashSet<Vec<setword>>,
    pub output_path: Option<String>,
    pub compact: bool,
    pub use_nauty_orbits: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            target_size: 10,
            roots: vec![0],
            limit: None,
            symmetry_prune: true,
            max_nodes: None,
            max_seconds: None,
            progress_seconds: None,
            known_signatures: HashSet::new(),
            output_path: None,
            compact: false,
            use_nauty_orbits: false,
        }
    }
}

/// Check degree feasibility: can every chosen vertex still reach degree >= 3?
fn degree_feasible(
    chosen_vertices: &[u8],
    chosen_mask: u128,
    degrees: &[u8; 81],
    target_size: usize,
    candidate_mask: u128,
) -> bool {
    let remaining = target_size.saturating_sub(chosen_vertices.len());
    let addable_mask = candidate_mask & !chosen_mask;
    for &v in chosen_vertices {
        if degrees[v as usize] >= 3 {
            continue;
        }
        let possible = (NEIGHBORS_MASK[v as usize] & addable_mask).count_ones();
        if degrees[v as usize] as u32 + possible < 3 {
            return false;
        }
    }
    let _ = remaining; // used implicitly by target_size check above
    true
}

/// Format a pattern as an 81-char 0/1 bit string.
pub fn pattern_bitstring(pattern: &[u8]) -> String {
    let mut bits = ['0'; 81];
    for &v in pattern {
        if (v as usize) < 81 {
            bits[v as usize] = '1';
        }
    }
    bits.iter().collect()
}

/// Run the exhaustive search.
pub fn search_minimal_4chromatic(config: &SearchConfig) -> (Vec<Vec<u8>>, SearchStats) {
    let target_size = config.target_size;
    if target_size == 0 {
        return (vec![], SearchStats::default());
    }

    // Force pool initialization and log orbit mode
    if config.symmetry_prune {
        if config.use_nauty_orbits {
            eprintln!("orbits: nauty (exact)");
        } else {
            let n = crate::symmetry::pool_size();
            eprintln!("orbits: custom ({} group elements)", n);
        }
    }

    let mut results: Vec<Vec<u8>> = Vec::new();
    let mut stats = SearchStats::default();
    let mut degrees = [0u8; 81];
    let mut seen_leaves: HashSet<Vec<setword>> = config.known_signatures.clone();

    let start = Instant::now();
    let mut last_progress = start;
    let mut progress_frac: f64 = 0.0;

    // Output file handle
    let output_file = config.output_path.as_ref().map(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .expect("cannot open output file")
    });
    let output_file = std::cell::RefCell::new(output_file);

    let _should_stop = |stats: &mut SearchStats, start: Instant| -> bool {
        if let Some(max) = config.max_nodes {
            if stats.nodes >= max {
                stats.stopped = true;
                stats.stop_reason = Some("max_nodes".to_string());
                return true;
            }
        }
        if let Some(max) = config.max_seconds {
            if start.elapsed().as_secs_f64() >= max {
                stats.stopped = true;
                stats.stop_reason = Some("max_seconds".to_string());
                return true;
            }
        }
        false
    };

    fn backtrack(
        chosen_vertices: &mut Vec<u8>,
        chosen_mask: u128,
        candidate_list: &mut Vec<u8>,
        candidate_mask: u128,
        start_pos: usize,
        target_size: usize,
        degrees: &mut [u8; 81],
        stats: &mut SearchStats,
        results: &mut Vec<Vec<u8>>,
        seen_leaves: &mut HashSet<Vec<setword>>,
        symmetry_prune: bool,
        use_nauty_orbits: bool,
        limit: Option<u64>,
        max_nodes: Option<u64>,
        max_seconds: Option<f64>,
        progress_seconds: Option<f64>,
        start: Instant,
        last_progress: &mut Instant,
        progress_frac: &mut f64,
        frac_lo: f64,
        frac_hi: f64,
        output_file: &std::cell::RefCell<Option<std::fs::File>>,
    ) -> bool {
        stats.nodes += 1;

        // Check stop conditions
        if let Some(max) = max_nodes {
            if stats.nodes >= max {
                stats.stopped = true;
                stats.stop_reason = Some("max_nodes".to_string());
                return true;
            }
        }
        if let Some(max) = max_seconds {
            if stats.nodes % 1024 == 0 && start.elapsed().as_secs_f64() >= max {
                stats.stopped = true;
                stats.stop_reason = Some("max_seconds".to_string());
                return true;
            }
        }

        // Progress reporting
        if let Some(ps) = progress_seconds {
            if stats.nodes % 4096 == 0 {
                let now = Instant::now();
                if now.duration_since(*last_progress).as_secs_f64() >= ps {
                    stats.elapsed_seconds = start.elapsed().as_secs_f64();
                    stats.progress_pct = *progress_frac * 100.0;
                    let rate = stats.nodes as f64 / stats.elapsed_seconds;
                    let pct = stats.progress_pct;
                    let eta = if pct > 0.0 {
                        let remaining = stats.elapsed_seconds * (100.0 - pct) / pct;
                        if remaining < 3600.0 {
                            format!(" eta={:.0}s", remaining)
                        } else {
                            format!(" eta={:.1}h", remaining / 3600.0)
                        }
                    } else {
                        String::new()
                    };
                    eprintln!(
                        "progress: {:.6}% nodes={} leaves={} solutions={} k4={} deg={} orb={} sym={} elapsed={:.1}s rate={:.1}/s{}",
                        pct, stats.nodes, stats.leaves, stats.solutions,
                        stats.k4_prunes, stats.degree_prunes, stats.orbit_prunes,
                        stats.symmetry_prunes, stats.elapsed_seconds, rate, eta,
                    );
                    *last_progress = now;
                }
            }
        }

        // Degree feasibility pruning
        if !degree_feasible(chosen_vertices, chosen_mask, degrees, target_size, candidate_mask) {
            stats.degree_prunes += 1;
            return false;
        }

        // Leaf: reached target size
        if chosen_vertices.len() == target_size {
            stats.leaves += 1;
            if is_valid_minimal_4chromatic_pattern(chosen_vertices) {
                if symmetry_prune {
                    let sig = canonical_signature(chosen_mask);
                    if seen_leaves.contains(&sig) {
                        stats.symmetry_prunes += 1;
                        return false;
                    }
                    seen_leaves.insert(sig);
                }
                let result = chosen_vertices.clone();
                let bs = pattern_bitstring(&result);
                println!("FOUND: {}", bs);

                // Append to output file
                if let Some(ref mut f) = *output_file.borrow_mut() {
                    use std::io::Write;
                    writeln!(f, "{}", bs).ok();
                }

                results.push(result);
                stats.solutions += 1;

                if let Some(lim) = limit {
                    if stats.solutions >= lim {
                        stats.stopped = true;
                        stats.stop_reason = Some("solution_limit".to_string());
                        return true;
                    }
                }
            }
            return false;
        }

        // Collect eligible candidates (respecting start_pos, not yet chosen)
        let mut eligible: Vec<(usize, u8)> = Vec::new();
        for idx in start_pos..candidate_list.len() {
            let v = candidate_list[idx];
            if (chosen_mask >> v) & 1 == 0 {
                eligible.push((idx, v));
            }
        }

        // Orbit pruning
        let to_try = if symmetry_prune && eligible.len() > 1 {
            let just_cells: Vec<u8> = eligible.iter().map(|&(_, v)| v).collect();
            let reps = if use_nauty_orbits {
                let (r, _) = candidate_orbit_reps(chosen_mask, &just_cells);
                r
            } else {
                orbit_reps_custom(chosen_mask, &just_cells)
            };
            let rep_set: HashSet<u8> = reps.into_iter().collect();
            let mut filtered = Vec::new();
            for &(idx, v) in &eligible {
                if rep_set.contains(&v) {
                    filtered.push((idx, v));
                } else {
                    stats.orbit_prunes += 1;
                }
            }
            filtered
        } else {
            eligible
        };

        let n_cands = to_try.len();
        let step = if n_cands > 0 {
            (frac_hi - frac_lo) / n_cands as f64
        } else {
            0.0
        };

        for (child_idx, &(idx, v)) in to_try.iter().enumerate() {
            // K4-free check: does v's neighborhood in chosen contain a triangle?
            let neighbor_in_chosen = NEIGHBORS_MASK[v as usize] & chosen_mask;
            if neighbor_in_chosen != 0 && has_triangle_in_mask(neighbor_in_chosen) {
                stats.k4_prunes += 1;
                continue;
            }

            // Add v
            let new_mask = chosen_mask | (1u128 << v);
            let nic = neighbor_in_chosen;
            let mut updated: Vec<u8> = Vec::new();
            for u in iter_bits(nic) {
                degrees[u as usize] += 1;
                updated.push(u);
            }
            degrees[v as usize] = popcount(nic) as u8;
            updated.push(v);

            // Extend candidate list with new neighbors of v
            let old_cand_len = candidate_list.len();
            let new_neighbors = NEIGHBORS_MASK[v as usize] & !new_mask & !candidate_mask;
            let mut child_cand_mask = candidate_mask;
            for u in iter_bits(new_neighbors) {
                candidate_list.push(u);
                child_cand_mask |= 1u128 << u;
            }

            let child_lo = frac_lo + child_idx as f64 * step;
            let child_hi = child_lo + step;
            *progress_frac = child_lo;

            chosen_vertices.push(v);
            let stop = backtrack(
                chosen_vertices,
                new_mask,
                candidate_list,
                child_cand_mask,
                idx + 1,
                target_size,
                degrees,
                stats,
                results,
                seen_leaves,
                symmetry_prune,
                use_nauty_orbits,
                limit,
                max_nodes,
                max_seconds,
                progress_seconds,
                start,
                last_progress,
                progress_frac,
                child_lo,
                child_hi,
                output_file,
            );
            chosen_vertices.pop();
            candidate_list.truncate(old_cand_len);

            // Undo degree updates
            for &u in &updated {
                if u == v {
                    degrees[u as usize] = 0;
                } else {
                    degrees[u as usize] -= 1;
                }
            }

            if stop {
                return true;
            }
        }

        false
    }

    let num_roots = config.roots.len();
    for (root_idx, &root) in config.roots.iter().enumerate() {
        let root_lo = root_idx as f64 / num_roots.max(1) as f64;
        let root_hi = (root_idx + 1) as f64 / num_roots.max(1) as f64;

        let mut chosen_vertices = vec![root];
        let chosen_mask = 1u128 << root;
        degrees[root as usize] = 0;

        let mut candidate_list: Vec<u8> = iter_bits(NEIGHBORS_MASK[root as usize]).collect();
        let candidate_mask = NEIGHBORS_MASK[root as usize];

        let stop = backtrack(
            &mut chosen_vertices,
            chosen_mask,
            &mut candidate_list,
            candidate_mask,
            0,
            target_size,
            &mut degrees,
            &mut stats,
            &mut results,
            &mut seen_leaves,
            config.symmetry_prune,
            config.use_nauty_orbits,
            config.limit,
            config.max_nodes,
            config.max_seconds,
            config.progress_seconds,
            start,
            &mut last_progress,
            &mut progress_frac,
            root_lo,
            root_hi,
            &output_file,
        );

        progress_frac = root_hi;
        stats.progress_pct = progress_frac * 100.0;

        if stop || stats.stopped {
            break;
        }
    }

    stats.elapsed_seconds = start.elapsed().as_secs_f64();
    (results, stats)
}
