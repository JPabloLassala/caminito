#!/usr/bin/env bash
# CMN-196's check of the Network popup's tiles and card: mate-panel on X11 with the applet
# alone on it, against python-dbusmock's networkmanager template (bed.py) on a private bus
# that stands for the system bus. Each shot is of the popup, opened by a click on the applet:
#
#   01-no-networkmanager   the icon and "NetworkManager isn't running"
#   02-wifi-on             Wi-Fi "Home", Wired "Unplugged" and dim; the card "Home", its address
#   03-wifi-off            Wi-Fi "Off"; the card "Not connected"
#   04-cable-in            Wired "Connected"; the card "Wired connection 1", "Connected · 1 Gb/s"
#   05-cable-out           Wired "Unplugged" and dim; Wi-Fi "On"; "Not connected"
#   06-vpn                 VPN "Work VPN" on; the card "Office"
#   07-airplane            Airplane mode on, Wi-Fi dim and "Off"
#   08-limited             the card "Connected · No internet" with the -no-route icon
#   09-connecting          the card "Connecting to Home…" with a spinner
#   10-nothing             Wired "Disconnected", Wi-Fi "On", VPN "Off"; "Not connected"
#
# with the Networks and Details tabs under the card, empty until CMN-200 and CMN-201, and
# "Configure network connections…" at the bottom. It writes report.txt: whether the panel
# lived through each state, and its log's warnings. What the tiles' clicks do to
# NetworkManager is test-nm's (the-*-tile-* tests), and that a click asks for it test-popup's.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, python-dbusmock, ImageMagick, dconf, python3 with python-xlib for the clicks,
# and an X server of its own: Xvfb, or a rootful Xwayland window under Wayland, unless
# DISPLAY is set, or CAMINITO_OWN_X is, to keep off that display. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-196.XXXXXX)
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

server=
if [ -z "${DISPLAY:-}" ] || [ -n "${CAMINITO_OWN_X:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  if command -v Xvfb >/dev/null; then
    Xvfb ":$n" -screen 0 480x680x24 >/dev/null 2>&1 &
  else
    # A window on the Wayland session, which goes when the check ends.
    Xwayland ":$n" -geometry 480x680 >/dev/null 2>&1 &
  fi
  server=$!
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
[ -n "$server" ] && kill "$server"
cat "$out/report.txt"
echo "screenshots in $out"
