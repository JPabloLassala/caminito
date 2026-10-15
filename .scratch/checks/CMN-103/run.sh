#!/usr/bin/env bash
# CMN-103's check of the Bluetooth applet's popup: mate-panel with the applet on X11,
# against python-dbusmock's BlueZ on a private bus in the system bus's place. It runs the
# panel twice, on the top edge and on the bottom one, and screenshots the whole (small)
# screen:
#
#   <edge>-1-closed       the panel with no popup, which later shots are compared against
#   <edge>-2-connected    left-click: WH-1000XM4 connected (80%), MX Master 3 (12%, in red)
#                         and Pixel 7 paired and not, and a nearby Galaxy Buds2 that isn't
#                         paired, left out
#   <edge>-3-expanded     MX Master 3's row open: its type and address, and Connect
#   <edge>-4-connected    its Connect pressed: it moves up to Connected, still open
#   <edge>-5-disconnected WH-1000XM4's row open and its Disconnect pressed: it moves down
#   <edge>-6-off          the adapter off from outside, the popup open: the line saying so,
#                         in place of the list, and the window back to its short height
#   <edge>-7-after-add    "Add New Device…" pressed, after the adapter is on again: the
#                         popup closes and `caminito-bluetooth --scan` is run (a stub on
#                         PATH records it)
#
# and writes report.txt: the mock's Connected after each press, the height of the popup
# when a row closes, the gear's program run, and whether the popup closed on Escape, on a
# click on the desktop and on a second click on the applet, as the pixels that differ from
# the closed panel's (0 when it's gone).
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), xdotool, ImageMagick and
# dconf. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-103.XXXXXX)
trap 'rm -rf "$work"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
module=$(find "$(realpath "$build")" -name libcaminito-bluetooth-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-bluetooth-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$work/bin" "$out"
rm -f "$out"/*.png "$out/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-bluetooth-applet/org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet"
# The program the popup runs, which there's none of here: it records its arguments.
cat >"$work/bin/caminito-bluetooth" <<STUB
#!/bin/sh
echo "caminito-bluetooth \$*" >>"$work/spawned"
STUB
chmod +x "$work/bin/caminito-bluetooth"

xvfb=
if [ -z "${DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 520x520x24 >/dev/null 2>&1 &
  xvfb=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
fi

for edge in top bottom; do
  echo "== $edge" >>"$out/report.txt"
  : >"$work/spawned"
  mkdir -p "$work/config-$edge/gtk-3.0"
  printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
    >"$work/config-$edge/gtk-3.0/settings.ini"
  PATH=$work/bin:$PATH XDG_CONFIG_HOME=$work/config-$edge MATE_PANEL_APPLETS_DIR=$work/applets \
    NO_AT_BRIDGE=1 CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} timeout 180 \
    dbus-run-session -- bash "$here/session.sh" "$root" "$work" "$out" "$edge" \
    >>"$out/report.txt" 2>&1 || true
done
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
