#!/bin/bash
# A maximised pluma and a maximised xterm beside mate-panel in a nested Camino (64): both
# stop at the panel. Run it under xvfb-run, with CAMINO naming a camino binary (a debug
# build, as tools/dev-nested --bin makes):
#
#   CAMINO=target/debug/camino xvfb-run -s "-screen 0 1920x1080x24" \
#     dbus-run-session .scratch/camino-v0.1/issues/64-nested-check/check.sh
#
# It needs mate-panel, pluma, xterm, xdotool and python3. The sleeps wait for Camino, the
# panel and the windows, which say nothing when ready.
run=${TMPDIR:-/tmp}/camino-64-check; rm -rf $run; mkdir -p $run/config
export XDG_CONFIG_HOME=$run/config
outer=$DISPLAY
${CAMINO:-camino} >$run/camino.log 2>&1 &
sleep 6
wayland=$(sed -n 's/.*listening on WAYLAND_DISPLAY=//p' $run/camino.log | head -1)
x=$(sed -n 's/.*Xwayland is ready on DISPLAY=//p' $run/camino.log | head -1)
echo "nested: WAYLAND_DISPLAY=$wayland DISPLAY=$x"
dump() {
  gdbus call --session --dest org.mate.Camino.Debug --object-path /org/mate/Camino/Debug \
    --method org.mate.Camino.Debug.DumpState |
    python3 -c '
import ast, json, sys
dump = json.loads(ast.literal_eval(sys.stdin.read())[0])
for o in dump["outputs"]:
    print("output", o["name"], "geometry", o["geometry"], "work area", o["work_area"])
    for l in o["layer_surfaces"]:
        print("  layer", l["layer"], l["namespace"], l["geometry"], "zone", l["exclusive_zone"])
for w in dump["windows"]:
    print("window", w["app_id"], "xwayland" if w["xwayland"] else "wayland",
          "geometry", w["geometry"], "frame", w["frame_extents"])
'
}
export WAYLAND_DISPLAY=$wayland
GDK_BACKEND=wayland DISPLAY= mate-panel >$run/panel.log 2>&1 &
sleep 8
echo "--- with the panel"; dump
DISPLAY=$x xterm -maximized >$run/xterm.log 2>&1 &
sleep 3
GDK_BACKEND=wayland DISPLAY= pluma >$run/pluma.log 2>&1 &
sleep 5
# A double click on pluma's titlebar maximises it, as Marco's default action does.
title=$(gdbus call --session --dest org.mate.Camino.Debug --object-path /org/mate/Camino/Debug \
  --method org.mate.Camino.Debug.DumpState | python3 -c '
import ast, json, sys
dump = json.loads(ast.literal_eval(sys.stdin.read())[0])
g = next(w["geometry"] for w in dump["windows"] if w["app_id"] == "pluma")
print(g["x"] + g["width"] // 2, g["y"] - 12)
')
DISPLAY=$outer xdotool mousemove $title click --repeat 2 --delay 100 1
sleep 3
echo "--- with both maximised"; dump
echo "--- root"; DISPLAY=$x xprop -root _NET_WORKAREA _GTK_WORKAREAS_D0
pkill -x mate-panel; sleep 3
echo "--- without the panel"; dump
echo "--- camino log"; grep -i "warn\|error\|panic" $run/camino.log | head
kill $(jobs -p) 2>/dev/null
