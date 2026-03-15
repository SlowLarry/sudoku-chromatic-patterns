#![allow(dead_code)]

mod bitset;
mod canonical;
mod coloring;
mod search;
mod sudoku_graph;
mod symmetry;
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
    }
}
