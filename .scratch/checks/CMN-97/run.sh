#!/usr/bin/env bash
# CMN-97's check of the Thermal popup: mate-panel on X11 with the applet alone on it, against
# CMN-99's fake /sys/class/hwmon (../CMN-99/bed.py), which umockdev makes and libsensors reads.
# Each shot is of the popup alone:
#
#   ThinkPad (CPU Package and cores, one NVMe drive, Wi-Fi, the EC, one fan with no max):
#   t1-collapsed       the CPU, Storage and Fan tiles, Show all sensors closed
#   t2-expanded        Show all open, a few reads in: the fan has no bar yet
#   t3-fan-bar         more than 10 reads later: the fan's bar, against its fastest
#   t4-reopened        closed and opened again: Show all is still open, in the popup's state
#                      only until 269 keeps it as a preference
#   Desktop (k10temp, amdgpu with a stopped fan with a max, two NVMe drives, two fans):
#   d1-collapsed       the four tiles
#   d2-expanded        Show all open: the GPU fan "Stopped" with an empty bar
#   d3-warm-hot        the CPU at 80 °C, warm; the GPU's junction at 105 °C, hot, in its row
#                      and its tile; updated in place with the popup open
#
# and writes report.txt: whether the panel lived through each step, and the popup's size.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, umockdev and its Python bindings, lm_sensors, Xvfb (unless DISPLAY is set),
# ImageMagick and dconf.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-97.XXXXXX)
trap 'rm -rf "$work"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
module=$(find "$(realpath "$build")" -name libcaminito-thermal-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-thermal-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$out"
rm -f "$out"/*.png "$out/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-thermal-applet/org.mate.panel.CaminitoThermalApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoThermalApplet.mate-panel-applet"
# The applet's own thermometer, which the package installs in hicolor.
mkdir -p "$work/share/icons/hicolor/scalable/status"
cp "$root/helpers/caminito-thermal-applet/caminito-thermometer-symbolic.svg" \
  "$work/share/icons/hicolor/scalable/status/"

xvfb=
if [ -z "${DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 480x900x24 >/dev/null 2>&1 &
  xvfb=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
fi

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
  >"$work/config/gtk-3.0/settings.ini"
for computer in thinkpad desktop; do
  XDG_CONFIG_HOME=$work/config XDG_DATA_DIRS=$work/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share} \
    MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 \
    timeout 180 dbus-run-session -- \
    umockdev-wrapper python3 "$here/../CMN-99/bed.py" "$computer" \
    bash "$here/session.sh" "$computer" "$work" "$out" \
    2>&1 | grep -v '^dbus-daemon\[' >>"$out/report.txt" || true
done
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
