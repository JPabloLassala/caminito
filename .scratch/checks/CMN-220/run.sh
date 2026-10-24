#!/usr/bin/env bash
# CMN-220's check of the Dock applet's Trash, in three parts.
#
# The first is the Trash's own tests, without a panel (meson test): whether it is full as its
# folder changes, putting files in it, emptying it with folders and their contents.
#
# The second runs the Trash against a headless Caminito, with gvfs's trash daemon on a bus of
# each test's own, as tools/check does: dk-probe, the dock in a layer surface of its own, is
# clicked with the pointer the way a user would, and the scenarios that mean to show something
# leave a picture of the output, turned into a PNG in the output directory:
#
#   07-trash-empty          the Trash after a separator at the end of the row, empty
#   08-trash-full           the same after a file was dropped on it: its icon is full
#   09-trash-menu           its menu, Open and Empty Trash, on a full Trash
#   10-trash-confirmation   the question Empty Trash asks first, as Caja does
#
# The tests behind them also check that a dropped file is moved to the Trash and the icon turns
# full, that emptying confirmed empties it and cancelled leaves it, that a click opens it in
# Caja, and that the Trash is at the end of the row in every orientation.
#
# The third starts mate-panel on X11 with the real applet alone on its panel, on an Xvfb of its
# own, and reports that the panel lives with the Trash in the applet.
#
# report.txt has the tests that ran and the panel's answer.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs what
# CMN-219's check needs, and gvfs for the second part, and for the third mate-panel, Xvfb, dconf
# and dbus-run-session.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-220.XXXXXX)
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
  echo "== the Trash's own tests, without a panel"
  meson test -C "$build" trash --print-errorlogs 2>&1 | grep -E '^ *[0-9]+/[0-9]+|^(Ok|Fail|Expected)'

  echo "== the Trash against a headless Caminito, over gvfs"
  mkdir -p "$work/shots"
  # All the Dock's tests: the Trash's have "trash" in their names, and the pictures are theirs.
  (cd "$root" && CAMINITO_DOCK_PROBE=$probe CAMINITO_DOCK_SHOTS=$work/shots \
    cargo test -p caminito tests::dock_applet -- --ignored --test-threads=1 2>&1 |
    grep -E '^test |^test result: .*(passed|failed)') || true
  # Only the Trash's pictures are this ticket's.
  for shot in "$work"/shots/0[789]-*.ppm "$work"/shots/10-*.ppm; do
    python3 -I "$here/../CMN-219/ppm2png.py" "$shot"
    cp "${shot%.ppm}.png" "$out/"
  done

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
