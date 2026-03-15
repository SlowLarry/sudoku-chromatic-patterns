"""Validators for minimal 4-chromatic patterns."""

from __future__ import annotations

from typing import Iterable, List

from .coloring import is_3_colorable
from .sudoku_graph import induced_subgraph
from .utils_bitset import iter_bits, popcount


def _is_connected(adj_masks: List[int]) -> bool:
    n = len(adj_masks)
    if n == 0:
        return True
    seen = 1
    frontier = 1
    while frontier:
        next_frontier = 0
        mask = frontier
        while mask:
            lsb = mask & -mask
            i = lsb.bit_length() - 1
            next_frontier |= adj_masks[i]
            mask ^= lsb
        next_frontier &= ~seen
        seen |= next_frontier
        frontier = next_frontier
    return popcount(seen) == n


def _is_k4_free(adj_masks: List[int]) -> bool:
    n = len(adj_masks)
    for u in range(n):
        neighbors_u = adj_masks[u]
        vmask = neighbors_u & ~((1 << (u + 1)) - 1)
        while vmask:
            lsb = vmask & -vmask
            v = lsb.bit_length() - 1
            vmask ^= lsb
            common = neighbors_u & adj_masks[v]
            if popcount(common) < 2:
                continue
            common_vertices = [w for w in iter_bits(common)]
            for i in range(len(common_vertices)):
                w = common_vertices[i]
                w_neighbors = adj_masks[w]
                for j in range(i + 1, len(common_vertices)):
                    x = common_vertices[j]
                    if (w_neighbors >> x) & 1:
                        return False
    return True


def is_valid_minimal_4chromatic_pattern(vertices: Iterable[int]) -> bool:
    """Return True if the induced graph is minimal 4-chromatic and K4-free."""
    local_vertices, adj_masks = induced_subgraph(vertices)
    n = len(adj_masks)
    if n == 0:
        return False
    if not _is_connected(adj_masks):
        return False
    if not _is_k4_free(adj_masks):
        return False
    if min(popcount(mask) for mask in adj_masks) < 3:
        return False
    if is_3_colorable(adj_masks):
        return False
    for i in range(n):
        sub_vertices = [v for j, v in enumerate(local_vertices) if j != i]
        _, sub_adj = induced_subgraph(sub_vertices)
        if not is_3_colorable(sub_adj):
            return False
    return True
