#!/usr/bin/env bash
# Ticket 110's check by hand: a nested Caminito on a private bus with a scratch
# XDG_CONFIG_HOME, a GTK 3 and a GTK 4 GtkApplication with a menubar, and mate-panel
# loading the applet from a built vala-panel-appmenu-mate-caminito package. It prints what
# GlobalMenu has, whether each app's gtk-shell-shows-menubar follows the applet, and what
# the applet asks the focused app for on the bus. It clicks nothing.
#
#   run.sh <vala-panel-appmenu-mate-caminito-*.pkg.tar.zst>
#
# It needs a Wayland desktop to nest in, a debug build of caminito in target/, python-gobject
# and mate-panel-caminito.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-110.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work"/{pkg,applets,schemas,config}
tar -C "$work/pkg" -xf "$1" 2>/dev/null
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#^Location=.*#Location=$work/pkg/usr/lib/mate-panel/libappmenu-mate.so#" \
  "$work/pkg/usr/share/mate-panel/applets/org.vala-panel.appmenu.mate-panel-applet" \
  >"$work/applets/org.vala-panel.appmenu.mate-panel-applet"
cp "$work/pkg/usr/share/glib-2.0/schemas/org.valapanel.appmenu.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"
XDG_CONFIG_HOME=$work/config timeout 90 dbus-run-session -- \
  bash "$here/session.sh" "$here" "$root" "$work" >/dev/null 2>&1 || true
cat "$work/out/report"
