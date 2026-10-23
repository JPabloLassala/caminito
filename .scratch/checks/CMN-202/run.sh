#!/usr/bin/env bash
# CMN-202's check of the Network applet's packaging: it installs the top-level Meson project into
# a staging directory, as the PKGBUILD's package(), debian/rules' dh_auto_install and the spec's
# %install do, and checks
#
#   - the applet's module, its entry in Add to Panel, named "Network", whose Location is the
#     installed module, and caminito-network's autostart entry
#   - that the default panel, caminito.layout, has the applet in the top panel, between the
#     notification area and the Bluetooth applet, with the Power applet still beside that
#   - that each package installs caminito-network beside the other binaries, and the Fedora
#     spec lists every file the applet installs
#
#   run.sh [<meson build directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs Meson,
# mate-panel's applet library and libnm. panel.sh is the by-eye half: a fresh user's panel.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-202.XXXXXX)
trap 'rm -rf "$work"' EXIT
build=${1:-$work/build}
[ -d "$build" ] || meson setup --prefix=/usr --sysconfdir=/etc "$build" "$root" >/dev/null
ninja -C "$build" >/dev/null
DESTDIR=$work/dest meson install -C "$build" --no-rebuild --quiet
option() {
  meson introspect --buildoptions "$build" |
    python3 -c 'import json,sys; print(next(o["value"] for o in json.load(sys.stdin) if o["name"]==sys.argv[1]))' "$1"
}
prefix=$(option prefix)
libdir=$(option libdir)
case $libdir in /*) ;; *) libdir=$prefix/$libdir ;; esac
sysconfdir=$(option sysconfdir)
case $sysconfdir in /*) ;; *) sysconfdir=$prefix/$sysconfdir ;; esac

failed=0
check() {
  if eval "$2"; then echo "ok    $1"; else echo "FAIL  $1"; failed=1; fi
}
module=$libdir/mate-panel/libcaminito-network-applet.so
entry=$prefix/share/mate-panel/applets/org.mate.panel.CaminitoNetworkApplet.mate-panel-applet
autostart=$sysconfdir/xdg/autostart/caminito-network.desktop
layout=$work/dest$prefix/share/mate-panel/layouts/caminito.layout

check "the module is installed at $module" "[ -f '$work/dest$module' ]"
check "the entry is called Network" "grep -qx 'Name=Network' '$work/dest$entry'"
check "the entry's Location is the installed module" \
  "grep -qx 'Location=$module' '$work/dest$entry'"
check "caminito-network's autostart entry is installed" \
  "grep -qx 'Exec=caminito-network' '$work/dest$autostart'"

# <object> <key>: the key's value in the layout's [Object <object>] group.
key() { sed -n "/^\[Object $1\]/,/^\[/p" "$layout" | sed -n "s/^$2=//p"; }
check "the default panel names the Network applet" \
  "[ \"\$(key network applet-iid)\" = CaminitoNetworkAppletFactory::CaminitoNetworkApplet ]"
check "it's in the top panel, at its end" \
  "[ \"\$(key network toplevel-id)\" = top ] && [ \"\$(key network pack-type)\" = end ]"
check "it's beside the notification area" \
  "[ \"\$(key network pack-index)\" = \$((\$(key notification-area pack-index) + 1)) ]"
check "the Bluetooth applet is beside it" \
  "[ \"\$(key bluetooth pack-index)\" = \$((\$(key network pack-index) + 1)) ]"
check "the Power applet is beside the Bluetooth applet" \
  "[ \"\$(key power pack-index)\" = \$((\$(key bluetooth pack-index) + 1)) ]"
check "no two objects at one end share a pack-index" \
  "[ -z \"\$(awk -F= '/^\[Object/{t=\"\";p=\"start\"} /^toplevel-id/{t=\$2} /^pack-type/{p=\$2} /^pack-index/{print t, p, \$2}' '$layout' | sort | uniq -d)\" ]"

pkgbuild=$root/packaging/arch/caminito-git/PKGBUILD
rules=$root/packaging/debian/caminito/debian/rules
control=$root/packaging/debian/caminito/debian/control
spec=$root/packaging/fedora/caminito/caminito.spec
for f in "$pkgbuild" "$rules" "$spec"; do
  check "$(basename "$f") installs caminito-network" \
    "grep -q 'target/release/caminito-network' '$f'"
done
srcinfo=$(cd "$(dirname "$pkgbuild")" && makepkg --printsrcinfo)
check "caminito-git depends on libnm and nm-connection-editor" \
  "grep -qxP '\tdepends = libnm' <<<\"\$srcinfo\" && grep -qxP '\tdepends = nm-connection-editor' <<<\"\$srcinfo\""
check "Debian builds with libnm-dev and recommends nm-connection-editor" \
  "grep -qx ' libnm-dev,' '$control' && sed -n '/^Recommends:/,/^[A-Z]/p' '$control' | grep -qx ' nm-connection-editor,'"
check "Fedora builds with libnm and requires nm-connection-editor" \
  "grep -q '^BuildRequires: *NetworkManager-libnm-devel' '$spec' && grep -q '^Requires: *nm-connection-editor' '$spec'"
# Every file Meson installs, by the spec's macros, is in %files.
while read -r path; do
  rpm=$(sed -e "s#^$sysconfdir/#%{_sysconfdir}/#" -e "s#^$libdir/#%{_libdir}/#" \
    -e "s#^$prefix/share/#%{_datadir}/#" <<<"$path")
  check "the spec lists $rpm" "grep -qF '$rpm' '$spec'"
done < <(cd "$work/dest" && find . -type f -ipath '*network*' | sed 's#^\.##')
exit "$failed"
