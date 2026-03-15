"""Puzzle input helpers."""

from __future__ import annotations

from collections.abc import Iterable, Iterator


def _clean_line(line: str) -> str:
    return "".join(ch for ch in line.strip() if not ch.isspace())


def _count_invalid(puzzle: str) -> int:
    return sum(ch not in "0123456789." for ch in puzzle)


def iter_puzzles(sources: Iterable[str], from_file: bool = False) -> Iterator[tuple[str, int]]:
    """Yield (puzzle, invalid_count) pairs from strings or files."""
    if from_file:
        for path in sources:
            with open(path, "r", encoding="utf-8") as handle:
                for line in handle:
                    puzzle = _clean_line(line)
                    if not puzzle:
                        continue
                    invalid = _count_invalid(puzzle)
                    yield puzzle, invalid
    else:
        for puzzle in sources:
            cleaned = _clean_line(puzzle)
            if not cleaned:
                continue
            invalid = _count_invalid(cleaned)
            yield cleaned, invalid
