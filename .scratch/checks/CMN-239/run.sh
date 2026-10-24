#!/usr/bin/env bash
# CMN-239's check of the Dock applet's Kept folders, in two parts.
#
# The first is the C tests without a panel (meson test): the order of the items after the
# separator with the Trash last (model), the Kept folders in the settings across a restart and
# apart from the kept items (store), and the folder's contents in the popup's order, its custom
# icon, and putting files in it, a move on the same file system and a copy across (folder).
#
# The second runs the Kept folders against a headless Caminito: dk-probe, the dock in a layer
# surface of its own, is clicked with the pointer the way a user would, and the scenarios that
# mean to show something leave a picture of the output, turned into a PNG in the output directory:
#
#   10-kept-folder                a Kept folder in the row, between the separator and the Trash
#   11-kept-folder-grid           the grid popup of its contents, folders first, Open in Caja last
#   12-kept-folder-menu           its menu, Open in Caja and Remove from Dock
#   13-kept-folder-gone           a Kept folder whose folder was deleted: dimmed, and the click's
#                                 answer that it isn't available
#   14-kept-folder-vertical       the same on a vertical panel
#
# The tests behind them also check that a folder is kept and survives the panel's restart, that
# Remove from Dock takes it away, that dragging reorders the Kept folders and never puts one
# before the separator, that a file dropped on one is moved into it, and that a folder that is
# gone comes back with the folder. The Trash's own tests run with the rest, as the ticket asks
# that it still works.
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
work=$(mktemp -d -t caminito-cmn-239.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
fi
ninja -C "$build" >/dev/null
build=$(realpath "$build")
probe=$(find "$build" -name dk-probe -type f | head -1)
[ -n "$probe" ] || { echo "no dk-probe in $build" >&2; exit 1; }
mkdir -p "$out"
rm -f "${out:?}"/*.png "${out:?}"/*.ppm "${out:?}/report.txt"

{
  echo "== the C tests, without a panel"
  meson test -C "$build" model store folder trash --print-errorlogs 2>&1 |
    grep -E '^ *[0-9]+/[0-9]+|^(Ok|Fail|Expected)'

  echo "== the Kept folders against a headless Caminito"
  mkdir -p "$work/shots"
  (cd "$root" && CAMINITO_DOCK_PROBE=$probe CAMINITO_DOCK_SHOTS=$work/shots \
    cargo test -p caminito tests::dock_applet -- --ignored --test-threads=1 2>&1 |
    grep -E '^test |^test result: .*(passed|failed)') || true
  for shot in "$work"/shots/1[0-4]-*.ppm; do
    python3 -I "$here/../CMN-219/ppm2png.py" "$shot"
    cp "${shot%.ppm}.png" "$out/"
  done
} | tee "$out/report.txt"
echo "pictures in $out"
