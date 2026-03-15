import time
import sys
sys.path.insert(0, "src")

from sudoku_graph_searches.canonical import (
    candidate_orbit_reps, _build_colored_graph, _load_pynauty,
    _ROW_NODES, _COL_NODES, _BOX_NODES, _ADJACENCY_DICT, _AUX_VERTEX_COUNT,
)
from sudoku_graph_searches.sudoku_graph import neighbors_mask
from sudoku_graph_searches.utils_bitset import iter_bits, popcount

pynauty = _load_pynauty()

# Check group orders with different colorings (no selected cells)
all_cells = set(range(81))

# Current: 5 classes (row, col, box separate)
g5 = pynauty.Graph(
    number_of_vertices=_AUX_VERTEX_COUNT,
    directed=False,
    adjacency_dict=_ADJACENCY_DICT,
    vertex_coloring=[_ROW_NODES, _COL_NODES, _BOX_NODES, set(), all_cells],
)
_, grpsize1_5, grpsize2_5, _, _ = pynauty.autgrp(g5)
print(f"5-class (row/col/box separate): group order = {grpsize1_5} * 10^{grpsize2_5}")

# Proposed: 4 classes (row+col merged, box separate)
g4 = pynauty.Graph(
    number_of_vertices=_AUX_VERTEX_COUNT,
    directed=False,
    adjacency_dict=_ADJACENCY_DICT,
    vertex_coloring=[_ROW_NODES | _COL_NODES, _BOX_NODES, set(), all_cells],
)
_, grpsize1_4, grpsize2_4, _, _ = pynauty.autgrp(g4)
print(f"4-class (row+col merged):        group order = {grpsize1_4} * 10^{grpsize2_4}")

# 3 classes (row+col+box all same)
g3 = pynauty.Graph(
    number_of_vertices=_AUX_VERTEX_COUNT,
    directed=False,
    adjacency_dict=_ADJACENCY_DICT,
    vertex_coloring=[_ROW_NODES | _COL_NODES | _BOX_NODES, set(), all_cells],
)
_, grpsize1_3, grpsize2_3, _, _ = pynauty.autgrp(g3)
print(f"3-class (all structure merged):   group order = {grpsize1_3} * 10^{grpsize2_3}")

# Expected: without transpose = 1,679,616 = 1.679616 * 10^6
# Expected: with transpose    = 3,359,232 = 3.359232 * 10^6
print(f"\nExpected without transpose: 1679616")
print(f"Expected with transpose:    3359232")

# Now test orbits at depth 1 with 4-class coloring
mask0 = 1 << 0
selected = {0}
unselected = set(range(81)) - selected
g4_d1 = pynauty.Graph(
    number_of_vertices=_AUX_VERTEX_COUNT,
    directed=False,
    adjacency_dict=_ADJACENCY_DICT,
    vertex_coloring=[_ROW_NODES | _COL_NODES, _BOX_NODES, selected, unselected],
)
_, _, _, orbits_4, _ = pynauty.autgrp(g4_d1)

# Group neighbors of 0 by orbit
cands = list(iter_bits(neighbors_mask[0]))
orbit_groups = {}
for v in cands:
    oid = orbits_4[v]
    orbit_groups.setdefault(oid, []).append(v)
print(f"\n4-class orbits of neighbors of cell 0:")
for oid, members in sorted(orbit_groups.items()):
    print(f"  orbit rep={oid}: {members} (size {len(members)})")

# Benchmark with 4-class coloring
start = time.perf_counter()
for _ in range(1000):
    g = pynauty.Graph(
        number_of_vertices=_AUX_VERTEX_COUNT,
        directed=False,
        adjacency_dict=_ADJACENCY_DICT,
        vertex_coloring=[_ROW_NODES | _COL_NODES, _BOX_NODES, selected, unselected],
    )
    pynauty.autgrp(g)
elapsed = time.perf_counter() - start
print(f"\n4-class autgrp benchmark: {elapsed/1000*1000:.3f}ms/call")

