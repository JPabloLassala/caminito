#!/bin/bash
# Breaks one part of 246 on purpose, runs the Overview's tests on two outputs, and puts the
# file back. Run in the OrbStack machine from the worktree:
# 246-break.sh <file> <python-replace-old> <new>
set -u
file=$1; old=$2; new=$3
cp "$file" "$file.bak"
python3 -c "import sys;p=sys.argv[1];s=open(p).read();assert sys.argv[2] in s, 'not found';open(p,'w').write(s.replace(sys.argv[2],sys.argv[3],1))" "$file" "$old" "$new" || { mv "$file.bak" "$file"; exit 2; }
touch "$file"
ulimit -n $(ulimit -Hn)
unset DISPLAY
CARGO_TARGET_DIR=$HOME/target-246 dbus-run-session -- cargo test -p caminito --lib -- tests::overview_outputs 2>&1 | grep -E "^error|test result|FAILED" | sort -u | head -20
mv "$file.bak" "$file"
touch "$file"
