#!/usr/bin/env bash
# CMN-95's check of the Thermal applet's packaging: it installs the top-level Meson project into a
# staging directory, as the PKGBUILD's package() and debian/rules' dh_auto_install do, and checks
#
#   - the applet's module, its entry in Add to Panel, named "Temperatures and Fans", whose
#     Location is the installed module, and its thermometer in hicolor
#   - helpers/caminito-applet-common/'s code in the module, as the applet links it statically
#   - that the module links libsensors, which ${shlibs:Depends} and Arch's lm_sensors give
#   - that the default panel, caminito.layout, doesn't name the applet
#
#   run.sh [<meson build directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs Meson,
# mate-panel's applet library and lm_sensors, and readelf.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-95.XXXXXX)
trap 'rm -rf "$work"' EXIT
build=${1:-$work/build}
[ -d "$build" ] || meson setup --prefix=/usr "$build" "$root" >/dev/null
ninja -C "$build" >/dev/null
DESTDIR=$work/dest meson install -C "$build" --no-rebuild --quiet
option() {
  meson introspect --buildoptions "$build" |
    python3 -c 'import json,sys; print(next(o["value"] for o in json.load(sys.stdin) if o["name"]==sys.argv[1]))' "$1"
}
prefix=$(option prefix)
libdir=$(option libdir)
case $libdir in /*) ;; *) libdir=$prefix/$libdir ;; esac

failed=0
check() {
  if eval "$2"; then echo "ok    $1"; else echo "FAIL  $1"; failed=1; fi
}
module=$libdir/mate-panel/libcaminito-thermal-applet.so
entry=$prefix/share/mate-panel/applets/org.mate.panel.CaminitoThermalApplet.mate-panel-applet
icon=$prefix/share/icons/hicolor/scalable/status/caminito-thermometer-symbolic.svg
layout=$prefix/share/mate-panel/layouts/caminito.layout

check "the module is installed at $module" "[ -f '$work/dest$module' ]"
check "the entry is installed" "[ -f '$work/dest$entry' ]"
check "the entry is called Temperatures and Fans" \
  "grep -qx 'Name=Temperatures and Fans' '$work/dest$entry'"
check "the entry's Location is the installed module" \
  "grep -qx 'Location=$module' '$work/dest$entry'"
check "the thermometer is in hicolor" "[ -f '$work/dest$icon' ]"
check "the module links libsensors" \
  "readelf -d '$work/dest$module' | grep -q 'NEEDED.*libsensors\.so'"
check "the module has caminito-applet-common's popup in it" \
  "readelf -Ws '$work/dest$module' | grep -q caminito_applet_popup"
check "the default panel is installed" "[ -f '$work/dest$layout' ]"
check "the default panel doesn't name the Thermal applet" \
  "! grep -q CaminitoThermalApplet '$work/dest$layout'"
exit "$failed"
