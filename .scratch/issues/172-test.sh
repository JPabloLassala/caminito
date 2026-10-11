#!/bin/bash
# Runs cargo for ticket 172's worktree in the OrbStack machine, on a target directory of its
# own: `172-test.sh test -p caminito minimise_animation`.
set -eu
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR="$HOME/target-172"
ulimit -n "$(ulimit -Hn)"
exec dbus-run-session -- cargo "$@"
