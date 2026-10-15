#!/usr/bin/env bash
# CMN-106's check of the Power applet's icon: mate-panel with the applet on X11, against
# python-dbusmock's upower template on a private bus in the system bus's place. It screenshots
# the whole (small) screen, the pointer on the applet so that the tooltip shows:
#
#   1-no-battery      the display device with no battery: a plug, "Plugged in"
#   2-battery         78%, discharging, 3 h 40 min to empty: "78%" and "3 h 40 min left"
#   3-charging        42%, charging, 50 min to full: the charging glyph, "Full in 50 min"
#   4-full            100%, fully charged: "Plugged in"
#   5-low             8%, discharging: the glyph for the lowest level
#   6-percentage      "Show percentage" on (through dconf, the key the applet's
#                     Preferences writes), 42% and charging: the number beside the glyph
#   7-percentage-off  and off again
#   8-popup           left-click: the popup, empty but for "Configure Power Management…"
#   9-menu            right-click: Preferences and "Configure Power Management…"
#   10-preferences    Preferences: a window with a "Show percentage" check button
#   11-upower-gone    upowerd stopped: back to a plug
#   12-upower-back    upowerd started again and set up: the battery again
#
# and writes report.txt: the programs the popup's and the menu's "Configure Power
# Management…" ran (a stub on PATH records them), the pixels that differ between the shots
# with the number on and off (0 would be a failure), and whether the panel lived.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), xdotool, ImageMagick, dconf and
# glib-compile-schemas. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-106.XXXXXX)
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
  Xvfb ":$n" -screen 0 360x240x24 >/dev/null 2>&1 &
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
