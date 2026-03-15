"""Sudoku constraint graph construction."""

from __future__ import annotations

from typing import Dict, Set

GridGraph = Dict[int, Set[int]]


def _row_col_box(index: int) -> tuple[int, int, int]:
    row = index // 9
    col = index % 9
    box = (row // 3) * 3 + (col // 3)
    return row, col, box


def build_constraint_graph() -> GridGraph:
    """Build a graph where nodes share row, column, or box constraints."""
    graph: GridGraph = {i: set() for i in range(81)}
    rows = [[] for _ in range(9)]
    cols = [[] for _ in range(9)]
    boxes = [[] for _ in range(9)]

    for index in range(81):
        row, col, box = _row_col_box(index)
        rows[row].append(index)
        cols[col].append(index)
        boxes[box].append(index)

    groups = rows + cols + boxes
    for group in groups:
        for i in range(len(group)):
            for j in range(i + 1, len(group)):
                a = group[i]
                b = group[j]
                graph[a].add(b)
                graph[b].add(a)

    return graph
