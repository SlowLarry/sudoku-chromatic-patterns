"""Sudoku graph construction and helpers."""

from __future__ import annotations

from typing import Iterable, List, Set, Tuple

cells: List[int] = list(range(81))
row_of: List[int] = [0] * 81
col_of: List[int] = [0] * 81
box_of: List[int] = [0] * 81
neighbors: List[Set[int]] = [set() for _ in range(81)]
neighbors_mask: List[int] = [0] * 81


def _build_sudoku_graph() -> None:
    rows = [[] for _ in range(9)]
    cols = [[] for _ in range(9)]
    boxes = [[] for _ in range(9)]

    for index in range(81):
        row = index // 9
        col = index % 9
        box = (row // 3) * 3 + (col // 3)
        row_of[index] = row
        col_of[index] = col
        box_of[index] = box
        rows[row].append(index)
        cols[col].append(index)
        boxes[box].append(index)

    groups = rows + cols + boxes
    for group in groups:
        for i in range(len(group)):
            for j in range(i + 1, len(group)):
                a = group[i]
                b = group[j]
                neighbors[a].add(b)
                neighbors[b].add(a)

    for index in range(81):
        mask = 0
        for v in neighbors[index]:
            mask |= 1 << v
        neighbors_mask[index] = mask


_build_sudoku_graph()


def cell_to_rcb(v: int) -> Tuple[int, int, int]:
    """Return (row, col, box) for a cell index."""
    if v < 0 or v >= 81:
        raise ValueError("cell index must be in 0..80")
    return row_of[v], col_of[v], box_of[v]


def neighbors_of(v: int) -> Set[int]:
    """Return the neighbor set for a cell."""
    if v < 0 or v >= 81:
        raise ValueError("cell index must be in 0..80")
    return neighbors[v]


def is_adjacent(u: int, v: int) -> bool:
    """Return True if two cells share a row, column, or box."""
    if u < 0 or u >= 81 or v < 0 or v >= 81:
        raise ValueError("cell index must be in 0..80")
    return v in neighbors[u]


def induced_subgraph(vertices: Iterable[int]) -> Tuple[List[int], List[int]]:
    """Return (local_vertices, local_adj_masks) for an induced subgraph."""
    local_vertices: List[int] = []
    seen = set()
    for v in vertices:
        if v in seen:
            continue
        if v < 0 or v >= 81:
            raise ValueError("cell index must be in 0..80")
        seen.add(v)
        local_vertices.append(v)

    index_of = {v: i for i, v in enumerate(local_vertices)}
    local_adj_masks: List[int] = [0] * len(local_vertices)

    for i, v in enumerate(local_vertices):
        mask = 0
        for u in local_vertices:
            if u == v:
                continue
            if neighbors_mask[v] & (1 << u):
                mask |= 1 << index_of[u]
        local_adj_masks[i] = mask

    return local_vertices, local_adj_masks
