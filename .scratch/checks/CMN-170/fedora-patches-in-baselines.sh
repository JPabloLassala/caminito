#!/usr/bin/env bash
# Tells, for each of Fedora f45's own patches, whether the new baseline already has it (it
# reverse-applies), or it applies to the baseline, or neither. Runs in the Fedora machine:
#   orb -m camino-fedora bash .scratch/checks/CMN-170/fedora-patches-in-baselines.sh <fedora dist-git dir>
set -uo pipefail
dist=$1
work=$HOME/cmn-170-work/baselines
mkdir -p "$work"
fetch() { # name url
  [ -d "$work/$1" ] && return
  mkdir -p "$work/$1"
  curl -fsSL "$2" | tar -x -z -C "$work/$1" --strip-components=1 2>/dev/null ||
    curl -fsSL "$2" | tar -x -J -C "$work/$1" --strip-components=1
}
fetch mate-session-manager https://github.com/mate-desktop/mate-session-manager/archive/57a9178e766cd02176dc1c2d70cc48f963ee9fff.tar.gz
fetch mate-panel https://github.com/mate-desktop/mate-panel/archive/58ac6c101bdf2e2975c0395723ad16e9c9764f5c.tar.gz
fetch mate-settings-daemon https://github.com/mate-desktop/mate-settings-daemon/releases/download/v1.29.1/mate-settings-daemon-1.29.1.tar.xz
fetch mate-control-center https://github.com/mate-desktop/mate-control-center/releases/download/v1.29.1/mate-control-center-1.29.1.tar.xz
fetch caja https://pub.mate-desktop.org/releases/1.28/caja-1.28.0.tar.xz
fetch mate-utils https://github.com/mate-desktop/mate-utils/releases/download/v1.29.1/mate-utils-1.29.1.tar.xz
fetch mate-power-manager https://pub.mate-desktop.org/releases/1.28/mate-power-manager-1.28.1.tar.xz
for c in mate-session-manager mate-settings-daemon mate-control-center caja mate-utils mate-power-manager; do
  for p in "$dist/$c"/*.patch; do
    [ -e "$p" ] || continue
    if patch -d "$work/$c" -p1 -R --dry-run -f -s <"$p" >/dev/null 2>&1; then r=in-baseline
    elif patch -d "$work/$c" -p1 --dry-run -f -s <"$p" >/dev/null 2>&1; then r=applies
    else r=NEITHER; fi
    echo "$c $(basename "$p") $r"
  done
done
