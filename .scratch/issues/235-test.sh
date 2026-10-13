#!/bin/bash
# Runs `cargo test -p caminito` with the arguments given, in the OrbStack machine.
cd "$(dirname "$0")/../.." || exit 1
export CARGO_TARGET_DIR=$HOME/target-234
ulimit -n "$(ulimit -Hn)"
touch caminito/src/lib.rs
dbus-run-session -- cargo test -p caminito --lib -- "$@" 2>&1 | tail -n "${TAIL:-60}"
