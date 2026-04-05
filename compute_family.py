"""Compute parent-child family tree relationships between minimal 4-chromatic patterns.

A pattern P of size N is a child of pattern Q of size M (M < N) if:
  - P can be reduced to Q's graph by a chain of diamond merges
  - Each merge identifies two non-adjacent vertices sharing ≥2 common neighbors

Supports chains of arbitrary depth to bridge gaps (e.g., N=12 → N=10 via 2 merges).

This script reads patterns.json, computes all diamond-merge relationships,
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
    """Find all diamond structures: non-adjacent pairs with two adjacent common neighbors.

    A valid diamond is K₄ minus one edge: tips a,b (non-adjacent) sharing
    spine vertices u,v where u-v IS an edge."""
    adj = defaultdict(set)
    for i, j in edges:
        adj[i].add(j)
        adj[j].add(i)

    diamonds = []
    for a in range(n):
        for b in range(a + 1, n):
            if b in adj[a]:
                continue
            common = sorted(adj[a] & adj[b])
            # Need at least one adjacent pair among common neighbors (spine)
            has_spine = False
            for ci in range(len(common)):
                for cj in range(ci + 1, len(common)):
                    if common[cj] in adj[common[ci]]:
                        has_spine = True
                        break
                if has_spine:
                    break
            if has_spine:
                diamonds.append((a, b))
    return diamonds


def find_ancestors(edges, n, catalog_sizes, cert_to_id, max_depth, visited_certs=None):
    """Recursively find catalog matches via chains of diamond merges.

    Returns list of (ancestor_id, chain_depth) tuples.
    """
    if visited_certs is None:
        visited_certs = set()

    # Prevent revisiting same graph
    cert = graph_certificate(edges, n)
    if cert in visited_certs:
        return []
    visited_certs.add(cert)

    results = []
    diamonds = find_diamonds(edges, n)

    for a, b in diamonds:
        merged_edges, merged_n = merge_vertices(edges, n, a, b)
        merged_cert = graph_certificate(merged_edges, merged_n)

        # Check if merged graph matches a catalog pattern
        if merged_n in catalog_sizes:
            key = (merged_n, merged_cert)
            if key in cert_to_id:
                results.append((cert_to_id[key], 1))

        # Recurse deeper if allowed
        if max_depth > 1 and merged_n > min(catalog_sizes):
            deeper = find_ancestors(
                [tuple(e) for e in merged_edges], merged_n,
                catalog_sizes, cert_to_id, max_depth - 1, visited_certs
            )
            for anc_id, depth in deeper:
                results.append((anc_id, depth + 1))

    return results


def main():
    base = Path(__file__).parent
    data = json.load(open(base / 'web' / 'data' / 'patterns.json'))
    patterns = data['patterns']

    # Group by size
    by_size = defaultdict(list)
    for p in patterns:
        by_size[p['size']].append(p)

    catalog_sizes = set(by_size.keys())

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

    # Determine max chain depth per size based on gaps
    # e.g., N=12 needs depth 2 to reach N=10 (gap of 1 at N=11)
    all_sizes = sorted(catalog_sizes)
    max_chain = {}
    for size in all_sizes:
        # Find nearest smaller catalog size
        smaller = [s for s in all_sizes if s < size]
        if not smaller:
            max_chain[size] = 0
        else:
            gap = size - max(smaller)
            max_chain[size] = gap  # depth = gap (1 merge per step)
    print(f"\nMax chain depths: {max_chain}")

    # Find parent-child relationships via diamond merge chains
    print("\nFinding diamond parent-child relationships...")
    family = defaultdict(lambda: {'parents': [], 'children': []})
    total_links = 0

    for size in sorted(by_size.keys()):
        depth = max_chain[size]
        if depth == 0:
            print(f"  N={size}: no smaller patterns, skipping")
            continue

        pats = by_size[size]
        found = 0
        t1 = time.time()

        for pi, p in enumerate(pats):
            n = p['size']
            edges = [tuple(e) for e in p['edges']]

            ancestors = find_ancestors(edges, n, catalog_sizes, cert_to_id, depth)

            # Deduplicate by ancestor id
            seen_ancestors = set()
            for anc_id, chain_depth in ancestors:
                if anc_id in seen_ancestors:
                    continue
                seen_ancestors.add(anc_id)

                family[p['id']]['parents'].append({
                    'id': anc_id,
                    'chain': chain_depth,
                    'type': 'diamond',
                })
                family[anc_id]['children'].append({
                    'id': p['id'],
                    'chain': chain_depth,
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
