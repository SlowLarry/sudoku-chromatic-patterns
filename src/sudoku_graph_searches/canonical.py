"""Canonicalization for Sudoku patterns using a colored auxiliary graph."""

from __future__ import annotations

from typing import Iterable, List, Tuple, Union

from .sudoku_graph import box_of, col_of, row_of
from .utils_bitset import iter_bits

_AUX_VERTEX_COUNT = 108
_CELL_OFFSET = 0
_ROW_OFFSET = 81
_COL_OFFSET = 90
_BOX_OFFSET = 99

_ROW_NODES = set(range(_ROW_OFFSET, _ROW_OFFSET + 9))
_COL_NODES = set(range(_COL_OFFSET, _COL_OFFSET + 9))
_BOX_NODES = set(range(_BOX_OFFSET, _BOX_OFFSET + 9))
_ALL_CELLS = frozenset(range(81))


def _build_aux_adjacency() -> List[set[int]]:
    adjacency: List[set[int]] = [set() for _ in range(_AUX_VERTEX_COUNT)]
    for cell in range(81):
        row_node = _ROW_OFFSET + row_of[cell]
        col_node = _COL_OFFSET + col_of[cell]
        box_node = _BOX_OFFSET + box_of[cell]
        adjacency[cell].add(row_node)
        adjacency[cell].add(col_node)
        adjacency[cell].add(box_node)
        adjacency[row_node].add(cell)
        adjacency[col_node].add(cell)
        adjacency[box_node].add(cell)
    return adjacency


_AUX_ADJACENCY = _build_aux_adjacency()
_ADJACENCY_DICT = {i: _AUX_ADJACENCY[i] for i in range(_AUX_VERTEX_COUNT)}

_pynauty_module = None


def _load_pynauty():
    global _pynauty_module
    if _pynauty_module is not None:
        return _pynauty_module
    try:
        import pynauty  # type: ignore

        _pynauty_module = pynauty
        return pynauty
    except ImportError as exc:
        raise ImportError(
            "pynauty is required for symmetry canonicalization. Install it with 'pip install pynauty'."
        ) from exc


def _build_colored_graph(chosen_mask: int):
    """Build pynauty Graph for the auxiliary structure with chosen cells colored.

    Row and column nodes share a color class so the transpose symmetry
    (swapping rows and columns) is included in the automorphism group.
    This gives the full 3,359,232-element sudoku symmetry group.
    """
    selected_cells = set(iter_bits(chosen_mask))
    unselected_cells = _ALL_CELLS - selected_cells

    coloring = [
        _ROW_NODES | _COL_NODES,  # merged: captures transpose
        _BOX_NODES,
        selected_cells,
        unselected_cells,
    ]

    pynauty = _load_pynauty()
    return pynauty.Graph(
        number_of_vertices=_AUX_VERTEX_COUNT,
        directed=False,
        adjacency_dict=_ADJACENCY_DICT,
        vertex_coloring=coloring,
    )


def canonical_signature(vertices: Union[Iterable[int], int]) -> bytes:
    """Return a canonical certificate for a pattern using sudoku symmetries.

    Uses pynauty.certificate() which gives the canonical adjacency matrix —
    two patterns are equivalent iff their certificates are identical.
    """
    if isinstance(vertices, int):
        chosen_mask = vertices
    else:
        chosen_mask = 0
        for v in vertices:
            chosen_mask |= 1 << v

    pynauty = _load_pynauty()
    graph = _build_colored_graph(chosen_mask)
    return pynauty.certificate(graph)


def candidate_orbit_reps(chosen_mask: int, candidate_cells: list[int]) -> tuple[list[int], bool]:
    """Return (reps, trivial) for candidate cells.

    *reps* contains one representative per orbit of candidates under the
    automorphism group of the colored auxiliary graph.  *trivial* is True
    when that group has order 1 (the identity only), meaning every vertex
    is its own orbit.  Once the stabiliser is trivial, all deeper nodes
    are also trivial, so callers can skip future autgrp calls.
    """
    if not candidate_cells:
        return [], True

    pynauty = _load_pynauty()
    graph = _build_colored_graph(chosen_mask)
    _, grpsize1, grpsize2, orbits, _ = pynauty.autgrp(graph)

    trivial = grpsize1 == 1.0 and grpsize2 == 0

    seen_orbit_ids: set[int] = set()
    reps: list[int] = []
    for v in candidate_cells:
        oid = orbits[v]
        if oid not in seen_orbit_ids:
            seen_orbit_ids.add(oid)
            reps.append(v)
    return reps, trivial



