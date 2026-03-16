#![allow(dead_code)]

mod bitset;
mod canonical;
mod coloring;
mod minlex;
mod proof;
mod search;
mod sudoku_graph;
mod symmetry;
mod te_depth;
mod validation;

use std::collections::HashSet;
use std::fs;

use clap::{Parser, Subcommand};

use search::{pattern_bitstring, search_minimal_4chromatic, SearchConfig};

#[derive(Parser)]
#[command(name = "chromatic-search", about = "Exhaustive search for minimal 4-chromatic patterns in the 9x9 sudoku graph")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the minimal 4-chromatic pattern search
    Search {
        /// Target pattern size (e.g. 10, 11, 12)
        #[arg(long)]
        size: usize,

        /// Maximum number of solutions to find
        #[arg(long)]
        limit: Option<u64>,

        /// Disable symmetry pruning (no nauty)
        #[arg(long)]
        no_symmetry: bool,

        /// Stop after this many search nodes
        #[arg(long)]
        max_nodes: Option<u64>,

        /// Stop after this many seconds
        #[arg(long)]
        max_seconds: Option<f64>,

        /// Print progress every N seconds
        #[arg(long)]
        progress_seconds: Option<f64>,

        /// Root cells: comma-separated or ranges (e.g. "0,1,2" or "0-8")
        #[arg(long)]
        roots: Option<String>,

        /// Print compact 0/1 bitstrings only
        #[arg(long)]
        compact: bool,

        /// Append found patterns to this file
        #[arg(long)]
        output: Option<String>,

        /// File of known 0/1 patterns to skip (one per line)
        #[arg(long)]
        skip_file: Option<String>,

        /// Use nauty for orbit computation (slower but exact orbits)
        #[arg(long)]
        nauty_orbits: bool,
    },

    /// Validate a pattern file
    Validate {
        /// Path to file of 81-char 0/1 patterns
        file: String,
    },

    /// Compute minlex canonical forms of patterns
    Minlex {
        /// Input file of 81-char 0/1 patterns
        #[arg(long)]
        input: String,

        /// Output file (stdout if omitted)
        #[arg(long)]
        output: Option<String>,

        /// Output in input order (no sort/dedup)
        #[arg(long)]
        preserve_order: bool,
    },

    /// Generate human-readable proofs of non-3-colorability
    Prove {
        /// Input file of 81-char 0/1 patterns
        #[arg(long)]
        input: String,

        /// Output file (stdout if omitted)
        #[arg(long)]
        output: Option<String>,

        /// Print only the summary line per pattern
        #[arg(long)]
        summary_only: bool,
    },

    /// Compute T&E (Trial & Error) depth for patterns
    TeDepth {
        /// Input file of 81-char 0/1 patterns
        #[arg(long)]
        input: String,

        /// Output file (one depth per line; stdout if omitted)
        #[arg(long)]
        output: Option<String>,

        /// Maximum T&E depth to check (default: 5)
        #[arg(long, default_value_t = 5)]
        max_depth: u32,
    },
}

fn parse_roots(spec: &str) -> Vec<u8> {
    let mut roots = Vec::new();
    for chunk in spec.split(',') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        if let Some((a, b)) = chunk.split_once('-') {
            let start: u8 = a.trim().parse().expect("invalid root start");
            let end: u8 = b.trim().parse().expect("invalid root end");
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            for v in lo..=hi {
                roots.push(v);
            }
        } else {
            roots.push(chunk.parse().expect("invalid root cell"));
        }
    }
    roots
}

fn bitstring_to_vertices(s: &str) -> Vec<u8> {
    s.chars()
        .enumerate()
        .filter(|&(_, c)| c == '1')
        .map(|(i, _)| i as u8)
        .collect()
}

