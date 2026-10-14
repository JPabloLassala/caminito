#!/bin/bash
# Runs a cargo command for ticket 241 in the OrbStack machine, from the worktree, with its
# own target directory: `orb -m camino bash .scratch/issues/241-checks/run.sh test -p caminito-policy overview`.
set -euo pipefail
cd "$(dirname "$0")/../../.."
unset DISPLAY
# Ticket 239's directory, which is merged: the machine's disk has no room for another.
export CARGO_TARGET_DIR="$HOME/target-239"
ulimit -n "$(ulimit -Hn)"
exec dbus-run-session -- cargo "$@"
