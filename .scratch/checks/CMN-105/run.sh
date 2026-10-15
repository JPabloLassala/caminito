#!/usr/bin/env bash
# CMN-105's check of the default panel: a fresh user's mate-panel, the patched one
# (mate-panel-caminito, built by packaging/arch/mate-panel-caminito) on Xvfb, gets the Bluetooth
# applet in its top panel from the files `meson install` puts in the package:
# data/caminito.layout and data/90_caminito.gschema.override.
#
#   run.sh <mate-panel-caminito package> [<output directory>]
#
# For each case it starts the panel on a keyfile GSettings backend with no settings (a fresh
# user), against python-dbusmock's BlueZ with one adapter on a private bus in the system bus's
# place, and screenshots it:
#
#   with-override.png     the package's files installed: the applet is beside the notification
#                         area
#   without-override.png  the override left out (the control): the stock layout, no applet
#   no-adapter.png        the override in, and a BlueZ with no adapter: the applet hides itself
#
# and writes report.txt with the panel's object list and each object's applet-iid. The panel's
# and the package's files are bound over /usr/share/mate-panel and /usr/lib/mate-panel in a
# mount namespace of their own (unshare -rm), so nothing is installed on the machine. The
# applet is built against the patched panel's applet library from the package, which checks
# the build dependency too. It needs python-dbusmock, Xvfb, xdotool, ImageMagick, meson and
# the packages the top-level project builds with. CAMINITO_PYTHON names a Python other than
# python3 that has dbusmock.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
pkg=$(realpath "$1")
out=$(realpath -m "${2:-$here/out}")
work=$(mktemp -d "$HOME/caminito-cmn-105.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$out" "$work/pkg"
rm -f "$out"/*.png "$out/report.txt"

tar -C "$work/pkg" -xf "$pkg"
# The panel's .pc files name /usr: point them at the unpacked package, ahead of the system's.
mkdir -p "$work/pc"
for pc in "$work"/pkg/usr/lib/pkgconfig/*.pc; do
  sed "s#^prefix=/usr#prefix=$work/pkg/usr#" "$pc" >"$work/pc/$(basename "$pc")"
done
PKG_CONFIG_PATH=$work/pc meson setup "$work/build" "$root" --prefix=/usr >"$work/setup.log"
meson compile -C "$work/build" >"$work/compile.log"
meson install -C "$work/build" --destdir "$work/inst" --no-rebuild >/dev/null
ldd "$(find "$work/inst" -name libcaminito-bluetooth-applet.so)" | grep -i mate-panel-applet \
  >>"$out/report.txt" || true

# The package's files, with Caminito's among them: what the panel finds under /usr.
mkdir -p "$work/share" "$work/lib" "$work/layouts-without"
cp -a "$work/pkg/usr/share/mate-panel/." "$work/share/"
cp -a "$work/inst/usr/share/mate-panel/." "$work/share/"
cp -a "$work/pkg/usr/lib/mate-panel/." "$work/lib/"
cp -a "$work/inst/usr/lib/mate-panel/." "$work/lib/"
# The same without the layout: the control's.
cp -a "$work/pkg/usr/share/mate-panel/layouts/." "$work/layouts-without/"

# The schemas: the system's, the patched panel's over its own, and optionally the override.
for case in with without; do
  mkdir -p "$work/schemas-$case"
  cp /usr/share/glib-2.0/schemas/*.xml "$work/schemas-$case/"
  cp "$work"/pkg/usr/share/glib-2.0/schemas/org.mate.panel*.xml "$work/schemas-$case/"
done
cp "$work/inst/usr/share/glib-2.0/schemas/90_caminito.gschema.override" "$work/schemas-with/"
for case in with without; do glib-compile-schemas --strict "$work/schemas-$case"; done

for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
Xvfb ":$n" -screen 0 900x200x24 >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null; rm -rf "$work"' EXIT
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

# <name> <schemas> <layouts> <adapter: yes|no>
run_case() {
  local name=$1 schemas=$2 layouts=$3 adapter=$4
  echo "== $name" >>"$out/report.txt"
  mkdir -p "$work/config-$name/gtk-3.0"
  printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
    >"$work/config-$name/gtk-3.0/settings.ini"
  DISPLAY=:$n CASE=$name ADAPTER=$adapter OUT=$out WORK=$work ROOT=$root \
    XDG_CONFIG_HOME=$work/config-$name GSETTINGS_SCHEMA_DIR=$schemas GSETTINGS_BACKEND=keyfile \
    LD_LIBRARY_PATH=$work/pkg/usr/lib NO_AT_BRIDGE=1 CAMINITO_PYTHON=${CAMINITO_PYTHON:-python3} \
    LAYOUTS=$layouts timeout 120 \
    unshare -rm bash -c '
      mount --bind "$WORK/share" /usr/share/mate-panel
      mount --bind "$LAYOUTS" /usr/share/mate-panel/layouts
      mount --bind "$WORK/lib" /usr/lib/mate-panel
      exec dbus-run-session -- bash "$ROOT/.scratch/checks/CMN-105/session.sh"' \
    >>"$out/report.txt" 2>&1 || echo "$name: session failed" >>"$out/report.txt"
}
run_case with-override "$work/schemas-with" "$work/share/layouts" yes
run_case without-override "$work/schemas-without" "$work/layouts-without" yes
run_case no-adapter "$work/schemas-with" "$work/share/layouts" no
cat "$out/report.txt"
echo "screenshots in $out"
