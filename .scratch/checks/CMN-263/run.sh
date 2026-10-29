#!/usr/bin/env bash
# CMN-263's check of Do Not Disturb in the Notifications applet: mate-panel on X11 with the
# applet alone on it, on a session bus of its own, against a mock of caminito-notifications's
# private API (helpers/caminito-notifications-applet/notifications-mock.py) that session.sh
# starts. Each shot is of the panel's corner, at twice its size:
#
#   01-off             Do Not Disturb off: the bell, with its dot for the one entry
#   02-center-off      left-click: the Do Not Disturb row on top, its switch off, and the
#                      durations "1 hour", "4 hours" and "Tomorrow morning" under it
#   03-on-for-an-hour  "1 hour" clicked: the bell struck through
#   04-center-on       the center again: the switch on, "Until HH:MM" an hour from now, and
#                      no durations
#   05-middle-off      middle-click on the applet: the bell again
#   06-middle-on       middle-click again: struck through, on until turned off
#   07-center-until    the center: "Until turned off"
#   08-open-center     the center closed, then OpenCenter called as Caminito does for
#                      Super+N: the center opens
#
# and writes report.txt: whether the panel lived through each state, the calls the mock took,
# and the panel's log's warnings.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, python-xlib, ImageMagick and dconf, and Xvfb or, without it, a
# Wayland session for a rootful Xwayland. CAMINITO_DISPLAY names an X display to use instead.
# CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-263.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
module=$(find "$(realpath "$build")" -name libcaminito-notifications-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-notifications-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$out"
rm -f "${out:?}"/*.png "${out:?}/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-notifications-applet/org.mate.panel.CaminitoNotificationsApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoNotificationsApplet.mate-panel-applet"

# A display of its own: Xvfb, or a rootful Xwayland's window in the Wayland session.
xserver=
if [ -z "${CAMINITO_DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  if command -v Xvfb >/dev/null; then
    Xvfb ":$n" -screen 0 640x640x24 >/dev/null 2>&1 &
  else
    Xwayland ":$n" -geometry 640x640 >/dev/null 2>&1 &
  fi
  xserver=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
else
  export DISPLAY=$CAMINITO_DISPLAY
fi

# The bell's last fallback, as the package installs it.
mkdir -p "$work/config/gtk-3.0" "$work/share/icons/hicolor/scalable/status"
cp "$root/helpers/caminito-notifications-applet/caminito-bell-symbolic.svg" \
  "$work/share/icons/hicolor/scalable/status/"
printf '[Settings]\ngtk-theme-name=%s\ngtk-icon-theme-name=%s\n' \
  "${CAMINITO_GTK_THEME:-Adwaita}" "${CAMINITO_ICON_THEME:-mate}" \
  >"$work/config/gtk-3.0/settings.ini"
# X11 even when run from a Wayland session, which GTK would take first, and settings of its
# own, so that nothing of the user's panel is touched.
XDG_CONFIG_HOME=$work/config MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 \
  GDK_BACKEND=x11 WAYLAND_DISPLAY= XDG_DATA_DIRS=$work/share:${XDG_DATA_DIRS:-/usr/share} \
  CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} \
  timeout 120 dbus-run-session -- bash "$here/session.sh" "$root" "$work" "$out" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/report.txt" || true
[ -n "$xserver" ] && kill "$xserver"
cat "$out/report.txt"
echo "screenshots in $out"
