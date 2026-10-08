#!/usr/bin/env bash
# The session run.sh starts: Caminito nested, two apps, a stand-in for the applet, then the panel.
set -u
here=$1; root=$2; work=$3; out=$work/out; rm -rf "$out"; mkdir -p "$out"
prop() { busctl --user get-property org.mate.Caminito /org/mate/Caminito org.mate.Caminito GlobalMenu 2>&1; }
"$root/target/debug/caminito" 2>"$out/caminito.log" &
caminito=$!
for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$out/caminito.log" && break; sleep 0.1; done
line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$out/caminito.log" | head -1)
export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland
unset DISPLAY
echo "nested: $WAYLAND_DISPLAY" >"$out/report"
for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
{
echo "== no window"; prop
python3 "$here/app.py" 3.0 "$out/gtk3.log" & gtk3=$!
sleep 2
echo "== gtk3 focused"; prop
python3 "$here/app.py" 4.0 "$out/gtk4.log" & gtk4=$!
sleep 2
echo "== gtk4 focused"; prop
echo "== stand-in owns the applet's name for 2 s"
python3 "$here/own.py" org.mate.Caminito.GlobalMenuApplet 2
sleep 1
echo "-- gtk3:"; cat "$out/gtk3.log"; echo "-- gtk4:"; cat "$out/gtk4.log"
echo "== the panel with the built applet"
dbus-monitor --session "type='method_call',interface='org.gtk.Menus'" "type='method_call',interface='org.gtk.Actions'" >"$out/monitor.log" 2>&1 & monitor=$!
sleep 0.5
MATE_PANEL_APPLETS_DIR=$work/applets GSETTINGS_SCHEMA_DIR=$work/schemas \
  \
  mate-panel >"$out/panel.log" 2>&1 & panel=$!
sleep 4
ids=$(dconf read /org/mate/panel/general/object-id-list)
echo "(before the applet)"
echo "-- windows:"; busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState --json=short | python3 -c "
import json,sys
d=json.loads(json.loads(sys.stdin.read())['data'][0])
print('focus', d.get('focus'))
for w in d.get('windows', []): print({k: w.get(k) for k in ('id','title','app_id','window_type','kind','geometry','parent')})
print('layers', [ (l.get('namespace'), l.get('layer'), l.get('keyboard')) for l in d.get('layers', [])] if isinstance(d.get('layers'), list) else list(d.keys()))
"
echo "objects before: $ids"
dconf write /org/mate/panel/objects/appmenu/object-type "'applet'"
dconf write /org/mate/panel/objects/appmenu/applet-iid "'AppmenuAppletFactory::AppmenuApplet'"
dconf write /org/mate/panel/objects/appmenu/toplevel-id "'top'"
dconf write /org/mate/panel/objects/appmenu/position 300
dconf write /org/mate/panel/general/object-id-list "${ids%]}, 'appmenu']"
sleep 4
echo "-- windows:"; busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState --json=short | python3 -c "
import json,sys
d=json.loads(json.loads(sys.stdin.read())['data'][0])
print('focus', d.get('focus'))
for w in d.get('windows', []): print({k: w.get(k) for k in ('id','title','app_id','window_type','kind','geometry','parent')})
print('layers', [ (l.get('namespace'), l.get('layer'), l.get('keyboard')) for l in d.get('layers', [])] if isinstance(d.get('layers'), list) else list(d.keys()))
"
echo "applet's name: $(busctl --user list | grep GlobalMenuApplet)"
echo "registrar's name: $(busctl --user list | grep -i "AppMenu.Registrar")"
echo "== with the applet, the last app focused"; prop
echo "-- gtk3:"; cat "$out/gtk3.log"; echo "-- gtk4:"; cat "$out/gtk4.log"
echo "-- what the panel asked the apps for:"; grep -A3 "member=" "$out/monitor.log" | grep -v "^--" | sed -n 1,40p
kill $monitor
echo "== the applet removed"
dconf write /org/mate/panel/general/object-id-list "$ids"
sleep 3
echo "applet's name: $(busctl --user list | grep GlobalMenuApplet)"
echo "-- gtk3:"; cat "$out/gtk3.log"; echo "-- gtk4:"; cat "$out/gtk4.log"
echo "== gtk4 closed"; kill $gtk4; sleep 1; prop
echo "== panel log"; grep -i "appmenu\|caminito\|critical\|warning" "$out/panel.log" | head -20
} >>"$out/report" 2>&1
kill $panel $gtk3 2>/dev/null; sleep 0.5; kill $caminito; wait $caminito 2>/dev/null
