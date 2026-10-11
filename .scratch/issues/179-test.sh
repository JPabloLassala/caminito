#!/bin/bash
# Runs cargo for ticket 179 in the OrbStack machine: `179-test.sh <cargo arguments>`.
cd "$(dirname "$0")/../.." || exit 1
export CARGO_TARGET_DIR="$HOME/target-179"
ulimit -n "$(ulimit -Hn)"
exec dbus-run-session -- cargo "$@"
