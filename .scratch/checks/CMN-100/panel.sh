#!/usr/bin/env bash
# CMN-100's by-eye check of the default panel: a fresh user's mate-panel, the patched one
# (mate-panel-caminito, built by packaging/arch/mate-panel-caminito) on Xvfb, gets the Power
# applet in its top panel beside the Bluetooth applet, from the files `meson install` puts in
# the package. It is CMN-105's run.sh with UPower's mock beside BlueZ's.
#
#   panel.sh <mate-panel-caminito package> [<output directory>]
#
# It starts the panel on a keyfile GSettings backend with no settings (a fresh user), against
# python-dbusmock's BlueZ with one adapter and its UPower with a battery at 78%, on a private
# bus in the system bus's place, and screenshots it:
#
#   fresh-user.png  the clock, the notification area, the Bluetooth icon, then the battery
#
# and writes panel-report.txt with the panel's object list and each object's applet-iid, and
# mate-power-manager's icon-policy with the package's schemas. The panel's and the package's
# files are bound over /usr/share/mate-panel and /usr/lib/mate-panel in a mount namespace of
# their own (unshare -rm), so nothing is installed on the machine. It needs python-dbusmock,
# Xvfb, xdotool, ImageMagick, meson and the packages the top-level project builds with.
# CAMINITO_PYTHON names a Python other than python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
pkg=$(realpath "$1")
out=$(realpath -m "${2:-$here/out}")
report=$out/panel-report.txt
work=$(mktemp -d "$HOME/caminito-cmn-100.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$out" "$work/pkg"
rm -f "$out/fresh-user.png" "$report"

tar -C "$work/pkg" -xf "$pkg"
# The panel's .pc files name /usr: point them at the unpacked package, ahead of the system's.
mkdir -p "$work/pc"
for pc in "$work"/pkg/usr/lib/pkgconfig/*.pc; do
  sed "s#^prefix=/usr#prefix=$work/pkg/usr#" "$pc" >"$work/pc/$(basename "$pc")"
done
PKG_CONFIG_PATH=$work/pc meson setup "$work/build" "$root" --prefix=/usr >"$work/setup.log"
meson compile -C "$work/build" >"$work/compile.log"
meson install -C "$work/build" --destdir "$work/inst" --no-rebuild >/dev/null

# The package's files, with Caminito's among them: what the panel finds under /usr.
mkdir -p "$work/share" "$work/lib"
cp -a "$work/pkg/usr/share/mate-panel/." "$work/share/"
cp -a "$work/inst/usr/share/mate-panel/." "$work/share/"
cp -a "$work/pkg/usr/lib/mate-panel/." "$work/lib/"
cp -a "$work/inst/usr/lib/mate-panel/." "$work/lib/"

# The schemas: the system's, the patched panel's over its own, mpm's from tests/schemas where
# the system has none, and Caminito's with both overrides.
mkdir -p "$work/schemas"
cp /usr/share/glib-2.0/schemas/*.xml "$work/schemas/"
cp "$work"/pkg/usr/share/glib-2.0/schemas/org.mate.panel*.xml "$work/schemas/"
[ -e "$work/schemas/org.mate.power-manager.gschema.xml" ] ||
  cp "$root/tests/schemas/org.mate.power-manager.gschema.xml" "$work/schemas/"
cp "$work"/inst/usr/share/glib-2.0/schemas/* "$work/schemas/"
glib-compile-schemas --strict "$work/schemas"
echo "icon-policy: $(GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=memory \
  gsettings get org.mate.power-manager icon-policy)" >>"$report"

for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
Xvfb ":$n" -screen 0 900x200x24 >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null; rm -rf "$work"' EXIT
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

mkdir -p "$work/config/gtk-3.0"
printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
  >"$work/config/gtk-3.0/settings.ini"
DISPLAY=:$n OUT=$out WORK=$work ROOT=$root \
  XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=keyfile \
  LD_LIBRARY_PATH=$work/pkg/usr/lib NO_AT_BRIDGE=1 CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} \
  timeout 120 unshare -rm bash -c '
    mount --bind "$WORK/share" /usr/share/mate-panel
    mount --bind "$WORK/lib" /usr/lib/mate-panel
    exec dbus-run-session -- bash "$ROOT/.scratch/checks/CMN-100/session.sh"' \
  >>"$report" 2>&1 || echo "session failed" >>"$report"
cat "$report"
echo "screenshot in $out"
