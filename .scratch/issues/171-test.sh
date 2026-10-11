#!/bin/bash
# Runs caminito's tests matching "$@" in the OrbStack machine, from this checkout.
cd "$(dirname "$0")/../.." || exit 1
export CARGO_TARGET_DIR=$HOME/target-171
ulimit -n "$(ulimit -Hn)"
exec dbus-run-session -- cargo test -p caminito "$@"
