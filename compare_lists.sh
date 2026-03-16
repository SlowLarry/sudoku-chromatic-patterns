#!/bin/bash
# Compare our minlexed results with the minlexed reference lists.
# 1. Check reference ⊆ ours (for N=10,12,13,14)
# 2. Filter our results to 2-band patterns, compare with reference

BASE="/mnt/c/Users/User/chromatic pattern search"

echo "============================================"
echo "Step 1: Check reference ⊆ our results"
echo "============================================"
echo

for pair in "10:all10" "12:all12" "13:all13_" "14:all14_"; do
    n="${pair%%:*}"
    ref_name="${pair##*:}"
    ours="$BASE/results_n${n}_minlex.txt"
    ref="$BASE/ref_${ref_name}_minlex.txt"

    if [ ! -f "$ours" ]; then
        echo "N=$n: our minlex file not found, skipping"
        continue
    fi
    if [ ! -f "$ref" ]; then
        echo "N=$n: reference minlex file not found, skipping"
        continue
    fi

    ref_count=$(wc -l < "$ref")
    ours_count=$(wc -l < "$ours")

    # comm -23: lines in ref but NOT in ours
    missing=$(comm -23 "$ref" "$ours" | wc -l)

    if [ "$missing" -eq 0 ]; then
        echo "N=$n: reference ($ref_count) ⊆ ours ($ours_count) ✓"
    else
        echo "N=$n: MISSING $missing reference patterns from our results!"
        comm -23 "$ref" "$ours" | head -5
    fi
done

echo
echo "============================================"
echo "Step 2: Filter our results to 2-band patterns"
echo "============================================"
echo
echo "A pattern is 2-band if it uses ≤2 row-bands or ≤2 column-stacks."
echo

python3 -c "
import sys

def is_2band(s):
    '''Check if pattern uses ≤2 row-bands or ≤2 column-stacks.'''
    bands = set()
    stacks = set()
    for i, ch in enumerate(s):
        if ch == '1':
            r, c = divmod(i, 9)
            bands.add(r // 3)
            stacks.add(c // 3)
    return len(bands) <= 2 or len(stacks) <= 2

pairs = [('10', 'all10'), ('12', 'all12'), ('13', 'all13_'), ('14', 'all14_')]
base = '$BASE'

for n, ref_name in pairs:
    ours_file = f'{base}/results_n{n}_minlex.txt'
    ref_file = f'{base}/ref_{ref_name}_minlex.txt'

    try:
        with open(ours_file) as f:
            ours = set(line.strip() for line in f if line.strip())
        with open(ref_file) as f:
            ref = set(line.strip() for line in f if line.strip())
    except FileNotFoundError as e:
        print(f'N={n}: file not found: {e}')
        continue

    # Filter our results to 2-band patterns
    ours_2band = set(p for p in ours if is_2band(p))

    # Compare
    only_ours = ours_2band - ref
    only_ref = ref - ours_2band

    print(f'N={n}: ours={len(ours)}, ours_2band={len(ours_2band)}, ref={len(ref)}')
    if only_ref:
        print(f'  In ref but not in ours_2band: {len(only_ref)}')
        for p in sorted(only_ref)[:3]:
            print(f'    {p}')
    if only_ours:
        print(f'  In ours_2band but not in ref: {len(only_ours)}')
        for p in sorted(only_ours)[:3]:
            print(f'    {p}')
    if not only_ref and not only_ours:
        print(f'  EXACT MATCH ✓')
    print()
"
