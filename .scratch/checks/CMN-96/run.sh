#!/usr/bin/env bash
# CMN-96's check of the panel sensors and the Thermal applet's preferences: mate-panel on X11
# with the applet alone on it, against CMN-99's fake desktop (../CMN-99/bed.py), which
# umockdev makes and libsensors reads, and the applet's schema compiled for the run. Each
# step has a shot of the panel's start (p*) and, with the popup open, of the popup (q*):
#
#   p1-defaults        the CPU's and the GPU junction's, 58°C / 61°C
#   p2, q2-pin-drive   the WD_BLACK drive's pin clicked: 61°C / 46°C, the CPU's pin gone
#   p3, q3-pin-systin  SYSTIN's pin: 46°C / 34°C
#   p4, q4-fahrenheit  units 'fahrenheit': 115°F / 93°F, the popup in °F too
#   p5, q5-warm-40     warm-threshold 40: the drive's 115°F in orange, in the popup too
#   p6, q6-restarted   mate-panel again: the drive and SYSTIN in °F, Show all open
#   p7-systin-gone     nct6798 removed, SYSTIN's pin's chip: the defaults, 136°F / 142°F, with
#                      no crash
#
# with panels.png, the p* shots one over the other, and writes report.txt: whether the panel lived through each step, the panel-sensors and
# show-all keys after the clicks and the restart, and the panel's warnings.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, umockdev and its Python bindings, lm_sensors, Xvfb (unless DISPLAY is set),
# ImageMagick, dconf and glib-compile-schemas.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-96.XXXXXX)
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

# The applet's preferences, which the package installs and compiles, with the system's: the
# variable replaces their directory.
mkdir -p "$work/schemas"
cp /usr/share/glib-2.0/schemas/*.xml /usr/share/glib-2.0/schemas/*.override "$work/schemas/" \
  2>/dev/null || true
cp "$root/helpers/caminito-thermal-applet/org.mate.panel.applet.caminito-thermal.gschema.xml" \
  "$work/schemas/"
glib-compile-schemas "$work/schemas"

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
  >"$work/config/gtk-3.0/settings.ini"
for computer in desktop; do
  XDG_CONFIG_HOME=$work/config XDG_DATA_DIRS=$work/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share} \
    MATE_PANEL_APPLETS_DIR=$work/applets GSETTINGS_SCHEMA_DIR=$work/schemas NO_AT_BRIDGE=1 \
    timeout 240 dbus-run-session -- \
    umockdev-wrapper python3 "$here/../CMN-99/bed.py" "$computer" \
    bash "$here/session.sh" "$computer" "$work" "$out" \
    2>&1 | grep -v '^dbus-daemon\[' >>"$out/report.txt" || true
done
[ -n "$xvfb" ] && kill "$xvfb"
# The panel's shots one over the other, for a look at them all at once.
magick "$out"/p*.png -background gray -splice 0x4 -append "$out/panels.png"
cat "$out/report.txt"
echo "screenshots in $out"
