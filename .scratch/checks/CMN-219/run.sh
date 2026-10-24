#!/usr/bin/env bash
# CMN-219's check of the Dock applet, in two halves.
#
# The first runs the applet's items and clicks against a headless Caminito, with
# libxfce4windowing reading the windows, as tools/check does: dk-probe, the dock in a layer
# surface of its own, is clicked with the pointer the way a user would, and the scenarios that
# mean to show something leave a picture of the output, turned into a PNG in the output
# directory:
#
#   01-kept-items               a kept program that is not running, after the click that
#                               launched it
#   02-running-items            a window with no desktop entry (two windows, one item) and a
#                               terminal, which has the focus: the indicator not highlighted
#   03-focused-indicator        the same after a click on the first item: its indicator is
#                               highlighted, the terminal's isn't
#   04-context-menu             the terminal's menu: its two windows by title, the desktop
#                               entry's action, Keep in Dock, Close all windows
#   05-vertical-dock            a dock along the left edge, in a column with the indicator on
#                               the panel's edge: a kept Files, not running, and a terminal
#                               with two windows
#   06-two-processes-one-item   the two windows of two processes of one program under one
#                               item, and Files' under its own
#
# An item the theme has no icon for, or whose image can't be loaded, is a rounded square with
# the program's first letter.
#
# The second starts mate-panel on X11 with the real applet alone on its panel, on an Xvfb of
# its own, and reports that the panel lives and what the instance's settings keep.
#
# report.txt has the tests that ran and the panel's answer: the criteria's, the app id to item
# resolution (its own tests, without a panel, are in the C build's `meson test`), keeping,
# removing and reordering across a restart, each click case, and the minimise animation.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs Meson,
# libxfce4windowing, gtk-layer-shell, mate-panel's applet library, a C compiler, Cargo and
# python3, and for the second half mate-panel, Xvfb, dconf and dbus-run-session.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-219.XXXXXX)
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
module=$(find "$build" -name libcaminito-dock-applet.so | head -1)
[ -n "$probe" ] && [ -n "$module" ] || { echo "no dk-probe or applet in $build" >&2; exit 1; }
mkdir -p "$out"
rm -f "${out:?}"/*.png "${out:?}"/*.ppm "${out:?}/report.txt"

{
  echo "== the items and clicks against a headless Caminito"
  mkdir -p "$work/shots"
  (cd "$root" && CAMINITO_DOCK_PROBE=$probe CAMINITO_DOCK_SHOTS=$work/shots \
    cargo test -p caminito tests::dock_applet -- --ignored --test-threads=1 2>&1 |
    grep -E '^test |^test result: .*(passed|failed)') || true
  python3 -I "$here/ppm2png.py" "$work"/shots/*.ppm
  cp "$work"/shots/*.png "$out/"

  echo "== the applet in mate-panel on X11"
  mkdir -p "$work/applets" "$work/run" "$work/config" "$work/schemas"
  chmod 700 "$work/run"
  glib-compile-schemas --targetdir="$work/schemas" "$root/helpers/caminito-dock-applet"
  # The stock applets too: the variable replaces the panel's own directory.
  cp /usr/share/mate-panel/applets/* "$work/applets/"
  sed "s#@APPLET_LOCATION@#$module#" \
    "$root/helpers/caminito-dock-applet/org.mate.panel.CaminitoDockApplet.mate-panel-applet.in" \
    >"$work/applets/org.mate.panel.CaminitoDockApplet.mate-panel-applet"
  n=90
  while [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
  Xvfb ":$n" -screen 0 800x300x24 >/dev/null 2>&1 &
  xserver=$!
  sleep 2
  DISPLAY=:$n XDG_CONFIG_HOME=$work/config XDG_RUNTIME_DIR=$work/run \
    MATE_PANEL_APPLETS_DIR=$work/applets GSETTINGS_SCHEMA_DIR=$work/schemas \
    NO_AT_BRIDGE=1 GDK_BACKEND=x11 WAYLAND_DISPLAY= timeout 120 \
    dbus-run-session -- bash "$here/session.sh" "$work" 2>&1 |
    grep -vE '^(dbus-daemon\[|fusermount)' || true
  kill "$xserver" 2>/dev/null
  wait "$xserver" 2>/dev/null || true
} | tee "$out/report.txt"
echo "pictures in $out"
