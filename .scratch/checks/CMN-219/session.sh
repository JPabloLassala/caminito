#!/usr/bin/env bash
# The session run.sh starts on an Xvfb of its own, on a bus of its own: mate-panel with the
# Dock applet alone on a bottom panel of 48 px, with caja.desktop kept in its instance's
# settings. It reports whether the panel lives, and the keys the instance wrote.
set -u
dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'bottom'"
dconf write /org/mate/panel/toplevels/panel/size 48
dconf write /org/mate/panel/general/object-id-list "['dock']"
dconf write /org/mate/panel/objects/dock/object-type "'applet'"
dconf write /org/mate/panel/objects/dock/applet-iid "'CaminitoDockAppletFactory::CaminitoDockApplet'"
dconf write /org/mate/panel/objects/dock/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/dock/position 0
dconf write /org/mate/panel/objects/dock/prefs/kept "['caja.desktop']"
mate-panel >"$1/panel.log" 2>&1 &
panel=$!
sleep 6
if kill -0 "$panel" 2>/dev/null; then echo "the panel lives with the Dock applet"
else echo "THE PANEL IS GONE"; fi
echo "the instance's keys:"
dconf dump /org/mate/panel/objects/dock/
kill "$panel" 2>/dev/null
wait "$panel" 2>/dev/null
grep -E "CRITICAL|WARNING.*(dock|Dock)" "$1/panel.log" | sed 's/^/panel log: /'
