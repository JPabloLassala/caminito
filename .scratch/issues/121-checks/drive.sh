#!/usr/bin/env bash
# Runs a script in the editor's extension host, through the driver, and prints what it
# returned:  drive.sh <work> '<the body of an async function of vscode and sleep>'
# With no host to take it within 20 s, it prints nothing and fails.
work=$1; dir=$work/driver
n=$(printf '%04d' $(( $(ls "$dir" | grep -c '\.out$') + $(ls "$dir" | grep -c '\.js') + 1 )))
printf '%s' "$2" >"$dir/$n.tmp"; mv "$dir/$n.tmp" "$dir/$n.js"
for _ in $(seq 400); do [ -e "$dir/$n.out" ] && { cat "$dir/$n.out"; echo; exit 0; }; sleep 0.05; done
exit 1
