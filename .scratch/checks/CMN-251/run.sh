#!/usr/bin/env bash
# CMN-251's check of the Dock applet's capture of a window, in two parts. Nothing of it is
# visible in the dock yet, so the check leaves no pictures: it reports the tests.
#
# The first is the C test without a display (meson test capture): with no Wayland display, as
# under X11 MATE, a capture fails at once, and the applet goes on as before.
#
# The second runs dk-probe against a headless Caminito, which captures a test client's window
# by its title through the dock's own capture:
#
#   - a window drawn in one colour is captured in that colour, at its size with its frame;
#   - a live session gets a new frame when the window redraws in another colour, and once the
#     probe stops it, Caminito has no session left for the window;
#   - a minimised window gives the colour it was last drawn in;
#   - a live session on a window that closes ends, and says so.
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
work=$(mktemp -d -t caminito-cmn-251.XXXXXX)
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
rm -f "${out:?}/report.txt"

{
  echo "== the capture without a display"
  meson test -C "$build" capture --print-errorlogs 2>&1 |
    grep -E '^ *[0-9]+/[0-9]+|^(Ok|Fail|Expected)'

  echo "== the capture against a headless Caminito"
  (cd "$root" && CAMINITO_DOCK_PROBE=$probe cargo test -p caminito -- --ignored \
    tests::dock_applet::the_dock_captures tests::dock_applet::a_live_capture \
    tests::dock_applet::a_minimised_window_is_captured 2>&1 |
    grep -E '^test |^test result: .*(passed|failed)') || true
} | tee "$out/report.txt"
