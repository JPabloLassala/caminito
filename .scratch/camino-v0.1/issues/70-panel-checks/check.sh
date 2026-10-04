#!/bin/bash
# The patched panel on Wayland, under a headless labwc: the layer surfaces, set_rectangle
# for each window, and what a click on Shut Down does. Run it inside dbus-run-session.
# "sm" as the first argument starts a stand-in session manager. WLRCTL names a wlrctl binary.
# The sleeps wait for the compositor, the panel and the windows, which say nothing when ready.
here=$(cd "$(dirname "$0")" && pwd)
work=${TMPDIR:-/tmp}/camino-70-checks
run=$work/run-${1:-nosm}; rm -rf $run; mkdir -p $run/config $run/rt; chmod 700 $run/rt
export XDG_RUNTIME_DIR=$run/rt XDG_CONFIG_HOME=$run/config GSETTINGS_BACKEND=keyfile
unset DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE
WLRCTL=${WLRCTL:-wlrctl}
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 labwc >$run/labwc.log 2>&1 &
sleep 2
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland
unset DISPLAY
zenity --info --title=T70FIRST --text=first >/dev/null 2>&1 &
[ "$1" = sm ] && { python3 $here/fake-sm.py >$run/sm.log 2>&1 & }
sleep 2
WAYLAND_DEBUG=1 mate-panel >$run/panel.log 2>&1 &
sleep 8
zenity --info --title=T70SECOND --text=second >/dev/null 2>&1 &
sleep 3
tops=$(gsettings get org.mate.panel toplevel-id-list); echo "toplevels: $tops"
top=$(echo "$tops" | sed "s/^\['\([^']*\)'.*/\1/")
object=/org/mate/panel/objects/t70shutdown/
gsettings set org.mate.panel.object:$object object-type action
gsettings set org.mate.panel.object:$object action-type shutdown
gsettings set org.mate.panel.object:$object toplevel-id "$top"
gsettings set org.mate.panel.object:$object position 600
ids=$(gsettings get org.mate.panel object-id-list)
gsettings set org.mate.panel object-id-list "${ids%]}, 't70shutdown']"
sleep 3
echo "--- toplevels before the click"; $WLRCTL toplevel list
$WLRCTL pointer move -4000 -4000; $WLRCTL pointer move 612 12; $WLRCTL pointer click left
sleep 3
echo "--- toplevels after the click"; $WLRCTL toplevel list
echo "--- session manager calls"; cat $run/sm.log 2>/dev/null
echo "--- set_rectangle"; grep -c set_rectangle $run/panel.log; grep set_rectangle $run/panel.log | tail -6
echo "--- layer surfaces"; grep -c "get_layer_surface" $run/panel.log; grep "get_layer_surface\|set_anchor" $run/panel.log | head -4
echo "--- workspace"; grep -c "ext_workspace" $run/panel.log
kill $(jobs -p) 2>/dev/null; sleep 1; pkill -u "$USER" -f "zenity --info --title=T70" 2>/dev/null
