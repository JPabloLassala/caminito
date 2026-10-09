#!/usr/bin/env bash
# Ticket 119's nested session, left up for the checks: a nested Caminito on a private bus with
# a runtime directory of its own, the accessibility registry, mate-panel with the global menu
# applet from a built vala-panel-appmenu-mate-caminito package, and a monitor of the bus. It
# writes the session's environment to <work>/env, and runs until <work>/stop is there.
#
#   up.sh <vala-panel-appmenu-mate-caminito-*.pkg.tar.zst> <work>
#
# It needs what caminito-vscode/nested/run.sh needs, and at-spi2-core.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
extension=$root/caminito-vscode
builtins=${EDITOR_BUILTINS:-/usr/share/code/resources/app/extensions}
work=$2
rm -rf "$work"
mkdir -p "$work"/{pkg,applets,schemas,config,builtin,out,driver}
mkdir -m 700 "$work/runtime"
tar -C "$work/pkg" -xf "$1" 2>/dev/null
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#^Location=.*#Location=$work/pkg/usr/lib/mate-panel/libappmenu-mate.so#" \
  "$work/pkg/usr/share/mate-panel/applets/org.vala-panel.appmenu.mate-panel-applet" \
  >"$work/applets/org.vala-panel.appmenu.mate-panel-applet"
cp "$work/pkg/usr/share/glib-2.0/schemas/org.valapanel.appmenu.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"
# The editor's built-ins, with this tree's extension in place of an installed one's, and the driver.
for dir in "$builtins"/*/; do ln -s "$dir" "$work/builtin/"; done
ln -sfn "$extension" "$work/builtin/caminito.global-menu"
ln -sfn "$here/driver" "$work/builtin/caminito.checks-driver"

host=${WAYLAND_DISPLAY:?up.sh needs a Wayland desktop}
[[ $host == /* ]] || host=$XDG_RUNTIME_DIR/$host
WAYLAND_DISPLAY=$host XDG_RUNTIME_DIR=$work/runtime XDG_CONFIG_HOME=$work/config \
  dbus-run-session -- bash "$here/session.sh" "$root" "$work"
