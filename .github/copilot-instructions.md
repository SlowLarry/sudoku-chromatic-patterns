# Sudoku 4-Chromatic Pattern Search — Project Guide

## Project overview

This project performs an exhaustive search for **minimal 4-chromatic patterns** in the 9×9 sudoku graph, generates human-readable proofs of non-3-colorability, and presents them in an interactive web viewer.

### Current state (completed)

- **Pattern search**: exhaustive search completed for N=10 through N=16
- **Proof engine**: Rust-based, generates proofs using 15 proof techniques (including Guardian, PermutationFixpoint)
- **Web viewer**: interactive viewer with proof step highlighting, deployed via GitHub Pages
- **Pattern counts**: N=10 (32), N=12 (60), N=13 (832), N=14 (620), N=15 (4,507), N=16 (19,750) = 25,801 total

---

## Terminology

- **Sudoku graph**: the fixed 81-vertex graph on cells `(r, c)` with `r, c ∈ 0..8`, adjacent iff they share a row, column, or box
- **Pattern**: a subset of cells of the 9×9 grid
- **Induced graph of a pattern**: the subgraph induced by those cells in the sudoku graph
- **Minimal 4-chromatic pattern**: a pattern whose induced graph is 4-chromatic, K₄-free, and vertex-critical (deleting any vertex makes it 3-colorable)
- **4-critical**: equivalent graph-theoretic phrasing — criticality is with respect to vertex deletion in the induced subgraph, not edge-criticality

---

## Repository structure

### Core components

| Component | Language | Location | Purpose |
|-----------|----------|----------|---------|
| Pattern search | Python | `src/sudoku_graph_searches/` | Exhaustive enumeration of minimal 4-chromatic patterns |
| Proof engine | Rust | `rust/src/proof.rs` (~3800 lines) | Generate human-readable non-3-colorability proofs |
| Export pipeline | Python | `export_json.py` | Parse proof text → JSON for web viewer |
| Web viewer | JS/CSS/HTML | `web/app.js`, `web/index.html`, `web/styles.css` | Interactive proof visualization |
| Pattern data | JSON | `web/data/patterns.json` (~43 MB) | All patterns with proofs for the web viewer |

### Key files

- `rust/src/main.rs` — CLI: `prove`, `te-depth` commands
- `rust/src/proof.rs` — `ProofGraph`, `ProofNode` enum, `find_best_proof`, `find_greedy_proof`, `prove_pattern`
- `export_json.py` — Parses `proofs_nN.txt` → structured JSON
- `web/app.js` — Rendering, highlighting, click handlers, proof step visualization
- `results_nN_minlex_ordered.txt` — Canonical pattern bitstrings (search input for prover)
- `proofs_nN.txt` — Generated proof text files

---

## Proof engine architecture

### ProofNode enum (15 proof techniques)

**Terminal contradictions** (size 1):
- `K4Contradiction` — 4-clique found
- `OddWheel` — hub forces odd rim to be 2-colored
- `BridgedHexagon` — C₆ with 3 opposite bridges
- `ParityTransport` — trivalue oddagon with odd parity
- `ParityChain` — 4+ rows/cols with parallel links forcing contradiction
- `HouseColoringContradiction` — no valid 3-coloring of house system
- `PigeonholeXwing` — induced C₄ with pigeonhole clash (includes trace)

**Non-terminal deductions** (recursive):
- `DiamondMerge` — 4-cycle merge + continue
- `Guardian` — bivalue oddagon: source vertex forced to same color as guardian via odd cycle, merging them + continue
- `CircularLadder` — 3-prism satellites forced distinct + continue
- `SetEquivalence` — multiset equation deduction (merge, virtual edges, or contradiction); requires ≥2 houses per side (1-house SET = diamond)
- `ParityTransportDeduction` — house constraint forces merge/edge + continue
- `PermutationFixpoint` — 4 full houses with pairwise column collisions → parity argument → transposition fixpoint merge (see below)
- `Branch` — Hajós-style case analysis (same_color / diff_color)
- `Failed` — proof search exhausted

### Two proof strategies

1. **Optimal proof** (`find_best_proof`): tries all techniques, minimizes total proof size via iterative deepening on branch count. Checks terminals first (K₄, odd wheel, parity, bridged hex, X-wing), then tries all diamonds/ladders/SET/branches recursively.

2. **Greedy proof** (`find_greedy_proof`): strict priority order for difficulty classification. Priority: K₄ → trivalue oddagon → parity chain → Diamond → Guardian → SET → parity chain deduction → PermutationFixpoint → Circular Ladder → Odd Wheel → Branch. First applicable technique is always taken.

`prove_pattern(cells)` runs both and returns `ProofResult` with the optimal proof tree + greedy difficulty metrics.

### PermutationFixpoint technique

