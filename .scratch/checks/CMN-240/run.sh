#!/usr/bin/env bash
# CMN-240's check of the Dock applet's Removable drive items: the C tests without a panel
# (meson test). The order of the items after the separator, the Removable drive items after
# the Kept folders and before the Trash (model); which mounts have an item, the list
# following the mounts as disks come and go, Eject or Unmount with the error told, and files
# dropped on one copied to it (drive), all against GIO's mounts faked in the test; and the
# Kept folders' and the Trash's own tests (folder, trash), as the drive's copy shares the
# folder's.
#
# There are no pictures: a mount can't be faked under the headless Caminito, whose dk-probe
# reads the real volume monitor. A drive in the row and its menu are the developer's to see in
# a real Session, under "The Dock applet's Removable drive items in a real Session" in
# docs/testing/checklist.md.
#
# report.txt has the tests that ran.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs what
# CMN-219's check needs.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-240.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
fi
ninja -C "$build" >/dev/null
mkdir -p "$out"
rm -f "${out:?}/report.txt"

{
  echo "== the C tests, without a panel"
  meson test -C "$build" model drive folder trash --print-errorlogs 2>&1 |
    grep -E '^ *[0-9]+/[0-9]+|^(Ok|Fail|Expected)'
} | tee "$out/report.txt"
