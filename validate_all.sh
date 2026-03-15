#!/bin/bash
source ~/.cargo/env
cd "/mnt/c/Users/User/chromatic pattern search/rust"

for n in 10 12 13 14; do
    echo "=== Validating N=$n ==="
    cargo run --release -- validate "/mnt/c/Users/User/chromatic pattern search/results_n${n}_rust.txt"
    echo
done
