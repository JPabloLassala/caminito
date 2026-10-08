#!/usr/bin/env bash
# The session run.sh starts: Caminito nested, the panel with the applet, and two windows of
# the editor.
set -u
root=$1; work=$2; editor=$3; out=$work/out; mkdir -p "$out"
RUST_LOG=${RUST_LOG:-info,caminito::dbus::menu_claim=debug} "$root/target/debug/caminito" 2>"$out/caminito.log" &
caminito=$!
for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$out/caminito.log" && break; sleep 0.1; done
line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$out/caminito.log" | head -1)
export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland
unset DISPLAY
for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done

MATE_PANEL_APPLETS_DIR=$work/applets GSETTINGS_SCHEMA_DIR=$work/schemas mate-panel >"$out/panel.log" 2>&1 &
panel=$!
sleep 4
ids=$(dconf read /org/mate/panel/general/object-id-list)
dconf write /org/mate/panel/objects/appmenu/object-type "'applet'"
dconf write /org/mate/panel/objects/appmenu/applet-iid "'AppmenuAppletFactory::AppmenuApplet'"
dconf write /org/mate/panel/objects/appmenu/toplevel-id "'top'"
dconf write /org/mate/panel/objects/appmenu/position 300
dconf write /org/mate/panel/general/object-id-list "${ids%]}, 'appmenu']"

open_window() {
  "$editor" --ozone-platform=wayland --user-data-dir="$work/user" --extensions-dir="$work/extensions" \
    --builtin-extensions-dir="$work/builtin" --new-window "$1" >>"$out/editor.log" 2>&1
}
open_window "$work/a"
sleep 3
open_window "$work/b"

cat <<TEXT
Check, with the panel's File menu:
  - it shows New Text File, and the item opens an editor in the focused one of the two windows;
  - after Developer: Reload Window in the focused window, the menu comes back with nothing refocused.
Press Enter to stop.
TEXT
read -r _

echo "== the claims Caminito logged"
grep -i "claim" "$out/caminito.log" | tail -20
echo "== the extension's log"
find "$work/user/logs" -name '*Caminito Global Menu*' -exec cat {} + 2>/dev/null
pkill -f -- "--user-data-dir=$work/user" 2>/dev/null
kill $panel 2>/dev/null; sleep 0.5; kill $caminito; wait $caminito 2>/dev/null