Detects 4 full houses (rows or columns, each with exactly 3 pattern cells) where all 6 pairs of houses share a column (or row). This forces 4 distinct permutations of 3 colors, which split into exactly two parity classes (even/odd). Parallel-linked pairs must be same parity; with only 3 permutations per class, pigeonhole forces the two pairs into opposite classes. A cross-pair sharing 2 positions then has a transposition as its relative permutation, yielding a fixpoint (two cells forced to the same color → merge).

Proof text follows the explicit logical chain:
1. List all 4 houses and their cells
2. Show parallel links (which pairs are same-parity)
3. "All 6 pairs share a column → 4 distinct permutations."
4. "{pair1} same parity; {pair2} same parity."
5. "Only 3 per parity class → opposite classes."
6. "row X and row Y share 2 columns: [explicit cells]."
7. "Opposite parity → relative permutation is a transposition."
8. "Fixpoint: color(A) = color(B). Identify."

Uses `TECH_PARITY` flag, counted under `parity_transport_count`.

### Multi-proof support

Generates multiple proof variations per pattern using different technique sets, so the web viewer can show alternative proofs.

Approach:
1. Compute optimal proof (no techniques disabled)
2. Compute technique signature (bitmask of technique types used)
3. For each disableable technique T in the optimal proof, run `find_best_proof` with T disabled
4. Store greedy proof tree if it uses different techniques
5. Deduplicate by technique signature; keep alternatives within reasonable length of optimal
6. Output all distinct proofs per pattern

Proofs are "different enough" iff they have a different technique signature (different *set* of technique types, not just different instances of the same techniques).

---

## Difficulty classification

The web viewer uses greedy proof metrics for difficulty:
```
branch > hexagon > xwing > parity > SET > ladder > guardian > oddagon > diamond
```
A pattern is classified by the "hardest" technique needed in its greedy proof.

---

## Development workflow

### During development

- **Only re-run proofs on N=10 through N=14** (6,551 patterns total, fast)
- **Do NOT re-run N=15 (4,507) or N=16 (19,750)** unless explicitly requested — these are large and time-consuming
- Test changes on small sizes first; N=10 is the primary test case
- The export pipeline and web viewer must remain backward-compatible with old proof formats (N=16 may have older format without X-wing traces)

### Build and test cycle

```bash
# Build Rust prover (in WSL)
cd rust && cargo build --release

# Run proofs for small sizes only
./rust/target/release/chromatic-search prove --input results_n10_minlex_ordered.txt --output proofs_n10.txt
./rust/target/release/chromatic-search prove --input results_n12_minlex_ordered.txt --output proofs_n12.txt
# ... up to n14

# Export to JSON
python export_json.py

# Serve web viewer
cd web && python -m http.server 8080
```

### Git workflow

- Repository: `SlowLarry/sudoku-chromatic-patterns` on GitHub
- Branch: `main`
- User: SlowLarry / maesterhareth@gmail.com
- Large files (`patterns.json`, proof files) are tracked in git but diffs are large — be mindful when staging

---

## Sudoku graph specification

- 81 vertices: cell `v = 9*r + c` for `r, c ∈ 0..8`
- Adjacency: same row OR same column OR same 3×3 box
- Each cell has degree 20
- Representations: neighbor bitmasks (`u128` in Rust, `int` in Python)

---

## Export pipeline (`export_json.py`)

Parses `proofs_nN.txt` files → `web/data/patterns.json`.

### JSON schema per pattern
```
{
  id, size, bitstring, cell_indices, cells, rows_used,
  edges, num_edges, degree_sequence, min_degree, max_degree,
  proof: {
    depth, proof_length, complete,
    diamonds, odd_wheels, circular_ladders, bridged_hexagons,
    set_equivalences, parity_transports, pigeonhole_xwings, guardians, branches,
    greedy_branches, greedy_odd_wheels, ..., greedy_guardians,
    tree: [ { type, step, ... }, ... ]
  },
  alt_proofs: [
    { label, tree, depth, proof_length, technique_sig, ... },
    ...
  ]
}
```

---

## Web viewer (`web/app.js`)

### Key functions
- `renderProof(p)` — renders proof tree as nested step divs with `data-step` JSON
- `highlightProofStep(pattern, el)` — highlights cells on grid for clicked step
- `highlightXwingSubstep(pattern, parentStepEl, stepData, caseNum, tidx)` — X-wing trace substep highlighting
- `computeAccumulatedColors(pattern, tree, upToStep)` — tracks diamond merges and virtual edges through proof
- `getDifficulty(p)` — classifies pattern difficulty from greedy metrics
- `renderGrid(pattern, highlights)` — draws 9×9 grid with cell highlights

### Accumulated state through proof
Diamond merges are tracked via `UnionFind`; virtual edges from SET/ladder/parity accumulated. Each step shows the graph state *after* all prior deductions.

---

