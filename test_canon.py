"""Test fixed canonicalization using pynauty.certificate()."""
import sys
sys.path.insert(0, "src")

from sudoku_graph_searches.canonical import canonical_signature
from sudoku_graph_searches.sudoku_graph import row_of, col_of

def decode(bitstring):
    return [i for i, c in enumerate(bitstring) if c == "1"]

with open("results_n10.txt") as f:
    lines = [l.strip() for l in f if l.strip()]

# Test known-equivalent pair (lines 13 and 14: col 3<->4 swap)
p13 = decode(lines[12])
p14 = decode(lines[13])
s13 = canonical_signature(p13)
s14 = canonical_signature(p14)
print(f"Lines 13 vs 14 (should match):  {s13 == s14}")

# Test non-equivalent pair (lines 1 and 54)
p1 = decode(lines[0])
p54 = decode(lines[53])
s1 = canonical_signature(p1)
s54 = canonical_signature(p54)
print(f"Lines 1 vs 54 (should differ): {s1 != s54}")

# Count unique signatures among all 54
sigs = {}
for i, line in enumerate(lines):
    verts = decode(line)
    sig = canonical_signature(verts)
    if sig not in sigs:
        sigs[sig] = []
    sigs[sig].append(i + 1)

print(f"\n{len(lines)} patterns -> {len(sigs)} unique canonical classes")
for sig, indices in sorted(sigs.items(), key=lambda x: x[1][0]):
    v = decode(lines[indices[0] - 1])
    coords = [(row_of[c], col_of[c]) for c in sorted(v)]
    print(f"  class ({len(indices)} members): lines {indices}")
    print(f"    representative: {coords}")





