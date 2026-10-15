#!/usr/bin/env bash
# CMN-101's check of Stay awake and the list of what keeps the computer awake: mate-panel
# with the Power applet on X11, against a mock of mate-session's inhibitor calls
# (helpers/caminito-power-applet/session-mock.py) on the session bus. It screenshots the
# whole (small) screen:
#
#   1-off          the popup with Stay awake off and nothing keeping the computer awake: no
#                  separator, no list
#   2-on           the switch pressed: the switch on, and one line below the separator, "You",
#                  "Stay awake is on"
#   3-application  an application's inhibitor (Firefox, with its desktop file, "Playing
#                  video") and one with no desktop file (its app ID stands for the name),
#                  next to the Stay awake line; a logout-only inhibitor isn't listed
#   4-removed      the applications' inhibitors removed: the Stay awake line alone
#   5-off-again    the switch pressed again: the popup is back to 1-off
#   6-restarted    Stay awake on, the panel killed and started again: the switch is off
#
# and writes report.txt: what the mock was called with (Inhibit with flags 12 and then
# Uninhibit with the same cookie), what mate-session lists at steps 3 and 4 (the popup's lines
# are in the screenshots), and whether the panel lived.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), xdotool, ImageMagick, dconf and
# glib-compile-schemas. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-101.XXXXXX)
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
mkdir -p "$work/applets" "$work/schemas" "$work/share/applications" "$out"
rm -f "$out"/*.png "$out/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-power-applet/org.mate.panel.CaminitoPowerApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoPowerApplet.mate-panel-applet"
cp "$root/helpers/caminito-power-applet/org.mate.panel.applet.caminito-power.gschema.xml" \
  "$work/schemas/"
glib-compile-schemas "$work/schemas"
# An application that has a desktop file, which gives the popup its name and icon.
cat >"$work/share/applications/firefox.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Firefox
Exec=true
Icon=applications-internet
DESKTOP

xvfb=
if [ -z "${DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 400x400x24 >/dev/null 2>&1 &
  xvfb=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
fi

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
  >"$work/config/gtk-3.0/settings.ini"
XDG_DATA_DIRS=$work/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share} \
  XDG_CONFIG_HOME=$work/config MATE_PANEL_APPLETS_DIR=$work/applets \
  GSETTINGS_SCHEMA_DIR=$work/schemas NO_AT_BRIDGE=1 CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} \
  timeout 240 dbus-run-session -- bash "$here/session.sh" "$root" "$work" "$out" \
  2>&1 | grep -v '^dbus-daemon\[' >"$out/report.txt" || true
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
