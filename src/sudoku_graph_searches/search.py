"""Recursive search for minimal 4-chromatic patterns."""

from __future__ import annotations

from dataclasses import dataclass
import time
from typing import Callable, Iterable, List, Optional

from .canonical import canonical_signature, candidate_orbit_reps
from .sudoku_graph import neighbors_mask
from .utils_bitset import iter_bits, popcount
from .validation import is_valid_minimal_4chromatic_pattern

FULL_MASK = (1 << 81) - 1


@dataclass
class SearchStats:
    nodes: int = 0
    k4_prunes: int = 0
    degree_prunes: int = 0
    symmetry_prunes: int = 0
    orbit_prunes: int = 0
    leaves: int = 0
    solutions: int = 0
    stopped: bool = False
    stop_reason: Optional[str] = None
    elapsed_seconds: float = 0.0
    progress_pct: float = 0.0  # estimated % of search space explored


def _has_triangle(mask: int) -> bool:
    """Return True if the induced subgraph on mask contains a triangle."""
    for u in iter_bits(mask):
        neighbors_u = neighbors_mask[u] & mask
        if popcount(neighbors_u) < 2:
            continue
        for v in iter_bits(neighbors_u):
            others = neighbors_u & ~(1 << v)
            if neighbors_mask[v] & others:
                return True
    return False


def _degree_feasible(
    chosen_vertices: List[int],
    chosen_mask: int,
    degrees: List[int],
    target_size: int,
    reachable_mask: int,
) -> bool:
    remaining = target_size - len(chosen_vertices)
    if remaining < 0:
        return False
    # Only count vertices that can actually still be added (candidates not yet chosen)
    addable_mask = reachable_mask & ~chosen_mask
    for v in chosen_vertices:
        if degrees[v] >= 3:
            continue
        possible = popcount(neighbors_mask[v] & addable_mask)
        if degrees[v] + possible < 3:
            return False
    return True


