"""Graph metrics for Sudoku constraint graphs."""

from __future__ import annotations

from collections import Counter
from typing import Dict, Set

GridGraph = Dict[int, Set[int]]


def graph_stats(graph: GridGraph) -> dict[str, float]:
    degrees = [len(neighbors) for neighbors in graph.values()]
    if not degrees:
        return {
            "nodes": 0,
            "edges": 0,
            "min_deg": 0,
            "max_deg": 0,
            "avg_deg": 0.0,
        }
    nodes = len(degrees)
    edges = sum(degrees) // 2
    return {
        "nodes": nodes,
        "edges": edges,
        "min_deg": min(degrees),
        "max_deg": max(degrees),
        "avg_deg": sum(degrees) / nodes,
    }


def degree_histogram(graph: GridGraph) -> dict[int, int]:
    degrees = [len(neighbors) for neighbors in graph.values()]
    counts = Counter(degrees)
    return dict(counts)
