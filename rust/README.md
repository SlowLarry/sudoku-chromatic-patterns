# chromatic-search

Exhaustive search for **minimal 4-chromatic K₄-free patterns** in the 9×9 sudoku graph.

## What this does

A standard sudoku puzzle is played on a 9×9 grid of 81 cells. Two cells "see" each other if they share the same row, column, or 3×3 box. This visibility relation defines a graph — the **sudoku graph** — with 81 vertices and 810 edges, where each cell has exactly 20 neighbors.

A *pattern* is any subset of cells from this grid. Given a pattern, we can ask: what is the **chromatic number** of its induced subgraph? That is, what is the minimum number of colors needed to color the chosen cells so that no two cells that see each other get the same color?

In a standard sudoku puzzle, digits 1–9 act as colors, and the rules demand a proper coloring, so any valid sudoku is a 9-coloring. But smaller patterns carved out of the grid may need fewer colors. This tool searches for patterns that:

1. **Need exactly 4 colors** (not 3-colorable, but 4-colorable since they contain no clique of size 4)
2. **Contain no K₄** — no four cells that all mutually see each other
3. **Are vertex-critical** — removing *any* single cell from the pattern makes the remainder 3-colorable

Such a pattern is called a **minimal 4-chromatic K₄-free pattern**. These are the smallest building blocks of 4-coloring obstruction in the sudoku graph.

## Why this matters

These patterns characterize all the *structurally irreducible* reasons that a subset of cells in a sudoku grid might require 4 colors instead of 3. Understanding them is a step toward understanding the coloring structure of the sudoku graph — a well-studied object in combinatorics and recreational mathematics.

The search is exhaustive: for a given pattern size N, the tool finds **all** such patterns up to the natural symmetries of the sudoku grid.

## Results so far

| Size N | Patterns found | Time | Search nodes | Rate |
|--------|---------------|------|-------------|------|
| 10 | 32 | 9s | 9.3M | 1.0M/s |
| 11 | 0 | — | — | — |
| 12 | 60 | ~55 min | 290M | 88k/s |
| 13 | 832 | ~4.1h | 1.38B | 93k/s |

N=10 time is with custom orbit computation. N=12 and N=13 were run with
nauty orbits (before the custom module existed); re-running them with
custom orbits would be substantially faster.

No minimal 4-chromatic K₄-free patterns of size 11 exist.

## Building

The tool must be built under Linux or WSL (the nauty C library requires POSIX headers).

**Prerequisites:**
- Rust 1.82+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- GCC (`apt install build-essential`)
- libclang for bindgen (`apt install libclang-dev`)

```bash
cd rust
cargo build --release
```

