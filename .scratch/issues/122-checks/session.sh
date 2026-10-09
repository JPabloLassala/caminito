#!/usr/bin/env bash
# What up.sh runs on its private bus.
set -u
root=$1; work=$2; out=$work/out
RUST_LOG=${RUST_LOG:-info,caminito::dbus::menu_claim=debug} "${CAMINITO:-$root/target/debug/caminito}" 2>"$out/caminito.log" &
caminito=$!
for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$out/caminito.log" && break; sleep 0.1; done
line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$out/caminito.log" | head -1)
export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland LANG=C LANGUAGE=C
export CAMINITO_122_DRIVER=$work/driver
# NO_AT_BRIDGE keeps an app off the accessibility bus, and an editor's terminal may have it.
unset DISPLAY GTK_MODULES NO_AT_BRIDGE
for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
# The private bus has no systemd to start the accessibility registry with.
/usr/lib/at-spi2-registryd >/dev/null 2>&1 & registry=$!
dbus-monitor --session "interface='com.canonical.dbusmenu'" "interface='org.mate.Caminito'" "member='RequestName'" >"$out/bus.log" 2>&1 & monitor=$!
sleep 1
MATE_PANEL_APPLETS_DIR=$work/applets GSETTINGS_SCHEMA_DIR=$work/schemas mate-panel >"$out/panel.log" 2>&1 &
panel=$!
sleep 4
ids=$(dconf read /org/mate/panel/general/object-id-list)
dconf write /org/mate/panel/objects/appmenu/object-type "'applet'"
dconf write /org/mate/panel/objects/appmenu/applet-iid "'AppmenuAppletFactory::AppmenuApplet'"
dconf write /org/mate/panel/objects/appmenu/toplevel-id "'top'"
dconf write /org/mate/panel/objects/appmenu/position 300
dconf write /org/mate/panel/general/object-id-list "${ids%]}, 'appmenu']"
sleep 3
export -p >"$work/env"
until [ -e "$work/stop" ]; do sleep 0.5; done
pkill -f -- "--user-data-dir=$work/user" 2>/dev/null
kill $panel $registry $monitor 2>/dev/null; sleep 0.5; kill $caminito; wait $caminito 2>/dev/null
