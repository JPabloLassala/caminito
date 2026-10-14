#!/bin/bash
# Breaks one part of 244 on purpose, runs the Overview's tests, and puts the file back.
# Run in the OrbStack machine from the worktree: 244-break.sh <file> <python-replace-old> <new>
set -u
file=$1; old=$2; new=$3
cp "$file" "$file.bak"
python3 -c "import sys;p=sys.argv[1];s=open(p).read();assert sys.argv[2] in s, 'not found';open(p,'w').write(s.replace(sys.argv[2],sys.argv[3],1))" "$file" "$old" "$new" || { mv "$file.bak" "$file"; exit 2; }
touch "$file"
ulimit -n $(ulimit -Hn)
CARGO_TARGET_DIR=$HOME/target-244 dbus-run-session -- cargo test -p caminito --lib -- tests::overview 2>&1 | grep -E "^error|test result|FAILED" | sort -u | head -20
mv "$file.bak" "$file"
touch "$file"
