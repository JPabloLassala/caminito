#!/usr/bin/env bash
# CMN-193's check of the Network applet's icon: mate-panel on X11 with the applet alone on it,
# against python-dbusmock's networkmanager template (bed.py) on a private bus that stands for
# the system bus. Each shot is of the panel's corner, at twice its size, with the tooltip under
# the icon where xdotool is installed to hover it:
#
#   01-no-networkmanager   network-offline, "NetworkManager isn't running"
#   02-wired               network-wired, "Wired connection 1"
#   03-wifi-20 … 06-wifi-95  the signal's four steps, "Home · 20%" …
#   07-connecting          the Wi-Fi -acquiring icon, "Connecting to Home…"
#   08-limited             the Wi-Fi -no-route icon, "Home · 70% · no internet"
#   09-vpn                 wired with the lock over it, "Office" over "VPN: Work"
#   10-airplane            airplane-mode, "Airplane mode"
#   11-nothing             network-offline, "Not connected"
#   12-stopped-again       network-offline, "NetworkManager isn't running"
#
# and writes report.txt: whether the panel lived through each state, and its log's warnings.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, Xvfb (unless DISPLAY is set), ImageMagick and dconf, and
# xdotool for the tooltips. CAMINITO_PYTHON names a Python other than python3 that has
# dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-193.XXXXXX)
trap 'rm -rf "$work"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
python=${CAMINITO_PYTHON:-python3}
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
module=$(find "$(realpath "$build")" -name libcaminito-network-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-network-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$out"
rm -f "$out"/*.png "$out/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-network-applet/org.mate.panel.CaminitoNetworkApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoNetworkApplet.mate-panel-applet"

xvfb=
if [ -z "${DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 480x240x24 >/dev/null 2>&1 &
  xvfb=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
fi

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\ngtk-icon-theme-name=%s\n' \
  "${CAMINITO_GTK_THEME:-Adwaita}" "${CAMINITO_ICON_THEME:-mate}" \
  >"$work/config/gtk-3.0/settings.ini"
# X11 even when run from a Wayland session, which GTK would take first.
XDG_CONFIG_HOME=$work/config MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 \
  GDK_BACKEND=x11 WAYLAND_DISPLAY= \
  timeout 180 dbus-run-session -- bash "$here/session.sh" "$here" "$work" "$out" "$python" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/report.txt" || true
[ -n "$xvfb" ] && kill "$xvfb"
cat "$out/report.txt"
echo "screenshots in $out"
