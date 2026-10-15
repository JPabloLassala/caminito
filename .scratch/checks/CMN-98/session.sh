#!/usr/bin/env bash
# The session run.sh starts, on a session bus of its own: a private bus for the power-profiles
# mock, a panel with the applet alone on it, then each step.
set -u
root=$1; work=$2; out=$3
new=org.freedesktop.UPower.PowerProfiles
new_path=/org/freedesktop/UPower/PowerProfiles
old=net.hadess.PowerProfiles
old_path=/net/hadess/PowerProfiles

DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
# start_mock <template> <bus name> <object path>
start_mock() {
  "$CAMINITO_PYTHON" -m dbusmock --system --template "$1" >"$work/mock.log" 2>&1 &
  mock=$!
  for _ in $(seq 100); do
    gdbus introspect --system --dest "$2" --object-path "$3" >/dev/null 2>&1 && break
    sleep 0.1
  done
}
# prop <name> <path> <property> <GVariant text>: a change the daemon makes by itself.
prop() {
  gdbus call --system --dest "$1" --object-path "$2" --method org.freedesktop.DBus.Properties.Set \
    "$1" "$3" "$4" >/dev/null
}
# active <name> <path>: the mock's ActiveProfile.
active() {
  gdbus call --system --dest "$1" --object-path "$2" --method org.freedesktop.DBus.Properties.Get \
    "$1" ActiveProfile | sed -e "s/^(<'//" -e "s/'>,)$//"
}
shot() { import -window root "$out/$1.png"; }
alive() { if kill -0 "$panel" 2>/dev/null; then echo "$1: panel running"; else echo "$1: PANEL GONE"; fi; }
differ() { echo "$3: $(compare -metric AE "$out/$1.png" "$out/$2.png" null: 2>&1 | cut -d" " -f1 || true) pixels differ"; }
# The popup's window, found as CMN-106 did: the one that's 320 to 360 wide, under the panel.
popup_geometry() {
  local w X Y WIDTH HEIGHT found=
  for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
    eval "$(xdotool getwindowgeometry --shell "$w")"
    if [ "$WIDTH" -ge 300 ] && [ "$WIDTH" -le 399 ] && [ "$Y" -ge 20 ]; then
      found="$X $Y $WIDTH $HEIGHT"
    fi
  done
  echo "$found"
}
# tile <index> <count>: the pointer on that tile of the row that's <count> wide, the popup's
# border being 8 and the tiles 6 apart.
tile() {
  set -- $(popup_geometry) "$1" "$2"
  local x=$1 y=$2 w=$3 i=$5 n=$6
  local tile_w=$(( (w - 16 - 6 * (n - 1)) / n ))
  xdotool mousemove $(( x + 8 + i * (tile_w + 6) + tile_w / 2 )) $(( y + 8 + 28 ))
}
open_popup() { xdotool mousemove 300 250; sleep 0.3; xdotool mousemove 14 14 click 1; sleep 1.2; }
close_popup() { xdotool mousemove 300 250 click 1; sleep 0.8; }

start_mock upower_power_profiles_daemon $new $new_path

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 28
dconf write /org/mate/panel/general/object-id-list "['power']"
dconf write /org/mate/panel/objects/power/object-type "'applet'"
dconf write /org/mate/panel/objects/power/applet-iid \
  "'CaminitoPowerAppletFactory::CaminitoPowerApplet'"
dconf write /org/mate/panel/objects/power/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/power/position 0

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4

# Three profiles, Balanced active.
open_popup
echo "popup (x y w h): $(popup_geometry)"
shot 1-three-profiles

# A click on Performance.
tile 2 3 && sleep 0.3 && xdotool click 1
sleep 1
echo "click on Performance: the mock's ActiveProfile is $(active $new $new_path)"
shot 2-clicked
# A click on the tile that's pressed leaves it pressed.
tile 2 3 && sleep 0.3 && xdotool click 1
sleep 1
echo "click on the pressed tile: the mock's ActiveProfile is $(active $new $new_path)"
shot 2b-pressed-again
differ 2-clicked 2b-pressed-again "the pressed tile stays pressed (0 expected)"
differ 1-three-profiles 2-clicked "Balanced to Performance (more than 0 expected)"

# A change from outside: powerprofilesctl set power-saver.
prop $new $new_path ActiveProfile "<'power-saver'>"; sleep 1
echo "outside change: the mock's ActiveProfile is $(active $new $new_path)"
shot 3-outside
differ 2-clicked 3-outside "the pressed tile moved (more than 0 expected)"
prop $new $new_path ActiveProfile "<'performance'>"; sleep 1

# Why Performance is degraded.
prop $new $new_path PerformanceDegraded "<'high-operating-temperature'>"; sleep 1
shot 4-degraded-hot
prop $new $new_path PerformanceDegraded "<'lap-detected'>"; sleep 1
shot 5-degraded-lap

# Who holds a profile, with the degraded line above it and without.
prop $new $new_path ActiveProfileHolds \
  "<[{'ApplicationId': <'org.gnome.GameMode'>, 'Profile': <'performance'>, 'Reason': <'a game'>}]>"
sleep 1
shot 6-held
prop $new $new_path PerformanceDegraded "<''>"; sleep 1
shot 6b-held-only
differ 6-held 6b-held-only "the degraded line went (more than 0 expected)"
prop $new $new_path ActiveProfileHolds "<@aa{sv} []>"; sleep 1
shot 6c-released
differ 6b-held-only 6c-released "the holder's line went (more than 0 expected)"

# Two profiles: no Performance, and the active one is Balanced.
prop $new $new_path Profiles "<[{'Profile': <'power-saver'>, 'Driver': <'test'>}, {'Profile': <'balanced'>, 'Driver': <'test'>}]>"
prop $new $new_path ActiveProfile "<'balanced'>"; sleep 1
shot 7-two-profiles
tile 1 2 && sleep 0.3 && xdotool click 1; sleep 0.3
tile 0 2 && sleep 0.3 && xdotool click 1; sleep 1
echo "two profiles, a click on Power Saver: the mock's ActiveProfile is $(active $new $new_path)"
shot 7b-two-clicked
alive "two profiles"

# The daemon stopped, and started again.
kill "$mock"; wait "$mock" 2>/dev/null; sleep 1.5
shot 8-no-daemon
differ 7b-two-clicked 8-no-daemon "the section went (more than 0 expected)"
alive "daemon stopped"
start_mock upower_power_profiles_daemon $new $new_path
sleep 2
shot 9-daemon-back
differ 8-no-daemon 9-daemon-back "the tiles came back (more than 0 expected)"
alive "daemon started again"

# Only the earlier bus name.
kill "$mock"; wait "$mock" 2>/dev/null; sleep 1.5
start_mock power_profiles_daemon $old $old_path
sleep 2
shot 10-old-name
differ 9-daemon-back 10-old-name "the old name's tiles (0 expected: the same three)"
tile 2 3 && sleep 0.3 && xdotool click 1; sleep 1
echo "the old name, a click on Performance: the mock's ActiveProfile is $(active $old $old_path)"
alive "old name"

kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
kill "$mock" 2>/dev/null
kill "$(cat "$work/system-bus.pid")" 2>/dev/null
wait 2>/dev/null
