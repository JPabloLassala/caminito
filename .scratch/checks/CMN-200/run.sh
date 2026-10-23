#!/usr/bin/env bash
# CMN-200's check of the Network popup's Networks tab: mate-panel on X11 with the applet
# alone on it, against python-dbusmock's networkmanager template (bed.py) on a private bus
# that stands for the system bus. Each shot is of the popup, opened by a click on the applet:
#
#   01-order           the list without Home, the card's: the VPNs, then Library and Dock,
#                      saved, then eduroam, Cafe and Old by signal; no hidden network
#   02-search          "lib" typed in the search: Library alone
#   03-new-secured     Old, WEP, selected: a password field, typed in, and "Join"
#   04-saved           Library selected: "Join Library"
#   05-enterprise      eduroam selected: "Join eduroam…"
#   06-connecting      Library coming up, selected: a spinner and "Connecting to Library…"
#   07-wifi-off        Wi-Fi off: the VPNs and Dock, and no "Connect to hidden network…"
#
# Between 05 and 06 it clicks "Join eduroam…", then "Connect to hidden network…", which run
# a fake nm-connection-editor that records its arguments. It writes report.txt: whether the
# panel lived through each state, the editor's commands, and the panel log's warnings. What
# a join does to NetworkManager is test-nm's, and what the bar calls back test-popup's.
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
work=$(mktemp -d -t caminito-cmn-200.XXXXXX)
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
mkdir -p "$work/applets" "$work/bin" "$out"
# The editor, as the applet runs it.
printf '#!/bin/sh\necho "nm-connection-editor $*" >>"%s/editor.log"\n' "$work" \
  >"$work/bin/nm-connection-editor"
chmod +x "$work/bin/nm-connection-editor"
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
    Xvfb ":$n" -screen 0 480x800x24 >/dev/null 2>&1 &
  else
    # A window on the Wayland session, which goes when the check ends.
    Xwayland ":$n" -geometry 480x800 >/dev/null 2>&1 &
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
  GDK_BACKEND=x11 WAYLAND_DISPLAY= PATH=$work/bin:$PATH \
  timeout 180 dbus-run-session -- bash "$here/session.sh" "$here" "$work" "$out" "$python" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/report.txt" || true
[ -n "$server" ] && kill "$server"
cat "$out/report.txt"
echo "screenshots in $out"
