"""Exact 3-colorability solver for small graphs."""

from __future__ import annotations

from typing import List

from .utils_bitset import popcount


def is_3_colorable(adj_masks: List[int]) -> bool:
    """Return True if the graph is 3-colorable."""
    n = len(adj_masks)
    if n == 0:
        return True

    colors = [-1] * n
    degrees = [popcount(mask) for mask in adj_masks]

    def choose_vertex() -> tuple[int, int]:
        best = -1
        best_sat = -1
        best_deg = -1
        best_used = 0
        for i in range(n):
            if colors[i] != -1:
                continue
            used = 0
            mask = adj_masks[i]
            while mask:
                lsb = mask & -mask
                j = lsb.bit_length() - 1
                if colors[j] != -1:
                    used |= 1 << colors[j]
                mask ^= lsb
            sat = popcount(used)
            if sat > best_sat or (sat == best_sat and degrees[i] > best_deg):
                best = i
                best_sat = sat
                best_deg = degrees[i]
                best_used = used
        return best, best_used

    def backtrack(colored: int) -> bool:
        if colored == n:
            return True
        v, used = choose_vertex()
        available = (~used) & 0b111
        if available == 0:
            return False
        for color in range(3):
            if not (available >> color) & 1:
                continue
            colors[v] = color
            if backtrack(colored + 1):
                return True
            colors[v] = -1
        return False

    return backtrack(0)
