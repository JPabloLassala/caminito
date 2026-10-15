#!/usr/bin/env bash
# Ticket 259's check: mate-panel with the Bluetooth applet on X11, against python-dbusmock's
# BlueZ on a private bus in the system bus's place. It screenshots the top-left corner of
# the panel in each state, once per icon theme it finds (mate, Humanity, Adwaita, breeze):
#
#   1-no-adapter   the template's BlueZ before an adapter: no icon
#   2-on           an adapter
#   3-off          the adapter powered off, as an rfkill block leaves it
#   4-connected    two devices connected, one with a battery, and the tooltip
#   5-menu         the right-click menu
#   5-off-from-menu, 5-on-from-menu
#                  its first item twice, where /dev/rfkill isn't writable (see session.sh)
#   6-restarted    BlueZ stopped and started again, with an adapter
#
# and writes report.txt: whether the panel was still running after each step.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), xdotool, ImageMagick and
# dconf. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-259.XXXXXX)
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
mkdir -p "$work/applets" "$out"
rm -f "$out"/*.png "$out/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-bluetooth-applet/org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet"

xvfb=
if [ -z "${DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 1024x600x24 >/dev/null 2>&1 &
  xvfb=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
fi

for theme in mate Humanity Adwaita breeze; do
  [ -f "/usr/share/icons/$theme/index.theme" ] || continue
  echo "== $theme" >>"$out/report.txt"
  mkdir -p "$work/config-$theme/gtk-3.0"
  printf '[Settings]\ngtk-icon-theme-name=%s\n' "$theme" >"$work/config-$theme/gtk-3.0/settings.ini"
  XDG_CONFIG_HOME=$work/config-$theme MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 \
    CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} timeout 120 dbus-run-session -- \
    bash "$here/session.sh" "$root" "$work" "$out" "$theme" >>"$out/report.txt" 2>&1 || true
done
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
