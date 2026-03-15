# Sudoku Graph Searches

A small Python tool to parse Sudoku puzzles, build the constraint graph, and report graph metrics for benchmarking and visualization.

## Quick start

- Run with a puzzle string:

```
python -m sudoku_graph_searches --puzzle 530070000600195000098000060800060003400803001700020006060000280000419005000080079
```

- Run with a file:

```
python -m sudoku_graph_searches --file puzzles.txt
```

- Run a search (requires pynauty):

```
python -m sudoku_graph_searches search --size 10 --limit 5
```

Each line in the file should be a single 81-character puzzle using digits with 0 or . for blanks.

## Output

The tool prints:
- Node and edge counts for the Sudoku constraint graph
- Degree statistics and a small histogram
- Per-puzzle metadata (filled cells, blanks, invalid characters)

## Sudoku graph utilities

This package now includes core helpers for the Sudoku graph used by the 4-chromatic pattern search:

- `cell_to_rcb`, `neighbors_of`, and `is_adjacent`
- bitmask-based induced subgraphs
- exact 3-colorability solver for small graphs
- validator for minimal 4-chromatic patterns
- symmetry canonicalization via a colored auxiliary graph

Example usage in Python:

```python
from sudoku_graph_searches.validation import is_valid_minimal_4chromatic_pattern

pattern = [0, 1, 2, 9, 10, 11, 18, 19, 20, 27]
print(is_valid_minimal_4chromatic_pattern(pattern))
```

## Symmetry canonicalization

Symmetry pruning uses `pynauty` to canonicalize patterns under the Sudoku symmetry group.
Install the dependency before running searches with symmetry enabled:

```
pip install pynauty
```

### Windows build notes

On Windows, `pynauty` builds a native extension and requires a `make`-based toolchain.
One common setup is MSYS2 with the MinGW toolchain installed and `make` available on PATH.
After installing the tools, retry `pip install pynauty` from the same virtual environment.

If you want to run without symmetry pruning, use the CLI flag:

```
python -m sudoku_graph_searches search --size 10 --no-symmetry
```
