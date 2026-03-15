"""Command line interface for Sudoku graph metrics."""

from __future__ import annotations

import argparse
import os
from dataclasses import dataclass
import time
from typing import Iterable

from .graph import build_constraint_graph
from .io import iter_puzzles
from .metrics import degree_histogram, graph_stats
from .search import search_minimal_4chromatic


@dataclass
class CliResult:
    processed: int
    skipped: int


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Report Sudoku constraint graph metrics for puzzles."
    )
    subparsers = parser.add_subparsers(dest="command")

    group = parser.add_mutually_exclusive_group()
    group.add_argument("--puzzle", help="81-char puzzle string (0 or . for blanks)")
    group.add_argument("--file", help="Path to file with one puzzle per line")
    parser.add_argument(
        "--histogram",
        action="store_true",
        help="Print a degree histogram",
    )

    search_parser = subparsers.add_parser("search", help="Run minimal 4-chromatic search")
    search_parser.add_argument(
        "--size",
        type=int,
        required=True,
        help="Target pattern size",
    )
    search_parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Maximum number of solutions to return",
    )
    search_parser.add_argument(
        "--no-symmetry",
        action="store_true",
        help="Disable symmetry pruning (requires no pynauty)",
    )
    search_parser.add_argument(
        "--max-nodes",
        type=int,
        default=None,
        help="Stop after visiting this many search nodes",
    )
    search_parser.add_argument(
        "--max-seconds",
        type=float,
        default=None,
        help="Stop after this many seconds of search time",
    )
    search_parser.add_argument(
        "--progress-seconds",
        type=float,
        default=None,
        help="Print a progress line every N seconds",
    )
    search_parser.add_argument(
        "--roots",
        default=None,
        help="Comma-separated root cells or ranges (e.g. 0,1,2,10-20)",
    )
    search_parser.add_argument(
        "--compact",
        action="store_true",
        help="Print the 0/1 string on a single line",
    )
    search_parser.add_argument(
        "--output",
        default=None,
        help="Append found patterns (0/1 strings) to this file",
    )
    search_parser.add_argument(
        "--skip-file",
        default=None,
        help="File of known 0/1 patterns to skip (one per line)",
    )
    return parser


def _print_header() -> None:
    print("sudoku-graph-searches")
    print("----------------------")


def _print_puzzle_stats(index: int, puzzle: str, invalid: int) -> None:
    filled = sum(ch in "123456789" for ch in puzzle)
    blanks = sum(ch in "0." for ch in puzzle)
    print(f"puzzle {index}: filled={filled} blanks={blanks} invalid={invalid}")


def _print_graph_stats(graph: dict[int, set[int]], show_histogram: bool) -> None:
    stats = graph_stats(graph)
    print(
        "graph: nodes={nodes} edges={edges} min_deg={min_deg} max_deg={max_deg} avg_deg={avg_deg:.2f}".format(
            **stats
        )
    )
    if show_histogram:
        hist = degree_histogram(graph)
        print("degree histogram:")
        for degree in sorted(hist):
            print(f"  {degree}: {hist[degree]}")


def _bitstring_to_vertices(bitstring: str) -> list[int]:
    """Parse an 81-char 0/1 string into a list of cell indices."""
    return [i for i, ch in enumerate(bitstring) if ch == "1"]


def _load_known_signatures(path: str) -> set[tuple[int, ...]]:
    """Load known patterns from a file and return their canonical signatures."""
    from .canonical import canonical_signature

    sigs: set[tuple[int, ...]] = set()
    if not os.path.exists(path):
        return sigs
    with open(path, "r") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if len(line) == 81 and all(ch in "01" for ch in line):
                verts = _bitstring_to_vertices(line)
                if verts:
                    sigs.add(canonical_signature(verts))
    print(f"skip-file: loaded {len(sigs)} known canonical signatures from {path}")
    return sigs


def _append_pattern_to_file(path: str, pattern: list[int]) -> None:
    """Append a pattern as an 81-char 0/1 string to a file."""
    with open(path, "a") as f:
        f.write(_pattern_bitstring(pattern) + "\n")


def run(puzzles: Iterable[str], show_histogram: bool) -> CliResult:
    graph = build_constraint_graph()
    processed = 0
    skipped = 0
    for index, (puzzle, invalid) in enumerate(puzzles, start=1):
        if len(puzzle) != 81:
            print(f"puzzle {index}: skipped (length {len(puzzle)} != 81)")
            skipped += 1
            continue
        _print_puzzle_stats(index, puzzle, invalid)
        _print_graph_stats(graph, show_histogram)
        processed += 1
    return CliResult(processed=processed, skipped=skipped)