def search_minimal_4chromatic(
    target_size: int,
    start_vertices: Optional[Iterable[int]] = None,
    limit: Optional[int] = None,
    symmetry_prune: bool = True,
    max_nodes: Optional[int] = None,
    max_seconds: Optional[float] = None,
    progress_seconds: Optional[float] = None,
    progress_callback: Optional[Callable[[SearchStats], None]] = None,
    known_signatures: Optional[set] = None,
    on_solution: Optional[Callable[[list[int]], None]] = None,
) -> tuple[list[list[int]], SearchStats]:
    """Search connected K4-free subsets of given size and validate them.

    All cells are equivalent under sudoku symmetry, so a single root
    (default: cell 0) suffices.  At each depth the automorphism group
    of the current partial pattern is computed and only one candidate
    per orbit is explored (*orbit pruning*).  ``start_pos`` ordering
    ensures each subset is generated in exactly one candidate-list
    order, preventing combinatorial duplicates without memory overhead.
    Leaf-level canonical dedup catches any remaining equivalences.
    """
    if target_size <= 0:
        return [], SearchStats()

    results: list[list[int]] = []
    stats = SearchStats()
    degrees = [0] * 81

    if start_vertices is None:
        # All cells are equivalent under sudoku symmetry; cell 0 suffices.
        roots = [0]
    else:
        roots = list(start_vertices)

    # Leaf-level symmetry dedup via canonical certificate.
    seen_leaves: set = set(known_signatures) if known_signatures else set()

    start_time = time.monotonic()
    last_progress = start_time

    def _should_stop() -> bool:
        if max_nodes is not None and stats.nodes >= max_nodes:
            stats.stopped = True
            stats.stop_reason = "max_nodes"
            return True
        if max_seconds is not None:
            elapsed = time.monotonic() - start_time
            if elapsed >= max_seconds:
                stats.stopped = True
                stats.stop_reason = "max_seconds"
                return True
        return False

    def _maybe_report() -> None:
        nonlocal last_progress
        if progress_seconds is None:
            return
        now = time.monotonic()
        if now - last_progress >= progress_seconds:
            stats.elapsed_seconds = now - start_time
            _update_progress_pct()
            if progress_callback is not None:
                progress_callback(stats)
            last_progress = now

    progress_frac = [0.0]

    def _update_progress_pct() -> None:
        stats.progress_pct = progress_frac[0] * 100.0

    def backtrack(
        chosen_vertices: List[int],
        chosen_mask: int,
        candidate_list: List[int],
        candidate_mask: int,
        start_pos: int,
        frac_lo: float,
        frac_hi: float,
    ) -> bool:
        stats.nodes += 1
        if _should_stop():
            return True
        _maybe_report()

        if not _degree_feasible(chosen_vertices, chosen_mask, degrees, target_size, candidate_mask):
            stats.degree_prunes += 1
            return False

        if len(chosen_vertices) == target_size:
            stats.leaves += 1
            if is_valid_minimal_4chromatic_pattern(chosen_vertices):
                if symmetry_prune:
                    signature = canonical_signature(chosen_mask)
                    if signature in seen_leaves:
                        stats.symmetry_prunes += 1
                        return False
                    seen_leaves.add(signature)
                result_copy = list(chosen_vertices)
                results.append(result_copy)
                stats.solutions += 1
                if on_solution is not None:
                    on_solution(result_copy)
                if limit is not None and stats.solutions >= limit:
                    stats.stopped = True
                    stats.stop_reason = "solution_limit"
                    return True
            return False

        # Collect eligible candidates (respecting start_pos, skipping chosen).
        eligible = []
        for idx in range(start_pos, len(candidate_list)):
            v = candidate_list[idx]
            if not (chosen_mask >> v) & 1:
                eligible.append((idx, v))

        # Orbit pruning via pynauty setwise-stabiliser orbits.
        if symmetry_prune and len(eligible) > 1:
            just_cells = [v for _, v in eligible]
            reps, _ = candidate_orbit_reps(chosen_mask, just_cells)
            rep_set = set(reps)
            to_try = []
            for idx, v in eligible:
                if v not in rep_set:
                    stats.orbit_prunes += 1
                    continue
                to_try.append((idx, v))
        else:
            to_try = eligible

        n_cands = len(to_try)
        step = (frac_hi - frac_lo) / max(n_cands, 1)

        for child_idx, (idx, v) in enumerate(to_try):
            neighbor_in_chosen = neighbors_mask[v] & chosen_mask
            if neighbor_in_chosen and _has_triangle(neighbor_in_chosen):
                stats.k4_prunes += 1
                continue

            new_mask = chosen_mask | (1 << v)
            updated = []
            for u in iter_bits(neighbor_in_chosen):
                degrees[u] += 1
                updated.append(u)
            degrees[v] = popcount(neighbor_in_chosen)
            updated.append(v)

            old_cand_len = len(candidate_list)
            child_cand_mask = candidate_mask
            new_neighbors = neighbors_mask[v] & ~new_mask & ~candidate_mask
            for u in iter_bits(new_neighbors):
                candidate_list.append(u)
                child_cand_mask |= 1 << u

            child_lo = frac_lo + child_idx * step
            child_hi = child_lo + step
            progress_frac[0] = child_lo

            chosen_vertices.append(v)
            should_stop = backtrack(
                chosen_vertices,
                new_mask,
                candidate_list,
                child_cand_mask,
                idx + 1,
                child_lo,
                child_hi,
            )
            chosen_vertices.pop()
            del candidate_list[old_cand_len:]

            for u in updated:
                degrees[u] = 0 if u == v else degrees[u] - 1
            if should_stop:
                return True
        return False

    num_roots = len(roots)
    for root_idx, root in enumerate(roots):
        root_lo = root_idx / max(num_roots, 1)
        root_hi = (root_idx + 1) / max(num_roots, 1)
        chosen_vertices = [root]
        chosen_mask = 1 << root
        degrees[root] = 0
        candidate_list = list(iter_bits(neighbors_mask[root]))
        candidate_mask = neighbors_mask[root]

        should_stop = backtrack(
            chosen_vertices,
            chosen_mask,
            candidate_list,
            candidate_mask,
            0,
            root_lo,
            root_hi,
        )

        progress_frac[0] = root_hi
        _update_progress_pct()
        if should_stop or stats.stopped:
            break

    stats.elapsed_seconds = time.monotonic() - start_time

    return results, stats
