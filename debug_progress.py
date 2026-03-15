import sys; sys.path.insert(0, "src")
from sudoku_graph_searches.sudoku_graph import neighbors_mask
from sudoku_graph_searches.utils_bitset import iter_bits, popcount

root = 0
cands = list(iter_bits(neighbors_mask[root]))
print("root 0: num_candidates =", len(cands))

v1 = cands[0]
mask1 = (1 << root) | (1 << v1)
cand1_mask = neighbors_mask[root]
new_nb = neighbors_mask[v1] & ~mask1 & ~cand1_mask
cand1_total = len(cands) + popcount(new_nb)
d2_total = cand1_total - 1  # candidates after d1_idx=0
print(f"d1_idx=0, v1={v1}, d2_total = {d2_total}")
print(f"With 4 roots, d2 progress of 5/{d2_total}: pct = {(0 + (0 + 5/max(d2_total,1))/20) / 4 * 100:.4f}%")
