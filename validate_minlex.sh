#!/bin/bash
source ~/.cargo/env
cd "/mnt/c/Users/User/chromatic pattern search/rust"

BASE="/mnt/c/Users/User/chromatic pattern search"

for n in 10 12 13 14; do
    echo "=== Validating minlex N=$n ==="
    cargo run --release -- validate "$BASE/results_n${n}_minlex.txt"
    echo
done
