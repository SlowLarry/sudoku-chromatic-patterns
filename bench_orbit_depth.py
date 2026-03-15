"""Quick benchmark of max_orbit_depth settings for N=10."""

import sys
from src.sudoku_graph_searches.search import search_minimal_4chromatic
import time

depth = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1] != "None" else None
t0 = time.monotonic()
results, stats = search_minimal_4chromatic(10, max_orbit_depth=depth)
elapsed = time.monotonic() - t0
print(
    f"max_orbit_depth={depth}: {elapsed:.1f}s, {stats.nodes} nodes, "
    f"{stats.solutions} solutions, orb={stats.orbit_prunes}, "
    f"triv={stats.autgrp_skipped}, sym={stats.symmetry_prunes}, "
    f"k4={stats.k4_prunes}, leaves={stats.leaves}"
)
