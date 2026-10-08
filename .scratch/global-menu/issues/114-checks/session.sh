#!/usr/bin/env bash
# The session run.sh starts: Caminito nested, an app, and the panel with the applet, which is
# killed, started again and then left without the applet.
set -u
here=$1; root=$2; work=$3; out=$work/out; rm -rf "$out"; mkdir -p "$out"
prefs=/org/mate/panel/objects/appmenu/prefs
switch() {
  busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState --json=short |
    python3 -c "import json,sys; print(json.loads(json.loads(sys.stdin.read())['data'][0])['global_menu_switch'])"
}
state() { echo "== $1: the switch is $(switch), and the app heard: $(tr '\n' ' ' <"$out/gtk3.log")"; }
panel() {
  MATE_PANEL_APPLETS_DIR=$work/applets GSETTINGS_SCHEMA_DIR=$work/schemas mate-panel >>"$out/panel.log" 2>&1 &
  panel=$!
}
shown() {
  GSETTINGS_SCHEMA_DIR=$work/schemas gsettings get "org.valapanel.appmenu:$prefs/" show-application-name
}
"$root/target/debug/caminito" 2>"$out/caminito.log" &
caminito=$!
for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$out/caminito.log" && break; sleep 0.1; done
line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$out/caminito.log" | head -1)
export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland
unset DISPLAY
for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
{
python3 "$here/../110-checks/app.py" 3.0 "$out/gtk3.log" & gtk3=$!
sleep 2
panel
sleep 4
ids=$(dconf read /org/mate/panel/general/object-id-list)
dconf write /org/mate/panel/objects/appmenu/object-type "'applet'"
dconf write /org/mate/panel/objects/appmenu/applet-iid "'AppmenuAppletFactory::AppmenuApplet'"
dconf write /org/mate/panel/objects/appmenu/toplevel-id "'top'"
dconf write /org/mate/panel/objects/appmenu/position 300
dconf write /org/mate/panel/general/object-id-list "${ids%]}, 'appmenu']"
sleep 4
state "the applet added"
echo "show-application-name of a new applet: $(shown)"
dconf write $prefs/show-application-name false
sleep 1
echo "show-application-name turned off: $(shown), and the panel runs: $(kill -0 $panel && echo yes)"
dconf write $prefs/show-application-name true
sleep 1
kill -9 $panel
sleep 1
state "1 s after the panel was killed"
sleep 3
panel
sleep 4
state "the panel started again 4 s after"
sleep 9
state "13 s after it started"
dconf write /org/mate/panel/general/object-id-list "$ids"
sleep 3
state "3 s after the applet was removed"
sleep 9
state "12 s after the applet was removed"
echo "== panel log"; grep -i "appmenu\|caminito\|critical" "$out/panel.log" | head -10
} >"$out/report" 2>&1
kill $panel $gtk3 2>/dev/null; sleep 0.5; kill $caminito; wait $caminito 2>/dev/null
