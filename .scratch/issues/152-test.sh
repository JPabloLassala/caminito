#!/usr/bin/env bash
# Runs caminito's tests that match the arguments, in the OrbStack machine, for ticket 152.
#   orb -m camino bash .scratch/issues/152-test.sh [cargo test arguments]
cd "$(dirname "$(realpath "$0")")/../.." || exit 1
export CARGO_TARGET_DIR=$HOME/target-152
ulimit -n "$(ulimit -Hn)"
exec dbus-run-session -- cargo test "$@"
