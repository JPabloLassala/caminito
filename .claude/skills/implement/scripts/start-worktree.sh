#!/usr/bin/env bash
# Create the worktree for <branch> from master, and check that it builds.
set -euo pipefail

branch="${1:?usage: start-worktree.sh <branch>}"
main_checkout="$(git worktree list --porcelain | sed -n '1s/^worktree //p')"
worktree="$main_checkout-${branch//\//-}"

git -C "$main_checkout" worktree add -q -b "$branch" "$worktree" master

# Seed target/ after the worktree exists, with -a: Cargo sees the checkout as
# newer than the copied artifacts and rebuilds the workspace's own crates.
seed=~/.cache/camino-target-seed
[ -d "$seed" ] && cp -a "$seed" "$worktree/target"

if ! (cd "$worktree" && cargo check -q --workspace --all-targets); then
  echo "cargo check failed. The worktree stays at: $worktree" >&2
  exit 1
fi

echo "$worktree"
