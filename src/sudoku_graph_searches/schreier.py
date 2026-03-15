"""Schreier-vector orbit and stabiliser computation for sudoku symmetries.

Replaces per-node pynauty.autgrp() calls (~0.4 ms each) with cheap
numpy-based group operations on explicit generators.  Generators of
the full sudoku symmetry group are obtained from pynauty once at
startup; stabiliser chains are maintained incrementally via Schreier's
lemma as the search fixes successive cells.

Permutations are numpy int8 arrays of length 81 (cell indices only;
the restriction from the 108-vertex auxiliary graph to cells is
faithful since each structure node is uniquely determined by its
cell neighbours).
"""

from __future__ import annotations

import numpy as np

N_CELLS = 81
IDENTITY = np.arange(N_CELLS, dtype=np.int8)


# ── permutation helpers ──────────────────────────────────────────────


def _inverse(p: np.ndarray) -> np.ndarray:
    inv = np.empty(N_CELLS, dtype=np.int8)
    inv[p] = IDENTITY
    return inv


# ── orbit / transversal ─────────────────────────────────────────────


def _orbit_and_transversal(
    generators: list[np.ndarray],
    point: int,
) -> tuple[list[int], dict[int, np.ndarray]]:
    """BFS orbit of *point*; ``transversal[w]`` maps *point* → *w*."""
    tv: dict[int, np.ndarray] = {point: IDENTITY.copy()}
    orbit: list[int] = [point]
    front = 0
    while front < len(orbit):
        u = orbit[front]
        front += 1
        t_u = tv[u]
        for g in generators:
            v = int(g[u])
            if v not in tv:
                tv[v] = g[t_u]  # compose(g, t_u)
                orbit.append(v)
    return orbit, tv


# ── Schreier generators (stabiliser of a point) ─────────────────────


def stabilizer_gens(
    generators: list[np.ndarray],
    point: int,
) -> list[np.ndarray]:
    """Generators of Stab(*point*) via Schreier's lemma (deduplicated)."""
    if not generators:
        return []
    orbit, tv = _orbit_and_transversal(generators, point)
    if len(orbit) == 1:
        # *point* is fixed by every generator → stabiliser = whole group
        return list(generators)
    # Pre-invert transversal entries to avoid redundant inverse() calls.
    tv_inv = {u: _inverse(tv[u]) for u in orbit}
    result: list[np.ndarray] = []
    seen: set[bytes] = set()
    # Add identity hash so we can skip it cheaply.
    seen.add(IDENTITY.tobytes())
    for u in orbit:
        t_u = tv[u]
        for g in generators:
            gu = int(g[u])
            sg = tv_inv[gu][g[t_u]]  # tv[gu]^{-1} ∘ g ∘ tv[u]
            key = sg.tobytes()
            if key not in seen:
                seen.add(key)
                result.append(sg)
    return result


# ── orbit representatives (union-find) ───────────────────────────────


def orbit_reps(generators: list[np.ndarray], points: list[int]) -> list[int]:
    """First element per orbit of *points* under *generators*."""
    if not points:
        return []
    if not generators:
        return list(points)
    parent: dict[int, int] = {}
    point_set = set(points)
    for p in points:
        parent[p] = p

    def find(x: int) -> int:
        r = x
        while parent[r] != r:
            r = parent[r]
        while parent[x] != r:
            parent[x], x = r, parent[x]
        return r

    for g in generators:
        for p in points:
            gp = int(g[p])
            if gp in point_set:
                rx, ry = find(p), find(gp)
                if rx != ry:
                    parent[ry] = rx

    seen_roots: set[int] = set()
    reps: list[int] = []
    for p in points:
        r = find(p)
        if r not in seen_roots:
            seen_roots.add(r)
            reps.append(p)
    return reps


# ── precomputed generators ───────────────────────────────────────────

_stab0_generators: list[np.ndarray] | None = None


def _init_generators() -> None:
    global _stab0_generators
    if _stab0_generators is not None:
        return
    from .canonical import (
        _ADJACENCY_DICT,
        _ALL_CELLS,
        _AUX_VERTEX_COUNT,
        _BOX_NODES,
        _COL_NODES,
        _ROW_NODES,
        _load_pynauty,
    )

    pynauty = _load_pynauty()
    # Full symmetry group of the auxiliary graph with all cells in one
    # colour class (no selection).  Three non-empty classes cover all
    # 108 vertices.
    graph = pynauty.Graph(
        number_of_vertices=_AUX_VERTEX_COUNT,
        directed=False,
        adjacency_dict=_ADJACENCY_DICT,
        vertex_coloring=[
            _ROW_NODES | _COL_NODES,
            _BOX_NODES,
            set(_ALL_CELLS),
        ],
    )
    raw_gens, _, _, _, _ = pynauty.autgrp(graph)
    assert raw_gens, "sudoku graph must have non-trivial symmetry"
    full_gens = [np.array(g[:N_CELLS], dtype=np.int8) for g in raw_gens]
    _stab0_generators = stabilizer_gens(full_gens, 0)


def get_stab0_generators() -> list[np.ndarray]:
    """Return generators of Stab(cell 0) in the sudoku symmetry group."""
    _init_generators()
    assert _stab0_generators is not None
    return _stab0_generators
