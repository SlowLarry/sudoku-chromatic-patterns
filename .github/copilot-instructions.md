# Sudoku 4-Chromatic Pattern Search — Implementation Brief

## Goal

Implement an exhaustive search for **minimal 4-chromatic patterns** in the 9x9 sudoku graph, for pattern sizes `N = 10..14` if feasible.

A pattern is a subset of cells of the 9x9 sudoku grid. Its induced graph is taken inside the standard sudoku visibility graph:

- one vertex per cell
- two cells adjacent iff they share a row, column, or box

We seek patterns whose induced graph is:

1. **4-chromatic**
2. **K4-free** (contains no clique of size 4 or larger)
3. **minimal with respect to vertex deletion**, meaning:
   - deleting any chosen cell makes the induced subgraph 3-colorable

Important:
- We care about **vertex-critical induced subgraphs**, not edge-critical graphs.
- We are **not** enumerating abstract graphs first and then testing sudoku embeddability.
- We search **directly inside the fixed sudoku graph**.

---

## Terminology

Use the following precise terminology throughout the code and comments:

- **Sudoku graph**: the fixed 81-vertex graph on cells
- **Pattern**: a subset of cells
- **Induced graph of a pattern**: the graph induced by those cells in the sudoku graph
- **Minimal 4-chromatic pattern**: a pattern whose induced graph is:
  - 4-chromatic
  - K4-free
  - vertex-critical under deletion of any chosen vertex

Equivalent graph-theoretic phrasing:
- induced graph is **4-critical**
- but criticality here is with respect to **vertex deletion in the induced subgraph**
- not abstract edge-criticality

---

## High-level strategy

Do a **sudoku-first search**.

Do **not**:
- enumerate all subsets of size N naively
- use NetworkX/rustworkx subgraph isomorphism in the hot loop
- start from abstract 4-critical graphs and test embeddability

Reason:
- generic subgraph isomorphism was a bottleneck in previous attempts
- the target graph is always the same fixed sudoku graph
- we can exploit that structure with bitsets and direct search

Main plan:

1. Build the fixed sudoku graph once.
2. Represent neighborhoods as bitmasks.
3. Search directly for connected K4-free candidate subsets.
4. Use pruning aggressively.
5. At size N, test:
   - non-3-colorability
   - 3-colorability after deleting any one selected vertex
6. Quotient results by sudoku symmetries using canonicalization.

---

## Recommended implementation language

Start in **Python**.

Reason:
- easier to iterate and debug
- better for validating pruning and correctness
- likely sufficient for N=10..12, possibly more with careful optimization
- Rust should only be considered later if profiling shows a genuine performance bottleneck after the algorithm is stable

Do not prematurely optimize by switching languages before the search architecture is validated.

---

## External libraries

Preferred:
- `pynauty` for canonical labeling / automorphism-based symmetry reduction

Avoid in the hot loop:
- `networkx`
- `rustworkx`
- generic subgraph-isomorphism libraries

Possible limited use:
- NetworkX only for debugging or visualization outside the main search

---

## Sudoku graph specification

The sudoku graph has 81 vertices corresponding to cells `(r, c)` with `r, c in 0..8`.

Adjacency rule:
- same row OR
- same column OR
- same 3x3 box

Each cell has degree 20.

### Required representations

Create:

- `cells = list(range(81))`
- maps:
  - `row_of[v]`
  - `col_of[v]`
  - `box_of[v]`
- `neighbors[v]` as:
  - Python `set[int]` for readability/debugging
  - Python `int` bitmask for speed in the hot loop

Use a consistent cell numbering, e.g.
- `v = 9*r + c`

---

## Core search constraints

A valid final pattern `X` of size `N` must satisfy:

1. `|X| = N`
2. induced subgraph `S[X]` is connected
3. `S[X]` is K4-free
4. `S[X]` is not 3-colorable
5. for every `x in X`, `S[X \ {x}]` is 3-colorable

We only care about such patterns **up to sudoku symmetry**.

---

## Symmetry handling

This is essential.

Do **not** manually implement the full sudoku symmetry group by applying all:
- row swaps within a band
- column swaps within a stack
- band swaps
- stack swaps
- transpose / reflection / rotation

Instead, canonicalize via a **colored auxiliary graph**.

### Auxiliary graph for symmetry reduction

Construct a graph with:

- 81 cell vertices
- 9 row vertices
- 9 column vertices
- 9 box vertices

Edges:
- each cell vertex connects to its row vertex
- each cell vertex connects to its column vertex
- each cell vertex connects to its box vertex

Vertex colors / partitions:
- row vertices one color class
- column vertices one color class
- box vertices one color class
- selected cell vertices one color class
- unselected cell vertices one color class

Then use `pynauty` to compute a canonical labeling / canonical form.

Two patterns are equivalent iff these colored structures are isomorphic.

Important:
- use this both for final solutions and for partial subsets if possible
- symmetry reduction on partial subsets is likely crucial for feasibility

---

## Search architecture

Use recursive backtracking.

### State of a partial search node

Maintain at least:

- current chosen set `X`
- bitmask `X_mask`
- current size `k = len(X)`
- candidate extension frontier
- current degrees inside `X`
- maybe a cached canonical signature of `X`

### Expansion rule

Only add vertices that keep the chosen set connected.

Suggested approach:
- after the first vertex, only add vertices adjacent to the current chosen set

This does not lose anything because every final 4-critical graph is connected.

### Incremental K4-free test

When adding a vertex `v` to current set `X`, a new K4 can only occur if `v` is adjacent to a triangle already contained in `X`.

Therefore:
- compute `Nv = neighbors[v] ∩ X`
- reject the addition iff the induced graph on `Nv` contains a triangle

