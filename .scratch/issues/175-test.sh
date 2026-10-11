#!/bin/bash
# Runs the tests ticket 175 touches, in the OrbStack machine: `175-test.sh [filters…]`.
cd "$(dirname "$0")/../.." || exit 1
export CARGO_TARGET_DIR="$HOME/target-174"
ulimit -n "$(ulimit -Hn)"
touch caminito/src/lib.rs
dbus-run-session -- cargo test -p caminito --lib -- "$@" 2>&1 | grep -v "^test .* ok$" | tail -60
