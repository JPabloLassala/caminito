#!/usr/bin/env bash
# Merge <branch> into master in the main checkout, then delete its worktree and the branch.
# Safe to run again after a stop: a merge that is already in master changes nothing.
set -euo pipefail

branch="${1:?usage: finish-worktree.sh <branch>}"
main_checkout="$(git worktree list --porcelain | sed -n '1s/^worktree //p')"
worktree="$main_checkout-${branch//\//-}"

if [ -d "$worktree" ] && [ -n "$(git -C "$worktree" status --porcelain)" ]; then
  echo "The worktree has uncommitted changes. Nothing merged: $worktree" >&2
  exit 1
fi

git -C "$main_checkout" checkout -q master
before="$(git -C "$main_checkout" rev-parse master)"

if ! git -C "$main_checkout" merge -q --no-edit "$branch"; then
  echo "The merge did not complete. The worktree stays at: $worktree" >&2
  exit 1
fi

if [ -d "$worktree" ]; then
  git -C "$main_checkout" worktree remove "$worktree"
fi
git -C "$main_checkout" branch -q -d "$branch"

git -C "$main_checkout" log --oneline "$before..master"
