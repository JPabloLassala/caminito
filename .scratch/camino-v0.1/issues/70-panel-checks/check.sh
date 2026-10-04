#!/bin/bash
# Run inside dbus-run-session. "sm" as the first argument starts a fake session manager.
H=/home/jpablo/t70/check
T=$H/run-${1:-nosm}; rm -rf $T; mkdir -p $T/config $T/rt; chmod 700 $T/rt
export XDG_RUNTIME_DIR=$T/rt XDG_CONFIG_HOME=$T/config GSETTINGS_BACKEND=keyfile
unset DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE
WLRCTL=/home/jpablo/ticket-65-tools/wlrctl/build/wlrctl
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 labwc >$T/labwc.log 2>&1 &
sleep 2
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland
unset DISPLAY
zenity --info --title=T70FIRST --text=first >/dev/null 2>&1 &
[ "$1" = sm ] && { python3 $H/fake-sm.py >$T/sm.log 2>&1 & }
sleep 2
WAYLAND_DEBUG=1 mate-panel >$T/panel.log 2>&1 &
sleep 8
zenity --info --title=T70SECOND --text=second >/dev/null 2>&1 &
sleep 3
tops=$(gsettings get org.mate.panel toplevel-id-list); echo "toplevels: $tops"
top=$(echo "$tops" | sed "s/^\['\([^']*\)'.*/\1/")
P=/org/mate/panel/objects/t70shutdown/
gsettings set org.mate.panel.object:$P object-type action
gsettings set org.mate.panel.object:$P action-type shutdown
gsettings set org.mate.panel.object:$P toplevel-id "$top"
gsettings set org.mate.panel.object:$P position 600
ids=$(gsettings get org.mate.panel object-id-list)
gsettings set org.mate.panel object-id-list "${ids%]}, 't70shutdown']"
sleep 3
echo "--- toplevels before the click"; $WLRCTL toplevel list
$WLRCTL pointer move -4000 -4000; $WLRCTL pointer move 612 12; $WLRCTL pointer click left
sleep 3
echo "--- toplevels after the click"; $WLRCTL toplevel list
echo "--- session manager calls"; cat $T/sm.log 2>/dev/null
echo "--- set_rectangle"; grep -c set_rectangle $T/panel.log; grep set_rectangle $T/panel.log | tail -6
echo "--- layer surfaces"; grep -c "get_layer_surface" $T/panel.log; grep "get_layer_surface\|set_anchor" $T/panel.log | head -4
echo "--- workspace"; grep -c "ext_workspace" $T/panel.log
kill $(jobs -p) 2>/dev/null; sleep 1; pkill -u jpablo -f "zenity --info --title=T70" 2>/dev/null
