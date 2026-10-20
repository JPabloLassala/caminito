#!/usr/bin/env bash
# CMN-100's check of the Power applet's packaging: it installs the top-level Meson project into a
# staging directory, as the PKGBUILD's package() and debian/rules' dh_auto_install do, and checks
#
#   - the applet's module and its entry in Add to Panel, named "Power", whose Location is the
#     installed module
#   - that the default panel, caminito.layout, has the applet in the top panel, at the end beside
#     the Bluetooth applet
#   - that the override is installed, and that with mate-power-manager's schema (the copy in
#     tests/schemas) it makes icon-policy 'never'
#   - that the schemas still compile without mate-power-manager's schema, as on an X11 MATE with
#     no mpm, the distributions' hooks compiling them without --strict
#
#   run.sh [<meson build directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs Meson,
# mate-panel's applet library, glib-compile-schemas and gsettings. panel.sh is the by-eye half: a
# fresh user's panel.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-100.XXXXXX)
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
module=$libdir/mate-panel/libcaminito-power-applet.so
entry=$prefix/share/mate-panel/applets/org.mate.panel.CaminitoPowerApplet.mate-panel-applet
layout=$work/dest$prefix/share/mate-panel/layouts/caminito.layout
schemas=$work/dest$prefix/share/glib-2.0/schemas
override=$schemas/90_caminito-power-applet.gschema.override

check "the module is installed at $module" "[ -f '$work/dest$module' ]"
check "the entry is called Power" "grep -qx 'Name=Power' '$work/dest$entry'"
check "the entry's Location is the installed module" \
  "grep -qx 'Location=$module' '$work/dest$entry'"

# <object> <key>: the key's value in the layout's [Object <object>] group.
key() { sed -n "/^\[Object $1\]/,/^\[/p" "$layout" | sed -n "s/^$2=//p"; }
check "the default panel names the Power applet" \
  "[ \"\$(key power applet-iid)\" = CaminitoPowerAppletFactory::CaminitoPowerApplet ]"
check "it's in the top panel, at its end" \
  "[ \"\$(key power toplevel-id)\" = top ] && [ \"\$(key power pack-type)\" = end ]"
check "it's beside the Bluetooth applet" \
  "[ \"\$(key power pack-index)\" = \$((\$(key bluetooth pack-index) + 1)) ]"
check "no two objects at one end share a pack-index" \
  "[ -z \"\$(awk -F= '/^\[Object/{t=\"\";p=\"start\"} /^toplevel-id/{t=\$2} /^pack-type/{p=\$2} /^pack-index/{print t, p, \$2}' '$layout' | sort | uniq -d)\" ]"

check "the override is installed" "[ -f '$override' ]"
mkdir -p "$work/with-mpm" "$work/without-mpm"
cp "$schemas"/*.xml "$schemas"/*.override "$root/tests/schemas/org.mate.power-manager.gschema.xml" \
  "$work/with-mpm/"
cp "$schemas"/*.xml "$schemas"/*.override "$work/without-mpm/"
# The panel's schema too, which 90_caminito.gschema.override sets a key of.
cp /usr/share/glib-2.0/schemas/org.mate.panel*.xml "$work/with-mpm/" "$work/without-mpm/" 2>/dev/null || true
check "with mpm's schema they compile" "glib-compile-schemas '$work/with-mpm' 2>'$work/with.log'"
check "and icon-policy is 'never'" \
  "[ \"\$(GSETTINGS_SCHEMA_DIR='$work/with-mpm' GSETTINGS_BACKEND=memory gsettings get org.mate.power-manager icon-policy)\" = \"'never'\" ]"
check "without mpm's schema they still compile" \
  "glib-compile-schemas '$work/without-mpm' 2>'$work/without.log'"
sed 's/^/      /' "$work/without.log"
exit "$failed"
