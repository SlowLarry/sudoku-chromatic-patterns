#!/bin/bash
source ~/.cargo/env
cd "/mnt/c/Users/User/chromatic pattern search/rust"

BASE="/mnt/c/Users/User/chromatic pattern search/src/sudoku_graph_searches/reference lists"

for f in "$BASE"/all*.txt; do
    name=$(basename "$f")
    echo "=== Validating $name ==="
    cargo run --release -- validate "$f"
    echo
done
