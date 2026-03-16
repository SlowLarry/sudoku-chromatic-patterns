#!/bin/bash
source ~/.cargo/env
cd "/mnt/c/Users/User/chromatic pattern search/rust"

BASE="/mnt/c/Users/User/chromatic pattern search"
REF="$BASE/src/sudoku_graph_searches/reference lists"

for f in "$REF"/all*.txt; do
    name=$(basename "$f" .txt)
    out="$BASE/ref_${name}_minlex.txt"
    echo "=== Minlexing $name ==="
    cargo run --release -- minlex --input "$f" --output "$out"
    echo
done
