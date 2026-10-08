#!/usr/bin/env bash
# Ticket 111's check by hand: a nested Caminito on a private bus with a scratch
# XDG_CONFIG_HOME, mate-panel, and pluma, caja, atril and mate-terminal one at a time: with
# the module and no applet, with the applet and no module, and with both. The module is
# loaded from a built appmenu-gtk-module-caminito package, which needn't be installed. It
# prints what GlobalMenu has, and through AT-SPI the height of each app's own menubar and
# the menus the panel shows. With both it activates Help > About in the panel's menubar and
# prints the windows there are then. Last it removes the applet from under pluma and
# mate-terminal. It looks at nothing drawn, and takes about two minutes.
#
#   run.sh <appmenu-gtk-module-caminito-*.pkg.tar.zst>
#
# It needs a Wayland desktop to nest in, a debug build of caminito in target/ or another
# caminito in CAMINITO, python-gobject,
# at-spi2-core, mate-panel-caminito and vala-panel-appmenu-mate-caminito, and no
# appmenu-gtk-module installed.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-111.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work"/{module,schemas,config}
tar -C "$work/module" -xf "$1" 2>/dev/null
cp "$work"/module/usr/share/glib-2.0/schemas/*.xml "$work/schemas/"
glib-compile-schemas "$work/schemas"
XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas timeout 240 dbus-run-session -- \
  bash "$here/session.sh" "$here" "$root" "$work" >/dev/null 2>&1 || true
cat "$work/out/report"