## Proof file format (`proofs_nN.txt`)

```
pattern {i}/{total}: PROVED cells={n} depth={d} diamonds={d} ... greedy_pigeonhole_xwings={gxw}
{81-char bitstring}
Proof of non-3-colorability:
  Assume for contradiction it is 3-colorable.

  1.  {technique step}
  2.  {technique step}
  ...

  Therefore the pattern is not 3-colorable. □

```
Each pattern block separated by blank lines. The export parser is sensitive to this format.

---

## Key implementation details

### Bitset representations
- Rust: `ProofGraph` uses `u32` bitmask adjacency (≤30 vertices per pattern)
- Python search: `int` bitmasks for neighbor masks on 81 cells

### Symmetry canonicalization
Uses a colored auxiliary graph with 81 cell + 9 row + 9 col + 9 box vertices, canonicalized via `pynauty`. Two patterns are equivalent iff their colored structures are isomorphic.

### 3-colorability
Custom DSATUR-style backtracking solver for small graphs (≤16 vertices).

---

## Terminal / WSL command guidelines

This project builds Rust code in WSL from a Windows PowerShell terminal. Several pitfalls cause garbled output and cascading failures:

### Use generous timeouts (or `timeout: 0`)

The tool's `timeout` parameter on foreground terminals does NOT kill the command on expiry — it dumps the **entire accumulated terminal buffer** (all prior commands), producing 16KB+ of stale, garbled output. This is the #1 cause of apparent "terminal failures".

- **`cargo build --release`**: use `timeout: 0` or at least 120000 (2 min)
- **Proof runs (N=10–14)**: use `timeout: 0` or at least 300000 (5 min)
- **Quick commands** (`echo`, `ls`, `wc`): 10000 is fine
- When in doubt, use `timeout: 0` — it waits for completion without risk

### PowerShell ↔ WSL quoting rules

Use **double quotes** around the `bash -c` argument. Escape bash `$` with backtick:

```powershell
# CORRECT — backtick-escape $HOME, $PWD, etc. for bash
wsl bash -c "source `$HOME/.cargo/env; cd '/mnt/c/Users/User/chromatic pattern search/rust'; cargo build --release 2>&1"

# CORRECT — PowerShell variable ${env:WSL_DIR} expands, bash $HOME is escaped
$env:WSL_DIR = "/mnt/c/Users/User/chromatic pattern search"
wsl bash -c "source `$HOME/.cargo/env; cd '${env:WSL_DIR}'; command 2>&1"

# WRONG — single quotes break with wsl bash -c
wsl bash -c 'echo $HOME'

# WRONG — unescaped $() gets swallowed by PowerShell
wsl bash -c "echo $(whoami)"
```

### Avoid terminal cascades

- Do NOT create background terminals just because a foreground command "failed" — first check if it was a timeout issue
- Do NOT retry the same command repeatedly; diagnose the root cause
- The foreground terminal is shared across calls; keep it clean by running one command at a time

### Standard WSL command pattern

```powershell
$env:WSL_DIR = "/mnt/c/Users/User/chromatic pattern search"
wsl bash -c "source `$HOME/.cargo/env; cd '${env:WSL_DIR}/rust'; cargo build --release 2>&1"
```

For running the prover:
```powershell
$env:WSL_DIR = "/mnt/c/Users/User/chromatic pattern search"
wsl bash -c "source `$HOME/.cargo/env; cd '${env:WSL_DIR}'; ./rust/target/release/chromatic-search prove --input results_n10_minlex_ordered.txt --output proofs_n10.txt 2>&1"
```

---

## Coding guidelines

- Keep changes focused. Don't refactor unrelated code.
- Test on N=10 first — it has 32 patterns and is fast.
- The proof text format is the interface between Rust and Python. Changes to it must be coordinated between `format_node` in `proof.rs` and parsing in `export_json.py`.
- X-wing trace lines have a specific format parsed by both `export_json.py` and `web/app.js` (`parseXwingTraceLine`). Changes must be synchronized.
- The web viewer is vanilla JS with no build step or framework.
- CSS uses dark theme colors (`#0d1117` background, `#c9d1d9` text, `#1f6feb` accent).

### Proof explicitness requirements

Proofs must be **fully explicit** — every logical step the reader needs to verify must be spelled out in the proof text. No "black box" techniques that just state a conclusion without showing the reasoning.

- Each proof step must trace its logical chain so a human reader can follow and verify it
- Parity-based arguments must state: which structures collide, why permutations are distinct, which pairs share parity, why pigeonhole forces opposite classes, and what the resulting fixpoint/contradiction is
- X-wing traces must show each case with the full chain of forced colorings
- Avoid techniques that merely restate the problem (e.g., "this house system has no valid 3-coloring" without showing why)
- When adding or modifying a technique, the proof text quality matters as much as correctness — the output is the product