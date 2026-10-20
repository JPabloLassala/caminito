#!/usr/bin/env bash
# CMN-99's check of the Thermal applet's panel item: mate-panel on X11 with the applet alone on
# it, against a fake /sys/class/hwmon that umockdev makes (bed.py) and libsensors reads. Each
# shot is of the panel's corner with the applet, at twice its size:
#
#   ThinkPad (one fan with no max, no GPU, a reading that can't be read, six at 0):
#   t1-32px            "55°C" over "2865 RPM", the dead readings ignored
#   t2-24px            one line: "55°C  2865 RPM"
#   t3-vertical        a vertical panel: the thermometer, and "55°" under it
#   Desktop (k10temp, amdgpu, two NVMe drives, three fans):
#   d1-32px            "58°C / 61°C" over "1180 RPM", the fastest fan
#   d2-24px            one line
#   d3-warm-hot        the CPU at 80 °C in orange, the GPU's junction at 105 °C (its crit 110
#                      minus 5) in red
#   d4-fans-off        every fan at 0: "Fans Off"
#   d5-gpu-gone        amdgpu's chip removed, as when its module is unloaded: the CPU alone
#
# and writes report.txt: what the stock sensors command reads of each tree, and whether the
# panel lived through each step.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, umockdev and its Python bindings, lm_sensors, Xvfb (unless DISPLAY is set),
# ImageMagick and dconf.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-99.XXXXXX)
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
  Xvfb ":$n" -screen 0 360x240x24 >/dev/null 2>&1 &
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
    umockdev-wrapper python3 "$here/bed.py" "$computer" \
    bash "$here/session.sh" "$computer" "$work" "$out" \
    2>&1 | grep -v '^dbus-daemon\[' >>"$out/report.txt" || true
done
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
