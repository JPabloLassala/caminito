#!/bin/bash
# What keyboard interactivity mate-panel's and caja's Layer surfaces ask for (163), in a nested
# Caminito on Xvfb. Run it in the OrbStack machine, with CAMINITO naming a caminito binary:
#   CAMINITO=$HOME/target-163/debug/caminito .scratch/issues/163-checks/layer-keyboard.sh
# The sleeps wait for Xvfb, Caminito, the panel and caja, which say nothing when ready.
work=$(mktemp -d "$HOME/t163.XXXXXX")
mkdir -p "$work/config" "$work/rt"; chmod 700 "$work/rt"
export XDG_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/config
unset WAYLAND_DISPLAY
Xvfb :163 -screen 0 1280x720x24 >"$work/xvfb.log" 2>&1 &
sleep 1
inner() {
    DISPLAY=:163 "${CAMINITO:-caminito}" >"$work/caminito.log" 2>&1 &
    sleep 6
    display=$(sed -n 's/.*listening on WAYLAND_DISPLAY=//p' "$work/caminito.log" | head -1)
    echo "nested display: $display"
    export WAYLAND_DISPLAY=$display GDK_BACKEND=wayland
    unset DISPLAY
    WAYLAND_DEBUG=1 mate-panel >"$work/panel.log" 2>&1 &
    WAYLAND_DEBUG=1 caja --force-desktop >"$work/caja.log" 2>&1 &
    sleep 10
    for client in panel caja; do
        echo "--- $client: layer surfaces"
        grep -o "get_layer_surface(.*" "$work/$client.log"
        echo "--- $client: set_keyboard_interactivity"
        grep -o "set_keyboard_interactivity(.*" "$work/$client.log" | sort | uniq -c
    done
    kill $(jobs -p) 2>/dev/null
}
export -f inner; export work
dbus-run-session -- bash -c inner
kill $(jobs -p) 2>/dev/null
# gvfs and the document portal mount under the runtime directory.
fusermount3 -u "$work/rt/gvfs" 2>/dev/null; fusermount3 -u "$work/rt/doc" 2>/dev/null
rm -rf "$work"
