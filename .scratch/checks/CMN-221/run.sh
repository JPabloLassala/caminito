#!/usr/bin/env bash
# CMN-221's check of the Dock applet's launch and attention feedback, against a headless
# Caminito with libxfce4windowing reading the windows, as tools/check does: dk-probe, the dock
# in a layer surface of its own, is clicked with the pointer the way a user would, and the
# scenarios leave pictures of the output, turned into PNGs in the output directory:
#
#   07-launch-pulse-1..4    the kept terminal's icon, after the click that launched it, in four
#                           frames: it pulses (opacity and size) until its first window maps
#   08-bounce-1..6          a window that demands attention: the icon, smaller, jumps from the
#                           panel's edge three times, in six frames, with the highlight behind
#   09-highlight            the bounce is over, and the highlight stays until the window is
#                           focused
#   NN-name-strip.png       each sequence's dock strips one above the next, to read as frames
#
# report.txt has the tests that ran: the token the launch carries (both variables) and what
# Caminito does with it, the pulse stopping at the first window and at the timeout, a new
# instance's pulse, the bounce and the highlight, and the highlight clearing when the window
# is focused from the dock or otherwise. The feedback's own logic, with the clock passed in, is
# `meson test` in the build: test-feedback.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs Meson,
# libxfce4windowing with Wayland, gtk-layer-shell, mate-panel's applet library, a C compiler,
# Cargo and python3.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-221.XXXXXX)
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
  echo "== the feedback's logic, with the clock passed in"
  meson test -C "$build" feedback 2>&1 | grep -E '^ *[0-9]+/[0-9]+|^(Ok|Fail):' || true
  echo "== the feedback against a headless Caminito"
  mkdir -p "$work/shots"
  (cd "$root" && CAMINITO_DOCK_PROBE=$probe CAMINITO_DOCK_SHOTS=$work/shots \
    cargo test -p caminito --lib tests::dock_applet -- --ignored --test-threads=1 2>&1 |
    grep -E '^test |^test result: .*(passed|failed)') || true
  python3 -I "$here/ppm2png.py" "$work"/shots/*.ppm
  python3 -I "$here/strips.py" "$work/shots" 48 400
  cp "$work"/shots/*.png "$out/"
} | tee "$out/report.txt"
echo "pictures in $out"
