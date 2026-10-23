#!/usr/bin/env bash
# CMN-209's experiments, questions 1 to 3, against a private PipeWire.
#
#   .scratch/checks/CMN-209/experiments.sh
#
# Prints what it finds; the answers in CMN-209 quote this output.

here=$(cd "$(dirname "$0")" && pwd)
export CMN209_ROOT=${CMN209_ROOT:-$(mktemp -d)}
export here

exec dbus-run-session -- bash -c '
. "$here/private-pipewire.sh"
dropin="$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d/50-restore-target.conf"

restore_target() { wpctl settings node.stream.restore-target | sed -n "s/^ *Value: //p"; }
persistent() { pw-metadata -n persistent-sm-settings 0 node.stream.restore-target 2>/dev/null | grep -o "value:[^ ]*" || echo "(none)"; }

# move_close_reopen NAME: plays NAME, moves it to the headset, closes it,
# reopens it, and prints where it plays each time.
move_close_reopen() {
    play_app "$1" 2; sleep 0.8
    local idx; idx=$(pactl -f json list sinks | python3 -c "import json,sys; print([s[\"index\"] for s in json.load(sys.stdin) if s[\"name\"]==\"headset\"][0])")
    echo "  starts on: $(sink_of "$1")"
    pactl move-sink-input "$(sink_input_of "$1")" "$idx"; sleep 0.4
    echo "  moved by index to sink $idx: $(sink_of "$1")"
    wait $APP_PID; sleep 1.5
    echo "  stored: $(state_file | grep "application.name:$1=" | grep -o "\"target\":\"[^\"]*\"" || echo "no target")"
    play_app "$1" 2; sleep 0.8
    echo "  reopened on: $(sink_of "$1")"
    wait $APP_PID
}

echo "== versions"
wireplumber --version | tail -1
pipewire --version | tail -1

pw_start; make_sinks
pactl info | grep "Server Name"

echo
echo "== 1a. wpctl settings --save node.stream.restore-target false"
echo "  before: value $(restore_target), persistent $(persistent)"
wpctl settings --save node.stream.restore-target false
sleep 0.5
echo "  after: value $(restore_target), persistent $(persistent)"
echo "  state files: $(ls "$XDG_STATE_HOME/wireplumber")"
echo "  sm-settings file:"; sed "s/^/    /" "$XDG_STATE_HOME/wireplumber/sm-settings" 2>/dev/null
wp_restart
echo "  after a WirePlumber restart: value $(restore_target), persistent $(persistent)"
pw_stop; pw_start; make_sinks
echo "  after a restart of the whole stack (a new login): value $(restore_target)"

echo
echo "== 3. a move with it off"
move_close_reopen Rhythmbox

echo
echo "== 2a. volume and mute restore with it off"
play_app Totem 2; sleep 0.8
pactl set-sink-input-volume "$(sink_input_of Totem)" 40%
pactl set-sink-input-mute "$(sink_input_of Totem)" 1
sleep 0.3; wait $APP_PID; sleep 1.5
play_app Totem 2; sleep 0.8
pactl -f json list sink-inputs | python3 -c "
import json, sys
for si in json.load(sys.stdin):
    if si[\"properties\"].get(\"application.name\") == \"Totem\":
        print(\"  reopened: volume\", si[\"volume\"][\"front-left\"][\"value_percent\"], \"mute\", si[\"mute\"])
"
wait $APP_PID

echo
echo "== 2b. a target stored while it was on, with it off"
wpctl settings node.stream.restore-target true; sleep 0.3
echo "  turned on for now (not saved): value $(restore_target)"
move_close_reopen Audacious
wpctl settings node.stream.restore-target false; sleep 0.3
echo "  turned off again: value $(restore_target)"
play_app Audacious 2; sleep 0.8
echo "  Audacious reopened, its stored target still there: $(sink_of Audacious)"
wait $APP_PID

echo
echo "== 1b. undoing --save"
wpctl settings --delete node.stream.restore-target; sleep 0.5
echo "  --delete: value $(restore_target), persistent $(persistent)"
wp_restart
echo "  after a WirePlumber restart: value $(restore_target)"
echo "  Audacious stored target comes back into force:"
play_app Audacious 2; sleep 0.8
echo "  Audacious reopened: $(sink_of Audacious)"
wait $APP_PID

echo
echo "== 1c. --reset"
wpctl settings --save node.stream.restore-target false; sleep 0.3
wpctl settings --reset node.stream.restore-target; sleep 0.5
echo "  --save false, then --reset: value $(restore_target), persistent $(persistent)"
wp_restart
echo "  after a WirePlumber restart: value $(restore_target), persistent $(persistent)"
wpctl settings --delete node.stream.restore-target; sleep 0.3

echo
echo "== 1d. writing the sm-settings metadata directly"
pw-metadata -n sm-settings 0 node.stream.restore-target false Spa:String:JSON > /dev/null
sleep 0.3
echo "  sm-settings written: value $(restore_target)"
wp_restart
echo "  after a WirePlumber restart: value $(restore_target)"
pw-metadata -n persistent-sm-settings 0 node.stream.restore-target false Spa:String:JSON > /dev/null
sleep 0.3
echo "  persistent-sm-settings written: value $(restore_target), persistent $(persistent)"
wp_restart
echo "  after a WirePlumber restart: value $(restore_target)"
wpctl settings --delete node.stream.restore-target; sleep 0.3
wp_restart
echo "  --delete and restart: value $(restore_target)"

echo
echo "== 1e. a user drop-in under ~/.config/wireplumber"
cat > "$dropin" <<EOF
wireplumber.settings = {
  node.stream.restore-target = false
}
EOF
echo "  written, no restart: value $(restore_target)"
wp_restart
echo "  after a WirePlumber restart: value $(restore_target)"
move_close_reopen Clementine
wpctl settings --save node.stream.restore-target true; sleep 0.3
wp_restart
echo "  drop-in false and --save true, after a restart: value $(restore_target)"
wpctl settings --delete node.stream.restore-target; sleep 0.3
rm "$dropin"
wp_restart
echo "  drop-in removed, after a restart: value $(restore_target)"

pw_cleanup
'
