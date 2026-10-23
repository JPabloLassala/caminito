#!/usr/bin/env bash
# CMN-202's by-eye check of the default panel: a fresh user's mate-panel, the patched one
# installed on the machine (mate-panel-caminito), gets the Network applet in its top panel
# between the notification area and the Bluetooth applet, from the files `meson install` puts
# in the package. It is CMN-100's panel.sh with NetworkManager's mock beside BlueZ's and
# UPower's.
#
#   panel.sh [<output directory>]
#
# It starts the panel on a keyfile GSettings backend with no settings (a fresh user), against
# python-dbusmock's BlueZ with one adapter, its UPower with a battery at 78%, and its
# NetworkManager on Wi-Fi (CMN-201's bed.py), on a private bus in the system bus's place, and
# screenshots it:
#
#   fresh-user.png  the clock, the notification area, the Wi-Fi icon, the Bluetooth icon, then
#                   the battery
#
# and writes panel-report.txt with the panel's object list and each object's applet-iid. The
# installed panel's files and the project's are bound over /usr/share/mate-panel and
# /usr/lib/mate-panel in a mount namespace of their own (unshare -rm), so nothing is installed
# on the machine. It needs python-dbusmock, ImageMagick, meson, an X server of its own (Xvfb,
# or a rootful Xwayland window under Wayland) and the packages the top-level project builds
# with. CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
out=$(realpath -m "${1:-$here/out}")
report=$out/panel-report.txt
work=$(mktemp -d "$HOME/caminito-cmn-202.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"
rm -f "$out/fresh-user.png" "$report"

meson setup "$work/build" "$root" --prefix=/usr --sysconfdir=/etc >"$work/setup.log"
meson compile -C "$work/build" >"$work/compile.log"
meson install -C "$work/build" --destdir "$work/inst" --no-rebuild >/dev/null

# The installed panel's files, with Caminito's among them: what the panel finds under /usr.
mkdir -p "$work/share" "$work/lib"
cp -a /usr/share/mate-panel/. "$work/share/"
cp -a "$work/inst/usr/share/mate-panel/." "$work/share/"
cp -a /usr/lib/mate-panel/. "$work/lib/"
cp -a "$work/inst/usr/lib/mate-panel/." "$work/lib/"

# The schemas: the system's, and Caminito's with its overrides.
mkdir -p "$work/schemas"
cp /usr/share/glib-2.0/schemas/*.xml "$work/schemas/"
[ -e "$work/schemas/org.mate.power-manager.gschema.xml" ] ||
  cp "$root/tests/schemas/org.mate.power-manager.gschema.xml" "$work/schemas/"
cp "$work"/inst/usr/share/glib-2.0/schemas/* "$work/schemas/"
glib-compile-schemas "$work/schemas"

for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
if command -v Xvfb >/dev/null; then
  Xvfb ":$n" -screen 0 900x200x24 >/dev/null 2>&1 &
else
  Xwayland ":$n" -geometry 900x200 >/dev/null 2>&1 &
fi
server=$!
trap 'kill $server 2>/dev/null; rm -rf "$work"' EXIT
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\ngtk-icon-theme-name=%s\n' \
  "${CAMINITO_GTK_THEME:-Adwaita}" "${CAMINITO_ICON_THEME:-mate}" \
  >"$work/config/gtk-3.0/settings.ini"
DISPLAY=:$n GDK_BACKEND=x11 WAYLAND_DISPLAY= OUT=$out WORK=$work ROOT=$root \
  XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=keyfile \
  NO_AT_BRIDGE=1 CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} \
  timeout 120 unshare -rm bash -c '
    mount --bind "$WORK/share" /usr/share/mate-panel
    mount --bind "$WORK/lib" /usr/lib/mate-panel
    exec dbus-run-session -- bash "$ROOT/.scratch/checks/CMN-202/session.sh"' \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$report" || echo "session failed" >>"$report"
cat "$report"
echo "screenshot in $out"
