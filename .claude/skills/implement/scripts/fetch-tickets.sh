#!/usr/bin/env bash
# Print each CMN ticket named in the arguments or on stdin: the ticket (the view includes its
# comments, parent and sub-issues), its relations, and the last comment of each ticket that
# blocks it. /implement runs this when the skill loads, and a non-zero exit there aborts the
# skill, so every failure prints a message and the script exits 0.

ids="$( { printf '%s\n' "$@"; [ -t 0 ] || cat; } \
  | grep -oiE 'CMN-[0-9]+' | tr '[:lower:]' '[:upper:]' | awk '!seen[$0]++' )"

if [ -z "$ids" ]; then
  echo "No CMN ticket in the arguments."
  exit 0
fi

for id in $ids; do
  if ! linear issue view "$id" --no-pager --no-input 2>&1; then
    echo "Could not fetch $id."
    echo
    continue
  fi

  relations="$(linear issue relation list "$id" --no-input 2>&1 | tail -n +2)"
  echo
  echo "## Relations"
  echo "$relations"

  for blocker in $(echo "$relations" | sed -n "s/^ *$id blocked-by \(CMN-[0-9]*\):.*/\1/p"); do
    echo
    echo "## Last comment on $blocker, which blocks $id"
    linear issue view "$blocker" --json --no-input 2>&1 \
      | python3 -I -c 'import json, sys; c = sorted(json.load(sys.stdin)["comments"], key=lambda c: c["createdAt"]); print(c[-1]["body"] if c else "(no comments)")' 2>&1 \
      || echo "Could not read the comments of $blocker."
  done
  echo
done

exit 0