def _pattern_bitstring(pattern: list[int]) -> str:
    bits = ["0"] * 81
    for v in pattern:
        if 0 <= v < 81:
            bits[v] = "1"
    return "".join(bits)


def _print_search_results(
    patterns: list[list[int]],
    stats: "SearchStats",
    compact: bool,
) -> None:
    print(
        "search: nodes={nodes} k4_prunes={k4_prunes} degree_prunes={degree_prunes} "
        "orbit_prunes={orbit_prunes} symmetry_prunes={symmetry_prunes} "
        "leaves={leaves} solutions={solutions}".format(
            nodes=stats.nodes,
            k4_prunes=stats.k4_prunes,
            degree_prunes=stats.degree_prunes,
            orbit_prunes=stats.orbit_prunes,
            symmetry_prunes=stats.symmetry_prunes,
            leaves=stats.leaves,
            solutions=stats.solutions,
        )
    )
    for index, pattern in enumerate(patterns, start=1):
        print(f"solution {index}: {pattern}")
        if compact:
            print(_pattern_bitstring(pattern))
        else:
            print(f"solution {index} 0/1 string: {_pattern_bitstring(pattern)}")


def _parse_roots(spec: str | None) -> list[int] | None:
    if not spec:
        return None
    roots: list[int] = []
    for chunk in spec.split(","):
        chunk = chunk.strip()
        if not chunk:
            continue
        if "-" in chunk:
            start_str, end_str = chunk.split("-", 1)
            start = int(start_str)
            end = int(end_str)
            if start > end:
                start, end = end, start
            roots.extend(range(start, end + 1))
        else:
            roots.append(int(chunk))
    return roots


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    _print_header()
    if args.command == "search":
        roots = _parse_roots(args.roots)
        start_time = time.monotonic()

        def progress(stats: "SearchStats") -> None:
            elapsed = stats.elapsed_seconds
            rate = stats.nodes / elapsed if elapsed > 0 else 0.0
            pct = stats.progress_pct
            eta = ""
            if pct > 0 and elapsed > 0:
                remaining = elapsed * (100.0 - pct) / pct
                if remaining < 3600:
                    eta = f" eta={remaining:.0f}s"
                else:
                    eta = f" eta={remaining/3600:.1f}h"
            print(
                "progress: {pct:.6f}% nodes={nodes} leaves={leaves} solutions={solutions} "
                "k4={k4_prunes} deg={degree_prunes} orb={orbit_prunes} sym={symmetry_prunes} "
                "elapsed={elapsed:.1f}s rate={rate:.1f}/s{eta}".format(
                    pct=pct,
                    nodes=stats.nodes,
                    leaves=stats.leaves,
                    solutions=stats.solutions,
                    k4_prunes=stats.k4_prunes,
                    degree_prunes=stats.degree_prunes,
                    orbit_prunes=stats.orbit_prunes,
                    symmetry_prunes=stats.symmetry_prunes,
                    elapsed=elapsed,
                    rate=rate,
                    eta=eta,
                )
            )
        try:
            known_sigs = None
            if args.skip_file:
                known_sigs = _load_known_signatures(args.skip_file)

            output_path = args.output

            def on_solution(pattern: list[int]) -> None:
                bs = _pattern_bitstring(pattern)
                print(f"FOUND: {bs}")
                if output_path:
                    _append_pattern_to_file(output_path, pattern)

            patterns, stats = search_minimal_4chromatic(
                target_size=args.size,
                limit=args.limit,
                symmetry_prune=not args.no_symmetry,
                max_nodes=args.max_nodes,
                max_seconds=args.max_seconds,
                progress_seconds=args.progress_seconds,
                progress_callback=progress if args.progress_seconds else None,
                start_vertices=roots,
                known_signatures=known_sigs,
                on_solution=on_solution,
            )
        except ImportError as exc:
            print(str(exc))
            return 2
        if stats.stopped and stats.stop_reason:
            elapsed = stats.elapsed_seconds
            total = time.monotonic() - start_time
            print(
                "stopped: reason={reason} elapsed={elapsed:.1f}s total={total:.1f}s".format(
                    reason=stats.stop_reason,
                    elapsed=elapsed,
                    total=total,
                )
            )
        _print_search_results(patterns, stats, args.compact)
        if args.output and patterns:
            print(f"output: wrote {len(patterns)} pattern(s) to {args.output}")
        return 0

    if args.puzzle:
        puzzles = iter_puzzles([args.puzzle])
    elif args.file:
        puzzles = iter_puzzles([args.file], from_file=True)
    else:
        parser.error("Provide --puzzle or --file, or use the 'search' command.")
    result = run(puzzles, args.histogram)
    print(f"done: processed={result.processed} skipped={result.skipped}")
    return 0
