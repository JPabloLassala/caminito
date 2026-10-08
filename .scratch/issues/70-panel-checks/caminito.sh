#!/bin/bash
# The patched panel in a nested Caminito: the layer surfaces, and set_rectangle for each window.
# Run it in a desktop session, inside dbus-run-session, with CAMINITO naming a caminito binary
# (a debug build, as tools/dev-nested --bin makes) and WLRCTL a wlrctl binary.
# The sleeps wait for Caminito, the panel and the windows, which say nothing when ready.
work=${TMPDIR:-/tmp}/caminito-70-checks
run=$work/run-caminito; rm -rf $run; mkdir -p $run/config
export XDG_CONFIG_HOME=$run/config
${CAMINITO:-caminito} >$run/caminito.log 2>&1 &
sleep 5
display=$(sed -n 's/.*listening on WAYLAND_DISPLAY=//p' $run/caminito.log | head -1)
echo "nested display: $display"
export WAYLAND_DISPLAY=$display GDK_BACKEND=wayland
unset DISPLAY
zenity --info --title=T70FIRST --text=first >/dev/null 2>&1 &
sleep 2
WAYLAND_DEBUG=1 mate-panel >$run/panel.log 2>&1 &
sleep 8
zenity --info --title=T70SECOND --text=second >/dev/null 2>&1 &
sleep 4
echo "--- toplevels"; ${WLRCTL:-wlrctl} toplevel list
echo "--- set_rectangle"; grep set_rectangle $run/panel.log
echo "--- layer surfaces"; grep "get_layer_surface\|set_anchor" $run/panel.log
echo "--- panel alive"; pgrep -x mate-panel >/dev/null && echo yes
echo "--- caminito log"; grep -i "warn\|error\|panic" $run/caminito.log | head
kill $(jobs -p) 2>/dev/null; sleep 1; pkill -u "$USER" -f "zenity --info --title=T70" 2>/dev/null
