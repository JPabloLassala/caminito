#!/usr/bin/env bash
# CMN-275: each patched package, as its AUR repo would hold it (its tracked directory and its
# patches, nothing else of the monorepo), passes makepkg --printsrcinfo and --verifysource.
# namcap's output for each PKGBUILD is written beside, to compare with master's.
#   .scratch/checks/CMN-275/aur-copies.sh <checkout> <work dir>
set -euo pipefail
root=$(realpath "$1")
work=$2
mkdir -p "$work"
fail=0
for dir in "$root"/packaging/arch/*/; do
  package=$(basename "$dir")
  component=$(sed -n 's/^_pkgname=//p' "$dir/PKGBUILD")
  patches=$root/packaging/patches/$component
  [[ -n $component && -d $patches ]] || continue
  aur=$work/$package
  rm -rf "$aur"
  mkdir -p "$aur"
  git -C "$root" ls-files -z "packaging/arch/$package" | while IFS= read -r -d '' f; do
    cp "$root/$f" "$aur/"
  done
  cp "$patches"/*.patch "$aur/"
  if [[ -d $patches/arch ]]; then cp "$patches"/arch/*.patch "$aur/"; fi
  echo "==> $package"
  if (cd "$aur" && makepkg --printsrcinfo >.SRCINFO && makepkg -A --verifysource >verify.log 2>&1); then
    echo "ok"
  else
    echo "FAILED"; tail -5 "$aur/verify.log"; fail=1
  fi
  namcap "$aur/PKGBUILD" >"$work/$package.namcap" 2>&1 || true
done
exit $fail
