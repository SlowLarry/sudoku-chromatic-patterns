"""Compute parent-child family tree relationships between minimal 4-chromatic patterns.

A pattern P of size N is a child of pattern Q of size N-1 if:
  - P contains a diamond (non-adjacent pair sharing ≥2 common neighbors)
  - Merging the diamond tips in P's graph yields a graph isomorphic to Q's graph

This script reads patterns.json, computes all diamond-merge parent relationships,
and writes family_tree.json for the web viewer.

Must be run under WSL with pynauty available.
"""

import json
import sys
import time
from collections import defaultdict
from pathlib import Path

import pynauty


def graph_certificate(edges, n):
    """Compute canonical certificate for a graph using pynauty."""
    g = pynauty.Graph(n)
    adj = defaultdict(list)
    for i, j in edges:
        adj[i].append(j)
        adj[j].append(i)
    for v in range(n):
        if adj[v]:
            g.connect_vertex(v, adj[v])
    return pynauty.certificate(g)


def merge_vertices(edges, n, a, b):
    """Merge vertices a and b. Returns (new_edges, n-1).
    Vertex b is removed; a inherits b's neighbors. Relabeled to 0..n-2."""
    adj = defaultdict(set)
    for i, j in edges:
        adj[i].add(j)
        adj[j].add(i)

    # Merge b into a
    for nbr in list(adj[b]):
        if nbr != a:
            adj[a].add(nbr)
            adj[nbr].add(a)
        adj[nbr].discard(b)
    del adj[b]
    adj[a].discard(a)  # no self-loops

    # Relabel: remaining vertices 0..n-2
    remaining = sorted(v for v in range(n) if v != b)
    remap = {v: i for i, v in enumerate(remaining)}

    new_edges = []
    seen = set()
    for v in remaining:
        for u in adj[v]:
            if u in remap:
                e = (min(remap[v], remap[u]), max(remap[v], remap[u]))
                if e not in seen:
                    seen.add(e)
                    new_edges.append(list(e))

    return new_edges, n - 1


def find_diamonds(edges, n):
    """Find all diamond structures: non-adjacent pairs sharing ≥2 common neighbors."""
    adj = defaultdict(set)
    for i, j in edges:
        adj[i].add(j)
        adj[j].add(i)

    diamonds = []
    for a in range(n):
        for b in range(a + 1, n):
            if b in adj[a]:
                continue
            common = adj[a] & adj[b]
            if len(common) >= 2:
                diamonds.append((a, b))
    return diamonds


def main():
    base = Path(__file__).parent
    data = json.load(open(base / 'web' / 'data' / 'patterns.json'))
    patterns = data['patterns']

    # Group by size
    by_size = defaultdict(list)
    for p in patterns:
        by_size[p['size']].append(p)

    # Precompute certificates for each size
    print("Computing canonical certificates...")
    t0 = time.time()
    cert_to_id = {}  # (size, cert_bytes) -> pattern_id
    for size in sorted(by_size.keys()):
        pats = by_size[size]
        for p in pats:
            cert = graph_certificate(p['edges'], p['size'])
            cert_to_id[(size, cert)] = p['id']
        print(f"  N={size}: {len(pats)} certificates computed")
    print(f"  Total: {len(cert_to_id)} in {time.time()-t0:.1f}s")

    # Find parent-child relationships via diamond merges
    print("\nFinding diamond parent-child relationships...")
    family = defaultdict(lambda: {'parents': [], 'children': []})
    total_links = 0

    for size in sorted(by_size.keys()):
        parent_size = size - 1
        has_parents = parent_size in {s for (s, _) in cert_to_id}
        if not has_parents:
            print(f"  N={size}: no N={parent_size} catalog, skipping")
            continue

        pats = by_size[size]
        found = 0
        t1 = time.time()

        for pi, p in enumerate(pats):
            n = p['size']
            edges = [tuple(e) for e in p['edges']]
            diamonds = find_diamonds(edges, n)

            found_parents = set()
            for a, b in diamonds:
                merged_edges, merged_n = merge_vertices(edges, n, a, b)
                cert = graph_certificate(merged_edges, merged_n)
                key = (parent_size, cert)

                if key in cert_to_id:
                    parent_id = cert_to_id[key]
                    if parent_id not in found_parents:
                        found_parents.add(parent_id)
                        cell_a = p['cells'][a]
                        cell_b = p['cells'][b]
                        family[p['id']]['parents'].append({
                            'id': parent_id,
                            'merge': f"r{cell_a[0]+1}c{cell_a[1]+1}=r{cell_b[0]+1}c{cell_b[1]+1}",
                            'type': 'diamond',
                        })
                        family[parent_id]['children'].append({
                            'id': p['id'],
                            'type': 'diamond',
                        })
                        found += 1
                        total_links += 1

            if (pi + 1) % 500 == 0:
                print(f"    {pi+1}/{len(pats)}...")

        elapsed = time.time() - t1
        with_parents = sum(1 for p in pats if family[p['id']]['parents'])
        print(f"  N={size}: {found} links found ({with_parents}/{len(pats)} patterns have parents) [{elapsed:.1f}s]")

    # Summary
    all_with_parents = sum(1 for v in family.values() if v['parents'])
    all_with_children = sum(1 for v in family.values() if v['children'])
    print(f"\nTotal: {total_links} parent-child links")
    print(f"Patterns with parents: {all_with_parents}")
    print(f"Patterns with children: {all_with_children}")

    # Write output
    out_path = base / 'web' / 'data' / 'family_tree.json'
    json.dump(dict(family), open(out_path, 'w'))
    print(f"\nWrote {out_path} ({out_path.stat().st_size / 1024:.1f} KB)")


if __name__ == '__main__':
    main()
