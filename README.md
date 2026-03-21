# Minimal 4-Chromatic Sudoku Patterns

Exhaustive search for **minimal 4-chromatic K₄-free patterns** in the 9×9 sudoku graph, with machine-verifiable proofs of non-3-colorability and an interactive web viewer.

## What this is

The standard sudoku grid defines a graph on 81 vertices where two cells are adjacent if they share a row, column, or 3×3 box. Each cell has exactly 20 neighbors.

A *pattern* is a subset of cells. This project finds patterns whose induced subgraph:

1. **Requires exactly 4 colors** — not 3-colorable, but contains no K₄ so is 4-colorable
2. **Is vertex-critical** — removing any single cell makes it 3-colorable

These are the structurally irreducible reasons a subset of sudoku cells can require 4 colors instead of 3.

## Results

The search is exhaustive: for each pattern size N, **all** such patterns are found up to the 3,359,232 geometric symmetries of the sudoku grid.

| Size N | Patterns |
|--------|----------|
| 10     | 32       |
| 11     | 0        |
| 12     | 60       |
| 13     | 832      |
| 14     | 620      |
| 15     | 4,507    |
| 16     | 19,750   |
| **Total** | **25,801** |

No patterns of size 11 exist.

## Repository structure

```
rust/                  Rust search engine (source of truth)
  src/
    main.rs            CLI entry point
    search.rs          Recursive backtracking with symmetry pruning
    proof.rs           Proof generation (diamonds, SET, oddagons, etc.)
    canonical.rs       Canonicalization under sudoku symmetries
    coloring.rs        Exact 3-colorability solver
    ...
web/                   Interactive web viewer
  index.html
  app.js
  styles.css
  data/                Generated data (gitignored, see below)
export_json.py         Converts proof text files → JSON for the web viewer
proofs_n{10..16}.txt   Machine-generated proofs of non-3-colorability
4chromatics_*_minlex.txt  Canonical pattern lists (minlex form)
```

## Building the search engine

Requires Linux or WSL. The [nauty-Traces-sys](https://crates.io/crates/nauty-Traces-sys) crate vendors the nauty C library automatically.

```bash
# Prerequisites: Rust 1.82+, GCC, libclang-dev
cd rust
cargo build --release
```

See [rust/README.md](rust/README.md) for full usage (search, validate, minlex, prove).

## Web viewer

The interactive viewer lets you browse all 25,801 patterns with their proofs, grid visualizations, and filters.

To run locally:

```bash
cd web
python -m http.server 8080
# open http://localhost:8080
```

All data is included in the repository — no build step needed.

### Regenerating the data (optional)

If you modify the Rust proof engine and want to re-export:

```bash
# 1. Generate proofs (from the Rust tool)
cd rust
cargo run --release -- prove --input ../proofs_n10.txt  # repeat for each N

# 2. Export to JSON for the web viewer
cd ..
python export_json.py
```

## Proof techniques

Every pattern has a machine-verifiable proof that it is not 3-colorable. The proof system uses:

- **Diamond** — two cells forced to the same color via a shared neighborhood structure
- **K₄ detection** — four mutually visible cells form a clique requiring 4 colors
- **Odd wheel** — a hub connected to an odd cycle forces a contradiction
- **Circular ladder** — paired rungs with satellite cells forced all-distinct
- **Bridged hexagon** — a 6-cycle with bridge edges
- **SET equivalence** — multi-cell color constraints via set equations
- **Parity transport** — deductions from house parity (trivalue oddagon / pigeonhole chain)
- **Branching** — case splits when no single-step deduction suffices

All proofs for all 25,801 patterns complete with at most 1 branch (T&E depth ≤ 3).

## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).