The [nauty-Traces-sys](https://crates.io/crates/nauty-Traces-sys) crate vendors and compiles the nauty C sources automatically — no separate nauty installation is required.

**Note:** Building on native Windows (MSVC) is not supported because nauty's C sources include POSIX-only headers (`unistd.h`). Use WSL.

## Usage

### Search for patterns of a given size

```bash
cargo run --release -- search --size 10 --progress-seconds 30 --output results.txt
```

This finds all minimal 4-chromatic K₄-free patterns of size 10, writing each one as an 81-character bitstring to `results.txt` (one pattern per line). Progress is printed to stderr every 30 seconds.

Output bitstrings use positional encoding: the i-th character is `1` if cell i is in the pattern, `0` otherwise. Cells are numbered left-to-right, top-to-bottom (cell 0 = top-left, cell 80 = bottom-right).

### Validate a results file

```bash
cargo run --release -- validate results.txt
```

Independently checks each pattern in the file against all five criteria (connected, K₄-free, minimum degree ≥ 3, not 3-colorable, deletion-critical).

### All search options

```
chromatic-search search
  --size <N>              Target pattern size (required)
  --output <FILE>         Append found patterns to this file
  --progress-seconds <S>  Print progress to stderr every S seconds
  --limit <N>             Stop after finding N solutions
  --max-nodes <N>         Stop after N search nodes
  --max-seconds <S>       Stop after S seconds
  --roots <SPEC>          Starting cells: "0" (default), "0-8", "0,1,2"
  --no-symmetry           Disable all symmetry pruning (much slower)
  --nauty-orbits          Use nauty for orbit computation (exact but ~12x slower)
  --compact               Print bitstrings only (no cell lists)
  --skip-file <FILE>      Skip patterns matching known bitstrings in FILE
```

By default, orbit pruning uses a custom permutation-pool algorithm
(see below). Pass `--nauty-orbits` to use nauty instead — this gives
slightly tighter orbits but is much slower per node.

## How the algorithm works

### Overview

The search operates directly on the fixed sudoku graph through recursive backtracking, growing candidate patterns one cell at a time and checking all required properties when the target size is reached.

### The sudoku graph as a data structure

Each of the 81 cells is identified by an integer 0–80 (cell = 9 × row + column). The neighborhood of each cell — the 20 other cells it sees — is stored as a 128-bit bitmask. All set operations (intersection, union, membership) become single CPU instructions on these bitmasks.

### Search strategy: grow connected subsets

The search starts from a root cell and only ever adds cells that are adjacent to at least one already-chosen cell. This guarantees every candidate pattern is connected (a necessary property of any 4-critical graph) without an explicit connectivity check.

At each step, the algorithm picks an un-chosen cell from the *frontier* (cells adjacent to the current pattern but not yet selected) and explores two branches: include the cell, or skip it.

### Pruning: cutting the search tree

Most of the search tree is eliminated by four pruning rules applied at every node, long before reaching the target size:

**1. K₄-freeness (clique pruning)**

When adding a cell v to the current pattern, a K₄ can only appear if v and three of its already-chosen neighbors form a 4-clique. Equivalently, a K₄ appears if and only if the already-chosen neighbors of v contain a *triangle*.

This is checked incrementally: compute `neighbors(v) ∩ chosen` using bitmasks, then scan that small set for triangles. If a triangle exists, the branch is pruned immediately.

**2. Degree feasibility**

Every vertex in a 4-critical graph has degree at least 3 within the pattern. If a currently-chosen cell has degree d < 3, it needs at least (3 − d) more neighbors from among the remaining candidates. If there aren't enough available neighbors, the branch is hopeless and is pruned.

**3. Orbit pruning (symmetry of the partial pattern)**

This is the most powerful pruning technique and the key to making exhaustive search feasible. The idea: if two candidate cells can be swapped by a symmetry that fixes the current partial pattern, they are equivalent — exploring both would produce duplicate results. So we only try one representative from each equivalence class.

For example, if the current pattern has a mirror symmetry and two candidate cells are mirror images of each other, only one is explored.

There are two implementations (selectable via `--nauty-orbits`):

- **Custom (default):** A pool of ~161 precomputed permutations from the sudoku symmetry group is filtered at each node for those that stabilize the current pattern. Candidate orbits are computed via union-find over the filtered permutations. This is very fast (~1 μs per node) and produces nearly identical orbits to the exact method.

- **Nauty (exact):** Calls the nauty C library at each node to compute the full automorphism group of a 108-vertex auxiliary graph. This gives exact orbits but costs ~12 μs per node due to FFI overhead and graph construction.

**4. Leaf deduplication**

When a complete pattern passes all validity checks, it is canonicalized (also via nauty) and compared against previously seen canonical forms. This catches any remaining duplicates that orbit pruning at intermediate depths didn't eliminate.

### Symmetry handling in detail

The sudoku grid has a rich symmetry group of 3,359,232 transformations that preserve the visibility relation. These include:

- Permuting the three rows within each "band" (top/middle/bottom group of 3 rows)
- Permuting the three columns within each "stack" (left/middle/right group of 3 columns)
- Permuting the three bands themselves
- Permuting the three stacks themselves
- Transposing the grid (swapping rows and columns)

Two patterns that are related by any combination of these symmetries are considered equivalent.

#### Custom orbit computation (default)

At startup, the tool builds a pool of ~161 group elements by:
1. Constructing the 17 natural generators (6 row swaps, 6 column swaps, 2 band swaps, 2 stack swaps, 1 transpose)
2. Computing Schreier generators for the stabilizer of cell 0 (the search root)
3. Expanding with pairwise products

Each element is stored as a simple permutation array `[u8; 81]`. At each search node, the pool is filtered for elements that map the chosen set to itself (checked by iterating the set bits of the chosen mask — typically only 2–14 cells). Candidate orbits are then computed via union-find over the filtered permutations.

This avoids all FFI overhead and graph construction. The trade-off is that the pool captures a subset of the full stabilizer, so orbits may occasionally be finer than the true orbits — meaning slightly more nodes are explored. In practice, the orbit counts match nauty almost exactly.

#### Nauty-based canonicalization (leaves only, or full with --nauty-orbits)

For **leaf deduplication**, we always use nauty to compute a canonical form. This is done via a 108-vertex auxiliary graph:

- **108 vertices**: the 81 cell vertices, plus 9 "row" nodes, 9 "column" nodes, and 9 "box" nodes
- **Edges**: each cell connects to its row node, its column node, and its box node
- **Vertex coloring (4 classes)**:
  - Row and column nodes share one color (this allows row/column transposition)
  - Box nodes get a second color
  - Selected cells get a third color
  - Unselected cells get a fourth color

The symmetries of this colored graph are exactly the sudoku symmetries that map selected cells to selected cells. nauty computes a canonical labeling, and two patterns are equivalent if and only if their canonical forms match.

With `--nauty-orbits`, nauty is also used at intermediate nodes for orbit computation (exact but slower).

### Coloring test

At the target size, we need to verify that the pattern is not 3-colorable but becomes 3-colorable after deleting any single cell.

The 3-colorability test uses **DSATUR backtracking**: a standard exact algorithm that always colors next the uncolored vertex with the most distinct colors on its neighbors (highest "saturation"). This heuristic finds failures quickly. For patterns of size 10–14, these checks are effectively instantaneous.

### Module structure

```
src/
  main.rs          CLI entry point (clap-based)
  search.rs        Recursive backtracking search engine
  symmetry.rs      Custom orbit computation via precomputed permutation pool
  canonical.rs     nauty FFI: canonicalization and (optional) orbit computation
  coloring.rs      DSATUR 3-colorability solver
  validation.rs    Full pattern validation (connected, K₄-free, critical)
  sudoku_graph.rs  81-vertex graph: neighbor bitmasks, induced subgraph
  bitset.rs        u128 bit manipulation utilities
```

## Output format

Each pattern is written as an 81-character string of `0`s and `1`s:

```
110100000100100000000000000100100000000000000000000000010011000000100000000000000
```

Position i=0 is cell (row 0, col 0), position i=80 is cell (row 8, col 8). `1` means the cell is in the pattern.

To decode to row/column coordinates:
```
cell i → row = i / 9, col = i % 9, box = (row / 3) * 3 + col / 3
```

## Interpreting the progress output

During a search, progress lines look like:

```
progress: 10.25% nodes=108523520 leaves=87146786 solutions=826 k4=9283419 deg=0 orb=62507017 sym=2814 elapsed=1200.5s rate=90401.1/s eta=2.9h
```

| Field | Meaning |
|-------|---------|
| progress | Estimated fraction of the search tree explored |
| nodes | Total recursive calls (search tree nodes visited) |
| leaves | Nodes that reached the target size |
| solutions | Valid minimal 4-chromatic patterns found |
| k4 | Branches pruned by K₄ detection |
| deg | Branches pruned by degree infeasibility |
| orb | Candidate cells skipped by orbit pruning |
| sym | Complete patterns skipped as symmetry duplicates |
| rate | Search nodes per second |
| eta | Estimated time remaining |

## License

Research code. No license specified.
