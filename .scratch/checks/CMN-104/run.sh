#!/usr/bin/env bash
# CMN-104's check of the Power popup's table of batteries: mate-panel with the applet on X11,
# against python-dbusmock's upower template on a private bus in the system bus's place. The
# popup is open, and every shot is of the whole (small) screen:
#
#   1-one-battery      one laptop battery, 73%, with a charge limit it supports: icon, name and
#                      state, bar, percentage and the ⋮
#   2-menu             the ⋮ pressed: "Health 58%" and the switch "Limit charging to 80%"
#   3-limit-on         the switch turned on: EnableChargeThreshold(true) was called, and the
#                      state says "· limit 80%"
#   4-limit-off        and off again
#   5-refused          polkit's refusal: the switch is where it was, and the menu says why
#   6-two-batteries    a second battery, that has no limit, in the table: "Battery 1" and
#                      "Battery 2", and the menu of the second has no switch
#   7-peripherals      no battery at all, a mouse and headphones: the table holds only them
#   8-low-peripheral   a battery and a mouse at 12%: its percentage in the warning colour
#   9-mouse-off        the mouse turned off: gone from the table
#
# and writes report.txt: what the mock's EnableChargeThreshold was called with, the rows each
# shot has, and whether the panel lived.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), xdotool, ImageMagick, dconf and
# glib-compile-schemas. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-104.XXXXXX)
trap 'rm -rf "$work"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
module=$(find "$(realpath "$build")" -name libcaminito-power-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-power-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$work/bin" "$work/schemas" "$out"
rm -f "$out"/*.png "$out/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-power-applet/org.mate.panel.CaminitoPowerApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoPowerApplet.mate-panel-applet"
# The preference's schema, which the package's hook compiles in a real install.
cp "$root/helpers/caminito-power-applet/org.mate.panel.applet.caminito-power.gschema.xml" \
  "$work/schemas/"
glib-compile-schemas "$work/schemas"
# The program the popup and the menu run, which there may be none of here: it records itself.
cat >"$work/bin/mate-power-preferences" <<STUB
#!/bin/sh
echo "mate-power-preferences \$*" >>"$work/spawned"
STUB
chmod +x "$work/bin/mate-power-preferences"

xvfb=
if [ -z "${DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 480x420x24 >/dev/null 2>&1 &
  xvfb=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
fi

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
  >"$work/config/gtk-3.0/settings.ini"
: >"$work/spawned"
PATH=$work/bin:$PATH XDG_CONFIG_HOME=$work/config MATE_PANEL_APPLETS_DIR=$work/applets \
  GSETTINGS_SCHEMA_DIR=$work/schemas NO_AT_BRIDGE=1 CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} \
  timeout 180 dbus-run-session -- bash "$here/session.sh" "$root" "$work" "$out" \
  2>&1 | grep -v '^dbus-daemon\[' >"$out/report.txt" || true
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
