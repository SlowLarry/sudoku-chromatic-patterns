#!/bin/bash
source ~/.cargo/env
cd "/mnt/c/Users/User/chromatic pattern search/rust"

BASE="/mnt/c/Users/User/chromatic pattern search"

for n in 10 12 13 14; do
    IN="$BASE/results_n${n}_rust.txt"
    OUT="$BASE/results_n${n}_minlex.txt"
    echo "=== Minlexing N=$n ==="
    time cargo run --release -- minlex --input "$IN" --output "$OUT"
    echo
done
