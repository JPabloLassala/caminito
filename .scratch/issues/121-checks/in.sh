#!/usr/bin/env bash
# Runs a command in the session up.sh left up:  in.sh <work> <command> ...
work=$1; shift
set -a; . "$work/env"; set +a
exec "$@"