Implement this efficiently with bitmasks.

Do not recompute clique structure from scratch.

---

## Pruning rules

These are mandatory.

### 1. Connectedness
Only search connected patterns.

### 2. K4-freeness
Reject any branch as soon as a K4 appears.

### 3. Degree lower bound
A 4-critical graph has minimum degree at least 3.

Therefore:
- at final size N, every chosen vertex must have degree at least 3 inside the induced graph
- during search, if a chosen vertex cannot possibly still reach degree 3 given the remaining slots, prune the branch

This should be implemented as an upper-bound feasibility check.

### 4. Symmetry pruning
Canonicalize partial sets and only continue canonical representatives.

This may be the difference between feasible and infeasible.

### 5. Optional feasibility pruning
If the remaining number of available additions is insufficient to fix:
- disconnectedness issues
- degree deficiencies
then prune.

---

## 3-colorability test

We need a fast test for whether the induced graph on a small pattern is 3-colorable.

Pattern sizes are small (`N <= 14`), so implement a custom exact backtracking solver.

Recommended:
- DSATUR-style branching or similar
- maintain available colors bitmask per vertex
- choose next uncolored vertex with strongest constraint:
  - highest saturation degree
  - tie-break by ordinary degree

This solver will be used for:

1. testing whether a candidate pattern is 3-colorable
2. testing whether each one-vertex deletion is 3-colorable

### Criticality test for a full pattern X

`X` is valid iff:

- `G = S[X]` is **not** 3-colorable
- for every vertex `x in X`, the graph `G - x` **is** 3-colorable

Note:
- no separate edge-criticality testing
- only induced vertex deletion matters

---

## Suggested implementation order

Implement in this order.

### Phase 1: basic sudoku graph
- cell numbering
- row/column/box maps
- neighbor sets
- neighbor bitmasks

### Phase 2: induced subgraph utilities
- convert a cell subset to local adjacency lists / bitmasks
- compute degrees
- connectedness test

### Phase 3: exact 3-colorability solver
- small-graph backtracking
- unit tests on known graphs:
  - triangle should be 3-colorable
  - K4 should not be 3-colorable
  - odd cycle should be 3-colorable unless constraints make otherwise
  - wheel W5 should not be 3-colorable, etc.

### Phase 4: full-pattern validator
Given a subset of cells:
- test connectedness
- test K4-free
- test non-3-colorability
- test deletion-criticality

### Phase 5: symmetry canonicalization
- build the colored auxiliary structure
- integrate `pynauty`
- compute a canonical signature for a pattern
- verify known symmetric patterns collapse correctly

### Phase 6: recursive search for fixed N
- start with N=10
- implement connected growth
- incremental K4 checks
- degree-feasibility pruning
- symmetry pruning of partial states
- final validation at leaves

### Phase 7: logging and persistence
Store:
- `N`
- canonical representative cell set
- maybe canonical form string
- degree sequence of induced graph
- adjacency of induced graph
- timing / node counts / prune counts

---

## Data structure recommendations

Use bitmasks wherever possible.

### Global board-level data
- `neighbor_mask[81] : int`

### Pattern-level data
For a pattern `X`:
- `X_mask : int`
- list of vertices in `X`
- maybe local indexing `0..k-1`
- local adjacency masks for the induced subgraph

Python integers are suitable as bitsets here.

Useful helpers:
- popcount: `int.bit_count()`
- iterate set bits efficiently
- precompute masks as much as possible

Avoid repeated conversion between high-level graph objects and low-level representations.

---

## Important non-goals

Do not spend time on:
- abstract graph generation first
- generic induced-subgraph embedding into sudoku
- edge-critical graph classification
- SAT/ILP unless the simpler approach clearly fails
- Rust rewrite before profiling

---

## Minimal viable result

A good first milestone is:

- correct exhaustive search for `N = 10`
- symmetry-quotiented output
- enough diagnostics to understand runtime and prune effectiveness

Then continue to:
- `N = 11`
- `N = 12`

Only then assess feasibility of `N = 13, 14`.

---

## Validation expectations

Before trusting results, verify:

1. every reported pattern is:
   - connected
   - K4-free
   - not 3-colorable
   - deletion-critical

2. no two reported patterns are equivalent under sudoku symmetry

3. rerunning the search gives the same canonical representatives

4. small hand-constructed examples behave as expected

---

## Coding style

Keep code modular and testable.

Suggested modules/files:
- `sudoku_graph.py`
- `coloring.py`
- `canonical.py`
- `search.py`
- `utils_bitset.py`
- `main.py`

Prefer small pure functions where practical.

Add debug modes:
- verbose tracing for one branch
- counters for:
  - recursive nodes visited
  - K4 prunes
  - degree-feasibility prunes
  - symmetry prunes
  - coloring rejections
  - successful patterns found

These counters will be important for improving the search.

---

## Initial task for Copilot

Start by implementing:

1. construction of the 81-vertex sudoku graph
2. neighbor bitmasks
3. helper functions:
   - `cell_to_rcb(v)`
   - `neighbors_of(v)`
   - `is_adjacent(u, v)`
   - `induced_subgraph(vertices)`
4. a small exact 3-colorability solver for graphs on up to ~14 vertices
5. a validator:
   - `is_valid_minimal_4chromatic_pattern(vertices)`

Only after these are correct should the recursive search be written.

---

## Final note

The core idea is:

Search **directly in the fixed sudoku graph**, using:

- bitset-based induced graph logic
- exact 3-colorability checking on small graphs
- aggressive pruning
- canonicalization under sudoku symmetries

Do not frame this as a generic subgraph-isomorphism problem.
That was already identified as the likely stumbling block.