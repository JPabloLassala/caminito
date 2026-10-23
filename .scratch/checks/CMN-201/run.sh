#!/usr/bin/env bash
# CMN-201's check of the Network popup's Details tab: mate-panel on X11 with the applet alone
# on it, against python-dbusmock's networkmanager template (bed.py) on a private bus that
# stands for the system bus. The first shot clicks Details, which the popup keeps after:
#
#   01-wifi              "Home": wlan0, its address, gateway, two DNS, MAC…
#   02-wifi-end          the tab scrolled: 866 Mb/s, 5 GHz, channel 36, WPA2, 70%
#   03-wired-vpn         "Office": eth0, IPv4 and two IPv6, gateway, DNS…
#   04-wired-vpn-end     the tab scrolled: Office's MAC and 1 Gb/s, then "Work VPN" with its
#                        address and DNS only
#   05-nothing           "No active connections"
#   06-address-before    "Wired connection 1" at 192.168.1.20/24
#   07-address-after     the same popup, never closed, at 192.168.1.77/24 once the mock moved it
#
# It writes report.txt: whether the panel lived through each state, and its log's warnings.
# What NetworkManager's values become is test-nm's (details-* tests), and how the tab draws
# them test-popup's.
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
work=$(mktemp -d -t caminito-cmn-201.XXXXXX)
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
