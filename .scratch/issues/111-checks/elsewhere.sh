#!/usr/bin/env bash
# Ticket 111's third criterion by hand: pluma, caja, atril, mate-terminal and mate-calc under
# X11 (Xvfb) and under a headless Wayfire, each on a private bus, once with Arch's stock
# appmenu-gtk-module and once with the patched one, without and with a stand-in owner of
# the registrar's name. For each it prints the height of the app's menubar through AT-SPI,
# under X11 the menu properties on its windows, and the module's complaints. Then it prints
# the diff of the stock's lines and the patched's. Neither package needs to be installed.
# It takes about four minutes.
#
#   elsewhere.sh <appmenu-gtk-module-*.pkg.tar.zst> <appmenu-gtk-module-caminito-*.pkg.tar.zst>
#
# It needs xorg-server-xvfb, xorg-xprop, xorg-xwininfo, wayfire, python-gobject and
# at-spi2-core, and no appmenu-gtk-module installed.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
work=$(mktemp -d -t caminito-111.XXXXXX)
trap 'rm -rf "$work"' EXIT
for which in stock patched; do
  mkdir -p "$work/$which/schemas"
  tar -C "$work/$which" -xf "$1" 2>/dev/null
  cp "$work/$which"/usr/share/glib-2.0/schemas/*.xml "$work/$which/schemas/"
  glib-compile-schemas "$work/$which/schemas"
  shift
done
printf '[core]\nplugins = \nxwayland = false\n' >"$work/wayfire.ini"
for where in x11 wayfire; do
  for which in stock patched; do
    XDG_CONFIG_HOME=$work/config-$where-$which timeout 100 dbus-run-session -- \
      bash "$here/elsewhere-session.sh" "$here" "$work" $where $which >/dev/null 2>&1 || true
  done
  echo "== $where, stock"; cat "$work/$where-stock"
  echo "== $where, patched against stock"
  diff "$work/$where-stock" "$work/$where-patched" && echo "the same"
done
