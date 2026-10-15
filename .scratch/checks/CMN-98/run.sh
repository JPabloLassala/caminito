#!/usr/bin/env bash
# CMN-98's check of the Power popup's power profile section: mate-panel with the applet on
# X11, against python-dbusmock's power-profiles-daemon templates on a private bus in the
# system bus's place. It opens the popup and screenshots the whole (small) screen:
#
#   1-three-profiles     the daemon lists three: Power Saver, Balanced (pressed), Performance
#   2-clicked            a click on Performance: the mock's ActiveProfile is performance
#   3-outside            ActiveProfile set from outside (powerprofilesctl): the pressed tile moves
#   4-degraded-hot       PerformanceDegraded is high-operating-temperature: a line in orange
#   5-degraded-lap       and lap-detected
#   6-held               GameMode holds Performance: a dimmed line (also with the degraded one)
#   7-two-profiles       the daemon lists two, no Performance: two tiles, and no line
#   8-no-daemon          the daemon stopped: the section is gone, "Configure Power Management…" stays
#   9-daemon-back        the daemon started again: the tiles are back
#   10-old-name          only net.hadess.PowerProfiles on the bus: the same tiles
#
# and writes report.txt: what the mock's ActiveProfile was after each click or change, the
# pixels that differ between shots that must differ, and whether the panel lived.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), xdotool, ImageMagick, dconf and
# glib-compile-schemas. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-98.XXXXXX)
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
  Xvfb ":$n" -screen 0 400x300x24 >/dev/null 2>&1 &
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
