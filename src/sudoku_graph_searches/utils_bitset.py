"""Bitset utilities for small graph algorithms."""

from __future__ import annotations

from typing import Iterator


if hasattr(int, "bit_count"):
    def popcount(mask: int) -> int:
        return mask.bit_count()
else:
    def popcount(mask: int) -> int:
        return bin(mask).count("1")


def iter_bits(mask: int) -> Iterator[int]:
    """Yield indices of set bits in a non-negative integer mask."""
    while mask:
        lsb = mask & -mask
        yield lsb.bit_length() - 1
        mask ^= lsb
