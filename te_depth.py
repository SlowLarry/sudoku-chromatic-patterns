#!/usr/bin/env python3
"""Compute T&E (Trial & Error) depth for proving non-3-colorability of sudoku patterns.

T&E depth measures the minimum nesting depth of tentative color assignments
needed to prove a pattern's induced graph is not 3-colorable:

  T&E(0): Constraint propagation alone (singleton forced assignments) reaches contradiction.
  T&E(n): By tentatively assigning a color to a vertex and showing contradiction
           via T&E(n-1), we can eliminate that color. Repeating at each nesting
           level eventually eliminates all possible 3-colorings.

Propagation = unit propagation: when a vertex has exactly one color left,
assign it and remove that color from all neighbors. Repeat until fixpoint.
"""

import sys
import time
from pathlib import Path


def build_sudoku_neighbors():
    """Build adjacency lists for the 81-cell sudoku graph."""
    nbrs = [[] for _ in range(81)]
    for v in range(81):
        r, c = divmod(v, 9)
        br, bc = 3 * (r // 3), 3 * (c // 3)
        for u in range(81):
            if u == v:
                continue
            ru, cu = divmod(u, 9)
            if ru == r or cu == c or (3 * (ru // 3) == br and 3 * (cu // 3) == bc):
                nbrs[v].append(u)
    return nbrs


SUDOKU_NBRS = build_sudoku_neighbors()


def bitstring_to_cells(bs):
    return [i for i, ch in enumerate(bs.strip()) if ch == '1']


def build_local_adj(cells):
    """Build local adjacency lists for the induced subgraph on `cells`."""
    idx = {c: i for i, c in enumerate(cells)}
    n = len(cells)
    adj = [[] for _ in range(n)]
    for i, c in enumerate(cells):
        for nb in SUDOKU_NBRS[c]:
            if nb in idx:
                adj[i].append(idx[nb])
    return adj


# Domain bitmask: bit 0 = color 1, bit 1 = color 2, bit 2 = color 3
# So domain 7 = 0b111 = all three colors available

_POPCOUNT = [bin(i).count('1') for i in range(8)]


def propagate(doms, adj):
    """Unit propagation for 3-coloring. Modifies `doms` in place.

    When a vertex has exactly one color left (singleton), remove that color
    from all its neighbors. Repeat until no more singletons to process.

    Returns True if a contradiction is found (some vertex has 0 colors).
    """
    n = len(doms)
    # Seed queue with existing singletons
    queue = [v for v in range(n) if doms[v] and (doms[v] & (doms[v] - 1)) == 0]

    while queue:
        v = queue.pop()
        d = doms[v]
        if d == 0:
            return True
        if d & (d - 1):  # no longer singleton (should not happen, but safe)
            continue
        for u in adj[v]:
            if doms[u] & d:
                doms[u] &= ~d
                if doms[u] == 0:
                    return True
                if (doms[u] & (doms[u] - 1)) == 0:  # became singleton
                    queue.append(u)
    return False


def te_solve(doms, adj, depth):
    """Test whether non-3-colorability can be proved at T&E nesting depth `depth`.

    At the current level we repeatedly:
      1. Pick any (vertex, color) where the vertex still has multiple colors.
      2. Tentatively assign that color and recurse at depth-1.
      3. If recursion proves contradiction, eliminate that color and propagate.
      4. Repeat until either a contradiction is reached or no more eliminations
         are possible.

    Returns True if non-3-colorability is proved.
    """
    doms = list(doms)  # work on a copy
    if propagate(doms, adj):
        return True
    if depth == 0:
        return False

    n = len(doms)
    changed = True
    while changed:
        changed = False
        # Try vertices in MRV order (smallest multi-valued domain first)
        order = sorted(range(n), key=lambda v: _POPCOUNT[doms[v]] if _POPCOUNT[doms[v]] > 1 else 99)

        for v in order:
            if _POPCOUNT[doms[v]] <= 1:
                break  # remaining are singletons or already assigned

            for c in (1, 2, 4):
                if not (doms[v] & c):
                    continue
                trial = list(doms)
                trial[v] = c
                if te_solve(trial, adj, depth - 1):
                    # Color c proved impossible for v — eliminate it
                    doms[v] &= ~c
                    changed = True
                    if doms[v] == 0:
                        return True  # all colors eliminated → contradiction
                    if propagate(doms, adj):
                        return True

            if changed:
                break  # restart scan with updated state

    return False


def compute_te_depth(cells, max_depth=5):
    """Compute the T&E depth of the induced subgraph on `cells`."""
    adj = build_local_adj(cells)
    n = len(adj)
    doms = [7] * n
    for d in range(max_depth + 1):
        if te_solve(list(doms), adj, d):
            return d
    return -1


def main():
    import argparse

    parser = argparse.ArgumentParser(description='Compute T&E depth for sudoku patterns')
    parser.add_argument('input', help='File with 81-char bitstrings, one per line')
    parser.add_argument('-d', '--max-depth', type=int, default=5, help='Maximum T&E depth to check (default: 5)')
    parser.add_argument('-o', '--output', help='Output file (one depth per line). Defaults to stdout.')
    parser.add_argument('--single', type=int, metavar='IDX', help='Process only the pattern at this 1-based index')
    args = parser.parse_args()

    lines = [l.strip() for l in Path(args.input).read_text().splitlines() if l.strip()]
    total = len(lines)

    if args.single:
        idx = args.single - 1
        cells = bitstring_to_cells(lines[idx])
        d = compute_te_depth(cells, args.max_depth)
        print(f"Pattern {args.single}/{total}: T&E({d})")
        return

    print(f"Processing {total} patterns (max_depth={args.max_depth})...", file=sys.stderr)

    depth_counts = {}
    results = []
    t0 = time.time()

    for i, bs in enumerate(lines):
        cells = bitstring_to_cells(bs)
        d = compute_te_depth(cells, args.max_depth)
        results.append(d)
        depth_counts[d] = depth_counts.get(d, 0) + 1

        if (i + 1) % 100 == 0 or (i + 1) == total:
            elapsed = time.time() - t0
            rate = (i + 1) / elapsed if elapsed > 0 else 0
            print(f"  {i + 1}/{total} ({elapsed:.1f}s, {rate:.1f}/s)", file=sys.stderr)

    elapsed = time.time() - t0
    print(f"\nDone in {elapsed:.1f}s", file=sys.stderr)
    print(f"Distribution:", file=sys.stderr)
    for d in sorted(depth_counts.keys()):
        label = f"T&E({d})" if d >= 0 else "UNKNOWN"
        print(f"  {label}: {depth_counts[d]}", file=sys.stderr)

    # Write results
    out = open(args.output, 'w') if args.output else sys.stdout
    for d in results:
        out.write(f"{d}\n")
    if args.output:
        out.close()
        print(f"Wrote {len(results)} results to {args.output}", file=sys.stderr)


if __name__ == '__main__':
    main()