fn load_known_signatures(path: &str) -> HashSet<Vec<nauty_Traces_sys::setword>> {
    let mut sigs = HashSet::new();
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return sigs,
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.len() == 81 && line.chars().all(|c| c == '0' || c == '1') {
            let verts = bitstring_to_vertices(line);
            if !verts.is_empty() {
                let mask = sudoku_graph::cells_to_mask(&verts);
                sigs.insert(canonical::canonical_signature(mask));
            }
        }
    }
    eprintln!("skip-file: loaded {} known canonical signatures from {}", sigs.len(), path);
    sigs
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Search {
            size,
            limit,
            no_symmetry,
            max_nodes,
            max_seconds,
            progress_seconds,
            roots,
            compact,
            output,
            skip_file,
            nauty_orbits,
        } => {
            eprintln!("chromatic-search");
            eprintln!("----------------");

            let root_list = roots.map(|s| parse_roots(&s)).unwrap_or_else(|| vec![0]);

            let known = skip_file
                .as_ref()
                .map(|p| load_known_signatures(p))
                .unwrap_or_default();

            let config = SearchConfig {
                target_size: size,
                roots: root_list,
                limit,
                symmetry_prune: !no_symmetry,
                max_nodes,
                max_seconds,
                progress_seconds,
                known_signatures: known,
                output_path: output.clone(),
                compact,
                use_nauty_orbits: nauty_orbits,
            };

            let (patterns, stats) = search_minimal_4chromatic(&config);

            if stats.stopped {
                if let Some(ref reason) = stats.stop_reason {
                    eprintln!(
                        "stopped: reason={} elapsed={:.1}s",
                        reason, stats.elapsed_seconds,
                    );
                }
            }

            eprintln!(
                "search: nodes={} k4_prunes={} degree_prunes={} orbit_prunes={} symmetry_prunes={} leaves={} solutions={}",
                stats.nodes, stats.k4_prunes, stats.degree_prunes,
                stats.orbit_prunes, stats.symmetry_prunes,
                stats.leaves, stats.solutions,
            );
            eprintln!("elapsed: {:.3}s", stats.elapsed_seconds);

            if !compact {
                for (i, pattern) in patterns.iter().enumerate() {
                    eprintln!("solution {}: {:?}", i + 1, pattern);
                    println!("{}", pattern_bitstring(pattern));
                }
            }

            if let Some(ref path) = output {
                eprintln!("output: wrote {} pattern(s) to {}", patterns.len(), path);
            }
        }

        Commands::Validate { file } => {
            let contents = fs::read_to_string(&file).expect("cannot read file");
            let mut total = 0;
            let mut valid = 0;
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.len() != 81 {
                    continue;
                }
                total += 1;
                let verts = bitstring_to_vertices(line);
                if validation::is_valid_minimal_4chromatic_pattern(&verts) {
                    valid += 1;
                    println!("VALID:   {}", line);
                } else {
                    println!("INVALID: {}", line);
                }
            }
            eprintln!("validated: {}/{} patterns valid", valid, total);
        }

        Commands::Minlex { input, output, preserve_order } => {
            let contents = fs::read_to_string(&input).expect("cannot read input file");
            let mut masks: Vec<u128> = Vec::new();
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(mask) = minlex::str81_to_mask(line) {
                    masks.push(mask);
                }
            }
            eprintln!("minlex: processing {} patterns...", masks.len());

            let mut results: Vec<String> = masks
                .iter()
                .enumerate()
                .map(|(i, &mask)| {
                    let ml = minlex::minlex_pattern(mask);
                    if (i + 1) % 100 == 0 {
                        eprintln!("  {}/{}", i + 1, masks.len());
                    }
                    minlex::mask_to_81str(ml)
                })
                .collect();

            if !preserve_order {
                results.sort();
            }
            let before = results.len();
            if !preserve_order {
                results.dedup();
            }
            let after = results.len();

            let out_str = results.join("\n") + "\n";
            if let Some(ref path) = output {
                fs::write(path, &out_str).expect("cannot write output file");
                eprintln!("minlex: wrote {} patterns to {} ({} duplicates removed)", after, path, before - after);
            } else {
                print!("{}", out_str);
                eprintln!("minlex: {} patterns ({} duplicates removed)", after, before - after);
            }
        }

        Commands::TeDepth { input, output, max_depth } => {
            let contents = fs::read_to_string(&input).expect("cannot read input file");
            let mut patterns: Vec<Vec<u8>> = Vec::new();
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.len() == 81 && line.chars().all(|c| c == '0' || c == '1') {
                    patterns.push(bitstring_to_vertices(line));
                }
            }
            eprintln!("te-depth: processing {} patterns (max_depth={})...", patterns.len(), max_depth);

            let mut results: Vec<i32> = Vec::with_capacity(patterns.len());
            let mut counts = std::collections::HashMap::new();
            let t0 = std::time::Instant::now();

            for (i, cells) in patterns.iter().enumerate() {
                let d = te_depth::compute_te_depth(cells, max_depth);
                results.push(d);
                *counts.entry(d).or_insert(0usize) += 1;

                if (i + 1) % 100 == 0 || (i + 1) == patterns.len() {
                    let elapsed = t0.elapsed().as_secs_f64();
                    let rate = (i + 1) as f64 / elapsed;
                    eprintln!("  {}/{} ({:.1}s, {:.1}/s)", i + 1, patterns.len(), elapsed, rate);
                }
            }

            let elapsed = t0.elapsed().as_secs_f64();
            eprintln!("\nDone in {:.1}s", elapsed);
            eprintln!("Distribution:");
            let mut sorted_counts: Vec<_> = counts.into_iter().collect();
            sorted_counts.sort();
            for (d, cnt) in &sorted_counts {
                if *d >= 0 {
                    eprintln!("  T&E({}): {}", d, cnt);
                } else {
                    eprintln!("  UNKNOWN: {}", cnt);
                }
            }

            let out_str: String = results.iter().map(|d| format!("{}\n", d)).collect();
            if let Some(ref path) = output {
                fs::write(path, &out_str).expect("cannot write output file");
                eprintln!("Wrote {} results to {}", results.len(), path);
            } else {
                print!("{}", out_str);
            }
        }

        Commands::Prove { input, output, summary_only } => {
            let contents = fs::read_to_string(&input).expect("cannot read input file");
            let mut patterns: Vec<(String, Vec<u8>)> = Vec::new();
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.len() == 81 && line.chars().all(|c| c == '0' || c == '1') {
                    let verts = bitstring_to_vertices(line);
                    patterns.push((line.to_string(), verts));
                }
            }
            eprintln!("prove: processing {} patterns...", patterns.len());

            let mut out = String::new();
            let mut proved = 0;
            let mut failed = 0;
            let mut max_branches = 0usize;

            for (i, (bitstr, cells)) in patterns.iter().enumerate() {
                let result = proof::prove_pattern(cells);
                let complete = result.proof.is_complete();
                let branches = result.proof.branch_count();
                max_branches = max_branches.max(branches);

                if complete { proved += 1; } else { failed += 1; }

                let summary = format!(
                    "pattern {}/{}: {} cells={} {}",
                    i + 1, patterns.len(),
                    if complete { "PROVED" } else { "FAILED" },
                    cells.len(),
                    result.summary(),
                );

                if summary_only {
                    out += &format!("{}\n", summary);
                } else {
                    out += &format!("{}\n{}\n", summary, bitstr);
                    out += &result.text;
                    out += "\n";
                }

                if (i + 1) % 100 == 0 {
                    eprintln!("  {}/{}", i + 1, patterns.len());
                }
            }

            if let Some(ref path) = output {
                fs::write(path, &out).expect("cannot write output file");
                eprintln!(
                    "prove: wrote {} proofs to {} (proved={}, failed={}, max_branches={})",
                    patterns.len(), path, proved, failed, max_branches,
                );
            } else {
                print!("{}", out);
                eprintln!(
                    "prove: {} patterns (proved={}, failed={}, max_branches={})",
                    patterns.len(), proved, failed, max_branches,
                );
            }
        }
    }
}
