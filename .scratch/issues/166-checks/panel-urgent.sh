#!/bin/bash
# Whether mate-panel's window list shows an urgent window (166), in a nested Caminito on
# Xvfb. Run it in the OrbStack machine, with CAMINITO naming a caminito binary and OUT a
# directory for the two screenshots:
#   CAMINITO=$HOME/target-166/debug/caminito OUT=$PWD/.scratch/issues/166-checks/out \
#       .scratch/issues/166-checks/panel-urgent.sh
# The first xterm rings its bell while the second has the focus, which sets its urgency hint.
# The sleeps wait for Xvfb, Caminito, the panel and the xterms, which say nothing when ready.
work=$(mktemp -d "$HOME/t166.XXXXXX")
mkdir -p "$work/config" "$work/rt" "${OUT:=$work}"; chmod 700 "$work/rt"
export XDG_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/config
unset WAYLAND_DISPLAY
Xvfb :166 -screen 0 1920x1200x24 >"$work/xvfb.log" 2>&1 &
sleep 1
inner() {
    DISPLAY=:166 "${CAMINITO:-caminito}" >"$work/caminito.log" 2>&1 &
    sleep 6
    display=$(sed -n 's/.*listening on WAYLAND_DISPLAY=//p' "$work/caminito.log" | head -1)
    xdisplay=$(sed -n 's/.*Xwayland is ready on DISPLAY=//p' "$work/caminito.log" | head -1)
    echo "nested displays: $display $xdisplay"
    WAYLAND_DISPLAY=$display GDK_BACKEND=wayland WAYLAND_DEBUG=1 \
        mate-panel >"$work/panel.log" 2>&1 &
    sleep 8
    DISPLAY=$xdisplay xterm -title urgent -xrm 'XTerm*bellIsUrgent: true' \
        -e sh -c 'sleep 8; printf "\a"; sleep 60' &
    sleep 3
    DISPLAY=$xdisplay xterm -title focused &
    sleep 2
    DISPLAY=:166 import -window root "$OUT/before.png"
    sleep 6
    DISPLAY=:166 import -window root "$OUT/urgent.png"
    echo "--- the panel's xfce state events"
    grep -o "xfce_foreign_toplevel_handle_v1#[0-9]*\.state(.*" "$work/panel.log"
    kill $(jobs -p) 2>/dev/null
}
export -f inner; export work OUT
dbus-run-session -- bash -c inner
kill $(jobs -p) 2>/dev/null
# gvfs and the document portal mount under the runtime directory.
fusermount3 -u "$work/rt/gvfs" 2>/dev/null; fusermount3 -u "$work/rt/doc" 2>/dev/null
[ "$KEEP" ] && echo "kept $work" || rm -rf "$work"
